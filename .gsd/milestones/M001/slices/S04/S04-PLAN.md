# S04: Tcp Frame Streaming

**Goal:** Create streaming module foundation with binary protocol definition and TCP server skeleton.
**Demo:** Create streaming module foundation with binary protocol definition and TCP server skeleton.

## Must-Haves


## Tasks

- [x] **T01: 04-tcp-frame-streaming 01** `est:17 min`
  - Create streaming module foundation with binary protocol definition and TCP server skeleton.

Purpose: Establish the networking layer that will stream captured frames to Windows viewers
Output: streaming/mod.rs, streaming/protocol.rs, updated state.rs
- [x] **T02: 04-tcp-frame-streaming 02** `est:14 min`
  - Implement TCP server that accepts viewer connections and streams frames.

Purpose: Enable network transmission of captured RGBA frames to Windows viewers
Output: client.rs with connection handler, updated mod.rs with accept loop
- [x] **T03: 04-tcp-frame-streaming 03** `est:9 min`
  - Implement multi-surface tracking with unique window IDs for streaming.

Purpose: Enable streaming of multiple Wayland surfaces simultaneously with distinct identifiers
Output: surface.rs with SurfaceTracker, updated state.rs

## Files Likely Touched

- `crates/server/src/streaming/mod.rs`
- `crates/server/src/streaming/protocol.rs`
- `crates/server/src/state.rs`
- `crates/server/src/streaming/client.rs`
- `crates/server/src/streaming/mod.rs`
- `crates/server/src/streaming/surface.rs`
- `crates/server/src/state.rs`
