import { describe, expect, it, afterEach } from "vitest";
import { execFileSync } from "node:child_process";
import {
  spawnRuntime,
  copyFixture,
  makeIntegrationPlan,
  type FixtureWorkspace,
} from "./helpers.js";

function hasPython(): boolean {
  try {
    execFileSync("python3", ["--version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

describe("Integration: pip install", () => {
  let fixture: FixtureWorkspace | null = null;

  afterEach(() => {
    fixture?.cleanup();
    fixture = null;
  });

  it.skipIf(!hasPython())(
    "runs pip install -e . on tiny-python fixture and exits cleanly",
    async () => {
      fixture = copyFixture("tiny-python");
      const rt = spawnRuntime();

      rt.send(
        makeIntegrationPlan({
          workspaceDir: fixture.workspaceDir,
          command: ["pip", "install", "-e", ".", "--break-system-packages"],
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

      const exit = await rt.waitForExit();
      expect(exit.code).toBe(0);
    },
  );
});
