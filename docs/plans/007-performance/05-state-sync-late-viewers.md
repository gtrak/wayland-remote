# 05 — Late-viewer state sync

## Objective

A viewer that connects **after** windows already exist must render them without
restarting the client. Today the server fans out only *new* `WindowEvent`s and frames
to every session, so a late session misses the pre-existing window tree (the
`Created` events were already delivered to earlier sessions) and shows nothing.

Builds on issue 03: the baseline the server sends is a `kind=full` frame in the new
full-vs-region format; the session then applies subsequent damage deltas normally.

## Files

| File | Change |
|------|--------|
| `crates/server/src/net/mod.rs` | On a new session's `Hello`→`Welcome`, ask the compositor (over the command channel) for a snapshot: the list of mapped windows and a full baseline frame per window. |
| `crates/server/src/net/session.rs` | After `Welcome`, send to *this* session only: a `WindowEvent::Created { width, height, title }` per mapped window, then a `kind=full` frame per window. Then proceed with normal fan-out. |
| `crates/server/src/lib.rs` | Handle a "snapshot / render baseline for window N" command from the net side: for each mapped window, emit the `Created` event (reused from the window tree) and `render_window` a full frame for that session. |
| `crates/server/src/window.rs` | Expose the current window tree (window_id → width/height/title) for the snapshot. |

## Steps

1. Expose the current mapped-window tree from `WindowManager` (ids + geometry + title).
2. On a new session handshake, the net side requests a snapshot; the compositor replies
   with the window list + one full baseline frame per window, targeted at that session.
3. The session sends the `Created` events + baseline frames to itself before resuming
   normal operation. Existing sessions are unaffected (they keep receiving deltas).
4. (Bonus) This removes the drive harness's "start the viewer before the client"
   ordering requirement — verify the harness still works and simplify its timing if
   trivial.

## Verification

- Start the server + `weston-flower` (or `weston-simple-egl`), let the window map,
  **then** connect a second viewer. The late viewer renders the existing window within
  ~1 s, with no client restart.
- The pre-existing (first) viewer is unaffected (still animating, no flicker).
- A viewer that connects, disconnects, and reconnects re-renders the current state each
  time.
- The cross-machine drive harness still PASSes (its single viewer is now a "late
  viewer" served by state sync); confirm the old viewer-before-client timing is no
  longer required.
- `lat check` green.
