import type { DriveResult } from "./run.js";

/**
 * Pass/fail logic for a drive run. The input action only "worked" if the
 * server's pixels actually changed at some point.
 */
export function evaluate(result: DriveResult): { pass: boolean; reason: string } {
  const detail =
    `frames=${result.frames} fps=${result.fps} rtt_ns=${result.rtt_ns} ` +
    `window_id=${result.window_id}`;
  if (result.pixels_changed_at === null) {
    return {
      pass: false,
      reason: `pixels_changed_at is null — input did not cause a visual change (${detail})`,
    };
  }
  const pc = result.pixels_changed_at;
  return {
    pass: true,
    reason: `pixels changed at frame ${pc.frame} (${pc.ms} ms) (${detail})`,
  };
}
