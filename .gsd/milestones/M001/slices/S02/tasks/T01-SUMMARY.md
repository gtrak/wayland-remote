---
id: T01
parent: S02
milestone: M001
provides:
  - Wayland compositor accepting client connections
  - wl_compositor global advertised
  - calloop event loop integration
  - ServerState struct with CompositorState
requires: []
affects: []
key_files: []
key_decisions: []
patterns_established: []
observability_surfaces: []
drill_down_paths: []
duration: 11 min
verification_result: passed
completed_at: 2026-03-10
blocker_discovered: false
---
# T01: 02-wayland-core-protocol 01

**# Phase 02 Plan 01: Wayland Core Protocol Summary**

## What Happened

# Phase 02 Plan 01: Wayland Core Protocol Summary

**Smithay-based headless compositor with calloop event loop, wl_compositor global advertised, accepting client connections via auto-named Wayland socket**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-10T11:48:04Z
- **Completed:** 2026-03-10T11:59:47Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Added Wayland dependencies (smithay with wayland_frontend, calloop, wayland-server, wayland-protocols)
- Created ServerState struct implementing CompositorHandler with CompositorState
- Implemented calloop-based event loop replacing tokio placeholder
- Server binary creates Wayland socket and advertises wl_compositor global

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Wayland dependencies to server Cargo.toml** - `fc3e87d` (feat)
2. **Task 2: Create ServerState struct with Smithay states** - `393fae4` (feat)
3. **Task 3: Rewrite main.rs to use calloop event loop** - `aab2f42` (feat)

**Plan metadata:** (pending final commit)

## Files Created/Modified
- `crates/server/Cargo.toml` - Added smithay wayland_frontend feature, calloop 0.14.0, wayland-server 0.31.9, wayland-protocols 0.32.8
- `crates/server/src/state.rs` - ServerState struct, ClientState with ClientData impl, CompositorHandler implementation
- `crates/server/src/main.rs` - calloop event loop, Display initialization, ServerState::new() call

## Decisions Made
- **calloop over tokio**: Smithay is built around calloop's callback model; tokio remains in dev-dependencies for Phase 4 TCP streaming
- **Smallvil pattern**: Followed minimal compositor pattern from Smithay's smallvil example
- **Auto-named socket**: Using ListeningSocketSource::new_auto() which creates socket in XDG_RUNTIME_DIR with auto-generated name (wayland-0, wayland-1, etc.)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed ClientData trait import path**
- **Found during:** Task 2 (ServerState implementation)
- **Issue:** ClientData and DisconnectReason are in `wayland_server::backend` module, not root
- **Fix:** Changed import to `smithay::reexports::wayland_server::backend::{ClientData, DisconnectReason}`
- **Files modified:** crates/server/src/state.rs
- **Verification:** cargo check passes
- **Committed in:** 393fae4 (Task 2 commit)

**2. [Rule 3 - Blocking] Fixed dispatch_clients() API call**
- **Found during:** Task 2 (event loop integration)
- **Issue:** Display's dispatch() method doesn't exist; must use dispatch_clients(state) with ServerState
- **Fix:** Changed from `display.get_mut().dispatch()` to `display.get_mut().dispatch_clients(state)`
- **Files modified:** crates/server/src/state.rs
- **Verification:** cargo check passes, server runs
- **Committed in:** 393fae4 (Task 2 commit)

**3. [Rule 3 - Blocking] Fixed CompositorHandler trait method signatures**
- **Found during:** Task 2 (CompositorHandler implementation)
- **Issue:** ClientData trait uses ClientId type, not Client; client_compositor_state needs borrow
- **Fix:** Updated signatures to use ClientId, added &client borrow in client_compositor_state
- **Files modified:** crates/server/src/state.rs
- **Verification:** cargo check passes
- **Committed in:** 393fae4 (Task 2 commit)

**4. [Rule 3 - Blocking] Fixed SurfaceAttributes access pattern**
- **Found during:** Task 2 (commit handler implementation)
- **Issue:** cached_state.current() doesn't exist; must use cached_state.get() with drop()
- **Fix:** Changed to `drop(states.cached_state.get::<SurfaceAttributes>())`
- **Files modified:** crates/server/src/state.rs
- **Verification:** cargo check passes
- **Committed in:** 393fae4 (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (all Rule 3 - blocking API compatibility issues)
**Impact on plan:** All auto-fixes necessary for Smithay 0.7.0 API compatibility. No scope creep.

## Issues Encountered
None - all API issues resolved via deviation rules.

## User Setup Required

None - no external service configuration required.

**Socket path documentation:**
- Socket created at: `/run/user/{uid}/wayland-{N}` (auto-named)
- Set environment variable: `export WAYLAND_DISPLAY=wayland-{N}`
- Server prints socket name on startup (e.g., "wayland-1")

## Next Phase Readiness
- Core compositor infrastructure complete
- Ready for Phase 2 Plan 02 (surface rendering or XDG shell)
- wl_compositor global advertised and functional
- Event loop dispatching client connections

---
*Phase: 02-wayland-core-protocol*
*Completed: 2026-03-10*

## Self-Check: PASSED
- All key files exist on disk
- All commits present in git history
