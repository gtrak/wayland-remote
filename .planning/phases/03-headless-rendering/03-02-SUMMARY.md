---
phase: 03-headless-rendering
plan: 02
subsystem: rendering
tags: [pixman, smithay, offscreen, buffer-rendering]

# Dependency graph
requires:
  - phase: 03-headless-rendering-01
    provides: PixmanRenderer initialized in ServerState
provides:
  - Offscreen buffer creation via create_offscreen_buffer()
  - Surface rendering via render_surface_to_buffer()
  - Per-surface buffer tracking in ServerState.offscreen_buffers
  - Integrated rendering in CompositorHandler::commit()
affects: [frame-streaming, pixel-extraction, buffer-lifecycle]

# Tech tracking
tech-stack:
  added: []
  patterns: [Offscreen buffer per surface, render-on-commit pattern]

key-files:
  created:
    - crates/server/src/rendering/mod.rs
    - crates/server/src/rendering/offscreen.rs
  modified:
    - crates/server/src/state.rs
    - crates/server/src/lib.rs

key-decisions:
  - "Use HashMap<ObjectId, Image> for per-surface buffer tracking"
  - "Render on commit when buffer attached"
  - "Reuse buffers across commits, recreate on dimension change"

patterns-established:
  - "Import buffer via ImportMemWl to get dimensions"
  - "Use try_render_surface_to_buffer for non-fatal error handling"

requirements-completed: [REND-02]

# Metrics
duration: 16 min
completed: 2026-03-10
---

# Phase 3 Plan 2: Offscreen Surface Rendering Summary

**Offscreen buffer creation and surface rendering integrated into commit handler, enabling headless frame capture via PixmanRenderer with per-surface buffer tracking in ServerState**

## Performance

- **Duration:** 16 min
- **Started:** 2026-03-10T21:13:17Z
- **Completed:** 2026-03-10T21:29:00Z
- **Tasks:** 4
- **Files modified:** 4

## Accomplishments
- Created rendering module with offscreen buffer management
- Implemented render_surface_to_buffer() using PixmanRenderer traits
- Added per-surface offscreen buffer tracking to ServerState
- Integrated rendering into CompositorHandler::commit() for automatic frame capture

## Task Commits

Each task was committed atomically:

1. **Task 1: Create rendering module structure** - `9471248` (feat)
2. **Task 2: Implement offscreen buffer creation and surface rendering** - `bea965f` (feat)
3. **Task 3: Add per-surface buffer tracking to ServerState** - `6d70655` (feat)
4. **Task 4: Integrate rendering into commit handler** - `10e2956` (feat)

**Plan metadata:** [pending]

## Files Created/Modified
- `crates/server/src/rendering/mod.rs` - Module exports for offscreen rendering
- `crates/server/src/rendering/offscreen.rs` - Buffer creation and surface rendering functions
- `crates/server/src/state.rs` - Added offscreen_buffers field and commit integration
- `crates/server/src/lib.rs` - Added rendering module export

## Decisions Made
- Use HashMap<ObjectId, Image> for per-surface buffer tracking (efficient lookup by surface ID)
- Render on commit when buffer attached (follows Smithay surface lifecycle pattern)
- Reuse buffers across commits, only recreate when needed (performance optimization)
- Use try_render_surface_to_buffer for non-fatal error handling (don't crash on render failures)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed Smithay API imports and signatures**
- **Found during:** Task 2
- **Issue:** Initial implementation used incorrect API signatures for Smithay 0.7.0
- **Fix:** Updated imports to use smithay::backend::allocator::Fourcc and smithay::reexports::pixman::Image; fixed render_texture_from_to signature with correct Rectangle and Point types
- **Files modified:** crates/server/src/rendering/offscreen.rs
- **Verification:** Code compiles successfully
- **Committed in:** bea965f (Task 2 commit)

**2. [Rule 3 - Blocking] Fixed SurfaceData lifetime issue**
- **Found during:** Task 2
- **Issue:** Could not return SurfaceData reference from with_states closure due to lifetime constraints
- **Fix:** Restructured to call import_shm_buffer inside with_states closure where SurfaceData is valid
- **Files modified:** crates/server/src/rendering/offscreen.rs
- **Verification:** Code compiles successfully
- **Committed in:** bea965f (Task 2 commit)

**3. [Rule 3 - Blocking] Fixed buffer dimension retrieval**
- **Found during:** Task 4
- **Issue:** WlBuffer doesn't have direct width/height methods
- **Fix:** Import buffer via ImportMemWl to get texture, then call texture.size() to get dimensions
- **Files modified:** crates/server/src/state.rs
- **Verification:** Code compiles and builds successfully
- **Committed in:** 10e2956 (Task 4 commit)

---

**Total deviations:** 3 auto-fixed (all Rule 3 - Blocking issues with Smithay API)
**Impact on plan:** All auto-fixes necessary for correct Smithay 0.7.0 API usage. No scope creep.

## Issues Encountered

None - all API issues were resolved through auto-fixes during implementation.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Offscreen rendering foundation complete (REND-02 satisfied)
- Ready for Phase 3 Plan 3: RGBA pixel extraction via ExportMem trait
- Surfaces are now rendered to offscreen buffers on each commit
- Next: Extract RGBA data from buffers for TCP streaming

---
*Phase: 03-headless-rendering*
*Completed: 2026-03-10*
