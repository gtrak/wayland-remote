# Phase 4: TCP Frame Streaming - Research

**Researched:** 2026-03-10
**Domain:** TCP networking, binary protocols, async Rust
**Confidence:** HIGH

## Summary

Phase 4 implements a TCP server that streams raw RGBA frames from the compositor to connected viewers. This requires integrating Tokio-based async networking with the existing calloop event loop used for Wayland protocol handling.

**Primary recommendation:** Use `tokio::net::TcpListener` with `tokio-util::codec` for length-prefixed binary framing. Track multiple surfaces using `Arc<RwLock<HashMap>>` shared state. Handle disconnections via task cancellation and proper cleanup of channel senders.

---

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STREAM-01 | TCP server accepts connections from Windows viewer | Tokio TcpListener with configurable port binding |
| STREAM-02 | Frame header is sent (width, height, timestamp, size) | Length-prefixed binary protocol with fixed 16-byte header |
| STREAM-03 | Raw RGBA pixel data is streamed over TCP | Direct write of RGBA bytes after header |
| STREAM-04 | Multiple surfaces can be tracked and streamed | HashMap<surface_id, FrameData> with unique window IDs |

</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio` | 1.40 (workspace) | Async runtime for TCP server | Already in workspace, stable API |
| `tokio-util` | 0.7+ | Codec for binary framing | Standard for protocol framing in Tokio |
| `bytes` | 1.x | Buffer management | Efficient byte handling, used by tokio-util |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `calloop::futures` | 0.14 | Executor for integrating Tokio with calloop | When spawning Tokio tasks from calloop event loop |
| `tokio::sync::RwLock` | built-in | Thread-safe shared state | Protecting multi-surface tracking map |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tokio-util codec | Manual `AsyncRead`/`AsyncWrite` framing | Codec is battle-tested, handles edge cases |
| Custom framing | Length-prefix (tokio-util default) | Length-prefix is standard for binary protocols |
|tokio::sync::Mutex| tokio::sync::RwLock | RwLock allows concurrent readers |
| Unbounded channels | Bounded mpsc with backpressure | Bounded prevents memory exhaustion on slow clients |

---

## Architecture Patterns

### Recommended Project Structure

```
crates/server/
├── src/
│   ├── streaming/
│   │   ├── mod.rs           # TCP server lifecycle
│   │   ├── protocol.rs      # Binary frame format definition
│   │   ├── client.rs        # Per-client connection handler
│   │   └── surface.rs       # Surface frame tracking
│   ├── state.rs             # Add streaming state to ServerState
│   └── main.rs              # Integrate streaming server
```

### Pattern 1: TCP Server with Tokio

**What:** Tokio-based TCP listener accepting connections in a loop
**When to use:** Any TCP server handling multiple clients
**Example:**

```rust
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn start_streaming_server(port: u16) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    
    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New viewer connection: {}", addr);
        
        // Spawn task for each client
        tokio::spawn(handle_client(socket));
    }
}

async fn handle_client(mut socket: TcpStream) {
    // Handle client connection
}
```

**Source:** Tokio documentation, docs.rs/tokio/latest/tokio/net/struct.TcpListener.html

### Pattern 2: Binary Frame Protocol (Length-Prefix)

**What:** Fixed-size header followed by variable payload
**When to use:** Custom binary protocols over TCP
**Header format (16 bytes):**

| Field | Size | Type | Description |
|-------|------|------|-------------|
| window_id | 4 bytes | u32 | Unique surface identifier |
| width | 4 bytes | u32 | Frame width in pixels |
| height | 4 bytes | u32 | Frame height in pixels |
| timestamp | 8 bytes | u64 | Unix timestamp in microseconds |

**Payload:** Raw RGBA bytes (width × height × 4)

**Example encoding:**

```rust
use bytes::{BufMut, BytesMut};

#[derive(Debug, Clone)]
pub struct FrameHeader {
    pub window_id: u32,
    pub width: u32,
    pub height: u32,
    pub timestamp_us: u64,
}

impl FrameHeader {
    pub const SIZE: usize = 20; // 4 + 4 + 4 + 8 bytes
    
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_u32(self.window_id);
        buf.put_u32(self.width);
        buf.put_u32(self.height);
        buf.put_u64(self.timestamp_us);
    }
    
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE { return None; }
        Some(Self {
            window_id: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
            width: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
            height: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            timestamp_us: u64::from_be_bytes([buf[12], buf[13], buf[14], buf[15], buf[16], buf[17], buf[18], buf[19]]),
        })
    }
}
```

**Source:** tokio.rs tutorial on framing, various binary protocol implementations

### Pattern 3: Multi-Client Shared State

**What:** Using `Arc<RwLock<HashMap>>` to share state across spawned tasks
**When to use:** Multiple connections need to access shared data
**Example:**

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Clone)]
pub struct StreamingState {
    surfaces: Arc<RwLock<HashMap<u32, FrameData>>>,
    clients: Arc<RwLock<HashMap<SocketAddr, ClientHandle>>>,
}

pub struct FrameData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp_us: u64,
}
```

**Source:** tokio.rs tutorial on shared state

### Pattern 4: Backpressure with Bounded Channels

**What:** Using bounded mpsc channels to prevent memory exhaustion from slow clients
**When to use:** Streaming data to potentially slow clients
**Example:**

```rust
use tokio::sync::mpsc;

// Bounded channel - sender waits when full
let (tx, rx) = mpsc::channel::<FrameData>(32); // 32 frame buffer

// In client handler:
// Try to send, drop if channel full (backpressure)
if tx.try_send(frame).is_err() {
    // Client too slow, either drop or queue
    tracing::warn!("Client backpressure - dropping frame");
}
```

**Source:** tokio.rs tutorial on channels

### Pattern 5: Graceful Disconnection

**What:** Handling client disconnection without crashing compositor
**When to use:** Network connections that can drop unexpectedly
**Example:**

```rust
async fn handle_client(socket: TcpStream, state: StreamingState) {
    let result = async {
        // Read frames from client
        // Handle writes
        Ok::<(), io::Error>(())
    }.await;
    
    match result {
        Ok(_) => tracing::info!("Client disconnected normally"),
        Err(e) => tracing::debug!("Client error: {}", e),
    }
    // Cleanup happens automatically when task ends
    // Remove from clients map if tracked
}
```

**Key insight:** Tokio tasks are cancelled when the sender is dropped. Use this for cleanup - just remove the sender from the clients map and the task will terminate naturally.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Message framing | Custom boundary detection | `tokio-util::codec` | Handles partial reads, edge cases |
| Buffer allocation | Vec with manual growth | `bytes::BytesMut` | Zero-copy where possible, efficient |
| Async TCP | std::net with threads | `tokio::net::TcpListener` | Non-blocking, integrates with async |
| Timer wheel | Custom implementation | `tokio::time` | Built-in, efficient |

---

## Common Pitfalls

### Pitfall 1: Memory Exhaustion from Slow Clients
**What goes wrong:** Server runs out of memory when client network is slow
**Why it happens:** Unbounded queue of frames to send
**How to avoid:** Use bounded mpsc channel with `try_send()` or explicit backpressure
**Warning signs:** Memory usage growing steadily, slow response times

### Pitfall 2: Byte Order Inconsistency
**What goes wrong:** Frames appear scrambled on receiver
**Why it happens:** Different endianness between server and client
**How to avoid:** Always use explicit byte order (big-endian / network byte order)
**Warning signs:** Periodic visual glitches, width/height swapped

### Pitfall 3: Connection State Leak on Disconnect
**What goes wrong:** Stale data remains after client disconnects
**Why it happens:** Not cleaning up client-specific state on task end
**How to avoid:** Use `Arc` with weak references or explicit cleanup on task end
**Warning signs:** Growing memory, stale windows in viewer

### Pitfall 4: Mixing calloop and Tokio Event Loops
**What goes wrong:** TCP server doesn't integrate with Wayland event loop
**Why it happens:** Two separate event loops don't share execution context
**How to avoid:** Use `calloop::futures` executor to run Tokio tasks within calloop
**Warning signs:** Events not processed, hangs on shutdown

---

## Code Examples

### Integrating Tokio TCP Server with calloop

```rust
use calloop::futures::Scheduler;
use std::sync::Arc;

// In ServerState::new():
// Create scheduler for futures
let (executor, scheduler) = calloop::futures::executor();

// Insert scheduler into event loop
event_loop.handle()
    .insert_source(scheduler, |fut, _, state| {
        // Futures complete here
    })
    .expect("Failed to insert scheduler");

// Later, spawn TCP server as a future
let streaming_future = async move {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:6080").await?;
    loop {
        let (socket, _) = listener.accept().await?;
        // Handle connection...
    }
};

// Schedule the future
executor.spawn(Box::pin(streaming_future));
```

### Reading Captured Frames from ServerState

```rust
// ServerState already has: captured_frames: HashMap<ObjectId, RgbaData>
// Need to convert to streaming format

use wayland_remote_server::rendering::pixel_export::RgbaData;

struct StreamingFrame {
    pub window_id: u32,
    pub width: u32,
    pub height: u32,
    pub timestamp_us: u64,
    pub data: Vec<u8>,
}

impl ServerState {
    pub fn get_frames_for_streaming(&self) -> Vec<StreamingFrame> {
        self.captured_frames
            .iter()
            .enumerate()
            .map(|(idx, (surface_id, rgba))| {
                // Get dimensions from offscreen buffer
                let (width, height) = self.offscreen_buffers
                    .get(surface_id)
                    .map(|buf| (buf.width() as u32, buf.height() as u32))
                    .unwrap_or((0, 0));
                
                StreamingFrame {
                    window_id: idx as u32, // Map surface to window ID
                    width,
                    height,
                    timestamp_us: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64,
                    data: rgba.as_bytes().to_vec(),
                }
            })
            .collect()
    }
}
```

---

## Open Questions

1. **Window ID Mapping**
   - What we know: Surfaces are tracked by `ObjectId`, need stable IDs for streaming
   - What's unclear: How to create stable numeric IDs that survive surface recreation
   - Recommendation: Use incrementing counter for new surfaces, clean up on surface destroy

2. **Frame Rate Control**
   - What we know: Phase 3 has frame callbacks that trigger rendering
   - What's unclear: Should streaming throttle to network capacity or run at compositor rate?
   - Recommendation: Run at compositor rate initially, add throttling if needed

3. **Protocol Versioning**
   - What we know: Initial protocol is simple header + RGBA
   - What's unclear: How to negotiate protocol version with viewer?
   - Recommendation: Add protocol version to initial handshake, defer for MVP

---

## Sources

### Primary (HIGH confidence)
- Tokio TcpListener documentation - docs.rs/tokio/latest/tokio/net/struct.TcpListener.html
- Tokio shared state tutorial - tokio.rs/tokio/tutorial/shared-state
- Tokio channels tutorial - tokio.rs/tokio/tutorial/channels
- Tokio framing tutorial - tokio.rs/tokio/tutorial/framing
- calloop futures module - docs.rs/calloop/latest/calloop/futures/index.html

### Secondary (MEDIUM confidence)
- Binary protocol framing patterns - Various implementations (length-prefix standard)
- WebSocket server with Tokio - oneuptime.com/blog (pattern reference)

### Tertiary (LOW confidence)
- calloop + tokio integration - General pattern, needs verification for this specific use case

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Tokio 1.40, tokio-util, bytes all stable and documented
- Architecture: HIGH - Well-established patterns for TCP servers in Rust
- Pitfalls: MEDIUM - Common async pitfalls well-documented, calloop+tk integration needs care

**Research date:** 2026-03-10
**Valid until:** 2026-04-10 (30 days for stable domain)