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
