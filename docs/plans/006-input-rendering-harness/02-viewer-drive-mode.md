# 02 — Viewer `--drive` Mode

## Objective

Add a no-GUI viewer subcommand that connects to `--addr`, runs a scripted input
sequence (clicks/keys at coordinates), captures N frames to PNG, and prints a
JSON summary `{frames, fps, rtt, pixelsChangedAt}`. Runs on Windows and Linux
with no display. This is the QUIC client the cross-machine driver (issue 04)
invokes.

## Files

| File | Change |
|------|--------|
| `crates/viewer/src/main.rs` | Add `--drive` subcommand parsing (`--addr`, `--insecure`/`--fingerprint`, `--click x,y[,button]`, `--type key`, `--frames N`, `--out dir`). |
| `crates/viewer/src/display/headless.rs` | Factor the connect/frame-loop so a new `drive` module reuses it, or add `display/drive.rs`. |
| `crates/viewer/src/display/drive.rs` (new) | The `run_drive` async fn: connect via `ViewerSession`, send scripted input, read frames, detect pixel change, write PNGs, return JSON. |
| `crates/viewer/src/session.rs` | No new API needed — reuse `connect`, `send_input`, `next_frame`, `ping`, `try_read_control`. |

## Steps

1. Refactor `headless.rs` so the connect + frame-read + control-drain loop is reusable; extract a helper that yields `(frame, control_msgs)` per tick.
2. Add `display/drive.rs::run_drive(addr, fingerprint, insecure, script, frames, out_dir) -> Result<DriveReport>` where `script` is a `Vec<DriveAction>` (`Click{x,y,button}`, `Key{scancode}`, `Wait{ms}`).
3. Implement: connect; for each action, `session.send_input(window_id, InputEvent)` (window_id from the first `WindowEvent::Created` seen on the control stream; buffer actions until a window exists). Between actions, read frames via `next_frame` and record the first frame whose bytes differ from the baseline (pixel change) → `pixelsChangedAt` (frame index + timestamp).
4. Write up to `--frames` PNGs to `--out` dir via `FrameBuffer::write_png`; print a JSON summary on stdout: `{"frames":N,"fps":N,"rtt_ns":N,"pixelsChangedAt":{"frame":N,"ms":N},"window_id":N}`.
5. Measure RTT once via `session.ping()` at start.
6. Parse args in `main.rs`: dispatch on `argv[1] == "drive"` to `run_drive`; otherwise the existing GUI/headless path. Keep `--addr` etc. parsing shared.
7. Default `--click` button to BTN_LEFT (272). Allow multiple `--click` flags for a sequence.

## Verification

- `cargo build -p wayland-remote-viewer` green on Windows; cross-builds on Linux (no Win32 code path touched — `drive.rs` is `#[cfg(not(windows))]`-agnostic, pure async).
- On gary-agents: launch server + a test client, run `./target/debug/wayland-remote-viewer drive --addr 127.0.0.1:9000 --insecure --frames 5 --out /tmp/drive --click 40,40`; assert JSON printed and PNGs exist. (Pixels will NOT change until issue 05 lands — that's expected; the harness reports `pixelsChangedAt: null`.)
- Unit test (Linux): drive against an in-process server with a static-fill client and assert it connects + captures frames + emits JSON (don't assert pixel change yet).
- `lat check` green.
