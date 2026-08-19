# Issue 01 — Windows viewer application

## Objective

(PRD Step 4) A standalone Windows binary that connects to the server over QUIC, receives frames, and displays them in a resizable window using GDI, with TOFU certificate pinning and automatic reconnect.

## Files

| File | Change |
|---|---|
| `crates/viewer/src/main.rs` | Entry: arg/env parsing (server addr, fingerprint or `--insecure`, window size), spawns tokio runtime + UI thread |
| `crates/viewer/src/session.rs` | QUIC client session: handshake, control stream, frame-stream reader loop with skip-stale (STOP_SENDING older frames as in [[001-m1-streaming-linux/05-quic-streaming|server issue 05]]) — pure logic, `#[cfg(test)]`-able on Linux with a loopback quinn endpoint |
| `crates/viewer/src/framebuf.rs` | `FrameBuffer`: double-buffered BGRA store (front for blit, back for decode), swap on complete frame |
| `crates/viewer/src/input.rs` | Input event construction: `WNDPROC` hook translating `WM_KEYDOWN/UP`, `WM_MOUSEMOVE`, `WM_LBUTTON*`, `WM_RBUTTON*`, `WM_MOUSEWHEEL` → `protocol::InputEvent` (scancodes from lParam bits 16-23 + extended flag; window coords → surface coords) — translation functions pure + Linux-tested |
| `crates/viewer/src/display/mod.rs` | Platform dispatch: `#[cfg(windows)]` → `win.rs`; a `--headless` stub (writes frames to raw file, used by Linux CI tests) |
| `crates/viewer/src/display/win.rs` | `RegisterClass`, message loop, `StretchDIBits` with negative biHeight for top-down BGRA, `WM_SIZE` handling, `InvalidateRect` on frame swap; window title shows fps/latency |
| `crates/viewer/tests/session.rs` | Loopback tests (run on Linux): session against an in-process quinn server |

## Implementation notes

- **Rendering**: `StretchDIBits` on `WM_PAINT` from the front `FrameBuffer`. BITMAPINFO `biBitCount=32, biCompression=BI_RGB, biHeight=-(h as i32)` — negative height = top-down rows, matching pixman readback order. Verify with M1's byte-order test; if colors are swapped on real hardware the bug is in this assumption, not the protocol.
- **Threads**: UI thread owns the message loop + GDI (single-threaded, as Win32 tradition demands); tokio session task pushes decoded frames into `FrameBuffer` behind a mutex/atomics; UI repaints on a `PostMessage`-triggered invalidation — never blit from the network thread.
- **TOFU**: on first connect without `--fingerprint`, display/copy the server's SPKI fingerprint, store it under `%APPDATA%\wayland-remote\known_hosts` after user passes `--trust` (or interactive y/N via console). Subsequent connects verify; mismatch → hard fail with loud message.
- **Reconnect**: connection lost → exponential backoff (250ms → 8s cap), re-handshake, re-Hello. Surface resize while disconnected tolerated.
- **Latency display**: viewer sends Ping every 500ms, Pong RTT shown in title bar; frame pipeline latency (header timestamp_ns vs local receive time) tracked as a rolling average.
- `--headless N` mode: run N seconds, write last frame to file, exit 0 — this is how Linux CI exercises the full session path without a display.

## Steps

1. `session.rs` + `framebuf.rs` (pure logic) with loopback tests first.
2. `input.rs` translation functions + tests (scancode extraction, coord mapping, button code mapping to `BTN_LEFT/RIGHT/MIDDLE`, wheel → Axis ticks).
3. `win.rs` platform layer; `headless` stub.
4. Manual UAT on the Windows box against the M1 server (both zigbuild exe and native build).
5. `lat.md/` updates: viewer architecture section, test specs, TOFU UX decision.

## Verification

- Linux tests: `session` handshake/Welcome handling; frame decode → `FrameBuffer` swap; skip-stale issues STOP_SENDING for stale streams; reconnect logic (kill loopback server, restart, session recovers).
- Linux tests: input translation table spot checks (VK ↔ scancode extraction incl. extended keys like arrows; wheel sign).
- `cargo test -p wayland-remote-viewer` green on Linux (headless paths); `cargo zigbuild --target x86_64-pc-windows-gnu` green.
- Manual on Windows box: launch server + test client on Linux, viewer on Windows → pattern visible, window resizes cleanly (letterbox or stretch — pick stretch for M2, note in decisions), fps/RTT in title bar.
- Clippy/fmt clean; `lat check` green.
