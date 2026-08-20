# 04 — Cross-Machine Driver (`tools/drive/`)

## Objective

A Bun/TS driver that orchestrates a full run against a remote Linux box (e.g.
`gary-agents`) over SSH: build + launch the server and Wayland test clients,
run the local Rust pieces (`--drive` binary and/or `cargo test`) against the
remote server, collect artifacts (PNGs + JSON), and report pass/fail. The
driver does **no QUIC** — it only does SSH + process lifecycle + file diff,
reusing the Rust QUIC client from issue 02.

## Files

| File | Change |
|------|--------|
| `tools/drive/package.json` (new) | Bun/TS project; deps only for SSH + arg parsing. |
| `tools/drive/src/drive.ts` (new) | CLI entry: `--server <ip:port> --checkout <path> --ssh <host> [--client weston-clickdot|xdg-test] [--frames N]`. |
| `tools/drive/src/remote.ts` (new) | SSH helpers: build server (`cargo build --release`), launch server + clients with `WAYLAND_DISPLAY`, tail `telemetry:` log lines, tear down. |
| `tools/drive/src/run.ts` (new) | Invoke the local `--drive` binary (or `cargo test -p wayland-remote-server input_roundtrip -- --ignored`) and capture stdout JSON + PNG dir. |
| `tools/drive/src/compare.ts` (new) | Optional PNG diff against a baseline (reject if all-black or unchanged). |
| `tools/drive/README.md` (new) | Usage + the contract: what the driver assumes on the remote host (cargo, weston-* clients, XDG_RUNTIME_DIR). |
| `tools/drive/tsconfig.json` (new) | Strict TS config. |

## Steps

1. Confirm Bun is installed on the Windows host (`bun --version`); fall back to Node (`tsx`) if not. Pick one and pin it in `package.json`.
2. `remote.ts`: over SSH run `source ~/.cargo/env && cd <checkout> && git pull && cargo build --release`; kill any stale server; start `RUST_LOG=wayland_remote_server=info ./target/release/wayland-remote-server --listen 0.0.0.0:<port>` via `nohup` + a pidfile; wait for the `wayland-remote listening on:` line; launch `weston-clickdot` (or the chosen client) with `XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=<socket>`.
3. `run.ts`: on the Windows host, run `.\target\debug\wayland-remote-viewer.exe drive --addr <ip:port> --insecure --frames N --out <tmp> --click x,y`; parse the JSON from stdout; copy PNGs to a results dir. Also support a `--mode cargo-test` that SSHes and runs `cargo test ... input_roundtrip -- --ignored` remotely.
4. `compare.ts`: load the captured PNGs (use a tiny dep or shell out to a diff); report pass if `pixelsChangedAt` is non-null (and a dot is non-black at the click coords), else fail with the JSON + log tail.
5. `drive.ts`: wire the phases, print a summary, exit non-zero on fail, clean up remote processes on exit (trap).
6. Keep the driver host-agnostic: `--ssh` target, `--checkout` path, `--server` addr, `--client` selector all parameterized.

## Verification

- `bun run src/drive.ts --server 192.168.10.31:9000 --checkout ~/dev/wayland-remote --ssh gary-agents --client weston-clickdot --frames 10 --click 100,100` runs end-to-end; before 05 it reports FAIL (`pixelsChangedAt: null`); after 05 it reports PASS.
- Driver tears down the server + client on exit (no orphans on gary-agents).
- README documents the remote-host prerequisites and one-line usage.
- `lat check` green (no lat.md section required for tooling, but add a note in the architecture section if it references the harness).
