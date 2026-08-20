# Issue 01 — Server per-window rendering

## Objective

Make the server emit one frame **per mapped window** tagged with that window's `window_id`, instead of one composite full-desktop frame hardcoded to `window_id: 0`, and emit `WindowEventKind::Resized` when a mapped window's committed size changes. This is the wire contract the viewer's multi-HWND layer demuxes ([[005-windows-client-e2e|Plan 005]] Phase A; closes the [[003-m3-window-mapping]] render gap).

## Files

| File | Change |
|---|---|
| `crates/server/src/rendering/mod.rs` | Add `render_surface(buffer, w, h) -> FrameBuffer` that renders a single `WlBuffer` at origin into a `w×h` BGRA target. Keep `render(&[...])` for tests/snapshot. Add `window_id: u64` field to `FrameBuffer`. |
| `crates/server/src/state.rs` | Add `render_window(window_id) -> FrameBuffer` that looks up the mapped window's committed buffer (via `WindowManager::surface_id_for(window_id)`) and renders it at its current committed size. Add `surface_id_for`/`mapped_windows()` accessors on `WindowManager` as needed. |
| `crates/server/src/bridge.rs` | `NetCommand::Frame` already carries `FrameBuffer`; `FrameBuffer` gains `window_id` — update the bridge test fixture accordingly. |
| `crates/server/src/lib.rs` | Replace the single `render_frame()`+`push_frame` block in the streaming loop with a loop over mapped windows: render each that has a buffer, push a `NetCommand::Frame` tagged with that window's id. Keep the 50 ms dispatch cadence; sender-side coalescing in `net/session.rs` handles rate. |
| `crates/server/src/window.rs` | `on_commit`: when a *mapped* window's committed `(width,height)` differs from the stored size, update stored size AND push a `WindowEventKind::Resized { width, height }` to `pending_events` (today only `Created` is pushed). |
| `crates/server/src/net/session.rs` | `write_frame`: use `frame.window_id` for `FrameHeader::window_id` instead of hardcoded `0`. |
| `crates/server/tests/render.rs` | Keep the existing full-desktop `render()` test; do not break it. |
| `crates/server/tests/streaming.rs` | Update/extend: assert streamed frames carry the `window_id` of the created toplevel (not 0); if multi-window, assert per-window ids. |
| `crates/server/tests/xdg.rs` | Add/extend: a mapped window that re-commits at a larger size emits `WindowEventKind::Resized` with the new dimensions. |

## Implementation notes

- **Per-window render target**: each `render_window` call builds a fresh pixman `Argb8888` image of the window's current committed size, renders that window's single buffer at `(0,0)`, clears to black, and reads back. Reuse `OffscreenRenderer`'s pixman renderer instance across calls (it is stateless between passes); only the target image is per-call. Do NOT allocate a new `PixmanRenderer` per window.
- **Window lookup**: `WindowManager::surface_to_window` maps `ObjectId -> window_id`; add the inverse `window_id -> surface_id` (already trivially derivable from `Window::surface_id`). A window's committed buffer lives in `State::surfaces[surface_id].buffer`; render only when `Some`.
- **Coalescing**: the streaming loop already ticks every 50 ms; rendering N windows per tick is fine for M3 sizes (1–3 windows). Sender-side coalescing in `net/session.rs` (`COALESCE_WINDOW`) already drops intermediate frames per connection; per-window frames are independent `NetCommand::Frame` messages on the same channel, so coalescing may drop a specific window's intermediate frame — acceptable (newest wins).
- **Resized event**: guard against duplicate `Resized` events for unchanged sizes — only emit when `width`/`height` actually changed since last commit.
- **`window_id` on `FrameBuffer`**: server-side `FrameBuffer` is distinct from `crates/viewer/src/framebuf.rs::FrameBuffer` (different crate). Both gain `window_id`; viewer-side is Phase B.
- **Snapshot path** (`--snapshot`) still uses `render()` (full-desktop) — leave it; it's a debug tool, not on the per-window path.

## Steps

1. Add `window_id: u64` to server `FrameBuffer`; update `write_frame` to use it; update bridge test fixture.
2. Add `OffscreenRenderer::render_surface(buffer, w, h)`; add `render_window(window_id)` to `State` with `WindowManager` accessors.
3. Replace the streaming loop's single-frame block with a per-mapped-window render+push loop.
4. `WindowManager::on_commit`: emit `Resized` on mapped size change (guard against no-op).
5. Update `tests/streaming.rs` and `tests/xdg.rs` for the new contract; keep `tests/render.rs` green.
6. `cargo test -p wayland-remote-server` green on Linux.

## Verification

- `cargo test -p wayland-remote-server` green on `gary-agents` (Linux).
- New test: a test client commits a toplevel → streamed frame's `window_id` matches the `Created` event's `window_id` (≠ 0).
- New/extended test: a mapped window re-commits at a larger size → a `WindowEventKind::Resized { width, height }` with the new dimensions is observed on the control stream.
- Existing `tests/render.rs` (full-desktop `render()`) still passes unchanged.
- `cargo clippy -p wayland-remote-server` clean; `cargo fmt --check` clean.
