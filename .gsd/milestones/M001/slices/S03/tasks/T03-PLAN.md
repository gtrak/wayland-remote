# T03: 03-headless-rendering 03

**Slice:** S03 — **Milestone:** M001

## Description

Implement RGBA pixel extraction from offscreen buffers and frame callback management.

Purpose: Extract raw RGBA pixel data from rendered framebuffers for TCP streaming, and respond to wl_surface.frame callbacks to drive client rendering loops without freezing.
Output: Pixel export module with RGBA extraction and buffer lifecycle management with frame callbacks.

## Must-Haves

- [ ] "RGBA pixel data can be extracted from rendered surfaces"
- [ ] "Buffer is held until RGBA extraction completes"
- [ ] "Frame callbacks are sent after rendering to drive client rendering"
