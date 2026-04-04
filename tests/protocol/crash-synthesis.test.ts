import { describe, expect, it } from "vitest";
import { synthesizeCrashResult } from "../../packages/pi-sandbox-extension/src/crash-synthesis.js";
import type { PlanPayload, ValidationPayload } from "../../packages/pi-sandbox-extension/src/contract.js";

describe("Protocol Test 5: Crash Synthesis (TS-only)", () => {
  // Minimal PlanPayload for testing
  const basePlan: PlanPayload = {
    version: 1,
    sessionId: "test-session",
    executionId: "test-exec",
    requestedProfile: "build-install",
    manifest: {
      mounts: [],
      env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
      cwd: "/tmp",
    },
    policy: {
      namespaces: ["user"],
      network: { mode: "full" },
      allowedWritableTargets: ["/workspace", "/tmp"],
      strictWritePolicy: false,
    },
    command: ["echo", "hello"],
  };

  it("Case 1: with validation state — preserves effective network, workspaceModified=true", () => {
    const lastValidation: ValidationPayload = {
      ok: true,
      errors: [],
      warnings: [],
      effectiveState: {
        network: {
          requested: "full",
          actual: "full",
          enforcement: "none",
          degraded: false,
        },
        namespacesApplied: ["user"],
        envApplied: ["HOME", "PATH"],
      },
    };

    const result = synthesizeCrashResult(lastValidation, basePlan, null, null, 500);

    expect(result.reconciliationHints.workspaceModified).toBe(true);
    expect(result.reconciliationHints.terminalState).toBe("supervisor_crash");
    expect(result.reconciliationHints.cleanupSucceeded).toBe(false);

    // effectiveNetwork should be preserved from validation state
    expect(result.effectiveNetwork).toEqual(lastValidation.effectiveState!.network);
    expect(result.effectiveNetwork.requested).toBe("full");
    expect(result.effectiveNetwork.actual).toBe("full");
    expect(result.effectiveNetwork.degraded).toBe(false);

    expect(result.exitCode).toBe(-1);
    expect(result.timedOut).toBe(false);
    expect(result.durationMs).toBe(500);
  });

  it("Case 2: without validation state — conservative fallback, workspaceModified=false", () => {
    const result = synthesizeCrashResult(null, basePlan, 1, null, 100);

    expect(result.reconciliationHints.workspaceModified).toBe(false);
    expect(result.reconciliationHints.terminalState).toBe("supervisor_crash");
    expect(result.reconciliationHints.cleanupSucceeded).toBe(false);

    // Conservative fallback: actual=full, degraded=true, enforcement=none
    expect(result.effectiveNetwork.actual).toBe("full");
    expect(result.effectiveNetwork.degraded).toBe(true);
    expect(result.effectiveNetwork.enforcement).toBe("none");
    // requested comes from the plan
    expect(result.effectiveNetwork.requested).toBe("full");

    expect(result.exitCode).toBe(1);
    expect(result.timedOut).toBe(false);
    expect(result.durationMs).toBe(100);
  });
});
