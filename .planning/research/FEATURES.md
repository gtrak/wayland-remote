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
