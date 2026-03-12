# S02: Wayland Core Protocol — UAT

**Milestone:** M001
**Written:** 2026-03-12

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: Core protocol implementation is verified through compilation and unit tests. Live runtime verification requires a full Wayland client which is deferred to S07 (XDG Shell Window Management).

## Preconditions

- Rust toolchain 1.85+ installed
- `cargo build --package wayland-remote-server` completes successfully
- `cargo test --package wayland-remote-server` passes

## Smoke Test

```bash
cd /home/gary/dev/wayland-remote
cargo build --package wayland-remote-server
cargo test --package wayland-remote-server
```

Expected: Build succeeds with warnings (dead code expected). All 17 tests pass, 3 ignored.

## Test Cases

### 1. Verify ServerState Structure

1. Inspect `crates/server/src/state.rs`
2. Confirm ServerState contains:
   - `compositor_state: CompositorState`
   - `shm_state: ShmState`
   - `seat_state: SeatState`
   - `output_manager_state: OutputManagerState`
   - `surface_tracker: Arc<SurfaceTracker>`

**Expected:** All fields present and properly typed.

### 2. Verify Handler Modules

1. Check `crates/server/src/handlers/seat.rs` exists
2. Check `crates/server/src/handlers/output.rs` exists
3. Verify both modules are exported in `crates/server/src/handlers/mod.rs`

**Expected:** Both handler modules present with SeatHandler and output creation functions.

### 3. Verify SurfaceTracker API

1. Inspect `crates/server/src/streaming/surface.rs`
2. Confirm SurfaceTracker has methods:
   - `allocate_window_id(&self, ObjectId) -> u32`
   - `get_window_id(&self, ObjectId) -> Option<u32>`
   - `get_surface_id(&self, u32) -> Option<ObjectId>`
   - `remove_surface(&self, ObjectId) -> Option<u32>`

**Expected:** All methods present with correct signatures.

### 4. Verify Test Coverage

1. Run `cargo test --package wayland-remote-server -- --list`
2. Count tests in each module

**Expected:**
- streaming::surface::tests: 3 tests (test_object_id_null, test_object_id_type, test_surface_tracker_new)
- streaming::client::tests: 2 tests (test_client_registration, test_bounded_channel_backpressure)
- streaming::protocol::tests: 5 tests
- test_surface_lifecycle.rs: 5 tests (2 ignored)

## Edge Cases

### ObjectId Null Handling

1. Check `test_object_id_null` test in surface.rs
2. Verify `ObjectId::null()` can be created and `is_null()` returns true

**Expected:** Test passes, null ObjectId handled correctly.

### Surface Tracker Empty State

1. Check `test_surface_tracker_new` test
2. Verify new tracker has surface_count() == 0

**Expected:** Empty tracker returns 0 for surface_count().

## Failure Signals

- Build errors in state.rs — trait implementation issues
- Test failures in streaming::surface — ObjectId or HashMap problems
- Missing handler modules — incomplete file structure

## Requirements Proved By This UAT

- WAYL-01 — Core Wayland globals (wl_compositor, wl_seat, wl_output) present in ServerState
- WAYL-02 — Surface lifecycle tracking via CompositorHandler (verified by compilation)
- STREAM-01 — SurfaceTracker provides ObjectId -> window_id mapping

## Not Proven By This UAT

- Live client connections (requires running compositor)
- Actual surface rendering (S03)
- TCP frame streaming (S04)
- Windows viewer display (S05-S06)
- XDG shell window management (S07)
- Input handling (S08)

## Notes for Tester

- Dead code warnings are expected — fields like `seat`, `output` will be used in S07-S08
- Ignored tests require full Wayland client integration (deferred to S07)
- Surface destruction cleanup is deferred — surfaces tracked in HashMap not removed until client disconnect
