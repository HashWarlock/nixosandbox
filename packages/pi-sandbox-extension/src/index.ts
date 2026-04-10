/**
 * Pi Sandbox Extension — entry point
 *
 * Default export: `sandboxExtension(pi)` registers all tools and lifecycle
 * event handlers against the Pi host.
 *
 * All public types are also re-exported for consumers.
 */

import { createSandboxTools } from "./extension.js";
import { BrowserManager } from "./browser.js";

// ---------------------------------------------------------------------------
// Extension entry point
// ---------------------------------------------------------------------------

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
  } = {},
): void {
  const binaryPath = opts.binaryPath ?? "nixosandbox";
  const browserManager = new BrowserManager();

  // Register tools
  // Pi requires `required` array in JSON Schema; TypeBox omits it for all-optional schemas
  const tools = createSandboxTools(binaryPath, browserManager);
  for (const tool of tools) {
    const params = tool.parameters as Record<string, unknown>;
    if (params.type === "object" && !params.required) {
      params.required = [];
    }
    pi.registerTool(tool);
  }

  // Lifecycle: on session_shutdown → shut down browser
  pi.on("session_shutdown", () => {
    browserManager.shutdown().catch(() => {});
  });
}

// ---------------------------------------------------------------------------
// Public type re-exports
// ---------------------------------------------------------------------------

export * from "./contract.js";
export { synthesizeCrashResult } from "./crash-synthesis.js";
export type { ToolDefinition } from "./extension.js";
export { createSandboxTools } from "./extension.js";
export type {
  SessionMetadata,
  StatusResponse,
  ExecResult,
  CreateOptions,
  CatalogEntry,
  CatalogResponse,
} from "./cli-client.js";
export {
  createSession,
  statusSession,
  listSessions,
  destroySession,
  execCommand,
  catalogPackages,
} from "./cli-client.js";
