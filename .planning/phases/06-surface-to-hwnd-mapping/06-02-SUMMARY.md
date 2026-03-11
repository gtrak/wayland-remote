---
phase: 06-surface-to-hwnd-mapping
plan: '02'
subsystem: viewer
tags: [multi-window, frame-routing, event-loop]

requires:
  - phase: 06-01
    provides: WindowManager with bidirectional mappings

provides:
  - ViewerApp uses WindowManager instead of Option<DisplayWindow>
  - Frame routing to correct window by window_id via get_or_create_window()
  - Multi-window event routing via get_window_id() reverse mapping
  - Lazy window creation on first frame arrival

affects: [viewer-architecture, frame-routing, event-handling]

tech-stack:
  added: []
  patterns: [dynamic-window-creation, event-routing]

key-files:
  modified:
    - crates/viewer/src/app.rs

key-decisions:
  - "Create windows dynamically when first frame arrives (lazy creation)"
  - "Defer window creation from resumed() to process_frames()"
  - "Route all window events via get_window_id() reverse lookup"

patterns-established:
  - "Check window exists via get_window_id() before routing events"
  - "Use get_or_create_window() for lazy window creation on frame arrival"

requirements-completed: [VIEW-03]

duration: 2 min
completed: 2026-03-11
---

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
