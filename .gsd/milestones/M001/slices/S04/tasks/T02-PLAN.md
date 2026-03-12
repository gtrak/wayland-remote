# T02: 04-tcp-frame-streaming 02

**Slice:** S04 — **Milestone:** M001

## Description

Implement TCP server that accepts viewer connections and streams frames.

Purpose: Enable network transmission of captured RGBA frames to Windows viewers
Output: client.rs with connection handler, updated mod.rs with accept loop

## Must-Haves

- [ ] "TCP listener accepts viewer connections on configured port"
- [ ] "Connected clients receive frame data via TCP"
- [ ] "Slow clients experience backpressure (bounded channel)"

## Files

- `crates/server/src/streaming/client.rs`
- `crates/server/src/streaming/mod.rs`
