# 03 — Partial-frame damage (protocol + viewer) + re-sync

## Objective

When a window *does* change, send only the changed region, not the whole window, and
make the viewer apply only that region. Includes a re-sync safety net so the lossy
transport (STOP_SENDING / drop-oldest) can't leave a viewer's baseline corrupted.

This is a **protocol change** (a version bump). It builds on issue 02 (the region is
what we gate on and send).

## Files

| File | Change |
|------|--------|
| `crates/protocol/src/message.rs` | Extend `FrameHeader` with a region: `kind: u8` (`0`=full, `1`=region) + `region_x: u32` + `region_y: u32` (top-left of the region; `width`/`height`/`stride` describe the region when `kind==1`). Bump `FRAME_HEADER_SIZE` and the protocol version constant. |
| `crates/protocol/src/codec.rs` | Encode/decode the new fields; add `Message::RequestFullFrame { window_id }` (viewer → server) for re-sync. Update the tag table in the doc comment. |
| `crates/server/src/window.rs` | Per-window damage accumulator: a bounding box (start full). New-buffer commit → full; a `wl_surface.damage` rect → expand the box; resize → full. |
| `crates/server/src/state.rs` + `crates/server/src/lib.rs` | On stream: if the window's damage box is "small" (e.g. < 50% of the window area) render + readback **only that sub-rectangle** and send `kind=region` with the box; otherwise render + send `kind=full` (the whole window) and reset the box to full. Handle `RequestFullFrame` by forcing a full render+send for that window. |
| `crates/viewer/src/` (frame store + blit, `framebuf.rs` / `display/win.rs`) | Frame store holds the full window. On a `kind=full` frame, replace the whole store. On a `kind=region` frame, copy only the region bytes into the store at `(region_x, region_y)` and invalidate/redraw only that child-window rect. If the viewer receives a `region` frame for a window it has no baseline for (or detects a `frame_id` gap), it sends `RequestFullFrame`. |

## Steps

1. Bump the protocol version; add `kind`/`region_x`/`region_y` to `FrameHeader` + codec
   (round-trip test in `crates/protocol`).
2. Server: maintain the per-window damage box (issue 02's dirty flag becomes "dirty +
   box"). Decide region-vs-full by area threshold. For the region case, do a
   sub-rectangle readback (read back only `box` from the offscreen target) so the
   readback cost drops too, not just the bandwidth.
3. Viewer: apply region frames incrementally to the per-window store; blit only the
   changed rect; request a full frame on a gap / missing baseline.
4. Add a periodic full-frame backstop (e.g. every 2 s or every N frames per window) so
   a silently-dropped delta self-heals even without an explicit `RequestFullFrame`.

## Verification

- A tiny change (move the caret in `weston-terminal`, or a small UI blink) produces a
  frame whose on-wire byte count is a small fraction of the full frame (verify via
  issue 01's `sent_bytes`), and the viewer composites it with no ghosting.
- A large change (window content fully redraws) falls back to a full frame.
- Re-sync: force a drop (e.g. temporarily stop the QUIC frame stream / send
  STOP_SENDING) and confirm the viewer recovers to a correct full image within one
  backstop period or via `RequestFullFrame`.
- Protocol round-trip test passes for both `kind` values; `lat check` green.
- Drive harness still PASSes for `weston-flower` and `weston-simple-egl`.
