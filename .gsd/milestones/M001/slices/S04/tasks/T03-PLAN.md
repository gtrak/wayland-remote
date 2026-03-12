# T03: 04-tcp-frame-streaming 03

**Slice:** S04 — **Milestone:** M001

## Description

Implement multi-surface tracking with unique window IDs for streaming.

Purpose: Enable streaming of multiple Wayland surfaces simultaneously with distinct identifiers
Output: surface.rs with SurfaceTracker, updated state.rs

## Must-Haves

- [ ] "Each Wayland surface maps to unique window ID"
- [ ] "Multiple surfaces stream independently with correct IDs"
- [ ] "Surface destruction removes from streaming state"

## Files

- `crates/server/src/streaming/surface.rs`
- `crates/server/src/state.rs`
