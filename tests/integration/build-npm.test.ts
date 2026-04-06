import { describe, expect, it, afterEach } from "vitest";
import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  spawnRuntime,
  copyFixture,
  makeIntegrationPlan,
  type FixtureWorkspace,
} from "./helpers.js";

describe("Integration: npm install", () => {
  let fixture: FixtureWorkspace | null = null;

  afterEach(() => {
    fixture?.cleanup();
    fixture = null;
  });

  it("runs npm install on tiny-npm fixture and exits cleanly", async () => {
    fixture = copyFixture("tiny-npm");
    const rt = spawnRuntime();

    rt.send(
      makeIntegrationPlan({
        workspaceDir: fixture.workspaceDir,
        command: ["npm", "install"],
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
    expect(resultPayload.reconciliationHints.terminalState).toBe("clean_exit");

    expect(
      existsSync(join(fixture.workspaceDir, "package-lock.json")),
    ).toBe(true);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
