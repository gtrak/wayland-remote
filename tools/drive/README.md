# wayland-remote-drive

Cross-machine test driver. SSHes to a remote Linux box, builds + launches the
wayland-remote server and a Wayland client, runs the local `--drive` viewer
against it, and reports whether input caused a visible pixel change.

The driver does **no QUIC** — it only does SSH + process lifecycle + file
collection. The QUIC client is the local Rust `--drive` viewer binary
(`target/debug/wayland-remote-viewer.exe` / `wayland-remote-viewer`), so build
it first:

    cargo build -p wayland-remote-viewer

## Remote prerequisites

- SSH key-based auth to the host (no password prompts).
- A git checkout of this repo at `--checkout` (default `~/dev/wayland-remote`).
- `~/.cargo/env` (cargo) on the remote for the build step.
- A Wayland session with `XDG_RUNTIME_DIR=/run/user/1000` and
  `WAYLAND_DISPLAY=wayland-1`; the server binds its headless compositor socket
  into that runtime dir.
- One of the test clients installed: `weston-clickdot`, `weston-flower`,
  `weston-editor`.

## Usage

    bun run src/drive.ts --ssh gary-agents --server 192.168.10.31:9000 --client weston-clickdot --click 100,100 --frames 10

## Options

    --ssh <host>          SSH host (required)
    --server <ip:port>    Server address (required)
    --checkout <path>     Remote checkout path (default: ~/dev/wayland-remote)
    --client <name>       Wayland client to launch (default: weston-clickdot)
    --frames N            Max frames to capture (default: 10)
    --click x,y           Click coordinates (default: 100,100)
    --out <dir>           Local output dir for PNGs (default: ./drive-results)
    --skip-build          Skip git pull + cargo build

## Behavior

1. (Unless `--skip-build`) `git pull --rebase origin main` + `cargo build
   --release` on the remote.
2. Kill any stale server, launch the release server on `--listen
   0.0.0.0:<port>`, and wait for the `wayland-remote listening on:` log line.
3. Spawn the local `--drive` viewer (it connects and waits up to 5s for a
   window to be created), then launch the remote Wayland client 1s later so it
   maps inside that window.
4. The viewer prints a JSON summary
   (`{"frames":N,"fps":F,"rtt_ns":N,"pixels_changed_at":{...}|null,"window_id":N}`)
   and writes PNGs to `--out`. The driver passes iff `pixels_changed_at` is
   non-null, prints the result (and the server log tail on failure), tears
   down the remote client + server, and exits 0 on pass / 1 on fail.

Note: until the input fix (issue 05) lands, `pixels_changed_at` will be
`null` and the driver correctly reports FAIL.
