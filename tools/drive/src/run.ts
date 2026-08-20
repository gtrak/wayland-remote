/**
 * Local invocation of the viewer's `--drive` mode.
 *
 * The viewer binary does the QUIC; this module only spawns it and parses the
 * JSON summary line it prints on stdout:
 *   {"frames":N,"fps":F,"rtt_ns":N,"pixels_changed_at":{"frame":N,"ms":N}|null,"window_id":N}
 */

import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** JSON summary printed by `wayland-remote-viewer drive`. */
export interface DriveResult {
  frames: number;
  fps: number;
  rtt_ns: number;
  pixels_changed_at: { frame: number; ms: number } | null;
  window_id: number;
}

export interface DriveRun {
  result: DriveResult | null;
  stdout: string;
  stderr: string;
  exitCode: number;
}

const here = path.dirname(fileURLToPath(import.meta.url));
/** Repo root (tools/drive/src -> up three). */
const repoRoot = path.resolve(here, "..", "..", "..");

/** Resolve the local viewer binary path for the current platform. */
export function viewerPath(): string {
  const name =
    os.platform() === "win32"
      ? "wayland-remote-viewer.exe"
      : "wayland-remote-viewer";
  const p = path.join(repoRoot, "target", "debug", name);
  if (!existsSync(p)) {
    throw new Error(
      `viewer binary not found at ${p} — build it with 'cargo build -p wayland-remote-viewer' first`,
    );
  }
  return p;
}

/**
 * Spawn `<viewer> drive --addr <addr> --insecure [--click x,y[,button]]
 * --frames <frames> --out <outDir>`, wait for exit, and parse the JSON line
 * from stdout. `result` is null when no JSON line was found.
 */
export function runDrive(
  viewer: string,
  addr: string,
  click: { x: number; y: number; button?: number } | null,
  frames: number,
  outDir: string,
  timeoutMs = 120000,
): Promise<DriveRun> {
  const args = ["drive", "--addr", addr, "--insecure"];
  if (click) {
    args.push(
      "--click",
      click.button !== undefined
        ? `${click.x},${click.y},${click.button}`
        : `${click.x},${click.y}`,
    );
  }
  args.push("--frames", String(frames), "--out", outDir);

  return new Promise((resolve) => {
    execFile(viewer, args, { timeout: timeoutMs, cwd: repoRoot }, (error, stdout, stderr) => {
      let exitCode = 0;
      if (error) {
        const code = (error as { code?: unknown }).code;
        exitCode =
          typeof code === "string" && /^\d+$/.test(code)
            ? parseInt(code, 10)
            : 1;
      }

      let result: DriveResult | null = null;
      for (const line of stdout.split("\n")) {
        const t = line.trim();
        if (!t.startsWith("{")) continue;
        try {
          result = JSON.parse(t) as DriveResult;
        } catch {
          // not the JSON summary line; keep looking
        }
      }

      resolve({ result, stdout, stderr, exitCode });
    });
  });
}
