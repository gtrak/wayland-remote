# Code Review: Phase 04 - Plan 02 (TCP Client Handler)

**Review Cycle:** 1/5
**Date:** 2026-03-11

## Issues Status

### Critical

- [c-1] Frame streaming loop never called → **FIXED**
  - Location: crates/server/src/streaming/client.rs:78-85
  - Issue: The `stream_frames` function (client.rs:119-153) was defined but never invoked
  - When a client connected, they were registered and socket was set up, but no frames were sent
  - Fix: Spawn a task that calls `stream_frames(tx_clone, state_clone, addr)` after setting up the mpsc channel
  - Added: `stream_handle = tokio::spawn(async move { stream_frames(...) })` 
  - Also added: `let _ = stream_handle.await;` to wait for the task on disconnect

## Summary

| Status | Critical | Major | Minor |
|--------|----------|-------|-------|
| Fixed | 1 | 0 | 0 |
| Still Open | 0 | 0 | 0 |
| **Total** | **1 FIXED** | **0** | **0** |

## Verification

### Compilation Check

```
cargo check -p wayland-remote-server
```

Result: **PASSED** (4 warnings, all pre-existing)

### Changes Made

1. Added frame streaming task spawn in `handle_client()` (client.rs:78-85)
2. Added awaiting `stream_handle` on client disconnect (client.rs:105)

### Code Flow After Fix

1. `handle_client` called when viewer connects
2. Client registered in StreamingState
3. Bounded mpsc channel created (32 frame buffer)
4. **NEW:** `stream_frames` task spawned - reads from `StreamingState.captured_frames`, encodes with `encode_frame`, sends to mpsc channel
5. Socket write task spawns - receives from channel, writes to TCP socket
6. On disconnect: drop tx, wait for both tasks, unregister client

---

*Reviewed by: gsd-code-reviewer | Cycle: 1/5*
