# 04 — Local viewer-side cursor

## Objective

Remove the pointer cursor from the streamed frame. Today the server composites the
cursor surface into `render_window`, so every cursor move costs a full
render → readback → compress → stream → blit round-trip — this is the "late cursor
tracking" the user observed. Move the cursor to the **viewer** as a native Win32
cursor (the RDP/VNC model): the server sends the sprite (when it changes) + position
(when it moves) over the control channel; the viewer draws the OS cursor. Cursor
motion then has no frame round-trip and no perceptible lag.

Depends on issue 02 (a pure cursor move must **not** mark a window dirty once the
cursor is out of the frame).

## Files

| File | Change |
|------|--------|
| `crates/protocol/src/message.rs` | Add `Message::CursorShape { window_id, width: u32, height: u32, hot_x: i32, hot_y: i32, data: Vec<u8> }` (BGRA sprite, sent on change), `Message::CursorMove { window_id, x: f64, y: f64 }` (position in the focused window, sent on move), `Message::CursorHide { window_id }`. |
| `crates/protocol/src/codec.rs` | Encode/decode the three messages; add a `varint(len) || bytes` encoder for the sprite `data` (cap ~256 KiB). Update the tag table. |
| `crates/server/src/state.rs` | Stop passing the cursor to `render_window`. When `State.cursor_surface` / `CursorImageSurfaceData` (sprite + hotspot) changes, import the cursor surface → readback → send `CursorShape`. On pointer motion, send `CursorMove` with the position in the focused window's coordinate space. On hide, `CursorHide`. Do **not** mark a window dirty for a pure cursor move. |
| `crates/server/src/input/` (pointer path) | Emit `CursorMove` on each pointer motion (and on focus change) to the net side. |
| `crates/viewer/src/display/win.rs` (+ `input.rs`/`session.rs`) | Per focused child window: on `CursorShape`, build an `HCURSOR` (e.g. `CreateDIBSection`/`CreateCursor`) from the BGRA sprite and `SetCursor`; on `CursorMove`, `SetCursorPos` clamped into the child; on `CursorHide`, `ShowCursor(FALSE)`. Only the focused window shows the cursor. |

## Steps

1. Add the three cursor messages + a byte-array codec (protocol round-trip test).
2. Server: drop the cursor from `render_window`. Track the last sent sprite+hotspot;
   on change, readback the cursor surface (a small surface) and send `CursorShape`.
   On pointer motion / focus change, send `CursorMove`. On hide, `CursorHide`.
3. Viewer: create/update the native cursor on `CursorShape`; reposition on
   `CursorMove`; hide on `CursorHide`. Scope the cursor to the focused child window so
   it doesn't appear over other windows.
4. Ensure a pure cursor move emits no full frame (coordinate with issue 02's dirty
   logic).

## Verification

- With the server no longer compositing the cursor, moving the mouse moves the
  viewer's native cursor with no perceptible lag; while only the cursor moves, the
  per-second telemetry `frames` is flat (no full frame emitted) — confirm via issue 01.
- The hotspot is correct: a click lands under the pointer tip (drive harness `--click`
  still hits the right spot; input round-trip test green).
- `CursorHide` works (e.g. the cursor disappears over a terminal / text field).
- Switching focus between two child windows moves the native cursor to the focused one.
- No regression: drive harness + `cargo test --workspace` green.
