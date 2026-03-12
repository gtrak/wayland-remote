# S03: Headless Rendering

**Goal:** Initialize PixmanRenderer for headless software rendering.
**Demo:** Initialize PixmanRenderer for headless software rendering.

## Must-Haves


## Tasks

- [x] **T01: 03-headless-rendering 01** `est:5 min`
  - Initialize PixmanRenderer for headless software rendering.

Purpose: PixmanRenderer provides CPU-based rendering without GPU/display requirements, enabling true headless operation for offscreen framebuffers.
Output: ServerState with PixmanRenderer field, Cargo.toml with renderer_pixman feature.
- [x] **T02: 03-headless-rendering 02** `est:16 min`
  - Implement surface rendering to offscreen buffers using PixmanRenderer.

Purpose: Render Wayland surface content to memory-backed framebuffers that can be read back as RGBA data for streaming.
Output: Rendering module with offscreen buffer creation and surface rendering functions.
- [x] **T03: 03-headless-rendering 03** `est:18 min`
  - Implement RGBA pixel extraction from offscreen buffers and frame callback management.

Purpose: Extract raw RGBA pixel data from rendered framebuffers for TCP streaming, and respond to wl_surface.frame callbacks to drive client rendering loops without freezing.
Output: Pixel export module with RGBA extraction and buffer lifecycle management with frame callbacks.

## Files Likely Touched

