/**
 * Extension Tools
 *
 * Thin CLI adapter — all sandbox operations delegate to the nixosandbox binary.
 */

import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { normalize, resolve as resolvePath } from "node:path";
import { Type } from "@sinclair/typebox";
import type { TSchema } from "@sinclair/typebox";
import {
  createSession,
  statusSession,
  listSessions,
  execCommand,
  catalogPackages,
} from "./cli-client.js";
import type { BrowserManager } from "./browser.js";

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

function formatExecResult(result: Awaited<ReturnType<typeof execCommand>>): string {
  const stdoutLines: string[] = [];
  const stderrLines: string[] = [];
  let exitCode: number | null = null;
  let durationMs = 0;

  for (const event of result.events) {
    if (event.type === "stdout") {
      stdoutLines.push((event as any).payload.data);
    } else if (event.type === "stderr") {
      stderrLines.push((event as any).payload.data);
    } else if (event.type === "result") {
      const p = (event as any).payload;
      exitCode = p.exitCode;
      durationMs = p.durationMs;
    }
  }

  const lines: string[] = [
    `exit_code: ${exitCode ?? result.exitCode}`,
    `duration_ms: ${durationMs}`,
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
// Battlecard formatter
// ---------------------------------------------------------------------------

function formatBattlecard(status: Record<string, unknown>): string {
  const lines: string[] = [];
  const fields = [
    ["Session", status.sessionId],
    ["Name", status.name],
    ["Description", status.description ?? "-"],
    ["Agent", status.agent ?? "-"],
    ["Profile", status.profile],
    ["Created", status.createdAt],
    ["Last Exec", status.lastExecAt ?? "-"],
    ["Network", status.network ?? "-"],
    ["Isolation", status.isolation ?? "-"],
    ["Workspace", status.workspace],
  ];

  for (const [label, value] of fields) {
    lines.push(`${label}: ${value}`);
  }
  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createSandboxTools(
  binaryPath: string,
  browserManager: BrowserManager,
): ToolDefinition[] {
  // -------------------------------------------------------------------------
  // Tool: sandbox_run
  // -------------------------------------------------------------------------
  const sandboxRun: ToolDefinition = {
    name: "sandbox_run",
    description:
      "Run a command inside an isolated sandbox. " +
      "Use 'with' to compose from catalog packages (call sandbox_catalog first to see available), " +
      "or 'profile' for a built-in profile. Returns combined stdout/stderr and execution metadata.",
    parameters: Type.Object({
      command: Type.Array(Type.String(), {
        description: "Command and arguments to execute, e.g. [\"bash\", \"-c\", \"echo hello\"]",
        minItems: 1,
      }),
      sessionId: Type.Optional(
        Type.String({ description: "Reuse an existing session. Omit to create a new one." }),
      ),
      with: Type.Optional(
        Type.Array(Type.String(), {
          description: "Package names from the catalog (agents + tools). Mutually exclusive with profile.",
        }),
      ),
      profile: Type.Optional(
        Type.String({ description: "Built-in profile name. Defaults to build-install. Mutually exclusive with 'with'." }),
      ),
      network: Type.Optional(
        Type.String({ description: "Network mode: 'off' for review/analysis, 'full' for build/install. Default: 'off'. Only used with 'with'." }),
      ),
      agent: Type.Optional(
        Type.String({ description: "Agent runtime identifier, e.g. 'claude:opus-4-6'" }),
      ),
      description: Type.Optional(
        Type.String({ description: "Purpose of this sandbox session" }),
      ),
      timeoutMs: Type.Optional(
        Type.Number({ description: "Execution timeout in milliseconds." }),
      ),
    }),
    async execute(args: unknown): Promise<string> {
      const {
        command,
        sessionId: maybeSessionId,
        with: withPackages,
        profile = withPackages ? undefined : "build-install",
        network,
        agent,
        description,
        timeoutMs,
      } = args as {
        command: string[];
        sessionId?: string;
        with?: string[];
        profile?: string;
        network?: string;
        agent?: string;
        description?: string;
        timeoutMs?: number;
      };

      let sid = maybeSessionId;
      if (!sid) {
        const meta = createSession(binaryPath, {
          withPackages,
          profile,
          network,
          agent,
          description,
        });
        sid = meta.sessionId;
      }

      const result = await execCommand(binaryPath, sid, command, { timeoutMs });
      return formatExecResult(result);
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

      const status = statusSession(binaryPath, sessionId);
      const absPath = safePath(status.workspace, callerPath);
      return readFileSync(absPath, "utf8");
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

      const status = statusSession(binaryPath, sessionId);
      const absPath = safePath(status.workspace, callerPath);

      const parentDir = absPath.substring(0, absPath.lastIndexOf("/"));
      if (parentDir && parentDir !== status.workspace) {
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

      const status = statusSession(binaryPath, sessionId);
      const absPath = safePath(status.workspace, callerPath);

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
      "Show sandbox session battlecard or list all sessions.",
    parameters: Type.Object({
      sessionId: Type.Optional(
        Type.String({ description: "Session ID for detailed battlecard. Omit to list all." }),
      ),
    }),
    async execute(args: unknown): Promise<string> {
      const { sessionId } = args as { sessionId?: string };

      if (sessionId) {
        const status = statusSession(binaryPath, sessionId);
        return formatBattlecard(status as unknown as Record<string, unknown>);
      }

      const sessions = listSessions(binaryPath);
      if (sessions.length === 0) return "No sessions found.";

      return sessions
        .map(
          (s) =>
            `${s.sessionId}  profile=${s.profile}  agent=${s.agent ?? "-"}  created=${s.createdAt}`,
        )
        .join("\n");
    },
  };

  // -------------------------------------------------------------------------
  // Tool: sandbox_catalog
  // -------------------------------------------------------------------------
  const sandboxCatalog: ToolDefinition = {
    name: "sandbox_catalog",
    description:
      "List available packages for sandbox composition. " +
      "Returns agents (AI coding tools like claude-code, pi, codex) and tools (utilities like python312, git, ripgrep). " +
      "Call this before sandbox_run with 'with' to see what packages are available.",
    parameters: Type.Object({
      filter: Type.Optional(
        Type.String({ description: "Filter results by name or description substring." }),
      ),
    }),
    async execute(args: unknown): Promise<string> {
      const { filter } = args as { filter?: string };
      const catalog = catalogPackages(binaryPath, filter);

      const lines: string[] = [];

      const agentNames = Object.keys(catalog.agents).sort();
      if (agentNames.length > 0) {
        lines.push("Agents (AI coding tools):");
        for (const name of agentNames) {
          lines.push(`  ${name}  ${catalog.agents[name].description}`);
        }
        lines.push("");
      }

      const toolNames = Object.keys(catalog.tools).sort();
      if (toolNames.length > 0) {
        lines.push("Tools (utilities):");
        for (const name of toolNames) {
          lines.push(`  ${name}  ${catalog.tools[name].description}`);
        }
      }

      return lines.join("\n");
    },
  };

  // -------------------------------------------------------------------------
  // Tool: sandbox_browser
  // -------------------------------------------------------------------------
  const sandboxBrowser: ToolDefinition = {
    name: "sandbox_browser",
    description:
      "Interact with a web browser within a sandbox session. Supports goto, screenshot, evaluate, click, type, and close actions.",
    parameters: Type.Object({
      sessionId: Type.String({ description: "Session ID to operate within." }),
      action: Type.Union(
        [
          Type.Literal("goto"),
          Type.Literal("screenshot"),
          Type.Literal("evaluate"),
          Type.Literal("click"),
          Type.Literal("type"),
          Type.Literal("close"),
        ],
        { description: "Browser action to perform." },
      ),
      url: Type.Optional(Type.String({ description: "URL to navigate to (goto action)." })),
      selector: Type.Optional(Type.String({ description: "CSS selector (click/type actions)." })),
      text: Type.Optional(Type.String({ description: "Text to type (type action)." })),
      script: Type.Optional(Type.String({ description: "JavaScript to evaluate." })),
    }),
    async execute(args: unknown): Promise<string> {
      const { sessionId, action, url, selector, text, script } = args as {
        sessionId: string;
        action: string;
        url?: string;
        selector?: string;
        text?: string;
        script?: string;
      };

      return browserManager.execute(sessionId, action, {
        url,
        selector,
        text,
        script,
      });
    },
  };

  return [
    sandboxRun,
    sandboxReadFile,
    sandboxWriteFile,
    sandboxListFiles,
    sandboxSessionInfo,
    sandboxCatalog,
    sandboxBrowser,
  ];
}
