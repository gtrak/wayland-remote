import type { DriveResult } from "./run.js";

/**
 * Pass/fail logic for a drive run. By default the input action only "worked"
 * if the server's pixels actually changed at some point. With
 * `expectChange` false (e.g. cursor-sprite clients like weston-clickdot),
 * a successful connect + window + frame stream is enough to pass.
 */
export function evaluate(
  result: DriveResult,
  expectChange: boolean,
): { pass: boolean; reason: string } {
  const detail =
    `frames=${result.frames} fps=${result.fps} rtt_ns=${result.rtt_ns} ` +
    `window_id=${result.window_id}`;

  if (result.pixels_changed_at !== null) {
    const pc = result.pixels_changed_at;
    return {
      pass: true,
      reason: `pixels changed at frame ${pc.frame} (${pc.ms} ms) (${detail})`,
    };
  }

  // No pixel change detected.
  if (!expectChange) {
    return {
      pass: true,
      reason: `no pixel change (expected) — input pipeline verified: ${detail}`,
    };
  }

  return {
    pass: false,
    reason: `pixels_changed_at is null — input did not cause a visual change (${detail})`,
  };
}
