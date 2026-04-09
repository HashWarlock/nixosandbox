/**
 * Crash Synthesis
 *
 * When the Rust runtime exits without emitting a "result" message,
 * the TS client synthesizes one to ensure the extension always has
 * a complete execution result.
 */

import type {
  EffectiveNetwork,
  ResultPayload,
  ValidationPayload,
} from "./contract.js";

/**
 * Synthesize a crash result when the CLI process exits without emitting a result.
 *
 * @param lastValidation - Last validation received (if any)
 * @param requestedNetworkMode - The network mode that was requested (e.g. "off", "full")
 * @param exitCode - Process exit code
 * @param signal - Signal that killed the process (if any)
 * @param durationMs - Execution duration in milliseconds
 */
export function synthesizeCrashResult(
  lastValidation: ValidationPayload | null,
  requestedNetworkMode: string,
  exitCode: number | null,
  signal: string | null,
  durationMs: number,
): ResultPayload {
  let effectiveNetwork: EffectiveNetwork;
  let workspaceModified: boolean;

  if (lastValidation?.effectiveState) {
    effectiveNetwork = lastValidation.effectiveState.network;
    workspaceModified = true;
  } else {
    effectiveNetwork = {
      requested: requestedNetworkMode as any,
      actual: "full",
      enforcement: "none",
      degraded: true,
    };
    workspaceModified = false;
  }

  return {
    exitCode: exitCode ?? -1,
    signal,
    timedOut: false,
    durationMs,
    effectiveNetwork,
    observedConnections: [],
    wouldHaveBlocked: [],
    reconciliationHints: {
      terminalState: "supervisor_crash",
      workspaceModified,
      cleanupSucceeded: false,
    },
  };
}
