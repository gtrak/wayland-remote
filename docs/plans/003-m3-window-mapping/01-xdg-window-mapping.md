# Issue 01 — xdg-shell ↔ HWND mapping

## Objective

(PRD Step 6) Each xdg_toplevel maps to a `window_id`; server renders per-window, emits `WindowEvent`s; viewer maintains one HWND per window with focus/close/resize wired both ways.

## Files

| File | Change |
|---|---|
| `crates/server/src/handlers/xdg_shell.rs` | `delegate_xdg_shell!` + `XdgShellHandler` impl: toplevel lifecycle, configure/ack, close requests, states |
| `crates/server/src/window.rs` | `WindowManager`: `window_id` allocation (monotonic u64), `Window { surface, toplevel, size, title, mapped, focus }`, stacking order, focus switching, render-target-per-window |
| `crates/server/src/rendering/mod.rs` | Render per-window `PixmanTarget` instead of one full-desktop target; window layout becomes stack-of-windows (only focused visible? **No** — render all mapped windows, viewer composites via HWND z-order; overlapping windows render their full extent) |
| `crates/server/src/state.rs` | Wire `WindowManager` into commit handler (map/unmap transitions → `WindowEvent::Created/Destroyed`), input focus follows `WindowManager::focused()` |
| `crates/server/src/input/mod.rs` | Focus switching on viewer `Focused`/`Unfocused` input control messages (add `Message::Input` sibling: `Message::SetFocus { window_id }` — protocol minor version bump, kept backward compatible by treating unknown messages as errors on old peers) |
| `crates/viewer/src/window_manager.rs` | HWND lifecycle keyed by `window_id`: create on `Created`, destroy on `Destroyed`, resize on `Resized`, title from `Created` |
| `crates/viewer/src/display/win.rs` | Per-window message loop/WNDPROC instances; frame demux by `window_id` → correct window's `FrameBuffer` |
| `crates/viewer/tests/windows.rs` | Loopback tests for event handling |
| `crates/server/tests/xdg.rs` | xdg-shell lifecycle tests via test client |

## Implementation notes

- **Smithay 0.7 xdg**: `smithay::wayland::shell::xdg` module — `XdgShellHandler`, `delegate_xdg_shell!`, `ToplevelSurface` with `send_configure`, `send_close`, `with_pending_state`/`ack_configure` flow, `initial_configure` requirement (first configure must be sent before the client's first buffer commit is meaningful). Smallvil and `anvil`'s xdg handlers are the reference implementations on this API line.
- **The initial-configure trap**: a toplevel isn't renderable until it has acked its first configure. `WindowManager` only creates the pixman target + emits `Created` when the surface commits *after* `is_acked()`; before that the window exists internally but is invisible on the wire. Test this explicitly.
- **Resize negotiation**: Windows `WM_SIZING`/`WM_SIZE` → viewer holds the resize locally (framebuffer stretches momentarily) AND sends `WindowEvent`-equivalent upstream — add `Message::ConfigureWindow { window_id, width, height }` (viewer→server). Server calls `ToplevelSurface::send_configure` with the new size; on ack + commit, new-size frames flow back; viewer then resizes its `FrameBuffer` to match exactly (discard stretched mode). Throttle upstream configure to ~30Hz during interactive drags.
- **Wire protocol changes** (bump `Hello.version` to 2): `SetFocus`, `ConfigureWindow`, and `WindowEventKind::Created` gains `title: String` (already in the M1 protocol). Old peers fail version handshake cleanly.
- **Close**: HWND `WM_CLOSE` → `Message::CloseWindow { window_id }` → server `ToplevelSurface::send_close()` → client destroys → `WindowEvent::Destroyed` → viewer destroys HWND. If the app ignores close for >2s, viewer force-destroys the HWND and sends a `DestroyWindow` message that makes the server drop the surface (app gets disconnected — matches native behavior of killing a hung window's process at the protocol level as closely as Wayland allows).
- **Focus**: viewer `WM_ACTIVATE` → `SetFocus { window_id }` → server switches `WindowManager` focus, sends xdg `activated`/`deactivated` states in the next configure, retargets keyboard focus. Initial focus: first mapped window.
- **Cursor**: server sends the client-set cursor buffer content as part of frame metadata (or M3 fallback: viewer always draws the default arrow — choose fallback, note in decisions).
- **Frame demux**: each frame stream already carries `window_id` (M1 header); viewer's session routes to per-window `FrameBuffer`s. Skip-stale stays per-connection (simplest) — per-window coalescing in each window's `FrameBuffer` handles slow windows.

## Steps (execution order within the issue)

1. Protocol v2 messages + round-trip tests.
2. Server `WindowManager` + xdg handler; test client gains xdg-shell support (toplevel creation, ack, set title, commit buffers of varied sizes).
3. Per-window rendering; snapshot tests per window id.
4. WindowEvent emission on lifecycle transitions; loopback tests.
5. Viewer HWND lifecycle + frame demux; `--headless` mode extended to create N logical windows.
6. Focus/close/resize round-trips end-to-end.
7. `lat.md/` updates throughout; final `lat check`.

## Verification

- Test `toplevel_lifecycle`: client creates xdg_toplevel → initial configure sent → client acks + commits → `WindowEvent::Created` with correct size/title on the wire; client destroy → `Destroyed`.
- Test `initial_configure_before_created`: no `Created` event before first ack+commit even if buffers were committed pre-configure.
- Test `per_window_render`: two toplevels with distinct patterns → two frame streams, each readback matches its pattern; window overlap does not bleed pixels between windows.
- Test `resize_negotiation`: viewer sends `ConfigureWindow { 256, 256 }` → server configure → client ack+commit 256x256 → frames arrive at 256x256.
- Test `focus_roundtrip`: `SetFocus { B }` while A focused → xdg states: A deactivated, B activated; keyboard events route to B's surface.
- Test `close_roundtrip`: `CloseWindow` → client receives wm-close → client destroys → `Destroyed` on wire.
- Loopback: viewer headless creates two logical windows from two `Created` events, demuxes frames correctly.
- Manual on Windows box: run two terminals + a GTK app (e.g. `gtk3-demo` or `gnome-calculator`) — three independent HWNDs, typing focuses correctly, close works, resize works.
- Clippy/fmt clean; `lat check` green.
