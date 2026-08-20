# Plan 005 — Windows Client + End-to-End (Archived)

Closed the two gaps blocking a real end-to-end run: `display/win.rs` was
`unimplemented!()`, and the server composite all surfaces into one desktop
buffer with `window_id` hardcoded to 0. This plan landed per-window rendering,
the real Win32 viewer, and a Linux→Windows E2E run validating PRD Steps 4/5/6.

E2E results (server on gary-agents, native MSVC viewer on Windows): QUIC
handshake and TOFU `--fingerprint` both work; two xdg clients render as two
independent Windows HWNDs (multi-window); per-window frames stream at ~20 fps
with ~4 ms round-trip time; keyboard/mouse input reaches the focused toplevel,
and X-close and HWND resize round-trip to the remote surface.

Scope:
- Server: per-window render targets, `window_id` on the wire, `Resized` on re-commit.
- Viewer: net-task/UI-thread split, per-window `FrameStore`, GDI blit, input/focus/close/resize.
- E2E run against gary-agents; lat.md updates; archival of 002/003/005.

Out of scope: reconnect-with-backoff + persistent TOFU store, popup grabs,
cursor sprite, lossy/WAN tuning, clipboard/DnD, video codec / dmabuf.

## 01-server-per-window
## 02-viewer-win32
## 03-e2e-test
## 04-lat-docs-archive
