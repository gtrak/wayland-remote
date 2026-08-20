# 01 — Server Telemetry

## Objective

Add structured server-side telemetry so the harness (and an operator) can
assert frame/input/commit behavior without eyeballing a GUI: a `Telemetry`
snapshot with per-window fps, frame bytes, input→commit latency, and
commit/error counters, plus a periodic log line the driver greps.

## Files

| File | Change |
|------|--------|
| `crates/server/src/state.rs` | Add `Telemetry` struct + `Snapshot` accessor on `State`; counters incremented in `commit`, `render_window`, `inject_input`. |
| `crates/server/src/lib.rs` | Emit a periodic telemetry log line each loop tick; increment frame counter in `push_frame`. |
| `crates/server/src/window.rs` | Expose per-window commit count if needed by `Telemetry`. |
| `crates/server/src/net/session.rs` | Stamp last-input time for input→commit latency. |

## Steps

1. Define `pub struct Telemetry { frames_total, frames_per_sec, frame_bytes_total, commits_total, input_events_total, last_input_to_commit_ms: Option<u32>, errors_total }` and a `Snapshot` value type that is cheap to copy/log.
2. Own a `Telemetry` inside `State`. Increment `commits_total` in `CompositorHandler::commit` (state.rs:433). Increment `input_events_total` in `inject_input` (state.rs:362). Increment `frames_total`/`frame_bytes_total` in `push_frame` (lib.rs:261) — pass a `&mut Telemetry` or return counts via the bridge; keep the send-side unchanged.
3. Track input→commit latency: record `last_input_at: Option<Instant>` on each `PointerButton`/`KeyDown` inject; in `commit`, if `last_input_at` is recent, compute elapsed ms into `last_input_to_commit_ms` and clear it.
4. Compute `frames_per_sec` over a 1s sliding window (store `frames_this_second` + `second_start: Instant`).
5. Each main-loop tick (lib.rs:177 area), if ≥1s since last emit, log a single line:
   `telemetry: fps=N frames=N bytes=N commits=N inputs=N in2commit=Nms errors=N` at INFO.
6. Reset per-second counters after emit.

## Verification

- `cargo test -p wayland-remote-server` green on gary-agents.
- Start the server, connect a client, click around: `grep "telemetry:" /tmp/wr-server.log` shows lines with non-zero `commits`/`inputs`/`fps`.
- Unit test: drive a commit + a render and assert the snapshot counters moved (extend `tests/render.rs` pattern).
- `lat check` green; add a `// @lat:` ref if a telemetry test spec is added to `lat.md/`.
