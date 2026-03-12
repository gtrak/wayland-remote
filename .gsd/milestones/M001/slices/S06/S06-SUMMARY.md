---
id: S06
parent: M001
milestone: M001
provides:
  - WindowManager struct with bidirectional HashMap mappings
  - HashMap window_id -> DisplayWindow for frame routing
  - HashMap WindowId -> window_id for event routing
  - ViewerApp uses WindowManager instead of Option<DisplayWindow>
  - Frame routing to correct window by window_id via get_or_create_window()
  - Multi-window event routing via get_window_id() reverse mapping
  - Lazy window creation on first frame arrival
  - Per-window resize handling with 10% threshold
  - Aspect ratio preservation via StretchDIBits
  - Multi-window resize event routing
  - Window close event handling (CloseRequested)
  - Proper window destruction via remove_window()
  - Application exit when all windows closed
  - Cascading window positions (30px offset)
  - Lifecycle tracing for window creation and removal
requires: []
affects: []
key_files: []
key_decisions:
  - "Use HashMap<u32, DisplayWindow> for window_id -> window mapping"
  - "Use HashMap<WindowId, u32> for reverse lookup in event handling"
  - "Window ID 0 is invalid/reserved (SurfaceTracker starts at 1)"
  - "Create windows dynamically when first frame arrives (lazy creation)"
  - "Defer window creation from resumed() to process_frames()"
  - "Route all window events via get_window_id() reverse lookup"
  - "Preserve existing 10% resize threshold from Phase 5 (per-window)"
  - "StretchDIBits already handles aspect ratio preservation"
  - "Each window resizes independently"
  - "Destroy window on CloseRequested event (viewer-side only)"
  - "Exit application when last window is closed"
  - "Cascading positions (30px offset) to avoid window stacking"
patterns_established:
  - "HashMap::entry().or_insert_with() for atomic get-or-create"
  - "Check window exists via get_window_id() before routing events"
  - "Use get_or_create_window() for lazy window creation on frame arrival"
  - "Check window exists before routing resize events"
  - "is_empty() check before event_loop.exit()"
  - "Remove window from both HashMaps on close"
  - "tracing::info! for lifecycle events"
observability_surfaces: []
drill_down_paths: []
duration: 4 min
verification_result: passed
completed_at: 2026-03-11
blocker_discovered: false
---
# S06: Surface To Hwnd Mapping

**# Phase 06 Plan 01: WindowManager Core Summary**

## What Happened

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

# Phase 06 Plan 02: ViewerApp Multi-Window Integration Summary

**Integrated WindowManager into ViewerApp, replacing single-window Option<DisplayWindow> with multi-window support via lazy window creation and bidirectional event routing**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-11T15:44:54Z
- **Completed:** 2026-03-11T15:47:26Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments
- Replaced `display_window: Option<DisplayWindow>` with `window_manager: WindowManager` in ViewerApp
- Implemented lazy window creation in `process_frames()` via `get_or_create_window()` - windows are created when first frame arrives
- Updated `resumed()` to remove window creation logic (now deferred to process_frames)
- Implemented multi-window event routing in `window_event()` using `get_window_id()` reverse lookup
- Added proper window cleanup on CloseRequested with automatic shutdown when last window closes

## Task Commits

Each task was committed atomically:

1. **Task 1-3: Replace Option<DisplayWindow> with WindowManager and implement multi-window routing** - `9208b37` (feat)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified
- `crates/viewer/src/app.rs` - Integrated WindowManager with lazy window creation and multi-window event routing

## Decisions Made
- Lazy window creation: Windows are created in `process_frames()` when the first frame arrives for a window_id, not in `resumed()`
- Window cleanup: When a window is closed, it's removed from WindowManager; app shuts down when no windows remain
- Event routing: All window events use `get_window_id()` for reverse lookup before processing

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- WindowManager fully integrated with ViewerApp
- Multi-window frame routing operational via `get_or_create_window()`
- Multi-window event routing operational via `get_window_id()`
- Ready for Phase 06 Plan 03 (Frame routing verification) and Plan 04 (Event routing implementation)

---
*Phase: 06-surface-to-hwnd-mapping*
*Completed: 2026-03-11*

## Self-Check: PASSED

# Phase 06 Plan 03: Window Resize Handling Summary

**Implemented per-window resize event routing with 10% threshold verification and confirmed aspect ratio preservation via StretchDIBits**

## Performance

- **Duration:** 1 min
- **Started:** 2026-03-11T15:58:44Z
- **Completed:** 2026-03-11T15:59:56Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- Added resize event routing in ViewerApp::window_event() to route WindowEvent::Resized to correct DisplayWindow
- Added handle_resize() method to DisplayWindow for resize event handling
- Verified per-window 10% resize threshold in submit_frame() prevents feedback loops
- Verified GdiRenderer::render() preserves aspect ratio via StretchDIBits with letterboxing/pillarboxing

## Task Commits

Each task was committed atomically:

1. **Task 1: Route resize events to correct window** - `df4b50e` (feat)
2. **Task 2-3: Verify per-window resize threshold and aspect ratio preservation** - `62cfd67` (feat)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified
- `crates/viewer/src/app.rs` - Added resize event routing to correct DisplayWindow via get_window_mut()
- `crates/viewer/src/display/window.rs` - Added handle_resize() method for resize event handling

## Decisions Made
- Preserve existing 10% resize threshold implementation (already per-window via DisplayWindow fields)
- Use existing StretchDIBits aspect ratio preservation (already implemented with letterboxing/pillarboxing)
- Each window independently tracks its resize threshold via last_resized_width/last_resized_height

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Resize event routing complete with proper window lookup
- 10% threshold prevents resize feedback loops
- Aspect ratio preservation verified in GDI renderer
- Ready for Phase 06 Plan 04 (Event routing implementation)

---
*Phase: 06-surface-to-hwnd-mapping*
*Completed: 2026-03-11*
## Self-Check: PASSED

# Phase 06 Plan 04: Window Lifecycle Management Summary

**Implemented window lifecycle tracing with tracing::info! logs for window creation and removal, added unit test for lifecycle verification, verified existing CloseRequested handling and cascading positions**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-11T16:12:40Z
- **Completed:** 2026-03-11T16:17:08Z
- **Tasks:** 3 (Tasks 1-2 already complete, Task 3 executed)
- **Files modified:** 1

## Accomplishments
- Added tracing::info! log for window creation with window_id, dimensions, and position in WindowManager::get_or_create_window()
- Added tracing::info! log for window removal in WindowManager::remove_window()
- Added unit test test_window_lifecycle_is_empty_and_remove() for lifecycle verification
- Verified existing CloseRequested handling in app.rs removes window and exits when is_empty()
- Verified cascading positions (30px offset) already implemented in WindowManager
- Verified is_empty() method already exists for application exit logic

## Task Commits

Each task was committed atomically:

1. **Task 3: Add lifecycle tracing and verification** - `07b1ad7` (feat)

**Plan metadata:** pending (docs: complete plan)

_Note: Tasks 1 and 2 were already completed in previous plans (06-01, 06-02, 06-03)_

## Files Created/Modified
- `crates/viewer/src/window_manager.rs` - Added tracing::info! logs for window creation and removal, added unit test for lifecycle verification

## Decisions Made
- None - followed plan as specified, most functionality already implemented

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Tasks 1-2 already completed in previous plans**
- **Found during:** Task analysis
- **Issue:** WindowManager::is_empty(), remove_window() returning Option, and cascading positions were already implemented in plans 06-01, 06-02, and 06-03
- **Fix:** Focused Task 3 on adding lifecycle tracing and unit test as the remaining work
- **Files modified:** crates/viewer/src/window_manager.rs
- **Verification:** cargo build -p wayland-remote-viewer succeeds
- **Committed in:** 07b1ad7 (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Plan executed with focus on remaining Task 3 work. Tasks 1-2 functionality verified as already complete.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Window lifecycle management complete with tracing
- CloseRequested handling verified and working
- Application exit on last window close implemented
- Cascading positions prevent window stacking
- Ready for Phase 7 (XDG Toplevel Protocol)

---
*Phase: 06-surface-to-hwnd-mapping*
*Completed: 2026-03-11*

## Self-Check: PASSED
