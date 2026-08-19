# Plan 003 — M3: xdg-shell Window Mapping

## Why

M1/M2 stream the full composited desktop into one Windows window. M3 implements PRD Step 6: real applications use xdg-shell, and each Wayland toplevel becomes its own native Windows window (surface ↔ HWND), with focus, close, and resize flowing both directions.

## What

- **Server**: `delegate_xdg_shell!` with proper toplevel lifecycle (map/unmap, configure/ack, state negotiation: activated/maximized/fullscreen vs window events), per-toplevel rendering (each mapped toplevel gets its own pixman target keyed by `window_id`), a simple stacking/focus order, and `WindowEvent` emission on the control stream.
- **Viewer**: one HWND per `window_id`; window creation/teardown on `Created`/`Destroyed`, client-side resize honoring server `configure` negotiation, focus forwarding on Windows `WM_ACTIVATE` → server-side focus change → xdg `activated` state, close (X button) → graceful toplevel close.

## Success criteria

- Two Wayland applications (e.g. two terminals) appear as two independent Windows windows.
- Typing goes to the focused Windows window's remote surface (focus round-trip works).
- Closing the Windows window closes the remote app's toplevel (app exits cleanly); resizing the Windows window resizes the remote surface after one configure/ack round-trip.
- Frame streams remain skip-stale per-window without head-of-line blocking between windows (each frame stream carries `window_id`; viewer demultiplexes).

## Task order

```
01-xdg-window-mapping   (single large issue: server xdg-shell + per-window render/events + viewer HWND lifecycle; each layer individually testable, executed in the step order below)
```

One issue because the server/viewer halves of the mapping contract can't be verified independently end-to-end — but its internal step order enforces server-first, test-client-second, viewer-third.

## Scope

In: xdg-shell toplevels (no popups beyond what menus need — `xdg_popup` minimum: positioner + unmap, no grabs), per-window pixman targets, window events on the wire, viewer HWND lifecycle, focus/close/resize round-trips, server-side cursor sprite (default arrow only).

Out: decorations (server-side title bars are Windows' own; xdg-decoration protocol is M3-optional if time permits — else apps draw client-side and we note it), fullscreen/multi-monitor semantics beyond simple maximize, `xdg_popup` with keyboard grabs, clipboard/DnD, move/resize via server-initiated operations.
