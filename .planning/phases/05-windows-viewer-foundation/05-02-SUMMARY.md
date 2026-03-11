---
phase: 05-windows-viewer-foundation
plan: 02
subsystem: ui
tags: [winit, winapi, gdi, windows]

# Dependency graph
requires:
  - phase: 05-01
    provides: TCP client with async frame reception via mpsc channel
provides:
  - winit 0.30 window management with ApplicationHandler
  - GDI renderer using StretchDIBits for RGBA frame display
  - DisplayWindow wrapper integrating winit and GDI
  - ViewerApp implementing main application loop
affects: [input handling, multi-window support, performance optimization]

# Tech tracking
tech-stack:
  added: [winapi 0.3]
  patterns: [double buffering, top-down DIB, RGBA→BGRA conversion]

key-files:
  created:
    - crates/viewer/src/display/mod.rs
    - crates/viewer/src/display/gdi.rs
    - crates/viewer/src/display/window.rs
    - crates/viewer/src/app.rs
  modified:
    - crates/viewer/src/lib.rs
    - crates/viewer/src/main.rs
    - crates/viewer/Cargo.toml

key-decisions:
  - "Use StretchDIBits for GDI rendering (simplest path for MVP)"
  - "Double buffering with front/back bitmap swap"
  - "Negative biHeight for top-down DIB format"
  - "Explicit RGBA→BGRA channel swap before GDI rendering"

patterns-established:
  - "GDI handle management: GetDC/ReleaseDC always paired"
  - "Bitmap cleanup in Drop impl to prevent GDI leaks"
  - "winit ApplicationHandler for event-driven architecture"

requirements-completed: ["VIEW-02"]

# Metrics
duration: 11 min
completed: 2026-03-11
---

# Phase 5 Plan 2: Window Display with GDI Rendering Summary

**winit 0.30 window with GDI StretchDIBits rendering, RGBA→BGRA conversion, and double-buffered frame display**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-11T11:22:31Z
- **Completed:** 2026-03-11T11:34:02Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- GdiRenderer using StretchDIBits with proper BITMAPINFO configuration
- RGBA to BGRA color channel conversion for Windows GDI compatibility
- Top-down DIB format using negative biHeight (prevents upside-down images)
- Double buffering implementation to prevent screen tearing
- DisplayWindow wrapper integrating winit 0.30 with GDI renderer
- ViewerApp implementing winit ApplicationHandler trait for event loop
- Proper GDI handle lifecycle management (GetDC/ReleaseDC pairing)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create display module with GDI renderer** - `68ff707` (feat)
   - GdiRenderer with StretchDIBits
   - RGBA→BGRA conversion
   - Double buffering
2. **Task 2: Implement window wrapper for winit integration** - `68ff707` (feat)
   - DisplayWindow struct
   - winit window management
3. **Task 3: Create ViewerApp with ApplicationHandler trait** - `68ff707` (feat)
   - ApplicationHandler implementation
   - Frame channel integration

**Plan metadata:** `68ff707` (docs: complete plan)

## Files Created/Modified

- `crates/viewer/src/display/mod.rs` - Display module exports
- `crates/viewer/src/display/gdi.rs` - GDI renderer with StretchDIBits
- `crates/viewer/src/display/window.rs` - DisplayWindow wrapper
- `crates/viewer/src/app.rs` - ViewerApp with ApplicationHandler
- `crates/viewer/src/lib.rs` - Added display and app module exports
- `crates/viewer/src/main.rs` - Updated to use app::run()
- `crates/viewer/Cargo.toml` - Added winapi dependency
- `Cargo.lock` - Updated with winapi package

## Decisions Made

- **StretchDIBits over SetDIBitsToDevice**: StretchDIBits allows proper scaling and aspect ratio maintenance
- **Negative biHeight**: Creates top-down DIB format, preventing upside-down images
- **Explicit RGBA→BGRA swap**: Windows GDI expects BGRA order; frames arrive as RGBA
- **Double buffering**: Front/back buffer swap prevents tearing during rapid frame updates
- **winit 0.30 ApplicationHandler**: Event-driven architecture, non-blocking event loop

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added cfg(windows) guards for cross-platform build compatibility**
- **Found during:** Task 1 (initial build attempt)
- **Issue:** winapi and winit are Windows-only crates; building on Linux failed
- **Fix:** Added `#[cfg(windows)]` attributes to display, window, and app modules
- **Files modified:** crates/viewer/src/display/mod.rs, crates/viewer/src/lib.rs, crates/viewer/src/app.rs
- **Verification:** `cargo build -p wayland-remote-viewer` succeeds on Linux
- **Committed in:** 68ff707 (Task 1 commit)

**2. [Rule 3 - Blocking] Updated main.rs to handle non-Windows platforms gracefully**
- **Found during:** Task 1 (build failure)
- **Issue:** main.rs had `#![cfg(windows)]` causing "main function not found" error on Linux
- **Fix:** Removed crate-level cfg, added conditional compilation for Windows-specific code
- **Files modified:** crates/viewer/src/main.rs
- **Verification:** Binary builds successfully on both Windows and Linux
- **Committed in:** 68ff707 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (both blocking issues)
**Impact on plan:** Both fixes essential for cross-platform development while maintaining Windows-only runtime. No scope creep.

## Issues Encountered

None - all issues resolved via deviation rules.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Window display foundation complete
- GDI rendering pipeline operational
- Ready for plan 05-03: wiring network frames to display
- No blockers for next phase

---

*Phase: 05-windows-viewer-foundation*
*Completed: 2026-03-11*
