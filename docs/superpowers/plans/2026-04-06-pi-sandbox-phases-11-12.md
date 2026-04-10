# Pi Sandbox Phases 11-12 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the legacy sandbox-rs server, add session-based Playwright browser automation, and implement real iptables-based network allowlist enforcement.

**Architecture:** Phase 11 hard-deletes `sandbox-rs/`. Phase 12a adds a BrowserManager + `sandbox_browser` tool in the TS extension using `playwright-core`. Phase 12b adds DNS pre-resolution in the Rust validator, iptables wrapper script generation in plan_builder, and enforcement leak detection in the supervisor.

**Tech Stack:** TypeScript (Playwright, vitest), Rust (std::net for DNS, iptables CLI), NDJSON protocol (unchanged for browser; contract extended for allowlist enforcement)

---

## File Map

### Phase 11 — Delete

| Path | Action |
|---|---|
| `sandbox-rs/` (entire directory) | Delete |

### Phase 12a — Browser

| Path | Action | Responsibility |
|---|---|---|
| `packages/pi-sandbox-extension/src/browser.ts` | Create | BrowserManager: lazy Playwright lifecycle, session-scoped pages |
| `packages/pi-sandbox-extension/src/extension.ts` | Modify | Register `sandbox_browser` tool |
| `packages/pi-sandbox-extension/src/session-manager.ts` | Modify | Call BrowserManager.closePage on session teardown |
| `packages/pi-sandbox-extension/package.json` | Modify | Add `playwright-core` dependency |
| `tests/extension/browser.test.ts` | Create | BrowserManager unit + integration tests |
| `tests/extension/package.json` | Create | Test package config |
| `tests/extension/tsconfig.json` | Create | TypeScript config for tests |
| `tests/extension/vitest.config.ts` | Create | vitest config |

### Phase 12b — Allowlist Enforcement

| Path | Action | Responsibility |
|---|---|---|
| `packages/pi-sandbox-extension/src/contract.ts` | Modify | Add `"allowlist"` to actual, `"best_effort"` to enforcement, new warning codes |
| `crates/pi-sandbox-runtime/src/contract.rs` | Modify | Add `ResolvedAllowlistEntry`, `resolved_allowlist` field |
| `crates/pi-sandbox-runtime/src/validator.rs` | Modify | DNS resolution, iptables detection, enforced allowlist states |
| `crates/pi-sandbox-runtime/src/plan_builder.rs` | Modify | iptables wrapper script, `--unshare-net` for allowlist, iptables mount |
| `crates/pi-sandbox-runtime/src/supervisor.rs` | Modify | Enforcement leak detection warning |
| `tests/protocol/allowlist-enforced.test.ts` | Create | Linux enforcement + macOS degradation tests |

---

### Task 1: Delete legacy sandbox-rs server

**Files:**
- Delete: `sandbox-rs/` (entire directory)

- [ ] **Step 1: Verify current state builds and tests pass**

Run: `cd /Users/hashwarlock/Projects/nixosandbox && cargo build -p pi-sandbox-runtime`
Expected: SUCCESS

- [ ] **Step 2: Delete the sandbox-rs directory**

```bash
rm -rf sandbox-rs/
```

- [ ] **Step 3: Verify pi-sandbox-runtime still builds**

Run: `cargo build -p pi-sandbox-runtime`
Expected: SUCCESS — the runtime crate has no dependency on sandbox-rs.

- [ ] **Step 4: Verify protocol tests still pass**

Run: `cd tests/protocol && npm test`
Expected: All existing tests pass.

- [ ] **Step 5: Verify integration tests still pass**

Run: `cd tests/integration && npm test`
Expected: All existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: remove legacy sandbox-rs server (Phase 11)

The v0-legacy-server tag preserves the full Axum REST server history.
All functionality has been replaced by pi-sandbox-runtime (Rust) and
pi-sandbox-extension (TypeScript) in Phases 0-10."
```

---

### Task 2: Add playwright-core dependency and scaffold browser.ts

**Files:**
- Modify: `packages/pi-sandbox-extension/package.json`
- Create: `packages/pi-sandbox-extension/src/browser.ts`

- [ ] **Step 1: Add playwright-core dependency**

In `packages/pi-sandbox-extension/package.json`, add `playwright-core` to dependencies:

```json
{
  "dependencies": {
    "@sinclair/typebox": "^0.34.0",
    "playwright-core": "^1.50.0"
  }
}
```

Run: `cd packages/pi-sandbox-extension && npm install`

- [ ] **Step 2: Create BrowserManager class**

Create `packages/pi-sandbox-extension/src/browser.ts`:

```typescript
/**
 * Browser Manager
 *
 * Manages a shared Playwright browser instance with session-scoped pages.
 * Lazy-initialized on first use. Each sandbox session gets one persistent
 * page that maintains state across navigation/click/type calls.
 */

import type { Browser, BrowserContext, Page } from "playwright-core";
import { chromium } from "playwright-core";

export class BrowserManager {
  private browser: Browser | null = null;
  private context: BrowserContext | null = null;
  private pages: Map<string, Page> = new Map();

  /**
   * Launch the browser if not already running.
   * Uses PLAYWRIGHT_CHROMIUM_PATH env var or system chromium.
   */
  private async ensureBrowser(): Promise<BrowserContext> {
    if (this.context) return this.context;

    const executablePath = process.env.PLAYWRIGHT_CHROMIUM_PATH || undefined;
    this.browser = await chromium.launch({
      headless: true,
      executablePath,
    });
    this.context = await this.browser.newContext();
    return this.context;
  }

  /**
   * Get or create a page for the given session.
   */
  async getOrCreatePage(sessionId: string): Promise<Page> {
    const existing = this.pages.get(sessionId);
    if (existing && !existing.isClosed()) return existing;

    const ctx = await this.ensureBrowser();
    const page = await ctx.newPage();
    this.pages.set(sessionId, page);
    return page;
  }

  /**
   * Close the page for a specific session (e.g., on session teardown).
   */
  async closePage(sessionId: string): Promise<void> {
    const page = this.pages.get(sessionId);
    if (page && !page.isClosed()) {
      await page.close();
    }
    this.pages.delete(sessionId);
  }

  /**
   * Execute a browser action for a session.
   */
  async execute(
    sessionId: string,
    action: string,
    params: {
      url?: string;
      selector?: string;
      text?: string;
      script?: string;
    },
  ): Promise<string> {
    if (action === "close") {
      await this.closePage(sessionId);
      return "Browser page closed.";
    }

    const page = await this.getOrCreatePage(sessionId);

    switch (action) {
      case "goto": {
        if (!params.url) throw new Error("url is required for goto action");
        const response = await page.goto(params.url, {
          waitUntil: "domcontentloaded",
        });
        const title = await page.title();
        const textContent = await page.evaluate(() => {
          const body = document.body;
          return body ? body.innerText.slice(0, 4000) : "";
        });
        const status = response?.status() ?? 0;
        return [
          `url: ${page.url()}`,
          `status: ${status}`,
          `title: ${title}`,
          "--- content ---",
          textContent,
        ].join("\n");
      }

      case "screenshot": {
        const buffer = await page.screenshot({ type: "png" });
        return buffer.toString("base64");
      }

      case "evaluate": {
        if (!params.script)
          throw new Error("script is required for evaluate action");
        const result = await page.evaluate(params.script);
        return JSON.stringify(result);
      }

      case "click": {
        if (!params.selector)
          throw new Error("selector is required for click action");
        await page.click(params.selector);
        return `Clicked: ${params.selector}`;
      }

      case "type": {
        if (!params.selector)
          throw new Error("selector is required for type action");
        if (!params.text)
          throw new Error("text is required for type action");
        await page.fill(params.selector, params.text);
        return `Typed into: ${params.selector}`;
      }

      default:
        throw new Error(
          `Unknown browser action: "${action}". Valid: goto, screenshot, evaluate, click, type, close`,
        );
    }
  }

  /**
   * Shut down the browser entirely. Called on extension teardown.
   */
  async shutdown(): Promise<void> {
    for (const [id, page] of this.pages) {
      if (!page.isClosed()) {
        await page.close();
      }
      this.pages.delete(id);
    }
    if (this.context) {
      await this.context.close();
      this.context = null;
    }
    if (this.browser) {
      await this.browser.close();
      this.browser = null;
    }
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/package.json packages/pi-sandbox-extension/package-lock.json packages/pi-sandbox-extension/src/browser.ts
git commit -m "feat: add BrowserManager with Playwright session-scoped pages (Phase 12a)"
```

---

### Task 3: Register sandbox_browser tool in extension.ts

**Files:**
- Modify: `packages/pi-sandbox-extension/src/extension.ts`

- [ ] **Step 1: Add BrowserManager import and instance**

At the top of `packages/pi-sandbox-extension/src/extension.ts`, add the import:

```typescript
import { BrowserManager } from "./browser.js";
```

- [ ] **Step 2: Add sandbox_browser tool to createSandboxTools**

The `createSandboxTools` function currently accepts `(sessionManager, runtimeBase, binaryPath)`. Change its signature to also accept a `BrowserManager`:

```typescript
export function createSandboxTools(
  sessionManager: SessionManager,
  runtimeBase: RuntimeBase,
  binaryPath: string,
  browserManager: BrowserManager,
): ToolDefinition[] {
```

Add the `sandbox_browser` tool definition after `sandboxSessionInfo` and before the return array:

```typescript
  // -------------------------------------------------------------------------
  // Tool: sandbox_browser
  // -------------------------------------------------------------------------
  const sandboxBrowser: ToolDefinition = {
    name: "sandbox_browser",
    description:
      "Interact with a web browser within a sandbox session. Supports goto, screenshot, evaluate, click, type, and close actions. The page persists between calls within the same session.",
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
      url: Type.Optional(
        Type.String({ description: "URL to navigate to (goto action)." }),
      ),
      selector: Type.Optional(
        Type.String({
          description: "CSS selector for element (click/type actions).",
        }),
      ),
      text: Type.Optional(
        Type.String({ description: "Text to type (type action)." }),
      ),
      script: Type.Optional(
        Type.String({
          description: "JavaScript to evaluate (evaluate action).",
        }),
      ),
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

      // Verify session exists (except for close which is best-effort)
      if (action !== "close") {
        resolveSession(sessionId);
      }

      return browserManager.execute(sessionId, action, {
        url,
        selector,
        text,
        script,
      });
    },
  };
```

Update the return array to include `sandboxBrowser`:

```typescript
  return [
    sandboxRun,
    sandboxReadFile,
    sandboxWriteFile,
    sandboxListFiles,
    sandboxSessionInfo,
    sandboxBrowser,
  ];
```

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/src/extension.ts
git commit -m "feat: register sandbox_browser tool in extension (Phase 12a)"
```

---

### Task 4: Wire browser cleanup into session manager

**Files:**
- Modify: `packages/pi-sandbox-extension/src/session-manager.ts`

- [ ] **Step 1: Add optional BrowserManager reference to SessionManager**

Add an optional browser manager field and a setter. At the top of `packages/pi-sandbox-extension/src/session-manager.ts`, add the import:

```typescript
import type { BrowserManager } from "./browser.js";
```

Inside the `SessionManager` class, after the `private readonly baseDir: string;` field, add:

```typescript
  private browserManager: BrowserManager | null = null;

  setBrowserManager(bm: BrowserManager): void {
    this.browserManager = bm;
  }
```

- [ ] **Step 2: Call closePage in tombstone method**

In the `tombstone` method of `SessionManager`, add browser cleanup before writing the record:

```typescript
  tombstone(session: Session): Session {
    // Close browser page if browser manager is wired
    if (this.browserManager) {
      this.browserManager.closePage(session.record.sessionId).catch(() => {});
    }
    const record: SessionRecord = {
      ...session.record,
      state: "tombstoned",
      activeExecution: null,
    };
    this._writeRecord(session.dir, record);
    return { record, dir: session.dir };
  }
```

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/src/session-manager.ts
git commit -m "feat: wire browser cleanup into session manager tombstone (Phase 12a)"
```

---

### Task 5: Add browser extension tests

**Files:**
- Create: `tests/extension/package.json`
- Create: `tests/extension/tsconfig.json`
- Create: `tests/extension/vitest.config.ts`
- Create: `tests/extension/browser.test.ts`

- [ ] **Step 1: Scaffold test infrastructure**

Create `tests/extension/package.json`:

```json
{
  "name": "@pi-sandbox/extension-tests",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "devDependencies": {
    "typescript": "^5.7.0",
    "vitest": "^3.0.0",
    "playwright-core": "^1.50.0"
  }
}
```

Create `tests/extension/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "esModuleInterop": true,
    "strict": true,
    "outDir": "dist",
    "rootDir": ".",
    "skipLibCheck": true
  },
  "include": ["*.ts"]
}
```

Create `tests/extension/vitest.config.ts`:

```typescript
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    testTimeout: 30_000,
  },
});
```

Run: `cd tests/extension && npm install`

- [ ] **Step 2: Write browser manager tests**

Create `tests/extension/browser.test.ts`:

```typescript
import { describe, it, expect, afterAll } from "vitest";
import { chromium } from "playwright-core";

// Import the BrowserManager from the extension source
// We test it directly rather than through the extension tool layer.
import { BrowserManager } from "../../packages/pi-sandbox-extension/src/browser.js";

// Skip all tests if no browser is available
const hasBrowser = await (async () => {
  try {
    const b = await chromium.launch({ headless: true });
    await b.close();
    return true;
  } catch {
    return false;
  }
})();

describe.skipIf(!hasBrowser)("BrowserManager", () => {
  const manager = new BrowserManager();

  afterAll(async () => {
    await manager.shutdown();
  });

  it("getOrCreatePage returns a page for a session", async () => {
    const page = await manager.getOrCreatePage("session-1");
    expect(page).toBeDefined();
    expect(page.isClosed()).toBe(false);
  });

  it("getOrCreatePage returns the SAME page for the same session", async () => {
    const page1 = await manager.getOrCreatePage("session-2");
    const page2 = await manager.getOrCreatePage("session-2");
    expect(page1).toBe(page2);
  });

  it("closePage closes the page and removes it from the map", async () => {
    const page = await manager.getOrCreatePage("session-close");
    expect(page.isClosed()).toBe(false);
    await manager.closePage("session-close");
    expect(page.isClosed()).toBe(true);
    // A new call should create a fresh page
    const page2 = await manager.getOrCreatePage("session-close");
    expect(page2).not.toBe(page);
  });

  it("execute goto navigates and returns content", async () => {
    const result = await manager.execute("session-goto", "goto", {
      url: "data:text/html,<html><head><title>Test</title></head><body>Hello World</body></html>",
    });
    expect(result).toContain("title: Test");
    expect(result).toContain("Hello World");
  });

  it("execute screenshot returns base64 PNG", async () => {
    // First navigate somewhere
    await manager.execute("session-ss", "goto", {
      url: "data:text/html,<html><body>Screenshot Test</body></html>",
    });
    const result = await manager.execute("session-ss", "screenshot", {});
    // PNG base64 starts with iVBOR
    expect(result.startsWith("iVBOR")).toBe(true);
  });

  it("execute evaluate runs JavaScript and returns result", async () => {
    await manager.execute("session-eval", "goto", {
      url: "data:text/html,<html><body></body></html>",
    });
    const result = await manager.execute("session-eval", "evaluate", {
      script: "1 + 2",
    });
    expect(result).toBe("3");
  });

  it("execute click clicks an element", async () => {
    await manager.execute("session-click", "goto", {
      url: 'data:text/html,<html><body><button id="btn" onclick="document.title=\'clicked\'">Click me</button></body></html>',
    });
    await manager.execute("session-click", "click", { selector: "#btn" });
    const title = await manager.execute("session-click", "evaluate", {
      script: "document.title",
    });
    expect(title).toBe('"clicked"');
  });

  it("execute type fills an input", async () => {
    await manager.execute("session-type", "goto", {
      url: 'data:text/html,<html><body><input id="inp" /></body></html>',
    });
    await manager.execute("session-type", "type", {
      selector: "#inp",
      text: "hello",
    });
    const value = await manager.execute("session-type", "evaluate", {
      script: 'document.querySelector("#inp").value',
    });
    expect(value).toBe('"hello"');
  });

  it("execute close closes the page", async () => {
    await manager.getOrCreatePage("session-close2");
    const result = await manager.execute("session-close2", "close", {});
    expect(result).toBe("Browser page closed.");
  });

  it("execute throws on unknown action", async () => {
    await expect(
      manager.execute("session-err", "invalid" as any, {}),
    ).rejects.toThrow("Unknown browser action");
  });

  it("shutdown closes all pages and the browser", async () => {
    await manager.getOrCreatePage("session-shutdown-1");
    await manager.getOrCreatePage("session-shutdown-2");
    await manager.shutdown();
    // After shutdown, a new call should re-launch
    const page = await manager.getOrCreatePage("session-after-shutdown");
    expect(page.isClosed()).toBe(false);
    await manager.shutdown();
  });
});
```

- [ ] **Step 3: Run tests**

Run: `cd tests/extension && npm test`
Expected: All tests pass (or all skip if no Chromium available).

- [ ] **Step 4: Commit**

```bash
git add tests/extension/
git commit -m "test: add BrowserManager unit and integration tests (Phase 12a)"
```

---

### Task 6: Update TS contract for allowlist enforcement types

**Files:**
- Modify: `packages/pi-sandbox-extension/src/contract.ts`

- [ ] **Step 1: Add new warning codes**

In `packages/pi-sandbox-extension/src/contract.ts`, update the `WarningCode` type (around line 31):

```typescript
export type WarningCode =
  | "ALLOWLIST_NOT_ENFORCED"
  | "NAMESPACE_DEGRADED"
  | "RESOURCE_LIMIT_IGNORED"
  | "DNS_RESOLUTION_PARTIAL"
  | "ALLOWLIST_DNS_FAILED"
  | "ENFORCEMENT_LEAK"
  | "IPTABLES_NOT_FOUND";
```

- [ ] **Step 2: Update EffectiveNetwork.actual to include "allowlist"**

Update the `EffectiveNetworkSchema` (around line 128):

```typescript
export const EffectiveNetworkSchema = Type.Object({
  requested: NetworkModeSchema,
  actual: Type.Union([
    Type.Literal("off"),
    Type.Literal("full"),
    Type.Literal("allowlist"),
  ]),
  enforcement: Type.Union([
    Type.Literal("enforced"),
    Type.Literal("observed"),
    Type.Literal("none"),
    Type.Literal("best_effort"),
  ]),
  degraded: Type.Boolean(),
});
```

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/src/contract.ts
git commit -m "feat: update TS contract for allowlist enforcement types (Phase 12b)"
```

---

### Task 7: Add ResolvedAllowlistEntry and resolved_allowlist to Rust contract

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/contract.rs`

- [ ] **Step 1: Add ResolvedAllowlistEntry struct**

In `crates/pi-sandbox-runtime/src/contract.rs`, after the `EffectiveNetwork` struct (after line 150), add:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAllowlistEntry {
    pub hostname: String,
    pub ips: Vec<String>,
    pub resolved: bool,
}
```

- [ ] **Step 2: Add resolved_allowlist to EffectiveState**

Update the `EffectiveState` struct (around line 136):

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveState {
    pub network: EffectiveNetwork,
    pub namespaces_applied: Vec<String>,
    pub env_applied: Vec<String>,
    pub resolved_allowlist: Vec<ResolvedAllowlistEntry>,
}
```

- [ ] **Step 3: Fix all EffectiveState construction sites**

In `crates/pi-sandbox-runtime/src/validator.rs`, the `EffectiveState` construction (around line 140) needs the new field:

```rust
    let effective_state = Some(EffectiveState {
        network: effective_network,
        namespaces_applied,
        env_applied,
        resolved_allowlist: vec![],
    });
```

In `crates/pi-sandbox-runtime/src/plan_builder.rs` tests, update `make_effective_state` (around line 139):

```rust
    fn make_effective_state(overrides: Option<EffectiveOverrides>) -> EffectiveState {
        let o = overrides.unwrap_or_default();
        EffectiveState {
            network: EffectiveNetwork {
                requested: o.network_requested.unwrap_or_else(|| "full".to_string()),
                actual: o.network_actual.unwrap_or_else(|| "full".to_string()),
                enforcement: o.network_enforcement.unwrap_or_else(|| "none".to_string()),
                degraded: o.network_degraded.unwrap_or(false),
            },
            namespaces_applied: o.namespaces.unwrap_or_else(|| vec!["user".to_string(), "pid".to_string()]),
            env_applied: vec!["HOME".to_string(), "PATH".to_string()],
            resolved_allowlist: vec![],
        }
    }
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p pi-sandbox-runtime`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add crates/pi-sandbox-runtime/src/contract.rs crates/pi-sandbox-runtime/src/validator.rs crates/pi-sandbox-runtime/src/plan_builder.rs
git commit -m "feat: add ResolvedAllowlistEntry and resolved_allowlist to Rust contract (Phase 12b)"
```

---

### Task 8: Implement DNS resolution and iptables detection in validator

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/validator.rs`

- [ ] **Step 1: Add DNS resolution and iptables detection functions**

At the top of `crates/pi-sandbox-runtime/src/validator.rs`, add the necessary imports and helper functions:

```rust
use std::net::ToSocketAddrs;
use std::path::PathBuf;

use crate::bubblewrap::BwrapAvailability;
use crate::contract::{
    EffectiveNetwork, EffectiveState, PlanPayload, ResolvedAllowlistEntry,
    ValidationError, ValidationPayload, ValidationWarning, PROTOCOL_VERSION,
};

/// Resolve a hostname to IP addresses using system DNS.
fn resolve_hostname(hostname: &str) -> Vec<String> {
    // Try resolving hostname:0 to get IPs
    let addr = format!("{hostname}:0");
    match addr.to_socket_addrs() {
        Ok(addrs) => addrs
            .map(|a| a.ip().to_string())
            .collect::<Vec<_>>(),
        Err(_) => vec![],
    }
}

/// Check if iptables binary is available on the host.
fn detect_iptables() -> Option<PathBuf> {
    #[cfg(not(target_os = "linux"))]
    {
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        match std::process::Command::new("which")
            .arg("iptables")
            .output()
        {
            Ok(output) if output.status.success() => {
                let path_str = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();
                let path = PathBuf::from(&path_str);
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
```

- [ ] **Step 2: Rewrite the effective network resolution logic**

Replace the current step 5-6 block (effective network resolution + allowlist warning, around lines 72-107) in the `validate` function with the expanded logic:

```rust
    // 5. Resolve effective network
    let has_net_namespace = plan.policy.namespaces.iter().any(|ns| ns == "net");
    let bwrap_available = matches!(bwrap, BwrapAvailability::Available { .. });

    let (effective_network, resolved_allowlist) = match plan.policy.network.mode.as_str() {
        "off" => {
            let enforcement = if bwrap_available && has_net_namespace {
                "enforced"
            } else {
                "best_effort"
            };
            let degraded = enforcement != "enforced";
            (
                EffectiveNetwork {
                    requested: "off".to_string(),
                    actual: "off".to_string(),
                    enforcement: enforcement.to_string(),
                    degraded,
                },
                vec![],
            )
        }
        "full" => (
            EffectiveNetwork {
                requested: "full".to_string(),
                actual: "full".to_string(),
                enforcement: "observed".to_string(),
                degraded: false,
            },
            vec![],
        ),
        "allowlist" => {
            let allowlist_hosts = plan
                .policy
                .network
                .allowlist
                .as_deref()
                .unwrap_or(&[]);

            // Resolve DNS for each hostname
            let mut entries: Vec<ResolvedAllowlistEntry> = Vec::new();
            for hostname in allowlist_hosts {
                let ips = resolve_hostname(hostname);
                let resolved = !ips.is_empty();
                if !resolved {
                    warnings.push(ValidationWarning {
                        code: "DNS_RESOLUTION_PARTIAL".to_string(),
                        message: format!(
                            "Failed to resolve allowlist hostname '{hostname}'"
                        ),
                    });
                }
                entries.push(ResolvedAllowlistEntry {
                    hostname: hostname.clone(),
                    ips,
                    resolved,
                });
            }

            let any_resolved = entries.iter().any(|e| e.resolved);
            let iptables_path = detect_iptables();

            // Determine if we can enforce
            let can_enforce = bwrap_available
                && has_net_namespace
                && any_resolved
                && iptables_path.is_some();

            if !bwrap_available || !has_net_namespace {
                warnings.push(ValidationWarning {
                    code: "ALLOWLIST_NOT_ENFORCED".to_string(),
                    message:
                        "Network allowlist requested but cannot be enforced; running in observed mode"
                            .to_string(),
                });
            } else if iptables_path.is_none() {
                warnings.push(ValidationWarning {
                    code: "IPTABLES_NOT_FOUND".to_string(),
                    message:
                        "iptables binary not found on host; allowlist degraded to full/observed"
                            .to_string(),
                });
            } else if !any_resolved {
                warnings.push(ValidationWarning {
                    code: "ALLOWLIST_DNS_FAILED".to_string(),
                    message:
                        "All allowlist hostnames failed DNS resolution; degraded to full/observed"
                            .to_string(),
                });
            }

            if can_enforce {
                (
                    EffectiveNetwork {
                        requested: "allowlist".to_string(),
                        actual: "allowlist".to_string(),
                        enforcement: "enforced".to_string(),
                        degraded: false,
                    },
                    entries,
                )
            } else {
                (
                    EffectiveNetwork {
                        requested: "allowlist".to_string(),
                        actual: "full".to_string(),
                        enforcement: "observed".to_string(),
                        degraded: true,
                    },
                    entries,
                )
            }
        }
        _ => (
            EffectiveNetwork {
                requested: plan.policy.network.mode.clone(),
                actual: "full".to_string(),
                enforcement: "none".to_string(),
                degraded: false,
            },
            vec![],
        ),
    };
```

- [ ] **Step 3: Update the EffectiveState construction**

Update the effective_state construction to use the new `resolved_allowlist`:

```rust
    let effective_state = Some(EffectiveState {
        network: effective_network,
        namespaces_applied,
        env_applied,
        resolved_allowlist,
    });
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p pi-sandbox-runtime`
Expected: SUCCESS

- [ ] **Step 5: Run existing tests to check nothing broke**

Run: `cargo test -p pi-sandbox-runtime`
Expected: All existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-sandbox-runtime/src/validator.rs
git commit -m "feat: add DNS resolution and iptables detection to validator (Phase 12b)"
```

---

### Task 9: Add iptables wrapper script generation to plan_builder

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/plan_builder.rs`

- [ ] **Step 1: Update the build function to handle allowlist enforcement**

In `crates/pi-sandbox-runtime/src/plan_builder.rs`, update the namespace handling (step 4) to also unshare-net for allowlist mode, and add wrapper script logic.

First, update the imports at the top:

```rust
use crate::contract::{EffectiveState, PlanPayload, ResolvedAllowlistEntry};
```

Replace the current `net` arm in the namespace match (around line 56-60) with:

```rust
            "net" => {
                // Unshare network for "off" mode AND for enforced "allowlist" mode
                if effective_state.network.actual == "off"
                    || (effective_state.network.actual == "allowlist"
                        && effective_state.network.enforcement == "enforced")
                {
                    argv.push("--unshare-net".to_string());
                }
            }
```

- [ ] **Step 2: Add iptables wrapper generation function**

After the `build` function (before `#[cfg(test)]`), add:

```rust
/// Generate an iptables wrapper script for allowlist enforcement.
///
/// The script sets iptables OUTPUT policy to DROP, adds ACCEPT rules for
/// each resolved IP + loopback + ESTABLISHED, then exec's the user command.
pub fn generate_iptables_wrapper(entries: &[ResolvedAllowlistEntry]) -> String {
    let mut script = String::new();
    script.push_str("#!/bin/sh\nset -e\n");
    script.push_str("iptables -P OUTPUT DROP\n");
    script.push_str("iptables -A OUTPUT -o lo -j ACCEPT\n");
    script.push_str("iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT\n");

    for entry in entries {
        for ip in &entry.ips {
            script.push_str(&format!("iptables -A OUTPUT -d {ip} -j ACCEPT\n"));
        }
    }

    script.push_str("exec \"$@\"\n");
    script
}

/// Build the Bubblewrap argument vector, wrapping the command with an iptables
/// script when allowlist enforcement is active.
///
/// When allowlist enforcement is enforced:
/// 1. The iptables binary is mounted read-only
/// 2. A wrapper script is written to /tmp/.pi-sandbox-allowlist.sh
/// 3. The bwrap command becomes: /bin/sh /tmp/.pi-sandbox-allowlist.sh <original command>
pub fn build_with_allowlist(
    plan: &PlanPayload,
    effective_state: &EffectiveState,
    iptables_path: Option<&str>,
) -> Vec<String> {
    let needs_wrapper = effective_state.network.actual == "allowlist"
        && effective_state.network.enforcement == "enforced"
        && iptables_path.is_some();

    let mut argv = Vec::new();

    // 1. Mounts
    for mount in &plan.manifest.mounts {
        match mount.mount_type.as_str() {
            "directory" | "file" => {
                let flag = if mount.writable { "--bind" } else { "--ro-bind" };
                let source = mount.source.as_deref().unwrap_or(&mount.target);
                argv.push(flag.to_string());
                argv.push(source.to_string());
                argv.push(mount.target.clone());
            }
            "tmpfs" => {
                argv.push("--tmpfs".to_string());
                argv.push(mount.target.clone());
            }
            _ => {}
        }
    }

    // Mount iptables binary if needed
    if let (true, Some(ipt)) = (needs_wrapper, iptables_path) {
        argv.push("--ro-bind".to_string());
        argv.push(ipt.to_string());
        argv.push("/usr/sbin/iptables".to_string());
    }

    // 2. Devices
    for dev in &["/dev/null", "/dev/zero", "/dev/urandom", "/dev/random"] {
        argv.push("--dev-bind".to_string());
        argv.push(dev.to_string());
        argv.push(dev.to_string());
    }

    // 3. Proc
    argv.push("--proc".to_string());
    argv.push("/proc".to_string());

    // 4. Namespaces
    for ns in &effective_state.namespaces_applied {
        match ns.as_str() {
            "pid" => argv.push("--unshare-pid".to_string()),
            "ipc" => argv.push("--unshare-ipc".to_string()),
            "uts" => argv.push("--unshare-uts".to_string()),
            "net" => {
                if effective_state.network.actual == "off"
                    || (effective_state.network.actual == "allowlist"
                        && effective_state.network.enforcement == "enforced")
                {
                    argv.push("--unshare-net".to_string());
                }
            }
            "cgroup-try" => argv.push("--unshare-cgroup-try".to_string()),
            "user" => {}
            _ => {}
        }
    }

    // 5. Environment
    argv.push("--clearenv".to_string());
    for (key, value) in &plan.manifest.env {
        argv.push("--setenv".to_string());
        argv.push(key.clone());
        argv.push(value.clone());
    }

    // 6. Working directory
    argv.push("--chdir".to_string());
    argv.push(plan.manifest.cwd.clone());

    // 7. Command
    argv.push("--".to_string());
    if needs_wrapper {
        // Wrapper script is written by supervisor to a temp file then bind-mounted
        argv.push("/bin/sh".to_string());
        argv.push("/tmp/.pi-sandbox-allowlist.sh".to_string());
    }
    for part in &plan.command {
        argv.push(part.clone());
    }

    argv
}
```

- [ ] **Step 3: Add tests for the new functions**

Add to the existing `#[cfg(test)] mod tests` block in plan_builder.rs:

```rust
    #[test]
    fn generate_iptables_wrapper_produces_valid_script() {
        let entries = vec![
            ResolvedAllowlistEntry {
                hostname: "example.com".to_string(),
                ips: vec!["93.184.216.34".to_string(), "2606:2800:220:1::1".to_string()],
                resolved: true,
            },
        ];
        let script = generate_iptables_wrapper(&entries);
        assert!(script.contains("#!/bin/sh"));
        assert!(script.contains("iptables -P OUTPUT DROP"));
        assert!(script.contains("iptables -A OUTPUT -d 93.184.216.34 -j ACCEPT"));
        assert!(script.contains("iptables -A OUTPUT -d 2606:2800:220:1::1 -j ACCEPT"));
        assert!(script.contains("iptables -A OUTPUT -o lo -j ACCEPT"));
        assert!(script.contains("exec \"$@\""));
    }

    #[test]
    fn generate_iptables_wrapper_with_no_entries_still_valid() {
        let script = generate_iptables_wrapper(&[]);
        assert!(script.contains("iptables -P OUTPUT DROP"));
        assert!(script.contains("exec \"$@\""));
    }

    #[test]
    fn build_with_allowlist_enforced_includes_unshare_net() {
        let plan = make_plan(None);
        let state = make_effective_state(Some(EffectiveOverrides {
            namespaces: Some(vec!["user".to_string(), "pid".to_string(), "net".to_string()]),
            network_requested: Some("allowlist".to_string()),
            network_actual: Some("allowlist".to_string()),
            network_enforcement: Some("enforced".to_string()),
            ..Default::default()
        }));
        let argv = build_with_allowlist(&plan, &state, Some("/usr/sbin/iptables"));
        assert!(argv.contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn build_with_allowlist_enforced_mounts_iptables() {
        let plan = make_plan(None);
        let state = make_effective_state(Some(EffectiveOverrides {
            namespaces: Some(vec!["user".to_string(), "pid".to_string(), "net".to_string()]),
            network_requested: Some("allowlist".to_string()),
            network_actual: Some("allowlist".to_string()),
            network_enforcement: Some("enforced".to_string()),
            ..Default::default()
        }));
        let argv = build_with_allowlist(&plan, &state, Some("/usr/sbin/iptables"));
        // Check iptables is mounted
        let idx = argv.windows(3).position(|w| {
            w[0] == "--ro-bind" && w[1] == "/usr/sbin/iptables" && w[2] == "/usr/sbin/iptables"
        });
        assert!(idx.is_some());
    }

    #[test]
    fn build_with_allowlist_enforced_uses_wrapper_command() {
        let plan = make_plan(Some(PlanOverrides {
            command: Some(vec!["curl".to_string(), "https://example.com".to_string()]),
            ..Default::default()
        }));
        let state = make_effective_state(Some(EffectiveOverrides {
            namespaces: Some(vec!["user".to_string(), "pid".to_string(), "net".to_string()]),
            network_requested: Some("allowlist".to_string()),
            network_actual: Some("allowlist".to_string()),
            network_enforcement: Some("enforced".to_string()),
            ..Default::default()
        }));
        let argv = build_with_allowlist(&plan, &state, Some("/usr/sbin/iptables"));
        let sep = argv.iter().position(|a| a == "--").unwrap();
        assert_eq!(argv[sep + 1], "/bin/sh");
        assert_eq!(argv[sep + 2], "/tmp/.pi-sandbox-allowlist.sh");
        assert_eq!(argv[sep + 3], "curl");
        assert_eq!(argv[sep + 4], "https://example.com");
    }

    #[test]
    fn build_with_allowlist_not_enforced_skips_wrapper() {
        let plan = make_plan(None);
        let state = make_effective_state(Some(EffectiveOverrides {
            network_requested: Some("allowlist".to_string()),
            network_actual: Some("full".to_string()),
            network_enforcement: Some("observed".to_string()),
            network_degraded: Some(true),
            ..Default::default()
        }));
        let argv = build_with_allowlist(&plan, &state, None);
        // Should NOT contain wrapper
        assert!(!argv.contains(&"/tmp/.pi-sandbox-allowlist.sh".to_string()));
        // Should have the original command directly
        let sep = argv.iter().position(|a| a == "--").unwrap();
        assert_eq!(argv[sep + 1], "echo");
    }
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p pi-sandbox-runtime`
Expected: All tests pass, including the new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-sandbox-runtime/src/plan_builder.rs
git commit -m "feat: add iptables wrapper generation and allowlist-aware plan builder (Phase 12b)"
```

---

### Task 10: Wire allowlist enforcement into supervisor and main

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/supervisor.rs`
- Modify: `crates/pi-sandbox-runtime/src/main.rs`

- [ ] **Step 1: Update supervisor to use build_with_allowlist and write wrapper script**

In `crates/pi-sandbox-runtime/src/supervisor.rs`, update the imports at the top:

```rust
use crate::plan_builder;
```

(Add this if not already imported. Currently the file uses `crate::plan_builder` only in the `Available` branch.)

In the `supervise` function, update the `BwrapAvailability::Available` branch (around line 46-52) to handle the allowlist wrapper:

```rust
        BwrapAvailability::Available { path } => {
            // Detect iptables path for allowlist enforcement
            let iptables_path = if effective_state.network.actual == "allowlist"
                && effective_state.network.enforcement == "enforced"
            {
                // Try to find iptables
                std::process::Command::new("which")
                    .arg("iptables")
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            };

            let argv = plan_builder::build_with_allowlist(
                plan,
                effective_state,
                iptables_path.as_deref(),
            );

            // If allowlist enforcement is active, write the wrapper script to a temp file
            // that bwrap will bind-mount into the sandbox
            if effective_state.network.actual == "allowlist"
                && effective_state.network.enforcement == "enforced"
            {
                let script = plan_builder::generate_iptables_wrapper(
                    &effective_state.resolved_allowlist,
                );
                let script_path = std::env::temp_dir().join(".pi-sandbox-allowlist.sh");
                std::fs::write(&script_path, &script).expect("failed to write iptables wrapper");
                // Add bind mount for the script
                let mut full_argv = vec![
                    "--ro-bind".to_string(),
                    script_path.to_string_lossy().to_string(),
                    "/tmp/.pi-sandbox-allowlist.sh".to_string(),
                ];
                full_argv.extend(argv);
                let mut c = Command::new(path);
                c.args(&full_argv);
                c
            } else {
                let mut c = Command::new(path);
                c.args(&argv);
                c
            }
        }
```

- [ ] **Step 2: Add enforcement leak detection after observer stops**

In `supervisor.rs`, after the observer `stop()` call and `compute_would_have_blocked` (around line 194), add enforcement leak detection:

```rust
    // Stop observer and collect observed connections.
    let observed = observer.stop();
    let would_have_blocked =
        compute_would_have_blocked(&observed, &plan.policy.network.allowlist);

    // Enforcement leak detection: if enforcement was active but observer saw blocked connections
    if effective_state.network.enforcement == "enforced"
        && effective_state.network.actual == "allowlist"
        && !would_have_blocked.is_empty()
    {
        let s = seq.fetch_add(1, Ordering::SeqCst);
        emit(&crate::contract::WarningEnvelope::new(
            s,
            "ENFORCEMENT_LEAK".to_string(),
            format!(
                "Observer detected {} connection(s) that should have been blocked by iptables",
                would_have_blocked.len()
            ),
        ));
    }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p pi-sandbox-runtime`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/pi-sandbox-runtime/src/supervisor.rs
git commit -m "feat: wire allowlist enforcement into supervisor with leak detection (Phase 12b)"
```

---

### Task 11: Add allowlist enforcement protocol tests

**Files:**
- Create: `tests/protocol/allowlist-enforced.test.ts`

- [ ] **Step 1: Write the protocol test**

Create `tests/protocol/allowlist-enforced.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";
import { platform } from "node:os";

describe("Protocol Test 8: Allowlist Enforcement", () => {
  // On Linux with bwrap + iptables: enforcement should be "enforced"
  // On macOS: should degrade to observed (existing behavior)
  const isLinux = platform() === "linux";

  it("enforces allowlist on Linux with bwrap and iptables", async () => {
    if (!isLinux) {
      console.log("Skipping: allowlist enforcement requires Linux with bwrap");
      return;
    }

    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "allowlist-enforced"],
        manifest: {
          mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin:/usr/sbin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid", "net"],
          network: {
            mode: "allowlist",
            allowlist: ["localhost"],
          },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    const validation = events[0];
    expect(validation.type).toBe("validation");
    const vPayload = validation.payload as any;
    expect(vPayload.ok).toBe(true);

    const eff = vPayload.effectiveState;

    // If bwrap and iptables are available, enforcement should be "enforced"
    // Otherwise it degrades — check what the runtime actually reports
    if (eff.network.enforcement === "enforced") {
      expect(eff.network.actual).toBe("allowlist");
      expect(eff.network.degraded).toBe(false);
      // resolved_allowlist should have entries
      expect(eff.resolvedAllowlist.length).toBeGreaterThan(0);
      expect(eff.resolvedAllowlist[0].hostname).toBe("localhost");
      expect(eff.resolvedAllowlist[0].resolved).toBe(true);
    } else {
      // Degraded — bwrap or iptables not available
      expect(eff.network.actual).toBe("full");
      expect(eff.network.enforcement).toBe("observed");
      expect(eff.network.degraded).toBe(true);
    }

    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    const rPayload = result.payload as any;
    expect(rPayload.exitCode).toBe(0);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });

  it("degrades allowlist to observed on macOS", async () => {
    if (isLinux) {
      console.log("Skipping: this test is for macOS degradation");
      return;
    }

    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "allowlist-degraded"],
        manifest: {
          mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid", "net"],
          network: {
            mode: "allowlist",
            allowlist: ["example.com"],
          },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    const validation = events[0];
    expect(validation.type).toBe("validation");
    const vPayload = validation.payload as any;
    expect(vPayload.ok).toBe(true);

    // macOS: always degraded
    expect(vPayload.effectiveState.network.requested).toBe("allowlist");
    expect(vPayload.effectiveState.network.actual).toBe("full");
    expect(vPayload.effectiveState.network.enforcement).toBe("observed");
    expect(vPayload.effectiveState.network.degraded).toBe(true);

    // Should have ALLOWLIST_NOT_ENFORCED warning
    const warnings: any[] = vPayload.warnings ?? [];
    const found = warnings.some(
      (w: any) => w.code === "ALLOWLIST_NOT_ENFORCED",
    );
    expect(found).toBe(true);

    const result = events[events.length - 1];
    expect(result.type).toBe("result");

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });

  it("emits DNS_RESOLUTION_PARTIAL for unresolvable hostname", async () => {
    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "dns-partial"],
        manifest: {
          mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid", "net"],
          network: {
            mode: "allowlist",
            allowlist: ["this-host-definitely-does-not-exist.invalid"],
          },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    const validation = events[0];
    expect(validation.type).toBe("validation");
    const vPayload = validation.payload as any;
    expect(vPayload.ok).toBe(true);

    // Should have DNS resolution warning
    const warnings: any[] = vPayload.warnings ?? [];
    const dnsWarning = warnings.find(
      (w: any) =>
        w.code === "DNS_RESOLUTION_PARTIAL" ||
        w.code === "ALLOWLIST_DNS_FAILED",
    );
    expect(dnsWarning).toBeDefined();

    // Should degrade since no IPs resolved
    expect(vPayload.effectiveState.network.actual).toBe("full");
    expect(vPayload.effectiveState.network.degraded).toBe(true);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
```

- [ ] **Step 2: Run protocol tests**

Run: `cd tests/protocol && npm test`
Expected: All tests pass (new tests execute on both platforms appropriately).

- [ ] **Step 3: Commit**

```bash
git add tests/protocol/allowlist-enforced.test.ts
git commit -m "test: add allowlist enforcement protocol tests (Phase 12b)"
```

---

### Task 12: Run full test suite and tag completion

**Files:** None (verification only)

- [ ] **Step 1: Build Rust runtime**

Run: `cargo build -p pi-sandbox-runtime --release`
Expected: SUCCESS with no errors.

- [ ] **Step 2: Run Rust unit tests**

Run: `cargo test -p pi-sandbox-runtime`
Expected: All tests pass (existing + new plan_builder tests + observer tests).

- [ ] **Step 3: Run protocol tests**

Run: `cd tests/protocol && npm test`
Expected: All tests pass.

- [ ] **Step 4: Run integration tests**

Run: `cd tests/integration && npm test`
Expected: All tests pass.

- [ ] **Step 5: Run extension tests (if browser available)**

Run: `cd tests/extension && npm test`
Expected: All tests pass (or skip if no Chromium).

- [ ] **Step 6: Tag completion**

```bash
git tag v1-phases-11-12-complete
```
