# Authoritative Requirements: Wayland Remote

**Defined:** 2025-03-10
**Core Value:** Users can run Linux GUI applications remotely and interact with them as native windows on Windows, with full input support and acceptable performance.

## v1 Authoritative Requirements

These are the human-defined requirements. Each maps to roadmap phases.

### Core Wayland Protocol

- [x] **WAYL-01**: Compositor accepts Wayland client connections and handles wl_compositor, wl_surface, wl_seat, wl_output protocols
- [x] **WAYL-02**: Applications can create surfaces, attach buffers, and commit changes
- [x] **WAYL-03**: Surface destruction and cleanup is handled properly

### Rendering Pipeline

- [x] **REND-01**: Compositor uses headless/offscreen rendering (PixmanRenderer)
- [x] **REND-02**: Surface content is rendered to an offscreen buffer/framebuffer
- [x] **REND-03**: Framebuffer can be read back as RGBA pixel data

### Frame Streaming

- [x] **STREAM-01**: TCP server accepts connections from Windows viewer
- [x] **STREAM-02**: Frame header is sent (width, height, timestamp, size)
- [x] **STREAM-03**: Raw RGBA pixel data is streamed over TCP
- [ ] **STREAM-04**: Multiple surfaces can be tracked and streamed

### Windows Viewer

- [ ] **VIEW-01**: Windows application connects to TCP server
- [ ] **VIEW-02**: Received frames are displayed in Win32 windows using GDI
- [ ] **VIEW-03**: Each Wayland surface maps to a native Windows HWND
- [ ] **VIEW-04**: Window resizes are handled (frame scaling)

### Input Handling

- [ ] **INPUT-01**: Windows viewer captures keyboard input and sends to server
- [ ] **INPUT-02**: Windows viewer captures mouse movement and button clicks
- [ ] **INPUT-03**: Server injects input events into Wayland input pipeline
- [ ] **INPUT-04**: XKB keymap handling is implemented

### Window Management

- [ ] **WM-01**: XDG shell (xdg_wm_base, xdg_surface, xdg_toplevel) is supported
- [ ] **WM-02**: Window configure/ack handshake is implemented
- [ ] **WM-03**: Window states (maximize, minimize, fullscreen, close) are handled
- [ ] **WM-04**: Popup windows (menus, tooltips) are supported

## v2 Authoritative Requirements

Deferred to future release but still authoritative.

### Performance Optimization

- **PERF-01**: Damage tracking sends only changed regions
- **PERF-02**: H264 video encoding reduces bandwidth
- **PERF-03**: Hardware acceleration (dmabuf) for GPU buffers

### Advanced Features

- **ADV-01**: Clipboard synchronization between Linux and Windows
- **ADV-02**: Session persistence (disconnect/reconnect without losing state)
- **ADV-03**: XWayland support for X11 applications
- **ADV-04**: Multi-monitor support (virtual outputs)

## Out of Scope

Explicitly excluded from this project.

| Feature | Reason |
|---------|--------|
| Wayland protocol proxy | Frame streaming approach chosen instead - simpler and more robust |
| Built-in authentication | Security via SSH tunnel; authentication is user's responsibility |
| Real-time collaboration | Single user per connection; multi-user is vastly more complex |
| 3D/GPU passthrough | Extremely complex driver dependencies; software rendering sufficient |
| Web client | Doubles implementation effort; native Windows client is MVP focus |
| Audio forwarding | Separate stream complexity; not core to remote display value |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| WAYL-01 | Phase 2 | Complete |
| WAYL-02 | Phase 2 | Complete |
| WAYL-03 | Phase 2 | Complete |
| REND-01 | Phase 3 | Complete |
| REND-02 | Phase 3 | Complete |
| REND-03 | Phase 3 | Complete |
| STREAM-01 | Phase 4 | Complete |
| STREAM-02 | Phase 4 | Complete |
| STREAM-03 | Phase 4 | Complete |
| STREAM-04 | Phase 4 | Pending |
| VIEW-01 | Phase 5 | Pending |
| VIEW-02 | Phase 5 | Pending |
| VIEW-03 | Phase 6 | Pending |
| VIEW-04 | Phase 6 | Pending |
| INPUT-01 | Phase 8 | Pending |
| INPUT-02 | Phase 8 | Pending |
| INPUT-03 | Phase 8 | Pending |
| INPUT-04 | Phase 8 | Pending |
| WM-01 | Phase 7 | Pending |
| WM-02 | Phase 7 | Pending |
| WM-03 | Phase 7 | Pending |
| WM-04 | Phase 7 | Pending |

---
---

*Authoritative requirements defined: 2025-03-10*
*Only humans may modify this file*
