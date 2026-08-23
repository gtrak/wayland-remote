# 01 — Baseline measurement (per-stage timing + bandwidth)

## Objective

Make the render → readback → compress → send path observable so the "perf drops in
the EGL demo" are attributable (readback vs compress vs send) and so every later
issue's win is measured, not assumed. This is the "before" for issues 02–05.

## Files

| File | Change |
|------|--------|
| `crates/server/src/rendering/mod.rs` | Time the readback stage (`copy_framebuffer` + `map_texture`, i.e. the GL PBO / pixman readback) inside `render`/`render_window_surface`. Return the stage time out of the render path (e.g. attach it to `FrameBuffer` or a small `RenderTimings` struct). |
| `crates/server/src/state.rs` | In `Telemetry`, add `readback_ns_total` and `compress_ns_total` (accumulators) + surface them in `TelemetrySnapshot`. `record_frame` already tracks bytes; extend it (or add `record_timings`) to also add the readback + compress ns. |
| `crates/server/src/net/*` (or wherever frames are compressed before `push_frame`) | Time the compress stage (Lz4/None) per frame and pass it to the telemetry sink. |
| `crates/server/src/lib.rs` | Emit `readback_ms` and `compress_ms` (and per-window bytes) in the per-second `tracing::info!` telemetry line; add a `debug!` per-frame line (window_id, w, h, readback_ms, compress_ms, full_bytes, sent_bytes). |

## Steps

1. Add a `RenderTimings { readback_ns: u64, ... }` (or similar) and thread it from the
   renderer through `render_window` back to the stream loop.
2. Time the readback in the renderer; time the compress where the frame is
   compressed for the wire.
3. Add the two accumulators to `Telemetry` + `TelemetrySnapshot`; emit them in the
   telemetry line as `readback_ms` / `compress_ms` (averaged or total per second —
   pick one and document it).
4. Add a per-frame `debug!` log gated behind `RUST_LOG=wayland_remote_server=debug`.
5. Capture the baseline: run (a) `weston-simple-egl` (animated) and (b) a static
   client (e.g. `weston-image` or `weston-simple-shm`) against the server, and record
   the per-second `fps / frames / bytes / readback_ms / compress_ms` and the per-frame
   debug numbers. Paste the "before" numbers into this issue's Verification.

## Verification

- The telemetry line now includes `readback_ms` and `compress_ms`.
- `RUST_LOG=...=debug` shows a per-frame line with window_id, dimensions, readback_ms,
  compress_ms, full_bytes, sent_bytes.
- Baseline recorded for the animated (`weston-simple-egl`) and a static client, clearly
  labeling which stage dominates (readback vs compress vs send).
- `cargo test --workspace` + `lat check` still green (this issue adds no behavior,
  only measurement).
