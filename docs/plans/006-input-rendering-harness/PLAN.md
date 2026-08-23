# Plan 006 — Input Fix, Rendering Gaps, and Self-Driving Harness

## Why

Plan 005 delivered per-window streaming and the Win32 viewer end-to-end, and the
first live run exposed two real gaps left open by the M2/M3 input-injection work:

- **Pointer/keyboard input does not reach clients.** `weston-clickdot` renders
  its window but clicks draw no dots. Server logs show 959 input events arriving
  yet zero `surface commit`s after any click — the click never triggers the
  client's redraw. Root cause is three-layered: `inject()` passes `None` to
  `ptr.motion` so the pointer never enters a surface; `window_id` is discarded
  in the network→compositor bridge; and `SurfaceFocus` (the `SeatHandler` focus
  type) is a hand-written stub whose `wl_surface()` returns `None` and whose
  `PointerTarget`/`KeyboardTarget` impls are empty no-ops. smithay *provides*
  real `PointerTarget<D> for WlSurface` / `KeyboardTarget<D> for WlSurface`
  impls that forward protocol events — they were never wired in.
- **Many programs don't render.** The server advertises only
  compositor/shm/seat/output/xdg_shell, and the renderer blits a single
  `wl_shm` buffer per window. Programs that compose via subsurfaces/popups
  (GTK/Qt menus), use `wp_viewporter`, speak legacy `wl_shell`, or render with
  EGL/dmabuf show nothing or fail to connect.

There is also no way for an agent to verify input/rendering without the Win32
GUI and a human eyeballing it. This plan closes that with a no-GUI scripted
client, a red→green integration test, and a cross-machine driver so the work is
self-verifying on `gary-agents`.

## What

Three phases. Phase 1 builds the harness and telemetry so Phase 2's fix is
verifiable by the agent (not by eye); Phase 3 broadens app compatibility.

- **Phase 1 — Harness + telemetry.** Server telemetry snapshots; a no-GUI
  `--drive` viewer mode that scripts input and captures frames/JSON; a Rust
  integration test that asserts an input round-trip changes pixels (written
  red first); and a Bun/TS cross-machine driver in `tools/drive/` that SSHes to
  build+launch the server and clients, runs the Rust pieces, and diffs frames.
- **Phase 2 — Input focus fix.** Replace the `SurfaceFocus` stub with `WlSurface`
  for the three seat focus types; thread `window_id` through the bridge; fix
  `inject()` to pass `Some((surface, (0,0)))` focus so the pointer enters the
  window's surface; set keyboard focus on `SetFocus`/map. Correct the
  smithay-07-api skill (pointer `motion` is 3-arg with a `focus` param, not the
  2-arg form the skill shows). Verified by the Phase-1 test (red→green) and a
  live `gary-agents` run (clickdot draws dots; weston-terminal types).
- **Phase 3 — Rendering gaps.** `wl_subcompositor` + `wp_viewporter` (renderer
  walks the subsurface tree, not one buffer); legacy `wl_shell` + `zwp_text_input_v3`
  globals; EGL/dmabuf feasibility investigation (likely deferred with a
  documented decision — headless software rendering can't easily do hw EGL).

## Success criteria

- `cargo test -p wayland-remote-server` (Linux) has a test that drives a real
  Wayland client, injects a pointer click over QUIC, and asserts the client
  committed a new buffer at the click location — red before Phase 2, green after.
- `wayland-remote-viewer --drive ...` runs on Windows and Linux with no GUI,
  scripts a click, captures frames to PNG, and prints JSON `{frames, fps, rtt,
  pixelsChangedAt}`.
- `tools/drive/` drives a full run against `gary-agents` over SSH and reports
  pass/fail with artifacts (PNGs + JSON).
- Live on `gary-agents`: `weston-clickdot` draws dots on click; `weston-terminal`
  accepts typed text; a subsurface/popup-using client renders its popup.
- Telemetry log line present per server tick with per-window fps, frame bytes,
  input→commit latency, and commit/error counters.
- `lat check` green; smithay-07-api skill corrected; `lat.md/` updated.

## Task order (dependency graph)

```
01-telemetry ──┐
02-drive ──────┤
03-integ-test ─┼──> 04-cross-machine-driver ──┐
               │                               ├──> 05-input-focus-fix ──┐
               └───────────────────────────────┘                          ├──> 06-subsurface-viewporter
                                                                       ├──> 07-additional-globals
                                                                       └──> 08-egl-dmabuf-research
```

- 01, 02, 03 are independent foundations and may run in parallel.
- 04 depends on 02 (drives the `--drive` binary) and 03 (may run `cargo test`).
- 05 depends on 03 (the red test it turns green) and benefits from 02/04 for
  live verification.
- 06, 07, 08 are independent of each other and may run after 05 (so Phase 2
  lands on a known-good baseline). 08 is research only and can start anytime.

## Scope

In: telemetry, `--drive` mode, input round-trip integration test, TS
cross-machine driver, seat-focus fix + window_id threading, subsurface/viewporter
rendering, wl_shell + text_input globals, EGL feasibility decision.

Out: reconnect-with-backoff + persistent TOFU store, cursor sprite, lossy/WAN
tuning, clipboard/DnD, video codec, actual EGL/GPU rendering (scoped to a
decision in 08), multi-viewer.

## Issues

- `01-server-telemetry.md`
- `02-viewer-drive-mode.md`
- `03-input-integration-test.md`
- `04-cross-machine-driver.md`
- `05-input-focus-fix.md`
- `06-subsurface-viewporter.md`
- `07-additional-globals.md`
- `08-egl-dmabuf-research.md`
- `09-egl-dmabuf-import.md`
