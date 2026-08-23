# 02 — Per-window change gating (server)

## Objective

Stop re-rendering and re-streaming a window that has no new content. Today the stream
loop in `run` (`crates/server/src/lib.rs`) calls `render_window` + `push_frame` for
**every** mapped window **every tick**, so a static window pays a full GL import + PBO
readback + compress + send repeatedly for an identical frame. This is the single
highest-value CPU/GPU saver and the prerequisite for issues 03 and 04.

## Files

| File | Change |
|------|--------|
| `crates/server/src/window.rs` | Add a `dirty: bool` to `Window` (default `true` on map). Provide a `mark_dirty()` accessor. |
| `crates/server/src/state.rs` | In `CompositorHandler::commit`, mark the window dirty when a new buffer is committed (`BufferAssignment::NewBuffer`) or on resize. Until issue 04 lands, also mark the focused window dirty on a pointer move that changes the composited cursor (so the cursor still moves). |
| `crates/server/src/lib.rs` | In the per-window stream loop, skip `render_window` + `push_frame` for a window whose `dirty` is false; when dirty, render, push, then clear the flag. |
| `crates/server/src/window.rs` | Ensure resize/move (viewer `ConfigureWindow` / `CloseWindow` path) also marks the window dirty. |

## Steps

1. Add `dirty: bool` to `Window`; set it `true` when the window first maps.
2. In `commit`, after the existing buffer/size handling, if the commit attached a new
   buffer or changed the size, `mark_dirty` the window.
3. In the stream loop, guard the render+push with the dirty flag and clear it after a
   successful push.
4. Until issue 04 removes cursor compositing, mark the focused window dirty on pointer
   motion so the (still-in-frame) cursor keeps moving.

## Verification

- With a mapped **static** client and no input, the per-second telemetry `frames` is
  flat (no re-stream) and `readback_ms` is ~0 for that window — confirm via issue 01's
  instrumentation.
- With `weston-flower` / `weston-simple-egl` (animating), frames still stream at the
  commit rate and still animate (no regression).
- Moving the pointer still moves the visible cursor (pre-04) — no "frozen cursor".
- `cargo test --workspace` green (input round-trip test included); drive harness
  `--client weston-flower` and `--client weston-simple-egl` still PASS.

## Result (measured at `9d83857`, gary-agents)

- **Animating (no regression):** `weston-flower` PASS (pixel change frame 1),
  `weston-simple-egl` PASS (pixel change frame 1). Both still re-render every frame —
  no frozen animation.
- **Static win (`weston-clickdot`):** rendered exactly **2 frames** (the initial map),
  then every subsequent second shows `fps=0 render_ms=0 readback_ms=0`. Before 02 a
  static window re-rendered + re-streamed every tick (issue 01: ~2.6 ms/frame); now it
  costs nothing while idle.
