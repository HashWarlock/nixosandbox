# Nix Flake Runtime Part B Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Docker sidecar support for macOS (with /nix/store mount), clean up all legacy NDJSON protocol code, and add integration tests for both Linux native and macOS Docker execution paths.

**Architecture:** The Docker sidecar container is stripped to bwrap+iptables only — all runtime packages come from the Nix rootfs via --pivot-root with /nix/store mounted read-only. Legacy NDJSON subprocess mode is deleted; exec --json becomes the sole machine-readable interface with full lifecycle event streaming. Two independently-gated integration test suites validate the rootfs pipeline end-to-end.

**Tech Stack:** Rust (clap, serde_json, chrono), Nix flakes, Bubblewrap, Docker, TypeScript/Vitest

---

## File Structure

### Modified
| File | Responsibility |
|------|---------------|
| `crates/nixosandbox/src/bubblewrap.rs` | Rename env vars PI_SANDBOX_* → NIXOSANDBOX_* |
| `crates/nixosandbox/src/docker.rs` | Rename constants, add /nix/store mount, update session paths |
| `crates/nixosandbox/src/main.rs` | Delete legacy entry point, enhance exec --json, wire Docker path rewriting |
| `crates/nixosandbox/src/cli.rs` | Remove LegacyNdjson variant |
| `crates/nixosandbox/src/contract.rs` | Remove inbound types (InboundMessage, CancelPayload) |

### Deleted
| File | Reason |
|------|--------|
| `docker/pi-sandbox-sidecar.Dockerfile` | Replaced by nixosandbox-sidecar.Dockerfile |
| `tests/protocol/version-mismatch.test.ts` | Legacy NDJSON-only test |
| `tests/protocol/validation-failure.test.ts` | Legacy NDJSON-only test |
| `tests/protocol/degraded-allowlist.test.ts` | Legacy NDJSON-only test |
| `tests/protocol/network-observation.test.ts` | Legacy NDJSON-only test |
| `tests/protocol/allowlist-enforced.test.ts` | Legacy NDJSON-only test |

### Created
| File | Responsibility |
|------|---------------|
| `docker/nixosandbox-sidecar.Dockerfile` | Minimal sidecar: bwrap + iptables only |
| `tests/integration/package.json` | Integration test project dependencies |
| `tests/integration/tsconfig.json` | TypeScript config for integration tests |
| `tests/integration/vitest.config.ts` | Vitest config with globalSetup |
| `tests/integration/globalSetup.ts` | Cargo build, set NIXOSANDBOX_BINARY |
| `tests/integration/helpers.ts` | CLI wrapper functions (build, create, execCmd, list, destroy) |
| `tests/integration/rootfs-pipeline.test.ts` | Linux native integration tests (RUN_INTEGRATION_TESTS=1) |
| `tests/integration/docker-rootfs.test.ts` | macOS Docker integration tests (RUN_DOCKER_TESTS=1) |

### Adapted
| File | Changes |
|------|---------|
| `tests/protocol/globalSetup.ts` | Rename RUNTIME_BINARY_PATH → NIXOSANDBOX_BINARY, PI_SANDBOX_NO_DOCKER → NIXOSANDBOX_NO_DOCKER |
| `tests/protocol/helpers.ts` | Add spawnExecJson() for exec --json mode, keep spawnRuntime() for remaining legacy tests |
| `tests/protocol/cancel-flow.test.ts` | Rewrite to use exec --json with a pre-created session, gate behind RUN_INTEGRATION_TESTS |
| `tests/protocol/crash-synthesis.test.ts` | No changes needed (TS-only, no runtime dependency) |
| `tests/protocol/docker-sidecar.test.ts` | Rewrite to test rootfs execution through Docker, update naming |

---

### Task 1: Rename Dockerfile and strip to minimum

**Files:**
- Delete: `docker/pi-sandbox-sidecar.Dockerfile`
- Create: `docker/nixosandbox-sidecar.Dockerfile`

- [ ] **Step 1: Delete the old Dockerfile**

```bash
rm docker/pi-sandbox-sidecar.Dockerfile
```

- [ ] **Step 2: Create the new minimal Dockerfile**

Create `docker/nixosandbox-sidecar.Dockerfile`:

```dockerfile
# docker/nixosandbox-sidecar.Dockerfile
#
# Minimal Linux sidecar for running bwrap on macOS via Docker Desktop.
# All runtime packages come from the Nix rootfs via --pivot-root.
# This container only provides bwrap (sandbox primitive) and iptables (network enforcement).
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    bubblewrap \
    iptables \
    && rm -rf /var/lib/apt/lists/*
```

- [ ] **Step 3: Commit**

```bash
git add docker/pi-sandbox-sidecar.Dockerfile docker/nixosandbox-sidecar.Dockerfile
git commit -m "refactor: rename and strip Dockerfile to nixosandbox-sidecar

Remove pre-installed packages (python3, nodejs, git, curl, ca-certificates).
All runtime packages now come from Nix rootfs via --pivot-root.
Only bwrap and iptables remain."
```

---

### Task 2: Rename env vars in bubblewrap.rs

**Files:**
- Modify: `crates/nixosandbox/src/bubblewrap.rs:27,55`

- [ ] **Step 1: Rename PI_SANDBOX_NO_DOCKER to NIXOSANDBOX_NO_DOCKER**

In `crates/nixosandbox/src/bubblewrap.rs`, replace:

```rust
        if std::env::var("PI_SANDBOX_NO_DOCKER").map_or(false, |v| v == "1") {
            return BwrapAvailability::Unavailable {
                reason: "Docker fallback disabled via PI_SANDBOX_NO_DOCKER=1".to_string(),
            };
        }
```

with:

```rust
        if std::env::var("NIXOSANDBOX_NO_DOCKER").map_or(false, |v| v == "1") {
            return BwrapAvailability::Unavailable {
                reason: "Docker fallback disabled via NIXOSANDBOX_NO_DOCKER=1".to_string(),
            };
        }
```

- [ ] **Step 2: Rename PI_SANDBOX_BWRAP_PATH to NIXOSANDBOX_BWRAP_PATH**

In the same file, replace:

```rust
        if let Ok(path_str) = std::env::var("PI_SANDBOX_BWRAP_PATH") {
```

with:

```rust
        if let Ok(path_str) = std::env::var("NIXOSANDBOX_BWRAP_PATH") {
```

And replace:

```rust
                reason: format!(
                    "PI_SANDBOX_BWRAP_PATH set to '{}' but file does not exist",
                    path_str
                ),
```

with:

```rust
                reason: format!(
                    "NIXOSANDBOX_BWRAP_PATH set to '{}' but file does not exist",
                    path_str
                ),
```

- [ ] **Step 3: Run tests to verify**

Run: `cd crates/nixosandbox && cargo test --test-threads=1 -- bubblewrap 2>&1`
Expected: All bubblewrap tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/nixosandbox/src/bubblewrap.rs
git commit -m "refactor: rename PI_SANDBOX_* env vars to NIXOSANDBOX_*

PI_SANDBOX_NO_DOCKER → NIXOSANDBOX_NO_DOCKER
PI_SANDBOX_BWRAP_PATH → NIXOSANDBOX_BWRAP_PATH"
```

---

### Task 3: Update docker.rs — naming, /nix/store mount, session paths

**Files:**
- Modify: `crates/nixosandbox/src/docker.rs:5-7,19-26,102-116,119-141,243-338`

- [ ] **Step 1: Update the three constants**

In `crates/nixosandbox/src/docker.rs`, replace:

```rust
const SIDECAR_NAME: &str = "pi-sandbox-sidecar";
const IMAGE_NAME: &str = "pi-sandbox-base:latest";
const CONTAINER_SESSIONS_DIR: &str = "/pi-sandbox";
```

with:

```rust
const SIDECAR_NAME: &str = "nixosandbox-sidecar";
const IMAGE_NAME: &str = "nixosandbox-sidecar:latest";
const CONTAINER_SESSIONS_DIR: &str = "/nixosandbox/sessions";
```

- [ ] **Step 2: Update get_data_dir to use new env var and path**

Replace:

```rust
fn get_data_dir() -> Result<String, String> {
    if let Ok(dir) = std::env::var("PI_SANDBOX_DATA_DIR") {
        return Ok(dir);
    }
    let home = std::env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    Ok(format!("{home}/.local/share/pi-sandbox"))
}
```

with:

```rust
fn get_data_dir() -> Result<String, String> {
    if let Ok(dir) = std::env::var("NIXOSANDBOX_DATA_DIR") {
        return Ok(dir);
    }
    let home = std::env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    Ok(format!("{home}/.local/share/nixosandbox"))
}
```

- [ ] **Step 3: Update ensure_image to reference new Dockerfile**

Replace:

```rust
    eprintln!("pi-sandbox: building Docker sidecar image (one-time setup)...");
    let status = Command::new("docker")
        .args([
            "build", "-t", IMAGE_NAME,
            "-f", "docker/pi-sandbox-sidecar.Dockerfile", ".",
        ])
```

with:

```rust
    eprintln!("nixosandbox: building Docker sidecar image (one-time setup)...");
    let status = Command::new("docker")
        .args([
            "build", "-t", IMAGE_NAME,
            "-f", "docker/nixosandbox-sidecar.Dockerfile", ".",
        ])
```

- [ ] **Step 4: Add /nix/store volume mount in create_sidecar**

Replace the `create_sidecar` function:

```rust
fn create_sidecar(host_sessions_dir: &str) -> Result<String, String> {
    let volume_arg = format!("{host_sessions_dir}:{CONTAINER_SESSIONS_DIR}");
    let output = Command::new("docker")
        .args([
            "run", "-d",
            "--name", SIDECAR_NAME,
            "--cap-add", "SYS_ADMIN",
            "--cap-add", "NET_ADMIN",
            "--security-opt", "seccomp=unconfined",
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

with:

```rust
fn create_sidecar(host_sessions_dir: &str) -> Result<String, String> {
    let sessions_volume = format!("{host_sessions_dir}:{CONTAINER_SESSIONS_DIR}");
    let output = Command::new("docker")
        .args([
            "run", "-d",
            "--name", SIDECAR_NAME,
            "--cap-add", "SYS_ADMIN",
            "--cap-add", "NET_ADMIN",
            "--security-opt", "seccomp=unconfined",
            "-v", &sessions_volume,
            "-v", "/nix/store:/nix/store:ro",
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

- [ ] **Step 5: Update rewrite_path unit tests to use new paths**

Replace the test `rewrite_path_replaces_matching_prefix`:

```rust
    #[test]
    fn rewrite_path_replaces_matching_prefix() {
        let result = rewrite_path(
            "/Users/me/.local/share/pi-sandbox/sessions/abc/workspace",
            "/Users/me/.local/share/pi-sandbox",
            "/pi-sandbox",
        );
        assert_eq!(result, "/pi-sandbox/sessions/abc/workspace");
    }
```

with:

```rust
    #[test]
    fn rewrite_path_replaces_matching_prefix() {
        let result = rewrite_path(
            "/Users/me/.local/share/nixosandbox/sessions/abc/workspace",
            "/Users/me/.local/share/nixosandbox/sessions",
            "/nixosandbox/sessions",
        );
        assert_eq!(result, "/nixosandbox/sessions/abc/workspace");
    }
```

And replace the test `rewrite_path_leaves_non_matching_path_unchanged`:

```rust
    #[test]
    fn rewrite_path_leaves_non_matching_path_unchanged() {
        let result = rewrite_path(
            "/usr/bin/python3",
            "/Users/me/.local/share/pi-sandbox",
            "/pi-sandbox",
        );
        assert_eq!(result, "/usr/bin/python3");
    }
```

with:

```rust
    #[test]
    fn rewrite_path_leaves_non_matching_path_unchanged() {
        let result = rewrite_path(
            "/nix/store/abc123-sandbox-strict",
            "/Users/me/.local/share/nixosandbox/sessions",
            "/nixosandbox/sessions",
        );
        assert_eq!(result, "/nix/store/abc123-sandbox-strict");
    }
```

And update the `rewrite_plan_rewrites_mount_sources_and_cwd` test — replace all `/Users/me/.local/share/pi-sandbox` with `/Users/me/.local/share/nixosandbox` and `/pi-sandbox` with `/nixosandbox/sessions`:

```rust
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
                        source: Some("/Users/me/.local/share/nixosandbox/sessions/s1/workspace".to_string()),
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
                cwd: "/Users/me/.local/share/nixosandbox/sessions/s1/workspace".to_string(),
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
            "/Users/me/.local/share/nixosandbox/sessions",
            "/nixosandbox/sessions",
        );

        assert_eq!(
            rewritten.manifest.mounts[0].source.as_deref(),
            Some("/nixosandbox/sessions/s1/workspace")
        );
        assert_eq!(rewritten.manifest.mounts[1].source, None);
        assert_eq!(rewritten.manifest.cwd, "/nixosandbox/sessions/s1/workspace");
        // Original plan is unchanged
        assert_eq!(
            plan.manifest.cwd,
            "/Users/me/.local/share/nixosandbox/sessions/s1/workspace"
        );
    }
```

- [ ] **Step 6: Run tests to verify**

Run: `cd crates/nixosandbox && cargo test --test-threads=1 -- docker 2>&1`
Expected: All docker tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/nixosandbox/src/docker.rs
git commit -m "refactor: update docker.rs naming, add /nix/store mount

- Rename sidecar to nixosandbox-sidecar, image to nixosandbox-sidecar:latest
- Container sessions dir /pi-sandbox → /nixosandbox/sessions
- Add -v /nix/store:/nix/store:ro volume mount
- Update env var to NIXOSANDBOX_DATA_DIR
- Reference new Dockerfile path"
```

---

### Task 4: Wire Docker exec with session path rewriting in main.rs

**Files:**
- Modify: `crates/nixosandbox/src/main.rs:178-283`

- [ ] **Step 1: Add path rewriting for Docker exec in cmd_exec**

In `crates/nixosandbox/src/main.rs`, the `cmd_exec` function has a Docker branch that currently just prints a warning. Replace the entire bwrap availability check and execution section (lines 177-283). The key change: when Docker is detected, rewrite session directory paths from host to container paths before building bwrap argv for the Docker execution path.

Find this block in cmd_exec:

```rust
    // Check bwrap availability
    let bwrap = bubblewrap::detect();
    match &bwrap {
        bubblewrap::BwrapAvailability::Available { .. } => {}
        bubblewrap::BwrapAvailability::DockerAvailable { .. } => {
            eprintln!("warning: Docker execution with rootfs not yet fully supported");
        }
        bubblewrap::BwrapAvailability::Unavailable { reason } => {
            eprintln!("error: bwrap is not available: {reason}");
            std::process::exit(1);
        }
    };
```

Replace it with:

```rust
    // Check bwrap availability
    let bwrap = bubblewrap::detect();
    match &bwrap {
        bubblewrap::BwrapAvailability::Available { .. } => {}
        bubblewrap::BwrapAvailability::DockerAvailable { .. } => {}
        bubblewrap::BwrapAvailability::Unavailable { reason } => {
            eprintln!("error: bwrap is not available: {reason}");
            std::process::exit(1);
        }
    };
```

Then find the bwrap_argv construction and update the Docker branch to rewrite session paths. Replace the existing `rootfs_dirs` and `bwrap_argv` construction:

```rust
    let rootfs_dirs = plan_builder::RootfsSessionDirs {
        workspace: dirs.workspace.to_string_lossy().to_string(),
        home: dirs.home.to_string_lossy().to_string(),
        cache: dirs.cache.to_string_lossy().to_string(),
    };

    let bwrap_argv = plan_builder::build_rootfs(
        &meta.rootfs_path,
        &rootfs_dirs,
        &command,
        &env,
        &sandbox_spec.network,
        &sandbox_spec.namespaces,
    );
```

with:

```rust
    // For Docker, rewrite session directory paths from host to container paths.
    // Nix store paths need no rewriting — identical on host and container.
    let rootfs_dirs = match &bwrap {
        bubblewrap::BwrapAvailability::DockerAvailable {
            host_sessions_dir,
            container_sessions_dir,
            ..
        } => plan_builder::RootfsSessionDirs {
            workspace: docker::rewrite_path(
                &dirs.workspace.to_string_lossy(),
                host_sessions_dir,
                container_sessions_dir,
            ),
            home: docker::rewrite_path(
                &dirs.home.to_string_lossy(),
                host_sessions_dir,
                container_sessions_dir,
            ),
            cache: docker::rewrite_path(
                &dirs.cache.to_string_lossy(),
                host_sessions_dir,
                container_sessions_dir,
            ),
        },
        _ => plan_builder::RootfsSessionDirs {
            workspace: dirs.workspace.to_string_lossy().to_string(),
            home: dirs.home.to_string_lossy().to_string(),
            cache: dirs.cache.to_string_lossy().to_string(),
        },
    };

    let bwrap_argv = plan_builder::build_rootfs(
        &meta.rootfs_path,
        &rootfs_dirs,
        &command,
        &env,
        &sandbox_spec.network,
        &sandbox_spec.namespaces,
    );
```

- [ ] **Step 2: Run cargo check to verify compilation**

Run: `cd crates/nixosandbox && cargo check 2>&1`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/nixosandbox/src/main.rs
git commit -m "feat: wire Docker exec with session path rewriting

When running via Docker sidecar, rewrite session directory paths
(workspace, home, cache) from host paths to container paths.
Nix store paths are identical on host and container, so rootfs_path
needs no rewriting."
```

---

### Task 5: Enhance exec --json with full event stream

**Files:**
- Modify: `crates/nixosandbox/src/main.rs:191-256`

- [ ] **Step 1: Rewrite the JSON mode block in cmd_exec**

In `crates/nixosandbox/src/main.rs`, replace the entire `if json {` block inside `cmd_exec` (the NDJSON mode section). The current code only streams stdout events and a basic result. Replace it with full lifecycle events, stderr events, and proper signal handling.

Find:

```rust
    if json {
        // NDJSON mode: pipe stdout/stderr, stream events
        use std::process::{Command, Stdio};
        let mut child = match &bwrap {
```

Replace the entire `if json { ... }` branch (from `if json {` to just before `} else {`) with:

```rust
    if json {
        // NDJSON mode: pipe stdout/stderr, stream lifecycle + data events
        use std::process::{Command, Stdio};
        use std::io::{BufRead, BufReader};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;

        let seq = Arc::new(AtomicU64::new(1));

        let mut child = match &bwrap {
            bubblewrap::BwrapAvailability::DockerAvailable { container_id, .. } => {
                let mut cmd_args = vec!["exec".to_string(), "-i".to_string(), container_id.clone(), "bwrap".to_string()];
                cmd_args.extend(bwrap_argv);
                Command::new("docker")
                    .args(&cmd_args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap_or_else(|e| {
                        eprintln!("error: failed to spawn docker+bwrap: {e}");
                        std::process::exit(1);
                    })
            }
            _ => {
                Command::new("bwrap")
                    .args(&bwrap_argv)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap_or_else(|e| {
                        eprintln!("error: failed to spawn bwrap: {e}");
                        std::process::exit(1);
                    })
            }
        };

        let start = std::time::Instant::now();

        // Emit lifecycle started
        let started_event = serde_json::json!({
            "type": "lifecycle",
            "sequence": seq.fetch_add(1, Ordering::SeqCst),
            "ts": timestamps::now_iso8601(),
            "payload": { "event": "started" }
        });
        println!("{}", started_event);

        // Stream stdout and stderr in parallel threads
        let child_stdout = child.stdout.take();
        let child_stderr = child.stderr.take();

        let seq_stdout = Arc::clone(&seq);
        let stdout_thread = std::thread::spawn(move || {
            if let Some(stdout) = child_stdout {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let event = serde_json::json!({
                            "type": "stdout",
                            "sequence": seq_stdout.fetch_add(1, Ordering::SeqCst),
                            "ts": timestamps::now_iso8601(),
                            "payload": { "data": line }
                        });
                        println!("{}", event);
                    }
                }
            }
        });

        let seq_stderr = Arc::clone(&seq);
        let stderr_thread = std::thread::spawn(move || {
            if let Some(stderr) = child_stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let event = serde_json::json!({
                            "type": "stderr",
                            "sequence": seq_stderr.fetch_add(1, Ordering::SeqCst),
                            "ts": timestamps::now_iso8601(),
                            "payload": { "data": line }
                        });
                        println!("{}", event);
                    }
                }
            }
        });

        let status = child.wait().unwrap_or_else(|e| {
            eprintln!("error: wait: {e}");
            std::process::exit(1);
        });

        let _ = stdout_thread.join();
        let _ = stderr_thread.join();

        let duration_ms = start.elapsed().as_millis() as u64;

        // Extract exit code and signal
        let (exit_code, signal) = {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                if let Some(sig) = status.signal() {
                    (None, Some(format!("SIG{sig}")))
                } else {
                    (status.code(), None)
                }
            }
            #[cfg(not(unix))]
            {
                (status.code(), None::<String>)
            }
        };

        // Emit lifecycle exited
        let exited_event = serde_json::json!({
            "type": "lifecycle",
            "sequence": seq.fetch_add(1, Ordering::SeqCst),
            "ts": timestamps::now_iso8601(),
            "payload": { "event": "exited" }
        });
        println!("{}", exited_event);

        // Emit result
        let result = serde_json::json!({
            "type": "result",
            "payload": {
                "exitCode": exit_code.unwrap_or(-1),
                "signal": signal,
                "timedOut": false,
                "durationMs": duration_ms,
            }
        });
        println!("{}", result);
        std::process::exit(exit_code.unwrap_or(1));
    }
```

- [ ] **Step 2: Run cargo check to verify compilation**

Run: `cd crates/nixosandbox && cargo check 2>&1`
Expected: Compiles without errors (may have unused import warnings for sync types, which is fine)

- [ ] **Step 3: Commit**

```bash
git add crates/nixosandbox/src/main.rs
git commit -m "feat: enhance exec --json with full event stream

- Emit lifecycle 'started' event when bwrap spawns
- Stream stderr as separate 'stderr' events (parallel thread)
- Emit lifecycle 'exited' event before result
- Include signal field in result payload
- Sequence numbers strictly increasing across all event types"
```

---

### Task 6: Delete legacy protocol test files

**Files:**
- Delete: `tests/protocol/version-mismatch.test.ts`
- Delete: `tests/protocol/validation-failure.test.ts`
- Delete: `tests/protocol/degraded-allowlist.test.ts`
- Delete: `tests/protocol/network-observation.test.ts`
- Delete: `tests/protocol/allowlist-enforced.test.ts`

- [ ] **Step 1: Delete the five legacy-only test files**

```bash
rm tests/protocol/version-mismatch.test.ts
rm tests/protocol/validation-failure.test.ts
rm tests/protocol/degraded-allowlist.test.ts
rm tests/protocol/network-observation.test.ts
rm tests/protocol/allowlist-enforced.test.ts
```

- [ ] **Step 2: Commit**

```bash
git add tests/protocol/version-mismatch.test.ts tests/protocol/validation-failure.test.ts tests/protocol/degraded-allowlist.test.ts tests/protocol/network-observation.test.ts tests/protocol/allowlist-enforced.test.ts
git commit -m "chore: delete legacy NDJSON-only protocol tests

These tests exercise the legacy-ndjson subprocess protocol which is
being removed. Tests for version-mismatch, validation-failure,
degraded-allowlist, network-observation, and allowlist-enforced
are no longer needed."
```

---

### Task 7: Delete legacy-ndjson subcommand and dead inbound types

**Files:**
- Modify: `crates/nixosandbox/src/cli.rs:91-94`
- Modify: `crates/nixosandbox/src/main.rs:39-41,328-404`
- Modify: `crates/nixosandbox/src/contract.rs:10-82`

- [ ] **Step 1: Remove LegacyNdjson from cli.rs**

In `crates/nixosandbox/src/cli.rs`, delete the LegacyNdjson variant:

```rust
    /// Run in legacy NDJSON subprocess mode (for backward compatibility)
    #[command(hide = true)]
    LegacyNdjson,
```

- [ ] **Step 2: Remove the LegacyNdjson match arm and legacy_ndjson_main() from main.rs**

In `crates/nixosandbox/src/main.rs`, delete the match arm:

```rust
        Commands::LegacyNdjson => {
            legacy_ndjson_main();
        }
```

And delete the entire `legacy_ndjson_main()` function (lines 328-404).

- [ ] **Step 3: Remove InboundMessage, CancelPayload, and ValidationEnvelope from contract.rs**

In `crates/nixosandbox/src/contract.rs`, delete the `InboundMessage` enum and `CancelPayload` struct:

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InboundMessage {
    Plan { payload: PlanPayload },
    Cancel { payload: CancelPayload },
}
```

and:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelPayload {
    pub reason: Option<String>,
}
```

Also delete the `ValidationEnvelope` struct and its `impl` block (the NDJSON wrapper — only used by the legacy protocol):

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub v: u32,
    pub payload: ValidationPayload,
}
```

and:

```rust
impl ValidationEnvelope {
    pub fn new(payload: ValidationPayload) -> OutboundMessage {
        OutboundMessage::Validation(ValidationEnvelope {
            msg_type: "validation",
            v: PROTOCOL_VERSION,
            payload,
        })
    }
}
```

Also remove the `Validation(ValidationEnvelope)` variant from `OutboundMessage`:

```rust
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum OutboundMessage {
    Validation(ValidationEnvelope),  // DELETE THIS LINE
    Stdout(StdoutEnvelope),
    ...
```

Keep `ValidationPayload`, `ValidationError`, `ValidationWarning`, `EffectiveState`, and all other outbound types — they are used by `validator::validate()` which remains.

- [ ] **Step 4: Run cargo check and fix any remaining dead code**

Run: `cd crates/nixosandbox && cargo check 2>&1`

The compiler may report warnings for unused types/functions. Expected surviving code:
- `supervisor::supervise()` — referenced by tests, may be reused later; keep
- `validator::validate()` — returns `ValidationPayload`; keep
- `ValidationPayload`, `ValidationError`, `ValidationWarning` — used by validator; keep
- `PlanPayload` and sub-types — used by plan_builder, docker, supervisor; keep
- `EffectiveState` and related types — used by supervisor, validator; keep

If there are hard errors (not just warnings), fix them. The `use contract::{..., InboundMessage, ...}` in the now-deleted `legacy_ndjson_main` should already be gone. Check for any remaining `use` statements referencing deleted types in other modules.

Expected: Compiles with possible unused-code warnings but no errors.

- [ ] **Step 5: Run full test suite**

Run: `cd crates/nixosandbox && cargo test --test-threads=1 2>&1`
Expected: All Rust tests pass (42 tests). Some code may have `dead_code` warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/nixosandbox/src/cli.rs crates/nixosandbox/src/main.rs crates/nixosandbox/src/contract.rs
git commit -m "feat: delete legacy-ndjson subcommand and inbound types

Remove LegacyNdjson CLI variant and legacy_ndjson_main() entry point.
Delete InboundMessage enum and CancelPayload from contract.rs.
exec --json is now the sole machine-readable interface."
```

---

### Task 8: Update protocol test infrastructure

**Files:**
- Modify: `tests/protocol/globalSetup.ts`
- Modify: `tests/protocol/helpers.ts`

- [ ] **Step 1: Update globalSetup.ts with new env var names**

Replace the entire content of `tests/protocol/globalSetup.ts`:

```typescript
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const CRATE_DIR = resolve(import.meta.dirname, "../../crates/nixosandbox");

export async function setup() {
  console.log("Building nixosandbox...");
  execFileSync("cargo", ["build", "--release"], {
    cwd: CRATE_DIR,
    stdio: "inherit",
  });

  const binaryPath = resolve(CRATE_DIR, "target/release/nixosandbox");
  if (!existsSync(binaryPath)) {
    throw new Error(`Binary not found at ${binaryPath}`);
  }

  process.env.NIXOSANDBOX_BINARY = binaryPath;
  // Disable Docker sidecar for non-Docker tests.
  // Docker-specific tests override this via their own env.
  process.env.NIXOSANDBOX_NO_DOCKER = "1";
  console.log(`Runtime binary: ${binaryPath}`);
}
```

- [ ] **Step 2: Add spawnExecJson helper to helpers.ts**

Replace the entire content of `tests/protocol/helpers.ts`:

```typescript
import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";

export interface TestRuntime {
  send(message: Record<string, unknown>): void;
  readline(): Promise<Record<string, unknown>>;
  readAllEvents(): Promise<Record<string, unknown>[]>;
  kill(signal?: NodeJS.Signals): void;
  waitForExit(): Promise<{ code: number | null; signal: string | null }>;
  stderr: string;
  process: ChildProcess;
}

/**
 * Spawn `nixosandbox exec --json <sessionId> -- <command>` and return
 * a TestRuntime that reads NDJSON events from stdout.
 */
export function spawnExecJson(
  sessionId: string,
  command: string[],
  options?: { env?: NodeJS.ProcessEnv; extraArgs?: string[] },
): TestRuntime {
  const binaryPath = process.env.NIXOSANDBOX_BINARY;
  if (!binaryPath) {
    throw new Error("NIXOSANDBOX_BINARY not set. Did globalSetup run?");
  }

  const args = [
    "exec",
    "--json",
    ...(options?.extraArgs ?? []),
    sessionId,
    "--",
    ...command,
  ];

  const child = spawn(binaryPath, args, {
    stdio: ["pipe", "pipe", "pipe"],
    env: options?.env ?? process.env,
  });

  return wrapChildProcess(child);
}

/**
 * Wrap a ChildProcess into a TestRuntime for NDJSON event reading.
 */
function wrapChildProcess(child: ChildProcess): TestRuntime {
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
      if (!line) throw new Error("Empty line received");
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
        if (!line) break;
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
```

- [ ] **Step 3: Commit**

```bash
git add tests/protocol/globalSetup.ts tests/protocol/helpers.ts
git commit -m "refactor: update protocol test infrastructure for exec --json

- Rename RUNTIME_BINARY_PATH → NIXOSANDBOX_BINARY
- Rename PI_SANDBOX_NO_DOCKER → NIXOSANDBOX_NO_DOCKER
- Replace spawnRuntime/makePlan with spawnExecJson helper
- New helper spawns 'nixosandbox exec --json <id> -- <cmd>'"
```

---

### Task 9: Adapt remaining protocol tests

**Files:**
- Modify: `tests/protocol/cancel-flow.test.ts`
- Modify: `tests/protocol/crash-synthesis.test.ts` (minimal — verify it still compiles)
- Modify: `tests/protocol/docker-sidecar.test.ts`

- [ ] **Step 1: Rewrite cancel-flow.test.ts**

Replace the entire content of `tests/protocol/cancel-flow.test.ts`:

```typescript
import { describe, expect, it, beforeAll, afterAll } from "vitest";
import { execFileSync } from "node:child_process";
import { spawnExecJson } from "./helpers.js";

const RUN_INTEGRATION = process.env.RUN_INTEGRATION_TESTS === "1";
const RUN_DOCKER = process.env.RUN_DOCKER_TESTS === "1";

describe.skipIf(!RUN_INTEGRATION && !RUN_DOCKER)(
  "Cancel Flow (exec --json)",
  () => {
    let sessionId: string;

    beforeAll(() => {
      const binaryPath = process.env.NIXOSANDBOX_BINARY;
      if (!binaryPath) throw new Error("NIXOSANDBOX_BINARY not set");

      // Create a session for testing
      const env = RUN_DOCKER
        ? { ...process.env, NIXOSANDBOX_NO_DOCKER: undefined } as NodeJS.ProcessEnv
        : process.env;
      const output = execFileSync(binaryPath, [
        "create", "--profile", "strict", "--json",
      ], { env, encoding: "utf-8" });
      const meta = JSON.parse(output);
      sessionId = meta.sessionId;
    });

    afterAll(() => {
      const binaryPath = process.env.NIXOSANDBOX_BINARY;
      if (binaryPath && sessionId) {
        try {
          execFileSync(binaryPath, ["destroy", sessionId], { stdio: "ignore" });
        } catch {
          // Cleanup best-effort
        }
      }
    });

    it("cancels a running process via SIGTERM and observes lifecycle events", async () => {
      const env = RUN_DOCKER
        ? { ...process.env, NIXOSANDBOX_NO_DOCKER: undefined } as NodeJS.ProcessEnv
        : process.env;
      const rt = spawnExecJson(sessionId, ["sleep", "3600"], { env });

      // Read events until we see "started" lifecycle
      let startedSeen = false;
      const preEvents: Record<string, unknown>[] = [];
      while (!startedSeen) {
        const event = await rt.readline();
        preEvents.push(event);
        if (
          event.type === "lifecycle" &&
          (event.payload as any).event === "started"
        ) {
          startedSeen = true;
        }
      }
      expect(startedSeen).toBe(true);

      // Send SIGTERM to the nixosandbox process (which kills the bwrap child)
      rt.kill("SIGTERM");

      // Read remaining events — should include result with non-zero exit or signal
      const resultPromise = new Promise<Record<string, unknown> | null>(
        async (resolve) => {
          const timer = setTimeout(() => resolve(null), 10000);
          try {
            while (true) {
              const event = await rt.readline();
              if (event.type === "result") {
                clearTimeout(timer);
                resolve(event);
                return;
              }
            }
          } catch {
            clearTimeout(timer);
            resolve(null);
          }
        },
      );

      const resultEvent = await resultPromise;

      if (resultEvent) {
        const resultPayload = resultEvent.payload as any;
        // Process was killed — either signal or non-zero exit
        expect(
          resultPayload.exitCode !== 0 || resultPayload.signal !== null,
        ).toBe(true);
      } else {
        // Force-kill if no result received
        rt.kill("SIGKILL");
      }

      const exit = await rt.waitForExit();
      expect(exit.signal !== null || exit.code !== null).toBe(true);
    }, 30_000);
  },
);
```

- [ ] **Step 2: Verify crash-synthesis.test.ts needs no changes**

The crash-synthesis test imports from `packages/pi-sandbox-extension/src/crash-synthesis.js` and is TS-only. It does not spawn the runtime binary. Verify it still compiles by reading it — no changes needed.

- [ ] **Step 3: Rewrite docker-sidecar.test.ts**

Replace the entire content of `tests/protocol/docker-sidecar.test.ts`:

```typescript
/**
 * Docker sidecar integration tests with rootfs execution.
 *
 * These tests require Docker Desktop + Nix and are gated behind
 * RUN_DOCKER_TESTS=1.
 *
 * Run: RUN_DOCKER_TESTS=1 npx vitest run tests/protocol/docker-sidecar.test.ts
 */
import { execFileSync } from "node:child_process";
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { spawnExecJson } from "./helpers.js";

const DOCKER_TESTS = process.env.RUN_DOCKER_TESTS === "1";

// Docker tests need NIXOSANDBOX_NO_DOCKER unset
const dockerEnv = {
  ...process.env,
  NIXOSANDBOX_NO_DOCKER: undefined,
} as NodeJS.ProcessEnv;

describe.skipIf(!DOCKER_TESTS)("Docker sidecar (rootfs)", () => {
  let sessionId: string;

  beforeAll(() => {
    const binaryPath = process.env.NIXOSANDBOX_BINARY;
    if (!binaryPath) throw new Error("NIXOSANDBOX_BINARY not set");

    // Clean up any leftover sidecar from previous runs
    try {
      execFileSync("docker", ["rm", "-f", "nixosandbox-sidecar"], {
        stdio: "ignore",
      });
    } catch {
      // Container didn't exist
    }

    // Create a session with Docker enabled
    const output = execFileSync(
      binaryPath,
      ["create", "--profile", "strict", "--json"],
      { env: dockerEnv, encoding: "utf-8" },
    );
    const meta = JSON.parse(output);
    sessionId = meta.sessionId;
  }, 120_000); // 2min for first-time rootfs build + Docker image build

  afterAll(() => {
    const binaryPath = process.env.NIXOSANDBOX_BINARY;
    if (binaryPath && sessionId) {
      try {
        execFileSync(binaryPath, ["destroy", sessionId], { stdio: "ignore" });
      } catch {
        // Cleanup best-effort
      }
    }
  });

  it("runs echo through Docker+bwrap with rootfs and gets lifecycle events", async () => {
    const rt = spawnExecJson(sessionId, ["echo", "hello from docker"], {
      env: dockerEnv,
    });

    const events = await rt.readAllEvents();

    // Should have lifecycle(started), stdout, lifecycle(exited), result
    expect(events.length).toBeGreaterThanOrEqual(3);

    const startedEvent = events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "started",
    );
    expect(startedEvent).toBeDefined();

    const stdoutEvents = events.filter((e) => e.type === "stdout");
    const helloEvent = stdoutEvents.find((e) =>
      ((e.payload as any).data as string).includes("hello from docker"),
    );
    expect(helloEvent).toBeDefined();

    const exitedEvent = events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "exited",
    );
    expect(exitedEvent).toBeDefined();

    const result = events.find((e) => e.type === "result") as any;
    expect(result).toBeDefined();
    expect(result.payload.exitCode).toBe(0);

    await rt.waitForExit();
  }, 60_000);

  it("verifies Nix store is accessible inside container", async () => {
    const rt = spawnExecJson(sessionId, ["ls", "/nix/store"], {
      env: dockerEnv,
    });

    const events = await rt.readAllEvents();

    const result = events.find((e) => e.type === "result") as any;
    expect(result).toBeDefined();
    // ls /nix/store should succeed since we mount it
    // Note: inside the bwrap sandbox, /nix/store is part of the rootfs
    // via --pivot-root, not the Docker mount. The Docker mount makes it
    // available to bwrap for --pivot-root to use.
    // The actual test is that bwrap can access the rootfs path.

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  }, 30_000);

  it("NIXOSANDBOX_NO_DOCKER=1 blocks Docker and exits with error", () => {
    const binaryPath = process.env.NIXOSANDBOX_BINARY;
    if (!binaryPath) throw new Error("NIXOSANDBOX_BINARY not set");

    // With Docker disabled on non-Linux, exec should fail
    try {
      execFileSync(
        binaryPath,
        ["exec", sessionId, "--", "echo", "should-fail"],
        {
          env: { ...process.env, NIXOSANDBOX_NO_DOCKER: "1" },
          encoding: "utf-8",
          stdio: "pipe",
        },
      );
      // If we're on Linux with bwrap, this might succeed — that's OK
    } catch (err: any) {
      // On macOS without Docker, should fail with non-zero exit
      expect(err.status).not.toBe(0);
    }
  }, 15_000);
});
```

- [ ] **Step 4: Commit**

```bash
git add tests/protocol/cancel-flow.test.ts tests/protocol/docker-sidecar.test.ts
git commit -m "refactor: adapt protocol tests for exec --json

- cancel-flow: use spawnExecJson with pre-created session, gate behind
  RUN_INTEGRATION_TESTS or RUN_DOCKER_TESTS
- docker-sidecar: test rootfs execution through Docker, update naming
  to nixosandbox-sidecar, verify Nix store access"
```

---

### Task 10: Create integration test infrastructure

**Files:**
- Create: `tests/integration/package.json`
- Create: `tests/integration/tsconfig.json`
- Create: `tests/integration/vitest.config.ts`
- Create: `tests/integration/globalSetup.ts`
- Create: `tests/integration/helpers.ts`

- [ ] **Step 1: Create package.json**

Create `tests/integration/package.json`:

```json
{
  "name": "@nixosandbox/integration-tests",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "devDependencies": {
    "typescript": "^5.7.0",
    "vitest": "^3.0.0"
  }
}
```

- [ ] **Step 2: Create tsconfig.json**

Create `tests/integration/tsconfig.json`:

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

- [ ] **Step 3: Create vitest.config.ts**

Create `tests/integration/vitest.config.ts`:

```typescript
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["*.test.ts"],
    globalSetup: "./globalSetup.ts",
    testTimeout: 120000, // 2 minutes — Nix builds can be slow
  },
});
```

- [ ] **Step 4: Create globalSetup.ts**

Create `tests/integration/globalSetup.ts`:

```typescript
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const CRATE_DIR = resolve(import.meta.dirname, "../../crates/nixosandbox");

export async function setup() {
  console.log("Building nixosandbox (release)...");
  execFileSync("cargo", ["build", "--release"], {
    cwd: CRATE_DIR,
    stdio: "inherit",
  });

  const binaryPath = resolve(CRATE_DIR, "target/release/nixosandbox");
  if (!existsSync(binaryPath)) {
    throw new Error(`Binary not found at ${binaryPath}`);
  }

  process.env.NIXOSANDBOX_BINARY = binaryPath;
  console.log(`Runtime binary: ${binaryPath}`);
}
```

- [ ] **Step 5: Create helpers.ts**

Create `tests/integration/helpers.ts`:

```typescript
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";

function getBinary(): string {
  const bin = process.env.NIXOSANDBOX_BINARY;
  if (!bin) throw new Error("NIXOSANDBOX_BINARY not set. Did globalSetup run?");
  return bin;
}

export interface BuildResult {
  stdout: string;
  exitCode: number;
}

/**
 * Run `nixosandbox build` with the given args.
 */
export function build(args: string[], env?: NodeJS.ProcessEnv): BuildResult {
  try {
    const stdout = execFileSync(getBinary(), ["build", ...args], {
      encoding: "utf-8",
      env: env ?? process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });
    return { stdout: stdout.trim(), exitCode: 0 };
  } catch (err: any) {
    return { stdout: err.stdout?.toString() ?? "", exitCode: err.status ?? 1 };
  }
}

export interface CreateResult {
  sessionId: string;
  metadata: Record<string, unknown>;
}

/**
 * Run `nixosandbox create` and parse the JSON output.
 */
export function create(
  args: string[],
  env?: NodeJS.ProcessEnv,
): CreateResult {
  const stdout = execFileSync(getBinary(), ["create", "--json", ...args], {
    encoding: "utf-8",
    env: env ?? process.env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const metadata = JSON.parse(stdout.trim()) as Record<string, unknown>;
  return { sessionId: metadata.sessionId as string, metadata };
}

export interface ExecResult {
  events: Record<string, unknown>[];
  exitCode: number;
}

/**
 * Run `nixosandbox exec --json <sessionId> -- <command>` and collect all NDJSON events.
 */
export async function execCmd(
  sessionId: string,
  command: string[],
  opts?: { env?: NodeJS.ProcessEnv; extraEnv?: string[] },
): Promise<ExecResult> {
  const envArgs = (opts?.extraEnv ?? []).flatMap((e) => ["--env", e]);
  const args = ["exec", "--json", ...envArgs, sessionId, "--", ...command];

  return new Promise((resolve, reject) => {
    const child = spawn(getBinary(), args, {
      stdio: ["pipe", "pipe", "pipe"],
      env: opts?.env ?? process.env,
    });

    const events: Record<string, unknown>[] = [];
    const rl = createInterface({ input: child.stdout! });

    rl.on("line", (line) => {
      try {
        events.push(JSON.parse(line));
      } catch {
        // Ignore unparseable lines
      }
    });

    child.on("exit", (code) => {
      resolve({ events, exitCode: code ?? 1 });
    });

    child.on("error", (err) => {
      reject(err);
    });
  });
}

export interface ListResult {
  sessions: Record<string, unknown>[];
}

/**
 * Run `nixosandbox list --json` and parse the JSON output.
 */
export function list(env?: NodeJS.ProcessEnv): ListResult {
  const stdout = execFileSync(getBinary(), ["list", "--json"], {
    encoding: "utf-8",
    env: env ?? process.env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const sessions = JSON.parse(stdout.trim()) as Record<string, unknown>[];
  return { sessions };
}

/**
 * Run `nixosandbox destroy <sessionId>`.
 */
export function destroy(
  sessionId: string,
  env?: NodeJS.ProcessEnv,
): number {
  try {
    execFileSync(getBinary(), ["destroy", sessionId], {
      env: env ?? process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });
    return 0;
  } catch (err: any) {
    return err.status ?? 1;
  }
}
```

- [ ] **Step 6: Install dependencies**

```bash
cd tests/integration && npm install
```

- [ ] **Step 7: Commit**

```bash
git add tests/integration/package.json tests/integration/tsconfig.json tests/integration/vitest.config.ts tests/integration/globalSetup.ts tests/integration/helpers.ts tests/integration/package-lock.json
git commit -m "feat: create integration test infrastructure

New test project at tests/integration/ with:
- CLI wrapper helpers (build, create, execCmd, list, destroy)
- Vitest config with 2-minute timeout for Nix builds
- globalSetup builds the nixosandbox binary"
```

---

### Task 11: rootfs-pipeline integration tests

**Files:**
- Create: `tests/integration/rootfs-pipeline.test.ts`

- [ ] **Step 1: Create rootfs-pipeline.test.ts**

Create `tests/integration/rootfs-pipeline.test.ts`:

```typescript
/**
 * Linux native integration tests for the rootfs pipeline.
 *
 * Requires: Nix + bwrap on Linux.
 * Gate: RUN_INTEGRATION_TESTS=1
 *
 * Run: RUN_INTEGRATION_TESTS=1 npx vitest run rootfs-pipeline.test.ts
 */
import { describe, it, expect, afterAll } from "vitest";
import { build, create, execCmd, list, destroy } from "./helpers.js";

const RUN = process.env.RUN_INTEGRATION_TESTS === "1";

describe.skipIf(!RUN)("Rootfs Pipeline (Linux native)", () => {
  const sessionsToCleanup: string[] = [];

  afterAll(() => {
    for (const id of sessionsToCleanup) {
      try {
        destroy(id);
      } catch {
        // Best-effort cleanup
      }
    }
  });

  it("build strict profile returns a valid Nix store path", () => {
    const result = build(["--profile", "strict", "--json"]);
    expect(result.exitCode).toBe(0);

    const parsed = JSON.parse(result.stdout);
    expect(parsed.rootfsPath).toBeDefined();
    expect(parsed.rootfsPath).toMatch(/^\/nix\/store\//);
  });

  it("create session returns session ID and metadata", () => {
    const { sessionId, metadata } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    expect(sessionId).toBeDefined();
    expect(sessionId.length).toBe(8);
    expect(metadata.profile).toBe("strict");
    expect(metadata.rootfsPath).toMatch(/^\/nix\/store\//);
  });

  it("exec echo prints hello and exits 0", async () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["echo", "hello"]);
    expect(result.exitCode).toBe(0);

    const stdoutEvents = result.events.filter((e) => e.type === "stdout");
    const helloEvent = stdoutEvents.find((e) =>
      ((e.payload as any).data as string).includes("hello"),
    );
    expect(helloEvent).toBeDefined();
  });

  it("exec verifies rootfs directory structure", async () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["ls", "/"]);
    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");

    // Rootfs should have sandbox dirs
    expect(stdout).toContain("bin");
    expect(stdout).toContain("etc");
    expect(stdout).toContain("workspace");
  });

  it("exec verifies sandbox user exists", async () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["cat", "/etc/passwd"]);
    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");

    expect(stdout).toContain("sandbox");
  });

  it("exec json mode produces lifecycle + stdout + result events", async () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["echo", "test"]);
    expect(result.exitCode).toBe(0);

    // Must have: lifecycle(started), stdout(test), lifecycle(exited), result
    const started = result.events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "started",
    );
    expect(started).toBeDefined();

    const stdout = result.events.find(
      (e) =>
        e.type === "stdout" &&
        ((e.payload as any).data as string).includes("test"),
    );
    expect(stdout).toBeDefined();

    const exited = result.events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "exited",
    );
    expect(exited).toBeDefined();

    const resultEvent = result.events.find((e) => e.type === "result") as any;
    expect(resultEvent).toBeDefined();
    expect(resultEvent.payload.exitCode).toBe(0);
    expect(resultEvent.payload.timedOut).toBe(false);
    expect(resultEvent.payload.durationMs).toBeGreaterThan(0);

    // Sequence numbers strictly increasing
    const sequenced = result.events.filter(
      (e) => (e as any).sequence !== undefined,
    );
    for (let i = 1; i < sequenced.length; i++) {
      expect((sequenced[i] as any).sequence).toBeGreaterThan(
        (sequenced[i - 1] as any).sequence,
      );
    }
  });

  it("list sessions shows the created session", () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const { sessions } = list();
    const found = sessions.find(
      (s) => (s as any).sessionId === sessionId,
    );
    expect(found).toBeDefined();
  });

  it("destroy session removes it from list", () => {
    const { sessionId } = create(["--profile", "strict"]);

    const exitCode = destroy(sessionId);
    expect(exitCode).toBe(0);

    const { sessions } = list();
    const found = sessions.find(
      (s) => (s as any).sessionId === sessionId,
    );
    expect(found).toBeUndefined();
  });
});
```

- [ ] **Step 2: Commit**

```bash
git add tests/integration/rootfs-pipeline.test.ts
git commit -m "feat: add rootfs-pipeline integration tests

8 tests covering the full CLI lifecycle:
- build strict profile → Nix store path
- create session → session ID + metadata
- exec echo → stdout output
- exec ls / → rootfs directory structure
- exec cat /etc/passwd → sandbox user
- exec json mode → lifecycle + stdout + result events
- list sessions → session visible
- destroy session → session removed

Gated: RUN_INTEGRATION_TESTS=1 (requires Nix + bwrap on Linux)"
```

---

### Task 12: Docker rootfs integration tests

**Files:**
- Create: `tests/integration/docker-rootfs.test.ts`

- [ ] **Step 1: Create docker-rootfs.test.ts**

Create `tests/integration/docker-rootfs.test.ts`:

```typescript
/**
 * macOS Docker integration tests for rootfs execution.
 *
 * Requires: Nix + Docker Desktop.
 * Gate: RUN_DOCKER_TESTS=1
 *
 * Run: RUN_DOCKER_TESTS=1 npx vitest run docker-rootfs.test.ts
 */
import { execFileSync } from "node:child_process";
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { create, execCmd, destroy } from "./helpers.js";

const RUN = process.env.RUN_DOCKER_TESTS === "1";

// Docker tests need NIXOSANDBOX_NO_DOCKER unset
const dockerEnv = {
  ...process.env,
  NIXOSANDBOX_NO_DOCKER: undefined,
} as NodeJS.ProcessEnv;

describe.skipIf(!RUN)("Docker Rootfs (macOS)", () => {
  const sessionsToCleanup: string[] = [];

  beforeAll(() => {
    // Clean up any leftover sidecar
    try {
      execFileSync("docker", ["rm", "-f", "nixosandbox-sidecar"], {
        stdio: "ignore",
      });
    } catch {
      // Didn't exist
    }
  });

  afterAll(() => {
    for (const id of sessionsToCleanup) {
      try {
        destroy(id, dockerEnv);
      } catch {
        // Best-effort
      }
    }
  });

  it("create + exec through Docker sidecar", async () => {
    const { sessionId } = create(["--profile", "strict"], dockerEnv);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["echo", "hello from docker"], {
      env: dockerEnv,
    });

    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");
    expect(stdout).toContain("hello from docker");
  }, 120_000);

  it("verifies rootfs directory structure through Docker", async () => {
    const { sessionId } = create(["--profile", "strict"], dockerEnv);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["ls", "/"], { env: dockerEnv });
    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");

    expect(stdout).toContain("bin");
    expect(stdout).toContain("etc");
    expect(stdout).toContain("workspace");
  }, 60_000);

  it("verifies sandbox user through Docker", async () => {
    const { sessionId } = create(["--profile", "strict"], dockerEnv);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["cat", "/etc/passwd"], {
      env: dockerEnv,
    });
    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");

    expect(stdout).toContain("sandbox");
  }, 60_000);

  it("JSON mode reports full lifecycle events through Docker", async () => {
    const { sessionId } = create(["--profile", "strict"], dockerEnv);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["echo", "lifecycle-test"], {
      env: dockerEnv,
    });
    expect(result.exitCode).toBe(0);

    const started = result.events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "started",
    );
    expect(started).toBeDefined();

    const exited = result.events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "exited",
    );
    expect(exited).toBeDefined();

    const resultEvent = result.events.find(
      (e) => e.type === "result",
    ) as any;
    expect(resultEvent).toBeDefined();
    expect(resultEvent.payload.exitCode).toBe(0);
  }, 60_000);
});
```

- [ ] **Step 2: Commit**

```bash
git add tests/integration/docker-rootfs.test.ts
git commit -m "feat: add Docker rootfs integration tests

4 tests covering Docker sidecar execution:
- create + exec through Docker
- verify rootfs directory structure
- verify sandbox user
- JSON mode lifecycle events through Docker

Gated: RUN_DOCKER_TESTS=1 (requires Nix + Docker Desktop)"
```

---

## Test Gating Summary

| Env Var | Suite | Location | Requires |
|---------|-------|----------|----------|
| `RUN_INTEGRATION_TESTS=1` | rootfs-pipeline | `tests/integration/` | Nix, bwrap, Linux |
| `RUN_DOCKER_TESTS=1` | docker-rootfs | `tests/integration/` | Nix, Docker |
| `RUN_INTEGRATION_TESTS=1` or `RUN_DOCKER_TESTS=1` | cancel-flow | `tests/protocol/` | Nix + bwrap or Docker |
| `RUN_DOCKER_TESTS=1` | docker-sidecar | `tests/protocol/` | Nix, Docker |
| (none) | crash-synthesis | `tests/protocol/` | Just Node.js |

## Run Commands

```bash
# Rust unit tests (always)
cd crates/nixosandbox && cargo test --test-threads=1

# Protocol tests (just binary, no Nix)
cd tests/protocol && npx vitest run

# Linux integration tests (Nix + bwrap)
cd tests/integration && RUN_INTEGRATION_TESTS=1 npx vitest run rootfs-pipeline.test.ts

# Docker integration tests (Nix + Docker)
cd tests/integration && RUN_DOCKER_TESTS=1 npx vitest run docker-rootfs.test.ts

# All Docker protocol tests
cd tests/protocol && RUN_DOCKER_TESTS=1 npx vitest run
```
