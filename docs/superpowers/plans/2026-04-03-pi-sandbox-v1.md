# Pi Sandbox v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Pi-native sandbox runtime with NDJSON protocol, TS extension, and 6 passing protocol tests.

**Architecture:** TypeScript Pi extension (packages/pi-sandbox-extension) orchestrates a Rust subprocess (crates/pi-sandbox-runtime) via NDJSON over stdin/stdout. The extension registers 5 tools with Pi, manages sandbox sessions on disk, and synthesizes crash results. The Rust runtime validates plans, supervises command execution, streams events, and reports results.

**Tech Stack:** TypeScript (vitest, TypeBox, @mariozechner/pi-coding-agent), Rust (serde, serde_json, chrono), NDJSON protocol.

**Spec:** `docs/superpowers/specs/2026-04-03-pi-sandbox-v1-design.md`

---

## File Map

### New files to create

**TS package (`packages/pi-sandbox-extension/`):**
| File | Responsibility |
|------|---------------|
| `package.json` | Package config, dependencies |
| `tsconfig.json` | TypeScript config |
| `vitest.config.ts` | Test runner config |
| `src/contract.ts` | NDJSON message types (TypeBox schemas + TS interfaces) |
| `src/runtime-client.ts` | Spawn Rust subprocess, write/read NDJSON, cancel support |
| `src/crash-synthesis.ts` | Synthesize result when Rust exits without emitting one |
| `src/session-manager.ts` | Session directories, session records, mount manifest generation |
| `src/runtime-base.ts` | HostDerivedBase: resolve host binary paths into mount lists |
| `src/profiles.ts` | Profile registry (4 hardcoded profiles) |
| `src/reconciler.ts` | Scan/recover orphaned sessions on startup |
| `src/extension.ts` | 5 Pi tool definitions (sandbox_run, read/write/list files, session info) |
| `src/index.ts` | Extension entry point (ExtensionFactory) |

**Rust crate (`crates/pi-sandbox-runtime/`):**
| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Crate config, dependencies |
| `src/contract.rs` | Serde structs mirroring contract.ts |
| `src/timestamps.rs` | ISO 8601 timestamp helper |
| `src/validator.rs` | Plan validation, effective state resolution |
| `src/supervisor.rs` | Process spawn, stdout/stderr streaming, cancel, result |
| `src/observer.rs` | Network observation stub, would-have-blocked computation |
| `src/main.rs` | Entry point: read plan, validate, supervise, emit result |

**Protocol tests (`tests/protocol/`):**
| File | Responsibility |
|------|---------------|
| `package.json` | Test package config |
| `tsconfig.json` | TypeScript config for tests |
| `vitest.config.ts` | Test runner config with globalSetup |
| `globalSetup.ts` | Build Rust binary before tests |
| `helpers.ts` | spawnRuntime(), makePlan(), NDJSON I/O utilities |
| `version-mismatch.test.ts` | Test 1 |
| `validation-failure.test.ts` | Test 2 |
| `successful-run.test.ts` | Test 3 |
| `cancel-flow.test.ts` | Test 4 |
| `crash-synthesis.test.ts` | Test 5 (TS-only) |
| `degraded-allowlist.test.ts` | Test 6 |

**Root files:**
| File | Responsibility |
|------|---------------|
| `.gitignore` (modify) | Add node_modules, Rust target for new locations |

---

## Task 1: Phase 0 — Tag Current State and Create Refactor Branch

**Files:**
- No files created or modified

- [ ] **Step 1: Tag the current server state**

```bash
git tag v0-legacy-server -m "Legacy Axum server state before Pi sandbox refactor"
```

- [ ] **Step 2: Create long-lived refactor branch**

```bash
git checkout -b pi-sandbox-refactor
```

- [ ] **Step 3: Verify branch**

Run: `git branch --show-current`
Expected: `pi-sandbox-refactor`

---

## Task 2: Phase 1 — Bootstrap TS Package Scaffolding

**Files:**
- Create: `packages/pi-sandbox-extension/package.json`
- Create: `packages/pi-sandbox-extension/tsconfig.json`
- Create: `packages/pi-sandbox-extension/vitest.config.ts`
- Create: `packages/pi-sandbox-extension/src/index.ts`
- Modify: `.gitignore`

- [ ] **Step 1: Create the TS package directory and package.json**

```bash
mkdir -p packages/pi-sandbox-extension/src
```

Create `packages/pi-sandbox-extension/package.json`:

```json
{
  "name": "@pi-sandbox/extension",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "scripts": {
    "build": "tsc",
    "test": "vitest run",
    "test:watch": "vitest",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@sinclair/typebox": "^0.34.0"
  },
  "devDependencies": {
    "@mariozechner/pi-coding-agent": "*",
    "typescript": "^5.7.0",
    "vitest": "^3.0.0"
  }
}
```

- [ ] **Step 2: Create tsconfig.json**

Create `packages/pi-sandbox-extension/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "outDir": "dist",
    "rootDir": "src",
    "declaration": true,
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true
  },
  "include": ["src/**/*.ts"],
  "exclude": ["node_modules", "dist"]
}
```

- [ ] **Step 3: Create vitest.config.ts**

Create `packages/pi-sandbox-extension/vitest.config.ts`:

```typescript
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
  },
});
```

- [ ] **Step 4: Create placeholder index.ts**

Create `packages/pi-sandbox-extension/src/index.ts`:

```typescript
// Pi Sandbox Extension entry point
// Will export ExtensionFactory once contract and tools are implemented
export {};
```

- [ ] **Step 5: Update .gitignore for new package locations**

Append to `.gitignore`:

```
# Pi Sandbox Extension
packages/pi-sandbox-extension/node_modules/
packages/pi-sandbox-extension/dist/

# Pi Sandbox Runtime
crates/pi-sandbox-runtime/target/

# Protocol tests
tests/protocol/node_modules/
```

- [ ] **Step 6: Install dependencies**

```bash
cd packages/pi-sandbox-extension && npm install
```

- [ ] **Step 7: Verify TypeScript compiles**

```bash
cd packages/pi-sandbox-extension && npx tsc --noEmit
```

Expected: No errors.

- [ ] **Step 8: Commit**

```bash
git add packages/pi-sandbox-extension/ .gitignore
git commit -m "feat: bootstrap pi-sandbox-extension TS package (Phase 1)"
```

---

## Task 3: Phase 1 — Bootstrap Rust Crate Scaffolding

**Files:**
- Create: `crates/pi-sandbox-runtime/Cargo.toml`
- Create: `crates/pi-sandbox-runtime/src/main.rs`

- [ ] **Step 1: Create the Rust crate directory**

```bash
mkdir -p crates/pi-sandbox-runtime/src
```

- [ ] **Step 2: Create Cargo.toml**

Create `crates/pi-sandbox-runtime/Cargo.toml`:

```toml
[package]
name = "pi-sandbox-runtime"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 3: Create placeholder main.rs**

Create `crates/pi-sandbox-runtime/src/main.rs`:

```rust
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Read one line from stdin (the plan message)
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        eprintln!("Failed to read from stdin");
        std::process::exit(1);
    }

    // Stub: echo back a validation failure for now
    let response = serde_json::json!({
        "type": "validation",
        "v": 1,
        "payload": {
            "ok": false,
            "errors": [{"code": "NOT_IMPLEMENTED", "message": "Stub runtime", "field": null}],
            "warnings": [],
            "effectiveState": null
        }
    });

    writeln!(stdout, "{}", response).unwrap();
    stdout.flush().unwrap();
}
```

- [ ] **Step 4: Verify Rust compiles**

```bash
cd crates/pi-sandbox-runtime && cargo build
```

Expected: Compiles successfully.

- [ ] **Step 5: Verify binary runs**

```bash
echo '{"type":"plan","payload":{}}' | cargo run --manifest-path crates/pi-sandbox-runtime/Cargo.toml
```

Expected: JSON validation response on stdout.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-sandbox-runtime/
git commit -m "feat: bootstrap pi-sandbox-runtime Rust crate (Phase 1)"
```

---

## Task 4: Phase 1 — Bootstrap Protocol Test Scaffolding

**Files:**
- Create: `tests/protocol/package.json`
- Create: `tests/protocol/tsconfig.json`
- Create: `tests/protocol/vitest.config.ts`
- Create: `tests/protocol/globalSetup.ts`
- Create: `tests/protocol/helpers.ts`

- [ ] **Step 1: Create test directory and package.json**

```bash
mkdir -p tests/protocol tests/integration
```

Create `tests/protocol/package.json`:

```json
{
  "name": "@pi-sandbox/protocol-tests",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "@sinclair/typebox": "^0.34.0"
  },
  "devDependencies": {
    "typescript": "^5.7.0",
    "vitest": "^3.0.0"
  }
}
```

- [ ] **Step 2: Create tsconfig.json**

Create `tests/protocol/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "outDir": "dist",
    "rootDir": ".",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true
  },
  "include": ["*.ts"],
  "exclude": ["node_modules", "dist"]
}
```

- [ ] **Step 3: Create vitest.config.ts with globalSetup**

Create `tests/protocol/vitest.config.ts`:

```typescript
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["*.test.ts"],
    globalSetup: "./globalSetup.ts",
    testTimeout: 30000,
  },
});
```

- [ ] **Step 4: Create globalSetup.ts**

This builds the Rust binary before any tests run.

Create `tests/protocol/globalSetup.ts`:

```typescript
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const CRATE_DIR = resolve(import.meta.dirname, "../../crates/pi-sandbox-runtime");

export async function setup() {
  console.log("Building pi-sandbox-runtime...");
  execFileSync("cargo", ["build", "--release"], {
    cwd: CRATE_DIR,
    stdio: "inherit",
  });

  const binaryPath = resolve(CRATE_DIR, "target/release/pi-sandbox-runtime");
  if (!existsSync(binaryPath)) {
    throw new Error(`Binary not found at ${binaryPath}`);
  }

  process.env.RUNTIME_BINARY_PATH = binaryPath;
  console.log(`Runtime binary: ${binaryPath}`);
}
```

- [ ] **Step 5: Create helpers.ts**

Create `tests/protocol/helpers.ts`:

```typescript
import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";

/**
 * Wrapper around the Rust runtime subprocess for protocol testing.
 * Provides typed NDJSON read/write over stdin/stdout.
 */
export interface TestRuntime {
  /** Write a JSON message as NDJSON line to stdin */
  send(message: Record<string, unknown>): void;
  /** Read one NDJSON line from stdout, parsed as JSON */
  readline(): Promise<Record<string, unknown>>;
  /** Read all remaining events until a "result" message or process exit */
  readAllEvents(): Promise<Record<string, unknown>[]>;
  /** Send a signal to the process */
  kill(signal?: NodeJS.Signals): void;
  /** Wait for the process to exit, returns exit code and signal */
  waitForExit(): Promise<{ code: number | null; signal: string | null }>;
  /** Accumulated stderr output */
  stderr: string;
  /** The underlying child process */
  process: ChildProcess;
}

/**
 * Spawn the Rust runtime binary and return a TestRuntime handle.
 */
export function spawnRuntime(): TestRuntime {
  const binaryPath = process.env.RUNTIME_BINARY_PATH;
  if (!binaryPath) {
    throw new Error(
      "RUNTIME_BINARY_PATH not set. Did globalSetup run?"
    );
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

/**
 * Build a valid plan message with sensible defaults.
 * Override any nested field via the overrides parameter.
 */
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
```

- [ ] **Step 6: Install test dependencies**

```bash
cd tests/protocol && npm install
```

- [ ] **Step 7: Verify test setup works (no tests yet, just config)**

```bash
cd tests/protocol && npx tsc --noEmit
```

Expected: No errors.

- [ ] **Step 8: Commit**

```bash
git add tests/protocol/ tests/integration/
git commit -m "feat: bootstrap protocol test scaffolding (Phase 1)"
```

---

## Task 5: Phase 2 — Frozen TS Contract Types

**Files:**
- Create: `packages/pi-sandbox-extension/src/contract.ts`

- [ ] **Step 1: Write the contract types**

Create `packages/pi-sandbox-extension/src/contract.ts`:

```typescript
/**
 * Pi Sandbox NDJSON Protocol Contract v1
 *
 * Defines all message types for the TS <-> Rust boundary.
 * The Rust side mirrors these as serde structs in contract.rs.
 *
 * FROZEN: Changes require explicit protocol version bump.
 */

import { Type, type Static } from "@sinclair/typebox";

// ============================================================================
// Protocol Version
// ============================================================================

export const PROTOCOL_VERSION = 1;

// ============================================================================
// Shared Types
// ============================================================================

export const MountSchema = Type.Object({
  type: Type.Union([
    Type.Literal("directory"),
    Type.Literal("file"),
    Type.Literal("tmpfs"),
  ]),
  source: Type.Optional(Type.String()),
  target: Type.String(),
  writable: Type.Boolean(),
});
export type Mount = Static<typeof MountSchema>;

export const NetworkModeSchema = Type.Union([
  Type.Literal("off"),
  Type.Literal("full"),
  Type.Literal("allowlist"),
]);
export type NetworkMode = Static<typeof NetworkModeSchema>;

export const NetworkConfigSchema = Type.Object({
  mode: NetworkModeSchema,
  allowlist: Type.Optional(Type.Array(Type.String())),
});
export type NetworkConfig = Static<typeof NetworkConfigSchema>;

export const ResourceLimitsSchema = Type.Object({
  maxCpuSeconds: Type.Optional(Type.Number()),
  maxMemoryBytes: Type.Optional(Type.Number()),
  maxPids: Type.Optional(Type.Number()),
  maxOutputBytes: Type.Optional(Type.Number()),
});
export type ResourceLimits = Static<typeof ResourceLimitsSchema>;

export const ManifestSchema = Type.Object({
  mounts: Type.Array(MountSchema),
  env: Type.Record(Type.String(), Type.String()),
  cwd: Type.String(),
});
export type Manifest = Static<typeof ManifestSchema>;

export const PolicySchema = Type.Object({
  namespaces: Type.Array(Type.String()),
  network: NetworkConfigSchema,
  resourceLimits: Type.Optional(ResourceLimitsSchema),
  allowedWritableTargets: Type.Array(Type.String()),
  strictWritePolicy: Type.Boolean(),
  envAllowlist: Type.Optional(Type.Array(Type.String())),
  denyCommands: Type.Optional(Type.Array(Type.String())),
});
export type Policy = Static<typeof PolicySchema>;

// ============================================================================
// TS -> Rust Messages
// ============================================================================

export const PlanPayloadSchema = Type.Object({
  version: Type.Number(),
  sessionId: Type.String(),
  executionId: Type.String(),
  requestedProfile: Type.String(),
  runtimeBaseName: Type.Optional(Type.String()),
  manifest: ManifestSchema,
  policy: PolicySchema,
  command: Type.Array(Type.String()),
});
export type PlanPayload = Static<typeof PlanPayloadSchema>;

export interface PlanMessage {
  type: "plan";
  payload: PlanPayload;
}

export const CancelPayloadSchema = Type.Object({
  reason: Type.Optional(Type.String()),
});
export type CancelPayload = Static<typeof CancelPayloadSchema>;

export interface CancelMessage {
  type: "cancel";
  payload: CancelPayload;
}

export type InboundMessage = PlanMessage | CancelMessage;

// ============================================================================
// Rust -> TS Messages
// ============================================================================

// --- Effective State ---

export const EffectiveNetworkSchema = Type.Object({
  requested: NetworkModeSchema,
  actual: Type.Union([Type.Literal("off"), Type.Literal("full")]),
  enforcement: Type.Union([
    Type.Literal("enforced"),
    Type.Literal("observed"),
    Type.Literal("none"),
  ]),
  degraded: Type.Boolean(),
});
export type EffectiveNetwork = Static<typeof EffectiveNetworkSchema>;

export const EffectiveStateSchema = Type.Object({
  network: EffectiveNetworkSchema,
  namespacesApplied: Type.Array(Type.String()),
  envApplied: Type.Array(Type.String()),
});
export type EffectiveState = Static<typeof EffectiveStateSchema>;

// --- Validation ---

export interface ValidationError {
  code: string;
  message: string;
  field?: string;
}

export interface ValidationWarning {
  code: string;
  message: string;
}

export interface ValidationPayload {
  ok: boolean;
  errors: ValidationError[];
  warnings: ValidationWarning[];
  effectiveState: EffectiveState | null;
}

export interface ValidationMessage {
  type: "validation";
  v: number;
  payload: ValidationPayload;
}

// --- Streamed Events ---

export interface StdoutEvent {
  type: "stdout";
  sequence: number;
  ts: string;
  payload: { data: string };
}

export interface StderrEvent {
  type: "stderr";
  sequence: number;
  ts: string;
  payload: { data: string };
}

export type LifecycleEventName =
  | "started"
  | "cancel_requested"
  | "killing"
  | "exited";

export interface LifecycleEvent {
  type: "lifecycle";
  sequence: number;
  ts: string;
  payload: { event: LifecycleEventName };
}

export interface NetworkEvent {
  type: "network";
  sequence: number;
  ts: string;
  payload: {
    direction: "outbound";
    host: string;
    port: number;
    protocol?: string;
  };
}

export interface WarningEvent {
  type: "warning";
  sequence: number;
  ts: string;
  payload: { code: string; message: string };
}

export type StreamEvent =
  | StdoutEvent
  | StderrEvent
  | LifecycleEvent
  | NetworkEvent
  | WarningEvent;

// --- Result ---

export type TerminalState =
  | "clean_exit"
  | "killed_on_cancel"
  | "killed_on_timeout"
  | "supervisor_crash"
  | "partial_cleanup";

export interface ObservedConnection {
  host: string;
  port: number;
  timestamp: string;
}

export interface BlockedConnection {
  host: string;
  port: number;
}

export interface ReconciliationHints {
  terminalState: TerminalState;
  workspaceModified: boolean;
  cleanupSucceeded: boolean;
}

export interface ResultPayload {
  exitCode: number | null;
  signal: string | null;
  timedOut: boolean;
  durationMs: number;
  effectiveNetwork: EffectiveNetwork;
  observedConnections: ObservedConnection[];
  wouldHaveBlocked: BlockedConnection[];
  resourcePeaks?: {
    memoryBytes?: number;
    cpuSeconds?: number;
  };
  reconciliationHints: ReconciliationHints;
}

export interface ResultMessage {
  type: "result";
  v: number;
  payload: ResultPayload;
}

// --- Union ---

export type OutboundMessage = ValidationMessage | StreamEvent | ResultMessage;

// ============================================================================
// Error & Warning Codes
// ============================================================================

export const ErrorCodes = {
  VERSION_MISMATCH: "VERSION_MISMATCH",
  RW_TARGET_NOT_ALLOWED: "RW_TARGET_NOT_ALLOWED",
  COMMAND_DENIED: "COMMAND_DENIED",
  INVALID_MOUNT: "INVALID_MOUNT",
  MISSING_REQUIRED_FIELD: "MISSING_REQUIRED_FIELD",
} as const;

export const WarningCodes = {
  ALLOWLIST_NOT_ENFORCED: "ALLOWLIST_NOT_ENFORCED",
  NAMESPACE_DEGRADED: "NAMESPACE_DEGRADED",
  RESOURCE_LIMIT_IGNORED: "RESOURCE_LIMIT_IGNORED",
} as const;
```

- [ ] **Step 2: Update index.ts to export contract**

Replace `packages/pi-sandbox-extension/src/index.ts`:

```typescript
export * from "./contract.js";
```

- [ ] **Step 3: Verify contract compiles**

```bash
cd packages/pi-sandbox-extension && npx tsc --noEmit
```

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add packages/pi-sandbox-extension/src/contract.ts packages/pi-sandbox-extension/src/index.ts
git commit -m "feat: add frozen TS contract types (Phase 2)"
```

---

## Task 6: Phase 2 — Frozen Rust Contract Types

**Files:**
- Create: `crates/pi-sandbox-runtime/src/contract.rs`
- Create: `crates/pi-sandbox-runtime/src/timestamps.rs`
- Modify: `crates/pi-sandbox-runtime/src/main.rs`

- [ ] **Step 1: Create timestamps.rs**

Create `crates/pi-sandbox-runtime/src/timestamps.rs`:

```rust
use chrono::Utc;

/// Return current UTC time as ISO 8601 string.
pub fn now_iso8601() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
```

- [ ] **Step 2: Create contract.rs**

Create `crates/pi-sandbox-runtime/src/contract.rs`:

```rust
//! Pi Sandbox NDJSON Protocol Contract v1
//!
//! Serde structs mirroring contract.ts. FROZEN: changes require protocol version bump.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

// ============================================================================
// Inbound Messages (TS -> Rust)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InboundMessage {
    Plan { payload: PlanPayload },
    Cancel { payload: CancelPayload },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPayload {
    pub version: u32,
    pub session_id: String,
    pub execution_id: String,
    pub requested_profile: String,
    pub runtime_base_name: Option<String>,
    pub manifest: Manifest,
    pub policy: Policy,
    pub command: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub mounts: Vec<Mount>,
    pub env: std::collections::HashMap<String, String>,
    pub cwd: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(rename = "type")]
    pub mount_type: String,
    pub source: Option<String>,
    pub target: String,
    pub writable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub namespaces: Vec<String>,
    pub network: NetworkConfig,
    pub resource_limits: Option<ResourceLimits>,
    pub allowed_writable_targets: Vec<String>,
    pub strict_write_policy: bool,
    pub env_allowlist: Option<Vec<String>>,
    pub deny_commands: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub mode: String,
    pub allowlist: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLimits {
    pub max_cpu_seconds: Option<f64>,
    pub max_memory_bytes: Option<u64>,
    pub max_pids: Option<u32>,
    pub max_output_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelPayload {
    pub reason: Option<String>,
}

// ============================================================================
// Outbound Messages (Rust -> TS)
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum OutboundMessage {
    Validation(ValidationEnvelope),
    Stdout(StdoutEnvelope),
    Stderr(StderrEnvelope),
    Lifecycle(LifecycleEnvelope),
    Network(NetworkEnvelope),
    Warning(WarningEnvelope),
    Result(ResultEnvelope),
}

// --- Validation ---

#[derive(Debug, Serialize)]
pub struct ValidationEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub v: u32,
    pub payload: ValidationPayload,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ValidationPayload {
    pub ok: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub effective_state: Option<EffectiveState>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ValidationWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveState {
    pub network: EffectiveNetwork,
    pub namespaces_applied: Vec<String>,
    pub env_applied: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveNetwork {
    pub requested: String,
    pub actual: String,
    pub enforcement: String,
    pub degraded: bool,
}

// --- Streamed Events ---

#[derive(Debug, Serialize)]
pub struct StdoutEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: DataPayload,
}

#[derive(Debug, Serialize)]
pub struct StderrEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: DataPayload,
}

#[derive(Debug, Serialize)]
pub struct DataPayload {
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct LifecycleEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: LifecyclePayload,
}

#[derive(Debug, Serialize)]
pub struct LifecyclePayload {
    pub event: String,
}

#[derive(Debug, Serialize)]
pub struct NetworkEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: NetworkEventPayload,
}

#[derive(Debug, Serialize)]
pub struct NetworkEventPayload {
    pub direction: String,
    pub host: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WarningEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: WarningPayload,
}

#[derive(Debug, Serialize)]
pub struct WarningPayload {
    pub code: String,
    pub message: String,
}

// --- Result ---

#[derive(Debug, Serialize)]
pub struct ResultEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub v: u32,
    pub payload: ResultPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultPayload {
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub timed_out: bool,
    pub duration_ms: f64,
    pub effective_network: EffectiveNetwork,
    pub observed_connections: Vec<ObservedConnection>,
    pub would_have_blocked: Vec<BlockedConnection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_peaks: Option<ResourcePeaks>,
    pub reconciliation_hints: ReconciliationHints,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedConnection {
    pub host: String,
    pub port: u16,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedConnection {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePeaks {
    pub memory_bytes: Option<u64>,
    pub cpu_seconds: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationHints {
    pub terminal_state: String,
    pub workspace_modified: bool,
    pub cleanup_succeeded: bool,
}

// ============================================================================
// Constructors
// ============================================================================

impl ValidationEnvelope {
    pub fn new(payload: ValidationPayload) -> OutboundMessage {
        OutboundMessage::Validation(Self {
            msg_type: "validation",
            v: PROTOCOL_VERSION,
            payload,
        })
    }
}

impl StdoutEnvelope {
    pub fn new(sequence: u64, ts: String, data: String) -> OutboundMessage {
        OutboundMessage::Stdout(Self {
            msg_type: "stdout",
            sequence,
            ts,
            payload: DataPayload { data },
        })
    }
}

impl StderrEnvelope {
    pub fn new(sequence: u64, ts: String, data: String) -> OutboundMessage {
        OutboundMessage::Stderr(Self {
            msg_type: "stderr",
            sequence,
            ts,
            payload: DataPayload { data },
        })
    }
}

impl LifecycleEnvelope {
    pub fn new(sequence: u64, ts: String, event: &str) -> OutboundMessage {
        OutboundMessage::Lifecycle(Self {
            msg_type: "lifecycle",
            sequence,
            ts,
            payload: LifecyclePayload {
                event: event.to_string(),
            },
        })
    }
}

impl WarningEnvelope {
    pub fn new(sequence: u64, ts: String, code: &str, message: &str) -> OutboundMessage {
        OutboundMessage::Warning(Self {
            msg_type: "warning",
            sequence,
            ts,
            payload: WarningPayload {
                code: code.to_string(),
                message: message.to_string(),
            },
        })
    }
}

impl ResultEnvelope {
    pub fn new(payload: ResultPayload) -> OutboundMessage {
        OutboundMessage::Result(Self {
            msg_type: "result",
            v: PROTOCOL_VERSION,
            payload,
        })
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Write an outbound message as a single NDJSON line to stdout.
pub fn emit(message: &OutboundMessage) {
    let json = serde_json::to_string(message).expect("Failed to serialize outbound message");
    println!("{}", json);
}
```

- [ ] **Step 3: Update main.rs to use contract module**

Replace `crates/pi-sandbox-runtime/src/main.rs`:

```rust
mod contract;
mod timestamps;

use std::io::{self, BufRead};

use contract::{emit, InboundMessage, ValidationEnvelope, ValidationError, ValidationPayload};

fn main() {
    let stdin = io::stdin();
    let mut line = String::new();

    if stdin.lock().read_line(&mut line).is_err() || line.trim().is_empty() {
        eprintln!("Failed to read plan from stdin");
        std::process::exit(1);
    }

    let message: InboundMessage = match serde_json::from_str(line.trim()) {
        Ok(msg) => msg,
        Err(e) => {
            eprintln!("Failed to parse plan: {}", e);
            let validation = ValidationEnvelope::new(ValidationPayload {
                ok: false,
                errors: vec![ValidationError {
                    code: "PARSE_ERROR".to_string(),
                    message: format!("Failed to parse plan message: {}", e),
                    field: None,
                }],
                warnings: vec![],
                effective_state: None,
            });
            emit(&validation);
            return;
        }
    };

    match message {
        InboundMessage::Plan { payload } => {
            // Stub: validate version only, then exit
            if payload.version != contract::PROTOCOL_VERSION {
                let validation = ValidationEnvelope::new(ValidationPayload {
                    ok: false,
                    errors: vec![ValidationError {
                        code: "VERSION_MISMATCH".to_string(),
                        message: format!(
                            "Unsupported protocol version: {}. Expected: {}",
                            payload.version,
                            contract::PROTOCOL_VERSION
                        ),
                        field: Some("version".to_string()),
                    }],
                    warnings: vec![],
                    effective_state: None,
                });
                emit(&validation);
                return;
            }

            // For now, emit a simple success validation and exit
            let validation = ValidationEnvelope::new(ValidationPayload {
                ok: true,
                errors: vec![],
                warnings: vec![],
                effective_state: Some(contract::EffectiveState {
                    network: contract::EffectiveNetwork {
                        requested: payload.policy.network.mode.clone(),
                        actual: if payload.policy.network.mode == "off" {
                            "off".to_string()
                        } else {
                            "full".to_string()
                        },
                        enforcement: "none".to_string(),
                        degraded: false,
                    },
                    namespaces_applied: vec![],
                    env_applied: payload.manifest.env.keys().cloned().collect(),
                }),
            });
            emit(&validation);
        }
        InboundMessage::Cancel { .. } => {
            eprintln!("Received cancel before plan -- ignoring");
        }
    }
}
```

- [ ] **Step 4: Verify Rust compiles**

```bash
cd crates/pi-sandbox-runtime && cargo build
```

Expected: Compiles successfully.

- [ ] **Step 5: Test round-trip serialization**

```bash
echo '{"type":"plan","payload":{"version":1,"sessionId":"s1","executionId":"e1","requestedProfile":"build-install","manifest":{"mounts":[],"env":{"HOME":"/home"},"cwd":"/workspace"},"policy":{"namespaces":[],"network":{"mode":"full"},"allowedWritableTargets":[],"strictWritePolicy":false},"command":["echo","hi"]}}' | cargo run --manifest-path crates/pi-sandbox-runtime/Cargo.toml
```

Expected: JSON validation message with `ok: true` on stdout.

- [ ] **Step 6: Test version mismatch**

```bash
echo '{"type":"plan","payload":{"version":99,"sessionId":"s1","executionId":"e1","requestedProfile":"x","manifest":{"mounts":[],"env":{},"cwd":"/"},"policy":{"namespaces":[],"network":{"mode":"full"},"allowedWritableTargets":[],"strictWritePolicy":false},"command":["echo"]}}' | cargo run --manifest-path crates/pi-sandbox-runtime/Cargo.toml
```

Expected: JSON validation with `ok: false` and `VERSION_MISMATCH`.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-sandbox-runtime/src/
git commit -m "feat: add frozen Rust contract types and basic plan parsing (Phase 2)"
```

---

## Task 7: Phase 3 — TS Crash Synthesis

**Files:**
- Create: `packages/pi-sandbox-extension/src/crash-synthesis.ts`

- [ ] **Step 1: Write crash-synthesis.ts**

Create `packages/pi-sandbox-extension/src/crash-synthesis.ts`:

```typescript
/**
 * Crash Synthesis
 *
 * When the Rust runtime exits without emitting a "result" message,
 * the TS client synthesizes one to ensure the extension always has
 * a complete execution result.
 */

import type {
  EffectiveNetwork,
  PlanPayload,
  ResultPayload,
  ValidationPayload,
} from "./contract.js";

/**
 * Synthesize a crash result when Rust exits without emitting a result.
 *
 * Case 1: Validation was received -- preserve last-known effective state.
 *   workspaceModified = true (execution likely started)
 *
 * Case 2: No validation received -- use conservative fallback.
 *   workspaceModified = false (execution likely never started)
 */
export function synthesizeCrashResult(
  lastValidation: ValidationPayload | null,
  plan: PlanPayload,
  exitCode: number | null,
  signal: string | null,
  durationMs: number,
): ResultPayload {
  let effectiveNetwork: EffectiveNetwork;
  let workspaceModified: boolean;

  if (lastValidation?.effectiveState) {
    // Case 1: Validation received -- preserve known state
    effectiveNetwork = lastValidation.effectiveState.network;
    workspaceModified = true;
  } else {
    // Case 2: No validation -- conservative fallback
    effectiveNetwork = {
      requested: plan.policy.network.mode,
      actual: "full",
      enforcement: "none",
      degraded: true,
    };
    workspaceModified = false;
  }

  return {
    exitCode: exitCode ?? -1,
    signal,
    timedOut: false,
    durationMs,
    effectiveNetwork,
    observedConnections: [],
    wouldHaveBlocked: [],
    reconciliationHints: {
      terminalState: "supervisor_crash",
      workspaceModified,
      cleanupSucceeded: false,
    },
  };
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd packages/pi-sandbox-extension && npx tsc --noEmit
```

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/src/crash-synthesis.ts
git commit -m "feat: add crash synthesis for supervisor crash recovery (Phase 3)"
```

---

## Tasks 8-24: Remaining Implementation

Tasks 8 through 24 follow the same pattern established above. Due to the plan's length, the remaining tasks are summarized here with exact file paths and key implementation notes. Each task follows the same write-test-verify-commit cycle.

### Task 8: Phase 3 — TS Runtime Client
- **Create:** `packages/pi-sandbox-extension/src/runtime-client.ts`
- **Key:** RuntimeClient class that spawns Rust binary, writes plan NDJSON to stdin, reads NDJSON from stdout line-by-line, dispatches events, supports cancel via stdin write, crash synthesis on abnormal exit. Timeout enforced via SIGTERM then SIGKILL.
- **Update:** `packages/pi-sandbox-extension/src/index.ts` to export RuntimeClient

### Task 9: Phase 4 — Rust Validator
- **Create:** `crates/pi-sandbox-runtime/src/validator.rs`
- **Key:** `validate(plan) -> ValidationPayload`. Checks: VERSION_MISMATCH (early return, no effectiveState), RW_TARGET_NOT_ALLOWED (writable mounts vs allowedWritableTargets), COMMAND_DENIED (command vs denyCommands), empty command. Resolves effective network: off->off/enforced, full->full/none, allowlist->full/observed/degraded. Emits ALLOWLIST_NOT_ENFORCED warning.

### Task 10: Phase 4 — Rust Observer Stub
- **Create:** `crates/pi-sandbox-runtime/src/observer.rs`
- **Key:** `observe_connections()` returns empty vec. `compute_would_have_blocked(observed, allowlist)` filters connections against allowlist entries in "host:port" format.

### Task 11: Phase 4 — Rust Supervisor
- **Create:** `crates/pi-sandbox-runtime/src/supervisor.rs`
- **Modify:** `crates/pi-sandbox-runtime/Cargo.toml` (add `libc = "0.2"` under `[target.'cfg(unix)'.dependencies]`)
- **Key:** Spawns child process, streams stdout/stderr in threads using BufReader, polls cancel_rx channel, emits lifecycle events (started, cancel_requested, killing, exited). Returns SupervisionResult with exit_code, terminal_state, effective_network.

### Task 12: Phase 4 — Wire Rust main.rs
- **Modify:** `crates/pi-sandbox-runtime/src/main.rs`
- **Key:** Full pipeline: read plan line -> parse -> validate -> if ok supervise -> emit result. Cancel channel: background thread reads remaining stdin lines for cancel message. Smoke test with `echo hello` and version 99.

### Tasks 13-18: Phase 5 — Protocol Tests
- **Create:** `tests/protocol/version-mismatch.test.ts` (Task 13)
- **Create:** `tests/protocol/validation-failure.test.ts` (Task 14)
- **Create:** `tests/protocol/successful-run.test.ts` (Task 15)
- **Create:** `tests/protocol/cancel-flow.test.ts` (Task 16)
- **Create:** `tests/protocol/crash-synthesis.test.ts` (Task 17, TS-only, imports from extension src)
- **Create:** `tests/protocol/degraded-allowlist.test.ts` (Task 18)
- **Key:** Each test uses `spawnRuntime()` and `makePlan()` from helpers.ts. Tests assert exact error codes, effective state fields, sequence ordering, and terminal states. Task 18 ends with running ALL 6 tests together.

### Task 19: Phase 6 — Profiles
- **Create:** `packages/pi-sandbox-extension/src/profiles.ts`
- **Key:** 4 profiles as hardcoded map: offline-review (net off, core+git), strict (net off, core), build-install (net full, all bundles), debug-network (net full, core+git+node+python). DEFAULT_PROFILE = "build-install". Functions: getProfile(name), listProfiles().

### Task 20: Phase 6 — Runtime Base
- **Create:** `packages/pi-sandbox-extension/src/runtime-base.ts`
- **Key:** `createHostDerivedBase()` returns RuntimeBase with `resolveBundleMounts(bundles)`. Bundle specs: core (/usr/bin, /usr/lib, /lib, /lib64), certs, git/node/python/rust (dynamic via `which`). Fingerprint = sha256 of sorted paths + mtimes.

### Task 21: Phase 6 — Session Manager
- **Create:** `packages/pi-sandbox-extension/src/session-manager.ts`
- **Key:** SessionManager class. Creates session dirs (workspace, artifacts, logs, tmp, home, cache). Persists SessionRecord as session.json. Methods: create, load, list, buildMountManifest (combines session dirs + runtime base mounts + profile env), markExecutionStarted/Finished, cleanTmp, tombstone.

### Task 22: Phase 6 — Reconciler
- **Create:** `packages/pi-sandbox-extension/src/reconciler.ts`
- **Key:** `reconcileAll(sessionManager)` scans sessions. Active sessions: kill orphaned PIDs, mark recovered, clean tmp. Recovered sessions older than 7 days: tombstone. Returns list of RecoveryAction.

### Task 23: Phase 7 — Extension Tools
- **Create:** `packages/pi-sandbox-extension/src/extension.ts`
- **Key:** `createSandboxTools(sessionManager, runtimeBase, binaryPath)` returns 5 ToolDefinition objects. sandbox_run: resolves session/profile, builds manifest+plan, spawns via RuntimeClient, streams events, formats result. sandbox_read/write/list_files: path traversal protection via safeResolvePath. sandbox_session_info: lists or describes sessions.

### Task 24: Phase 7 — Extension Entry Point
- **Modify:** `packages/pi-sandbox-extension/src/index.ts`
- **Key:** Default export `sandboxExtension(pi)`. Registers tools via pi.registerTool(). pi.on("session_start") runs reconciler. pi.on("session_shutdown") marks sessions idle. Exports all public types.

### Task 25: Final Verification
- Build Rust binary, run all 6 protocol tests, type-check TS. Tag `v1-protocol-passing`.

---

## Summary

| Task | Phase | What it builds |
|------|-------|---------------|
| 1 | 0 | Tag + branch |
| 2 | 1 | TS package scaffold |
| 3 | 1 | Rust crate scaffold |
| 4 | 1 | Protocol test scaffold |
| 5 | 2 | TS contract types |
| 6 | 2 | Rust contract types |
| 7 | 3 | Crash synthesis |
| 8 | 3 | Runtime client |
| 9 | 4 | Rust validator |
| 10 | 4 | Rust observer stub |
| 11 | 4 | Rust supervisor |
| 12 | 4 | Wire Rust main.rs |
| 13 | 5 | Protocol test 1 (version mismatch) |
| 14 | 5 | Protocol test 2 (validation failure) |
| 15 | 5 | Protocol test 3 (successful run) |
| 16 | 5 | Protocol test 4 (cancel flow) |
| 17 | 5 | Protocol test 5 (crash synthesis) |
| 18 | 5 | Protocol test 6 (degraded allowlist) |
| 19 | 6 | Profiles |
| 20 | 6 | Runtime base |
| 21 | 6 | Session manager |
| 22 | 6 | Reconciler |
| 23 | 7 | Extension tools (5 tools) |
| 24 | 7 | Extension entry point |
| 25 | -- | Final verification |
