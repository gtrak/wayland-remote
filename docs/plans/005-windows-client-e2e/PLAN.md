# Plan 005 — Windows Client + End-to-End

## Why

Plans [[002-m2-windows-viewer-input]] and [[003-m3-window-mapping]] scoped the Windows viewer and xdg window mapping but left two gaps that block a real end-to-end run: (1) `crates/viewer/src/display/win.rs` is still `unimplemented!()`, and (2) the server composites **all** surfaces into one full-desktop buffer and hardcodes `FrameHeader::window_id = 0`, so per-window HWND mapping on the viewer has no per-window frame source. This plan closes both, lands a native Windows binary, and runs a real Linux→Windows end-to-end test against `gary-agents` to validate the PRD Step 4/5/6 milestones (you see the Linux windows; you type in them).

## What

- **Server per-window rendering** (Phase A): generalize `OffscreenRenderer` to render one surface at a given size; tag `FrameBuffer` and `NetCommand::Frame` with `window_id`; the compositor loop renders one frame per mapped window per tick instead of one composite frame; `write_frame` uses the frame's `window_id`; `WindowManager::on_commit` emits `WindowEventKind::Resized` on a mapped window's size change so resize round-trips.
- **Viewer window-id plumbing + Win32 layer** (Phase B+C): `FrameBuffer`/`next_frame` carry `window_id`; `display/win.rs` implements the real Win32 message loop — one controller HWND owns the loop, one child/overlapped HWND per `window_id` with per-window `FrameStore`, GDI `StretchDIBits` blit (32bpp, top-down BGRA, stretch-to-fit), input translation → `session.send_input(window_id, event)`, focus/`SetFocus`, close/`CloseWindow`, resize/`ConfigureWindow` with an echo-loop guard.
- **End-to-end build & test** (Phase D): push to GitHub, pull on `gary-agents`, build the server there; build the viewer natively on Windows; run two xdg clients (`weston-terminal`, `weston-simple-egl`) and verify two independent HWNDs, typing, focus, close, resize, and the TOFU `--fingerprint` path.
- **Docs & closeout** (Phase E): update `lat.md/` (Viewer architecture section, per-window rendering pipeline, stretch-to-fit + per-window render decisions, new test specs), run `lat check`, and archive plans 002/003/005 per the plan-process skill.

## Success criteria

- `cargo build -p wayland-remote-viewer` green on Windows; `cargo build -p wayland-remote-server --release` green on `gary-agents`.
- `cargo test` (workspace) green on `gary-agents` including updated/new per-window render + resize tests.
- Manual E2E on the Windows box against the `gary-agents` server: two Wayland apps appear as two independent Windows windows; typing reaches the focused toplevel; X-close closes the remote app; resizing the HWND resizes the remote surface after one configure/ack round-trip; `--fingerprint` TOFU path works alongside `--insecure`.
- `lat check` green with zero warnings; `lat.md/` documents the viewer net-task/UI-thread split and the per-window render pipeline.
- `docs/plans/` contains only `archive/` after closeout (002, 003, 005 archived as 20–30 line summaries).

## Task order

```
01-server-per-window  ──>  02-viewer-win32  ──>  03-e2e-test  ──>  04-lat-docs-archive
```

01 first because per-window frames are the contract the viewer demuxes and it is fully Linux-testable before any Windows work. 02 builds the Win32 layer against the new wire contract. 03 is verification-only (git push/pull + builds + manual run). 04 is documentation and archival.

## Scope

In: server per-window render + `window_id` on the wire + `Resized` event; viewer `window_id` plumbing + real `display/win.rs` (multi-HWND, GDI, input, focus, close, resize); E2E run against `gary-agents`; `lat.md` updates; plan archival.

Out: reconnect-with-backoff and persistent `%APPDATA%\known_hosts` TOFU store (deferred — minimal `--insecure`/`--fingerprint` only); `xdg_popup` beyond no-op; server-side cursor sprite (M3-optional, deferred — viewer draws default arrow); lossy/WAN tuning, clipboard/DnD; video codec / dmabuf zero-copy (PRD §7).
