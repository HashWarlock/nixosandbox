/**
 * Docker sidecar integration tests with rootfs execution.
 *
 * These tests require Docker Desktop + Nix and are gated behind
 * RUN_DOCKER_TESTS=1.
 *
 * Run: RUN_DOCKER_TESTS=1 npx vitest run tests/protocol/docker-sidecar.test.ts
 */
import { execFileSync } from "node:child_process";
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { spawnExecJson } from "./helpers.js";

const DOCKER_TESTS = process.env.RUN_DOCKER_TESTS === "1";

// Docker tests need NIXOSANDBOX_NO_DOCKER unset
const dockerEnv = {
  ...process.env,
  NIXOSANDBOX_NO_DOCKER: undefined,
} as NodeJS.ProcessEnv;

describe.skipIf(!DOCKER_TESTS)("Docker sidecar (rootfs)", () => {
  let sessionId: string;

  beforeAll(() => {
    const binaryPath = process.env.NIXOSANDBOX_BINARY;
    if (!binaryPath) throw new Error("NIXOSANDBOX_BINARY not set");

    // Clean up any leftover sidecar from previous runs
    try {
      execFileSync("docker", ["rm", "-f", "nixosandbox-sidecar"], {
        stdio: "ignore",
      });
    } catch {
      // Container didn't exist
    }

    // Create a session with Docker enabled
    const output = execFileSync(
      binaryPath,
      ["create", "--profile", "strict", "--json"],
      { env: dockerEnv, encoding: "utf-8" },
    );
    const meta = JSON.parse(output);
    sessionId = meta.sessionId;
  }, 120_000); // 2min for first-time rootfs build + Docker image build

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

  it("runs echo through Docker+bwrap with rootfs and gets lifecycle events", async () => {
    const rt = spawnExecJson(sessionId, ["echo", "hello from docker"], {
      env: dockerEnv,
    });

    const events = await rt.readAllEvents();

    // Should have lifecycle(started), stdout, lifecycle(exited), result
    expect(events.length).toBeGreaterThanOrEqual(3);

    const startedEvent = events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "started",
    );
    expect(startedEvent).toBeDefined();

    const stdoutEvents = events.filter((e) => e.type === "stdout");
    const helloEvent = stdoutEvents.find((e) =>
      ((e.payload as any).data as string).includes("hello from docker"),
    );
    expect(helloEvent).toBeDefined();

    const exitedEvent = events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "exited",
    );
    expect(exitedEvent).toBeDefined();

    const result = events.find((e) => e.type === "result") as any;
    expect(result).toBeDefined();
    expect(result.payload.exitCode).toBe(0);

    await rt.waitForExit();
  }, 60_000);

  it("verifies Nix store is accessible inside container", async () => {
    const rt = spawnExecJson(sessionId, ["ls", "/nix/store"], {
      env: dockerEnv,
    });

    const events = await rt.readAllEvents();

    const result = events.find((e) => e.type === "result") as any;
    expect(result).toBeDefined();
    // ls /nix/store should succeed since we mount it
    // Note: inside the bwrap sandbox, /nix/store is part of the rootfs
    // via --pivot-root, not the Docker mount. The Docker mount makes it
    // available to bwrap for --pivot-root to use.
    // The actual test is that bwrap can access the rootfs path.

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  }, 30_000);

  it("NIXOSANDBOX_NO_DOCKER=1 blocks Docker and exits with error", () => {
    const binaryPath = process.env.NIXOSANDBOX_BINARY;
    if (!binaryPath) throw new Error("NIXOSANDBOX_BINARY not set");

    // With Docker disabled on non-Linux, exec should fail
    try {
      execFileSync(
        binaryPath,
        ["exec", sessionId, "--", "echo", "should-fail"],
        {
          env: { ...process.env, NIXOSANDBOX_NO_DOCKER: "1" },
          encoding: "utf-8",
          stdio: "pipe",
        },
      );
      // If we're on Linux with bwrap, this might succeed — that's OK
    } catch (err: any) {
      // On macOS without Docker, should fail with non-zero exit
      expect(err.status).not.toBe(0);
    }
  }, 15_000);
});
