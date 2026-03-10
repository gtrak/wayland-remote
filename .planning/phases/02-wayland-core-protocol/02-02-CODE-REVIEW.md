# Code Review: Phase 02 - Plan 02 (wl_seat and wl_output globals)

**Review Cycle:** 2/5
**Date:** 2026-03-10

## Previous Issues Status

### From Cycle 1

- **[M-1]** wl_output global NOT being advertised → **FIXED**
  - Fixed in commit c39eace: Added `output.create_global::<S>(dh);` in output.rs:41
  - Also added OutputManagerState and OutputHandler trait implementation

- **[M-2]** ClientState missing SeatClientState → **NON-ISSUE**
  - Smithay 0.7.0 does NOT provide a SeatClientState type
  - The PLAN.md requirement was incorrect
  - Research shows ClientState only needs CompositorClientState (see 02-RESEARCH.md:169-171)
  - Commit message confirms: "Fix ClientState to remove non-existent SeatClientState"

- **[m-1]** Unused DisplayHandle parameter → **FIXED**
  - dh is now used in `output.create_global::<S>(dh)`

- **[m-2]** Unused serial_counter field → **STILL_OPEN**
  - Field still present at state.rs:48, 139
  - Never read or written (only initialized to 0)
  - Ordering import also unused (compiler warning)

- **[m-3]** Unjustified .unwrap() on keyboard addition → **STILL_OPEN**
  - Still present in seat.rs:32: `seat.add_keyboard(...).unwrap()`
  - No error handling or justification comment

- **[m-4]** Extra blank line in ClientState struct → **FIXED**
  - Blank line was removed in fix commit

- **[m-5]** Missing tests for new functionality → **STILL_OPEN**
  - No tests were added for seat, output, or state

## Current Issues

### Critical

*None found*

### Major

*None found*

### Minor

- **[m-6]** Unused Ordering import (compiler warning)
  - **Location:** `crates/server/src/state.rs:23`
  - **Issue:** `use std::sync::atomic::{AtomicU32, Ordering};` - Ordering is never used
  - **Severity:** Minor
  - **Category:** Code Quality
  - **Fix:** Remove `Ordering` from import: `use std::sync::atomic::AtomicU32;`

- **[m-7]** Dead code warnings for ServerState fields
  - **Location:** `crates/server/src/state.rs:40, 42, 44, 48`
  - **Issue:** Fields `seat`, `output_manager_state`, `output`, and `serial_counter` are never read
  - **Severity:** Minor
  - **Category:** Code Quality
  - **Fix:** Either use these fields or remove them if not needed. The serial_counter should be used for input event serials.

- **[m-8]** Output configured twice unnecessarily
  - **Location:** `crates/server/src/handlers/output.rs:44-47` and `crates/server/src/state.rs:82-88`
  - **Issue:** Output mode is added in `create_virtual_output()` via `output.add_mode()`, then immediately overwritten in state.rs via `output.change_current_state()` with the same parameters
  - **Severity:** Minor
  - **Category:** Code Quality / Redundancy
  - **Fix:** Remove the `add_mode` call from output.rs since it's immediately replaced, or remove the redundant configuration from state.rs

- **[m-9]** Unjustified .unwrap() calls
  - **Location:** `crates/server/src/handlers/seat.rs:32`, `crates/server/src/state.rs:95, 110, 112, 124, 129, 196`
  - **Issue:** Multiple .unwrap() and .expect() calls without error handling
  - **Severity:** Minor
  - **Category:** Best Practices
  - **Fix:** Add error handling or descriptive error messages, especially for the keyboard initialization

- **[m-10]** Missing tests for new functionality
  - **Location:** `crates/server/tests/` (does not exist)
  - **Issue:** No unit or integration tests for seat handler, output handler, or state initialization
  - **Severity:** Minor
  - **Category:** Useless/Missing Tests
  - **Fix:** Add tests verifying seat creation with keyboard/pointer, output creation with correct mode, and proper global advertisement

## Summary

| Status | Critical | Major | Minor |
|--------|----------|-------|-------|
| Previous Fixed | 0 | 1 (M-1) | 2 (m-1, m-4) |
| Previous Remaining | 0 | 0 | 3 (m-2, m-3, m-5) |
| New | 0 | 0 | 5 (m-6, m-7, m-8, m-9, m-10) |
| **Total Open** | 0 | 0 | 8 |

**Previous Issues:** 3 fixed (M-1, m-1, m-4), 1 reclassified as non-issue (M-2), 3 remaining
**New Issues:** 0 critical, 0 major, 5 minor
**Status:** CYCLE_2 - ISSUES_REMAINING

### Priority Actions

1. **[m-2]** Either use serial_counter for input event serials or remove the field and Ordering import
2. **[m-3]** Add error handling or justification for keyboard initialization unwrap
3. **[m-5]** Add tests for seat and output handlers
4. **[m-7]** Address dead code warnings by using the fields or removing them
5. **[m-8]** Remove redundant output configuration

### Notes

- Build succeeds with 2 warnings (unused import, dead code)
- All major functionality is working: wl_seat and wl_output globals are properly advertised
- The PLAN.md had an incorrect requirement for SeatClientState - Smithay doesn't provide this type
- Previous review correctly identified that `create_global` was missing, now fixed

---
*Reviewed by: gsd-code-reviewer | Cycle: 2/5*
