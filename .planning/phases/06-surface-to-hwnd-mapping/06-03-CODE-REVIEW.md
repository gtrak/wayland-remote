---
phase: 06-surface-to-hwnd-mapping
plan: '03'
subsystem: viewer
tags: [resize, aspect-ratio, code-review]
review_cycle: 2
---

# Code Review: Phase 06 - Plan 03: Window Resize Handling

**Review Cycle:** 2/5  
**Date:** 2026-03-11  
**Reviewer:** gsd-code-reviewer

## Previous Issues Status

### From Cycle 1

- [m-1] Tracing field name inconsistency in resize handler → **FIXED**
  - Location: `crates/viewer/src/app.rs:162`
  - Changed from `compositor_window_id` to `window_id` for consistency

- [m-2] Unused parameters in handle_resize() → **FIXED**
  - Location: `crates/viewer/src/display/window.rs:166-167`
  - Added explanatory comment: "Parameters accepted for API consistency but current window size is obtained directly from winit when rendering"

- [m-3] Inconsistent indentation in match arm → **FIXED**
  - Location: `crates/viewer/src/app.rs:160-171`
  - Indentation now consistent with other match arms

## Current Issues

### Critical

*No critical issues found.*

### Major

*No major issues found.*

### Minor

*No new minor issues found.*

## Verification Against Plan

### ✅ Task 1: Route resize events to correct window
**Status:** COMPLETE
- Resize event routing implemented in `ViewerApp::window_event()` at lines 160-171
- Uses `get_window_id()` for reverse lookup (line 134)
- Uses `get_window_mut()` to route to correct DisplayWindow (line 168)
- Verifies window exists before routing (line 168)
- Tracing log present with window_id and dimensions (lines 161-166)

### ✅ Task 2: Verify per-window resize threshold implementation
**Status:** COMPLETE
- `DisplayWindow` has per-window resize threshold fields: `last_resized_width` and `last_resized_height` (lines 30-31)
- 10% threshold logic implemented in `submit_frame()` at lines 93-111
- Threshold compares against last resized dimensions (correctly prevents feedback loops)
- Each window independently tracks its own threshold via instance fields

### ✅ Task 3: Verify aspect ratio preservation in GDI renderer
**Status:** COMPLETE
- `GdiRenderer::render()` uses `StretchDIBits()` with proper destination rectangle calculation
- Aspect ratio preservation implemented with letterboxing/pillarboxing (lines 238-248)
- Uses `SRCCOPY` raster operation (line 268)
- Uses `DIB_RGB_COLORS` color table (line 266)

## Plan Compliance Check

| Criterion | Status | Notes |
|-----------|--------|-------|
| Resizing window routes to correct DisplayWindow | ✅ PASS | Via `get_window_id()` → `get_window_mut()` |
| Content scales with aspect ratio preserved | ✅ PASS | StretchDIBits with calculated dest rect |
| 10% threshold prevents resize feedback loop | ✅ PASS | Threshold against last_resized dimensions |
| Each window independently tracks threshold | ✅ PASS | Per-DisplayWindow fields |
| `cargo build` succeeds | ✅ PASS | Clean build with only unrelated warnings |

## Test Coverage

- [x] All 15 unit tests pass
- [x] No test failures
- [x] Tests cover RGBA→BGRA conversion
- [x] Tests cover frame dimension updates
- [x] Tests cover window manager basic operations

## Security Review

- No input validation issues (resize dimensions come from trusted winit)
- No unsafe code blocks in modified files (beyond existing GDI calls)
- No secrets or credentials

## Performance Review

- Resize threshold prevents excessive window resizing (good)
- Aspect ratio calculations use f32 (efficient)
- No unnecessary allocations in hot paths

## Summary

| Status | Critical | Major | Minor |
|--------|----------|-------|-------|
| Previous Fixed | 0 | 0 | 3 |
| Previous Remaining | 0 | 0 | 0 |
| New | 0 | 0 | 0 |
| **Total Open** | **0** | **0** | **0** |

**Previous Issues:** 3 fixed, 0 remaining  
**New Issues:** 0 critical, 0 major, 0 minor  
**Status:** **ISSUES_RESOLVED**

## Reviewer Notes

All 3 minor issues from Cycle 1 have been successfully fixed:

1. **m-1 Fixed:** The tracing field name was changed from `compositor_window_id` to `window_id` for consistency with the rest of the codebase. This makes the logging more uniform and easier to understand.

2. **m-2 Fixed:** A clarifying comment was added to the `handle_resize()` method documentation explaining why the width and height parameters are accepted even though they're not used directly. This improves API clarity for future maintainers.

3. **m-3 Fixed:** The indentation in the `WindowEvent::Resized` match arm is now consistent with other arms in the match block.

The implementation correctly fulfills all requirements from the plan:

1. **Resize Event Routing:** The flow `WindowEvent::Resized` → `get_window_id()` → `window.handle_resize()` is correctly implemented with proper window existence checks.

2. **10% Threshold:** The threshold implementation correctly prevents feedback loops by comparing new frame dimensions against `last_resized_width/height` rather than current window size. Each DisplayWindow independently tracks its threshold.

3. **Aspect Ratio Preservation:** The GDI renderer correctly implements aspect ratio preservation via destination rectangle calculations that add letterboxing or pillarboxing as needed.

All tests pass, the build succeeds, and no new issues were identified in this review cycle.

---
*Reviewed by: gsd-code-reviewer | Cycle: 2/5*
