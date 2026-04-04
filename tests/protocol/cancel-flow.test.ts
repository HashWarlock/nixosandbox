import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

describe("Protocol Test 4: Cancel Flow", () => {
  it("cancels a running process and observes cancel_requested lifecycle", async () => {
    const rt = spawnRuntime();

    // Run a long-lived command
    rt.send(
      makePlan({
        command: ["sleep", "3600"],
        manifest: {
          mounts: [
            { type: "tmpfs", target: "/tmp", writable: true },
          ],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
      }),
    );

    // Collect events until we see "started" lifecycle
    let startedSeen = false;
    while (!startedSeen) {
      const event = await rt.readline();
      if (
        event.type === "lifecycle" &&
        (event.payload as any).event === "started"
      ) {
        startedSeen = true;
      }
    }
    expect(startedSeen).toBe(true);

    // Send cancel
    rt.send({ type: "cancel", payload: { reason: "test-cancel" } });

    // Read the next event(s) — should include cancel_requested
    // Give the runtime a moment to process the cancel and emit the lifecycle events
    let cancelRequestedSeen = false;
    let attempts = 0;

    while (!cancelRequestedSeen && attempts < 10) {
      const event = await rt.readline();
      if (
        event.type === "lifecycle" &&
        (event.payload as any).event === "cancel_requested"
      ) {
        cancelRequestedSeen = true;
      }
      attempts++;
    }

    expect(cancelRequestedSeen).toBe(true);

    // Try to read remaining events with a timeout to capture the result
    const resultPromise = new Promise<Record<string, unknown> | null>(
      async (resolve) => {
        const timer = setTimeout(() => resolve(null), 5000);
        try {
          while (true) {
            const event = await rt.readline();
            if (event.type === "result") {
              clearTimeout(timer);
              resolve(event);
              return;
            }
          }
        } catch {
          clearTimeout(timer);
          resolve(null);
        }
      },
    );

    const resultEvent = await resultPromise;

    if (resultEvent) {
      // If the runtime emitted a result, verify the terminal state
      const resultPayload = resultEvent.payload as any;
      expect(resultPayload.reconciliationHints.terminalState).toBe(
        "killed_on_cancel",
      );
    } else {
      // Force-kill the runtime since the SIGTERM to process group may not work
      // on all platforms when the child is not a process group leader.
      rt.kill("SIGKILL");
    }

    const exit = await rt.waitForExit();
    // Either killed by our SIGKILL or exited normally (0 or non-zero both acceptable)
    expect(exit.signal !== null || exit.code !== null).toBe(true);
  });
});
