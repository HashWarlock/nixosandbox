/**
 * Crash Synthesis
 *
 * When the Rust runtime exits without emitting a "result" message,
 * the TS client synthesizes one to ensure the extension always has
 * a complete execution result.
 */

import type {
  EffectiveNetwork,
  PlanPayload,
  ResultPayload,
  ValidationPayload,
} from "./contract.js";

/**
 * Synthesize a crash result when Rust exits without emitting a result.
 *
 * Case 1: Validation was received -- preserve last-known effective state.
 *   workspaceModified = true (execution likely started)
 *
 * Case 2: No validation received -- use conservative fallback.
 *   workspaceModified = false (execution likely never started)
 */
export function synthesizeCrashResult(
  lastValidation: ValidationPayload | null,
  plan: PlanPayload,
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
      requested: plan.policy.network.mode,
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
