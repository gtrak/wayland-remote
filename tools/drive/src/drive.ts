/**
 * Cross-machine drive harness — CLI entry point.
 *
 * Orchestrates a full run against a remote Linux box over SSH: builds +
 * launches the wayland-remote server and a Wayland test client remotely,
 * runs the local `--drive` viewer against it, collects artifacts (PNGs +
 * JSON), and reports pass/fail. The driver does no QUIC itself.
 */

import { existsSync, mkdirSync, readdirSync } from "node:fs";
import path from "node:path";

import { evaluate } from "./compare.js";
import {
  buildServer,
  getServerIp,
  killClient,
  killStaleServer,
  launchClient,
  startServer,
  tailServerLog,
} from "./remote.js";
import type { DriveResult } from "./run.js";
import { runDrive, viewerPath } from "./run.js";

interface Args {
  ssh: string;
  server: string;
  checkout: string;
  client: string;
  frames: number;
  click: { x: number; y: number } | null;
  out: string;
  skipBuild: boolean;
  expectChange: boolean;
  help: boolean;
}

const USAGE = `usage: bun run src/drive.ts --ssh <host> --server <ip:port> [options]

Options:
  --ssh <host>          SSH host (required)
  --server <ip:port>    Server address (required; port-only resolves IP via SSH)
  --checkout <path>     Remote checkout path (default: ~/dev/wayland-remote)
  --client <name>       Wayland client to launch (default: weston-clickdot)
  --frames N            Max frames to capture (default: 10)
  --click x,y           Click coordinates (default: 100,100)
  --out <dir>           Local output dir for PNGs (default: ./drive-results)
  --skip-build          Skip git pull + cargo build on the remote
  --no-expect-change    Pass without a pixel change (e.g. cursor-sprite clients)
  --expect-change <bool> Require pixel change (default: true)
  --help, -h            Show this help`;

function fail(msg: string): never {
  console.error(`drive: ${msg}`);
  console.error(USAGE);
  process.exit(2);
}

function parseArgs(argv: string[]): Args {
  const a: Args = {
    ssh: "",
    server: "",
    checkout: "~/dev/wayland-remote",
    client: "weston-clickdot",
    frames: 10,
    click: { x: 100, y: 100 },
    out: "./drive-results",
    skipBuild: false,
    expectChange: true,
    help: false,
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = () => {
      if (i + 1 >= argv.length) fail(`missing value for ${arg}`);
      i++;
      return argv[i];
    };
    switch (arg) {
      case "--ssh":
        a.ssh = next();
        break;
      case "--server":
        a.server = next();
        break;
      case "--checkout":
        a.checkout = next();
        break;
      case "--client":
        a.client = next();
        break;
      case "--frames": {
        const v = next();
        const n = Number(v);
        if (!Number.isInteger(n) || n <= 0) fail(`bad --frames: ${v}`);
        a.frames = n;
        break;
      }
      case "--click": {
        const v = next();
        const parts = v.split(",");
        const x = Number(parts[0]);
        const y = Number(parts[1]);
        if (parts.length < 2 || !Number.isFinite(x) || !Number.isFinite(y)) {
          fail(`bad --click: ${v} (expected x,y)`);
        }
        a.click = { x, y };
        break;
      }
      case "--out":
        a.out = next();
        break;
      case "--skip-build":
        a.skipBuild = true;
        break;
      case "--no-expect-change":
        a.expectChange = false;
        break;
      case "--expect-change": {
        const v = next().toLowerCase();
        a.expectChange = v === "true" || v === "1" || v === "yes";
        break;
      }
      case "--help":
      case "-h":
        a.help = true;
        break;
      default:
        fail(`unknown arg: ${arg}`);
    }
  }

  if (a.help) {
    console.log(USAGE);
    process.exit(0);
  }
  if (!a.ssh) fail("--ssh <host> is required");
  if (!a.server) fail("--server <ip:port> is required");
  return a;
}

/** Split "<ip:port>" or "<port>" into {ip, port}. */
function splitAddr(server: string): { ip: string; port: number } {
  const m = server.match(/^(.*):(\d+)$/);
  if (!m) {
    const p = Number(server);
    if (!Number.isInteger(p) || p <= 0 || p > 65535) {
      throw new Error(`bad --server: ${server} (expected ip:port or port)`);
    }
    return { ip: "", port: p };
  }
  const port = Number(m[2]);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(`bad --server port: ${server}`);
  }
  return { ip: m[1], port };
}

async function main() {
  const a = parseArgs(process.argv.slice(2));
  const { ip: serverIp, port } = splitAddr(a.server);
  const viewer = viewerPath();

  try {
    if (!a.skipBuild) {
      await buildServer(a.ssh, a.checkout);
    } else {
      console.log("[drive] skipping remote build (--skip-build)");
    }

    await killStaleServer(a.ssh);
    await startServer(a.ssh, a.checkout, port);

    const addrIp =
      serverIp.length > 0 ? serverIp : await getServerIp(a.ssh);
    const addr = `${addrIp}:${port}`;
    console.log(`[drive] server address: ${addr}`);

    mkdirSync(a.out, { recursive: true });
    const outDir = path.resolve(a.out);

    // Timing-critical: start the drive viewer first (it connects and then
    // waits up to 5s for a window Created event), then launch the remote
    // client so it maps within that window.
    const drivePromise = runDrive(viewer, addr, a.click, a.frames, outDir);
    await new Promise((r) => setTimeout(r, 1000));
    await launchClient(a.ssh, a.client);

    const run = await drivePromise;

    let result: DriveResult | null = run.result;
    if (result === null) {
      console.error(
        `drive: no JSON summary from viewer (exit ${run.exitCode})\n` +
          `stdout:\n${run.stdout}\nstderr:\n${run.stderr}`,
      );
      process.exitCode = 1;
      return;
    }

    const verdict = evaluate(result, a.expectChange);
    const pngs = existsSync(outDir)
      ? readdirSync(outDir).filter((f) => f.endsWith(".png"))
      : [];

    console.log("\n=== Drive result ===");
    console.log(JSON.stringify(result, null, 2));
    console.log(`[drive] PNGs collected: ${pngs.length} in ${outDir}`);
    console.log(`[drive] ${verdict.pass ? "PASS" : "FAIL"}: ${verdict.reason}`);

    if (!verdict.pass) {
      console.log("\n=== Server log (last 10 lines) ===");
      try {
        console.log((await tailServerLog(a.ssh, 10)) || "(empty)");
      } catch (e) {
        console.log(`(could not tail server log: ${(e as Error).message})`);
      }
    }
    process.exitCode = verdict.pass ? 0 : 1;
  } catch (e) {
    console.error(`drive: ${(e as Error).message}`);
    process.exitCode = 1;
  } finally {
    // Always tear down remote processes, even on error.
    console.log("\n[drive] cleanup...");
    for (const step of [
      () => killClient(a.ssh, a.client),
      () => killStaleServer(a.ssh),
    ]) {
      try {
        await step();
      } catch (e) {
        console.error(`[drive] cleanup warning: ${(e as Error).message}`);
      }
    }
    console.log("[drive] done");
  }
}

void main();
