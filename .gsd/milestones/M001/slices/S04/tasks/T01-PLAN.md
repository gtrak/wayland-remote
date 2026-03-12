# T01: 04-tcp-frame-streaming 01

**Slice:** S04 — **Milestone:** M001

## Description

Create streaming module foundation with binary protocol definition and TCP server skeleton.

Purpose: Establish the networking layer that will stream captured frames to Windows viewers
Output: streaming/mod.rs, streaming/protocol.rs, updated state.rs

## Must-Haves

- [ ] "TCP server can bind to configurable port"
- [ ] "Frame protocol defines 20-byte header with window_id, width, height, timestamp"
- [ ] "RGBA payload follows header with correct byte order (big-endian)"

## Files

- `crates/server/src/streaming/mod.rs`
- `crates/server/src/streaming/protocol.rs`
- `crates/server/src/state.rs`
