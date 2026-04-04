import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";

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

export function makePlan(overrides?: {
  version?: number;
  sessionId?: string;
  executionId?: string;
  requestedProfile?: string;
  runtimeBaseName?: string;
  manifest?: {
    mounts?: Array<{
      type: string;
      source?: string;
      target: string;
      writable: boolean;
    }>;
    env?: Record<string, string>;
    cwd?: string;
  };
  policy?: {
    namespaces?: string[];
    network?: {
      mode: string;
      allowlist?: string[];
    };
    resourceLimits?: Record<string, number>;
    allowedWritableTargets?: string[];
    strictWritePolicy?: boolean;
    envAllowlist?: string[];
    denyCommands?: string[];
  };
  command?: string[];
}): Record<string, unknown> {
  const defaults = {
    version: 1,
    sessionId: "test-session-001",
    executionId: "test-exec-001",
    requestedProfile: "build-install",
    runtimeBaseName: "host-derived",
    manifest: {
      mounts: [
        {
          type: "directory",
          source: "/tmp/pi-sandbox-test/workspace",
          target: "/workspace",
          writable: true,
        },
        {
          type: "tmpfs",
          target: "/tmp",
          writable: true,
        },
      ],
      env: {
        HOME: "/home/sandbox",
        PATH: "/usr/bin:/bin",
      },
      cwd: "/tmp/pi-sandbox-test/workspace",
    },
    policy: {
      namespaces: ["user", "pid"],
      network: {
        mode: "full",
      },
      allowedWritableTargets: ["/workspace", "/tmp"],
      strictWritePolicy: false,
      envAllowlist: ["HOME", "PATH"],
      denyCommands: [],
    },
    command: ["echo", "hello"],
  };

  const merged = {
    ...defaults,
    ...overrides,
    manifest: {
      ...defaults.manifest,
      ...overrides?.manifest,
    },
    policy: {
      ...defaults.policy,
      ...overrides?.policy,
      network: {
        ...defaults.policy.network,
        ...overrides?.policy?.network,
      },
    },
  };

  return {
    type: "plan",
    payload: merged,
  };
}
