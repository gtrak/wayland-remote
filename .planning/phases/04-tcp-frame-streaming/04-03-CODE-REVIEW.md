# Code Review: Phase 04, Plan 03

## Status: FIXED

### Critical
None

### Major
1. ~~captured_frames not cleaned up on surface destruction~~ **FIXED**: Added `self.captured_frames.remove(&surface_id);` to `remove_streaming_surface()`

### Minor
1. ~~Duplicate doc comment~~ **FIXED**: Removed duplicate
2. ~~Unused field warning~~ **FIXED**: Added `#[allow(dead_code)]` to `creation_time` field
3. ~~Unused function warning~~ **FIXED**: Added `#[allow(dead_code)]` to `SurfaceInfo::new()`
