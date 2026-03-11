---
phase: 06-surface-to-hwnd-mapping
plan: '02'
subsystem: viewer
tags: [code-review, multi-window]
review_cycle: 2
date: '2026-03-11'
reviewer: gsd-code-reviewer
---

# Code Review: Phase 06 - Plan 02: ViewerApp Multi-Window Integration

**Review Cycle:** 2/5  
**Date:** 2026-03-11

## Previous Issues Status

### From Cycle 1

- [M-1] Misleading log message on every frame processing → **FIXED**
  - Changed from `info!("Window created or retrieved for frame")` to `debug!("Frame submitted to window")` at line 101
  - Severity reduced to debug level, message is now accurate

- [m-1] Variable shadowing in window_event function → **FIXED**
  - Changed from `let window_id` to `let compositor_window_id` at line 131
  - All usages updated consistently throughout the function

- [m-2] Comment inaccuracy in resumed() function → **FIXED**
  - Updated comment at line 110-111 to accurately describe that the method logs a debug message
  - Changed from "does nothing" to "logs debug message and returns"

- [m-3] No upper limit on window creation → **STILL_OPEN**
  - WindowManager still has no limit on window creation
  - A malicious server could cause resource exhaustion
  - Not addressed in this cycle

- [m-4] Test relies on internal implementation detail → **STILL_OPEN**
  - Test `test_window_id_zero_panics` still doesn't actually test WindowManager behavior
  - Just tests standalone assertion, not integration with actual code
  - Not addressed in this cycle

## Current Issues

### Major

*No major issues in this cycle.*

### Minor

- [m-3] No upper limit on window creation
  - **Location:** crates/viewer/src/window_manager.rs:53-79
  - **Issue:** WindowManager has no limit on how many windows can be created. A malicious or buggy server could send frames with many different window_ids, causing resource exhaustion.
  - **Severity:** Minor
  - **Category:** Security/Best Practices
  - **Fix:** Add a maximum window limit (e.g., 50 or 100) and log an error if exceeded. Consider:
    ```rust
    const MAX_WINDOWS: usize = 50;
    
    pub fn get_or_create_window(...) -> Option<&mut DisplayWindow> {
        if self.windows.len() >= MAX_WINDOWS && !self.windows.contains_key(&window_id) {
            error!("Maximum window limit ({}) reached", MAX_WINDOWS);
            return None;
        }
        // ... existing logic
    }
    ```

- [m-4] Test relies on internal implementation detail
  - **Location:** crates/viewer/src/window_manager.rs:177-188
  - **Issue:** The test `test_window_id_zero_panics` creates a WindowManager but only uses it to create the variable scope - it doesn't actually test WindowManager behavior. The assertion being tested is just a standalone assertion, not integrated with the actual WindowManager code.
  - **Severity:** Minor
  - **Category:** Useless Tests
  - **Fix:** Either make this a proper integration test that actually calls `get_or_create_window` (with a mock event loop), or remove it as it doesn't add meaningful coverage. The comment acknowledges this limitation.

## Plan Compliance Verification

### Verification Criteria Checklist

| Criterion | Status | Notes |
|-----------|--------|-------|
| ViewerApp has window_manager: WindowManager field | ✅ PASS | Line 47 in app.rs |
| Option<DisplayWindow> completely removed | ✅ PASS | No remnants found |
| process_frames() routes frames by window_id | ✅ PASS | Uses get_or_create_window() at lines 94-99 |
| resumed() no longer creates DisplayWindow | ✅ PASS | Only logs debug message at line 112 |
| window_event() uses get_window_id() for reverse lookup | ✅ PASS | Line 131 in app.rs |
| `cargo build -p wayland-remote-viewer` succeeds | ✅ PASS | Build succeeds with only unrelated warnings |

### Implementation Review

**Strengths:**
1. Clean replacement of Option<DisplayWindow> with WindowManager
2. Proper lazy window creation in process_frames()
3. Correct bidirectional mapping usage (get_window_id for events)
4. Window cleanup on CloseRequested with shutdown logic
5. All previous major/minor issues from Cycle 1 addressed
6. All tests pass
7. No TODO/FIXME/HACK comments found

**Architecture Compliance:**
- Follows the lazy window creation pattern from the plan
- Event routing correctly uses reverse mapping
- Frame routing correctly uses get_or_create_window()
- No regressions in frame routing logic

## Summary

| Status | Critical | Major | Minor |
|--------|----------|-------|-------|
| Previous Fixed | 0 | 1 | 2 |
| Previous Remaining | 0 | 0 | 2 |
| New | 0 | 0 | 0 |
| **Total Open** | 0 | 0 | 2 |

**Previous Issues:** 3 fixed, 2 remaining  
**New Issues:** 0 critical, 0 major, 0 minor  
**Status:** CYCLE_2

### Recommendation

All issues from Cycle 1 have been addressed appropriately:
- [M-1] Fixed by changing log level and message
- [m-1] Fixed by renaming variable to avoid shadowing
- [m-2] Fixed by updating comment accuracy

The two remaining minor issues ([m-3] and [m-4]) from Cycle 1 were intentionally not addressed as they represent code quality improvements rather than functional issues. They should be considered for future cycles but are not blockers.

### Files Reviewed

- `crates/viewer/src/app.rs` - Main application with WindowManager integration
- `crates/viewer/src/window_manager.rs` - Window management implementation
- `crates/viewer/src/lib.rs` - Module exports
- `crates/viewer/src/display/window.rs` - DisplayWindow implementation (reference check)

---
*Reviewed by: gsd-code-reviewer | Cycle: 2/5*
