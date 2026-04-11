# Nix Flake Runtime Part C Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplify the Pi extension into a thin CLI adapter, add agent runtime metadata and battlecard-style session info to the CLI, and clean up all dead code across Rust and TypeScript.

**Architecture:** The Pi extension stops managing sessions, profiles, and runtime bases directly. Instead, it shells out to `nixosandbox create/exec/status/list/destroy`. The CLI gains `--agent` and `--description` flags on `create` and a new `status` subcommand with a battlecard view. Dead code from the legacy NDJSON protocol is deleted from both Rust and TypeScript.

**Tech Stack:** Rust (clap, serde_json), TypeScript, Vitest

---

## File Structure

### Modified
| File | Responsibility |
|------|---------------|
| `crates/nixosandbox/src/cli.rs` | Add `--agent`, `--description` to Create; add Status subcommand |
| `crates/nixosandbox/src/session.rs` | Add `agent`, `description` fields to SessionMetadata |
| `crates/nixosandbox/src/main.rs` | Wire new flags, add `cmd_status` with battlecard output |
| `crates/nixosandbox/src/contract.rs` | Gut dead types — keep only what observer.rs and docker.rs need |
| `crates/nixosandbox/src/observer.rs` | Clean up unused imports |
| `crates/nixosandbox/src/plan_builder.rs` | Delete legacy `build()` and `build_with_allowlist()` |
| `crates/nixosandbox/src/docker.rs` | Delete `rewrite_plan()` and its test (only used by dead supervisor) |
| `packages/pi-sandbox-extension/src/contract.ts` | Delete inbound types, keep outbound |
| `packages/pi-sandbox-extension/src/extension.ts` | Rewrite as thin CLI adapter |
| `packages/pi-sandbox-extension/src/index.ts` | Simplify entry point |
| `packages/pi-sandbox-extension/package.json` | Remove unused dependencies |

### Deleted
| File | Reason |
|------|--------|
| `crates/nixosandbox/src/supervisor.rs` | Legacy NDJSON supervisor — entirely dead |
| `crates/nixosandbox/src/validator.rs` | Legacy plan validator — entirely dead |
| `packages/pi-sandbox-extension/src/session-manager.ts` | CLI owns sessions |
| `packages/pi-sandbox-extension/src/runtime-base.ts` | Nix flake profiles replace this |
| `packages/pi-sandbox-extension/src/profiles.ts` | CLI handles --profile |
| `packages/pi-sandbox-extension/src/reconciler.ts` | Single-shot CLI, nothing to reconcile |
| `packages/pi-sandbox-extension/src/runtime-client.ts` | Replaced by cli-client.ts |

### Created
| File | Responsibility |
|------|---------------|
| `packages/pi-sandbox-extension/src/cli-client.ts` | Thin wrappers for nixosandbox CLI invocations |

### Untouched
| File | Reason |
|------|--------|
| `flake.nix`, `nix/` | Nix flake and profiles |
| `docker/nixosandbox-sidecar.Dockerfile` | Docker sidecar |
| `packages/pi-sandbox-extension/src/browser.ts` | Independent of CLI adapter |
| `packages/pi-sandbox-extension/src/crash-synthesis.ts` | TS-only, kept |
| `tests/integration/` | Part B integration tests |

---

### Task 1: Add agent and description fields to SessionMetadata

**Files:**
- Modify: `crates/nixosandbox/src/session.rs:5-16,69-78,199-209`

- [ ] **Step 1: Add the two new fields to SessionMetadata**

In `crates/nixosandbox/src/session.rs`, add `agent` and `description` after `pid`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub session_id: String,
    pub name: String,
    pub profile: String,
    pub rootfs_path: String,
    pub workspace: String,
    pub created_at: String,
    pub last_exec_at: Option<String>,
    pub pid: Option<u32>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}
```

The `#[serde(default)]` ensures existing metadata.json files without these fields deserialize cleanly as `None`.

- [ ] **Step 2: Update create_session to accept the new fields**

Replace the `create_session` function signature and the metadata construction:

```rust
pub fn create_session(
    name: &str, profile: &str, rootfs_path: &str, workspace: Option<&str>,
    agent: Option<&str>, description: Option<&str>,
) -> Result<SessionMetadata, String> {
```

And update the `SessionMetadata` construction inside the function:

```rust
    let metadata = SessionMetadata {
        session_id: session_id.clone(),
        name: name.to_string(),
        profile: profile.to_string(),
        rootfs_path: rootfs_path.to_string(),
        workspace: workspace_path,
        created_at: crate::timestamps::now_iso8601(),
        last_exec_at: None,
        pid: None,
        agent: agent.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
    };
```

- [ ] **Step 3: Update the metadata_roundtrip test**

Replace the `metadata_roundtrip` test:

```rust
    #[test]
    fn metadata_roundtrip() {
        let meta = SessionMetadata {
            session_id: "abc".to_string(), name: "test".to_string(),
            profile: "strict".to_string(), rootfs_path: "/nix/store/fake".to_string(),
            workspace: "/tmp/ws".to_string(), created_at: "2026-04-08T12:00:00Z".to_string(),
            last_exec_at: None, pid: None,
            agent: Some("claude:opus-4-6".to_string()),
            description: Some("test session".to_string()),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let de: SessionMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(de.session_id, "abc");
        assert_eq!(de.agent.as_deref(), Some("claude:opus-4-6"));
        assert_eq!(de.description.as_deref(), Some("test session"));
    }
```

- [ ] **Step 4: Add a test for backward-compatible deserialization**

Add a new test after `metadata_roundtrip`:

```rust
    #[test]
    fn metadata_deserializes_without_new_fields() {
        let json = r#"{
            "sessionId": "abc",
            "name": "test",
            "profile": "strict",
            "rootfsPath": "/nix/store/fake",
            "workspace": "/tmp/ws",
            "createdAt": "2026-04-08T12:00:00Z",
            "lastExecAt": null,
            "pid": null
        }"#;
        let de: SessionMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(de.session_id, "abc");
        assert!(de.agent.is_none());
        assert!(de.description.is_none());
    }
```

- [ ] **Step 5: Fix all call sites of create_session**

In `crates/nixosandbox/src/session.rs` tests, update all `create_session` calls to pass the new args:

Replace all test calls like:
```rust
create_session("test-session", "strict", "/nix/store/fake", None)
```

with:
```rust
create_session("test-session", "strict", "/nix/store/fake", None, None, None)
```

Do this for all 5 test calls: `create_and_list_sessions`, `load_session_by_id`, `destroy_session_removes_dir`, `destroy_nonexistent_errors` (no call), and `create_with_external_workspace`.

- [ ] **Step 6: Run tests**

Run: `cd crates/nixosandbox && cargo test -- --test-threads=1 2>&1`
Expected: All tests pass (44 tests — 42 existing + 2 new).

- [ ] **Step 7: Commit**

```bash
git add crates/nixosandbox/src/session.rs
git commit -m "feat: add agent and description fields to SessionMetadata

New optional fields stored in metadata.json:
- agent: agent runtime identifier (e.g. 'claude:opus-4-6')
- description: purpose of this sandbox session

Backward-compatible: existing sessions without these fields
deserialize cleanly via #[serde(default)]."
```

---

### Task 2: Wire --agent and --description flags into CLI

**Files:**
- Modify: `crates/nixosandbox/src/cli.rs:12-33`
- Modify: `crates/nixosandbox/src/main.rs:21,89-114`

- [ ] **Step 1: Add the two new flags to the Create command**

In `crates/nixosandbox/src/cli.rs`, add after the `name` field in the Create variant:

```rust
    /// Create a new sandbox session
    Create {
        /// Use a built-in profile
        #[arg(long)]
        profile: Option<String>,

        /// Use a custom spec file
        #[arg(long)]
        spec: Option<String>,

        /// Host directory to mount as /workspace
        #[arg(long)]
        workspace: Option<String>,

        /// Human-readable session name
        #[arg(long)]
        name: Option<String>,

        /// Agent runtime identifier (e.g. 'claude:opus-4-6')
        #[arg(long)]
        agent: Option<String>,

        /// Purpose of this sandbox session
        #[arg(long)]
        description: Option<String>,

        /// Output session info as JSON
        #[arg(long)]
        json: bool,
    },
```

- [ ] **Step 2: Update the main() match arm for Create**

In `crates/nixosandbox/src/main.rs`, update the Create match arm:

```rust
        Commands::Create { profile, spec: spec_file, workspace, name, agent, description, json } => {
            cmd_create(profile, spec_file, workspace, name, agent, description, json);
        }
```

- [ ] **Step 3: Update cmd_create to pass agent and description**

Update the `cmd_create` function signature and call:

```rust
fn cmd_create(profile: Option<String>, spec_file: Option<String>, workspace: Option<String>, name: Option<String>, agent: Option<String>, description: Option<String>, json: bool) {
    let sandbox_spec = resolve_spec(profile.clone(), spec_file);
    let rootfs_path = build_rootfs_for_spec(&sandbox_spec, &profile);

    nix::validate_rootfs(&rootfs_path).unwrap_or_else(|e| {
        eprintln!("rootfs validation failed: {e}");
        std::process::exit(1);
    });

    let session_name = name.unwrap_or_else(|| sandbox_spec.name.clone());
    let meta = session::create_session(
        &session_name,
        &sandbox_spec.name,
        &rootfs_path,
        workspace.as_deref(),
        agent.as_deref(),
        description.as_deref(),
    ).unwrap_or_else(|e| {
        eprintln!("session creation failed: {e}");
        std::process::exit(1);
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&meta).unwrap());
    } else {
        println!("{}", meta.session_id);
    }
}
```

- [ ] **Step 4: Run cargo check**

Run: `cd crates/nixosandbox && cargo check 2>&1`
Expected: Compiles without errors.

- [ ] **Step 5: Commit**

```bash
git add crates/nixosandbox/src/cli.rs crates/nixosandbox/src/main.rs
git commit -m "feat: wire --agent and --description flags into create

Both flags are optional strings passed through to session::create_session
and stored in metadata.json."
```

---

### Task 3: Add status subcommand with battlecard output

**Files:**
- Modify: `crates/nixosandbox/src/cli.rs:64-68`
- Modify: `crates/nixosandbox/src/main.rs:17-39,377-417`

- [ ] **Step 1: Add Status variant to the Commands enum**

In `crates/nixosandbox/src/cli.rs`, add after Destroy:

```rust
    /// Show detailed session status (battlecard)
    Status {
        /// Session ID
        session_id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
```

- [ ] **Step 2: Add match arm in main()**

In `crates/nixosandbox/src/main.rs`, add in the match block:

```rust
        Commands::Status { session_id, json } => {
            cmd_status(&session_id, json);
        }
```

- [ ] **Step 3: Implement cmd_status**

Add after `cmd_build` in `crates/nixosandbox/src/main.rs`:

```rust
fn cmd_status(session_id: &str, json: bool) {
    let meta = session::load_session(session_id).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    // Derive isolation backend
    let isolation = match bubblewrap::detect() {
        bubblewrap::BwrapAvailability::Available { .. } => "native",
        bubblewrap::BwrapAvailability::DockerAvailable { .. } => "docker",
        bubblewrap::BwrapAvailability::Unavailable { .. } => "unavailable",
    };

    // Derive network mode from profile spec
    let network = {
        let flake_root = nix::find_flake_root().ok();
        if let Some(ref root) = flake_root {
            spec::load_profile(&meta.profile, root)
                .map(|s| s.network.clone())
                .unwrap_or_else(|_| "unknown".to_string())
        } else {
            "unknown".to_string()
        }
    };

    if json {
        let status = serde_json::json!({
            "sessionId": meta.session_id,
            "name": meta.name,
            "profile": meta.profile,
            "rootfsPath": meta.rootfs_path,
            "workspace": meta.workspace,
            "createdAt": meta.created_at,
            "lastExecAt": meta.last_exec_at,
            "agent": meta.agent,
            "description": meta.description,
            "isolation": isolation,
            "network": network,
        });
        println!("{}", serde_json::to_string_pretty(&status).unwrap());
    } else {
        let truncate = |s: &str, max: usize| -> String {
            if s.len() > max { format!("{}...", &s[..max-3]) } else { s.to_string() }
        };

        let desc = meta.description.as_deref().unwrap_or("-");
        let agent = meta.agent.as_deref().unwrap_or("-");
        let last_exec = meta.last_exec_at.as_deref().unwrap_or("-");
        let rootfs_display = truncate(&meta.rootfs_path, 36);
        let workspace_display = truncate(&meta.workspace, 36);

        let w = 48;
        println!("╭{}╮", "─".repeat(w));
        println!("│ {:<width$} │", format!("Session: {}", meta.session_id), width = w - 2);
        println!("├{}┤", "─".repeat(w));
        println!("│ {:<13}{:<width$} │", "Name:", meta.name, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Description:", truncate(desc, w - 15), width = w - 15);
        println!("│ {:<13}{:<width$} │", "Agent:", agent, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Profile:", meta.profile, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Created:", meta.created_at, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Last Exec:", last_exec, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Rootfs:", rootfs_display, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Workspace:", workspace_display, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Network:", network, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Isolation:", isolation, width = w - 15);
        println!("╰{}╯", "─".repeat(w));
    }
}
```

- [ ] **Step 4: Run cargo check**

Run: `cd crates/nixosandbox && cargo check 2>&1`
Expected: Compiles without errors.

- [ ] **Step 5: Commit**

```bash
git add crates/nixosandbox/src/cli.rs crates/nixosandbox/src/main.rs
git commit -m "feat: add status subcommand with battlecard output

nixosandbox status <id> shows a box-drawn session card with:
name, description, agent, profile, timestamps, rootfs, workspace,
network mode, and isolation backend.

--json flag returns structured JSON with the same fields."
```

---

### Task 4: Delete supervisor.rs and validator.rs

**Files:**
- Delete: `crates/nixosandbox/src/supervisor.rs`
- Delete: `crates/nixosandbox/src/validator.rs`
- Modify: `crates/nixosandbox/src/main.rs:1-12`

- [ ] **Step 1: Delete the two files**

```bash
rm crates/nixosandbox/src/supervisor.rs crates/nixosandbox/src/validator.rs
```

- [ ] **Step 2: Remove mod declarations from main.rs**

In `crates/nixosandbox/src/main.rs`, delete these two lines:

```rust
mod supervisor;
```

and:

```rust
mod validator;
```

- [ ] **Step 3: Run cargo check**

Run: `cd crates/nixosandbox && cargo check 2>&1`
Expected: Compiles. Some dead code warnings remain (for contract.rs types still referenced by other dead code).

- [ ] **Step 4: Commit**

```bash
git add crates/nixosandbox/src/supervisor.rs crates/nixosandbox/src/validator.rs crates/nixosandbox/src/main.rs
git commit -m "chore: delete supervisor.rs and validator.rs

Both modules were entirely dead code after the legacy NDJSON
protocol was removed in Part B. supervisor.rs handled process
supervision for the legacy protocol. validator.rs validated
PlanPayload messages."
```

---

### Task 5: Clean up contract.rs — delete dead outbound types

**Files:**
- Modify: `crates/nixosandbox/src/contract.rs`

After deleting supervisor.rs and validator.rs, the only remaining consumers of contract.rs types are:
- `docker.rs` — uses `PlanPayload`, `Manifest`, `Mount`, `NetworkConfig`, `Policy` (for `rewrite_plan` and its test)
- `observer.rs` — uses `emit`, `BlockedConnection`, `NetworkEnvelope`, `ObservedConnection`
- `plan_builder.rs` — uses `EffectiveNetwork`, `EffectiveState`, `PlanPayload`, `ResolvedAllowlistEntry`, `Manifest`, `Mount`, `NetworkConfig`, `Policy`

However, `docker::rewrite_plan` is only called from the now-deleted `supervisor.rs`. And `plan_builder::build()` and `build_with_allowlist()` are also only called from supervisor.rs. So we can cascade the cleanup. But that's Tasks 6 and 7. For now, delete the types that have zero remaining references.

- [ ] **Step 1: Delete all outbound envelope types and emit()**

In `crates/nixosandbox/src/contract.rs`, delete everything from the outbound section comment through the end of the file. Keep only the inbound types section (PlanPayload and sub-types) since docker.rs and plan_builder.rs still reference them.

Replace the entire content of `crates/nixosandbox/src/contract.rs` with:

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Plan types (used by docker.rs::rewrite_plan and plan_builder)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub mounts: Vec<Mount>,
    pub env: HashMap<String, String>,
    pub cwd: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(rename = "type")]
    pub mount_type: String,
    pub source: Option<String>,
    pub target: String,
    pub writable: bool,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub mode: String,
    pub allowlist: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLimits {
    pub max_cpu_seconds: Option<f64>,
    pub max_memory_bytes: Option<u64>,
    pub max_pids: Option<u32>,
    pub max_output_bytes: Option<u64>,
}

// ---------------------------------------------------------------------------
// Network observation types (used by observer.rs)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedConnection {
    pub direction: String,
    pub host: String,
    pub port: u16,
    pub protocol: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedConnection {
    pub direction: String,
    pub host: String,
    pub port: u16,
    pub protocol: Option<String>,
}
```

- [ ] **Step 2: Run cargo check**

Run: `cd crates/nixosandbox && cargo check 2>&1`

This will fail because `observer.rs` still references `emit`, `NetworkEnvelope`. We'll fix that in the next step.

- [ ] **Step 3: Fix observer.rs — remove references to deleted types**

The `observer.rs` `poll_loop` function calls `emit(&NetworkEnvelope::new(...))`. Since we deleted those types, we need to update observer.rs. The observer can emit NDJSON directly like main.rs does. Replace the import line:

```rust
use crate::contract::{emit, BlockedConnection, NetworkEnvelope, ObservedConnection};
```

with:

```rust
use crate::contract::{BlockedConnection, ObservedConnection};
```

And in the `poll_loop` function (Linux-only), replace the `emit` call:

```rust
                    let s = seq.fetch_add(1, Ordering::SeqCst);
                    emit(&NetworkEnvelope::new(
                        s,
                        "outbound".to_string(),
                        conn.host.clone(),
                        conn.port,
                        Some("tcp".to_string()),
                    ));
```

with:

```rust
                    let s = seq.fetch_add(1, Ordering::SeqCst);
                    let event = serde_json::json!({
                        "type": "network",
                        "sequence": s,
                        "ts": crate::timestamps::now_iso8601(),
                        "payload": {
                            "direction": "outbound",
                            "host": &conn.host,
                            "port": conn.port,
                            "protocol": "tcp"
                        }
                    });
                    println!("{}", event);
```

Also add the unused import cleanup — remove `HashSet` if still unused (it's used in `poll_loop`), `thread::self` (unused), and `Duration` (used in `poll_loop`). The `#[cfg(target_os = "linux")]` blocks use `HashSet`, `thread`, and `Duration`, so on non-Linux they appear unused. Add conditional imports:

Replace the top-level imports:

```rust
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::contract::{emit, BlockedConnection, NetworkEnvelope, ObservedConnection};
```

with:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::contract::{BlockedConnection, ObservedConnection};

#[cfg(target_os = "linux")]
use std::sync::atomic::AtomicU64;
#[cfg(target_os = "linux")]
use std::thread::JoinHandle;
```

And update the `NetworkObserver` struct to use conditional types:

```rust
pub struct NetworkObserver {
    #[cfg(target_os = "linux")]
    handle: Option<JoinHandle<Vec<ObservedConnection>>>,
    #[cfg(not(target_os = "linux"))]
    handle: Option<()>,
    stop_flag: Arc<AtomicBool>,
}
```

Actually, this refactoring is getting complex. Let's keep it simpler — just conditionally compile the whole observer as a no-op on non-Linux and keep the imports cleaner. Replace the entire file:

Replace the full content of `crates/nixosandbox/src/observer.rs` with:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::contract::{BlockedConnection, ObservedConnection};

/// Background network observer that polls /proc/net/tcp for outbound connections.
///
/// On Linux: polls at ~500ms intervals, deduplicates, emits network events.
/// On non-Linux: no-op (returns empty results immediately).
pub struct NetworkObserver {
    #[cfg(target_os = "linux")]
    handle: Option<std::thread::JoinHandle<Vec<ObservedConnection>>>,
    stop_flag: Arc<AtomicBool>,
}

impl NetworkObserver {
    /// Start the observer. On Linux, spawns a polling thread.
    /// On non-Linux, returns a no-op observer.
    #[cfg(target_os = "linux")]
    pub fn start(seq: Arc<std::sync::atomic::AtomicU64>) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop_flag);
        let handle = std::thread::spawn(move || poll_loop(flag, seq));
        NetworkObserver {
            handle: Some(handle),
            stop_flag,
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn start(_seq: Arc<std::sync::atomic::AtomicU64>) -> Self {
        NetworkObserver {
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Stop the observer and return all observed connections.
    pub fn stop(self) -> Vec<ObservedConnection> {
        self.stop_flag.store(true, Ordering::Relaxed);
        #[cfg(target_os = "linux")]
        {
            match self.handle {
                Some(h) => h.join().unwrap_or_default(),
                None => vec![],
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            vec![]
        }
    }
}

/// The polling loop (Linux only).
#[cfg(target_os = "linux")]
fn poll_loop(
    stop_flag: Arc<AtomicBool>,
    seq: Arc<std::sync::atomic::AtomicU64>,
) -> Vec<ObservedConnection> {
    use std::collections::HashSet;
    use std::io::{BufRead, BufReader};
    use std::sync::atomic::Ordering as Ord;
    use std::time::Duration;

    let mut seen: HashSet<(String, u16)> = HashSet::new();
    let mut results: Vec<ObservedConnection> = Vec::new();

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        if let Ok(connections) = parse_proc_net_tcp("/proc/net/tcp") {
            for conn in connections {
                if seen.insert((conn.host.clone(), conn.port)) {
                    let s = seq.fetch_add(1, Ord::SeqCst);
                    let event = serde_json::json!({
                        "type": "network",
                        "sequence": s,
                        "ts": crate::timestamps::now_iso8601(),
                        "payload": {
                            "direction": "outbound",
                            "host": &conn.host,
                            "port": conn.port,
                            "protocol": "tcp"
                        }
                    });
                    println!("{}", event);
                    results.push(conn);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    results
}

/// Parse /proc/net/tcp and return outbound established connections.
#[cfg(target_os = "linux")]
fn parse_proc_net_tcp(path: &str) -> std::io::Result<Vec<ObservedConnection>> {
    use std::io::{BufRead, BufReader};
    use std::fs::File;

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut connections = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if i == 0 { continue; }

        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 { continue; }

        let state = fields[3];
        if state != "01" { continue; }

        let rem_addr = fields[2];
        let parts: Vec<&str> = rem_addr.split(':').collect();
        if parts.len() != 2 { continue; }

        let ip_hex = parts[0];
        let port_hex = parts[1];

        let ip_u32 = match u32::from_str_radix(ip_hex, 16) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let a = (ip_u32 & 0xFF) as u8;
        let b = ((ip_u32 >> 8) & 0xFF) as u8;
        let c = ((ip_u32 >> 16) & 0xFF) as u8;
        let d = ((ip_u32 >> 24) & 0xFF) as u8;

        if a == 127 || (a == 0 && b == 0 && c == 0 && d == 0) { continue; }

        let host = format!("{a}.{b}.{c}.{d}");
        let port = match u16::from_str_radix(port_hex, 16) {
            Ok(v) => v,
            Err(_) => continue,
        };

        connections.push(ObservedConnection {
            direction: "outbound".to_string(),
            host,
            port,
            protocol: Some("tcp".to_string()),
        });
    }

    Ok(connections)
}

/// Compute which observed connections would have been blocked under the given allowlist.
pub fn compute_would_have_blocked(
    observed: &[ObservedConnection],
    allowlist: &Option<Vec<String>>,
) -> Vec<BlockedConnection> {
    let Some(list) = allowlist else {
        return vec![];
    };

    observed
        .iter()
        .filter(|conn| {
            let entry = format!("{}:{}", conn.host, conn.port);
            !list.iter().any(|allowed| allowed == &entry)
        })
        .map(|conn| BlockedConnection {
            direction: conn.direction.clone(),
            host: conn.host.clone(),
            port: conn.port,
            protocol: conn.protocol.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_would_have_blocked_with_no_allowlist() {
        let observed = vec![ObservedConnection {
            direction: "outbound".to_string(),
            host: "1.2.3.4".to_string(),
            port: 443,
            protocol: Some("tcp".to_string()),
        }];
        let blocked = compute_would_have_blocked(&observed, &None);
        assert!(blocked.is_empty());
    }

    #[test]
    fn compute_would_have_blocked_with_matching_allowlist() {
        let observed = vec![ObservedConnection {
            direction: "outbound".to_string(),
            host: "1.2.3.4".to_string(),
            port: 443,
            protocol: Some("tcp".to_string()),
        }];
        let allowlist = Some(vec!["1.2.3.4:443".to_string()]);
        let blocked = compute_would_have_blocked(&observed, &allowlist);
        assert!(blocked.is_empty());
    }

    #[test]
    fn compute_would_have_blocked_with_non_matching_allowlist() {
        let observed = vec![ObservedConnection {
            direction: "outbound".to_string(),
            host: "1.2.3.4".to_string(),
            port: 443,
            protocol: Some("tcp".to_string()),
        }];
        let allowlist = Some(vec!["5.6.7.8:443".to_string()]);
        let blocked = compute_would_have_blocked(&observed, &allowlist);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].host, "1.2.3.4");
        assert_eq!(blocked[0].port, 443);
    }

    #[test]
    fn network_observer_noop_on_stop() {
        let seq = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let observer = NetworkObserver::start(seq);
        let connections = observer.stop();
        let _ = connections;
    }
}
```

- [ ] **Step 4: Run cargo check and test**

Run: `cd crates/nixosandbox && cargo check 2>&1`
Run: `cd crates/nixosandbox && cargo test -- --test-threads=1 2>&1`
Expected: Compiles and all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/nixosandbox/src/contract.rs crates/nixosandbox/src/observer.rs
git commit -m "chore: delete dead outbound types from contract.rs

Remove OutboundMessage enum, all envelope types (Stdout, Stderr,
Lifecycle, Network, Warning, Result), ValidationPayload and related
types, EffectiveState, emit() function, and PROTOCOL_VERSION constant.

Keep only PlanPayload sub-types (used by docker.rs, plan_builder.rs)
and network observation types (used by observer.rs).

Rewrite observer.rs to emit NDJSON directly instead of using the
deleted emit()/NetworkEnvelope types."
```

---

### Task 6: Delete legacy plan_builder functions and docker::rewrite_plan

**Files:**
- Modify: `crates/nixosandbox/src/plan_builder.rs`
- Modify: `crates/nixosandbox/src/docker.rs`

- [ ] **Step 1: Delete build() and build_with_allowlist() from plan_builder.rs**

In `crates/nixosandbox/src/plan_builder.rs`, delete:
1. The `use crate::contract::{EffectiveNetwork, EffectiveState, PlanPayload, ResolvedAllowlistEntry};` import — replace with just what `build_rootfs` and its tests need
2. The `build()` function (lines 16-92)
3. The `generate_iptables_wrapper()` function (lines 94-110)
4. The `build_with_allowlist()` function (lines 118-end of that function)
5. All tests that test the deleted functions

First, read the full file to identify `build_rootfs` and its dependencies. The `build_rootfs` function does NOT use any contract.rs types — it takes raw string arguments. So we can remove the contract import entirely.

Replace the import at line 1:

```rust
use crate::contract::{EffectiveNetwork, EffectiveState, PlanPayload, ResolvedAllowlistEntry};
```

with nothing (delete the line entirely). The `build_rootfs` function and `RootfsSessionDirs` struct don't reference contract types.

Then delete the `build()` function body, `generate_iptables_wrapper()`, and `build_with_allowlist()`. Keep only `RootfsSessionDirs`, `build_rootfs()`, and the tests for `build_rootfs`.

Delete all tests that reference `PlanPayload`, `EffectiveState`, `EffectiveNetwork`, `Manifest`, `Mount`, `NetworkConfig`, `Policy`, `ResolvedAllowlistEntry` — these are the tests for the deleted `build()` and `build_with_allowlist()` functions.

Keep these tests (they test `build_rootfs`):
- `build_rootfs_produces_pivot_root_argv`
- `build_rootfs_network_off_adds_unshare_net`

And keep the test helper `use crate::contract::{Manifest, Mount, NetworkConfig, Policy};` only if those tests use it. Since `build_rootfs` tests don't use contract types, remove that import too.

- [ ] **Step 2: Delete rewrite_plan() and its test from docker.rs**

In `crates/nixosandbox/src/docker.rs`:

Delete the `use crate::contract::PlanPayload;` import at line 3.

Delete the `rewrite_plan()` function (lines 227-247).

Delete the `rewrite_plan_rewrites_mount_sources_and_cwd` test and the `use crate::contract::{Manifest, Mount, NetworkConfig, Policy};` import inside the test module.

Keep `rewrite_path()` and its tests (still used by main.rs for session path rewriting).

- [ ] **Step 3: Run cargo check and test**

Run: `cd crates/nixosandbox && cargo check 2>&1`
Run: `cd crates/nixosandbox && cargo test -- --test-threads=1 2>&1`
Expected: Compiles and tests pass. Fewer tests now (deleted legacy test functions).

- [ ] **Step 4: Commit**

```bash
git add crates/nixosandbox/src/plan_builder.rs crates/nixosandbox/src/docker.rs
git commit -m "chore: delete legacy build(), build_with_allowlist(), rewrite_plan()

These functions were only called from the now-deleted supervisor.rs.
build_rootfs() remains as the sole bwrap argv builder.
rewrite_path() remains for session directory path rewriting."
```

---

### Task 7: Final Rust dead code pass — delete contract.rs if fully dead

**Files:**
- Modify: `crates/nixosandbox/src/contract.rs`
- Modify: `crates/nixosandbox/src/main.rs`

- [ ] **Step 1: Check if contract.rs has any remaining consumers**

After Tasks 4-6, check: does any file still import from contract.rs?

Run: `grep -r "use crate::contract" crates/nixosandbox/src/`

If only `observer.rs` uses `ObservedConnection` and `BlockedConnection`, consider whether observer is itself dead. The observer is not called from main.rs's exec path. Check:

Run: `grep -r "observer::" crates/nixosandbox/src/main.rs`

If no references, observer.rs is also dead. Delete it and contract.rs entirely.

If observer is still referenced, keep contract.rs with just the observation types.

- [ ] **Step 2: Delete dead modules based on findings**

If both contract.rs and observer.rs are dead:

```bash
rm crates/nixosandbox/src/contract.rs crates/nixosandbox/src/observer.rs
```

And remove from main.rs:
```rust
mod contract;
mod observer;
```

If only contract.rs plan types are dead (observer still used), delete just the plan types from contract.rs, keeping observation types.

- [ ] **Step 3: Remove the logs field from SessionDirs if unused**

Check: `grep -r "\.logs" crates/nixosandbox/src/`

If only `session.rs` references it (the struct definition and directory creation), and no one reads it, remove it:
- Delete `pub logs: PathBuf,` from the struct
- Delete `logs: root.join("logs"),` from `session_dirs()`
- Delete `fs::create_dir_all(&logs_dir)` and `let logs_dir` from `create_session()`

- [ ] **Step 4: Run cargo check and test**

Run: `cd crates/nixosandbox && cargo check 2>&1`
Run: `cd crates/nixosandbox && cargo test -- --test-threads=1 2>&1`
Expected: Compiles cleanly with minimal or no warnings. All tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A crates/nixosandbox/src/
git commit -m "chore: final Rust dead code cleanup

Remove remaining unreferenced modules and types.
Clean up unused struct fields."
```

---

### Task 8: Delete 5 extension modules

**Files:**
- Delete: `packages/pi-sandbox-extension/src/session-manager.ts`
- Delete: `packages/pi-sandbox-extension/src/runtime-base.ts`
- Delete: `packages/pi-sandbox-extension/src/profiles.ts`
- Delete: `packages/pi-sandbox-extension/src/reconciler.ts`
- Delete: `packages/pi-sandbox-extension/src/runtime-client.ts`

- [ ] **Step 1: Delete the 5 files**

```bash
rm packages/pi-sandbox-extension/src/session-manager.ts \
   packages/pi-sandbox-extension/src/runtime-base.ts \
   packages/pi-sandbox-extension/src/profiles.ts \
   packages/pi-sandbox-extension/src/reconciler.ts \
   packages/pi-sandbox-extension/src/runtime-client.ts
```

- [ ] **Step 2: Commit**

```bash
git add packages/pi-sandbox-extension/src/session-manager.ts \
       packages/pi-sandbox-extension/src/runtime-base.ts \
       packages/pi-sandbox-extension/src/profiles.ts \
       packages/pi-sandbox-extension/src/reconciler.ts \
       packages/pi-sandbox-extension/src/runtime-client.ts
git commit -m "chore: delete 5 extension modules replaced by CLI

Removed:
- session-manager.ts (CLI owns sessions)
- runtime-base.ts (Nix flake profiles replace host-derived bundles)
- profiles.ts (CLI handles --profile)
- reconciler.ts (single-shot CLI, nothing to reconcile)
- runtime-client.ts (replaced by cli-client.ts)"
```

---

### Task 9: Clean up contract.ts — delete inbound types

**Files:**
- Modify: `packages/pi-sandbox-extension/src/contract.ts`

- [ ] **Step 1: Delete inbound types from contract.ts**

Delete the following from `packages/pi-sandbox-extension/src/contract.ts`:

1. The `PlanPayloadSchema` and `PlanPayload` type (lines 101-111)
2. The `PlanMessage` interface (lines 113-116)
3. The `CancelPayloadSchema` and `CancelPayload` type (lines 118-121)
4. The `CancelMessage` interface (lines 123-126)
5. The `InboundMessage` type (line 128)
6. The `ManifestSchema` and `Manifest` type (lines 79-84)
7. The `PolicySchema` and `Policy` type (lines 86-95)
8. The `MountSchema` and `Mount` type (lines 46-56)
9. The `NetworkConfigSchema` and `NetworkConfig` type (lines 65-69)
10. The `ResourceLimitsSchema` and `ResourceLimits` type (lines 71-77)
11. The `NetworkModeSchema` and `NetworkMode` type (lines 58-63)

Keep:
- `PROTOCOL_VERSION`
- Error/warning code types
- All outbound types (EffectiveNetwork, EffectiveState, ValidationPayload, StreamEvent types, ResultPayload, etc.)

- [ ] **Step 2: Check if crash-synthesis.ts still compiles**

The crash-synthesis.ts imports `EffectiveNetwork`, `PlanPayload`, `ResultPayload`, `ValidationPayload`. Since we deleted `PlanPayload`, we need to update crash-synthesis.ts.

The `synthesizeCrashResult` function takes a `PlanPayload` to extract `plan.policy.network.mode` for the fallback effective network. Since the extension no longer constructs plans, update the function to take the network mode directly:

Replace the entire content of `packages/pi-sandbox-extension/src/crash-synthesis.ts` with:

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
  ResultPayload,
  ValidationPayload,
} from "./contract.js";

/**
 * Synthesize a crash result when the CLI process exits without emitting a result.
 *
 * @param lastValidation - Last validation received (if any)
 * @param requestedNetworkMode - The network mode that was requested (e.g. "off", "full")
 * @param exitCode - Process exit code
 * @param signal - Signal that killed the process (if any)
 * @param durationMs - Execution duration in milliseconds
 */
export function synthesizeCrashResult(
  lastValidation: ValidationPayload | null,
  requestedNetworkMode: string,
  exitCode: number | null,
  signal: string | null,
  durationMs: number,
): ResultPayload {
  let effectiveNetwork: EffectiveNetwork;
  let workspaceModified: boolean;

  if (lastValidation?.effectiveState) {
    effectiveNetwork = lastValidation.effectiveState.network;
    workspaceModified = true;
  } else {
    effectiveNetwork = {
      requested: requestedNetworkMode as any,
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

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/src/contract.ts packages/pi-sandbox-extension/src/crash-synthesis.ts
git commit -m "chore: delete inbound types from contract.ts

Remove PlanPayload, CancelPayload, ManifestSchema, PolicySchema,
and all inbound message types. Keep outbound types for NDJSON parsing.

Update crash-synthesis.ts to take requestedNetworkMode string
instead of full PlanPayload."
```

---

### Task 10: Create cli-client.ts

**Files:**
- Create: `packages/pi-sandbox-extension/src/cli-client.ts`

- [ ] **Step 1: Create the CLI client module**

Create `packages/pi-sandbox-extension/src/cli-client.ts`:

```typescript
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
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

export function createSession(binary: string, opts: CreateOptions): SessionMetadata {
  const args = ["create", "--json"];
  if (opts.profile) { args.push("--profile", opts.profile); }
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
```

- [ ] **Step 2: Commit**

```bash
git add packages/pi-sandbox-extension/src/cli-client.ts
git commit -m "feat: create cli-client.ts — thin CLI wrappers

Replaces session-manager.ts + runtime-client.ts with direct
nixosandbox CLI delegation: createSession, statusSession,
listSessions, destroySession, execCommand."
```

---

### Task 11: Rewrite extension.ts as thin CLI adapter

**Files:**
- Modify: `packages/pi-sandbox-extension/src/extension.ts`

- [ ] **Step 1: Replace the entire extension.ts**

Replace the full content of `packages/pi-sandbox-extension/src/extension.ts` with:

```typescript
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
        profile = "build-install",
        agent,
        description,
        timeoutMs,
      } = args as {
        command: string[];
        sessionId?: string;
        profile?: string;
        agent?: string;
        description?: string;
        timeoutMs?: number;
      };

      let sid = maybeSessionId;
      if (!sid) {
        const meta = createSession(binaryPath, { profile, agent, description });
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
    sandboxBrowser,
  ];
}
```

- [ ] **Step 2: Commit**

```bash
git add packages/pi-sandbox-extension/src/extension.ts
git commit -m "feat: rewrite extension.ts as thin CLI adapter

All tools now delegate to the nixosandbox binary via cli-client.ts:
- sandbox_run: create session + exec command via CLI
- sandbox_read/write/list_files: get workspace path from status
- sandbox_session_info: battlecard view from status/list
- sandbox_browser: unchanged (delegates to BrowserManager)

New parameters: agent and description on sandbox_run.
Removed: SessionManager, RuntimeBase, profile resolution,
RuntimeClient, reconciler — all handled by CLI."
```

---

### Task 12: Simplify index.ts and package.json

**Files:**
- Modify: `packages/pi-sandbox-extension/src/index.ts`
- Modify: `packages/pi-sandbox-extension/package.json`

- [ ] **Step 1: Replace index.ts**

Replace the entire content of `packages/pi-sandbox-extension/src/index.ts` with:

```typescript
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
  const tools = createSandboxTools(binaryPath, browserManager);
  for (const tool of tools) {
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
} from "./cli-client.js";
export {
  createSession,
  statusSession,
  listSessions,
  destroySession,
  execCommand,
} from "./cli-client.js";
```

- [ ] **Step 2: Update package.json — rename and clean up**

Replace the content of `packages/pi-sandbox-extension/package.json`:

```json
{
  "name": "@nixosandbox/extension",
  "version": "0.2.0",
  "private": true,
  "type": "module",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "scripts": {
    "build": "tsc",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@sinclair/typebox": "^0.34.0",
    "playwright-core": "^1.50.0"
  },
  "devDependencies": {
    "typescript": "^5.7.0"
  }
}
```

Changes: renamed from `@pi-sandbox/extension` to `@nixosandbox/extension`, bumped to 0.2.0, removed vitest (tests that depended on deleted modules — crash-synthesis test remains in `tests/protocol/`), removed `test` and `test:watch` scripts.

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/src/index.ts packages/pi-sandbox-extension/package.json
git commit -m "refactor: simplify index.ts and rename extension package

- Remove SessionManager, RuntimeBase, Reconciler wiring from entry point
- Default binary path now 'nixosandbox' (was 'pi-sandbox-supervisor')
- Remove session_start reconciliation (no reconciler)
- Rename package to @nixosandbox/extension v0.2.0
- Remove vitest dependency (tests live in tests/protocol/)"
```

---

### Task 13: Update protocol tests for extension changes

**Files:**
- Modify: `tests/protocol/crash-synthesis.test.ts`

- [ ] **Step 1: Update crash-synthesis test for new function signature**

The crash-synthesis.test.ts imports `synthesizeCrashResult` and `PlanPayload` from the extension. Since `PlanPayload` is deleted, update the test.

Read the current test to understand what needs changing. The test constructs a `PlanPayload` and passes it to `synthesizeCrashResult`. Update to pass `requestedNetworkMode` string instead.

Replace all calls like:
```typescript
synthesizeCrashResult(validation, plan, exitCode, signal, durationMs)
```
with:
```typescript
synthesizeCrashResult(validation, "off", exitCode, signal, durationMs)
```

where `"off"` was `plan.policy.network.mode`.

Remove the `PlanPayload` import and the plan construction.

- [ ] **Step 2: Run the crash-synthesis test**

Run: `cd tests/protocol && npx vitest run crash-synthesis.test.ts 2>&1`
Expected: All crash-synthesis tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/protocol/crash-synthesis.test.ts
git commit -m "test: update crash-synthesis test for simplified API

Replace PlanPayload argument with requestedNetworkMode string
to match the updated synthesizeCrashResult signature."
```

---

### Task 14: Final TypeScript cleanup and build verification

**Files:**
- Modify: `packages/pi-sandbox-extension/tsconfig.json` (if needed)

- [ ] **Step 1: Run TypeScript typecheck**

Run: `cd packages/pi-sandbox-extension && npx tsc --noEmit 2>&1`

Fix any type errors that arise from the deleted modules or changed signatures.

- [ ] **Step 2: Run all Rust tests**

Run: `cd crates/nixosandbox && cargo test -- --test-threads=1 2>&1`
Expected: All tests pass.

- [ ] **Step 3: Run crash-synthesis protocol test**

Run: `cd tests/protocol && npx vitest run crash-synthesis.test.ts 2>&1`
Expected: Passes.

- [ ] **Step 4: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore: final Part C cleanup — typecheck and test fixes"
```

---

## Test Gating Summary

| Suite | Location | Requires | Changed in Part C? |
|-------|----------|----------|--------------------|
| Rust unit tests | `crates/nixosandbox/` | Just Rust | Yes — new metadata tests, fewer legacy tests |
| crash-synthesis | `tests/protocol/` | Just Node.js | Yes — updated for new API |
| rootfs-pipeline | `tests/integration/` | Nix, bwrap, Linux | No |
| docker-rootfs | `tests/integration/` | Nix, Docker | No |

## Run Commands

```bash
# Rust unit tests (always)
cd crates/nixosandbox && cargo test -- --test-threads=1

# Protocol tests (just binary, no Nix)
cd tests/protocol && npx vitest run crash-synthesis.test.ts

# TypeScript typecheck
cd packages/pi-sandbox-extension && npx tsc --noEmit
```
