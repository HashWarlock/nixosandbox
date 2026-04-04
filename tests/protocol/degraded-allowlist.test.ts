import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

describe("Protocol Test 6: Degraded Allowlist", () => {
  it("degrades allowlist mode to observed/full and emits ALLOWLIST_NOT_ENFORCED warning", async () => {
    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "allowlist-test"],
        manifest: {
          mounts: [
            { type: "tmpfs", target: "/tmp", writable: true },
          ],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          network: {
            mode: "allowlist",
            allowlist: ["example.com"],
          },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    // First event must be a successful validation
    const validation = events[0];
    expect(validation.type).toBe("validation");
    const validationPayload = validation.payload as any;
    expect(validationPayload.ok).toBe(true);

    // Must have ALLOWLIST_NOT_ENFORCED warning
    const warnings: any[] = validationPayload.warnings ?? [];
    const allowlistWarning = warnings.find(
      (w: any) => w.code === "ALLOWLIST_NOT_ENFORCED",
    );
    expect(allowlistWarning).toBeDefined();

    // effectiveState.network.mode must be "full" (degraded from allowlist)
    const effectiveState = validationPayload.effectiveState;
    expect(effectiveState).not.toBeNull();
    expect(effectiveState.network.mode).toBe("full");

    // Last event must be result
    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    const resultPayload = result.payload as any;

    // Result's effectiveNetwork must also reflect the degraded state
    expect(resultPayload.effectiveNetwork.mode).toBe("full");

    // Clean exit
    expect(resultPayload.exitCode).toBe(0);
    expect(resultPayload.reconciliationHints.terminalState).toBe("clean_exit");

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
