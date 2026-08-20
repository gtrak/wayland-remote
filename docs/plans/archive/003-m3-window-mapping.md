# Plan 003 — M3: xdg-shell Window Mapping (Archived)

Completed by Plan 005 (Phase A server, Phases B+C viewer). M1/M2 streamed the
full composited desktop into one Windows window; M3 implemented PRD Step 6 so
each Wayland xdg toplevel becomes its own native Windows HWND, with focus,
close, and resize flowing both directions.

The server delegates xdg-shell with real toplevel lifecycle (map/unmap,
configure/ack, state negotiation), renders one frame per mapped toplevel keyed
by `window_id`, and emits `WindowEvent`s on the control stream. The viewer
maps one HWND per `window_id`, forwarding focus/close/resize with an
echo-loop guard.

Scope:
- Server: xdg toplevels, per-window pixman targets, window events, Resized on re-commit.
- Viewer: per-window HWND lifecycle, focus/close/resize round-trips.
- `window_id` on the wire as the frame demux key.

Out of scope: decorations, multi-monitor/fullscreen beyond maximize, popup
grabs, clipboard/DnD, server-initiated move/resize.

## 01-xdg-window-mapping
