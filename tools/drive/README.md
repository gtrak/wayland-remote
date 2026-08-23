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
    --no-expect-change    Pass without a pixel change (e.g. cursor-sprite clients)
    --expect-change <bool> Require pixel change (default: true)
    --watch               Open a live window to watch the render (close or Ctrl+C to stop)
    --bg                  Run in the background; print pid + log path and return
    --stop                Tear down the remote server + clients (e.g. after --bg)

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
   and writes PNGs to `--out`. By default the driver passes iff
   `pixels_changed_at` is non-null; with `--no-expect-change` it passes if
   the connection, window creation, and frame streaming succeeded,
   regardless of pixels. Either way it prints the result (and the server log
   tail on failure), tears down the remote client + server, and exits 0 on
   pass / 1 on fail.

Note: `weston-clickdot` draws its click feedback via a **cursor sprite**
(`wl_pointer.set_cursor`), not by re-committing the toplevel surface — and
the headless server does not composite cursors into streamed frames. So
`pixels_changed_at` is always `null` for clickdot even when input works. Use
`--no-expect-change` with it to verify the input pipeline is wired, or use a
content-changing client (e.g. `weston-flower`).

## Watch mode (live window)

To watch the render live instead of running the headless capture, pass
`--watch`. It opens a native GDI window on your screen streaming the remote
composite in real time; close the window (or press Ctrl+C) to stop, which
tears down the remote client + server:

    bun run src/drive.ts --ssh gary-agents --server 192.168.10.31:9000 --watch [--client weston-flower]

The remote client launches ~2s after the viewer connects, because the headless
server does not re-send already-mapped windows to a late-connecting viewer.
With `--watch` and no explicit `--client`, the default is `weston-flower` (it
animates) rather than the static `weston-clickdot` cursor sprite.

## Running in the background / stopping

`--watch` runs until the window is closed, so a foreground invocation blocks
the terminal indefinitely. Add `--bg` to run the whole invocation in the
background: the script re-spawns itself detached (without `--bg`), prints the
background pid + a log file path, and returns immediately. The live window
opens and keeps running in the background:

    bun run src/drive.ts --ssh gary-agents --server 192.168.10.31:9000 --watch --bg

To stop a backgrounded watch (a detached child does not receive your
terminal's Ctrl+C), run:

    bun run src/drive.ts --ssh gary-agents --server 192.168.10.31:9000 --stop

This tears down the remote server + any weston test clients. Close the local
viewer window if it is still open (killing the server usually ends the stream
too, but a local viewer may linger).
