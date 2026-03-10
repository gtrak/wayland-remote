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
