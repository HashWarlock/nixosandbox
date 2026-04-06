import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdtempSync, cpSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// ---------------------------------------------------------------------------
// TestRuntime — identical to protocol test helpers
// ---------------------------------------------------------------------------

export interface TestRuntime {
  send(message: Record<string, unknown>): void;
  readline(): Promise<Record<string, unknown>>;
  readAllEvents(): Promise<Record<string, unknown>[]>;
  kill(signal?: NodeJS.Signals): void;
  waitForExit(): Promise<{ code: number | null; signal: string | null }>;
  stderr: string;
  process: ChildProcess;
}

export function spawnRuntime(): TestRuntime {
  const binaryPath = process.env.RUNTIME_BINARY_PATH;
  if (!binaryPath) {
    throw new Error("RUNTIME_BINARY_PATH not set. Did globalSetup run?");
  }

  const child = spawn(binaryPath, [], {
    stdio: ["pipe", "pipe", "pipe"],
  });

  const rl = createInterface({ input: child.stdout! });
  const lineQueue: string[] = [];
  let lineResolve: ((line: string) => void) | null = null;
  let closed = false;

  rl.on("line", (line) => {
    if (lineResolve) {
      const resolve = lineResolve;
      lineResolve = null;
      resolve(line);
    } else {
      lineQueue.push(line);
    }
  });

  rl.on("close", () => {
    closed = true;
    if (lineResolve) {
      const resolve = lineResolve;
      lineResolve = null;
      resolve("");
    }
  });

  let stderrBuf = "";
  child.stderr!.on("data", (chunk: Buffer) => {
    stderrBuf += chunk.toString();
  });

  function nextLine(): Promise<string> {
    if (lineQueue.length > 0) {
      return Promise.resolve(lineQueue.shift()!);
    }
    if (closed) {
      return Promise.reject(new Error("stdout closed before line received"));
    }
    return new Promise((resolve) => {
      lineResolve = resolve;
    });
  }

  const runtime: TestRuntime = {
    send(message: Record<string, unknown>): void {
      child.stdin!.write(JSON.stringify(message) + "\n");
    },

    async readline(): Promise<Record<string, unknown>> {
      const line = await nextLine();
      return JSON.parse(line) as Record<string, unknown>;
    },

    async readAllEvents(): Promise<Record<string, unknown>[]> {
      const events: Record<string, unknown>[] = [];
      while (true) {
        let line: string;
        try {
          line = await nextLine();
        } catch {
          break;
        }
        const parsed = JSON.parse(line) as Record<string, unknown>;
        events.push(parsed);
        if (parsed.type === "result") {
          break;
        }
      }
      return events;
    },

    kill(signal: NodeJS.Signals = "SIGTERM"): void {
      child.kill(signal);
    },

    waitForExit(): Promise<{ code: number | null; signal: string | null }> {
      return new Promise((resolve) => {
        if (child.exitCode !== null || child.signalCode !== null) {
          resolve({ code: child.exitCode, signal: child.signalCode });
          return;
        }
        child.on("exit", (code, signal) => {
          resolve({ code, signal });
        });
      });
    },

    get stderr(): string {
      return stderrBuf;
    },

    process: child,
  };

  return runtime;
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

const FIXTURES_DIR = resolve(import.meta.dirname, "fixtures");

export interface FixtureWorkspace {
  workspaceDir: string;
  cleanup: () => void;
}

export function copyFixture(fixtureName: string): FixtureWorkspace {
  const fixtureDir = join(FIXTURES_DIR, fixtureName);
  const tempDir = mkdtempSync(join(tmpdir(), `pi-sandbox-integ-${fixtureName}-`));
  cpSync(fixtureDir, tempDir, { recursive: true });
  return {
    workspaceDir: tempDir,
    cleanup: () => rmSync(tempDir, { recursive: true, force: true }),
  };
}

export function makeIntegrationPlan(opts: {
  workspaceDir: string;
  command: string[];
  networkMode?: string;
}): Record<string, unknown> {
  const currentPath = process.env.PATH ?? "/usr/bin:/bin";
  return {
    type: "plan",
    payload: {
      version: 1,
      sessionId: "integ-session-001",
      executionId: "integ-exec-001",
      requestedProfile: "build-install",
      runtimeBaseName: "host-derived",
      manifest: {
        mounts: [
          {
            type: "directory",
            source: opts.workspaceDir,
            target: opts.workspaceDir,
            writable: true,
          },
          {
            type: "tmpfs",
            target: "/tmp",
            writable: true,
          },
        ],
        env: {
          HOME: opts.workspaceDir,
          PATH: currentPath,
        },
        cwd: opts.workspaceDir,
      },
      policy: {
        namespaces: ["user", "pid"],
        network: {
          mode: opts.networkMode ?? "full",
        },
        allowedWritableTargets: [opts.workspaceDir, "/tmp"],
        strictWritePolicy: false,
        envAllowlist: ["HOME", "PATH"],
        denyCommands: [],
      },
      command: opts.command,
    },
  };
}
