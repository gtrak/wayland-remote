---
id: S07
parent: M001
milestone: M001
provides:
  - XdgShellState initialization for xdg_wm_base global advertisement
  - XdgShellHandler trait implementation for ServerState
  - new_toplevel() handler that assigns window IDs via SurfaceTracker
  - new_popup() handler for popup surface support
  - grab() and reposition_request() handlers for popup management
  - toplevel_windows HashMap for tracking surface-to-window mappings
  - Unit tests proving XDG Shell types and handler methods exist
  - Unit tests verifying surface tracker integration
  - Unit tests validating window ID allocation patterns
requires:
  - slice: S06
    provides: SurfaceTracker and window ID allocation infrastructure
affects:
  - S08 (Bidirectional Input - will use toplevel_windows mapping)
key_files:
  - crates/server/src/state.rs
  - crates/server/tests/test_xdg_shell.rs
key_decisions:
  - Use Smithay's built-in XdgShellState and delegate_xdg_shell! macro for protocol compliance
  - Map toplevel surfaces to window IDs via SurfaceTracker for consistent streaming identification
  - Store toplevel-to-window mappings in HashMap<ObjectId, u32> for lifecycle tracking
  - Popups are handled but not assigned window IDs (rendered as part of parent toplevel)
  - Follow Smallvil pattern for minimal XDG Shell implementation
patterns_established:
  - delegate_xdg_shell! macro for automatic protocol delegation
  - SurfaceTracker.allocate_window_id() in new_toplevel() for window ID assignment
  - HashMap<ObjectId, u32> for bidirectional surface-to-window lookups
  - tracing::info! for XDG Shell lifecycle events
observability_surfaces:
  - Log messages: "XDG Shell state initialized", "XDG Toplevel created", "XDG Popup created"
drill_down_paths:
  - .gsd/milestones/M001/slices/S07/
duration: 15 min
verification_result: passed
completed_at: 2026-03-12
blocker_discovered: false
---

# S07: XDG Shell Window Management

**XDG Shell protocol support with toplevel window tracking and surface-to-window ID mapping for remote streaming**

## What Happened

Implemented XDG Shell window management support following the Smallvil pattern. The XDG Shell protocol is essential for desktop applications to create proper windows (toplevels) that can be managed by the compositor and streamed to remote viewers.

The implementation adds:
1. **XdgShellState** - Smithay's state manager that advertises the `xdg_wm_base` global to clients
2. **XdgShellHandler trait** - Implemented for ServerState to handle toplevel and popup creation
3. **Window ID assignment** - Each toplevel surface gets a unique window ID via SurfaceTracker
4. **Lifecycle tracking** - The `toplevel_windows` HashMap maintains surface-to-window mappings

When a client creates an `xdg_toplevel` surface, the `new_toplevel()` handler:
- Gets the underlying wl_surface ObjectId
- Allocates a window ID via SurfaceTracker
- Stores the mapping in toplevel_windows HashMap
- Logs the creation for observability

This enables the streaming pipeline to associate captured frames with specific viewer windows.

## Verification

All unit tests pass:
- `test_xdg_shell_types_available` - Confirms Smithay XDG Shell types are importable
- `test_xdg_shell_handler_trait` - Verifies XdgShellHandler is implemented (compilation check)
- `test_toplevel_windows_tracking_structure` - Validates HashMap structure for mappings
- `test_xdg_shell_state_initialized` - Confirms XdgShellState initialization in ServerState::new()
- `test_surface_tracker_window_id_allocation` - Verifies SurfaceTracker integration
- `test_window_id_allocation_pattern` - Validates window IDs start at 1
- `test_toplevel_window_mapping_structure` - Tests mapping insertion patterns
- `test_xdg_shell_handler_methods_exist` - Confirms all required methods implemented
- `test_xdg_shell_surface_tracker_integration` - Validates integration between components
- `test_xdg_wm_base_global_advertised` - Confirms global advertisement

**Test Results:** 10 passed, 0 failed, 3 ignored (integration tests deferred)

Build verification:
```bash
cargo build -p wayland-remote-server  # Compiles with only expected unused warnings
cargo test -p wayland-remote-server   # All 32 tests pass
```

## Deviations

None - implementation followed standard Smithay patterns.

## Known Limitations

- **Toplevel destruction cleanup**: The `toplevel_windows` HashMap is not yet cleaned up when toplevels are destroyed. This requires implementing the `XdgToplevel` protocol's destroy handling, which will be addressed when implementing window close events in S08.
- **Popup parenting**: Full popup positioning relative to parent surfaces is handled by Smithay but not customized for our use case.
- **No integration tests**: Full lifecycle tests with actual Wayland clients are deferred to Phase 3.

## Follow-ups

- Implement toplevel destruction cleanup when xdg_toplevel is destroyed (S08)
- Add window title tracking via xdg_toplevel.set_title events
- Implement window state tracking (maximized, minimized, fullscreen)

## Files Created/Modified

- `crates/server/src/state.rs` — Added XdgShellState field, XdgShellHandler implementation with new_toplevel/popup handlers, toplevel_windows HashMap
- `crates/server/tests/test_xdg_shell.rs` — Comprehensive unit tests for XDG Shell functionality

## Forward Intelligence

### What the next slice should know
- Window IDs are allocated in `new_toplevel()` via `SurfaceTracker.allocate_window_id()`
- The `toplevel_windows` HashMap provides surface-to-window lookups
- XDG Shell globals are advertised automatically by Smithay when XdgShellState is created
- Popups don't get window IDs - they're rendered as part of their parent toplevel

### What's fragile
- **Toplevel destruction**: Currently no cleanup of toplevel_windows entries when surfaces are destroyed. This could cause memory growth with frequent window open/close cycles.
- **Window ID reuse**: SurfaceTracker removes mappings but doesn't reuse IDs. For long-running servers, this could exhaust the u32 space (though unlikely in practice).

### Authoritative diagnostics
- Log message "XDG Shell state initialized" confirms xdg_wm_base global is advertised
- Log message "XDG Toplevel created" with surface_id and window_id confirms mapping creation
- Compilation success of delegate_xdg_shell! macro proves all handler methods are implemented

### What assumptions changed
- Original assumption: Would need custom protocol handling for xdg_wm_base
- What actually happened: Smithay's XdgShellState and delegate_xdg_shell! macro provide complete protocol handling with minimal code
