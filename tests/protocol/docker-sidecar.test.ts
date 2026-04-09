/**
 * Docker sidecar integration tests.
 *
 * These tests require Docker Desktop to be running and are gated behind
 * the RUN_DOCKER_TESTS=1 environment variable.
 *
 * Run: RUN_DOCKER_TESTS=1 npx vitest run tests/protocol/docker-sidecar.test.ts
 */
import { execFileSync, spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { describe, it, expect, beforeAll } from "vitest";
import { spawnRuntime, makePlan } from "./helpers.js";

const DOCKER_TESTS = process.env.RUN_DOCKER_TESTS === "1";

// Docker tests need PI_SANDBOX_NO_DOCKER unset (globalSetup sets it to "1" for non-Docker tests)
const dockerEnv = { ...process.env, PI_SANDBOX_NO_DOCKER: undefined } as NodeJS.ProcessEnv;

describe.skipIf(!DOCKER_TESTS)("Docker sidecar", () => {
  beforeAll(() => {
    // Clean up any leftover sidecar from previous runs
    try {
      execFileSync("docker", ["rm", "-f", "pi-sandbox-sidecar"], {
        stdio: "ignore",
      });
    } catch {
      // Container didn't exist, that's fine
    }
  });

  it("runs echo through Docker+bwrap and reports isolationBackend=docker", async () => {
    const rt = spawnRuntime({ env: dockerEnv });
    const plan = makePlan({
      command: ["echo", "hello from docker sidecar"],
    });

    rt.send(plan);

    const validation = await rt.readline();
    expect(validation).toHaveProperty("type", "validation");

    const payload = (validation as any).payload;
    expect(payload.ok).toBe(true);
    expect(payload.effectiveState.isolationBackend).toBe("docker");

    const events = await rt.readAllEvents();
    const result = events.find((e: any) => e.type === "result") as any;
    expect(result).toBeDefined();
    expect(result.payload.exitCode).toBe(0);

    const stdout = events
      .filter((e: any) => e.type === "stdout")
      .map((e: any) => e.payload.data)
      .join("\n");
    expect(stdout).toContain("hello from docker sidecar");

    await rt.waitForExit();
  }, 60_000); // 60s timeout for first-time image build

  it("reports enforcement=enforced for network=off", async () => {
    const rt = spawnRuntime({ env: dockerEnv });
    const plan = makePlan({
      command: ["echo", "offline test"],
      policy: {
        namespaces: ["user", "pid", "net"],
        network: { mode: "off" },
        allowedWritableTargets: ["/workspace", "/tmp"],
        strictWritePolicy: false,
      },
    });

    rt.send(plan);

    const validation = await rt.readline();
    const payload = (validation as any).payload;
    expect(payload.ok).toBe(true);
    expect(payload.effectiveState.network.actual).toBe("off");
    expect(payload.effectiveState.network.enforcement).toBe("enforced");
    expect(payload.effectiveState.isolationBackend).toBe("docker");

    const events = await rt.readAllEvents();
    const result = events.find((e: any) => e.type === "result") as any;
    expect(result.payload.exitCode).toBe(0);

    await rt.waitForExit();
  }, 30_000);

  it("PI_SANDBOX_NO_DOCKER=1 skips Docker and degrades", async () => {
    const binaryPath = process.env.RUNTIME_BINARY_PATH;
    if (!binaryPath) throw new Error("RUNTIME_BINARY_PATH not set");

    const child = spawn(binaryPath, ["legacy-ndjson"], {
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env, PI_SANDBOX_NO_DOCKER: "1" },
    });

    const rl = createInterface({ input: child.stdout! });
    const lines: string[] = [];
    rl.on("line", (line: string) => lines.push(line));

    const plan = makePlan({ command: ["echo", "no docker"] });
    child.stdin!.write(JSON.stringify(plan) + "\n");

    await new Promise<void>((resolve) => child.on("exit", () => resolve()));

    const validation = JSON.parse(lines[0]);
    expect(validation.payload.effectiveState.isolationBackend).toBe("none");
  }, 15_000);
});
