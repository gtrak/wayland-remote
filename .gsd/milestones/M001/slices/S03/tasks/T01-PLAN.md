# T01: 03-headless-rendering 01

**Slice:** S03 — **Milestone:** M001

## Description

Initialize PixmanRenderer for headless software rendering.

Purpose: PixmanRenderer provides CPU-based rendering without GPU/display requirements, enabling true headless operation for offscreen framebuffers.
Output: ServerState with PixmanRenderer field, Cargo.toml with renderer_pixman feature.

## Must-Haves

- [ ] "PixmanRenderer instance exists in ServerState"
- [ ] "Server compiles with renderer_pixman feature"
- [ ] "Offscreen buffer creation API is available"
