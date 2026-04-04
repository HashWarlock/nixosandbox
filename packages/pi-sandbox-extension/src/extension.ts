/**
 * Extension Tools
 *
 * Factories for the 5 sandbox tools exposed to the Pi host.
 */

import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { normalize, resolve as resolvePath } from "node:path";
import { randomUUID } from "node:crypto";
import { Type } from "@sinclair/typebox";
import type { TSchema } from "@sinclair/typebox";
import { RuntimeClient } from "./runtime-client.js";
import type { StreamEvent, PlanPayload } from "./contract.js";
import { PROTOCOL_VERSION } from "./contract.js";
import { SessionManager } from "./session-manager.js";
import type { Session } from "./session-manager.js";
import { getProfile, DEFAULT_PROFILE } from "./profiles.js";
import type { RuntimeBase } from "./runtime-base.js";

// ---------------------------------------------------------------------------
// Minimal ToolDefinition interface (avoids importing from Pi directly)
// ---------------------------------------------------------------------------

export interface ToolDefinition {
  name: string;
  description: string;
  parameters: TSchema;
  execute(args: unknown): Promise<string>;
}

// ---------------------------------------------------------------------------
// Path safety
// ---------------------------------------------------------------------------

/**
 * Resolve a caller-supplied relative path against a workspace root and
 * verify it does not escape the workspace via path traversal.
 *
 * Returns the resolved absolute path, or throws on violation.
 */
function safePath(workspaceRoot: string, callerPath: string): string {
  const resolved = resolvePath(workspaceRoot, normalize(callerPath));
  if (!resolved.startsWith(workspaceRoot + "/") && resolved !== workspaceRoot) {
    throw new Error(
      `Path traversal detected: "${callerPath}" resolves outside workspace`,
    );
  }
  return resolved;
}

// ---------------------------------------------------------------------------
// Result formatter
// ---------------------------------------------------------------------------

function formatRunResult(
  exitCode: number | null,
  durationMs: number,
  stdoutLines: string[],
  stderrLines: string[],
  terminalState: string,
  effectiveNetworkMode: string,
): string {
  const lines: string[] = [
    `exit_code: ${exitCode ?? "null"}`,
    `duration_ms: ${durationMs}`,
    `terminal_state: ${terminalState}`,
    `network: ${effectiveNetworkMode}`,
  ];

  if (stdoutLines.length > 0) {
    lines.push("--- stdout ---");
    lines.push(...stdoutLines);
  }

  if (stderrLines.length > 0) {
    lines.push("--- stderr ---");
    lines.push(...stderrLines);
  }

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createSandboxTools(
  sessionManager: SessionManager,
  runtimeBase: RuntimeBase,
  binaryPath: string,
): ToolDefinition[] {
  // -------------------------------------------------------------------------
  // Helper: resolve or create a session
  // -------------------------------------------------------------------------
  function resolveSession(sessionId?: string): Session {
    if (sessionId) {
      const existing = sessionManager.load(sessionId);
      if (!existing) {
        throw new Error(`Session not found: ${sessionId}`);
      }
      return existing;
    }
    // Create a new session
    return sessionManager.create(runtimeBase);
  }

  // -------------------------------------------------------------------------
  // Tool: sandbox_run
  // -------------------------------------------------------------------------
  const sandboxRun: ToolDefinition = {
    name: "sandbox_run",
    description:
      "Run a command inside an isolated sandbox. Returns combined stdout/stderr and execution metadata.",
    parameters: Type.Object({
      command: Type.Array(Type.String(), {
        description: "Command and arguments to execute, e.g. [\"bash\", \"-c\", \"echo hello\"]",
        minItems: 1,
      }),
      sessionId: Type.Optional(
        Type.String({ description: "Reuse an existing session. Omit to create a new one." }),
      ),
      profile: Type.Optional(
        Type.String({ description: "Execution profile name. Defaults to build-install." }),
      ),
      timeoutMs: Type.Optional(
        Type.Number({ description: "Execution timeout in milliseconds." }),
      ),
    }),
    async execute(args: unknown): Promise<string> {
      const {
        command,
        sessionId: maybeSessionId,
        profile: profileName = DEFAULT_PROFILE,
        timeoutMs,
      } = args as {
        command: string[];
        sessionId?: string;
        profile?: string;
        timeoutMs?: number;
      };

      const profile = getProfile(profileName);
      const session = resolveSession(maybeSessionId);
      const manifest = sessionManager.buildMountManifest(session, profile, runtimeBase);

      const executionId = randomUUID();

      const plan: PlanPayload = {
        version: PROTOCOL_VERSION,
        sessionId: session.record.sessionId,
        executionId,
        requestedProfile: profile.name,
        runtimeBaseName: runtimeBase.name,
        manifest,
        policy: {
          namespaces: profile.namespaces,
          network: profile.network,
          resourceLimits: profile.resourceLimits,
          allowedWritableTargets: profile.allowedWritableTargets,
          strictWritePolicy: profile.strictWritePolicy,
          envAllowlist: profile.envAllowlist,
          denyCommands: profile.denyCommands,
        },
        command,
      };

      const client = new RuntimeClient({
        binaryPath,
        timeout: timeoutMs,
      });

      const stdoutLines: string[] = [];
      const stderrLines: string[] = [];

      const handle = client.execute(plan, (event: StreamEvent) => {
        if (event.type === "stdout") {
          stdoutLines.push(event.payload.data);
        } else if (event.type === "stderr") {
          stderrLines.push(event.payload.data);
        }
      });

      // Update session record with execution info — best-effort (no real PID from client API)
      const updatedSession = sessionManager.markExecutionStarted(
        session,
        executionId,
        0, // PID not surfaced by RuntimeClient; supervisor tracks it internally
        profile.name,
      );

      try {
        const result = await handle.result;
        sessionManager.markExecutionFinished(updatedSession);

        return formatRunResult(
          result.exitCode,
          result.durationMs,
          stdoutLines,
          stderrLines,
          result.reconciliationHints.terminalState,
          result.effectiveNetwork.actual,
        );
      } catch (err) {
        sessionManager.markExecutionFinished(updatedSession);
        throw err;
      }
    },
  };

  // -------------------------------------------------------------------------
  // Tool: sandbox_read_file
  // -------------------------------------------------------------------------
  const sandboxReadFile: ToolDefinition = {
    name: "sandbox_read_file",
    description: "Read a file from the sandbox workspace.",
    parameters: Type.Object({
      sessionId: Type.String({ description: "Session ID whose workspace to read from." }),
      path: Type.String({ description: "Path relative to the workspace root." }),
    }),
    async execute(args: unknown): Promise<string> {
      const { sessionId, path: callerPath } = args as {
        sessionId: string;
        path: string;
      };

      const session = resolveSession(sessionId);
      const workspaceRoot = sessionManager.getWorkspacePath(session);
      const absPath = safePath(workspaceRoot, callerPath);

      const content = readFileSync(absPath, "utf8");
      return content;
    },
  };

  // -------------------------------------------------------------------------
  // Tool: sandbox_write_file
  // -------------------------------------------------------------------------
  const sandboxWriteFile: ToolDefinition = {
    name: "sandbox_write_file",
    description: "Write a file into the sandbox workspace.",
    parameters: Type.Object({
      sessionId: Type.String({ description: "Session ID whose workspace to write into." }),
      path: Type.String({ description: "Path relative to the workspace root." }),
      content: Type.String({ description: "File content to write." }),
    }),
    async execute(args: unknown): Promise<string> {
      const { sessionId, path: callerPath, content } = args as {
        sessionId: string;
        path: string;
        content: string;
      };

      const session = resolveSession(sessionId);
      const workspaceRoot = sessionManager.getWorkspacePath(session);
      const absPath = safePath(workspaceRoot, callerPath);

      // Ensure parent directories exist
      const parentDir = absPath.substring(0, absPath.lastIndexOf("/"));
      if (parentDir && parentDir !== workspaceRoot) {
        mkdirSync(parentDir, { recursive: true });
      }

      writeFileSync(absPath, content, "utf8");
      return `Written ${content.length} bytes to ${callerPath}`;
    },
  };

  // -------------------------------------------------------------------------
  // Tool: sandbox_list_files
  // -------------------------------------------------------------------------
  const sandboxListFiles: ToolDefinition = {
    name: "sandbox_list_files",
    description: "List files and directories in the sandbox workspace.",
    parameters: Type.Object({
      sessionId: Type.String({ description: "Session ID whose workspace to list." }),
      path: Type.Optional(
        Type.String({ description: "Sub-path relative to the workspace root. Defaults to root." }),
      ),
    }),
    async execute(args: unknown): Promise<string> {
      const { sessionId, path: callerPath = "." } = args as {
        sessionId: string;
        path?: string;
      };

      const session = resolveSession(sessionId);
      const workspaceRoot = sessionManager.getWorkspacePath(session);
      const absPath = safePath(workspaceRoot, callerPath);

      const entries = readdirSync(absPath, { withFileTypes: true });
      if (entries.length === 0) return "(empty directory)";

      return entries
        .map((e) => (e.isDirectory() ? `${e.name}/` : e.name))
        .sort()
        .join("\n");
    },
  };

  // -------------------------------------------------------------------------
  // Tool: sandbox_session_info
  // -------------------------------------------------------------------------
  const sandboxSessionInfo: ToolDefinition = {
    name: "sandbox_session_info",
    description:
      "List all sandbox sessions or describe a specific session.",
    parameters: Type.Object({
      sessionId: Type.Optional(
        Type.String({ description: "Session ID to describe. Omit to list all sessions." }),
      ),
    }),
    async execute(args: unknown): Promise<string> {
      const { sessionId } = args as { sessionId?: string };

      if (sessionId) {
        const session = resolveSession(sessionId);
        return JSON.stringify(session.record, null, 2);
      }

      const records = sessionManager.list();
      if (records.length === 0) return "No sessions found.";

      return records
        .map(
          (r) =>
            `${r.sessionId}  state=${r.state}  created=${r.createdAt}  lastActive=${r.lastActiveAt}`,
        )
        .join("\n");
    },
  };

  return [
    sandboxRun,
    sandboxReadFile,
    sandboxWriteFile,
    sandboxListFiles,
    sandboxSessionInfo,
  ];
}
