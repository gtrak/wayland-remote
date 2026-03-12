# Phase 5: Windows Viewer Foundation - Research

**Researched:** 2026-03-11
**Domain:** Windows GUI, TCP client, GDI rendering, Rust Win32
**Confidence:** HIGH

## Summary

Phase 5 implements the Windows viewer application that connects to the Linux server via TCP and displays streamed frames in native Win32 windows using GDI. This requires integrating async TCP networking with Windows message loop handling.

**Primary recommendation:** Use `tokio` for async TCP client, `winit` 0.30.x for window management, and Win32 GDI (`StretchDIBits`) for frame display. Implement a message-only window for network communication and separate display windows for each surface.

---

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| VIEW-01 | Windows application connects to TCP server | Tokio TcpStream with async connection handling |
| VIEW-02 | Received frames displayed in Win32 windows using GDI | GDI StretchDIBits with BITMAPINFO, RGBA→BGRA conversion |

</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio` | 1.40 (workspace) | Async runtime for TCP client | Already in workspace, handles async networking |
| `winit` | 0.30.x | Cross-platform window creation | Specified in ROADMAP, handles event loop |
| `windows-sys` | 0.52+ | Raw Win32 API bindings | Standard for GDI, minimal overhead |
| `bytes` | 1.x | Buffer management | Efficient byte handling, integrates with tokio |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio-util` | 0.7+ | Codec for binary framing | Decode length-prefixed frames |
| `anyhow` | 1.x | Error handling | Contextual errors across async boundaries |
| `tracing` | 0.1.x | Logging | Structured logging for viewer diagnostics |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| winit | Raw Win32 API (`CreateWindowEx`) | Winit abstracts message loop, recommended for MVP |
| GDI StretchDIBits | Direct2D/Direct3D | GDI is simpler, sufficient for MVP |
| RGBA | BGRA native | GDI expects BGRA, conversion needed |
| Async TCP | Blocking std::net | Async prevents UI freezing on slow network |

---

## Architecture Patterns

### Recommended Project Structure

```
crates/viewer/
├── src/
│   ├── main.rs              # Application entry, event loop
│   ├── app.rs               # ViewerApp state management
│   ├── network/
│   │   ├── mod.rs           # TCP client module
│   │   ├── protocol.rs      # Frame header parsing
│   │   └── client.rs        # Async TCP connection handler
│   ├── display/
│   │   ├── mod.rs           # Window/display management
│   │   ├── window.rs        # Per-window GDI rendering
│   │   └── gdi.rs           # GDI helpers (StretchDIBits)
│   └── lib.rs
```

### Pattern 1: Async TCP Client with Tokio

**What:** Async TCP client connecting to server and receiving frames
**When to use:** Any TCP client that must not block UI
**Example:**

```rust
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, BufReader};
use bytes::BytesMut;

pub struct TcpClient {
    stream: TcpStream,
    buffer: BytesMut,
}

impl TcpClient {
    pub async fn connect(addr: &str) -> anyhow::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?; // Disable Nagle for low latency
        
        Ok(Self {
            stream,
            buffer: BytesMut::with_capacity(4096),
        })
    }
    
    pub async fn read_frame(&mut self) -> anyhow::Result<Frame> {
        // Read header first (20 bytes)
        let mut header = [0u8; 20];
        self.stream.read_exact(&mut header).await?;
        
        let window_id = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let width = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
        let height = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
        let timestamp = u64::from_be_bytes([header[12], header[13], header[14], 
                                             header[15], header[16], header[17], 
                                             header[18], header[19]]);
        
        // Read RGBA payload
        let data_len = (width * height * 4) as usize;
        let mut rgba_data = vec![0u8; data_len];
        self.stream.read_exact(&mut rgba_data).await?;
        
        Ok(Frame {
            window_id,
            width,
            height,
            timestamp,
            rgba_data,
        })
    }
}
```

**Key:** Use `read_exact` to ensure complete header/payload reading.

**Source:** Tokio documentation, Phase 4 protocol spec

### Pattern 2: Binary Frame Protocol Parsing

**What:** Parse 20-byte big-endian header and variable RGBA payload
**When to use:** Custom binary protocols with fixed header
**Protocol Format:**

| Field | Size | Type | Description |
|-------|------|------|-------------|
| window_id | 4 bytes | u32 (BE) | Unique surface identifier |
| width | 4 bytes | u32 (BE) | Frame width in pixels |
| height | 4 bytes | u32 (BE) | Frame height in pixels |
| timestamp | 8 bytes | u64 (BE) | Unix timestamp in microseconds |
| payload | width×height×4 | [u8] | Raw RGBA pixel data |

**Example parsing:**

```rust
use bytes::Buf;

#[derive(Debug, Clone)]
pub struct FrameHeader {
    pub window_id: u32,
    pub width: u32,
    pub height: u32,
    pub timestamp_us: u64,
}

impl FrameHeader {
    pub const SIZE: usize = 20;
    
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        
        Some(Self {
            window_id: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
            width: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
            height: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            timestamp_us: u64::from_be_bytes([buf[12], buf[13], buf[14], buf[15],
                                              buf[16], buf[17], buf[18], buf[19]]),
        })
    }
    
    pub fn payload_size(&self) -> usize {
        (self.width * self.height * 4) as usize
    }
}
```

**Source:** Phase 4 protocol spec, bytes crate documentation

### Pattern 3: Winit Window Creation (Recommended)

**What:** Cross-platform window creation with Windows backend
**When to use:** Need event loop integration, window management
**Example:**

```rust
use winit::application::ApplicationHandler;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

pub struct ViewerApp {
    windows: HashMap<WindowId, WindowState>,
    network_tx: mpsc::Sender<NetworkCommand>,
}

impl ApplicationHandler for ViewerApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        // Create initial window or message-only window
    }
    
    fn window_event(&mut self, 
                   event_loop: &winit::event_loop::ActiveEventLoop,
                   window_id: WindowId,
                   event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.windows.remove(&window_id);
            }
            WindowEvent::RedrawRequested => {
                // Trigger GDI redraw
            }
            _ => {}
        }
    }
}

fn main() -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = ViewerApp::new();
    
    event_loop.run_app(&mut app)?;
    Ok(())
}
```

**Key:** Winit 0.30 uses new ApplicationHandler trait pattern.

**Source:** winit 0.30 documentation, examples

### Pattern 4: Raw Win32 Window (Alternative)

**What:** Direct Win32 API window creation
**When to use:** Need full control over window class, message loop
**Example:**

```rust
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub unsafe fn create_window(title: &str, width: i32, height: i32) -> HWND {
    let instance = GetModuleHandleW(std::ptr::null());
    
    let class_name: Vec<u16> = "WaylandViewerWindow\0".encode_utf16().collect();
    let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class_name.as_ptr(),
        ..std::mem::zeroed()
    };
    
    RegisterClassW(&wc);
    
    CreateWindowExW(
        0,
        class_name.as_ptr(),
        title_wide.as_ptr(),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        CW_USEDEFAULT, CW_USEDEFAULT,
        width, height,
        0,
        0,
        instance,
        std::ptr::null(),
    )
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            // GDI rendering here
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
```

**Tradeoff:** More control but more boilerplate. Prefer winit unless specific Win32 features needed.

**Source:** windows-sys documentation, Win32 API reference

### Pattern 5: GDI Frame Display with StretchDIBits

**What:** Display RGBA frame using GDI StretchDIBits
**When to use:** Software rendering of bitmap data to window
**Example:**

```rust
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Foundation::*;

pub struct GdiRenderer {
    hwnd: HWND,
    width: u32,
    height: u32,
}

impl GdiRenderer {
    pub unsafe fn render_rgba(&self, rgba_data: &[u8]) {
        let hdc = GetDC(self.hwnd);
        
        // RGBA to BGRA conversion (GDI expects BGRA)
        let mut bgra_data = vec![0u8; rgba_data.len()];
        for i in (0..rgba_data.len()).step_by(4) {
            bgra_data[i] = rgba_data[i + 2];     // B = R
            bgra_data[i + 1] = rgba_data[i + 1]; // G = G
            bgra_data[i + 2] = rgba_data[i];     // R = B
            bgra_data[i + 3] = rgba_data[i + 3]; // A = A
        }
        
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: self.width as i32,
                biHeight: -(self.height as i32), // Negative = top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default(); 1],
        };
        
        StretchDIBits(
            hdc,
            0, 0, self.width as i32, self.height as i32, // dest
            0, 0, self.width as i32, self.height as i32, // src
            bgra_data.as_ptr() as *const _,
            &bmi,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        
        ReleaseDC(self.hwnd, hdc);
    }
}
```

**Key:** GDI expects bottom-up BGRA by default. Use negative biHeight for top-down or convert.

**Source:** Microsoft GDI documentation, BITMAPINFOHEADER reference

### Pattern 6: Async/Window Event Loop Integration

**What:** Run async TCP client alongside winit event loop
**When to use:** Need both async networking and GUI event loop
**Approach 1: Spawn Tokio runtime in separate thread:**

```rust
use tokio::runtime::Runtime;
use std::thread;

fn main() {
    // Spawn network thread with Tokio runtime
    let (frame_tx, frame_rx) = std::sync::mpsc::channel::<Frame>();
    
    thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut client = TcpClient::connect("localhost:6080").await.unwrap();
            loop {
                let frame = client.read_frame().await.unwrap();
                frame_tx.send(frame).unwrap();
            }
        });
    });
    
    // Run winit event loop in main thread
    let event_loop = EventLoop::new().unwrap();
    // ... process frame_rx in window_event
}
```

**Approach 2: Use winit's EventLoop::create_proxy() with user events:**

```rust
use winit::event::Event;

enum UserEvent {
    FrameReceived(Frame),
    ConnectionLost,
}

fn main() {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    
    // Spawn async task that sends events to proxy
    tokio::spawn(async move {
        let mut client = TcpClient::connect("localhost:6080").await.unwrap();
        loop {
            match client.read_frame().await {
                Ok(frame) => { proxy.send_event(UserEvent::FrameReceived(frame)).ok(); }
                Err(_) => { proxy.send_event(UserEvent::ConnectionLost).ok(); break; }
            }
        }
    });
    
    event_loop.run(move |event, elwt| {
        match event {
            Event::UserEvent(UserEvent::FrameReceived(frame)) => {
                // Update window
            }
            _ => {}
        }
    }).unwrap();
}
```

**Recommendation:** Approach 2 (user events) is cleaner but requires winit 0.30+. Approach 1 is simpler for MVP.

**Source:** winit documentation, tokio runtime docs

### Pattern 7: Double Buffering for Smooth Display

**What:** Buffer incoming frames, swap on redraw to prevent tearing
**When to use:** Frame rate mismatches between network and display
**Example:**

```rust
pub struct DisplayWindow {
    front_buffer: Option<Vec<u8>>,   // Currently displayed
    back_buffer: Option<Vec<u8>>,    // Next frame being received
    dimensions: (u32, u32),
}

impl DisplayWindow {
    pub fn submit_frame(&mut self, frame: Frame) {
        // Swap to back buffer
        self.back_buffer = Some(frame.rgba_data);
        self.dimensions = (frame.width, frame.height);
        
        // Request redraw (will swap buffers)
        self.request_redraw();
    }
    
    pub fn on_paint(&mut self) {
        // Swap buffers during paint
        if let Some(new_frame) = self.back_buffer.take() {
            self.front_buffer = Some(new_frame);
        }
        
        if let Some(ref buffer) = self.front_buffer {
            self.render_to_window(buffer);
        }
    }
}
```

**Key:** Prevents displaying partial frame updates.

**Source:** Standard double-buffering pattern

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Window creation | Raw Win32 API manually | `winit` | Handles message loop, resizing, DPI |
| Async runtime | Thread-per-connection | `tokio` | Efficient, battle-tested |
| TCP framing | Manual byte reading | `tokio::io::AsyncReadExt::read_exact` | Handles partial reads |
| RGBA conversion | Custom SIMD | Simple loop | GDI is bottleneck, not conversion |
| Event loop | Custom Win32 loop | `winit::event_loop` | Cross-platform, handles edge cases |

---

## Common Pitfalls

### Pitfall 1: RGBA vs BGRA Color Swapping
**What goes wrong:** Colors appear swapped (red/blue)
**Why it happens:** GDI expects BGRA, protocol sends RGBA
**How to avoid:** Convert RGBA to BGRA before StretchDIBits, or use proper BITMAPINFO
**Warning signs:** Blue appears red, red appears blue

### Pitfall 2: Window Message Loop Blocking
**What goes wrong:** UI freezes while connecting to server
**Why it happens:** Blocking TCP connect in main thread
**How to avoid:** Use async TCP in separate thread or tokio runtime
**Warning signs:** Window not responding, white/unpainted content

### Pitfall 3: Memory Leaks from GetDC Without ReleaseDC
**What goes wrong:** Memory exhaustion, GDI handle exhaustion
**Why it happens:** Forgetting ReleaseDC after GetDC
**How to avoid:** Use RAII pattern or careful cleanup
**Warning signs:** "Out of memory" errors, crashes after long runtime

### Pitfall 4: Incorrect biHeight Sign
**What goes wrong:** Image appears upside-down
**Why it happens:** Positive biHeight means bottom-up DIB
**How to avoid:** Set biHeight to negative for top-down, or flip rows manually
**Warning signs:** Image vertically flipped

### Pitfall 5: Frame Tearing Without Double Buffering
**What goes wrong:** Visual artifacts, partial frame updates visible
**Why it happens:** Updating displayed buffer while being read
**How to avoid:** Implement front/back buffer swap
**Warning signs:** Horizontal tearing, flickering

### Pitfall 6: Window Resize Handling
**What goes wrong:** Stretched/squashed frames on resize
**Why it happens:** Not updating display dimensions
**How to avoid:** Track window size, use StretchDIBits with proper dest rect
**Warning signs:** Distorted aspect ratio

---

## Open Questions

1. **Window Lifecycle**
   - What we know: Each surface maps to HWND (Phase 6)
   - What's unclear: How to handle window creation from network events
   - Recommendation: Use message-only window for network, spawn display windows on first frame

2. **Frame Timing**
   - What we know: Timestamp in header for latency measurement
   - What's unclear: Should viewer throttle frame display?
   - Recommendation: Display all received frames, throttle at server if needed

3. **Error Recovery**
   - What we know: TCP connection can drop
   - What's unclear: Should viewer auto-reconnect?
   - Recommendation: Show "Disconnected" state, manual reconnect for MVP

4. **Multiple Windows**
   - What we know: Phase 6 adds multi-surface support
   - What's unclear: How to manage multiple winit windows
   - Recommendation: Design for HashMap<window_id, Window> from start

---

## Sources

### Primary (HIGH confidence)
- winit 0.30 documentation - docs.rs/winit/0.30
- windows-sys documentation - docs.rs/windows-sys/0.52
- Microsoft GDI reference - docs.microsoft.com/en-us/windows/win32/gdi
- Tokio TCP client docs - docs.rs/tokio/latest/tokio/net/struct.TcpStream.html

### Secondary (MEDIUM confidence)
- BITMAPINFOHEADER structure - microsoft.com (well-documented, stable API)
- StretchDIBits function - microsoft.com (standard GDI pattern)

### Tertiary (LOW confidence)
- Winit + Tokio integration patterns - Community examples, varies by version

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - winit, tokio, windows-sys are stable
- Architecture: HIGH - Standard async + GUI patterns
- Pitfalls: MEDIUM - GDI specifics well-documented, winit+tokio integration needs testing

**Research date:** 2026-03-11
**Valid until:** 2026-04-11 (30 days for stable domain)