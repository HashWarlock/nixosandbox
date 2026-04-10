# Nix Flake Runtime — Part C Design Spec

## Overview

Part C of the nixosandbox Nix flake runtime redesign. Part A (complete) built the standalone CLI + Nix flake. Part B (complete) delivered Docker sidecar support, legacy cleanup, and integration tests. Part C simplifies the Pi extension into a thin CLI adapter with agent runtime metadata and a battlecard-style session info view, then performs a dead code cleanup pass.

## Goals

1. **Thin CLI adapter** — Rewrite the Pi extension to delegate all session management, profile resolution, and execution to the `nixosandbox` CLI. Delete modules that duplicate CLI functionality.
2. **Agent runtime metadata** — Add `--agent` and `--description` flags to `nixosandbox create` so each session records which agent platform/model is using it and why.
3. **Battlecard session info** — Enrich `sandbox_session_info` tool and `nixosandbox list/status` to return dense, operator-grade session snapshots (identity, environment, isolation backend, agent runtime, description).
4. **Dead code cleanup** — Final pass to remove unreferenced code in both the Rust crate and the TS extension, refactor any redundancy detected across the codebase.

## Out of Scope

- Execution history tracking (future)
- Network allowlist enforcement
- Browser tool changes (kept as-is)
- Nix search fallback for package resolution

---

## 1. CLI Changes

### 1.1 Agent Runtime Format

Agent runtime strings follow the format `<platform>:<model_and_config>`:

```
claude:opus-4-6
codex:gpt-4.1
pi:sonnet-4-6+tools
copilot:gpt-4.1
amp:sonnet-4-6
droid:gemini-2.5-pro
```

The platform portion is a short identifier for the agent system. The model_and_config portion is freeform — it can include model name, version, and configuration flags separated by `+`.

### 1.2 New CLI Flags on `create`

```
nixosandbox create --profile strict \
  --agent "claude:opus-4-6" \
  --description "Debugging auth middleware for PR #42" \
  --json
```

Both `--agent` and `--description` are optional strings stored in `metadata.json`.

### 1.3 SessionMetadata Changes

Add two fields to the `SessionMetadata` struct in `session.rs`:

```rust
pub struct SessionMetadata {
    pub session_id: String,
    pub name: String,
    pub profile: String,
    pub rootfs_path: String,
    pub workspace: String,
    pub created_at: String,
    pub last_exec_at: Option<String>,
    pub pid: Option<u32>,
    // New fields
    pub agent: Option<String>,        // e.g. "claude:opus-4-6"
    pub description: Option<String>,  // e.g. "Debugging auth middleware"
}
```

These are `Option<String>` — omitted when not provided. Existing sessions without these fields deserialize cleanly (serde defaults to `None`).

### 1.4 New `status` Subcommand

Add `nixosandbox status <session_id>` that outputs a battlecard-style view.

**Plain text output:**

```
╭──────────────────────────────────────────────╮
│ Session: a1b2c3d4                            │
├──────────────────────────────────────────────┤
│ Name:        auth-debug                      │
│ Description: Debugging auth middleware       │
│ Agent:       claude:opus-4-6                 │
│ Profile:     strict                          │
│ Created:     2026-04-09T14:30:00Z            │
│ Last Exec:   2026-04-09T14:35:12Z            │
│ Rootfs:      /nix/store/abc123...-strict     │
│ Workspace:   ~/.local/share/.../workspace    │
│ Network:     off                             │
│ Isolation:   native (bwrap)                  │
╰──────────────────────────────────────────────╯
```

**JSON output (`--json`):**

Returns the full `SessionMetadata` struct as JSON (same as `create --json` but for an existing session). The isolation backend is determined at exec time, not stored — the status command reports the *current* backend by running `bubblewrap::detect()`.

```json
{
  "sessionId": "a1b2c3d4",
  "name": "auth-debug",
  "profile": "strict",
  "rootfsPath": "/nix/store/abc123-sandbox-strict",
  "workspace": "/Users/me/.local/share/nixosandbox/sessions/a1b2c3d4/workspace",
  "createdAt": "2026-04-09T14:30:00Z",
  "lastExecAt": "2026-04-09T14:35:12Z",
  "agent": "claude:opus-4-6",
  "description": "Debugging auth middleware",
  "isolation": "native",
  "network": "off"
}
```

The `isolation` and `network` fields are computed at status time — not stored in metadata.json but derived for the battlecard:
- `isolation`: one of `"native"`, `"docker"`, or `"unavailable"` — from `bubblewrap::detect()`
- `network`: one of `"off"`, `"full"`, or `"allowlist"` — from loading the session's profile spec

---

## 2. Extension Simplification

### 2.1 Delete

| Module | LOC | Reason |
|--------|-----|--------|
| `session-manager.ts` | 335 | CLI owns sessions via `nixosandbox create/list/destroy` |
| `runtime-base.ts` | 179 | Nix flake profiles replace host-derived bundles |
| `profiles.ts` | 121 | CLI handles `--profile` flag |
| `reconciler.ts` | 132 | No daemon mode — single-shot CLI, nothing to reconcile |
| `runtime-client.ts` | 271 | Replaced by direct CLI spawning |

### 2.2 Keep (Modified)

| Module | Changes |
|--------|---------|
| `extension.ts` | Rewrite all tools to shell out to `nixosandbox` CLI |
| `contract.ts` | Keep outbound types (StreamEvent, ResultPayload) for NDJSON parsing. Delete inbound types (PlanPayload, CancelPayload, PlanMessage, CancelMessage, InboundMessage) and related schemas (ManifestSchema, PolicySchema, PlanPayloadSchema, CancelPayloadSchema) |
| `crash-synthesis.ts` | Keep — TS-only crash result generation, no runtime dependency |
| `browser.ts` | Keep as-is — independent of CLI adapter changes |
| `index.ts` | Simplify — remove SessionManager/RuntimeBase/Reconciler wiring |

### 2.3 Rewritten extension.ts

The extension becomes a thin adapter that shells out to the CLI:

**sandbox_run:**
```typescript
async execute(args) {
  const { command, sessionId, profile, description, agent } = args;

  // Create session if none provided
  let sid = sessionId;
  if (!sid) {
    const createArgs = ["create", "--profile", profile ?? "build-install", "--json"];
    if (agent) createArgs.push("--agent", agent);
    if (description) createArgs.push("--description", description);
    const meta = JSON.parse(execFileSync(binaryPath, createArgs, { encoding: "utf-8" }));
    sid = meta.sessionId;
  }

  // Execute command
  const result = await spawnExecJson(binaryPath, sid, command);
  return formatRunResult(result);
}
```

**sandbox_read_file / sandbox_write_file / sandbox_list_files:**
These tools still operate on the workspace directory directly (host filesystem access). They need the session's workspace path, which they get from `nixosandbox status <id> --json`.

**sandbox_session_info:**
Calls `nixosandbox status <id> --json` for single session or `nixosandbox list --json` for all sessions. Returns the battlecard view.

### 2.4 New sandbox_run Parameters

Add `agent` and `description` to the `sandbox_run` tool schema so the calling agent can self-identify:

```typescript
parameters: Type.Object({
  command: Type.Array(Type.String()),
  sessionId: Type.Optional(Type.String()),
  profile: Type.Optional(Type.String()),
  agent: Type.Optional(Type.String({ description: "Agent runtime identifier, e.g. 'claude:opus-4-6'" })),
  description: Type.Optional(Type.String({ description: "Purpose of this sandbox session" })),
  timeoutMs: Type.Optional(Type.Number()),
})
```

### 2.5 Helper Module

Create a small `cli-client.ts` module that wraps CLI invocations:

```typescript
// Thin helpers for shelling out to nixosandbox CLI
export function createSession(binary, opts): SessionMetadata { ... }
export function statusSession(binary, sessionId): StatusResponse { ... }
export function listSessions(binary): SessionMetadata[] { ... }
export function destroySession(binary, sessionId): void { ... }
export async function execCommand(binary, sessionId, command, opts): ExecResult { ... }
```

This replaces both `runtime-client.ts` (subprocess NDJSON protocol) and `session-manager.ts` (directory management) with a single module that delegates to the CLI.

---

## 3. Dead Code Cleanup

### 3.1 Rust Crate

After Parts A+B, several modules have dead code warnings:

| Module | Dead Code | Action |
|--------|-----------|--------|
| `contract.rs` | `OutboundMessage` enum, all envelope types, `emit()`, `PROTOCOL_VERSION` | Delete — exec --json uses inline `serde_json::json!()` |
| `supervisor.rs` | `SupervisionResult`, `build_docker_command`, `supervise` | Delete — legacy supervisor for NDJSON protocol |
| `validator.rs` | `validate`, `resolve_hostname`, `detect_iptables` | Delete — validation was for legacy protocol |
| `observer.rs` | Unused imports, `NetworkObserver` partially dead | Clean up unused imports; keep core observation logic if referenced |
| `plan_builder.rs` | `build()` and `build_with_allowlist()` (legacy functions) | Delete — only `build_rootfs()` is used now |
| `session.rs` | `logs` field in `SessionDirs` | Remove if unused |

### 3.2 TypeScript Extension

After deleting the 5 modules:

| Item | Action |
|------|--------|
| Inbound types in `contract.ts` | Delete PlanPayload, CancelPayload, ManifestSchema, PolicySchema, etc. |
| Re-exports in `index.ts` | Remove exports for deleted modules |
| `@sinclair/typebox` dependency | May become unused if contract.ts types are simplified |
| `package.json` scripts | Clean up any obsolete scripts |

### 3.3 Cross-Codebase Redundancy

Scan for:
- Duplicated path constants between docker.rs and session.rs
- Duplicated spec loading logic
- Any shared types that could be consolidated

---

## 4. Files Changed

### Rust Crate — Modified
- `crates/nixosandbox/src/cli.rs` — Add `--agent`, `--description` to Create; add Status subcommand
- `crates/nixosandbox/src/session.rs` — Add `agent`, `description` fields to SessionMetadata
- `crates/nixosandbox/src/main.rs` — Wire new flags, add `cmd_status`, pass agent/description to create_session

### Rust Crate — Modified (Cleanup)
- `crates/nixosandbox/src/contract.rs` — Delete all dead types
- `crates/nixosandbox/src/supervisor.rs` — Delete entirely (or gut dead functions)
- `crates/nixosandbox/src/validator.rs` — Delete entirely (or gut dead functions)
- `crates/nixosandbox/src/observer.rs` — Clean up unused imports
- `crates/nixosandbox/src/plan_builder.rs` — Delete legacy `build()` and `build_with_allowlist()`

### Extension — Deleted
- `packages/pi-sandbox-extension/src/session-manager.ts`
- `packages/pi-sandbox-extension/src/runtime-base.ts`
- `packages/pi-sandbox-extension/src/profiles.ts`
- `packages/pi-sandbox-extension/src/reconciler.ts`
- `packages/pi-sandbox-extension/src/runtime-client.ts`

### Extension — Modified
- `packages/pi-sandbox-extension/src/extension.ts` — Rewrite as thin CLI adapter
- `packages/pi-sandbox-extension/src/contract.ts` — Delete inbound types, keep outbound
- `packages/pi-sandbox-extension/src/index.ts` — Simplify entry point and re-exports
- `packages/pi-sandbox-extension/package.json` — Remove unused dependencies

### Extension — Created
- `packages/pi-sandbox-extension/src/cli-client.ts` — CLI wrapper helpers

### Untouched
- `flake.nix`, `nix/` — Nix flake and profiles
- `docker/nixosandbox-sidecar.Dockerfile`
- `packages/pi-sandbox-extension/src/browser.ts`
- `packages/pi-sandbox-extension/src/crash-synthesis.ts`
- `tests/integration/` — Integration tests from Part B
- `tests/protocol/crash-synthesis.test.ts` — TS-only test
