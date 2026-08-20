# 05 — Pointer/Keyboard Focus Fix

## Objective

Make injected pointer/keyboard input reach client surfaces. Replace the
`SurfaceFocus` stub with `WlSurface` as the seat focus type, thread `window_id`
through the network→compositor bridge, and fix `inject()` to pass
`Some((surface, (0,0)))` as pointer focus so the pointer enters the window's
surface. Set keyboard focus on `SetFocus`/map. Correct the smithay-07-api skill.

## Files

| File | Change |
|------|--------|
| `crates/server/src/state.rs` | Change `type KeyboardFocus/PointerFocus/TouchFocus = WlSurface` (state.rs:527-529); **delete** the `SurfaceFocus` struct + its `WaylandFocus`/`KeyboardTarget`/`PointerTarget`/`TouchTarget` impls (state.rs:109-225); update `focus_changed` signature (state.rs:535). |
| `crates/server/src/bridge.rs` | Change `CompositorCommand::Input(InputEvent)` → `CompositorCommand::Input { window_id: u64, event: InputEvent }`. |
| `crates/server/src/net/session.rs` | Forward the already-deserialized `window_id`: `Message::Input { window_id, event }` → `CompositorCommand::Input { window_id, event }` (currently `Message::Input { event, .. }` discards it). |
| `crates/server/src/lib.rs` | Handle `CompositorCommand::Input { window_id, event }` → `state.inject_input(window_id, event, serial, time)` (lib.rs:132). `SetFocus { window_id }` → also call `kbd.set_focus(state, Some(&surface), serial)` (lib.rs:137). |
| `crates/server/src/input/mod.rs` | `inject(state, window_id, event, serial, time)`. Resolve `WindowManager::surface_for(window_id) -> Option<&WlSurface>`. `PointerMove` → `ptr.motion(state, Some((surface, Point::new(0.0,0.0))), &MotionEvent{location:(x,y),serial,time})`. Keyboard: `set_focus` then `kbd.input`. |
| `crates/server/src/window.rs` | Add `surface_for(window_id) -> Option<&WlSurface>` (via `toplevel.wl_surface()`). |
| `.agents/skills/smithay-07-api/SKILL.md` | Fix the pointer snippet (lines ~706-718): `motion` is **3-arg** with a `focus: Option<(PointerFocus, Point<f64,Logical>)>` param; document the per-window `(0,0)`-origin pattern and the `event.location - focus_origin` surface-local computation. |

## Steps

1. Add `WindowManager::surface_for(window_id)` returning `toplevel.wl_surface()` for the mapped window.
2. Delete `SurfaceFocus` (state.rs:109-225) and its uses; change the three `SeatHandler` focus types to `WlSurface`. smithay provides `KeyboardTarget<D> for WlSurface` / `PointerTarget<D> for WlSurface` / `TouchTarget<D> for WlSurface` (verified at `smithay-0.7.0/src/wayland/seat/{keyboard,pointer}.rs`) — do **not** reimplement them. Update `focus_changed(&mut self, _seat, _focused: Option<&WlSurface>)`.
3. Update `bridge.rs` `CompositorCommand::Input` to carry `window_id`; update `session.rs` to forward it; update `lib.rs` to destructure `{ window_id, event }`.
4. Change `inject()` signature to take `window_id`. For `PointerMove{x,y}`: resolve `surface`; `ptr.motion(state, Some((surface.clone(), Point::<f64,Logical>::new(0.0,0.0))), &MotionEvent{location: Point::from((x,y)), serial, time})`. For `PointerButton`: rely on the focus established by the preceding `motion` (the grab stores `self.focus`); call `ptr.button(...)`. For `Axis`: same focus; `ptr.axis(...)`.
5. Keyboard: `KeyDown`/`KeyUp` need a focused surface. In `SetFocus{window_id}` (lib.rs:137), after `window_manager.set_focus`, resolve the surface and call `seat.get_keyboard().set_focus(state, Some(&surface), serial)`. Also set focus when a window maps (in `new_toplevel` ack path / `WindowManager::register`-equivalent) and clear it on `toplevel_destroyed`/`CloseWindow`.
6. Handle the no-window / no-surface case gracefully: if `surface_for(window_id)` is `None`, log `debug` and drop the event (do not panic).
7. Build on gary-agents (`cargo build --release` after `git pull`); restart server; relaunch clients **after** the viewer connects (issue 005 ordering).
8. Correct the smithay-07-api skill per the file table above.

## Verification

- `cargo test -p wayland-remote-server input_roundtrip -- --ignored` (issue 03) passes — pixels change at the click location. Remove the `#[ignore]` so default `cargo test` includes it.
- Live on gary-agents: `weston-clickdot` draws dots on click (new `surface commit` log lines appear after clicks); `weston-terminal` accepts typed text.
- `tools/drive/` (issue 04) reports PASS (`pixelsChangedAt` non-null).
- `cargo test` (full workspace, Linux) green; `cargo clippy -D warnings` green.
- `lat check` green; smithay-07-api skill updated (AGENTS.md post-task: skill files are required to stay accurate).
