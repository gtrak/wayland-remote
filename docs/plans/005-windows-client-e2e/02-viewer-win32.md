# Issue 02 — Viewer window-id plumbing + Win32 multi-window layer

## Objective

Implement the real Windows viewer: carry `window_id` through the session, and replace the `unimplemented!()` stub in `crates/viewer/src/display/win.rs` with a Win32 message loop that creates one HWND per `window_id`, blits per-window frames via GDI `StretchDIBits`, translates keyboard/mouse input upstream tagged with the focused window's id, and wires focus/close/resize round-trips. This is [[005-windows-client-e2e|Plan 005]] Phases B+C and closes the [[002-m2-windows-viewer-input]] viewer-app gap.

## Files

| File | Change |
|---|---|
| `crates/viewer/src/framebuf.rs` | Add `window_id: u64` to `FrameBuffer`. |
| `crates/viewer/src/session.rs` | `next_frame`: populate `window_id` from `FrameHeader::window_id`. |
| `crates/viewer/src/display/win.rs` | Replace stub with the full Win32 implementation (see Implementation notes). |
| `crates/viewer/src/display/headless.rs` | Update `headless` log line / `FrameBuffer` field access for the new `window_id` field (no behavior change). |
| `crates/viewer/src/window_manager.rs` | Extend `ViewerWindowManager` to map `window_id -> HWND` (Windows-only field behind `#[cfg(windows)]`) alongside the existing `ViewerWindow` state; `Created`/`Destroyed`/`Resized` mutate the HWND map. |
| `crates/viewer/tests/session.rs` | Update any `FrameBuffer` construction for the new `window_id` field; add an assertion that `next_frame` surfaces the header's `window_id`. |

## Implementation notes

- **Threads**: UI thread owns the message loop + GDI (single-threaded, per Win32 tradition). A background thread runs a single-threaded tokio runtime owning `ViewerSession`. Net→UI signaling is via `PostMessageW` (never blit from the net thread). UI→net is via a `tokio::mpsc::UnboundedSender<InputCommand>` where `InputCommand = (u64 window_id, InputEvent)`.
- **Controller HWND**: a hidden top-level window owns the message loop and the `ViewerWindowManager`. It receives:
  - `WM_USER_FRAME` (wParam = `window_id`): a frame arrived for that window; `InvalidateRect` that window's HWND.
  - `WM_USER_WIN_EVENT` (lParam points to a `Box<Message::WindowEvent>` the controller must drop): route to `ViewerWindowManager::handle_event`, which creates/destroys/resizes child HWNDs.
  - `WM_USER_RTT` (wParam = rtt ms): update the focused window's title with `wayland-remote — {fps}fps {rtt}ms`.
- **Per-window HWND**: `CreateWindowEx` an overlapped window per `WindowEventKind::Created` with the event's `title`/`width`/`height`. Each has its own `Arc<FrameStore>`. The net task keeps `HashMap<u64, Arc<FrameStore>>` and swaps incoming frames into the matching store.
- **Blit**: `WM_PAINT` → `frame_store.borrow()` → `StretchDIBits` with `BITMAPINFO { biBitCount=32, biCompression=BI_RGB, biHeight=-(h as i32) }` (negative height = top-down rows, matching pixman readback). Stretch-to-fit the client area (M2/M3 decision: stretch, not letterbox). If no frame yet, fill with black.
- **Input**: child HWND `WM_KEYDOWN`/`WM_KEYUP` → `input::extract_scancode` + `input::key_event` → input channel tagged with that HWND's `window_id`. `WM_MOUSEMOVE`/`WM_LBUTTON*`/`WM_RBUTTON*`/`WM_MBUTTONDOWN`/``WM_MBUTTONUP`/`WM_MOUSEWHEEL` → `input::pointer_move`/`pointer_button`/`scroll` → input channel. Map window-relative coords to surface coords (identity for stretch-to-fit at 1:1; for stretched blit, scale by `surface_size / client_size`).
- **Focus**: child `WM_ACTIVATE` (activated) → send `Message::SetFocus { window_id }` on the control stream so the server activates that toplevel and routes keyboard there.
- **Close**: child `WM_CLOSE` (X button) → send `Message::CloseWindow { window_id }`; do NOT `DestroyWindow` locally — wait for the server's `WindowEventKind::Destroyed` which triggers `DestroyWindow`. (Matches native "app decides to close" semantics.)
- **Resize**: child `WM_SIZE` → send `Message::ConfigureWindow { window_id, w, h }`. Set a `pending_configure` flag on that window to ignore the immediate local `WM_SIZE` echo. When the server's `Resized` event arrives, clear the flag and `SetWindowPos` to the negotiated size. Guard the echo loop: while `pending_configure` is set, ignore `WM_SIZE`.
- **Quit**: controller `WM_CLOSE` → `session.close()` → `PostQuitMessage(0)`. Net task observes stream end and exits.
- **Frame demux / skip-stale**: keep per-connection (already in `ViewerSession::next_frame` via fresh unidirectional streams). Each window's `FrameStore` only holds the newest frame for that window; the net task overwrites on each arrival.
- **windows-sys features** already declared workspace-wide: `Win32_Graphics_Gdi`, `Win32_UI_WindowsAndMessaging`, `Win32_Foundation`, `Win32_System_LibraryLoader`, `Win32_UI_Input_KeyboardAndMouse`. No new deps.

## Steps

1. Add `window_id` to viewer `FrameBuffer`; populate in `session.rs::next_frame`; update `headless.rs` and `tests/session.rs` for the field.
2. Extend `ViewerWindowManager` with a `#[cfg(windows)]` `HWND` map and lifecycle hooks used by `win.rs`.
3. Implement `display/win.rs`: controller HWND + message loop, per-window HWNDs, `FrameStore` map on the net task, `PostMessage` signaling, `WM_PAINT`/`StretchDIBits`, input translation, focus/close/resize round-trips, ping/RTT title.
4. `cargo build -p wayland-remote-viewer` green on Windows; `cargo test -p wayland-remote-viewer` green on Linux (headless + session + input unit tests).
5. `cargo clippy` / `cargo fmt --check` clean.

## Verification

- `cargo test -p wayland-remote-viewer` green on `gary-agents` (Linux): existing `input.rs` pure-function tests, `session.rs` loopback handshake test, `headless` mode. Update `tests/session.rs` to assert `next_frame` surfaces `window_id`.
- `cargo build -p wayland-remote-viewer` green on the Windows box (native MSVC).
- (End-to-end visual + input verification is [[005-windows-client-e2e/03-e2e-test|Issue 03]].)
- `cargo clippy -p wayland-remote-viewer` clean; `cargo fmt --check` clean.
