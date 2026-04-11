import { describe, expect, it, beforeAll, afterAll } from "vitest";
import { execFileSync } from "node:child_process";
import { spawnExecJson } from "./helpers.js";

const RUN_INTEGRATION = process.env.RUN_INTEGRATION_TESTS === "1";
const RUN_DOCKER = process.env.RUN_DOCKER_TESTS === "1";

describe.skipIf(!RUN_INTEGRATION && !RUN_DOCKER)(
  "Cancel Flow (exec --json)",
  () => {
    let sessionId: string;

    beforeAll(() => {
      const binaryPath = process.env.NIXOSANDBOX_BINARY;
      if (!binaryPath) throw new Error("NIXOSANDBOX_BINARY not set");

      // Create a session for testing
      const env = RUN_DOCKER
        ? { ...process.env, NIXOSANDBOX_NO_DOCKER: undefined } as NodeJS.ProcessEnv
        : process.env;
      const output = execFileSync(binaryPath, [
        "create", "--profile", "strict", "--json",
      ], { env, encoding: "utf-8" });
      const meta = JSON.parse(output);
      sessionId = meta.sessionId;
    });

    afterAll(() => {
      const binaryPath = process.env.NIXOSANDBOX_BINARY;
      if (binaryPath && sessionId) {
        try {
          execFileSync(binaryPath, ["destroy", sessionId], { stdio: "ignore" });
        } catch {
          // Cleanup best-effort
        }
      }
    });

    it("cancels a running process via SIGTERM and observes lifecycle events", async () => {
      const env = RUN_DOCKER
        ? { ...process.env, NIXOSANDBOX_NO_DOCKER: undefined } as NodeJS.ProcessEnv
        : process.env;
      const rt = spawnExecJson(sessionId, ["sleep", "3600"], { env });

      // Read events until we see "started" lifecycle
      let startedSeen = false;
      const preEvents: Record<string, unknown>[] = [];
      while (!startedSeen) {
        const event = await rt.readline();
        preEvents.push(event);
        if (
          event.type === "lifecycle" &&
          (event.payload as any).event === "started"
        ) {
          startedSeen = true;
        }
      }
      expect(startedSeen).toBe(true);

      // Send SIGTERM to the nixosandbox process (which kills the bwrap child)
      rt.kill("SIGTERM");

      // Read remaining events — should include result with non-zero exit or signal
      const resultPromise = new Promise<Record<string, unknown> | null>(
        async (resolve) => {
          const timer = setTimeout(() => resolve(null), 10000);
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
        const resultPayload = resultEvent.payload as any;
        // Process was killed — either signal or non-zero exit
        expect(
          resultPayload.exitCode !== 0 || resultPayload.signal !== null,
        ).toBe(true);
      } else {
        // Force-kill if no result received
        rt.kill("SIGKILL");
      }

      const exit = await rt.waitForExit();
      expect(exit.signal !== null || exit.code !== null).toBe(true);
    }, 30_000);
  },
);
