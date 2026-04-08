# Docker Fallback for macOS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Docker-based bwrap sandboxing on macOS so macOS users get real kernel-level isolation via a Docker sidecar container.

**Architecture:** When macOS is detected and Docker Desktop is available, the Rust runtime starts a lightweight Debian-slim sidecar container with bwrap installed. The supervisor delegates bwrap execution via `docker exec -i <container_id> bwrap <argv...>`. Host paths are rewritten to container paths before building the bwrap command. The NDJSON protocol and plan_builder are unchanged.

**Tech Stack:** Rust (pi-sandbox-runtime crate), Docker CLI, Debian bookworm-slim, bubblewrap, TypeScript (contract types)

**Design Spec:** `docs/superpowers/specs/2026-04-08-docker-fallback-macos-design.md`

---

## File Map

### New Files

| Path | Responsibility |
|------|----------------|
| `docker/pi-sandbox-sidecar.Dockerfile` | Debian-slim image with bwrap + iptables + common tools |
| `crates/pi-sandbox-runtime/src/docker.rs` | Docker detection, sidecar lifecycle, path rewriting |
| `tests/protocol/docker-sidecar.test.ts` | Docker sidecar lifecycle and execution tests (env-gated) |

### Modified Files

| Path | Change |
|------|--------|
| `crates/pi-sandbox-runtime/src/contract.rs:19-30` | Add `Clone` to plan types |
| `crates/pi-sandbox-runtime/src/contract.rs:135-142` | Add `isolation_backend` to `EffectiveState` |
| `crates/pi-sandbox-runtime/src/plan_builder.rs:257-269` | Update test helper `make_effective_state()` |
| `packages/pi-sandbox-extension/src/contract.ts:31-38` | Add new warning codes |
| `packages/pi-sandbox-extension/src/contract.ts:149-154` | Add `isolationBackend` to EffectiveState |
| `crates/pi-sandbox-runtime/src/bubblewrap.rs:4-8` | Add `DockerAvailable` variant |
| `crates/pi-sandbox-runtime/src/bubblewrap.rs:18-24` | Update `detect()` for macOS Docker |
| `crates/pi-sandbox-runtime/src/validator.rs:49,114,236-252,266` | Treat `DockerAvailable` as `Available`, set `isolation_backend` |
| `crates/pi-sandbox-runtime/src/supervisor.rs:46-102` | Add Docker execution branch + crash recovery |
| `crates/pi-sandbox-runtime/src/main.rs:1` | Add `mod docker;` |

---

### Task 1: Create the sidecar Dockerfile

**Files:**
- Create: `docker/pi-sandbox-sidecar.Dockerfile`

- [ ] **Step 1: Create the docker directory and Dockerfile**

```dockerfile
# docker/pi-sandbox-sidecar.Dockerfile
#
# Lightweight Linux sidecar for running bwrap on macOS via Docker Desktop.
# The container provides a Linux kernel for namespace isolation;
# bwrap inside it provides per-execution sandboxing.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    bubblewrap \
    iptables \
    python3 \
    nodejs \
    git \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
```

- [ ] **Step 2: Build the image to verify it works**

Run: `docker build -t pi-sandbox-base:latest -f docker/pi-sandbox-sidecar.Dockerfile .`

Expected: Image builds successfully. Final output like `Successfully tagged pi-sandbox-base:latest`.

- [ ] **Step 3: Verify bwrap is available inside the image**

Run: `docker run --rm pi-sandbox-base:latest which bwrap`

Expected: `/usr/bin/bwrap`

- [ ] **Step 4: Commit**

```bash
git add docker/pi-sandbox-sidecar.Dockerfile
git commit -m "feat: add Docker sidecar Dockerfile for macOS bwrap fallback"
```

---

### Task 2: Rust contract — Clone derives + isolation_backend field

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/contract.rs:19-30,40-48,50-60,62-67,69-76,135-142`
- Modify: `crates/pi-sandbox-runtime/src/validator.rs:266-271`
- Modify: `crates/pi-sandbox-runtime/src/plan_builder.rs:257-269`

The `PlanPayload` and its nested types need `Clone` so the Docker path-rewriting function can clone and modify the plan. The `EffectiveState` needs a new `isolation_backend` field for observability.

- [ ] **Step 1: Add Clone derive to plan types in contract.rs**

In `crates/pi-sandbox-runtime/src/contract.rs`, update the derive macros on these structs:

```rust
// Line 19: PlanPayload
#[derive(Debug, Clone, Deserialize)]

// Line 32: Manifest
#[derive(Debug, Clone, Deserialize)]

// Line 40: Mount
#[derive(Debug, Clone, Deserialize)]

// Line 50: Policy
#[derive(Debug, Clone, Deserialize)]

// Line 62: NetworkConfig
#[derive(Debug, Clone, Deserialize)]

// Line 69: ResourceLimits
#[derive(Debug, Clone, Deserialize)]

// Line 79: CancelPayload
#[derive(Debug, Clone, Deserialize)]
```

- [ ] **Step 2: Add isolation_backend to EffectiveState in contract.rs**

In `crates/pi-sandbox-runtime/src/contract.rs`, update the `EffectiveState` struct (line 135-142):

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveState {
    pub network: EffectiveNetwork,
    pub namespaces_applied: Vec<String>,
    pub env_applied: Vec<String>,
    pub resolved_allowlist: Vec<ResolvedAllowlistEntry>,
    pub isolation_backend: String,
}
```

- [ ] **Step 3: Update validator.rs to set isolation_backend**

In `crates/pi-sandbox-runtime/src/validator.rs`, update the `EffectiveState` construction (line 266-271):

```rust
    let isolation_backend = match bwrap {
        BwrapAvailability::Available { .. } => "native".to_string(),
        BwrapAvailability::Unavailable { .. } => "none".to_string(),
    };

    let effective_state = Some(EffectiveState {
        network: effective_network,
        namespaces_applied,
        env_applied,
        resolved_allowlist,
        isolation_backend,
    });
```

- [ ] **Step 4: Update plan_builder.rs test helper**

In `crates/pi-sandbox-runtime/src/plan_builder.rs`, update `make_effective_state()` (line 257-269):

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
            isolation_backend: "native".to_string(),
        }
    }
```

- [ ] **Step 5: Run Rust tests to verify compilation and correctness**

Run: `cd crates/pi-sandbox-runtime && cargo test`

Expected: All existing tests pass. No compilation errors.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-sandbox-runtime/src/contract.rs crates/pi-sandbox-runtime/src/validator.rs crates/pi-sandbox-runtime/src/plan_builder.rs
git commit -m "feat: add Clone to plan types and isolation_backend to EffectiveState"
```

---

### Task 3: TS contract — isolationBackend + warning codes

**Files:**
- Modify: `packages/pi-sandbox-extension/src/contract.ts:31-38,149-154`

- [ ] **Step 1: Add new warning codes**

In `packages/pi-sandbox-extension/src/contract.ts`, update the `WarningCode` type (line 31-38):

```typescript
export type WarningCode =
  | "ALLOWLIST_NOT_ENFORCED"
  | "NAMESPACE_DEGRADED"
  | "RESOURCE_LIMIT_IGNORED"
  | "DNS_RESOLUTION_PARTIAL"
  | "ALLOWLIST_DNS_FAILED"
  | "ENFORCEMENT_LEAK"
  | "IPTABLES_NOT_FOUND"
  | "DOCKER_NOT_AVAILABLE"
  | "DOCKER_SIDECAR_RESTARTED";
```

- [ ] **Step 2: Add isolationBackend to EffectiveStateSchema**

In `packages/pi-sandbox-extension/src/contract.ts`, update the `EffectiveStateSchema` (line 149-154):

```typescript
export const EffectiveStateSchema = Type.Object({
  network: EffectiveNetworkSchema,
  namespacesApplied: Type.Array(Type.String()),
  envApplied: Type.Array(Type.String()),
  isolationBackend: Type.Union([
    Type.Literal("native"),
    Type.Literal("docker"),
    Type.Literal("none"),
  ]),
});
export type EffectiveState = Static<typeof EffectiveStateSchema>;
```

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/src/contract.ts
git commit -m "feat: add isolationBackend and Docker warning codes to TS contract"
```

---

### Task 4: docker.rs — path rewriting with unit tests

**Files:**
- Create: `crates/pi-sandbox-runtime/src/docker.rs`
- Modify: `crates/pi-sandbox-runtime/src/main.rs:1`

This task creates the `docker.rs` module with the pure path-rewriting functions and their unit tests. The Docker lifecycle code comes in Task 6.

- [ ] **Step 1: Write the failing tests for path rewriting**

Create `crates/pi-sandbox-runtime/src/docker.rs`:

```rust
use crate::contract::PlanPayload;

/// Rewrite a single host path to its container-side equivalent.
///
/// If the path starts with `host_prefix`, replace that prefix with `container_prefix`.
/// Otherwise return the path unchanged.
pub fn rewrite_path(path: &str, host_prefix: &str, container_prefix: &str) -> String {
    todo!()
}

/// Clone a PlanPayload and rewrite all host paths to container paths.
///
/// Rewrites:
/// - `manifest.mounts[].source` for directory/file bind mounts
/// - `manifest.cwd`
pub fn rewrite_plan(
    plan: &PlanPayload,
    host_prefix: &str,
    container_prefix: &str,
) -> PlanPayload {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Manifest, Mount, NetworkConfig, Policy};
    use std::collections::HashMap;

    #[test]
    fn rewrite_path_replaces_matching_prefix() {
        let result = rewrite_path(
            "/Users/me/.local/share/pi-sandbox/sessions/abc/workspace",
            "/Users/me/.local/share/pi-sandbox",
            "/pi-sandbox",
        );
        assert_eq!(result, "/pi-sandbox/sessions/abc/workspace");
    }

    #[test]
    fn rewrite_path_leaves_non_matching_path_unchanged() {
        let result = rewrite_path(
            "/usr/bin/python3",
            "/Users/me/.local/share/pi-sandbox",
            "/pi-sandbox",
        );
        assert_eq!(result, "/usr/bin/python3");
    }

    #[test]
    fn rewrite_path_replaces_only_first_occurrence() {
        let result = rewrite_path(
            "/data/data/nested",
            "/data",
            "/mnt",
        );
        assert_eq!(result, "/mnt/data/nested");
    }

    #[test]
    fn rewrite_plan_rewrites_mount_sources_and_cwd() {
        let plan = PlanPayload {
            version: 1,
            session_id: "test".to_string(),
            execution_id: "test".to_string(),
            requested_profile: "build-install".to_string(),
            runtime_base_name: None,
            manifest: Manifest {
                mounts: vec![
                    Mount {
                        mount_type: "directory".to_string(),
                        source: Some("/Users/me/.local/share/pi-sandbox/sessions/s1/workspace".to_string()),
                        target: "/workspace".to_string(),
                        writable: true,
                    },
                    Mount {
                        mount_type: "tmpfs".to_string(),
                        source: None,
                        target: "/tmp".to_string(),
                        writable: true,
                    },
                ],
                env: HashMap::new(),
                cwd: "/Users/me/.local/share/pi-sandbox/sessions/s1/workspace".to_string(),
            },
            policy: Policy {
                namespaces: vec![],
                network: NetworkConfig {
                    mode: "full".to_string(),
                    allowlist: None,
                },
                resource_limits: None,
                allowed_writable_targets: vec!["/workspace".to_string(), "/tmp".to_string()],
                strict_write_policy: false,
                env_allowlist: None,
                deny_commands: None,
            },
            command: vec!["echo".to_string(), "hello".to_string()],
        };

        let rewritten = rewrite_plan(
            &plan,
            "/Users/me/.local/share/pi-sandbox",
            "/pi-sandbox",
        );

        assert_eq!(
            rewritten.manifest.mounts[0].source.as_deref(),
            Some("/pi-sandbox/sessions/s1/workspace")
        );
        assert_eq!(rewritten.manifest.mounts[1].source, None);
        assert_eq!(rewritten.manifest.cwd, "/pi-sandbox/sessions/s1/workspace");
        // Original plan is unchanged
        assert_eq!(
            plan.manifest.cwd,
            "/Users/me/.local/share/pi-sandbox/sessions/s1/workspace"
        );
    }
}
```

- [ ] **Step 2: Add `mod docker;` to main.rs**

In `crates/pi-sandbox-runtime/src/main.rs`, add `docker` to the module declarations (replace lines 1-7):

```rust
mod bubblewrap;
mod contract;
mod docker;
mod observer;
mod plan_builder;
mod supervisor;
mod timestamps;
mod validator;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd crates/pi-sandbox-runtime && cargo test docker`

Expected: FAIL — `todo!()` panics.

- [ ] **Step 4: Implement rewrite_path**

In `crates/pi-sandbox-runtime/src/docker.rs`, replace the `rewrite_path` body:

```rust
pub fn rewrite_path(path: &str, host_prefix: &str, container_prefix: &str) -> String {
    if path.starts_with(host_prefix) {
        path.replacen(host_prefix, container_prefix, 1)
    } else {
        path.to_string()
    }
}
```

- [ ] **Step 5: Implement rewrite_plan**

In `crates/pi-sandbox-runtime/src/docker.rs`, replace the `rewrite_plan` body:

```rust
pub fn rewrite_plan(
    plan: &PlanPayload,
    host_prefix: &str,
    container_prefix: &str,
) -> PlanPayload {
    let mut rewritten = plan.clone();

    for mount in &mut rewritten.manifest.mounts {
        if let Some(ref mut source) = mount.source {
            *source = rewrite_path(source, host_prefix, container_prefix);
        }
    }

    rewritten.manifest.cwd = rewrite_path(
        &rewritten.manifest.cwd,
        host_prefix,
        container_prefix,
    );

    rewritten
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd crates/pi-sandbox-runtime && cargo test docker`

Expected: All 4 tests pass.

- [ ] **Step 7: Run full test suite**

Run: `cd crates/pi-sandbox-runtime && cargo test`

Expected: All tests pass (existing + new).

- [ ] **Step 8: Commit**

```bash
git add crates/pi-sandbox-runtime/src/docker.rs crates/pi-sandbox-runtime/src/main.rs
git commit -m "feat: add docker.rs with path rewriting functions and tests"
```

---

### Task 5: BwrapAvailability::DockerAvailable + validator + supervisor stub

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/bubblewrap.rs:4-8,71-82,84-96`
- Modify: `crates/pi-sandbox-runtime/src/validator.rs:114,236-252,266`
- Modify: `crates/pi-sandbox-runtime/src/supervisor.rs:46-102`

This task adds the `DockerAvailable` variant to the `BwrapAvailability` enum and updates all exhaustive match arms. The supervisor gets a temporary fallback arm (replaced with real Docker execution in Task 8). The validator treats `DockerAvailable` identically to `Available`.

- [ ] **Step 1: Add DockerAvailable variant to BwrapAvailability**

In `crates/pi-sandbox-runtime/src/bubblewrap.rs`, update the enum (line 4-8):

```rust
#[derive(Debug, Clone)]
pub enum BwrapAvailability {
    Available { path: PathBuf },
    DockerAvailable {
        container_id: String,
        host_sessions_dir: String,
        container_sessions_dir: String,
    },
    Unavailable { reason: String },
}
```

- [ ] **Step 2: Update bubblewrap.rs test for the new variant**

In `crates/pi-sandbox-runtime/src/bubblewrap.rs`, update the `detect_returns_a_result` test (line 71-82):

```rust
    #[test]
    fn detect_returns_a_result() {
        let result = detect();
        match &result {
            BwrapAvailability::Available { path } => {
                assert!(path.exists());
            }
            BwrapAvailability::DockerAvailable { container_id, .. } => {
                assert!(!container_id.is_empty());
            }
            BwrapAvailability::Unavailable { reason } => {
                assert!(!reason.is_empty());
            }
        }
    }
```

- [ ] **Step 3: Update validator.rs — bwrap_available check**

In `crates/pi-sandbox-runtime/src/validator.rs`, update line 114:

```rust
    let bwrap_available = matches!(
        bwrap,
        BwrapAvailability::Available { .. } | BwrapAvailability::DockerAvailable { .. }
    );
```

- [ ] **Step 4: Update validator.rs — namespace resolution match**

In `crates/pi-sandbox-runtime/src/validator.rs`, update the namespace match (line 236-252):

```rust
    let namespaces_applied = match bwrap {
        BwrapAvailability::Available { .. } | BwrapAvailability::DockerAvailable { .. } => {
            plan.policy.namespaces.clone()
        }
        BwrapAvailability::Unavailable { .. } => {
            for ns in &plan.policy.namespaces {
                warnings.push(ValidationWarning {
                    code: "NAMESPACE_DEGRADED".to_string(),
                    message: format!(
                        "Namespace '{}' requested but cannot be applied (bwrap unavailable)",
                        ns
                    ),
                });
            }
            vec![]
        }
    };
```

- [ ] **Step 5: Update validator.rs — isolation_backend match**

In `crates/pi-sandbox-runtime/src/validator.rs`, update the `isolation_backend` assignment (added in Task 2):

```rust
    let isolation_backend = match bwrap {
        BwrapAvailability::Available { .. } => "native".to_string(),
        BwrapAvailability::DockerAvailable { .. } => "docker".to_string(),
        BwrapAvailability::Unavailable { .. } => "none".to_string(),
    };
```

- [ ] **Step 6: Emit DOCKER_NOT_AVAILABLE warning on macOS when degrading**

In `crates/pi-sandbox-runtime/src/validator.rs`, add after the `isolation_backend` assignment and before the `EffectiveState` construction:

```rust
    // On non-Linux, if bwrap is unavailable (Docker not found), emit a warning
    #[cfg(not(target_os = "linux"))]
    if matches!(bwrap, BwrapAvailability::Unavailable { .. }) {
        warnings.push(ValidationWarning {
            code: "DOCKER_NOT_AVAILABLE".to_string(),
            message: "macOS detected but Docker not available; running without isolation"
                .to_string(),
        });
    }
```

- [ ] **Step 7: Add temporary DockerAvailable arm to supervisor.rs**

In `crates/pi-sandbox-runtime/src/supervisor.rs`, update the match (line 46-102). Add a new arm between `Available` and `Unavailable`:

```rust
        BwrapAvailability::DockerAvailable { .. } => {
            // Placeholder: Docker execution implemented in Task 8.
            // This arm is unreachable until the detection chain (Task 7)
            // returns DockerAvailable. Falls through to direct execution.
            let mut c = Command::new(&plan.command[0]);
            if plan.command.len() > 1 {
                c.args(&plan.command[1..]);
            }
            c.current_dir(&plan.manifest.cwd)
                .envs(&plan.manifest.env);
            c
        }
```

- [ ] **Step 8: Run full Rust test suite**

Run: `cd crates/pi-sandbox-runtime && cargo test`

Expected: All tests pass. No compilation errors.

- [ ] **Step 9: Commit**

```bash
git add crates/pi-sandbox-runtime/src/bubblewrap.rs crates/pi-sandbox-runtime/src/validator.rs crates/pi-sandbox-runtime/src/supervisor.rs
git commit -m "feat: add DockerAvailable variant to BwrapAvailability, update all match arms"
```

---

### Task 6: docker.rs — Docker detection and sidecar lifecycle

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/docker.rs`

This task adds the Docker CLI interaction functions: checking Docker availability, finding/starting/creating the sidecar container, and determining the pi-sandbox data directory.

- [ ] **Step 1: Add imports and constants at the top of docker.rs**

In `crates/pi-sandbox-runtime/src/docker.rs`, add at the very top (before the existing `use crate::contract::PlanPayload;` line):

```rust
use std::process::{Command, Stdio};

use crate::contract::PlanPayload;

const SIDECAR_NAME: &str = "pi-sandbox-sidecar";
const IMAGE_NAME: &str = "pi-sandbox-base:latest";
const CONTAINER_SESSIONS_DIR: &str = "/pi-sandbox";

/// Information about a running Docker sidecar container.
pub struct DockerSidecar {
    pub container_id: String,
    pub host_sessions_dir: String,
    pub container_sessions_dir: String,
}
```

(Remove the old standalone `use crate::contract::PlanPayload;` line to avoid duplication.)

- [ ] **Step 2: Add get_data_dir helper**

Add after the `DockerSidecar` struct, before `rewrite_path`:

```rust
/// Get the pi-sandbox data directory on the host.
///
/// Uses `PI_SANDBOX_DATA_DIR` env var if set, otherwise `$HOME/.local/share/pi-sandbox`.
fn get_data_dir() -> Result<String, String> {
    if let Ok(dir) = std::env::var("PI_SANDBOX_DATA_DIR") {
        return Ok(dir);
    }
    let home = std::env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    Ok(format!("{home}/.local/share/pi-sandbox"))
}
```

- [ ] **Step 3: Add is_docker_available function**

Add after `get_data_dir`:

```rust
/// Check whether Docker is available by running `docker info`.
pub fn is_docker_available() -> bool {
    Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

- [ ] **Step 4: Add sidecar discovery functions**

Add after `is_docker_available`:

```rust
/// Find a running sidecar container. Returns its short ID if found.
fn find_running_sidecar() -> Option<String> {
    let output = Command::new("docker")
        .args([
            "ps",
            "--filter", &format!("name={SIDECAR_NAME}"),
            "--format", "{{.ID}}",
        ])
        .output()
        .ok()?;
    if output.status.success() {
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if id.is_empty() { None } else { Some(id) }
    } else {
        None
    }
}

/// Find a stopped sidecar container. Returns its short ID if found.
fn find_stopped_sidecar() -> Option<String> {
    let output = Command::new("docker")
        .args([
            "ps", "-a",
            "--filter", &format!("name={SIDECAR_NAME}"),
            "--format", "{{.ID}}",
        ])
        .output()
        .ok()?;
    if output.status.success() {
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if id.is_empty() { None } else { Some(id) }
    } else {
        None
    }
}
```

- [ ] **Step 5: Add start_container and ensure_image functions**

```rust
/// Start a stopped container.
fn start_container(id: &str) -> Result<(), String> {
    let status = Command::new("docker")
        .args(["start", id])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("failed to start container: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("docker start failed".to_string())
    }
}

/// Build the sidecar Docker image if it doesn't already exist.
fn ensure_image() -> Result<(), String> {
    let output = Command::new("docker")
        .args(["images", IMAGE_NAME, "--format", "{{.ID}}"])
        .output()
        .map_err(|e| format!("docker images check failed: {e}"))?;

    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !id.is_empty() {
        return Ok(());
    }

    eprintln!("pi-sandbox: building Docker sidecar image (one-time setup)...");
    let status = Command::new("docker")
        .args([
            "build", "-t", IMAGE_NAME,
            "-f", "docker/pi-sandbox-sidecar.Dockerfile", ".",
        ])
        .status()
        .map_err(|e| format!("docker build failed: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("docker build failed with non-zero exit".to_string())
    }
}
```

- [ ] **Step 6: Add create_sidecar function**

```rust
/// Create and start a new sidecar container.
fn create_sidecar(host_sessions_dir: &str) -> Result<String, String> {
    let volume_arg = format!("{host_sessions_dir}:{CONTAINER_SESSIONS_DIR}");
    let output = Command::new("docker")
        .args([
            "run", "-d",
            "--name", SIDECAR_NAME,
            "--cap-add", "SYS_ADMIN",
            "--cap-add", "NET_ADMIN",
            "-v", &volume_arg,
            IMAGE_NAME,
            "sleep", "infinity",
        ])
        .output()
        .map_err(|e| format!("docker run failed: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("docker run failed: {stderr}"))
    }
}
```

- [ ] **Step 7: Add the top-level detect_docker_sidecar and restart_sidecar functions**

```rust
/// Detect and ensure a Docker sidecar is running.
///
/// This is the main entry point called from `bubblewrap::detect()` on macOS.
/// Returns a `DockerSidecar` with container info and path mapping,
/// or an error string explaining why Docker is not available.
pub fn detect_docker_sidecar() -> Result<DockerSidecar, String> {
    if !is_docker_available() {
        return Err("Docker not available (docker info failed)".to_string());
    }

    let host_sessions_dir = get_data_dir()?;

    // Ensure the data directory exists on the host
    std::fs::create_dir_all(&host_sessions_dir)
        .map_err(|e| format!("failed to create data dir {host_sessions_dir}: {e}"))?;

    // 1. Check if container is already running
    if let Some(id) = find_running_sidecar() {
        return Ok(DockerSidecar {
            container_id: id,
            host_sessions_dir,
            container_sessions_dir: CONTAINER_SESSIONS_DIR.to_string(),
        });
    }

    // 2. Check if container exists but is stopped
    if let Some(id) = find_stopped_sidecar() {
        start_container(&id)?;
        return Ok(DockerSidecar {
            container_id: id,
            host_sessions_dir,
            container_sessions_dir: CONTAINER_SESSIONS_DIR.to_string(),
        });
    }

    // 3. Container doesn't exist — build image and create it
    ensure_image()?;
    let id = create_sidecar(&host_sessions_dir)?;

    Ok(DockerSidecar {
        container_id: id,
        host_sessions_dir,
        container_sessions_dir: CONTAINER_SESSIONS_DIR.to_string(),
    })
}

/// Restart the sidecar container after a failure.
pub fn restart_sidecar(container_id: &str) -> Result<(), String> {
    let status = Command::new("docker")
        .args(["restart", container_id])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("docker restart failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("docker restart failed".to_string())
    }
}
```

- [ ] **Step 8: Run Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test`

Expected: All tests pass (no new tests here — lifecycle functions require Docker and are tested in Task 9).

- [ ] **Step 9: Commit**

```bash
git add crates/pi-sandbox-runtime/src/docker.rs
git commit -m "feat: add Docker detection and sidecar lifecycle management"
```

---

### Task 7: bubblewrap.rs — detection chain with Docker on macOS

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/bubblewrap.rs:19-24,84-96`

This is the critical wiring task. On non-Linux platforms, `detect()` now checks `PI_SANDBOX_NO_DOCKER` and then tries Docker before returning `Unavailable`.

- [ ] **Step 1: Update the non-Linux detection path**

In `crates/pi-sandbox-runtime/src/bubblewrap.rs`, replace the `#[cfg(not(target_os = "linux"))]` block (lines 19-24):

```rust
    #[cfg(not(target_os = "linux"))]
    {
        // Check opt-out env var
        if std::env::var("PI_SANDBOX_NO_DOCKER").map_or(false, |v| v == "1") {
            return BwrapAvailability::Unavailable {
                reason: "Docker fallback disabled via PI_SANDBOX_NO_DOCKER=1".to_string(),
            };
        }

        // Try Docker sidecar for bwrap support on macOS
        match crate::docker::detect_docker_sidecar() {
            Ok(sidecar) => {
                return BwrapAvailability::DockerAvailable {
                    container_id: sidecar.container_id,
                    host_sessions_dir: sidecar.host_sessions_dir,
                    container_sessions_dir: sidecar.container_sessions_dir,
                };
            }
            Err(reason) => {
                return BwrapAvailability::Unavailable {
                    reason: format!(
                        "Bubblewrap requires Linux; Docker fallback failed: {reason}"
                    ),
                };
            }
        }
    }
```

- [ ] **Step 2: Update the non-Linux test**

In `crates/pi-sandbox-runtime/src/bubblewrap.rs`, replace the `non_linux_always_unavailable` test (lines 84-96):

```rust
    #[test]
    #[cfg(not(target_os = "linux"))]
    fn non_linux_returns_docker_or_unavailable() {
        let result = detect();
        match result {
            BwrapAvailability::Unavailable { reason } => {
                assert!(!reason.is_empty(), "reason: {}", reason);
            }
            BwrapAvailability::DockerAvailable { container_id, .. } => {
                assert!(!container_id.is_empty());
            }
            BwrapAvailability::Available { .. } => {
                panic!("Should not return native Available on non-Linux");
            }
        }
    }
```

- [ ] **Step 3: Run Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test`

Expected: All tests pass. On macOS with Docker: `detect()` returns `DockerAvailable`. On macOS without Docker: `detect()` returns `Unavailable`. On Linux: behavior unchanged.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-sandbox-runtime/src/bubblewrap.rs
git commit -m "feat: wire Docker sidecar detection into bubblewrap detect() on macOS"
```

---

### Task 8: supervisor.rs — Docker execution branch + crash recovery

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/supervisor.rs`

This task replaces the temporary `DockerAvailable` stub with the real Docker execution path: path rewriting, `docker exec -i <container_id> bwrap`, and crash recovery with sidecar restart.

- [ ] **Step 1: Add docker import to supervisor.rs**

In `crates/pi-sandbox-runtime/src/supervisor.rs`, add to the imports (after line 14):

```rust
use crate::docker;
```

- [ ] **Step 2: Add the build_docker_command helper function**

Add this function before the `supervise` function (before line 29):

```rust
/// Build a Command for Docker-based bwrap execution.
///
/// Rewrites plan paths from host to container, builds bwrap argv via plan_builder,
/// and prefixes with `docker exec -i <container_id> bwrap`.
fn build_docker_command(
    plan: &PlanPayload,
    effective_state: &EffectiveState,
    container_id: &str,
    host_sessions_dir: &str,
    container_sessions_dir: &str,
) -> Command {
    let rewritten_plan = docker::rewrite_plan(plan, host_sessions_dir, container_sessions_dir);

    // Inside the Docker container, iptables is always at /usr/sbin/iptables
    let iptables_path = if effective_state.network.actual == "allowlist"
        && effective_state.network.enforcement == "enforced"
    {
        Some("/usr/sbin/iptables".to_string())
    } else {
        None
    };

    let argv = plan_builder::build_with_allowlist(
        &rewritten_plan,
        effective_state,
        iptables_path.as_deref(),
    );

    // If allowlist enforcement is active, write the wrapper script to the sessions dir
    // (which is mounted in the container) so bwrap can bind-mount it
    let full_argv = if effective_state.network.actual == "allowlist"
        && effective_state.network.enforcement == "enforced"
    {
        let script = plan_builder::generate_iptables_wrapper(&effective_state.resolved_allowlist);
        let host_script_dir = format!("{host_sessions_dir}/tmp");
        let host_script_path = format!("{host_script_dir}/.pi-sandbox-allowlist.sh");
        let container_script_path =
            format!("{container_sessions_dir}/tmp/.pi-sandbox-allowlist.sh");
        std::fs::create_dir_all(&host_script_dir).ok();
        std::fs::write(&host_script_path, &script).expect("failed to write iptables wrapper");

        let mut full = vec![
            "--ro-bind".to_string(),
            container_script_path,
            "/tmp/.pi-sandbox-allowlist.sh".to_string(),
        ];
        full.extend(argv);
        full
    } else {
        argv
    };

    let mut cmd = Command::new("docker");
    cmd.args(["exec", "-i", container_id, "bwrap"]);
    cmd.args(&full_argv);
    cmd
}
```

- [ ] **Step 3: Replace the DockerAvailable stub arm in supervise()**

In `crates/pi-sandbox-runtime/src/supervisor.rs`, replace the placeholder `DockerAvailable` match arm with:

```rust
        BwrapAvailability::DockerAvailable {
            ref container_id,
            ref host_sessions_dir,
            ref container_sessions_dir,
        } => build_docker_command(
            plan,
            effective_state,
            container_id,
            host_sessions_dir,
            container_sessions_dir,
        ),
```

- [ ] **Step 4: Add crash recovery to the spawn error handling**

In `crates/pi-sandbox-runtime/src/supervisor.rs`, replace the spawn error handling block (the `Err(e)` arm of `cmd.spawn()`) with:

```rust
        Err(e) => {
            // For Docker: if spawn failed, try restarting the sidecar once
            if let BwrapAvailability::DockerAvailable {
                ref container_id,
                ref host_sessions_dir,
                ref container_sessions_dir,
            } = bwrap
            {
                let seq_val = next_seq(&seq);
                emit(&WarningEnvelope::new(
                    seq_val,
                    "DOCKER_SIDECAR_RESTARTED".to_string(),
                    format!("docker exec failed ({e}), restarting sidecar"),
                ));

                if docker::restart_sidecar(container_id).is_ok() {
                    let mut retry_cmd = build_docker_command(
                        plan,
                        effective_state,
                        container_id,
                        host_sessions_dir,
                        container_sessions_dir,
                    );
                    retry_cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
                    match retry_cmd.spawn() {
                        Ok(c) => c,
                        Err(e2) => {
                            let s = next_seq(&seq);
                            emit(&LifecycleEnvelope::new(
                                s,
                                format!("spawn_failed_after_recovery: {e2}"),
                            ));
                            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
                            return SupervisionResult {
                                exit_code: None,
                                signal: None,
                                timed_out: false,
                                duration_ms,
                                effective_network: effective_state.network.clone(),
                                observed_connections: vec![],
                                would_have_blocked: vec![],
                                terminal_state: "supervisor_crash".to_string(),
                                workspace_modified: false,
                            };
                        }
                    }
                } else {
                    let s = next_seq(&seq);
                    emit(&LifecycleEnvelope::new(
                        s,
                        format!("spawn_failed: {e} (sidecar restart also failed)"),
                    ));
                    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
                    return SupervisionResult {
                        exit_code: None,
                        signal: None,
                        timed_out: false,
                        duration_ms,
                        effective_network: effective_state.network.clone(),
                        observed_connections: vec![],
                        would_have_blocked: vec![],
                        terminal_state: "supervisor_crash".to_string(),
                        workspace_modified: false,
                    };
                }
            } else {
                let seq_val = next_seq(&seq);
                emit(&LifecycleEnvelope::new(
                    seq_val,
                    format!("spawn_failed: {e}"),
                ));
                let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
                return SupervisionResult {
                    exit_code: None,
                    signal: None,
                    timed_out: false,
                    duration_ms,
                    effective_network: effective_state.network.clone(),
                    observed_connections: vec![],
                    would_have_blocked: vec![],
                    terminal_state: "supervisor_crash".to_string(),
                    workspace_modified: false,
                };
            }
        }
```

- [ ] **Step 5: Run Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test`

Expected: All tests pass. The Docker execution path compiles correctly.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-sandbox-runtime/src/supervisor.rs
git commit -m "feat: add Docker execution branch with path rewriting and crash recovery"
```

---

### Task 9: Docker integration tests

**Files:**
- Create: `tests/protocol/docker-sidecar.test.ts`

These tests are gated behind `RUN_DOCKER_TESTS=1` and require Docker Desktop to be running. They verify the full Docker sidecar flow: detection, execution with isolation, and the `PI_SANDBOX_NO_DOCKER` opt-out.

- [ ] **Step 1: Write the test file**

Create `tests/protocol/docker-sidecar.test.ts`:

```typescript
/**
 * Docker sidecar integration tests.
 *
 * These tests require Docker Desktop to be running and are gated behind
 * the RUN_DOCKER_TESTS=1 environment variable.
 *
 * Run: RUN_DOCKER_TESTS=1 npx vitest run tests/protocol/docker-sidecar.test.ts
 */
import { execFileSync } from "node:child_process";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { describe, it, expect, beforeAll } from "vitest";
import { spawnRuntime, makePlan } from "./helpers.js";

const DOCKER_TESTS = process.env.RUN_DOCKER_TESTS === "1";

describe.skipIf(!DOCKER_TESTS)("Docker sidecar", () => {
  beforeAll(() => {
    // Clean up any leftover sidecar from previous runs
    try {
      execFileSync("docker", ["rm", "-f", "pi-sandbox-sidecar"], {
        stdio: "ignore",
      });
    } catch {
      // Container didn't exist, that's fine
    }
  });

  it("runs echo through Docker+bwrap and reports isolationBackend=docker", async () => {
    const rt = spawnRuntime();
    const plan = makePlan({
      command: ["echo", "hello from docker sidecar"],
    });

    rt.send(plan);

    const validation = await rt.readline();
    expect(validation).toHaveProperty("type", "validation");

    const payload = (validation as any).payload;
    expect(payload.ok).toBe(true);
    expect(payload.effectiveState.isolationBackend).toBe("docker");

    const events = await rt.readAllEvents();
    const result = events.find((e: any) => e.type === "result") as any;
    expect(result).toBeDefined();
    expect(result.payload.exitCode).toBe(0);

    const stdout = events
      .filter((e: any) => e.type === "stdout")
      .map((e: any) => e.payload.data)
      .join("\n");
    expect(stdout).toContain("hello from docker sidecar");

    await rt.waitForExit();
  }, 60_000); // 60s timeout for first-time image build

  it("reports enforcement=enforced for network=off", async () => {
    const rt = spawnRuntime();
    const plan = makePlan({
      command: ["echo", "offline test"],
      policy: {
        namespaces: ["user", "pid", "net"],
        network: { mode: "off" },
        allowedWritableTargets: ["/workspace", "/tmp"],
        strictWritePolicy: false,
      },
    });

    rt.send(plan);

    const validation = await rt.readline();
    const payload = (validation as any).payload;
    expect(payload.ok).toBe(true);
    expect(payload.effectiveState.network.actual).toBe("off");
    expect(payload.effectiveState.network.enforcement).toBe("enforced");
    expect(payload.effectiveState.isolationBackend).toBe("docker");

    const events = await rt.readAllEvents();
    const result = events.find((e: any) => e.type === "result") as any;
    expect(result.payload.exitCode).toBe(0);

    await rt.waitForExit();
  }, 30_000);

  it("PI_SANDBOX_NO_DOCKER=1 skips Docker and degrades", async () => {
    const binaryPath = process.env.RUNTIME_BINARY_PATH;
    if (!binaryPath) throw new Error("RUNTIME_BINARY_PATH not set");

    const child = spawn(binaryPath, [], {
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env, PI_SANDBOX_NO_DOCKER: "1" },
    });

    const rl = createInterface({ input: child.stdout! });
    const lines: string[] = [];
    rl.on("line", (line: string) => lines.push(line));

    const plan = makePlan({ command: ["echo", "no docker"] });
    child.stdin!.write(JSON.stringify(plan) + "\n");

    await new Promise<void>((resolve) => child.on("exit", () => resolve()));

    const validation = JSON.parse(lines[0]);
    expect(validation.payload.effectiveState.isolationBackend).toBe("none");
  }, 15_000);
});
```

- [ ] **Step 2: Run the tests (Docker required)**

Run: `RUN_DOCKER_TESTS=1 npx vitest run tests/protocol/docker-sidecar.test.ts`

Expected: All 3 tests pass (on macOS with Docker Desktop running).

- [ ] **Step 3: Run without Docker flag to verify skip**

Run: `npx vitest run tests/protocol/docker-sidecar.test.ts`

Expected: Tests are skipped (not failed).

- [ ] **Step 4: Run full test suite to verify no regressions**

Run: `cd crates/pi-sandbox-runtime && cargo test && cd ../.. && npx vitest run tests/protocol/`

Expected: All Rust tests pass. All protocol tests pass (Docker tests skipped unless flag set).

- [ ] **Step 5: Commit**

```bash
git add tests/protocol/docker-sidecar.test.ts
git commit -m "feat: add Docker sidecar integration tests (gated behind RUN_DOCKER_TESTS=1)"
```

---

## Phase Gate Checklist

After all tasks are complete, verify:

- [ ] Dockerfile builds successfully (`docker build -t pi-sandbox-base:latest -f docker/pi-sandbox-sidecar.Dockerfile .`)
- [ ] Sidecar starts, stops, and recovers from crash
- [ ] `sandbox_run` on macOS with Docker produces `enforcement: "enforced"` and `isolationBackend: "docker"`
- [ ] `sandbox_run` on macOS without Docker still degrades gracefully + `isolationBackend: "none"`
- [ ] `PI_SANDBOX_NO_DOCKER=1` skips Docker detection
- [ ] All existing Linux and macOS tests continue to pass
- [ ] Path rewriting unit tests pass on any platform
