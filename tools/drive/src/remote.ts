/**
 * SSH helpers for the cross-machine drive harness.
 *
 * Every operation is a single non-interactive `ssh -o ConnectTimeout=5 <host>
 * '<command>'` invocation, run through `child_process.execFile` (no local
 * shell, so no quoting surprises on Windows). Key-based auth is assumed.
 */

import { execFile } from "node:child_process";

const SUPPORTED_CLIENTS = ["weston-clickdot", "weston-flower", "weston-editor"];

/** Runs a command on the remote host, returns trimmed stdout. Throws on non-zero exit. */
export async function sshExec(
  host: string,
  command: string,
  timeoutMs = 30000,
): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile(
      "ssh",
      ["-o", "ConnectTimeout=5", host, command],
      { timeout: timeoutMs },
      (error, stdout, stderr) => {
        if (error) {
          reject(
            new Error(
              `ssh ${host} failed: ${error.message}\n${stderr.trim()}`.trimEnd(),
            ),
          );
          return;
        }
        resolve(stdout.trim());
      },
    );
  });
}

/** Pull latest main and build the release server on the remote host. */
export async function buildServer(
  host: string,
  checkout: string,
): Promise<void> {
  console.log(`[drive] building server on ${host} (${checkout}) ...`);
  const out = await sshExec(
    host,
    // `. file` is POSIX (the remote shell may be dash, where `source` fails).
    `. "$HOME/.cargo/env" && cd ${checkout} && ` +
      `git pull --rebase origin main 2>&1 | tail -3 && ` +
      `cargo build --release 2>&1 | tail -5`,
    600000,
  );
  for (const line of out.split("\n")) {
    console.log(`  ${line}`);
  }
  console.log(`[drive] build done`);
}

/**
 * Kill any previously launched wayland-remote-server on the remote host.
 *
 * The `[w]` char-class trick keeps `pkill -f` from matching the remote shell
 * running this very command (its argv contains the literal pattern).
 */
export async function killStaleServer(host: string): Promise<void> {
  await sshExec(host, `pkill -f '[w]ayland-remote-server' 2>/dev/null; true`);
  console.log(`[drive] stale server killed (if any)`);
}

/**
 * Start the server in the background via nohup and wait for the
 * "wayland-remote listening on:" line in /tmp/wr-server.log (up to 10s).
 *
 * IMPORTANT: the remote command must be a *single simple command* with the
 * redirects applied to it. `a && b &` makes bash background the whole list —
 * the forked subshell keeps the ssh channel's pipes open for the lifetime of
 * the server and the ssh call never returns. `nohup sh -c '<list>' > log
 * 2>&1 < /dev/null &` avoids that: only the simple command is backgrounded,
 * its fds point at the log file, and ssh exits immediately.
 */
export async function startServer(
  host: string,
  checkout: string,
  port: number,
): Promise<void> {
  const inner =
    `cd ${checkout} && . "$HOME/.cargo/env" && ` +
    `RUST_LOG=wayland_remote_server=info ` +
    `./target/release/wayland-remote-server --listen 0.0.0.0:${port}`;
  const cmd = `nohup sh -c '${inner}' > /tmp/wr-server.log 2>&1 < /dev/null &`;
  await sshExec(host, cmd);
  console.log(`[drive] server launched on ${host} port ${port}, waiting for ready...`);

  const deadline = Date.now() + 10000;
  for (;;) {
    const ready = await sshExec(
      host,
      // `|| true` so a not-yet-ready log (grep exit 1) doesn't throw.
      `tail -50 /tmp/wr-server.log | grep -m1 "wayland-remote listening on:" || true`,
    );
    if (ready) {
      console.log(`[drive] server ready`);
      return;
    }
    if (Date.now() > deadline) {
      const tail = await sshExec(host, `tail -10 /tmp/wr-server.log`);
      throw new Error(`server did not report ready within 10s; log tail:\n${tail}`);
    }
    await new Promise((r) => setTimeout(r, 500));
  }
}

/** Launch one of the supported Wayland test clients on the remote host. */
export async function launchClient(
  host: string,
  client: string,
): Promise<void> {
  if (!SUPPORTED_CLIENTS.includes(client)) {
    throw new Error(
      `unsupported client: ${client} (supported: ${SUPPORTED_CLIENTS.join(", ")})`,
    );
  }
  const cmd =
    `export XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1; ` +
    `nohup ${client} > /tmp/wr-client.log 2>&1 < /dev/null &`;
  await sshExec(host, cmd);
  console.log(`[drive] client ${client} launched`);
}

/**
 * Kill the given Wayland test client on the remote host (no-op if absent).
 * Uses the `[w]` char-class trick so `pkill -f` doesn't match the remote
 * shell that is running this command.
 */
export async function killClient(host: string, client: string): Promise<void> {
  const pattern = `[${client.charAt(0)}]${client.slice(1)}`;
  await sshExec(host, `pkill -f '${pattern}' 2>/dev/null; true`);
}

/** Tail the remote server log; used for error reporting. */
export async function tailServerLog(host: string, lines: number): Promise<string> {
  return sshExec(host, `tail -${lines} /tmp/wr-server.log`);
}

/** Return the host's LAN IP (first address from `hostname -I`). */
export async function getServerIp(host: string): Promise<string> {
  const ip = await sshExec(host, `hostname -I | awk '{print $1}'`);
  if (!ip || !/^\d+\.\d+\.\d+\.\d+$/.test(ip)) {
    throw new Error(`could not resolve server IP on ${host}: got "${ip}"`);
  }
  return ip;
}
