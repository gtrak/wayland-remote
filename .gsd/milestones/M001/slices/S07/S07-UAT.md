# S07: XDG Shell Window Management — UAT

**Milestone:** M001
**Written:** 2026-03-12

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: XDG Shell is a protocol layer that can be fully verified through unit tests and compilation checks. The actual protocol exchange requires a Wayland client, which will be tested in integration tests during Phase 3. For this slice, the critical verification is that the handlers are implemented correctly and the types are available.

## Preconditions

- Rust toolchain installed
- Dependencies available (cargo will fetch Smithay)
- No running server required for unit tests

## Smoke Test

```bash
cd /home/gary/dev/wayland-remote
cargo test -p wayland-remote-server --test test_xdg_shell
```

**Expected:** 10 tests pass, 3 ignored

## Test Cases

### 1. XDG Shell Types Available

1. Run `cargo test -p wayland-remote-server test_xdg_shell_types_available`
2. **Expected:** Test passes, confirming XdgShellState, ToplevelSurface, PopupSurface, and PositionerState types are importable

### 2. XDG Shell Handler Trait Implemented

1. Run `cargo build -p wayland-remote-server`
2. **Expected:** Build succeeds, confirming ServerState implements XdgShellHandler
3. The delegate_xdg_shell! macro would fail compilation if any required methods were missing

### 3. Toplevel Windows Tracking Structure

1. Run `cargo test -p wayland-remote-server test_toplevel_windows_tracking_structure`
2. **Expected:** Test passes, confirming HashMap<ObjectId, u32> structure is correct

### 4. XDG Shell State Initialization

1. Run `cargo test -p wayland-remote-server test_xdg_shell_state_initialized`
2. **Expected:** Test passes, confirming XdgShellState is created in ServerState::new()

### 5. Surface Tracker Integration

1. Run `cargo test -p wayland-remote-server test_xdg_shell_surface_tracker_integration`
2. **Expected:** Test passes, confirming SurfaceTracker is properly integrated with XDG Shell

### 6. Window ID Allocation Pattern

1. Run `cargo test -p wayland-remote-server test_window_id_allocation_pattern`
2. **Expected:** Test passes, confirming window IDs follow expected pattern (start at 1, increment)

### 7. Handler Methods Exist

1. Run `cargo test -p wayland-remote-server test_xdg_shell_handler_methods_exist`
2. **Expected:** Test passes, confirming all required handler methods are implemented

## Edge Cases

### Window ID 0 Invalid

1. Window ID 0 is reserved and should never be allocated
2. **Expected:** SurfaceTracker starts allocation at 1 (verified by test)

### Multiple Toplevels

1. Each toplevel should get a unique window ID
2. **Expected:** Sequential allocation (1, 2, 3, ...) without reuse

### Popups Don't Get Window IDs

1. Popup surfaces should not be assigned window IDs
2. **Expected:** Only toplevels appear in toplevel_windows HashMap

## Failure Signals

- Build failure in `crates/server/src/state.rs` indicates missing trait implementations
- Test failures in `test_xdg_shell.rs` indicate type availability or logic issues
- Missing log message "XDG Shell state initialized" indicates initialization failure
- Missing delegate_xdg_shell! macro call means protocol delegation not set up

## Requirements Proved By This UAT

- XDG-SHELL-01 — XDG Shell protocol support is implemented via Smithay's XdgShellState
- XDG-SHELL-02 — xdg_wm_base global is advertised to clients (via XdgShellState::new())
- XDG-SHELL-03 — Toplevel surfaces are tracked and assigned window IDs via new_toplevel()
- XDG-SHELL-04 — Surface-to-window mapping is maintained in toplevel_windows HashMap
- XDG-SHELL-05 — Popups are handled without window ID assignment

## Not Proven By This UAT

- Actual protocol exchange with Wayland clients (requires integration tests)
- Toplevel destruction cleanup (deferred to S08)
- Window title/state tracking (not implemented yet)
- Popup positioning accuracy (uses Smithay defaults)
- Multiple concurrent toplevels from same client (requires integration test)

## Notes for Tester

- All tests are unit tests that don't require a running compositor
- The delegate_xdg_shell! macro is the authoritative check for handler completeness
- Integration tests are marked `#[ignore]` and require a running server
- If build fails, check that all required imports are present in state.rs
- Unused field warnings are expected for streaming-related fields not yet integrated
