/**
 * CLI Client
 *
 * Thin wrappers for shelling out to the nixosandbox CLI binary.
 * Replaces session-manager.ts + runtime-client.ts with direct CLI delegation.
 */

import { execFileSync, spawn } from "node:child_process";
import { createInterface } from "node:readline";
import type { StreamEvent, ResultPayload } from "./contract.js";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SessionMetadata {
  sessionId: string;
  name: string;
  profile: string;
  rootfsPath: string;
  workspace: string;
  createdAt: string;
  lastExecAt: string | null;
  agent: string | null;
  description: string | null;
}

export interface StatusResponse extends SessionMetadata {
  isolation: string;
  network: string;
}

export interface ExecResult {
  events: Array<StreamEvent | { type: "result"; payload: ResultPayload } | Record<string, unknown>>;
  exitCode: number;
}

export interface CreateOptions {
  profile?: string;
  workspace?: string;
  name?: string;
  agent?: string;
  description?: string;
  withPackages?: string[];
  network?: string;
}

export interface CatalogEntry {
  description: string;
}

export interface CatalogResponse {
  agents: Record<string, CatalogEntry>;
  tools: Record<string, CatalogEntry>;
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

export function createSession(binary: string, opts: CreateOptions): SessionMetadata {
  const args = ["create", "--json"];
  if (opts.withPackages && opts.withPackages.length > 0) {
    args.push("--with", opts.withPackages.join(","));
    if (opts.network) {
      args.push("--network", opts.network);
    }
  } else if (opts.profile) {
    args.push("--profile", opts.profile);
  }
  if (opts.workspace) { args.push("--workspace", opts.workspace); }
  if (opts.name) { args.push("--name", opts.name); }
  if (opts.agent) { args.push("--agent", opts.agent); }
  if (opts.description) { args.push("--description", opts.description); }

  const stdout = execFileSync(binary, args, { encoding: "utf-8" });
  return JSON.parse(stdout.trim()) as SessionMetadata;
}

export function statusSession(binary: string, sessionId: string): StatusResponse {
  const stdout = execFileSync(binary, ["status", sessionId, "--json"], {
    encoding: "utf-8",
  });
  return JSON.parse(stdout.trim()) as StatusResponse;
}

export function listSessions(binary: string): SessionMetadata[] {
  const stdout = execFileSync(binary, ["list", "--json"], {
    encoding: "utf-8",
  });
  return JSON.parse(stdout.trim()) as SessionMetadata[];
}

export function destroySession(binary: string, sessionId: string): void {
  execFileSync(binary, ["destroy", sessionId], { stdio: "pipe" });
}

export function catalogPackages(binary: string, filter?: string): CatalogResponse {
  const args = ["catalog", "--json"];
  if (filter) { args.push("--filter", filter); }
  const stdout = execFileSync(binary, args, { encoding: "utf-8" });
  return JSON.parse(stdout.trim()) as CatalogResponse;
}

export async function execCommand(
  binary: string,
  sessionId: string,
  command: string[],
  opts?: { env?: NodeJS.ProcessEnv; timeoutMs?: number },
): Promise<ExecResult> {
  const args = ["exec", "--json", sessionId, "--", ...command];

  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, {
      stdio: ["pipe", "pipe", "pipe"],
      env: opts?.env ?? process.env,
    });

    const events: ExecResult["events"] = [];
    const rl = createInterface({ input: child.stdout! });

    rl.on("line", (line) => {
      try {
        events.push(JSON.parse(line));
      } catch {
        // Ignore unparseable lines
      }
    });

    let timer: ReturnType<typeof setTimeout> | undefined;
    if (opts?.timeoutMs) {
      timer = setTimeout(() => {
        child.kill("SIGTERM");
      }, opts.timeoutMs);
    }

    child.on("exit", (code) => {
      if (timer) clearTimeout(timer);
      resolve({ events, exitCode: code ?? 1 });
    });

    child.on("error", (err) => {
      if (timer) clearTimeout(timer);
      reject(err);
    });
  });
}
