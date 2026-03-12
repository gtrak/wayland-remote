# Derived Requirements: Wayland Remote

**Generated:** 2025-03-10
**Source:** REQUIREMENTS.authoritative.md

## Implementation Requirements

These are derived from authoritative requirements during planning. They represent the breakdown needed to implement features.

### Core Wayland Protocol

- [x] **DER-WAYL-01** (from WAYL-01): Implement wl_compositor interface with surface creation
- [x] **DER-WAYL-02** (from WAYL-01): Implement wl_seat with keyboard and pointer capabilities
- [x] **DER-WAYL-03** (from WAYL-01): Implement wl_output with virtual display configuration
- [ ] **DER-WAYL-04** (from WAYL-02): Handle wl_surface.attach with SHM buffers
- [ ] **DER-WAYL-05** (from WAYL-02): Handle wl_surface.commit to trigger rendering
- [ ] **DER-WAYL-06** (from WAYL-03): Proper surface destruction and resource cleanup

### Rendering Pipeline

- [x] **DER-REND-01** (from REND-01): Configure Smithay with Virtual backend
- [x] **DER-REND-02** (from REND-01): Initialize PixmanRenderer for CPU rendering
- [x] **DER-REND-03** (from REND-02): Implement headless output (no physical display)
- [ ] **DER-REND-04** (from REND-03): Read pixels from Pixman buffer to RGBA bytes

### Frame Streaming

- [x] **DER-STREAM-01** (from STREAM-01): Tokio TCP listener on configurable port
- [x] **DER-STREAM-02** (from STREAM-01): Handle multiple concurrent connections
- [x] **DER-STREAM-03** (from STREAM-02): Binary frame header protocol (u32 width, height, size; u64 timestamp)
- [x] **DER-STREAM-04** (from STREAM-03): Send raw pixel buffer over TCP
- [ ] **DER-STREAM-05** (from STREAM-04): Track surface IDs and map to client windows

### Windows Viewer

- [x] **DER-VIEW-01** (from VIEW-01): TCP client connects to Linux server
- [x] **DER-VIEW-02** (from VIEW-01): Async frame reading with tokio
- [x] **DER-VIEW-03** (from VIEW-02): Parse frame header and allocate buffer
- [x] **DER-VIEW-04** (from VIEW-02): Use StretchDIBits to display RGBA in HWND
- [ ] **DER-VIEW-05** (from VIEW-03): Create HWND per surface with unique IDs
- [ ] **DER-VIEW-06** (from VIEW-04): Handle WM_SIZE and rescale displayed frame

### Input Handling

- [ ] **DER-INPUT-01** (from INPUT-01): Win32 message loop captures WM_KEYDOWN/UP
- [ ] **DER-INPUT-02** (from INPUT-02): Win32 mouse tracking and button capture
- [ ] **DER-INPUT-03** (from INPUT-02): Serialize input events to binary protocol
- [ ] **DER-INPUT-04** (from INPUT-03): Deserialize and inject into Smithay input pipeline
- [ ] **DER-INPUT-05** (from INPUT-04): Generate and send XKB keymap via fd
- [ ] **DER-INPUT-06** (from INPUT-04): Map Win32 VK codes to Linux evdev scancodes (+8)

### Window Management

- [ ] **DER-WM-01** (from WM-01): Implement xdg_wm_base global interface
- [ ] **DER-WM-02** (from WM-01): Handle xdg_surface and xdg_toplevel roles
- [ ] **DER-WM-03** (from WM-02): Send xdg_toplevel.configure on resize
- [ ] **DER-WM-04** (from WM-02): Wait for xdg_surface.ack_configure before rendering
- [ ] **DER-WM-05** (from WM-03): Track and forward window state changes
- [ ] **DER-WM-06** (from WM-04): Support xdg_popup for menus/tooltips

## Technical Requirements

Implementation-level requirements not directly from human requirements.

### Security

- [ ] **DER-SEC-01**: All network communication assumes SSH tunnel or VPN
- [ ] **DER-SEC-02**: No authentication implemented in protocol (by design)

### Performance

- [ ] **DER-PERF-01**: Target 30fps for 1080p over LAN (raw RGBA ~90MB/s)
- [ ] **DER-PERF-02**: Frame callback throttling based on network capacity
- [ ] **DER-PERF-03**: Buffer release only after network transmission complete

### Error Handling

- [ ] **DER-ERR-01**: Graceful handling of disconnects
- [ ] **DER-ERR-02**: Reconnection support on viewer side
- [ ] **DER-ERR-03**: Surface cleanup when client disconnects

## Traceability

| Derived ID | Source | Status |
|------------|--------|--------|
| DER-WAYL-01 | WAYL-01 | Pending |
| DER-WAYL-02 | WAYL-01 | Pending |
| DER-WAYL-03 | WAYL-01 | Pending |
| DER-WAYL-04 | WAYL-02 | Pending |
| DER-WAYL-05 | WAYL-02 | Pending |
| DER-WAYL-06 | WAYL-03 | Pending |
| DER-REND-01 | REND-01 | Pending |
| DER-REND-02 | REND-01 | Pending |
| DER-REND-03 | REND-02 | Pending |
| DER-REND-04 | REND-03 | Pending |
| DER-STREAM-01 | STREAM-01 | Pending |
| DER-STREAM-02 | STREAM-01 | Pending |
| DER-STREAM-03 | STREAM-02 | Pending |
| DER-STREAM-04 | STREAM-03 | Pending |
| DER-STREAM-05 | STREAM-04 | Pending |
| DER-VIEW-01 | VIEW-01 | Pending |
| DER-VIEW-02 | VIEW-01 | Pending |
| DER-VIEW-03 | VIEW-02 | Pending |
| DER-VIEW-04 | VIEW-02 | Pending |
| DER-VIEW-05 | VIEW-03 | Pending |
| DER-VIEW-06 | VIEW-04 | Pending |
| DER-INPUT-01 | INPUT-01 | Pending |
| DER-INPUT-02 | INPUT-02 | Pending |
| DER-INPUT-03 | INPUT-02 | Pending |
| DER-INPUT-04 | INPUT-03 | Pending |
| DER-INPUT-05 | INPUT-04 | Pending |
| DER-INPUT-06 | INPUT-04 | Pending |
| DER-WM-01 | WM-01 | Pending |
| DER-WM-02 | WM-01 | Pending |
| DER-WM-03 | WM-02 | Pending |
| DER-WM-04 | WM-02 | Pending |
| DER-WM-05 | WM-03 | Pending |
| DER-WM-06 | WM-04 | Pending |

---

*Generated: 2025-03-10*
*This file is auto-generated from authoritative requirements*
*Planning commands may update this file*
