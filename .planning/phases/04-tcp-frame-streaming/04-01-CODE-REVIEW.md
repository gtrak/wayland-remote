# Code Review: Phase 04 - Plan 01 (TCP Frame Streaming)

**Review Cycle:** 1/5
**Date:** 2026-03-11

## Issues Status

### Critical

- [c-1] Unstable window_id mapping → **FIXED**
  - Location: crates/server/src/state.rs:234-249
  - Fix verified: Now uses HashMap-based ID assignment that persists until surface removal
  - Implementation: Added `window_id_map: HashMap<ObjectId, u32>` and `next_window_id: u32` fields
  - Code: `self.window_id_map.entry(surface_id.clone()).or_insert_with(|| { ... })`
  - Also added cleanup in `remove_streaming_surface()`: `self.window_id_map.retain(|_, &mut v| v != window_id);`

### Major

- [M-1] calloop::futures integration → **OK_SKELETON**
  - Location: crates/server/src/streaming/mod.rs:190
  - Status: Plain `tokio::spawn` used as per skeleton phase scope
  - Note: RESEARCH.md shows specific pattern for full implementation; acceptable for Plan 01 skeleton

### Minor

- [m-1] Unnecessary `mut` in state.rs → **FIXED**
  - Location: crates/server/src/state.rs:264 (was line 256)
  - Fix verified: Removed unnecessary `mut` from `let state = self.streaming_state.write().await;`
  - Note: The `mut` in `remove_streaming_surface` at line 277 is necessary (calls `&mut self` method)

## Summary

| Status | Critical | Major | Minor |
|--------|----------|-------|-------|
| Fixed | 1 | 0 | 1 |
| Still Open | 0 | 0 | 0 |
| **Total** | **1 FIXED** | **0** | **1 FIXED** |

## Verification

### Compilation Check

```
cargo check -p wayland-remote-server
```

Result: **PASSED** (4 warnings, all pre-existing from previous phases)

### Changes Made

1. Added `window_id_map: HashMap<ObjectId, u32>` to ServerState for stable ID mapping
2. Added `next_window_id: u32` counter for assigning new IDs
3. Updated `get_frames_for_streaming()` to use stable mapping via `HashMap::entry().or_insert_with()`
4. Updated `update_streaming_state()` to take `&mut self`
5. Updated `remove_streaming_surface()` to clean up window_id_map and take `&mut self`
6. Removed unnecessary `mut` in `update_streaming_state()`

## Recommendations

None - all code review issues resolved.

---

*Reviewed by: gsd-code-reviewer | Cycle: 1/5*
