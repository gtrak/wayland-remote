# T03: 02-wayland-core-protocol 03

**Slice:** S02 — **Milestone:** M001

## Description

Implement CompositorHandler to track surface lifecycle: create, buffer attach, commit, and destruction. This satisfies WAYL-02 (surface operations) and WAYL-03 (cleanup). Add integration tests using wayland-client to verify the full surface lifecycle works end-to-end. This is the final piece of Phase 2 core protocol support.

Purpose: Surface lifecycle is the heart of Wayland - clients create surfaces, attach buffers (pixel data), commit changes, and eventually destroy them. Without proper handling, clients will freeze, crash, or leak memory.
Output: Working surface lifecycle with test coverage, completing Phase 2 requirements.

## Must-Haves

- [ ] "Surface commits trigger CompositorHandler::commit callback"
- [ ] "Buffer attachments are detected and tracked"
- [ ] "Surface destruction releases resources without leaks"
- [ ] "Test client can create surface, attach buffer, commit, and destroy"
