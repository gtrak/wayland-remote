# Plan 002 — M2: Windows Viewer + Input Round-trip (Archived)

Completed by Plan 005 (Phases B+C). M1 streamed frames from Linux; M2 put those
pixels on a Windows desktop and closed the input loop so keyboard and mouse
events from Windows reach Wayland clients. This was the PRD "see the Linux
window" + "type in a remote terminal" milestone.

The viewer is a raw Win32 app (windows-sys, no winit) that connects over QUIC
with TOFU fingerprint pinning, decodes frames, blits them with GDI
`StretchDIBits`, and sends input upstream on the control stream. The server
injects received `Input` messages into the smithay seat, translating Windows
scancodes to Linux keycodes.

Scope:
- Viewer: QUIC session, frame decode + GDI blit, resize, TOFU, input translation.
- Server: scancode-to-keycode translation and seat injection.
- Non-Win32 viewer logic unit-tested on Linux; CI runs those tests.

Out of scope: multi-window HWND mapping (Plan 003), cursor sprite, lossy/WAN
tuning, clipboard.

## 01-viewer-app
## 02-input-injection
