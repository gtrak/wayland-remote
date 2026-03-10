# Architecture Research: Wayland Remote Compositor

**Domain:** Remote Desktop / Wayland Compositor / Frame Streaming
**Researched:** 2025-03-10
**Confidence:** MEDIUM (based on training data + Smithay docs knowledge, recommend verification with current Smithay/examples)

## Standard Wayland Compositor Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Wayland Protocol Layer                    │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ wl_display  │  │ wl_registry │  │ wl_compositor       │ │
│  │ (singleton) │  │ (discovery) │  │ (surface manager)   │ │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
│         │                │                    │            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ wl_seat     │  │ xdg_shell   │  │ presentation_time   │ │
│  │ (input)     │  │ (windows)   │  │ (sync)              │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                    Compositor Core (Smithay)                 │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐    │
│  │                 Anvil (or custom)                    │    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐          │    │
│  │  │ Space    │  │ Backend  │  │ Renderer │          │    │
│  │  │ (layout) │  │ (drm/virt)│  │ (pixman/GL)│         │    │
│  │  └──────────┘  └──────────┘  └──────────┘          │    │
│  └─────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────┤
│                    Platform Abstraction                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │ GBM/DRM  │  │ Pixman   │  │ EGL/GL   │                   │
│  │ (buffer) │  │ (sw)     │  │ (hw)     │                   │
│  └──────────┘  └──────────┘  └──────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **Wayland Protocol** | Communication with clients | `wayland-server` crate, protocol generation |
| **Compositor State** | Track all protocol objects | `State` struct in Smithay holding all globals |
| **xdg_shell** | Window management, sizing, activation | Implement `XdgShellHandler` trait |
| **wl_seat** | Input device aggregation | Implement `SeatHandler`, forward events |
| **Renderer** | Convert surfaces to pixels | `MultiRenderer`, `GlesRenderer`, or `PixmanRenderer` |
| **Backend** | Platform integration | `Virtual` backend for headless operation |
| **Frame Capture** | Extract rendered frames | `Renderer::blit_to` or DMA-BUF export |

## Recommended Project Structure

```
wayland-remote/
├── Cargo.toml              # Workspace definition
├── compositor/             # Headless Wayland compositor (Linux)
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── state.rs        # Smithay State implementation
│   │   ├── protocols/      # Wayland protocol handlers
│   │   │   ├── xdg_shell.rs
│   │   │   ├── seat.rs
│   │   │   └── shm.rs
│   │   ├── renderer/       # Frame capture and encoding
│   │   │   ├── buffer.rs
│   │   │   └── capture.rs
│   │   ├── streaming/      # TCP streaming server
│   │   │   ├── server.rs
│   │   │   ├── protocol.rs # Binary frame protocol
│   │   │   └── encoder.rs  # RGBA encoding (later H264)
│   │   └── config.rs       # Settings
│   └── Cargo.toml
├── viewer/                 # Windows display client
│   ├── src/
│   │   ├── main.rs         # Win32 entry
│   │   ├── window.rs       # HWND management
│   │   ├── decoder.rs      # Frame decoding
│   │   ├── input.rs        # Mouse/keyboard capture
│   │   └── network.rs      # TCP client
│   └── Cargo.toml
└── proto/                  # Shared protocol definitions
    ├── src/
    │   └── lib.rs          # Frame packet types
    └── Cargo.toml
```

### Structure Rationale

- **compositor/:** Isolated Wayland server, only dependency is Smithay + network
- **viewer/:** Windows-specific code, no Wayland knowledge needed
- **proto/:** Shared serialization between client/server
- **protocol/:** Separated handlers make it clear which Wayland features we support
- **streaming/:** Pluggable encoder allows H264/AV1 later without compositor changes

## Architectural Patterns

### Pattern 1: Headless Compositor (Virtual Backend)

**What:** Compositor that renders to offscreen buffers without physical display
**When to use:** Remote display, testing, offscreen rendering
**Trade-offs:** No hardware composition acceleration, more GPU memory usage

**Smithay Implementation:**
```rust
// Use Virtual backend for no physical display
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::egl::EGLDevice;

pub struct HeadlessState {
    display: Display<Self>,
    renderer: PixmanRenderer,
    shm_state: ShmState,
    xdg_shell_state: XdgShellState,
    seat_state: SeatState,
    // Window ID mapping for streaming
    windows: HashMap<WindowId, SurfaceId>,
}
```

### Pattern 2: Surface-to-Window Mapping

**What:** Map Wayland surfaces to Windows HWNDs 1:1
**When to use:** Each Linux app window becomes a native Windows window
**Trade-offs:** Complex to implement (lifecycle management), best UX

**Data Flow:**
```
Wayland Surface (xdg_toplevel)
        ↓ (commit signal)
Compositor Renderer → Pixel buffer (RGBA)
        ↓ (frame callback)
Streaming Server → TCP → Viewer Window (HWND)
        ↑ (input events)
Mouse/Keyboard ← Windows messages
```

### Pattern 3: Frame Capture on Damage

**What:** Only capture surfaces when content changes
**When to use:** Optimize bandwidth, enable partial updates later
**Trade-offs:** Complexity in tracking damaged regions

**Implementation:**
```rust
// Listen for surface commits
fn surface_commit(surface: &WlSurface) {
    if has_damage(surface) {
        let buffer = capture_surface(surface);
        stream_frame(surface.id(), buffer);
    }
}
```

## Data Flow

### Frame Flow (Compositor → Viewer)

```
Wayland Client App
        ↓ (wl_surface::commit)
Compositor State
        ↓ (Smithay renderer)
Offscreen Buffer (RGBA)
        ↓ (read_pixels or DMA-BUF)
Frame Queue
        ↓ (TCP socket)
Viewer Decoder
        ↓ (GDI StretchDIBits)
Windows HWND
```

### Input Flow (Viewer → Compositor → App)

```
Windows Input (WM_MOUSEMOVE, WM_KEYDOWN)
        ↓
Input Capture (viewer input.rs)
        ↓ (TCP)
Compositor Input Server
        ↓ (wl_seat event)
Wayland Client App
```

### Surface Lifecycle Flow

```
Client creates xdg_toplevel
        ↓
Compositor: xdg_shell::new_toplevel()
        ↓
Allocate WindowId, notify viewer (new window)
        ↓
Viewer creates HWND with matching WindowId
        ↓
Surface commits → frames → that HWND
        ↓
Client closes window
        ↓
Compositor notifies viewer (destroy window)
```

## Build Order Implications

Based on component dependencies:

### Phase 1: Protocol Foundation
1. **Shared Protocol** (proto/) — Frame packet types, surface IDs
2. **Compositor Core** (compositor/src/state.rs) — Empty State struct
3. **Protocol Handlers** (compositor/src/protocols/) — SHM, basic wl_surface

**Why first:** Everything else depends on these types

### Phase 2: Rendering Pipeline
1. **Virtual Backend** — Offscreen Pixman renderer
2. **Surface Renderer** — Render single surface to buffer
3. **Frame Capture** — Extract RGBA pixels

**Depends on:** Protocol foundation (needs surfaces to render)

### Phase 3: Streaming Server
1. **TCP Server** — Accept viewer connections
2. **Frame Protocol** — Serialize captured frames
3. **Window Management** — Track surface-to-ID mapping

**Depends on:** Rendering pipeline (needs frames to stream)

### Phase 4: Windows Viewer
1. **TCP Client** — Connect to compositor
2. **Window Manager** — Create/destroy HWNDs per surface
3. **Frame Display** — GDI StretchDIBits
4. **Input Capture** — Windows messages → compositor

**Depends on:** Streaming server (needs protocol defined)

**Can parallelize:** Phases 2-3 can be developed simultaneously with Phase 4 once protocol is defined

## Scaling Considerations

| Concern | Single User | Multi-User | High-Density |
|---------|-------------|------------|--------------|
| **Frame encoding** | Raw RGBA (~50MB/s for 1080p) | H264 essential | Hardware encoding |
| **Connection** | Single TCP socket | Connection pooling | UDP + reliability layer |
| **Memory** | 2-3 frame buffers | LRU cache for inactive windows | Shared GPU memory |
| **Latency** | Acceptable for LAN | Frame pacing needed | Edge deployment |

### Scaling Priorities

1. **First bottleneck:** Bandwidth — raw RGBA saturates fast
   - Fix: Add H264/AV1 encoding post-MVP
   
2. **Second bottleneck:** Latency — frame delivery timing
   - Fix: Presentation time protocol, frame pacing

3. **Third bottleneck:** Input lag — round-trip time
   - Fix: Predictive input, local cursor rendering

## Anti-Patterns

### Anti-Pattern 1: Implementing Protocol Proxy Instead of Frame Streaming

**What people do:** Try to proxy Wayland protocol directly to Windows
**Why it's wrong:** Windows doesn't have Wayland stack; need full compositor emulation
**Do this instead:** Frame streaming (what we're doing) — render on Linux, display on Windows

### Anti-Pattern 2: Rendering on Windows Side

**What people do:** Send GPU commands (OpenGL/Vulkan) to Windows
**Why it's wrong:** Requires GPU on Windows side, complex synchronization, security issues
**Do this instead:** CPU-rendered frames (Pixman) for MVP, investigate GPU readback later

### Anti-Pattern 3: Blocking on Frame Capture

**What people do:** Synchronously read pixels in render thread
**Why it's wrong:** Stalls Wayland compositor, degrades all client performance
**Do this instead:** Asynchronous frame capture with triple buffering

### Anti-Pattern 4: DRM/KMS Backend for Headless

**What people do:** Try to use physical GPU output even for headless
**Why it's wrong:** Requires real display connection, limits deployment
**Do this instead:** Use Smithay's `Virtual` backend with offscreen buffers

## Integration Points

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| **Compositor ↔ Streaming** | Channel/mpsc | Frame buffers + metadata, don't block render loop |
| **Streaming ↔ Network** | Async TCP | Handle backpressure, drop frames if needed |
| **Viewer Network ↔ Display** | Lock-free queue | Decouple network from 60Hz display thread |
| **Viewer Display ↔ Input** | Windows messages | Hook window proc for input capture |

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| **Wayland Clients** | Unix socket | Standard WAYLAND_DISPLAY mechanism |
| **SSH Tunnel** | Port forward | Forward compositor TCP port to localhost |
| **Audio** | Out of scope (defer) | PipeWire could stream audio separately |

## Sources

- Smithay documentation and examples: https://smithay.github.io/smithay/
- Anvil compositor (Smithay reference implementation): https://github.com/Smithay/smithay/tree/master/anvil
- Wayland protocol specifications: https://wayland.freedesktop.org/docs/html/
- Virtual backend in Smithay: `smithay::backend::virtual`
- XDG Shell protocol: xdg-shell.xml (stable window management)
- wlroots headless backend (reference design): https://gitlab.freedesktop.org/wlroots/wlroots/-/blob/master/backend/headless

**Confidence Notes:**
- Architecture patterns: HIGH (standard Wayland/Smithay patterns)
- Build order: MEDIUM (depends on team velocity)
- Scaling thresholds: LOW (estimated, need benchmarking)

---
*Architecture research for: Wayland Remote Compositor*
*Researched: 2025-03-10*
