# 03 — Input Round-Trip Integration Test

## Objective

A Linux `cargo test` that authoritatively verifies pointer input reaches a
client: spawn the server in-process, connect a reactive Wayland test client
that binds `wl_pointer` and commits a new buffer at the click location, inject
a pointer click over QUIC, and assert the client committed (pixels changed).
Written **red first** (fails before issue 05); goes **green** after the fix.

## Files

| File | Change |
|------|--------|
| `crates/server/tests/common/mod.rs` | Extend `XdgClient` to bind `wl_pointer`, dispatch `enter`/`motion`/`button`/`frame`, and on a button event commit a distinct "dot" buffer. |
| `crates/server/tests/input_roundtrip.rs` (new) | The test: spawn server (QUIC enabled), drive the reactive client, connect a QUIC viewer session, send a `PointerMove` + `PointerButton` via `ViewerSession::send_input`, read frames, assert pixels changed at the click location. |

## Steps

1. In `common/mod.rs`, extend `XdgClient` (around the existing `wl_seat` bind at common/mod.rs:94) to also bind `wl_pointer` and implement `Dispatch<wl_pointer::WlPointer, ()>`. Track current surface-local pointer position from `motion` events; on `button` with `ButtonState::Pressed`, call `commit_buffer` with a "dot" pattern (e.g. fill a small region around the pointer with a distinct color, leaving the rest the base color).
2. Add `XdgClient::wait_for_pointer(&mut self)` helper that roundtrips until the pointer is entered (or a timeout), so the test can sequence move→button.
3. Create `tests/input_roundtrip.rs` following the `streaming.rs`/`xdg.rs` pattern: `Runtime::new()`, spawn `run(config, shutdown, status_tx, render_rx)` on a thread with `listen` set to a free loopback port, unique socket name via `SOCKET_COUNTER`.
4. In the test: create the reactive `XdgClient` and `ack_and_commit` a base-color window; wait for `WindowEvent::Created` on the viewer; capture a baseline frame via `ViewerSession::next_frame`; send `PointerMove{click_x, click_y}` then `PointerButton{272, Pressed}` then `Released` via `session.send_input(window_id, ...)`.
5. Read frames in a loop (timeout ~2s) and assert at least one frame's pixels differ from baseline at the dot location. Use the existing `PATTERN`/`argb_to_bgra` helpers to compute expected dot bytes.
6. Leave the test **non-ignored and failing** (CI red between issue 03 and issue 05). This is the decided posture — the red test is a live signal that input is broken. **Issue 01 must land and be verified green first**, since 01 and 03 both touch `crates/server` and verify with `cargo test -p wayland-remote-server`; landing 03 before 01 would block 01's verification. Issue 05 turns this test green.

## Verification

- Before 05: `cargo test -p wayland-remote-server input_roundtrip` fails (no commit / no pixel change) — confirming the test meaningfully exercises the gap. CI is red by design.
- After 05: the same command passes; the dot appears at the click location. CI green.
- `cargo test -p wayland-remote-server` (default) is red between 03 and 05 (expected); green after 05.
- Add a `lat.md/` test spec section `tests#Input round-trip` with a `// @lat:` ref in `input_roundtrip.rs`; `lat check` green.
