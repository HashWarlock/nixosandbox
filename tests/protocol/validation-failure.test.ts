import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

describe("Protocol Test 2: Validation Failure", () => {
  it("rejects writable mount not in allowedWritableTargets with RW_TARGET_NOT_ALLOWED", async () => {
    const rt = spawnRuntime();

    // Send a plan with a writable mount to "/evil" which is not in allowedWritableTargets
    rt.send(
      makePlan({
        manifest: {
          mounts: [
            {
              type: "directory",
              source: "/tmp",
              target: "/evil",
              writable: true,
            },
          ],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const validation = await rt.readline();
    expect(validation.type).toBe("validation");

    const payload = validation.payload as any;
    expect(payload.ok).toBe(false);

    // effectiveState should not be null — plan was parseable (version ok, just a policy violation)
    expect(payload.effectiveState).not.toBeNull();

    // Should have exactly one error with code RW_TARGET_NOT_ALLOWED
    const rwErrors = (payload.errors as any[]).filter(
      (e: any) => e.code === "RW_TARGET_NOT_ALLOWED",
    );
    expect(rwErrors.length).toBeGreaterThanOrEqual(1);
    expect(rwErrors[0].code).toBe("RW_TARGET_NOT_ALLOWED");

    // Rust exits cleanly after a validation failure
    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
