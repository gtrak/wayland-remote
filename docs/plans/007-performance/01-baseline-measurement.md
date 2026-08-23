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

## Baseline (captured at `e36d708`, gary-agents)

Host: 3× RTX 5060 Ti at ~90% (inference load). Client: `weston-simple-egl`,
1280×720, viewer connected (QUIC session open so the net path runs).

**Per-frame (steady state, fps ≈ 19):**

| stage | cost/frame | range | notes |
|-------|-----------|-------|-------|
| render pass (GL) | **~0.7 ms** | 0.64–0.83 | `create_buffer` → `finish`/`wait` |
| readback (PBO) | **~0.56 ms** | 0.55–0.57 | `copy_framebuffer` + `map_texture`; variable under GPU contention |
| compress (Lz4) | **~1.3 ms** | 1.2–1.45 | 3.5 MB → ~290 KB |
| **total server** | **~2.6 ms** | — | render + readback + compress |

**Per-second (fps = 19):** `render_ms` 21–28, `readback_ms` 10–45 (spikes under GPU
load; 10–11 when idle / no viewer connected). Frame size 3,686,400 B (1280×720×4) →
~298,000 B compressed (Lz4 ~12.5:1). On-wire bandwidth **~44 Mbps** (290 KB × 19 fps).

**Findings**

- The **server render + readback + compress is only ~2.6 ms/frame — ~5% of the ~52 ms
  frame budget at 19 fps. The server is NOT the bottleneck.**
- Per-frame cost ranking: **compress (1.3 ms) > render (0.7 ms) ≈ readback (0.56 ms).**
- **`readback_ms` is variable (10→45 ms/s)** — spikes track the GPUs' inference load
  (PBO contention). This is the likely source of the observed "perf drops" (occasional
  readback stalls), not steady cost.
- The **19 fps is client pacing** (`weston-simple-egl` paces via frame callbacks), not a
  server limit.

**Implications for 02–04**

- **02 (change gating):** a static window currently pays the full ~2.6 ms/frame for no
  reason (the stream loop re-renders every mapped window every tick). 02 drives that to
  0. (Validate post-02: a static client's `render_ms`/`readback_ms` should fall to ~0.)
- **03 (partial damage):** for a *full-screen* redraw (weston-simple-egl rotates the
  whole scene) there is no region to exploit — readback + compress still run on the full
  frame. 03 helps *localized* changes (bandwidth + compress).
- The definitive fix for high-fps full-screen animation is **video encoding (NVENC)**
  (PRD §7) — a follow-up plan, now data-supported (raw path sends ~44 Mbps + 1.3 ms
  compress/frame).
