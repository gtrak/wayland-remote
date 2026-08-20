# Issue 03 — End-to-end build & test (gary-agents ↔ Windows)

## Objective

Build the server on `gary-agents` (Linux) and the viewer on the Windows box, run a real end-to-end session over the network with two xdg-shell Wayland clients, and validate the PRD Step 4/5/6 milestones: you see the Linux windows as independent Windows windows, you type in them, you close and resize them, and the TOFU `--fingerprint` path works. This is [[005-windows-client-e2e|Plan 005]] Phase D — verification only, no code changes.

## Files

None (verification only). Relies on the commits from [[005-windows-client-e2e/01-server-per-window|Issue 01]] and [[005-windows-client-e2e/02-viewer-win32|Issue 02]].

## Environment

- **Windows box** (this workspace): native MSVC toolchain, `cargo build -p wayland-remote-viewer` produces `wayland-remote-viewer.exe`.
- **gary-agents** (Linux, SSH host): cargo 1.97.1 at `~/.cargo/bin` (run `source ~/.cargo/env`); checkout at `~/dev/wayland-remote` on the same branch as the Windows box; `weston-simple-egl` and `weston-terminal` installed as xdg-shell test clients; `libwayland-client` present.

## Steps

1. From the Windows workspace: `git push -f origin main` (overwrite main per user instruction). On `gary-agents`: `cd ~/dev/wayland-remote && git fetch && git reset --hard origin/main` to sync.
2. On `gary-agents`: `source ~/.cargo/env && cd ~/dev/wayland-remote && cargo test --workspace` — all server/protocol/viewer (Linux) tests green. Then `cargo build -p wayland-remote-server --release` (first run may be slow).
3. On `gary-agents`: print the server cert fingerprint — `cargo run -p wayland-remote-server --release -- --fingerprint` — and record the hex.
4. On `gary-agents`: start the server — `cargo run -p wayland-remote-server --release -- --listen 0.0.0.0:9000`. Note the printed `wayland-remote listening on: $XDG_RUNTIME_DIR/$socket` line for the `WAYLAND_DISPLAY` env to pass clients.
5. On `gary-agents` in two more shells: `WAYLAND_DISPLAY=<that socket> weston-terminal &` and `WAYLAND_DISPLAY=<that socket> weston-simple-egl &` (both use xdg-shell).
6. On the Windows box: `cargo build -p wayland-remote-viewer` then run `.\target\debug\wayland-remote-viewer.exe --addr <gary-agents-ip>:9000 --insecure`. Verify two independent HWNDs appear showing the two remote surfaces.
7. Validate input: click a HWND → focus; type → chars reach the focused `weston-terminal`; mouse move/click/scroll hit the right surface.
8. Validate close: click X on the `weston-simple-egl` HWND → the remote app exits and the HWND disappears.
9. Validate resize: drag a HWND border → after one configure/ack round-trip the remote surface resizes to match.
10. Validate TOFU: quit, rerun with `--fingerprint <hex from step 3>` instead of `--insecure`; confirm the connection succeeds and the fingerprint mismatch path fails loudly when given a wrong fingerprint.
11. If `gary-agents:9000` is firewalled from the Windows box, tunnel over SSH: `ssh -L 9000:localhost:9000 gary-agents` and connect the viewer to `--addr 127.0.0.1:9000` (run the server with `--listen 127.0.0.0:9000`).
12. Record a short results summary (what worked, any deviations, follow-up notes) in the plan's completion archive entry ([[005-windows-client-e2e/04-lat-docs-archive|Issue 04]]).

## Verification

- `cargo test --workspace` green on `gary-agents`.
- Two Wayland apps render as two independent Windows HWNDs.
- Keyboard/mouse input round-trips to the focused toplevel.
- X-close closes the remote app; resize resizes the remote surface after one configure/ack round-trip.
- `--fingerprint` TOFU path succeeds with the correct fingerprint and fails loudly with a wrong one.
- Any failures are logged with enough detail to file a follow-up (do not block documentation of partial success).
