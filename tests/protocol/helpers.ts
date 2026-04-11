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

/**
 * Spawn `nixosandbox exec --json <sessionId> -- <command>` and return
 * a TestRuntime that reads NDJSON events from stdout.
 */
export function spawnExecJson(
  sessionId: string,
  command: string[],
  options?: { env?: NodeJS.ProcessEnv; extraArgs?: string[] },
): TestRuntime {
  const binaryPath = process.env.NIXOSANDBOX_BINARY;
  if (!binaryPath) {
    throw new Error("NIXOSANDBOX_BINARY not set. Did globalSetup run?");
  }

  const args = [
    "exec",
    "--json",
    ...(options?.extraArgs ?? []),
    sessionId,
    "--",
    ...command,
  ];

  const child = spawn(binaryPath, args, {
    stdio: ["pipe", "pipe", "pipe"],
    env: options?.env ?? process.env,
  });

  return wrapChildProcess(child);
}

/**
 * Wrap a ChildProcess into a TestRuntime for NDJSON event reading.
 */
function wrapChildProcess(child: ChildProcess): TestRuntime {
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
      if (!line) throw new Error("Empty line received");
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
        if (!line) break;
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
