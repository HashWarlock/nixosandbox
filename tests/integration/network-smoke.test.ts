import { describe, expect, it, afterEach } from "vitest";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { spawnRuntime, makeIntegrationPlan } from "./helpers.js";

const RUN_NETWORK_TESTS = process.env.RUN_NETWORK_TESTS === "1";

describe("Integration: Network Smoke Tests", () => {
  let tempDir: string | null = null;

  afterEach(() => {
    if (tempDir) {
      rmSync(tempDir, { recursive: true, force: true });
      tempDir = null;
    }
  });

  it.skipIf(!RUN_NETWORK_TESTS)(
    "npm install with real network fetches a dependency",
    async () => {
      tempDir = mkdtempSync(join(tmpdir(), "pi-sandbox-network-"));
      writeFileSync(
        join(tempDir, "package.json"),
        JSON.stringify({
          name: "network-smoke-test",
          version: "1.0.0",
          private: true,
          dependencies: {
            "is-odd": "3.0.1",
          },
        }),
      );

      const rt = spawnRuntime();

      rt.send(
        makeIntegrationPlan({
          workspaceDir: tempDir,
          command: ["npm", "install"],
          networkMode: "full",
        }),
      );

      const events = await rt.readAllEvents();

      const validation = events[0];
      expect(validation.type).toBe("validation");
      expect((validation.payload as any).ok).toBe(true);

      const result = events[events.length - 1];
      expect(result.type).toBe("result");
      const resultPayload = result.payload as any;
      expect(resultPayload.exitCode).toBe(0);
      expect(resultPayload.reconciliationHints.terminalState).toBe(
        "clean_exit",
      );

      expect(existsSync(join(tempDir, "node_modules", "is-odd"))).toBe(true);

      const exit = await rt.waitForExit();
      expect(exit.code).toBe(0);
    },
  );
});
