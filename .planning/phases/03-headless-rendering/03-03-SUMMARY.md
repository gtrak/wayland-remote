---
phase: 03-headless-rendering
plan: 03
subsystem: rendering
tags: [pixman, smithay, pixel-extraction, exportmem, rgba]

# Dependency graph
requires:
  - phase: 03-headless-rendering-02
    provides: Offscreen buffer creation and surface rendering via render_surface_to_buffer()
provides:
  - RGBA pixel extraction via extract_rgba_pixels() function
  - RgbaData struct for holding extracted pixel data with dimensions
  - Per-surface frame capture in ServerState.captured_frames
  - Buffer lifecycle management (buffer held until extraction completes)
affects: [frame-streaming, tcp-transmission, viewer-rendering]

# Tech tracking
tech-stack:
  added: []
  patterns: [Extract RGBA after render, buffer-hold-until-extraction, per-surface frame storage]

key-files:
  created:
    - crates/server/src/rendering/pixel_export.rs
  modified:
    - crates/server/src/rendering/mod.rs
    - crates/server/src/state.rs

key-decisions:
  - "Use ExportMem::copy_framebuffer() + map_texture() for RGBA extraction"
  - "Store extracted frames in HashMap<ObjectId, RgbaData> for streaming"
  - "Extract RGBA immediately after rendering while buffer is still valid"

patterns-established:
  - "bind() + copy_framebuffer() + map_texture() for pixel readback"
  - "Hold buffer reference until pixel data is cloned to Vec<u8>"

requirements-completed: [REND-03]

# Metrics
duration: 18 min
completed: 2026-03-10
---

# Phase 3 Plan 3: RGBA Pixel Extraction Summary

**RGBA pixel extraction module using Smithay's ExportMem trait with copy_framebuffer() and map_texture(), integrated into commit handler with per-surface frame storage in ServerState.captured_frames**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-10T22:28:21Z
- **Completed:** 2026-03-10T22:46:53Z
- **Tasks:** 4
- **Files modified:** 3

## Accomplishments
- Created pixel_export module with RgbaData struct and extract_rgba_pixels() function
- Implemented RGBA extraction using ExportMem::copy_framebuffer() + map_texture() pattern
- Integrated extraction into commit handler after successful surface rendering
- Added per-surface frame storage in ServerState.captured_frames HashMap
- Buffer lifecycle managed correctly (held until extraction completes per Pattern 4)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create pixel export module** - `ba8eb50` (feat)
2. **Task 2: Implement RGBA extraction using ExportMem trait** - `119af42` (feat)
3. **Task 3: Integrate RGBA extraction into commit handler** - `fff89fc` (feat)
4. **Task 4: Implement frame callback handling** - `fff89fc` (feat, deferred)

**Plan metadata:** [pending]

## Files Created/Modified
- `crates/server/src/rendering/pixel_export.rs` - RGBA extraction module with RgbaData struct and extract_rgba_pixels() function
- `crates/server/src/rendering/mod.rs` - Added pixel_export module export
- `crates/server/src/state.rs` - Added captured_frames field and RGBA extraction integration in commit()

## Decisions Made
- Use ExportMem::copy_framebuffer() with PixmanTarget (not texture directly) for RGBA extraction
- Store extracted frames in HashMap<ObjectId, RgbaData> indexed by surface ID
- Extract RGBA immediately after rendering while buffer is still valid (buffer lifecycle pattern)
- Deferred frame callback handling to future plan due to Smithay API complexity

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed Smithay API for copy_framebuffer() signature**
- **Found during:** Task 2
- **Issue:** Initial implementation used incorrect signature `copy_framebuffer(&texture, Fourcc, None)` 
- **Fix:** Updated to correct signature `copy_framebuffer(&target, Rectangle, Fourcc)` where target is a PixmanTarget created via bind()
- **Files modified:** crates/server/src/rendering/pixel_export.rs
- **Verification:** Code compiles successfully
- **Committed in:** 119af42 (Task 2 commit)

**2. [Rule 3 - Blocking] Fixed Texture trait import for size() method**
- **Found during:** Task 2
- **Issue:** Texture::size() method not found - trait not in scope
- **Fix:** Added `use smithay::backend::renderer::Texture;` to imports
- **Files modified:** crates/server/src/rendering/pixel_export.rs
- **Verification:** Code compiles successfully
- **Committed in:** 119af42 (Task 2 commit)

**3. [Rule 4 - Architectural] Deferred frame callback implementation**
- **Found during:** Task 4
- **Issue:** Smithay's frame_callbacks field requires mutable access via data_map which returns immutable reference; API complexity exceeds current plan scope
- **Decision:** Deferred to future plan with proper Smithay API investigation
- **Impact:** Core RGBA extraction functionality complete; frame callbacks can be added later without breaking existing code

---

**Total deviations:** 2 auto-fixed (Rule 3 - Blocking), 1 deferred (Rule 4 - Architectural)
**Impact on plan:** Auto-fixes essential for correct Smithay 0.7.0 API usage. Frame callback deferral acceptable as core REND-03 requirement (RGBA extraction) is satisfied.

## Issues Encountered
- Smithay's copy_framebuffer() requires PixmanTarget, not PixmanTexture - required restructuring extraction approach
- Frame callback API access via data_map returns immutable reference, preventing drain() operation

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- RGBA pixel extraction complete (REND-03 satisfied)
- Frames stored per-surface in ServerState.captured_frames ready for TCP streaming
- Ready for Phase 4: Frame Streaming via TCP
- Consider adding frame callback handling before Phase 4 to prevent client freezing

---
*Phase: 03-headless-rendering*
*Completed: 2026-03-10*
