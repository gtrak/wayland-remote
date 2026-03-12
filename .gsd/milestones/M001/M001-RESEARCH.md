# Project Research Summary

**Project:** Wayland Remote Compositor
**Domain:** Remote Desktop / Wayland Compositor / Frame Streaming
**Researched:** 2025-03-10
**Confidence:** MEDIUM-HIGH

## Executive Summary

This project involves building a headless Wayland compositor that captures rendered application frames and streams them over TCP to a Windows viewer. The architecture follows the proven pattern of remote display systems like `wprs`, but simplifies to frame streaming rather than full Wayland protocol proxying. The recommended approach uses **Smithay 0.7.0** — the mature Rust Wayland compositor framework that powers COSMIC, niri, and other production compositors — combined with software rendering via Pixman for the MVP. This avoids GPU complexity while delivering functional remote display capabilities.

The research reveals a clear technical path: implement core Wayland protocols (compositor, XDG shell, seat) using Smithay's trait-based handlers, render to offscreen buffers with PixmanRenderer, and stream raw RGBA frames over TCP to a Windows client that displays them using Win32 GDI. The Windows viewer maps each Wayland surface to a native HWND, creating a seamless multi-window experience. Video encoding (H.264/AV1) and hardware acceleration should be deferred until the core frame streaming pipeline is validated, as raw RGBA is sufficient for LAN use and avoids significant encoding complexity.

Key risks center on Wayland protocol compliance — specifically frame callback timing, buffer lifecycle management, and XDG shell configure/ack synchronization. These pitfalls are well-documented in the Wayland ecosystem and have caused issues in similar projects. Prevention requires strict adherence to protocol semantics: always respond to frame callbacks (but throttle to network capacity), release buffers only after transmission is complete, and respect the XDG configure/ack handshake. The Smithay framework provides abstractions that help avoid these issues, but careful attention to protocol details remains essential.

## Key Findings

### Recommended Stack

The stack centers on Smithay 0.7.0 as the compositor framework, chosen for its maturity, active maintenance, and proven use in production compositors. Software rendering with PixmanRenderer is recommended for the MVP to avoid GPU driver complexity and debugging challenges. Tokio provides async networking for the TCP frame streaming server, while the Windows viewer uses winit for window management and Win32 GDI for frame display.

**Core technologies:**
- **smithay 0.7.0**: Wayland compositor framework — native Rust, powers production compositors like COSMIC and niri
- **calloop 0.14.0**: Event loop — callback-based, integrates seamlessly with Smithay
- **pixman 0.2.x**: Software rendering — CPU-based renderer ideal for headless/offscreen compositing without GPU requirements
- **tokio 1.40+**: Async runtime — industry standard for Rust async networking, TCP server implementation
- **image 0.25.x**: Image encoding — converting rendered frames to RGBA for streaming
- **winit 0.30.x**: Window creation — stable Win32 window management for Windows viewer
- **windows 0.58+**: Win32 API bindings — official Microsoft crate for GDI access (StretchDIBits, CreateDIBSection)

**Deferred technologies:**
- **Video encoding (H.264/AV1)**: Defer to post-MVP; adds latency and complexity
- **Hardware acceleration (dmabuf/GPU)**: Defer until basic functionality is stable
- **Waypipe/protocol proxy**: Avoid; frame streaming is simpler and more robust

### Expected Features

The feature set prioritizes correctness and usability over advanced capabilities. The MVP must support basic window management, input, and frame streaming. Competitive features like video encoding, clipboard sync, and session persistence are explicitly out of scope for v1.

**Must have (table stakes):**
- **Core Wayland Protocol** — Applications must connect and create surfaces (wl_compositor, wl_surface, wl_seat, wl_output)
- **XDG Shell Support** — Desktop window management (maximize, minimize, fullscreen, close) via xdg_wm_base
- **Basic Input** — Keyboard and mouse interaction with keymap handling via xkbcommon
- **Raw Frame Streaming** — Raw RGBA over TCP for simplest path to working product
- **Windows Viewer** — Native Win32 application displaying frames in HWNDs
- **Cursor Handling** — Visible cursor that tracks properly, forwarding cursor updates from clients

**Should have (competitive):**
- **Damage Tracking** — Send only changed regions to reduce bandwidth (post-MVP)
- **Video Encoding (H.264)** — 10-100x bandwidth reduction for WAN usage (post-MVP)
- **Clipboard Synchronization** — Copy-paste between local and remote (post-MVP)
- **XWayland Support** — Run X11 applications remotely (post-MVP)

**Defer (v2+):**
- **Session Persistence** — Disconnect/reconnect without losing state; complex state management
- **Hardware Acceleration** — dmabuf zero-copy; driver complexity not needed for correctness
- **Multi-Monitor** — Multiple virtual displays; can simulate with multiple connections
- **Audio Forwarding** — Nice-to-have but not core remote desktop value

### Architecture Approach

The architecture follows a **headless compositor with frame streaming** pattern. The Linux server runs a Smithay-based compositor without physical display, rendering applications to offscreen buffers. Captured frames flow over TCP to a Windows viewer that displays them in native HWNDs. Each Wayland surface maps 1:1 to a Windows window, providing native window management (minimize, maximize, Alt-Tab) on the Windows side.

**Major components:**

1. **Compositor Core (Smithay)** — Manages Wayland protocol state, implements handlers for xdg_shell, wl_seat, and rendering
2. **Virtual Backend** — Offscreen rendering using PixmanRenderer, no physical display required
3. **Frame Capture** — Extract RGBA pixels from rendered buffers, queue for transmission
4. **TCP Streaming Server** — Async Tokio server accepting viewer connections, sending framed packets (header + RGBA data)
5. **Windows Viewer** — winit-based application creating HWNDs per surface, displaying frames via GDI StretchDIBits
6. **Input Capture** — Windows message hook for mouse/keyboard, transmitting to compositor over TCP

**Data flow:**
```
Wayland App → wl_surface.commit → Smithay Renderer → RGBA Buffer → TCP Stream → Viewer → GDI Display
                                    ↑______________________________________________|
                                           (input events: mouse/keyboard)
```

### Critical Pitfalls

Research identified six critical pitfalls that have caused issues in similar projects. These must be addressed in their respective phases to avoid broken functionality.

1. **Frame Callback Timing Mismanagement** — Applications freeze or render at wrong rate if wl_callback.done events aren't sent at appropriate times. **Avoid:** Always respond to wl_surface.frame requests, but throttle to match network capacity, not local refresh rate.

2. **Surface Commit / Buffer Release Ordering** — Memory leaks or crashes if wl_buffer.release sent before buffer is fully transmitted. **Avoid:** Release buffers only after frame has been fully transmitted to Windows viewer; track buffer references per-surface.

3. **XDG Shell Configure / Ack Configure Synchronization** — Windows don't resize correctly if configure/ack handshake is violated. **Avoid:** Track configure serials per-surface, don't apply geometry changes until after ack_configure + commit, respect width/height values (0 means "up to you").

4. **Input Event Serial Mismatches** — Popups don't open or drag-and-drop fails if serial numbers don't match triggering events. **Avoid:** Maintain monotonically increasing serial counter per-seat, store serial from button/key events, use correct serial for xdg_toplevel operations.

5. **Keyboard Input Keymap Handling** — Wrong characters or dead modifiers if keymap not sent correctly. **Avoid:** Use xkbcommon to generate proper keymaps, send via file descriptor properly (mmap), remember evdev scancode + 8 = XKB scancode.

6. **Surface Roles and Subsurfaces** — Popups in wrong locations if parent-child relationships not tracked. **Avoid:** Use Smithay's desktop::PopupManager for popup handling, map subsurfaces as part of same HWND, implement proper popup grab dismissal.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Wayland Server Foundation
**Rationale:** All other components depend on the core protocol implementation. Must establish correct Wayland protocol semantics before adding rendering or networking.
**Delivers:** Headless compositor accepting Wayland client connections, creating surfaces, basic XDG shell window management
**Addresses:** Core Wayland Protocol, XDG Shell Support, Surface Roles
**Avoids:** XDG Configure/Ack synchronization pitfall, Surface Roles pitfall
**Research flag:** Standard pattern — Smithay provides trait-based handlers with documented implementations; use Anvil as reference

### Phase 2: Rendering Pipeline
**Rationale:** Cannot stream frames without rendering pipeline. PixmanRenderer provides software rendering without GPU complexity. Buffer lifecycle management must be correct from the start.
**Delivers:** Offscreen rendering to memory buffers, frame capture as RGBA, buffer lifecycle management
**Uses:** PixmanRenderer, image crate for pixel extraction
**Implements:** Virtual Backend, Frame Capture component
**Avoids:** Buffer Release Ordering pitfall
**Research flag:** Standard pattern — well-documented in Smithay; PixmanRenderer is straightforward

### Phase 3: TCP Frame Streaming Server
**Rationale:** Bridges compositor and viewer. Frame callback timing must coordinate with network capacity to avoid bandwidth saturation or application freezes.
**Delivers:** TCP server accepting viewer connections, binary frame protocol (header + RGBA), window ID mapping, frame throttling based on network conditions
**Uses:** Tokio async runtime, bytes crate for buffers
**Implements:** Streaming Server component
**Avoids:** Frame Callback Timing pitfall, Network backpressure issues
**Research flag:** Needs validation — custom protocol design requires testing with slow networks to verify backpressure handling

### Phase 4: Windows Viewer
**Rationale:** Client-side implementation can proceed in parallel once protocol is defined. Native Windows windows provide best UX but require careful HWND lifecycle management.
**Delivers:** Windows application connecting to compositor, creating/destroying HWNDs per surface, displaying frames via GDI, capturing input
**Uses:** winit, windows crate (Win32 GDI)
**Implements:** Windows Viewer, Input Capture components
**Research flag:** Standard pattern — well-documented Win32 patterns, but DPI scaling and pixel format (RGB vs BGR) need testing

### Phase 5: Input Handling Integration
**Rationale:** Input requires careful serial tracking and keymap handling. Deferring until after basic frame streaming ensures core pipeline works before adding interactivity complexity.
**Delivers:** Bidirectional input (keyboard/mouse from Windows to Linux), proper serial tracking, XKB keymap handling
**Avoids:** Input Event Serial Mismatches pitfall, Keyboard Keymap pitfall
**Research flag:** Needs research — XKB integration has subtleties (evdev scancodes, keymap FDs, modifier tracking)

### Phase 6: Bandwidth Optimization (Post-MVP)
**Rationale:** Raw RGBA saturates bandwidth quickly (~50MB/s for 1080p). Damage tracking and video encoding address this but add significant complexity.
**Delivers:** Damage tracking (send only changed regions), H.264 encoding option, adaptive quality based on network conditions
**Uses:** Additional encoder dependencies (ffmpeg bindings or hardware encode)
**Research flag:** Needs research — encoding integration has tradeoffs between latency and bandwidth

### Phase Ordering Rationale

- **Protocol before Rendering:** Cannot capture frames without surfaces to render; Smithay protocol handlers must be in place first
- **Rendering before Streaming:** Streaming server needs frames to send; establishes data flow direction
- **Server before Viewer:** Viewer depends on protocol format defined by streaming server; can develop in parallel once protocol spec is stable
- **Input last:** Input is bidirectional complexity; ensure unidirectional frame flow works first
- **Optimization last:** Bandwidth optimization is enhancement, not core functionality; proves product works before optimizing

This ordering also allows incremental validation — each phase produces demonstrable progress (protocol acceptance, rendering output, network transmission, visible windows) before proceeding.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (TCP Frame Streaming):** Custom protocol design needs validation with slow networks and high-latency scenarios; backpressure handling is critical
- **Phase 5 (Input Handling):** XKB integration has subtleties; serial tracking requires careful testing with diverse applications (GTK, Qt, SDL)
- **Phase 6 (Bandwidth Optimization):** Encoding integration tradeoffs between latency and bandwidth need benchmarking

Phases with standard patterns (skip research-phase):
- **Phase 1 (Wayland Server Foundation):** Well-documented Smithay patterns; Anvil reference implementation provides working template
- **Phase 2 (Rendering Pipeline):** PixmanRenderer is straightforward; buffer management patterns are standard
- **Phase 4 (Windows Viewer):** Win32 GDI patterns are mature and documented; winit provides standard window management

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Smithay 0.7.0 is latest stable with clear documentation; Tokio is industry standard; versions verified via crates.io |
| Features | MEDIUM-HIGH | Core Wayland features well-documented; remote-specific features based on wprs/Xpra analysis but scope is intentionally limited |
| Architecture | MEDIUM | Patterns are standard for Smithay/Wayland; build order logical but actual integration complexity may vary |
| Pitfalls | HIGH | Based on official Wayland documentation, The Wayland Book, and Smithay docs; these are well-known protocol requirements |

**Overall confidence:** MEDIUM-HIGH

The stack and pitfalls are well-understood with authoritative sources. Architecture follows established patterns, though the specific integration of frame streaming with Smithay's rendering pipeline needs validation during implementation. Feature scope is intentionally conservative (raw RGBA, no encoding for MVP), reducing risk.

### Gaps to Address

- **Protocol performance at scale:** Raw RGBA bandwidth requirements (~50MB/s for 1080p) may be prohibitive even on LAN; need early bandwidth testing to determine if Phase 6 (encoding) should be accelerated
- **Application compatibility:** Different toolkits (GTK, Qt, SDL) may have varying protocol compliance; plan for testing with diverse applications to surface edge cases
- **Windows DPI/HiDPI handling:** Research indicates DPI awareness is needed but specific implementation details depend on Windows version and display configuration; requires testing on actual hardware
- **Buffer synchronization timing:** The exact timing of buffer release (after transmission vs. after display) may need tuning based on application behavior; plan for instrumentation
- **Smithay 0.7.0 API stability:** Released June 2025; while stable, some APIs may evolve; pin exact version and review changelog before updates

## Sources

### Primary (HIGH confidence)
- Smithay v0.7.0 Release and docs.rs — Core compositor framework APIs and patterns
- Wayland Protocol Specification — Official protocol semantics, frame callbacks, buffer lifecycle
- The Wayland Book — Comprehensive Wayland development guide, pitfalls, best practices
- XDG Shell Protocol — Window management protocol specification
- Calloop 0.14.0 Documentation — Event loop integration patterns

### Secondary (MEDIUM confidence)
- wprs Project (wayland-transpositor/wprs) — Reference architecture for Rust-based remote Wayland compositor
- Anvil Compositor (Smithay reference implementation) — Working example of Smithay patterns
- winit 0.30.x Releases — Windows window management APIs
- Xpra Features Documentation — Remote desktop feature analysis

### Tertiary (LOW confidence)
- Scaling thresholds and bandwidth estimates — Based on calculations, need empirical validation
- Windows DPI handling specifics — General guidance, implementation details require testing
- Application toolkit compatibility — Assumed based on protocol compliance, edge cases unknown

---
*Research completed: 2025-03-10*
*Ready for roadmap: yes*

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

# Stack Research: Wayland Remote Compositor

**Domain:** Wayland Compositor with Remote Frame Streaming
**Researched:** 2025-03-10
**Confidence:** HIGH

## Overview

This stack is designed for building a headless Wayland compositor in Rust that captures rendered frames and streams them over TCP to a Windows viewer. The architecture follows the pattern established by `wprs` (Wayland compositor using Smithay for remote display), but with simplified frame streaming instead of full protocol proxying.

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **smithay** | 0.7.0 | Wayland compositor framework | Native Rust compositor library; powers COSMIC, niri, and wprs. Version 0.7.0 (June 2025) is latest stable with significant improvements over 0.6.x including updated DRM syncobj and XWayland support. | HIGH |
| **calloop** | 0.14.0 | Event loop | Callback-based event loop from Smithay team; integrates seamlessly with Smithay's architecture. Required dependency of Smithay. | HIGH |
| **wayland-server** | 0.31.x | Wayland protocol server | Core Wayland server implementation; Smithay 0.7.0 depends on wayland-server ^0.31.9. | HIGH |
| **wayland-protocols** | 0.32.x | Wayland protocol definitions | Required for xdg-shell, dmabuf, and other protocols. Smithay 0.7.0 uses ^0.32.8. | HIGH |
| **rustix** | 1.0.x | Safe Rust bindings to POSIX/Linux syscalls | Updated in Smithay 0.7.0; replaces unsafe libc calls. | HIGH |

### Rendering & Frame Capture

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **pixman** | 0.2.x | Software rendering | CPU-based renderer for headless/offscreen compositing. Smithay's PixmanRenderer works without GPU requirements. Ideal for MVP before adding GPU acceleration. | HIGH |
| **gbm** | 0.18.x | Generic Buffer Management | Required for dmabuf and hardware buffer operations. Smithay 0.7.0 uses gbm ^0.18.0. | MEDIUM |
| **image** | 0.25.x | Image encoding/decoding | Standard Rust image library for converting rendered frames to RGBA for streaming. | HIGH |

### Networking & Serialization

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **tokio** | 1.40+ | Async runtime | Industry standard for Rust async networking. Use `tokio::net::TcpListener` and `tokio::io::AsyncWriteExt` for frame streaming server. | HIGH |
| **bytes** | 1.7+ | Byte buffer handling | Efficient buffer management for frame data; integrates with Tokio. | HIGH |
| **thiserror** | 2.0.x | Error handling | Smithay 0.7.0 updated to thiserror 2.0; maintain consistency. | HIGH |
| **tracing** | 0.1.x | Logging | Smithay uses tracing extensively; configure with `max_level_trace` for debug, `release_max_level_debug` for release builds. | HIGH |

### Windows Client (Viewer)

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **winit** | 0.30.x | Window creation | Current stable (0.30.13, March 2025) for Win32 window management. 0.31 beta available but stick to 0.30 for stability. | HIGH |
| **windows** | 0.58+ | Win32 API bindings | Official Microsoft crate for Win32 GDI access (`StretchDIBits`, `CreateDIBSection`). | HIGH |
| **smithay-client-toolkit** | 0.19.x | Wayland client utils | For potential future Linux viewer; not needed for Windows MVP. | LOW |

### Supporting Libraries

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| **xkbcommon** | 0.8.x | Keyboard handling | Smithay depends on this; handles keymap interpretation. | HIGH |
| **input** | 0.9.x | libinput bindings | For input device handling (optional for headless compositor). | MEDIUM |
| **slotmap** | 1.0+ | Handle allocation | Efficient handle-based storage for surfaces and windows. | HIGH |
| **parking_lot** | 0.12+ | Synchronization | Faster mutex/rwlock implementation for concurrent frame handling. | HIGH |

## System Dependencies

Required system libraries (install via package manager):

```bash
# Debian/Ubuntu
sudo apt-get install libwayland-dev libwayland-server0 libxkbcommon-dev libpixman-1-dev

# Fedora
sudo dnf install wayland-devel libxkbcommon-devel pixman-devel
```

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **wlroots** (C library) | Not Rust-native; requires FFI bindings and unsafe code | Smithay provides same functionality in safe Rust |
| **Mesa/EGL offscreen** | Overkill for MVP; adds complexity | PixmanRenderer for software rendering |
| **H264/AV1 encoding** | Out of scope for MVP; adds encoding complexity | Raw RGBA frames over TCP |
| **Waypipe** | Protocol proxy approach adds complexity; different architecture | Frame streaming approach with custom protocol |
| **Vulkan GPU rendering** | Premature optimization; defer until basic functionality works | PixmanRenderer |
| **Async-std** | Less ecosystem integration than Tokio | Tokio |
| **winit 0.31 beta** | Still in beta; 0.30 is stable and sufficient | winit 0.30.x |

## Installation

```toml
# Cargo.toml
[dependencies]
# Core compositor
smithay = { version = "0.7.0", features = ["wayland_frontend", "desktop", "xwayland"] }
calloop = "0.14.0"
wayland-server = "0.31.10"
wayland-protocols = { version = "0.32.8", features = ["server"] }

# Rendering
smithay = { version = "0.7.0", features = ["backend_pixman"] }
image = "0.25"

# Networking (server-side)
tokio = { version = "1.40", features = ["full"] }
bytes = "1.7"

# Error handling & logging
thiserror = "2.0"
tracing = { version = "0.1", features = ["max_level_trace", "release_max_level_debug"] }
tracing-subscriber = "0.3"

# Windows client (separate crate)
[target.'cfg(windows)'.dependencies]
winit = "0.30"
windows = { version = "0.58", features = ["Win32_Graphics_Gdi", "Win32_UI_WindowsAndMessaging"] }

# Utilities
slotmap = "1.0"
parking_lot = "0.12"
```

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| smithay@0.7.0 | wayland-server@0.31.x, wayland-protocols@0.32.x, rustix@1.0 | Verified via Smithay Cargo.toml |
| smithay@0.7.0 | calloop@0.14.0 | Required dependency |
| calloop@0.14.0 | rustix@0.38 | Uses rustix for platform abstractions |

## Key Architecture Decisions

### Frame Capture Strategy

**Recommended:** Use Smithay's `Renderer` trait with `PixmanRenderer` for offscreen rendering:

1. Create virtual outputs using `Output::new()` without physical display
2. Render surfaces to memory buffers using PixmanRenderer
3. Read back pixels as RGBA using `image` crate
4. Stream raw RGBA over TCP

**Why not dmabuf/GPU?** Defer to post-MVP. dmabuf zero-copy adds complexity and requires GPU synchronization that's difficult to debug initially.

### Event Loop Architecture

Smithay is built around `calloop`. Structure:
- Single-threaded event loop (calloop)
- Tokio integration via `calloop::futures` module (enable `executor` feature)
- TCP server runs in Tokio runtime embedded in calloop

### Network Protocol

Simple binary protocol for MVP:
- Header: window_id (u64), width (u32), height (u32), timestamp (u64)
- Body: raw RGBA bytes (width * height * 4)
- Input events sent back via separate channel

## Sources

- [Smithay v0.7.0 Release](https://github.com/Smithay/smithay/releases/tag/v0.7.0) - HIGH confidence
- [Smithay docs.rs](https://docs.rs/smithay/0.7.0/smithay/) - HIGH confidence
- [Wayland Releases](https://wayland.freedesktop.org/releases.html) - HIGH confidence
- [wprs Project](https://github.com/wayland-transpositor/wprs) - MEDIUM confidence (similar architecture)
- [Winit Releases](https://github.com/rust-windowing/winit/releases) - HIGH confidence
- [Calloop docs.rs](https://docs.rs/calloop/0.14.0/calloop/) - HIGH confidence

---
*Research for: Wayland Remote Compositor*
*Researched: 2025-03-10*

# Feature Research: Remote Wayland Compositor

**Domain:** Remote display / Wayland compositor
**Researched:** 2025-03-10
**Confidence:** MEDIUM

## Feature Landscape

### Table Stakes (Must Have)

Features users expect from any remote desktop solution. Missing these = product feels broken.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Core Wayland Protocol** | Wayland applications require wl_compositor, wl_surface, wl_seat, wl_output | MEDIUM | Smithay provides these abstractions. Must support buffer attach, commit, and surface destruction. |
| **XDG Shell Support** | Desktop applications need xdg_wm_base, xdg_surface, xdg_toplevel for window management | MEDIUM | Critical for window state (maximize, minimize, fullscreen, close). Popup support needed for menus/tooltips. |
| **Basic Input (Keyboard/Mouse)** | Users must interact with remote applications | LOW | Wayland's wl_keyboard, wl_pointer protocols. Need keymap handling with xkbcommon. |
| **Surface Rendering** | Applications draw to buffers; compositor must read and display them | MEDIUM | Requires SHM (shared memory) buffers at minimum. dmabuf for GPU acceleration. |
| **Frame Streaming** | Remote display requires transmitting rendered frames to client | LOW (raw) / HIGH (encoded) | Raw RGBA is simplest (MVP). Video encoding (H.264/AV1) adds significant complexity but saves bandwidth. |
| **Window Lifecycle Management** | Windows must map/unmap, resize, and close properly | MEDIUM | Handle configure events, acknowledge with ack_configure, manage surface destruction order. |
| **Network Transport** | Frames and input must flow between Linux server and Windows client | LOW | TCP simplest. WebSocket adds compatibility. UDP for low-latency (harder). |
| **Cursor Handling** | Users need visible, responsive cursor | LOW | Wayland expects client to set cursor image. Compositor must forward cursor updates to client. |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required but create value.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Video Encoding (H.264/AV1)** | 10-100x bandwidth reduction vs raw frames; enables WAN usage | HIGH | Requires ffmpeg or hardware encoder integration. Adds latency (encoding time). |
| **Damage Tracking** | Send only changed regions, reducing bandwidth | MEDIUM | Track surface damage rectangles. Requires compositor bookkeeping. |
| **Hardware Acceleration (dmabuf)** | Zero-copy GPU buffer sharing; lower CPU usage | HIGH | Complex driver dependencies. Not all apps support it. |
| **Clipboard Synchronization** | Copy-paste between local and remote | MEDIUM | Requires data_device protocol implementation. Needs bi-directional sync. |
| **Session Persistence** | Disconnect and reconnect without losing application state | HIGH | wprs implements this by storing compositor state. Requires careful state management. |
| **XWayland Support** | Run X11 applications remotely | MEDIUM | Requires XWayland integration. X11 has different window model (needs XWM). |
| **Multi-Monitor (Virtual Outputs)** | Multiple virtual displays | LOW-MEDIUM | Create multiple wl_output objects. Client must handle multiple windows. |
| **Touch Input** | Support touchscreen devices | LOW | Wayland's wl_touch protocol. Less critical for desktop use. |
| **Low-Latency Scheduling** | Minimize perceived lag | HIGH | Requires frame pacing, prediction, and careful buffer management. |
| **Adaptive Quality** | Adjust quality based on network conditions | MEDIUM | Dynamic bitrate/quality changes. Requires encoder support. |
| **Scroll/Pinch Gestures** | Modern touchpad support | LOW | Wayland pointer gestures protocol. Nice but not critical. |
| **Audio Forwarding** | Hear remote application audio locally | MEDIUM | PulseAudio/PipeWire integration. Separate audio stream. |

### Anti-Features (Deliberately NOT Building)

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Full Window Manager** | Scope creep. User runs this alongside their existing WM. | Implement only window mapping to HWND. Let Windows manage window chrome, stacking, etc. |
| **Built-in Authentication** | Complex security requirement. Easy to get wrong. | Document SSH tunnel setup. Security is user's responsibility. |
| **Wayland Protocol Proxy** | Waypipe approach is fragile, complex, hard to maintain | Frame streaming approach is simpler and more robust. |
| **Real-time Collaboration** | Multi-user editing is vastly more complex | Single user per connection. Future: allow multiple viewers (read-only). |
| **3D/GPU Passthrough** | Extremely complex (PCIe passthrough, driver issues) | Use software rendering. Games won't perform well anyway over network. |
| **Web Client** | Doubles implementation effort (WebRTC, browser APIs) | Focus on native Windows client. Web can be future enhancement. |
| **Recording/Playback** | Niche feature, adds complexity | Use external screen recorder on Windows side. |
| **Printer Forwarding** | Complex driver integration | Not needed for typical use cases. |

## Feature Dependencies

```
Core Wayland Protocol
    └──requires──> XDG Shell Support
        └──requires──> Window Lifecycle Management
            └──requires──> Surface Rendering
                └──requires──> Frame Streaming
                    └──requires──> Network Transport

XDG Shell Support
    └──requires──> Basic Input (for interactive resize/move)
    
Video Encoding ──enhances──> Frame Streaming (reduces bandwidth)
Damage Tracking ──enhances──> Frame Streaming (reduces data sent)
Hardware Acceleration ──enhances──> Surface Rendering (performance)

XWayland Support
    └──requires──> XDG Shell Support
    └──conflicts──> None, but adds significant complexity

Session Persistence
    └──requires──> All core features (must serialize entire state)
    └──requires──> Network Transport (reconnection logic)
```

### Dependency Notes

- **XDG Shell requires Core Wayland:** xdg_surface is a role on wl_surface
- **Video Encoding requires Frame Streaming:** Encoder takes raw frames as input
- **Damage Tracking enhances but doesn't require:** Works with both raw and encoded streaming
- **Clipboard requires data_device:** Must implement Wayland's data sharing protocol
- **Session Persistence is a "capstone" feature:** Requires all other features to be stable first

## MVP Definition

### Launch With (v1)

Minimum viable product — validate that concept works end-to-end.

- [ ] **Core Wayland Protocol** — Applications can connect and create surfaces
- [ ] **XDG Shell Support** — Windows appear and can be managed (move, resize, close)
- [ ] **Basic Input** — Keyboard and mouse work for interaction
- [ ] **Raw Frame Streaming** — Raw RGBA over TCP (simplest path)
- [ ] **Windows Viewer** — Displays frames in native Win32 windows
- [ ] **Cursor Handling** — Cursor visible and tracks properly

**Explicitly NOT in MVP:**
- Video encoding (use raw RGBA)
- Hardware acceleration (use SHM buffers)
- Damage tracking (send full frames)
- Clipboard
- XWayland
- Session persistence

### Add After Validation (v1.x)

Features to add once core works and performance is acceptable.

- [ ] **Damage Tracking** — Trigger: Bandwidth usage too high with raw frames
- [ ] **Video Encoding (H.264)** — Trigger: Need WAN support or bandwidth limits
- [ ] **Clipboard Synchronization** — Trigger: User feedback indicates need
- [ ] **XWayland Support** — Trigger: Users want to run X11 apps

### Future Consideration (v2+)

Defer until product-market fit is clear.

- [ ] **Session Persistence** — Why defer: Complex state management, MVP focus is basic connectivity
- [ ] **Hardware Acceleration (dmabuf)** — Why defer: Driver complexity, not needed for correctness
- [ ] **Multi-Monitor** — Why defer: Can simulate with multiple connections
- [ ] **Audio Forwarding** — Why defer: Nice-to-have, not core value
- [ ] **Touch Input** — Why defer: Desktop focus, touch is secondary

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Core Wayland Protocol | HIGH | MEDIUM | P1 |
| XDG Shell Support | HIGH | MEDIUM | P1 |
| Basic Input | HIGH | LOW | P1 |
| Frame Streaming (raw) | HIGH | LOW | P1 |
| Cursor Handling | MEDIUM | LOW | P1 |
| Damage Tracking | MEDIUM | MEDIUM | P2 |
| Video Encoding | HIGH | HIGH | P2 |
| Clipboard | MEDIUM | MEDIUM | P2 |
| XWayland | MEDIUM | MEDIUM | P2 |
| Hardware Acceleration | MEDIUM | HIGH | P3 |
| Session Persistence | HIGH | HIGH | P3 |
| Multi-Monitor | MEDIUM | MEDIUM | P3 |
| Audio Forwarding | LOW | MEDIUM | P3 |

**Priority key:**
- P1: Must have for launch (MVP)
- P2: Should have, add after validation
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Waypipe | wprs | Xpra | Our Approach |
|---------|---------|------|------|--------------|
| Protocol Type | Protocol proxy | Protocol proxy | Frame streaming + proxy | Frame streaming |
| Video Encoding | No | Custom (SIMD) | Yes (H.264/VP8/VP9) | Deferred (raw RGBA MVP) |
| Session Persistence | No | Yes | Yes | Deferred |
| Clipboard | Yes | Partial | Yes | Deferred |
| XWayland | Yes | Partial | Yes | Deferred |
| Audio | No | No | Yes | Out of scope |
| Multi-User | No | No | Yes | Out of scope |

**Key Insight:** Waypipe and wprs take the protocol proxy approach (forward Wayland protocol messages). This is elegant but fragile — requires implementing almost the entire Wayland protocol. Frame streaming is simpler and more robust, trading some efficiency for reliability.

## Sources

- Wayland Protocol Documentation: https://wayland.freedesktop.org/docs/html/
- XDG Shell Protocol: https://wayland.app/protocols/xdg-shell
- wprs (Wayland remote compositor in Rust): https://github.com/wayland-transpositor/wprs
- Smithay (Rust Wayland compositor library): https://github.com/Smithay/smithay
- Xpra Features Documentation: https://github.com/Xpra-org/xpra
- Wayland Protocols Explorer: https://wayland.app/protocols/

**Confidence Notes:**
- Core Wayland features: HIGH (well-documented protocols)
- Remote-specific features: MEDIUM (based on wprs/Xpra analysis, limited to public docs)
- Anti-features: MEDIUM (based on project scope and common remote desktop pitfalls)

---
*Feature research for: Wayland Remote*
*Researched: 2025-03-10*

# Pitfalls Research

**Domain:** Wayland Remote Compositor  
**Researched:** 2025-03-10  
**Confidence:** HIGH (based on official Wayland documentation, Smithay docs, and Wayland Book)

## Critical Pitfalls

### Pitfall 1: Frame Callback Timing Mismanagement

**What goes wrong:**
Applications appear to freeze, render at the wrong frame rate, or consume excessive CPU/GPU resources. In a remote compositor, this manifests as jerky updates, high latency, or bandwidth saturation from unnecessary frame generation.

**Why it happens:**
Wayland clients request frame callbacks to synchronize rendering with display refresh. If the compositor doesn't send `wl_callback.done` events at appropriate times (or sends them too frequently), clients either:
- Block waiting for callbacks that never come
- Render at maximum rate, wasting resources
- Miss their intended presentation timing

In remote compositors, there's additional complexity: the compositor must balance local frame generation with network transmission capacity.

**How to avoid:**
- Always respond to `wl_surface.frame` requests with `wl_callback.done` events
- Throttle callbacks to match the remote display's actual refresh capability, not the local compositor's rate
- Implement adaptive throttling based on network conditions (higher latency = lower frame rate)
- Never send callbacks faster than you can actually transmit frames

**Warning signs:**
- Applications using 100% CPU when idle
- Visible stuttering in animations
- Network bandwidth spikes far exceeding what raw RGBA data should require
- Input latency growing over time

**Phase to address:**
Phase 2 (TCP Frame Streaming Server) - This is where frame generation and network transmission intersect. The frame server must coordinate with the callback system.

---

### Pitfall 2: Surface Commit / Buffer Release Ordering

**What goes wrong:**
Memory leaks, visual artifacts (tearing, flickering), or application crashes when clients access buffers that have been prematurely released.

**Why it happens:**
Wayland uses a "commit then release" pattern. When a client attaches a buffer and commits:
1. The compositor must wait for the buffer to be rendered/displayed
2. Only then can it send `wl_buffer.release` to the client
3. The client can then reuse or destroy the buffer

Remote compositors add a wrinkle: the buffer must be captured, transmitted, AND displayed on the remote end before release. Getting this wrong causes:
- Reading from released buffers (segfaults)
- Holding buffers too long (memory bloat)
- Releasing before network transmission (corrupted frames)

**How to avoid:**
- Use Smithay's `CompositorHandler` which manages buffer state correctly
- Release buffers only after the frame has been fully transmitted to the Windows viewer
- Track buffer references per-surface
- Implement proper damage tracking to only update changed regions

**Warning signs:**
- Memory usage growing continuously
- Random crashes in client applications
- Visual corruption in specific applications
- Valgrind showing invalid reads in wl_shm_pool areas

**Phase to address:**
Phase 2 (TCP Frame Streaming Server) - The streaming server must own buffer lifecycle management.

---

### Pitfall 3: XDG Shell Configure / Ack Configure Synchronization

**What goes wrong:**
Windows don't resize correctly, close buttons don't work, or window state (maximized, fullscreen) doesn't sync between client and compositor.

**Why it happens:**
XDG shell requires a specific ack/configure protocol:
1. Compositor sends `xdg_surface.configure` with new size/states
2. Client must send `xdg_surface.ack_configure` with the same serial
3. Client then commits a surface with the new size
4. Only after commit does the compositor apply the new state

If the compositor doesn't wait for the ack before applying state, or if the client doesn't respect the configure, you get:
- Windows stuck at wrong sizes
- Close events ignored
- Mismatched window decorations

**How to avoid:**
- Track configure serials per-surface and wait for acks
- Don't apply window geometry changes until after `ack_configure` + commit
- Respect the width/height values in configure events (0 means "up to you")
- Handle the `close` event properly to allow applications to save state

**Warning signs:**
- Windows appear at wrong sizes after maximize/restore
- Close button not responding
- Window decorations out of sync with actual state
- GTK/Qt apps misbehaving differently than simple clients

**Phase to address:**
Phase 1 (Wayland Server Foundation) - XDG shell implementation must be correct from the start.

---

### Pitfall 4: Input Event Serial Mismatches

**What goes wrong:**
Popups don't open, drag-and-drop doesn't work, or interactive resize/move fails.

**Why it happens:**
Many Wayland operations require serial numbers from input events (button presses, key presses) to prevent race conditions. The compositor sends serials with events, and clients must use those same serials in subsequent requests.

Common mistakes:
- Using wrong serial (not the one from the triggering event)
- Serials wrapping around or being reused incorrectly
- Generating serials without corresponding events
- Timestamp/serial confusion in remote scenarios

**How to avoid:**
- Maintain a monotonically increasing serial counter per-seat
- Store the serial from button/key events and use it for subsequent operations
- Pass the correct serial to `xdg_toplevel.show_window_menu`, `move`, `resize`
- Use the serial from `wl_pointer.button` for popup grabs

**Warning signs:**
- Right-click context menus not appearing
- Window resize handles not working
- Drag operations not starting
- "Failed to get serial" errors in client logs

**Phase to address:**
Phase 3 (Input Handling) - Input system must track and correlate serials correctly.

---

### Pitfall 5: Keyboard Input Keymap Handling

**What goes wrong:**
Keyboard input produces wrong characters, modifiers don't work, or some keys are dead.

**Why it happens:**
Wayland keyboard handling is complex:
1. Compositor must send `keymap` event with XKB keymap via file descriptor
2. Key events send evdev scancodes (not ASCII/Unicode)
3. Clients must use XKB to translate scancodes to keysyms
4. Modifiers must be tracked and sent separately
5. Key repeat is client-side in Wayland

Common pitfalls:
- Sending keymap incorrectly (wrong format, FD issues)
- Not adding 8 to evdev scancodes before XKB lookup
- Missing modifier events
- Not handling keymap updates (layout switches)

**How to avoid:**
- Use `xkbcommon` to generate proper keymaps
- Send keymap via file descriptor properly (mmap)
- Remember evdev scancode + 8 = XKB scancode
- Always send modifier events when they change
- Consider key repeat implications (client handles it)

**Warning signs:**
- Keys producing wrong characters
- Shift/Ctrl/Alt not working
- Dead keys not combining
- Some applications working, others not

**Phase to address:**
Phase 3 (Input Handling) - Keyboard input requires careful XKB integration.

---

### Pitfall 6: Surface Roles and Subsurfaces

**What goes wrong:**
Context menus, tooltips, or popup windows appear in wrong locations, don't close properly, or have incorrect stacking order.

**Why it happens:**
Wayland surfaces are generic until assigned a "role" (xdg_surface, wl_subsurface, etc.). Each role has specific semantics:
- XDG toplevels are application windows
- XDG popups must have parents and use positioners
- Subsurfaces are children of another surface

Remote compositors must:
- Properly map parent-child relationships to HWNDs
- Handle popup grabs (dismissal when clicking outside)
- Position popups relative to parents, not global coordinates

**How to avoid:**
- Track surface hierarchy in Smithay
- Use `desktop::PopupManager` for popup handling
- Map subsurfaces as part of the same HWND, not separate windows
- Implement proper popup grab dismissal
- Handle `xdg_popup.reposition` for dynamic positioning

**Warning signs:**
- Menus appearing at (0, 0) or wrong location
- Tooltips detached from their widgets
- Popups not closing when clicking outside
- Z-order issues with modal dialogs

**Phase to address:**
Phase 1 (Wayland Server Foundation) - Surface role tracking must be implemented early.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Send frame callbacks at fixed 60Hz regardless of network | Simple to implement | Bandwidth waste, latency on slow networks, stuttering on fast networks | Never - always adapt to actual capacity |
| Release buffers immediately after commit | Simpler buffer management | Visual corruption, client crashes | Only in single-buffer mode (rare) |
| Ignore damage tracking, always send full frames | Easier to implement | Excessive bandwidth usage, unusable over slow connections | MVP only, must implement before release |
| Raw RGBA without compression | Simplest protocol | Bandwidth saturation even on LAN | MVP only, H264/AV1 needed for production |
| Single-threaded event loop | Simple architecture | Input latency under load, frame drops | Prototype only |
| Fixed window size (ignore configure events) | Less state management | Apps that resize don't work properly | Never - breaks core Wayland semantics |

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| **SSH tunnel** | Assuming WAYLAND_DISPLAY is always set correctly | Validate socket path, provide clear error messages, support custom paths via config |
| **Win32 GDI** | Using wrong pixel format (RGB vs BGR) | Ensure consistent pixel format between compositor capture and GDI display |
| **Windows DPI scaling** | Ignoring HiDPI settings | Handle DPI awareness, scale cursor and UI appropriately |
| **XKB keymaps** | Hardcoding US layout | Load system keymap, handle layout switches at runtime |
| **TCP sockets** | Not handling partial writes | Use proper framing, buffer unsent data, handle EAGAIN |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Blocking socket I/O | Frame drops when network hiccups | Use non-blocking sockets with calloop integration | Single user on WiFi |
| No backpressure on frame generation | Memory bloat, OOM | Pause frame callbacks when send buffer fills | >10MB/s frame data |
| Per-pixel CPU conversion | 100% CPU usage, thermal throttling | Use GPU for format conversion (Mesa/GLES) or accept BGRA | 1080p at 60fps |
| Lock contention in buffer access | Stuttering, frame time variance | Use lock-free queues, minimize critical sections | Multiple high-rate clients |
| Synchronous round-trips | Input latency | Batch operations, avoid blocking waits | Interactive use |

## Security Mistakes

Domain-specific security issues beyond general application security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| **Shared memory permissions** | Other users can read window content | Create SHM with 0600 permissions, clean up on disconnect |
| **Socket path exposure** | Arbitrary code execution via forged socket | Validate socket path ownership, use abstract sockets if possible |
| **Buffer bounds checking** | Memory disclosure or corruption | Validate all buffer dimensions, use safe wrappers |
| **Keymap FD leaks** | Resource exhaustion | Always close keymap FDs after sending |
| **Unauthenticated TCP** | Unauthorized access, screen snooping | Require SSH tunnel or implement TLS + auth for non-local |

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Cursor lag | Pointer feels disconnected from physical mouse | Implement cursor prediction or use hardware cursor when possible |
| Window focus not synced | Typing goes to wrong window | Sync focus state between Windows and Wayland compositor |
| No visual feedback on slow networks | User thinks app is frozen | Show connection status, buffer indicators |
| Clipboard not working | Can't copy/paste between local and remote | Implement clipboard integration (wl_data_device) |
| Alt-Tab trapped in remote | Can't switch back to local | Capture/release based on focus, provide escape hotkey |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Frame streaming:** Often missing proper buffer lifecycle management — verify `wl_buffer.release` timing
- [ ] **Input handling:** Often missing modifier tracking — verify Shift/Ctrl/Alt work in all applications
- [ ] **XDG shell:** Often missing configure/ack_configure ordering — verify window resizing works correctly
- [ ] **Popups:** Often missing grab/dismissal logic — verify menus close when clicking outside
- [ ] **Surface roles:** Often missing subsurface handling — verify tooltips and dropdowns position correctly
- [ ] **Damage tracking:** Often implemented but disabled — verify it actually reduces bandwidth
- [ ] **Error handling:** Often missing recovery from network hiccups — verify automatic reconnection

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Frame callback stall | LOW | Detect via heartbeat, reset callback state, notify client |
| Buffer leak | MEDIUM | Implement buffer timeout, force release after threshold |
| Serial mismatch | LOW | Log and ignore mismatched serials, continue processing |
| Network disconnect | LOW | Buffer last frame, attempt reconnect with exponential backoff |
| Protocol violation | MEDIUM | Reset connection, clear surface state, client reconnects |
| Keymap error | LOW | Fall back to US layout, log error, continue |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Frame Callback Timing | Phase 2 | Test with `weston-presentation-shm`, verify smooth animation |
| Buffer Release Ordering | Phase 2 | Monitor memory usage, verify no growth during long sessions |
| XDG Configure/Ack | Phase 1 | Test with GTK/Qt apps, verify resize and close work |
| Input Serial Mismatches | Phase 3 | Test context menus and drag-drop in various apps |
| Keyboard Keymap | Phase 3 | Type in multiple languages, verify all keys work |
| Surface Roles | Phase 1 | Test applications with complex UI (menus, tooltips) |
| Damage Tracking | Phase 4 | Measure bandwidth with/without damage tracking |
| Network Backpressure | Phase 2 | Simulate slow network, verify graceful degradation |

## Sources

- [Wayland Protocol Specification](https://wayland.freedesktop.org/docs/html/ch04.html) - Official protocol documentation
- [The Wayland Book](https://wayland-book.com/) - Comprehensive guide to Wayland development
- [Smithay Documentation](https://docs.rs/smithay/latest/smithay/) - Rust Wayland compositor framework
- [Wayland Discussions](https://github.com/Smithay/smithay/discussions) - Community issues and patterns
- [XKB Common Documentation](https://xkbcommon.org/) - Keyboard handling reference

---
*Pitfalls research for: Wayland Remote Compositor*  
*Researched: 2025-03-10*