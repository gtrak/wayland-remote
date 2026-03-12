# T01: 02-wayland-core-protocol 01

**Slice:** S02 — **Milestone:** M001

## Description

Initialize the core Wayland compositor following the Smallvil pattern: set up CompositorState with wayland_frontend feature, create Display integrated with calloop event loop, and accept client connections via ListeningSocketSource. This establishes the foundation for all Wayland protocol handling.

Purpose: Without the core compositor infrastructure, no Wayland protocol can be handled and no clients can connect. This is the absolute minimum viable compositor.
Output: Working server binary that advertises wl_compositor global and accepts client connections.

## Must-Haves

- [ ] "Wayland socket is created and accepts client connections"
- [ ] "CompositorState is initialized and wl_compositor global is advertised"
- [ ] "calloop event loop integrates Display and dispatches client requests"
