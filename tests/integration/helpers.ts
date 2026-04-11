import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";

function getBinary(): string {
  const bin = process.env.NIXOSANDBOX_BINARY;
  if (!bin) throw new Error("NIXOSANDBOX_BINARY not set. Did globalSetup run?");
  return bin;
}

export interface BuildResult {
  stdout: string;
  exitCode: number;
}

/**
 * Run `nixosandbox build` with the given args.
 */
export function build(args: string[], env?: NodeJS.ProcessEnv): BuildResult {
  try {
    const stdout = execFileSync(getBinary(), ["build", ...args], {
      encoding: "utf-8",
      env: env ?? process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });
    return { stdout: stdout.trim(), exitCode: 0 };
  } catch (err: any) {
    return { stdout: err.stdout?.toString() ?? "", exitCode: err.status ?? 1 };
  }
}

export interface CreateResult {
  sessionId: string;
  metadata: Record<string, unknown>;
}

/**
 * Run `nixosandbox create` and parse the JSON output.
 */
export function create(
  args: string[],
  env?: NodeJS.ProcessEnv,
): CreateResult {
  const stdout = execFileSync(getBinary(), ["create", "--json", ...args], {
    encoding: "utf-8",
    env: env ?? process.env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const metadata = JSON.parse(stdout.trim()) as Record<string, unknown>;
  return { sessionId: metadata.sessionId as string, metadata };
}

export interface ExecResult {
  events: Record<string, unknown>[];
  exitCode: number;
}

/**
 * Run `nixosandbox exec --json <sessionId> -- <command>` and collect all NDJSON events.
 */
export async function execCmd(
  sessionId: string,
  command: string[],
  opts?: { env?: NodeJS.ProcessEnv; extraEnv?: string[] },
): Promise<ExecResult> {
  const envArgs = (opts?.extraEnv ?? []).flatMap((e) => ["--env", e]);
  const args = ["exec", "--json", ...envArgs, sessionId, "--", ...command];

  return new Promise((resolve, reject) => {
    const child = spawn(getBinary(), args, {
      stdio: ["pipe", "pipe", "pipe"],
      env: opts?.env ?? process.env,
    });

    const events: Record<string, unknown>[] = [];
    const rl = createInterface({ input: child.stdout! });

    rl.on("line", (line) => {
      try {
        events.push(JSON.parse(line));
      } catch {
        // Ignore unparseable lines
      }
    });

    child.on("exit", (code) => {
      resolve({ events, exitCode: code ?? 1 });
    });

    child.on("error", (err) => {
      reject(err);
    });
  });
}

export interface ListResult {
  sessions: Record<string, unknown>[];
}

/**
 * Run `nixosandbox list --json` and parse the JSON output.
 */
export function list(env?: NodeJS.ProcessEnv): ListResult {
  const stdout = execFileSync(getBinary(), ["list", "--json"], {
    encoding: "utf-8",
    env: env ?? process.env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const sessions = JSON.parse(stdout.trim()) as Record<string, unknown>[];
  return { sessions };
}

/**
 * Run `nixosandbox destroy <sessionId>`.
 */
export function destroy(
  sessionId: string,
  env?: NodeJS.ProcessEnv,
): number {
  try {
    execFileSync(getBinary(), ["destroy", sessionId], {
      env: env ?? process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });
    return 0;
  } catch (err: any) {
    return err.status ?? 1;
  }
}
