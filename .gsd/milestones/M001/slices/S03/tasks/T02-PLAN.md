# T02: 03-headless-rendering 02

**Slice:** S03 — **Milestone:** M001

## Description

Implement surface rendering to offscreen buffers using PixmanRenderer.

Purpose: Render Wayland surface content to memory-backed framebuffers that can be read back as RGBA data for streaming.
Output: Rendering module with offscreen buffer creation and surface rendering functions.

## Must-Haves

- [ ] "Surface content is rendered to offscreen buffer"
- [ ] "Per-surface offscreen buffer tracked in state"
- [ ] "Render target is created and bound before rendering"
