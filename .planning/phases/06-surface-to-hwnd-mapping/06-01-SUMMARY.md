---
phase: 06-surface-to-hwnd-mapping
plan: '01'
subsystem: viewer
tags: [multi-window, window-manager, hashmap]

requires:
  - phase: 05-03
    provides: ViewerApp with ApplicationHandler and frame channel
provides:
  - WindowManager struct with bidirectional HashMap mappings
  - HashMap window_id -> DisplayWindow for frame routing
  - HashMap WindowId -> window_id for event routing
affects: [multi-window-support, frame-routing]

tech-stack:
  added: []
  patterns: [bidirectional-lookup, hashmap-tracking]

key-files:
  created:
    - crates/viewer/src/window_manager.rs
  modified:
    - crates/viewer/src/lib.rs

key-decisions:
  - "Use HashMap<u32, DisplayWindow> for window_id -> window mapping"
  - "Use HashMap<WindowId, u32> for reverse lookup in event handling"
  - "Window ID 0 is invalid/reserved (SurfaceTracker starts at 1)"

patterns-established:
  - "HashMap::entry().or_insert_with() for atomic get-or-create"

requirements-completed: [VIEW-03]

duration: 2 min
completed: 2026-03-11
---

# Phase 06 Plan 01: WindowManager Core Summary

**WindowManager struct with bidirectional HashMap mappings for tracking multiple DisplayWindow instances with cascading positions**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-11T15:23:33Z
- **Completed:** 2026-03-11T15:25:54Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- WindowManager struct with bidirectional HashMap mappings (window_id -> DisplayWindow, WindowId -> window_id)
- Core methods: get_or_create_window(), get_window(), get_window_mut(), get_window_id(), remove_window()
- Cascading window positions with 30px offset to prevent stacking
- Module exported from lib.rs with proper cfg(windows) guards

## Task Commits

Each task was committed atomically:

1. **Task 1: Create WindowManager struct with bidirectional mappings** - `42e00ab` (feat)
2. **Task 2 & 3: Implement core methods and export module** - `cd2f4eb` (feat)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified
- `crates/viewer/src/window_manager.rs` - WindowManager struct with bidirectional HashMaps and core methods
- `crates/viewer/src/lib.rs` - Export window_manager module with cfg(windows) guard

## Decisions Made
- Used HashMap::entry().or_insert_with() pattern for atomic get-or-create operations
- Window ID 0 is reserved/invalid since SurfaceTracker starts at 1
- Cascading offset of 30px between windows to prevent stacking at origin

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added cfg(windows) guards to window_manager module**
- **Found during:** Task 3 (export module from lib.rs)
- **Issue:** window_manager.rs depends on winit and DisplayWindow which are Windows-only, causing compilation errors on non-Windows platforms
- **Fix:** Added #[cfg(windows)] guards to module declaration in lib.rs and to winit/DisplayWindow imports in window_manager.rs
- **Files modified:** crates/viewer/src/lib.rs, crates/viewer/src/window_manager.rs
- **Verification:** cargo check -p wayland-remote-viewer succeeds
- **Committed in:** cd2f4eb (Task 2 & 3 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential fix for cross-platform compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- WindowManager foundation complete, ready for integration with ViewerApp
- Multi-window tracking infrastructure in place for frame/event routing

---
*Phase: 06-surface-to-hwnd-mapping*
*Completed: 2026-03-11*
