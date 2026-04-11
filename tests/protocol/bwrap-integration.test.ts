import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

describe("Protocol Test 7: Bwrap Integration (Linux only)", () => {
  const isLinux = process.platform === "linux";

  it.skipIf(!isLinux)("runs command via bwrap with namespaces applied", async () => {
    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "bwrap-test"],
        manifest: {
          mounts: [
            { type: "tmpfs", target: "/tmp", writable: true },
          ],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid", "ipc"],
          network: { mode: "full" },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    // Validation must succeed
    const validation = events[0];
    expect(validation.type).toBe("validation");
    const validationPayload = validation.payload as any;
    expect(validationPayload.ok).toBe(true);

    // namespacesApplied must include the requested namespaces (bwrap available)
    expect(validationPayload.effectiveState.namespacesApplied).toContain("user");
    expect(validationPayload.effectiveState.namespacesApplied).toContain("pid");
    expect(validationPayload.effectiveState.namespacesApplied).toContain("ipc");

    // envApplied must include the env keys
    expect(validationPayload.effectiveState.envApplied).toContain("HOME");
    expect(validationPayload.effectiveState.envApplied).toContain("PATH");

    // Execution must succeed
    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    const resultPayload = result.payload as any;
    expect(resultPayload.exitCode).toBe(0);
    expect(resultPayload.reconciliationHints.terminalState).toBe("clean_exit");

    // Find stdout with "bwrap-test"
    const stdoutEvents = events.filter((e) => e.type === "stdout");
    const bwrapOutput = stdoutEvents.find(
      (e) => ((e.payload as any).data as string).includes("bwrap-test"),
    );
    expect(bwrapOutput).toBeDefined();

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });

  it.skipIf(isLinux)("falls back to direct execution on non-Linux with NAMESPACE_DEGRADED warnings", async () => {
    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "fallback-test"],
        manifest: {
          mounts: [
            { type: "tmpfs", target: "/tmp", writable: true },
          ],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid"],
          network: { mode: "full" },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    const validation = events[0];
    expect(validation.type).toBe("validation");
    const validationPayload = validation.payload as any;
    expect(validationPayload.ok).toBe(true);

    // namespacesApplied must be empty (no bwrap)
    expect(validationPayload.effectiveState.namespacesApplied).toEqual([]);

    // Must have NAMESPACE_DEGRADED warnings
    const nsWarnings = (validationPayload.warnings as any[]).filter(
      (w: any) => w.code === "NAMESPACE_DEGRADED",
    );
    expect(nsWarnings.length).toBe(2); // one for "user", one for "pid"

    // envApplied must still be populated
    expect(validationPayload.effectiveState.envApplied).toContain("HOME");
    expect(validationPayload.effectiveState.envApplied).toContain("PATH");

    // Execution must still succeed (direct execution fallback)
    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    expect((result.payload as any).exitCode).toBe(0);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
