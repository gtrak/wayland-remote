# Code Review: Phase 04, Plan 03

## Status: ISSUES_FOUND

### Critical
None

### Major
1. **captured_frames not cleaned up on surface destruction** (`crates/server/src/state.rs:272-281`): The `remove_streaming_surface()` function removes the surface from `SurfaceTracker` and `streaming_state`, but does not remove it from `self.captured_frames`. This leaves stale frame data in memory even after a surface is destroyed. Consider adding `self.captured_frames.remove(&surface_id);` to the function.

### Minor
1. **Duplicate doc comment** (`crates/server/src/state.rs:270-271`): The comment "Called when a surface is destroyed." appears twice consecutively.
2. **Unused field warning**: `creation_time` in `SurfaceInfo` is never read (warning from cargo check).
3. **Unused function warning**: `SurfaceInfo::new()` is never called.
