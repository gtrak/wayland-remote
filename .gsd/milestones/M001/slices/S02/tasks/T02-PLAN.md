# T02: 02-wayland-core-protocol 02

**Slice:** S02 — **Milestone:** M001

## Description

Add wl_seat and wl_output globals to enable complete Wayland client support. The seat provides input device advertisement (keyboard, pointer) that clients require to create windows. The virtual output provides display information so clients know where to render. Both are required for most Wayland applications to initialize successfully.

Purpose: Without wl_seat, clients cannot receive input focus. Without wl_output, clients don't know display parameters. These globals complete the core protocol foundation.
Output: Server with full WAYL-01 compliance: wl_compositor, wl_seat, wl_output, and wl_surface all functional.

## Must-Haves

- [ ] "wl_seat global advertised with keyboard and pointer capabilities"
- [ ] "wl_output global advertised with virtual display mode"
- [ ] "Per-client state tracks compositor and seat resources"
