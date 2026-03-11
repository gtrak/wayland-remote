---
phase: 04-tcp-frame-streaming
plan: '03'
subsystem: streaming
tags: [surface-tracking, window-id, multi-surface, streaming]

# Dependency graph
requires:
  - phase: 04-tcp-frame-streaming-02
    provides: TCP client connection handler and frame streaming infrastructure
provides:
  - SurfaceTracker struct for unique window ID management
  - Multi-surface tracking with ObjectId -> window_id mappings
  - Surface lifecycle management (allocate, lookup, remove)
  - Integration with ServerState for streaming
affects: [tcp-transmission, viewer-rendering, multi-window-support]

# Tech tracking
tech-stack:
  added: []
  patterns: [atomic-id-allocation, bidirectional-mapping, surface-lifecycle-tracking]

key-files:
  created:
    - crates/server/src/streaming/surface.rs
  modified:
    - crates/server/src/streaming/mod.rs
    - crates/server/src/state.rs

key-decisions:
  - "Use std::sync::RwLock instead of parking_lot to avoid dependency"
  - "Wrap SurfaceTracker in Arc for sharing across threads"
  - "Atomic counter for unique window ID generation"

patterns-established:
  - "allocate_window_id() for stable surface-to-window mapping"
  - "Bidirectional HashMap for ObjectId <-> window_id lookups"
  - "Surface lifecycle: allocate on first commit, remove on destruction"

requirements-completed: [STREAM-04]

# Metrics
duration: 9 min
completed: 2026-03-11
---

# Phase 4 Plan 3: Multi-Surface Tracking Summary

**SurfaceTracker module for unique window ID management with bidirectional ObjectId <-> window_id mappings, integrated into ServerState for multi-surface streaming**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-11T01:09:36Z
- **Completed:** 2026-03-11T01:19:10Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Created surface.rs with SurfaceTracker struct for window ID management
- Implemented allocate_window_id(), get_window_id(), remove_surface() methods
- Integrated SurfaceTracker into ServerState with Arc wrapper
- Updated get_frames_for_streaming() to use SurfaceTracker for stable IDs
- Updated remove_streaming_surface() to handle surface destruction properly
- Added comprehensive unit tests for SurfaceTracker

## Task Commits

Each task was committed atomically:

1. **Task 1: Create SurfaceTracker for window ID management** - `9d108b6` (feat)
2. **Task 2: Integrate SurfaceTracker with ServerState** - `13617fc` (feat)

**Plan metadata:** `714dea6` (docs: complete plan)

## Files Created/Modified
- `crates/server/src/streaming/surface.rs` - SurfaceTracker module with unique window ID allocation and bidirectional mappings
- `crates/server/src/streaming/mod.rs` - Added surface module export
- `crates/server/src/state.rs` - Integrated SurfaceTracker, updated frame streaming methods

## Decisions Made
- Used std::sync::RwLock instead of parking_lot to avoid adding a new dependency
- Wrapped SurfaceTracker in Arc for sharing across tokio tasks
- Used AtomicU32 with SeqCst ordering for unique ID generation
- Removed old window_id_map and next_window_id fields in favor of SurfaceTracker

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Multi-surface tracking complete (STREAM-04 satisfied)
- Each Wayland surface now has a unique, stable window ID
- Surface destruction properly removes from streaming state
- Ready for TCP frame streaming with multiple surfaces
- Ready for viewer-side window management

---
*Phase: 04-tcp-frame-streaming*
*Completed: 2026-03-11*

## Self-Check: PASSED
