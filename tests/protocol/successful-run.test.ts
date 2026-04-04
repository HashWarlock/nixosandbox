import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

describe("Protocol Test 3: Successful Run", () => {
  it("runs echo hello and produces expected event stream", async () => {
    const rt = spawnRuntime();

    // Use /tmp as cwd (always exists)
    rt.send(
      makePlan({
        command: ["echo", "hello"],
        manifest: {
          mounts: [
            { type: "tmpfs", target: "/tmp", writable: true },
          ],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
      }),
    );

    const events = await rt.readAllEvents();

    // Must have at least: validation, lifecycle(started), stdout(hello), lifecycle(exited), result
    expect(events.length).toBeGreaterThanOrEqual(4);

    // First event: validation ok=true
    const validation = events[0];
    expect(validation.type).toBe("validation");
    expect((validation.payload as any).ok).toBe(true);

    // Find the "started" lifecycle event
    const startedEvent = events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "started",
    );
    expect(startedEvent).toBeDefined();

    // Find stdout event containing "hello"
    const stdoutEvents = events.filter((e) => e.type === "stdout");
    expect(stdoutEvents.length).toBeGreaterThanOrEqual(1);
    const helloEvent = stdoutEvents.find((e) =>
      ((e.payload as any).data as string).includes("hello"),
    );
    expect(helloEvent).toBeDefined();

    // Find the "exited" lifecycle event
    const exitedEvent = events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "exited",
    );
    expect(exitedEvent).toBeDefined();

    // Last event: result
    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    const resultPayload = result.payload as any;
    expect(resultPayload.exitCode).toBe(0);
    expect(resultPayload.reconciliationHints.terminalState).toBe("clean_exit");

    // Sequence numbers must strictly increase across all sequenced events
    const sequencedEvents = events.filter((e) => e.sequence !== undefined);
    for (let i = 1; i < sequencedEvents.length; i++) {
      expect(sequencedEvents[i].sequence as number).toBeGreaterThan(
        sequencedEvents[i - 1].sequence as number,
      );
    }

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
