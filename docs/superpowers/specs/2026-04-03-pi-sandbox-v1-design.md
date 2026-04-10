# Pi Sandbox v1 Design Spec

**Date:** 2026-04-03
**Status:** Approved
**Approach:** Two-Package Flat (Approach B)

## Overview

Refactor nixosandbox from a Rust/Axum REST sandbox server into a focused local execution runtime that integrates directly with the Pi coding agent.

**Architecture:**
- Pi extension (TypeScript) for tool UX, approvals, session state, manifests, profiles, reconciliation, and orchestration.
- Rust runtime subprocess for policy interpretation, Bubblewrap argument construction, validation, execution, event streaming, network observation, and cleanup.

**Transport:** NDJSON over stdin/stdout.
**Process model:** Single-shot subprocess per execution.

## Out of Scope for v1

- Skills, factory, TEE
- Browser as a required core capability
- Long-lived worker/daemon mode
- Real allowlist network enforcement
- Nix-composed runtime bases

---

## Section 1: Repository Structure & Build Infrastructure

### Directory Layout

```text
nixosandbox/
  packages/
    pi-sandbox-extension/              # TS package (Pi extension + runtime client)
      package.json
      tsconfig.json
      vitest.config.ts
      src/
        index.ts                       # Extension entry (ExtensionFactory)
        contract.ts                    # NDJSON message types (TypeBox schemas)
        extension.ts                   # Pi tool registrations
        runtime-client.ts             # Subprocess spawn, NDJSON I/O, cancel
        crash-synthesis.ts            # Synthesize result when Rust exits uncleanly
        session-manager.ts            # Session directories, mount manifests
        runtime-base.ts               # HostDerivedBase bundle resolution
        profiles.ts                    # Profile registry
        reconciler.ts                  # Scan/recover sessions on startup

  crates/
    pi-sandbox-runtime/                # Rust crate (subprocess binary)
      Cargo.toml
      src/
        main.rs                        # stdin->plan, validate, execute, result->stdout
        contract.rs                    # Serde structs mirroring contract.ts
        plan_builder.rs               # Bubblewrap argv construction
        validator.rs                   # Policy validation, writable target checks
        supervisor.rs                  # Process supervision, signal handling
        observer.rs                    # Network observation, connection logging
        bubblewrap.rs                 # Bubblewrap binary invocation
        timestamps.rs                  # Monotonic timestamp utilities

  tests/
    protocol/                          # 6 canonical protocol tests (TS, spawns Rust binary)
      helpers.ts
      globalSetup.ts
      version-mismatch.test.ts
      validation-failure.test.ts
      successful-run.test.ts
      cancel-flow.test.ts
      crash-synthesis.test.ts
      degraded-allowlist.test.ts
    integration/                       # End-to-end with real commands (later phases)

  sandbox-rs/                          # Legacy server (untouched during migration)
  docs/
    rfc/
    architecture/
  nix/
    shell.nix
  docker-compose.yml
```

### Build Tooling

- **TS package:** vitest for testing, typescript for type checking, TypeBox for parameter schemas. Dev dependency on `@mariozechner/pi-coding-agent` for extension types.
- **Rust crate:** Standard `cargo build --release`. Standalone binary crate (no workspace with sandbox-rs). Dependencies: serde, serde_json, nix (namespace detection), chrono (timestamps). No axum, no tokio for the stub phase.
- **Protocol tests:** TS tests that build the Rust binary via vitest globalSetup, then spawn it as a subprocess per test case.
- **Legacy sandbox-rs/:** Left completely untouched. No shared dependencies.

### Key Decisions

1. No Cargo workspace. New crate is independent from sandbox-rs/.
2. Rust runtime is synchronous for the stub phase. No async runtime needed.
3. Protocol tests are the integration gate. Nothing merges until all 6 pass.

---

## Section 2: NDJSON Protocol Contract

### Envelope

Every message has a top-level envelope:

```json
{
  "type": "<message_type>",
  "v": 1,
  "sequence": 0,
  "payload": {}
}
```

Fields:
- `type` — message discriminator (always present)
- `v` — protocol version (on `validation` and `result` only)
- `sequence` — strictly increasing counter (streamed events only)
- `payload` — message body

### TS to Rust Messages

#### plan (exactly once)

```json
{
  "type": "plan",
  "payload": {
    "version": 1,
    "sessionId": "<uuid>",
    "executionId": "<uuid>",
    "requestedProfile": "build-install",
    "runtimeBaseName": "host-derived",
    "manifest": {
      "mounts": [
        {
          "type": "directory|file|tmpfs",
          "source": "/host/path",
          "target": "/sandbox/path",
          "writable": false
        }
      ],
      "env": { "HOME": "/home/sandbox" },
      "cwd": "/workspace"
    },
    "policy": {
      "namespaces": ["user", "pid", "ipc", "uts", "net", "cgroup-try"],
      "network": {
        "mode": "off|full|allowlist",
        "allowlist": ["registry.npmjs.org:443"]
      },
      "resourceLimits": {
        "maxCpuSeconds": 300,
        "maxMemoryBytes": 1073741824,
        "maxPids": 256,
        "maxOutputBytes": 10485760
      },
      "allowedWritableTargets": ["/workspace", "/tmp"],
      "strictWritePolicy": false,
      "envAllowlist": ["HOME", "PATH"],
      "denyCommands": ["rm"]
    },
    "command": ["npm", "install"]
  }
}
```

#### cancel (optional, at most once)

```json
{
  "type": "cancel",
  "payload": {
    "reason": "User cancelled"
  }
}
```

### Rust to TS Messages

#### validation (exactly once, before execution)

```json
{
  "type": "validation",
  "v": 1,
  "payload": {
    "ok": true,
    "errors": [
      { "code": "VERSION_MISMATCH", "message": "...", "field": "version" }
    ],
    "warnings": [
      { "code": "ALLOWLIST_NOT_ENFORCED", "message": "..." }
    ],
    "effectiveState": {
      "network": {
        "requested": "allowlist",
        "actual": "full",
        "enforcement": "observed",
        "degraded": true
      },
      "namespacesApplied": ["user", "pid"],
      "envApplied": ["HOME", "PATH"]
    }
  }
}
```

#### Streamed Events (zero or more, only after validation.ok = true)

All have `sequence` (strictly increasing) and `ts` (ISO 8601).

```json
{ "type": "stdout",    "sequence": 1, "ts": "...", "payload": { "data": "..." } }
{ "type": "stderr",    "sequence": 2, "ts": "...", "payload": { "data": "..." } }
{ "type": "lifecycle", "sequence": 3, "ts": "...", "payload": { "event": "started|cancel_requested|killing|exited" } }
{ "type": "network",   "sequence": 4, "ts": "...", "payload": { "direction": "outbound", "host": "...", "port": 443, "protocol": "tcp" } }
{ "type": "warning",   "sequence": 5, "ts": "...", "payload": { "code": "...", "message": "..." } }
```

#### result (exactly once, final message)

```json
{
  "type": "result",
  "v": 1,
  "payload": {
    "exitCode": 0,
    "signal": null,
    "timedOut": false,
    "durationMs": 12847,
    "effectiveNetwork": {
      "requested": "full",
      "actual": "full",
      "enforcement": "none",
      "degraded": false
    },
    "observedConnections": [
      { "host": "registry.npmjs.org", "port": 443, "timestamp": "..." }
    ],
    "wouldHaveBlocked": [],
    "resourcePeaks": {
      "memoryBytes": 52428800,
      "cpuSeconds": 2.3
    },
    "reconciliationHints": {
      "terminalState": "clean_exit",
      "workspaceModified": true,
      "cleanupSucceeded": true
    }
  }
}
```

### Validation Error Codes

| Code | Meaning |
|------|---------|
| `VERSION_MISMATCH` | Plan version not supported |
| `RW_TARGET_NOT_ALLOWED` | Writable mount outside allowedWritableTargets |
| `COMMAND_DENIED` | Command in denyCommands list |
| `INVALID_MOUNT` | Mount spec is malformed |
| `MISSING_REQUIRED_FIELD` | Required plan field missing |

### Warning Codes

| Code | Meaning |
|------|---------|
| `ALLOWLIST_NOT_ENFORCED` | Allowlist requested but degraded to full+observed |
| `NAMESPACE_DEGRADED` | Requested namespace could not be applied |
| `RESOURCE_LIMIT_IGNORED` | Resource limit requested but not enforced |

### Truthfulness Invariants

1. `effectiveState.network.actual` reflects what was actually applied, never what was requested.
2. If `requested = allowlist` and `actual = full`, then `degraded = true` and `enforcement = observed`.
3. `namespacesApplied` only lists namespaces that were successfully created.
4. `wouldHaveBlocked` is only meaningful when `degraded = true`.

### Terminal States

| State | Meaning |
|-------|---------|
| `clean_exit` | Process exited normally |
| `killed_on_cancel` | Terminated due to cancel message |
| `killed_on_timeout` | Terminated due to timeout |
| `supervisor_crash` | Rust runtime crashed (synthesized by TS) |
| `partial_cleanup` | Cleanup attempted but may be incomplete |

---

## Section 3: Runtime Client & Crash Synthesis

### runtime-client.ts

The runtime client manages the Rust subprocess lifecycle.

#### Interface

```typescript
interface RuntimeClientOptions {
  binaryPath: string;
  timeout?: number;
  onEvent?: (event: StreamEvent) => void;
}

interface ExecutionHandle {
  validation: Promise<ValidationMessage>;
  result: Promise<ResultMessage>;
  cancel(reason?: string): void;
}
```

#### Lifecycle

1. `client.execute(plan)` spawns child process.
2. Writes plan as single NDJSON line to stdin.
3. stdin stays open for potential cancel message.
4. Reads stdout line by line:
   - First line: validation message, resolves `handle.validation`.
   - If `validation.ok = false`: Rust exits, no more messages.
   - Subsequent lines: streamed events, dispatched to `onEvent`.
   - Last line: result message, resolves `handle.result`.
5. On `handle.cancel(reason)`: writes cancel NDJSON line to stdin.
6. On abnormal exit: crash synthesis takes over.

#### Key Details

- stdin is NOT closed after writing plan (cancel may follow).
- Client tracks state: `spawned -> plan_sent -> validation_received -> streaming -> result_received | crashed`.
- Timeout enforced on TS side: SIGTERM, brief wait, then SIGKILL.
- Stderr captured separately for diagnostics, not part of NDJSON protocol.

### crash-synthesis.ts

When Rust exits without emitting a result message, TS synthesizes one.

#### Case 1: Validation was received

Preserve the last-known effective state from validation:
- `effectiveNetwork` = validation's effectiveState.network
- `terminalState` = "supervisor_crash"
- `workspaceModified` = true (assume worst case: execution started)
- `cleanupSucceeded` = false

#### Case 2: No validation received

Use conservative fallback:
- `effectiveNetwork.actual` = "full" (assume worst case)
- `effectiveNetwork.enforcement` = "none"
- `effectiveNetwork.degraded` = true
- `terminalState` = "supervisor_crash"
- `workspaceModified` = false (execution likely never started)
- `cleanupSucceeded` = false

The difference in `workspaceModified` reflects whether execution likely occurred.

---

## Section 4: Session Manager, Profiles, Runtime Bases & Reconciler

### Session Directory Layout

```text
~/.local/share/pi-sandbox/sessions/<session-id>/
  workspace/       # project files, bind-mounted rw into sandbox
  artifacts/       # build outputs, logs
  logs/            # execution NDJSON transcripts
  tmp/             # sandbox tmpdir, cleaned between runs
  home/            # sandbox $HOME, persists across runs in a session
  cache/           # package manager caches (npm, pip, cargo)
```

### Session Record

Persisted as `session.json` in the session directory:

```typescript
interface SessionRecord {
  sessionId: string;
  state: "active" | "idle" | "recovered" | "tombstoned";
  createdAt: string;
  lastActiveAt: string;
  runtimeBaseName: string;
  runtimeBaseFingerprint: string;
  policyHash: string;
  activeExecution: {
    executionId: string;
    pid: number;
    startedAt: string;
    profileName: string;
  } | null;
  lastHeartbeat: string | null;
}
```

### Session Manager Responsibilities

- Create and manage session directories.
- Generate MountManifest from session + profile + runtime base.
- Track execution start/finish in session record.
- Clean tmp directories between runs.
- Tombstone old sessions.
- **Never** construct Bubblewrap arguments (that is Rust's job).
- **Never** expose arbitrary host paths to the model.

### Mount Manifest Generation

Combines session directories (writable) + runtime base paths (read-only) + profile config:

```typescript
interface MountManifest {
  mounts: Mount[];
  env: Record<string, string>;
  cwd: string;
}
```

### Profile Registry (profiles.ts)

Profiles are named policy presets. Hardcoded map for v1.

```typescript
interface Profile {
  name: string;
  description: string;
  network: { mode: "off" | "full" | "allowlist" };
  bundles: string[];
  resourceLimits?: ResourceLimits;
  allowedWritableTargets: string[];
  strictWritePolicy: boolean;
  namespaces: string[];
  envAllowlist: string[];
  denyCommands: string[];
}
```

#### v1 Profiles

| Profile | Network | Bundles | Writable Targets |
|---------|---------|---------|------------------|
| `offline-review` | off | core, git | /workspace, /tmp |
| `strict` | off | core | /workspace, /tmp |
| `build-install` | full | core, git, node, python, rust | /workspace, /home, /cache, /tmp |
| `debug-network` | full | core, git, node, python | /workspace, /home, /cache, /tmp |

Default profile: `build-install`.

### Runtime Bases (runtime-base.ts)

v1 uses only `HostDerivedBase`. Assembles read-only mounts from host filesystem based on named bundles.

```typescript
interface RuntimeBase {
  name: string;
  fingerprint: string;
  resolveBundleMounts(bundles: string[]): Mount[];
}
```

Bundle registry maps bundle names to host paths:
- `core`: /usr/bin, /usr/lib, /lib, /lib64, /etc/resolv.conf, /etc/hosts
- `certs`: /etc/ssl/certs/ca-certificates.crt
- `git`, `node`, `python`, `rust`: Dynamically resolved from `which` at session creation time.

Fingerprint = hash of all resolved paths + their mtimes.

No "mirror the host" profile. Every path is explicitly listed.

### Reconciler (reconciler.ts)

Runs once during Pi extension `session_start` event.

#### Flow

1. Scan all session.json files in the sessions directory.
2. For each session with `state = "active"`:
   a. Check if `activeExecution.pid` is still running.
   b. If running: SIGTERM, wait, SIGKILL if needed.
   c. Mark session as "recovered".
   d. Clean tmp/.
   e. Log recovery action.
3. Sessions with `state = "recovered"` older than 7 days: mark as "tombstoned".
4. Return list of recovered sessions for user notification.

#### What the reconciler does NOT do

- Does not delete workspaces (preserved by default).
- Does not re-run failed executions (agent's decision).
- Does not interpret what happened (just kills orphans and marks state).

---

## Section 5: Pi Extension Tools & Wiring

### Extension Entry Point (index.ts)

Exports an `ExtensionFactory` that Pi loads. On `session_start`:
- Initializes session manager.
- Runs reconciler, notifies user of recovered sessions.
- Registers all sandbox tools.

On `session_shutdown`:
- Marks active sessions as idle.
- Cleans tmp directories.

### Tool Definitions (extension.ts)

Five tools registered via `pi.registerTool()` with TypeBox parameter schemas.

#### sandbox_run

Primary tool. Executes a command inside a sandboxed environment.

Parameters:
- `command: string[]` — Command and arguments.
- `profile?: string` — Policy profile (default: build-install).
- `sessionId?: string` — Existing session ID (omit to create new).
- `timeout?: number` — Timeout in ms (default: 300000).

Execution flow:
1. Resolve or create session.
2. Resolve profile.
3. Build mount manifest.
4. Construct plan message.
5. Spawn Rust runtime via runtime client.
6. Stream events back via onUpdate callback.
7. On result or crash: update session record, return formatted output.

Return format to LLM:
```
[sandbox:build-install] $ npm install
--- stdout ---
added 347 packages in 12.4s
--- stderr ---
npm warn deprecated inflight@1.0.6
--- result ---
exit_code: 0
duration: 12847ms
network: full (observed, 23 connections logged)
terminal_state: clean_exit
```

Policy warnings prepended when present.

#### sandbox_read_file

Parameters: `path` (relative to workspace root), `sessionId`, `encoding?`.

Path safety: Joins relative path to workspace root, verifies resolved absolute path starts with workspace root. Rejects traversal attacks.

#### sandbox_write_file

Parameters: `path` (relative to workspace root), `content`, `sessionId`.

Same path safety as read. Creates parent directories if needed. Tracks file in extension state as modified.

#### sandbox_list_files

Parameters: `path?` (relative, default: root), `sessionId`, `recursive?`.

Standard directory listing with path traversal protection.

#### sandbox_session_info

Parameters: `sessionId?`.

If provided: returns session record, profile, workspace contents summary.
If omitted: returns list of all sessions with state and last activity.

### Extension State Persistence

Uses `pi.appendEntry()` to persist in Pi session file:

```typescript
interface SandboxExtensionState {
  sandboxSessionId: string;
  profile: string;
  workspaceRoot: string;
  readFiles: string[];
  modifiedFiles: string[];
  lastArtifacts: string[];
  recoveryStatus: "clean" | "recovered" | null;
}
```

Persisted after sandbox_run, sandbox_read_file, sandbox_write_file, and reconciler runs.

Not persisted: manifests, runtime plans, full execution logs (those go in session logs/).

### Event Streaming

During sandbox_run, streamed events map to onUpdate calls:
- stdout/stderr: accumulated, periodically flushed as text.
- lifecycle "started": notification message.
- warnings: formatted with code and message.
- network events: logged but not streamed to LLM (too noisy).

### Cancel Integration

Pi's abort signal (Ctrl+C / ctx.abort()) triggers `handle.cancel()`, which sends the cancel NDJSON message. Rust gracefully terminates and emits final result with `killed_on_cancel`.

---

## Section 6: Rust Runtime & Protocol Tests

### Rust Runtime Stub (Phase 4)

Implements full NDJSON protocol but executes commands directly without Bubblewrap.

#### main.rs Flow

1. Read one line from stdin, parse as Plan.
2. Validate the plan.
3. Write validation to stdout.
4. If validation failed: exit(0).
5. Emit lifecycle "started".
6. Spawn command as child process.
7. Stream stdout/stderr events.
8. Poll stdin for cancel (non-blocking, separate thread).
9. On child exit: emit lifecycle "exited".
10. Write result to stdout.
11. exit(0).

Synchronous core. Uses `std::process::Command` and `std::io::BufRead`. Only concurrency: a thread polling stdin for cancel.

#### contract.rs

Serde structs with `#[serde(tag = "type")]` for envelope discriminant and `#[serde(rename_all = "camelCase")]` for JSON field names. Mirrors contract.ts exactly.

#### validator.rs

Validates plan fields:
- Version check (VERSION_MISMATCH on version != 1, early return with no effectiveState).
- Writable target check (RW_TARGET_NOT_ALLOWED for mounts outside allowedWritableTargets).
- Denied command check (COMMAND_DENIED).
- Builds effective state: resolves actual network state, namespaces applied, env applied.
- Emits ALLOWLIST_NOT_ENFORCED warning when allowlist mode degrades.

Note: on VERSION_MISMATCH, effectiveState is null (Rust cannot interpret the plan). On other validation failures, effectiveState is still populated (the plan was parseable, just invalid).

#### supervisor.rs

Stub phase: runs command directly via `std::process::Command`.

Phase 8: replaces `Command::new(&plan.command[0])` with `Command::new("bwrap").args(bubblewrap_argv)`. All streaming, cancel, and result logic stays identical.

Cancel handling: separate thread with `Receiver<()>`. On cancel received, emits "cancel_requested" lifecycle, kills process tree, waits for exit.

#### observer.rs

Stub phase: returns empty connection lists.

Phase 10: will use /proc/net/tcp or ss to log outbound connections.

`compute_would_have_blocked()` works even in stub phase: filters observed connections against allowlist.

### Protocol Tests

All written in TypeScript using vitest. Spawn compiled Rust binary as subprocess.

#### Test Infrastructure

- `helpers.ts`: `spawnRuntime()` returns TestRuntime with typed NDJSON I/O. `makePlan()` returns valid default plan with override support.
- `globalSetup.ts`: Runs `cargo build --release`, sets `RUNTIME_BINARY_PATH` env var.

#### Test 1: Version Mismatch

Send plan with version 99. Assert: validation.ok = false, VERSION_MISMATCH error, Rust exits cleanly.

#### Test 2: Validation Failure

Send plan with writable mount /evil not in allowedWritableTargets. Assert: validation.ok = false, RW_TARGET_NOT_ALLOWED error, no execution.

#### Test 3: Successful Run

Run `echo hello`. Assert: validation.ok = true, "started" lifecycle event, stdout contains "hello", result.exitCode = 0, terminalState = clean_exit, sequence numbers strictly increase.

#### Test 4: Cancel Flow

Run `sleep 3600`, cancel after "started" event. Assert: "cancel_requested" lifecycle event, terminalState = killed_on_cancel.

#### Test 5: Crash Synthesis (TS-only)

Directly calls synthesizeCrashResult() with both cases:
- With validation: preserves effective network, workspaceModified = true.
- Without validation: conservative fallback, workspaceModified = false.

#### Test 6: Degraded Allowlist

Send plan with network.mode = "allowlist". Assert: ALLOWLIST_NOT_ENFORCED warning, effectiveState.network.actual = "full", degraded = true, enforcement = "observed".

---

## Migration Phases (from Handoff Document)

| Phase | Description | Gate |
|-------|-------------|------|
| 0 | Freeze and branch | Tag current state |
| 1 | Bootstrap packages | Directory layout created |
| 2 | Commit frozen contracts | contract.ts + contract.rs committed |
| 3 | Build TS runtime client | Subprocess spawn + NDJSON I/O works |
| 4 | Build Rust stub runtime | Parses plan, validates, streams, emits result |
| 5 | Protocol tests | All 6 tests pass |
| 6 | Session manager + profiles | Session dirs, manifests, profiles, reconciler skeleton |
| 7 | Pi extension v1 | 5 tools wired to session manager + runtime client |
| 8 | Bubblewrap integration | Real isolation replaces stub execution |
| 9 | Real build/install flows | Node/Python repos verified |
| 10 | Network observation | Observe-only reporting, wouldHaveBlocked |
| 11 | Deprecate legacy server | Remove skills/factory/TEE, then server |
| 12 | Phase 2 capabilities | Browser, real allowlist, Nix runtime bases |

## Engineering Rules

- Do not refactor the old server in place.
- Do not keep HTTP as the primary interface.
- Do not allow arbitrary host path access from the model.
- Do not let RuntimePlan cross the TS/Rust boundary.
- Do not fake allowlist enforcement.
- Do not start with Nix runtime bases.
- Do not bring browser into the v1 critical path.

## Definition of Done for v1

- Pi can launch sandboxed executions locally.
- Build/install works in representative repos.
- All 6 protocol tests pass.
- Session manager and reconciler work across restart/crash.
- Writable targets are enforced.
- Requested vs effective policy is surfaced honestly.
- Unrestricted network use is visible.
- Legacy skills/factory/TEE removed from main runtime path.
- Browser is clearly deferred, not half-implemented.
