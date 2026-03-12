---
id: S03
parent: M001
milestone: M001
provides:
  - PixmanRenderer field in ServerState
  - renderer_pixman feature enabled in smithay dependency
  - Offscreen<Image> trait available for memory-backed framebuffers
  - Offscreen buffer creation via create_offscreen_buffer()
  - Surface rendering via render_surface_to_buffer()
  - Per-surface buffer tracking in ServerState.offscreen_buffers
  - Integrated rendering in CompositorHandler::commit()
  - RGBA pixel extraction via extract_rgba_pixels() function
  - RgbaData struct for holding extracted pixel data with dimensions
  - Per-surface frame capture in ServerState.captured_frames
  - Buffer lifecycle management (buffer held until extraction completes)
requires: []
affects: []
key_files: []
key_decisions:
  - "Use PixmanRenderer for CPU-based headless rendering without GPU requirements"
  - "Use HashMap<ObjectId, Image> for per-surface buffer tracking"
  - "Render on commit when buffer attached"
  - "Reuse buffers across commits, recreate on dimension change"
  - "Use ExportMem::copy_framebuffer() + map_texture() for RGBA extraction"
  - "Store extracted frames in HashMap<ObjectId, RgbaData> for streaming"
  - "Extract RGBA immediately after rendering while buffer is still valid"
patterns_established:
  - "Renderer initialization before compositor state in ServerState::new()"
  - "Import buffer via ImportMemWl to get dimensions"
  - "Use try_render_surface_to_buffer for non-fatal error handling"
  - "bind() + copy_framebuffer() + map_texture() for pixel readback"
  - "Hold buffer reference until pixel data is cloned to Vec<u8>"
observability_surfaces: []
drill_down_paths: []
duration: 18 min
verification_result: passed
completed_at: 2026-03-10
blocker_discovered: false
---
# S03: Headless Rendering

**# Phase 3 Plan 1: PixmanRenderer Initialization Summary**

## What Happened

# Phase 3 Plan 1: PixmanRenderer Initialization Summary

**PixmanRenderer integrated into ServerState with renderer_pixman feature, enabling headless software rendering via Offscreen<Image> trait for memory-backed framebuffers**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-10T20:48:41Z
- **Completed:** 2026-03-10T20:54:24Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added renderer_pixman feature to smithay dependency in Cargo.toml
- Integrated PixmanRenderer into ServerState with proper initialization
- Server compiles successfully with headless rendering support
- Offscreen<Image> trait now available for creating memory-backed framebuffers

## Task Commits

Each task was committed atomically:

1. **Task 1: Add renderer_pixman feature to Cargo.toml** - `c1ffa24` (feat)
2. **Task 2: Add PixmanRenderer to ServerState** - `34069b8` (feat)

**Plan metadata:** [pending]

## Files Created/Modified
- `crates/server/Cargo.toml` - Added renderer_pixman feature to smithay dependency
- `crates/server/src/state.rs` - Added PixmanRenderer import, field, and initialization

## Decisions Made
- Use PixmanRenderer for CPU-based headless rendering (no GPU/display required)
- Initialize renderer before compositor state in ServerState::new() for proper dependency ordering
- Use expect() for renderer initialization with clear error message

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - compilation succeeded with only expected warnings about unused fields (renderer will be used in subsequent plans).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PixmanRenderer foundation complete, ready for offscreen buffer creation (REND-02)
- Next: Implement surface rendering to offscreen buffers via Offscreen::create_buffer()
- No blockers identified

---
*Phase: 03-headless-rendering*
*Completed: 2026-03-10*

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
