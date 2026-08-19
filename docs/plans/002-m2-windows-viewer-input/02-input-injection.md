# Issue 02 — Server-side input injection

## Objective

(PRD Step 5) Viewer input events reach Wayland clients: scancodes become keysyms via xkb, pointer motion/buttons/scroll are dispatched to the surface under the cursor, focus follows the M1 single-surface model (M3 adds per-window focus).

## Files

| File | Change |
|---|---|
| `crates/server/src/input/mod.rs` | `InputRouter`: consumes `protocol::InputEvent` from the bridge channel, routes to seat handles |
| `crates/server/src/input/keymap.rs` | Scancode translation: Windows scancode (+extended flag already folded by viewer: extended keys map to 0x100-offset space) → linux keycode (scancode + 8 convention) → xkb keysym/state → serial + text |
| `crates/server/src/state.rs` | `focused_surface` field; commit handler updates `KeyboardHandle` focus when the single mapped surface exists |
| `crates/server/src/net/session.rs` | Route control-stream `Input` messages into the bridge (from M1's session) |
| `crates/server/tests/input.rs` | End-to-end input tests with the test client |

## Implementation notes

- **Keycode path**: Windows scancodes for non-extended keys map 1:1 to (linux keycode − 8) for the standard set — the `Set 1` codes align. The viewer sends the raw scancode + sets the high bit (or a dedicated field — encode as scancode values ≥ 0x100) for extended keys (arrows, Ins/Del, RCtrl, etc.) which need a small translation table to linux keycodes (`KEY_RIGHT` etc.). Table lives in `keymap.rs` with unit tests; anything unmapped is logged and dropped, never a panic.
- **Keyboard injection** (smithay 0.7): `KeyboardHandle::input_key(keycode, KeyState, serial, time)` — smithay runs it through the seat's xkb state and emits proper wl_keyboard events + text where the keymap produces it. Verify the keyboard was created with a default keymap in Plan 001 issue 03.
- **Pointer injection**: `PointerHandle::input_pointer_motion_absolute`-style API (check exact 0.7 signature — smithay distinguishes relative/absolute motion; we want absolute, surface-local scaled to the rendered layout) then `input_pointer_button`/`input_pointer_axis`. The server knows the layout from issue 04's tiling, so viewer surface coords map directly.
- **Serial/time discipline**: maintain a monotonically increasing serial counter in `InputRouter`; time = server monotonic ms. Never reuse serials across button pairs.
- **Focus**: M2 has one layout; set keyboard focus to the (single) topmost mapped surface at all times when it exists. Pointer focus follows `PointerHandle`'s internal location logic given absolute motion.
- **Scroll**: `Axis { dx, dy }` discrete ticks → `input_pointer_axis` with `AxisSource::Wheel` + discrete steps (consult 0.7 `PointerHandle` axis API; if discrete-only is awkward, send value=15.0 per tick like libinput defaults).

## Steps

1. `keymap.rs` translation table + unit tests (pure, runs on Linux).
2. `InputRouter` wiring: bridge channel → seat handles; log unknown keycodes at debug.
3. Test client gains `wl_keyboard`/`wl_pointer` listeners recording received events.
4. End-to-end tests + `@lat:` refs; update `lat.md/` (input architecture, scancode convention decision, test specs).

## Verification

- Test `key_press_reaches_client`: inject `KeyDown { scancode: 0x1E }` (A) → test client's `wl_keyboard::key` sees keycode 30 (0x1E+8) pressed, then released; with xkb state, a client reading keysym gets `KEY_A`.
- Test `shift_modifies_keysym`: LeftShift down + A → modifier state includes Shift, keysym path produces uppercase behavior (assert via smithay's keyboard `text`/keysym accessor or client-side xkb — simplest: client asserts modifier group bit).
- Test `extended_key_mapping`: `KeyDown { scancode: 0x100 | 0x4B }` (Left arrow) → keycode `KEY_LEFT` (105).
- Test `unknown_scancode_dropped`: garbage scancode → no client event, no panic.
- Test `pointer_motion_and_button`: PointerMove to surface-local (10,10) + left press/release → client's `wl_pointer` sees enter + motion + button press/release with correct serial ordering (serials strictly increasing).
- Test `scroll_axis`: Axis { dy: 1.0 } → client `wl_pointer::axis` discrete step(s) down.
- Manual demo (PRD Step 5): run a terminal (e.g. `weston-terminal` or `foot`) under the compositor, type from the Windows viewer — characters appear.
- Clippy/fmt clean; `lat check` green.
