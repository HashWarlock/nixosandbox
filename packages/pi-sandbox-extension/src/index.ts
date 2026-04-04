/**
 * Pi Sandbox Extension — entry point
 *
 * Default export: `sandboxExtension(pi)` registers all tools and lifecycle
 * event handlers against the Pi host.
 *
 * All public types are also re-exported for consumers.
 */

import { SessionManager } from "./session-manager.js";
import { createHostDerivedBase } from "./runtime-base.js";
import { createSandboxTools } from "./extension.js";
import { reconcileAll } from "./reconciler.js";

// ---------------------------------------------------------------------------
// Extension entry point
// ---------------------------------------------------------------------------

/**
 * Register the Pi Sandbox extension with the Pi host.
 *
 * @param pi   - The Pi host object (typed as `any` to avoid a hard dependency
 *               on the Pi package; the runtime duck-types these calls).
 * @param opts - Optional overrides for binaryPath and sessionsDir.
 */
export default function sandboxExtension(
  pi: {
    registerTool(tool: {
      name: string;
      description: string;
      parameters: unknown;
      execute(args: unknown): Promise<string>;
    }): void;
    on(event: string, handler: (...args: unknown[]) => void | Promise<void>): void;
  },
  opts: {
    binaryPath?: string;
    sessionsDir?: string;
  } = {},
): void {
  const binaryPath = opts.binaryPath ?? "pi-sandbox-supervisor";
  const sessionManager = new SessionManager(opts.sessionsDir);
  const runtimeBase = createHostDerivedBase();

  // Register tools
  const tools = createSandboxTools(sessionManager, runtimeBase, binaryPath);
  for (const tool of tools) {
    pi.registerTool(tool);
  }

  // Lifecycle: on session_start → reconcile orphaned sessions
  pi.on("session_start", () => {
    try {
      reconcileAll(sessionManager);
    } catch {
      // Reconciliation is best-effort; do not crash the extension
    }
  });

  // Lifecycle: on session_shutdown → mark active sessions idle, clean tmp
  pi.on("session_shutdown", () => {
    const records = sessionManager.list();
    for (const record of records) {
      if (record.state === "active") {
        const session = { record, dir: sessionManager.sessionDir(record.sessionId) };
        try {
          sessionManager.markExecutionFinished(session);
          sessionManager.cleanTmp(session);
        } catch {
          // Best-effort
        }
      }
    }
  });
}

// ---------------------------------------------------------------------------
// Public type re-exports
// ---------------------------------------------------------------------------

export * from "./contract.js";
export { synthesizeCrashResult } from "./crash-synthesis.js";
export {
  RuntimeClient,
  type RuntimeClientOptions,
  type ExecutionHandle,
} from "./runtime-client.js";
export type { Profile } from "./profiles.js";
export { DEFAULT_PROFILE, getProfile, listProfiles } from "./profiles.js";
export type { RuntimeBase } from "./runtime-base.js";
export { createHostDerivedBase } from "./runtime-base.js";
export type {
  SessionRecord,
  ActiveExecution,
  SessionState,
  Session,
} from "./session-manager.js";
export { SessionManager } from "./session-manager.js";
export type { RecoveryAction, RecoveryActionKind } from "./reconciler.js";
export { reconcileAll } from "./reconciler.js";
export type { ToolDefinition } from "./extension.js";
export { createSandboxTools } from "./extension.js";
