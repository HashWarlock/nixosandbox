import { describe, expect, it, afterEach } from "vitest";
import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  spawnRuntime,
  copyFixture,
  makeIntegrationPlan,
  type FixtureWorkspace,
} from "./helpers.js";

describe("Integration: cargo build", () => {
  let fixture: FixtureWorkspace | null = null;

  afterEach(() => {
    fixture?.cleanup();
    fixture = null;
  });

  it("runs cargo build on tiny-rust fixture and exits cleanly", async () => {
    fixture = copyFixture("tiny-rust");
    const rt = spawnRuntime();

    rt.send(
      makeIntegrationPlan({
        workspaceDir: fixture.workspaceDir,
        command: ["cargo", "build"],
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

    expect(existsSync(join(fixture.workspaceDir, "target"))).toBe(true);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
