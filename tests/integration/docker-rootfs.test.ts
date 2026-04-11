/**
 * macOS Docker integration tests for rootfs execution.
 *
 * Requires: Nix + Docker Desktop.
 * Gate: RUN_DOCKER_TESTS=1
 *
 * Run: RUN_DOCKER_TESTS=1 npx vitest run docker-rootfs.test.ts
 */
import { execFileSync } from "node:child_process";
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { create, execCmd, destroy } from "./helpers.js";

const RUN = process.env.RUN_DOCKER_TESTS === "1";

// Docker tests need NIXOSANDBOX_NO_DOCKER unset
const dockerEnv = {
  ...process.env,
  NIXOSANDBOX_NO_DOCKER: undefined,
} as NodeJS.ProcessEnv;

describe.skipIf(!RUN)("Docker Rootfs (macOS)", () => {
  const sessionsToCleanup: string[] = [];

  beforeAll(() => {
    // Clean up any leftover sidecar
    try {
      execFileSync("docker", ["rm", "-f", "nixosandbox-sidecar"], {
        stdio: "ignore",
      });
    } catch {
      // Didn't exist
    }
  });

  afterAll(() => {
    for (const id of sessionsToCleanup) {
      try {
        destroy(id, dockerEnv);
      } catch {
        // Best-effort
      }
    }
  });

  it("create + exec through Docker sidecar", async () => {
    const { sessionId } = create(["--profile", "strict"], dockerEnv);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["echo", "hello from docker"], {
      env: dockerEnv,
    });

    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");
    expect(stdout).toContain("hello from docker");
  }, 120_000);

  it("verifies rootfs directory structure through Docker", async () => {
    const { sessionId } = create(["--profile", "strict"], dockerEnv);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["ls", "/"], { env: dockerEnv });
    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");

    expect(stdout).toContain("bin");
    expect(stdout).toContain("etc");
    expect(stdout).toContain("workspace");
  }, 60_000);

  it("verifies sandbox user through Docker", async () => {
    const { sessionId } = create(["--profile", "strict"], dockerEnv);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["cat", "/etc/passwd"], {
      env: dockerEnv,
    });
    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");

    expect(stdout).toContain("sandbox");
  }, 60_000);

  it("JSON mode reports full lifecycle events through Docker", async () => {
    const { sessionId } = create(["--profile", "strict"], dockerEnv);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["echo", "lifecycle-test"], {
      env: dockerEnv,
    });
    expect(result.exitCode).toBe(0);

    const started = result.events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "started",
    );
    expect(started).toBeDefined();

    const exited = result.events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "exited",
    );
    expect(exited).toBeDefined();

    const resultEvent = result.events.find(
      (e) => e.type === "result",
    ) as any;
    expect(resultEvent).toBeDefined();
    expect(resultEvent.payload.exitCode).toBe(0);
  }, 60_000);
});
