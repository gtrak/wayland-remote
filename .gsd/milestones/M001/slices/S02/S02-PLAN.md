# S02: Wayland Core Protocol

**Goal:** Initialize the core Wayland compositor following the Smallvil pattern: set up CompositorState with wayland_frontend feature, create Display integrated with calloop event loop, and accept client connections via ListeningSocketSource.
**Demo:** Initialize the core Wayland compositor following the Smallvil pattern: set up CompositorState with wayland_frontend feature, create Display integrated with calloop event loop, and accept client connections via ListeningSocketSource.

## Must-Haves


## Tasks

- [x] **T01: 02-wayland-core-protocol 01** `est:11 min`
  - Initialize the core Wayland compositor following the Smallvil pattern: set up CompositorState with wayland_frontend feature, create Display integrated with calloop event loop, and accept client connections via ListeningSocketSource. This establishes the foundation for all Wayland protocol handling.

Purpose: Without the core compositor infrastructure, no Wayland protocol can be handled and no clients can connect. This is the absolute minimum viable compositor.
Output: Working server binary that advertises wl_compositor global and accepts client connections.
- [x] **T02: 02-wayland-core-protocol 02** `est:25 min`
  - Add wl_seat and wl_output globals to enable complete Wayland client support. The seat provides input device advertisement (keyboard, pointer) that clients require to create windows. The virtual output provides display information so clients know where to render. Both are required for most Wayland applications to initialize successfully.

Purpose: Without wl_seat, clients cannot receive input focus. Without wl_output, clients don't know display parameters. These globals complete the core protocol foundation.
Output: Server with full WAYL-01 compliance: wl_compositor, wl_seat, wl_output, and wl_surface all functional.
- [x] **T03: 02-wayland-core-protocol 03**
  - Implement CompositorHandler to track surface lifecycle: create, buffer attach, commit, and destruction. This satisfies WAYL-02 (surface operations) and WAYL-03 (cleanup). Add integration tests using wayland-client to verify the full surface lifecycle works end-to-end. This is the final piece of Phase 2 core protocol support.

Purpose: Surface lifecycle is the heart of Wayland - clients create surfaces, attach buffers (pixel data), commit changes, and eventually destroy them. Without proper handling, clients will freeze, crash, or leak memory.
Output: Working surface lifecycle with test coverage, completing Phase 2 requirements.

## Files Likely Touched

