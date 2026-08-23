# Plan 007 — Performance: Damage Tracking, Local Cursor, Late-Viewer State Sync

## Why

The end-to-end path works (render, stream, input, animate), but watching the animated
EGL demo (`weston-simple-egl`) exposed two real problems, both rooted in the same
design: the compositor **re-renders and re-streams the full frame for every mapped
window on every event-loop tick** (the per-window stream loop in
`crates/server/src/lib.rs`, `run`), and the **pointer cursor is composited into that
frame** (`State.cursor_surface`, drawn last by `render_window`).

- **Perf drops in the EGL demo.** Every animation commit (and every tick) triggers a
  full GL import → PBO readback of the whole window → compress → QUIC send. For a
  1080p window at ~20 fps that is ~160 MB/s of GPU→CPU readback plus CPU compression,
  contending with GPUs already at ~90% (inference). When a frame's readback+compress
  exceeds the ~50 ms frame budget, frames drop and the motion stutters.
- **Late cursor tracking.** Because the cursor is drawn into the frame, a cursor move
  is only as fresh as the next full-frame round-trip (render → readback → compress →
  stream → blit). The pointer visibly lags the real mouse. This is the classic reason
  RDP/VNC render the pointer *locally* instead of inside the picture.

There is also a separate usability gap: a viewer that (re)connects **after** windows
already exist sees nothing. The server fans out only *new* window events and frames
(`SessionRegistry` fans `WindowEvents` + `Frame` to every session), so a late session
misses the pre-existing window tree (the `Created` events were already delivered to
earlier sessions).

This plan makes streaming **incremental** (only what changed), moves the **cursor to
the viewer** (zero-latency), and makes a **late viewer self-synchronise**. It opens
with measurement so the remaining cost is known with data — and so the follow-up
(video encoding / dmabuf zero-copy, PRD §7) is a decision, not a guess.

## What

Four capabilities, in dependency order:

1. **Measurement** — per-stage timing (readback / compress / send) and per-window byte
   counts, so every other issue's win is verifiable and the video-encoder follow-up is
   data-driven.
2. **Change gating** — a per-window dirty flag; the stream loop renders + sends only
   windows with new content. Kills the readback+compress cost for static windows
   (the multi-window case) and stops re-streaming identical frames.
3. **Partial-frame damage** — carry a damage rectangle in the frame header; send only
   the changed region (bounding box, or full when the region is large); the viewer
   applies only that region to its per-window frame store and blits only that rect.
   Includes a re-sync safety net for the lossy transport (a missed delta must not
   corrupt the viewer's baseline).
4. **Local viewer-side cursor** — the server stops compositing the cursor into frames
   and sends the cursor sprite (on change) + position (on move) over the control
   channel; the viewer draws a native Win32 cursor in the child window. This removes
   the cursor from the frame path entirely, so cursor motion no longer pays a
   full-frame round-trip.
5. **Late-viewer state sync** — on a new session's handshake, replay the current
   window tree (a `Created` per mapped window) plus one full baseline frame per window
   to *that* session before resuming deltas.

## Success criteria

- **Baseline captured** (issue 01): per-stage ms + bytes/frame for (a) the animated
  `weston-simple-egl` and (b) a static window, before and after the rest.
- **Static window streams ~0 bytes when idle** (change gating): with a mapped
  static client and no input, the stream loop performs no readback and sends no
  frames (telemetry `frames` is flat while the window is unmoved).
- **Partial frames**: a small region of change (e.g. moving a text cursor in
  `weston-terminal`) streams only that region — the on-wire byte count for that frame
  is a small fraction of the full frame; the viewer composites it correctly.
- **Cursor is frame-local, not frame-locked**: with the server compositing the cursor
  *off*, moving the mouse moves the viewer's native cursor immediately (measured as:
  no full frame is emitted for a pure cursor move; the cursor position arrives via the
  control channel). No visible lag relative to the local Windows mouse.
- **Late viewer self-syncs**: start the server + client, let a window map, *then*
  connect a second viewer; it renders the existing window within ~1 s without the
  client being restarted.
- **No regressions**: `weston-flower` still animates, `weston-simple-egl` still
  animates (issue 006/09 frame-callback fix intact), input round-trips
  (`cargo test -p wayland-remote-server` green, including the input round-trip test),
  and the cross-machine drive harness still PASSes for both shm and EGL clients.
- `lat check` green; `lat.md/` updated (Rendering Pipeline, QUIC Session Model,
  Viewer) and the protocol contract documented.

## Task order (dependency graph)

```
01-measurement ──┐
                 ├──> 02-change-gating ──┬──> 03-partial-frame-damage ──> 05-state-sync
                 │                        └──> 04-local-cursor
                 └── (01's baseline is the "before" for 02–04's "after")
```

- **01** first: it defines the "before" numbers and confirms where the drops come
  from (readback vs compress vs send). It does not block anything else, but land it
  first so the wins in 02–04 are measured, not assumed.
- **02** is independent of the wire format (server-side gating only) and is the single
  highest-value CPU/GPU saver. Land it early.
- **03** depends on 02 (it needs the damage/region to gate on and to send) and changes
  the wire format (a protocol version bump).
- **04** depends on 02 (so a pure cursor move does NOT mark a window dirty) and is
  otherwise independent of 03; it can land in parallel with 03 once 02 is in.
- **05** depends on 03 (the baseline it sends is a "full" frame in the new
  full-vs-region format; a late session then applies subsequent damage deltas).

## Scope

In: per-stage instrumentation, per-window change gating, partial-frame damage +
re-sync, local (viewer-side) cursor, late-viewer window-tree + baseline-frame replay.

Out (deferred / separate plans): video encoding (NVENC/H.264) and dmabuf zero-copy
(PRD §7 — this plan's measurement decides whether they're the next step); real
window management / tiling; audio; clipboard/DnD; TOFU cert store + auto-reconnect;
multi-head.

## Issues

- `01-baseline-measurement.md`
- `02-change-gating.md`
- `03-partial-frame-damage.md`
- `04-local-cursor.md`
- `05-state-sync-late-viewers.md`
