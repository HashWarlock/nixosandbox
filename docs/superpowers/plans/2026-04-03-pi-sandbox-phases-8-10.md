# Pi Sandbox Phases 8-10 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the stub execution and observation in the Pi Sandbox Rust runtime with real Bubblewrap isolation and `/proc/net/tcp` network observation, validated by real-world build flow integration tests.

**Architecture:** The Rust runtime gains bwrap binary discovery, a pure-function plan builder that converts manifests+policy into bwrap argv, and a background network observer that polls `/proc/net/tcp`. On macOS (no bwrap), the runtime falls back to direct execution with degraded warnings — truthful reporting throughout. Integration tests use checked-in fixture repos (tiny-npm, tiny-python, tiny-rust) to validate real build workflows.

**Tech Stack:** Rust (std only, no new deps), TypeScript (vitest), Bubblewrap (`bwrap`), `/proc/net/tcp`

**Spec:** `docs/superpowers/specs/2026-04-03-pi-sandbox-phases-8-10-design.md`

---

## File Map

### New Files

| File | Responsibility |
|------|----------------|
| `crates/pi-sandbox-runtime/src/bubblewrap.rs` | Bwrap binary discovery, platform detection, `BwrapAvailability` enum |
| `crates/pi-sandbox-runtime/src/plan_builder.rs` | Pure function: `PlanPayload` + `EffectiveState` → `Vec<String>` bwrap argv |
| `tests/protocol/bwrap-integration.test.ts` | Linux-only protocol test for bwrap isolation |
| `tests/integration/fixtures/tiny-npm/package.json` | Empty npm project fixture |
| `tests/integration/fixtures/tiny-python/setup.py` | Stdlib-only Python fixture |
| `tests/integration/fixtures/tiny-python/mypackage/__init__.py` | Python fixture package init |
| `tests/integration/fixtures/tiny-rust/Cargo.toml` | No-deps Rust fixture |
| `tests/integration/fixtures/tiny-rust/src/main.rs` | Rust fixture entrypoint |
| `tests/integration/helpers.ts` | Integration test utilities (copyFixture, makeIntegrationPlan) |
| `tests/integration/globalSetup.ts` | Build Rust binary for integration tests |
| `tests/integration/vitest.config.ts` | Vitest configuration for integration suite |
| `tests/integration/package.json` | Package manifest for integration tests |
| `tests/integration/tsconfig.json` | TypeScript config for integration tests |
| `tests/integration/build-npm.test.ts` | npm install integration test |
| `tests/integration/build-python.test.ts` | pip install integration test |
| `tests/integration/build-rust.test.ts` | cargo build integration test |
| `tests/integration/network-smoke.test.ts` | Optional network smoke test |
| `tests/protocol/network-observation.test.ts` | Linux-only network observation protocol test |

### Modified Files

| File | Changes |
|------|---------|
| `crates/pi-sandbox-runtime/src/contract.rs` | Add `namespaces_applied` and `env_applied` to `EffectiveState` |
| `crates/pi-sandbox-runtime/src/validator.rs` | Accept bwrap availability, resolve namespaces/env, emit NAMESPACE_DEGRADED |
| `crates/pi-sandbox-runtime/src/supervisor.rs` | Accept bwrap availability, dispatch to bwrap or direct, integrate observer |
| `crates/pi-sandbox-runtime/src/main.rs` | Call `bubblewrap::detect()`, pass to validator/supervisor |
| `crates/pi-sandbox-runtime/src/observer.rs` | Replace stub with `NetworkObserver` struct + `/proc/net/tcp` polling |

---

## Phase 8: Bubblewrap Integration

### Task 1: Extend EffectiveState in contract.rs

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/contract.rs:135-138`

- [ ] **Step 1: Add `namespaces_applied` and `env_applied` fields to `EffectiveState`**

In `crates/pi-sandbox-runtime/src/contract.rs`, replace the existing `EffectiveState` struct:

```rust
// OLD (lines 135-138):
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveState {
    pub network: EffectiveNetwork,
}
```

With:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveState {
    pub network: EffectiveNetwork,
    pub namespaces_applied: Vec<String>,
    pub env_applied: Vec<String>,
}
```

- [ ] **Step 2: Fix all compilation errors from the new fields**

Every place that constructs an `EffectiveState` must now provide the two new fields. There is one place in `validator.rs` (around line 115-117):

In `crates/pi-sandbox-runtime/src/validator.rs`, replace:

```rust
    let effective_state = Some(EffectiveState {
        network: effective_network,
    });
```

With:

```rust
    let env_applied: Vec<String> = plan.manifest.env.keys().cloned().collect();

    let effective_state = Some(EffectiveState {
        network: effective_network,
        namespaces_applied: vec![],
        env_applied,
    });
```

Note: `namespaces_applied` is empty for now (no bwrap yet). Task 4 will populate it properly.

- [ ] **Step 3: Verify compilation**

Run: `cd crates/pi-sandbox-runtime && cargo build --release 2>&1`
Expected: Builds successfully (may have dead_code warnings, that's fine).

- [ ] **Step 4: Verify existing protocol tests still pass**

Run: `cd tests/protocol && npx vitest run 2>&1`
Expected: All 6 tests pass (7 individual tests). The new `namespacesApplied` and `envApplied` fields appear in the validation JSON but existing tests don't assert their absence, so they pass unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-sandbox-runtime/src/contract.rs crates/pi-sandbox-runtime/src/validator.rs
git commit -m "feat: add namespacesApplied and envApplied to EffectiveState"
```

---

### Task 2: Create bubblewrap.rs — binary discovery and platform detection

**Files:**
- Create: `crates/pi-sandbox-runtime/src/bubblewrap.rs`
- Modify: `crates/pi-sandbox-runtime/src/main.rs:1` (add `mod bubblewrap;`)

- [ ] **Step 1: Create the bubblewrap module**

Create `crates/pi-sandbox-runtime/src/bubblewrap.rs`:

```rust
use std::path::PathBuf;

/// Whether Bubblewrap is available for sandboxed execution.
#[derive(Debug, Clone)]
pub enum BwrapAvailability {
    Available { path: PathBuf },
    Unavailable { reason: String },
}

/// Detect whether Bubblewrap is available on this platform.
///
/// Resolution order:
/// 1. `PI_SANDBOX_BWRAP_PATH` env var (if set and file exists)
/// 2. `which bwrap` on PATH (Linux only)
/// 3. Unavailable
///
/// On non-Linux platforms, always returns Unavailable.
pub fn detect() -> BwrapAvailability {
    #[cfg(not(target_os = "linux"))]
    {
        return BwrapAvailability::Unavailable {
            reason: "Bubblewrap requires Linux".to_string(),
        };
    }

    #[cfg(target_os = "linux")]
    {
        // 1. Check env var
        if let Ok(path_str) = std::env::var("PI_SANDBOX_BWRAP_PATH") {
            let path = PathBuf::from(&path_str);
            if path.exists() {
                return BwrapAvailability::Available { path };
            }
            return BwrapAvailability::Unavailable {
                reason: format!(
                    "PI_SANDBOX_BWRAP_PATH set to '{}' but file does not exist",
                    path_str
                ),
            };
        }

        // 2. Try which bwrap
        match std::process::Command::new("which")
            .arg("bwrap")
            .output()
        {
            Ok(output) if output.status.success() => {
                let path_str = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();
                let path = PathBuf::from(&path_str);
                if path.exists() {
                    return BwrapAvailability::Available { path };
                }
                BwrapAvailability::Unavailable {
                    reason: format!("which bwrap returned '{}' but file does not exist", path_str),
                }
            }
            _ => BwrapAvailability::Unavailable {
                reason: "bwrap not found on PATH".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_a_result() {
        // On any platform, detect() must return without panicking.
        let result = detect();
        match &result {
            BwrapAvailability::Available { path } => {
                assert!(path.exists());
            }
            BwrapAvailability::Unavailable { reason } => {
                assert!(!reason.is_empty());
            }
        }
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn non_linux_always_unavailable() {
        let result = detect();
        match result {
            BwrapAvailability::Unavailable { reason } => {
                assert!(reason.contains("Linux"), "reason: {}", reason);
            }
            BwrapAvailability::Available { .. } => {
                panic!("Should not be available on non-Linux");
            }
        }
    }
}
```

- [ ] **Step 2: Register the module in main.rs**

In `crates/pi-sandbox-runtime/src/main.rs`, add `mod bubblewrap;` to the module declarations. The top of the file should become:

```rust
mod bubblewrap;
mod contract;
mod observer;
mod supervisor;
mod timestamps;
mod validator;
```

- [ ] **Step 3: Run Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test 2>&1`
Expected: `bubblewrap::tests::detect_returns_a_result` passes. On macOS, `bubblewrap::tests::non_linux_always_unavailable` also passes.

- [ ] **Step 4: Verify protocol tests still pass**

Run: `cd tests/protocol && npx vitest run 2>&1`
Expected: All pass (no behavioral change yet).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-sandbox-runtime/src/bubblewrap.rs crates/pi-sandbox-runtime/src/main.rs
git commit -m "feat: add bubblewrap binary discovery module"
```

---

### Task 3: Create plan_builder.rs — bwrap argv construction

**Files:**
- Create: `crates/pi-sandbox-runtime/src/plan_builder.rs`
- Modify: `crates/pi-sandbox-runtime/src/main.rs:1` (add `mod plan_builder;`)

- [ ] **Step 1: Write the plan_builder module with tests**

Create `crates/pi-sandbox-runtime/src/plan_builder.rs`:

```rust
use crate::contract::{EffectiveNetwork, EffectiveState, PlanPayload};

/// Build the Bubblewrap argument vector from a validated plan and its effective state.
///
/// This is a pure function: no I/O, no side effects.
/// The returned Vec<String> is suitable for `Command::new("bwrap").args(result)`.
///
/// Construction order:
/// 1. Mounts (ro-bind / bind / tmpfs)
/// 2. Devices (hardcoded minimal set)
/// 3. Proc filesystem
/// 4. Namespaces
/// 5. Environment (clearenv + setenv)
/// 6. Working directory
/// 7. Command (after --)
pub fn build(plan: &PlanPayload, effective_state: &EffectiveState) -> Vec<String> {
    let mut argv: Vec<String> = Vec::new();

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
            _ => {
                // Unknown mount type — skip (validator should have caught this)
            }
        }
    }

    // 2. Devices — hardcoded minimal set
    for dev in &["/dev/null", "/dev/zero", "/dev/urandom", "/dev/random"] {
        argv.push("--dev-bind".to_string());
        argv.push(dev.to_string());
        argv.push(dev.to_string());
    }

    // 3. Proc filesystem
    argv.push("--proc".to_string());
    argv.push("/proc".to_string());

    // 4. Namespaces (from effective state, not requested)
    for ns in &effective_state.namespaces_applied {
        match ns.as_str() {
            "pid" => argv.push("--unshare-pid".to_string()),
            "ipc" => argv.push("--unshare-ipc".to_string()),
            "uts" => argv.push("--unshare-uts".to_string()),
            "net" => {
                // Only unshare network if actual mode is "off"
                if effective_state.network.actual == "off" {
                    argv.push("--unshare-net".to_string());
                }
            }
            "cgroup-try" => argv.push("--unshare-cgroup-try".to_string()),
            // "user" is implicit in bwrap — do not add --unshare-user
            "user" => {}
            _ => {
                // Unknown namespace — skip
            }
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

    // 7. Command (after --)
    argv.push("--".to_string());
    for part in &plan.command {
        argv.push(part.clone());
    }

    argv
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Manifest, Mount, NetworkConfig, Policy};
    use std::collections::HashMap;

    fn make_plan(overrides: Option<PlanOverrides>) -> PlanPayload {
        let o = overrides.unwrap_or_default();
        PlanPayload {
            version: 1,
            session_id: "test".to_string(),
            execution_id: "test".to_string(),
            requested_profile: "build-install".to_string(),
            runtime_base_name: None,
            manifest: Manifest {
                mounts: o.mounts.unwrap_or_else(|| vec![
                    Mount {
                        mount_type: "directory".to_string(),
                        source: Some("/host/workspace".to_string()),
                        target: "/workspace".to_string(),
                        writable: true,
                    },
                ]),
                env: o.env.unwrap_or_else(|| {
                    let mut m = HashMap::new();
                    m.insert("HOME".to_string(), "/home/sandbox".to_string());
                    m.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
                    m
                }),
                cwd: o.cwd.unwrap_or_else(|| "/workspace".to_string()),
            },
            policy: Policy {
                namespaces: vec!["user".to_string(), "pid".to_string()],
                network: NetworkConfig {
                    mode: o.network_mode.unwrap_or_else(|| "full".to_string()),
                    allowlist: None,
                },
                resource_limits: None,
                allowed_writable_targets: vec!["/workspace".to_string(), "/tmp".to_string()],
                strict_write_policy: false,
                env_allowlist: None,
                deny_commands: None,
            },
            command: o.command.unwrap_or_else(|| vec!["echo".to_string(), "hello".to_string()]),
        }
    }

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
        }
    }

    #[derive(Default)]
    struct PlanOverrides {
        mounts: Option<Vec<Mount>>,
        env: Option<HashMap<String, String>>,
        cwd: Option<String>,
        command: Option<Vec<String>>,
        network_mode: Option<String>,
    }

    #[derive(Default)]
    struct EffectiveOverrides {
        namespaces: Option<Vec<String>>,
        network_requested: Option<String>,
        network_actual: Option<String>,
        network_enforcement: Option<String>,
        network_degraded: Option<bool>,
    }

    #[test]
    fn read_only_directory_mount_produces_ro_bind() {
        let plan = make_plan(Some(PlanOverrides {
            mounts: Some(vec![Mount {
                mount_type: "directory".to_string(),
                source: Some("/host/src".to_string()),
                target: "/src".to_string(),
                writable: false,
            }]),
            ..Default::default()
        }));
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        let idx = argv.iter().position(|a| a == "--ro-bind").unwrap();
        assert_eq!(argv[idx + 1], "/host/src");
        assert_eq!(argv[idx + 2], "/src");
    }

    #[test]
    fn writable_directory_mount_produces_bind() {
        let plan = make_plan(Some(PlanOverrides {
            mounts: Some(vec![Mount {
                mount_type: "directory".to_string(),
                source: Some("/host/workspace".to_string()),
                target: "/workspace".to_string(),
                writable: true,
            }]),
            ..Default::default()
        }));
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        let idx = argv.iter().position(|a| a == "--bind").unwrap();
        assert_eq!(argv[idx + 1], "/host/workspace");
        assert_eq!(argv[idx + 2], "/workspace");
    }

    #[test]
    fn tmpfs_mount_produces_tmpfs() {
        let plan = make_plan(Some(PlanOverrides {
            mounts: Some(vec![Mount {
                mount_type: "tmpfs".to_string(),
                source: None,
                target: "/tmp".to_string(),
                writable: true,
            }]),
            ..Default::default()
        }));
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        let idx = argv.iter().position(|a| a == "--tmpfs").unwrap();
        assert_eq!(argv[idx + 1], "/tmp");
    }

    #[test]
    fn network_off_produces_unshare_net() {
        let plan = make_plan(Some(PlanOverrides {
            network_mode: Some("off".to_string()),
            ..Default::default()
        }));
        let state = make_effective_state(Some(EffectiveOverrides {
            namespaces: Some(vec!["user".to_string(), "pid".to_string(), "net".to_string()]),
            network_actual: Some("off".to_string()),
            network_enforcement: Some("enforced".to_string()),
            ..Default::default()
        }));
        let argv = build(&plan, &state);
        assert!(argv.contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn network_full_does_not_produce_unshare_net() {
        let plan = make_plan(None);
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        assert!(!argv.contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn env_produces_clearenv_and_setenv() {
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), "/home/test".to_string());
        let plan = make_plan(Some(PlanOverrides {
            env: Some(env),
            ..Default::default()
        }));
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        assert!(argv.contains(&"--clearenv".to_string()));
        let idx = argv.iter().position(|a| a == "--setenv").unwrap();
        assert_eq!(argv[idx + 1], "HOME");
        assert_eq!(argv[idx + 2], "/home/test");
    }

    #[test]
    fn cwd_produces_chdir() {
        let plan = make_plan(Some(PlanOverrides {
            cwd: Some("/my/cwd".to_string()),
            ..Default::default()
        }));
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        let idx = argv.iter().position(|a| a == "--chdir").unwrap();
        assert_eq!(argv[idx + 1], "/my/cwd");
    }

    #[test]
    fn devices_always_present() {
        let plan = make_plan(None);
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        // Count --dev-bind occurrences
        let dev_bind_count = argv.iter().filter(|a| a.as_str() == "--dev-bind").count();
        assert_eq!(dev_bind_count, 4); // null, zero, urandom, random
    }

    #[test]
    fn proc_always_present() {
        let plan = make_plan(None);
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        let idx = argv.iter().position(|a| a == "--proc").unwrap();
        assert_eq!(argv[idx + 1], "/proc");
    }

    #[test]
    fn command_is_last_after_separator() {
        let plan = make_plan(Some(PlanOverrides {
            command: Some(vec!["npm".to_string(), "install".to_string()]),
            ..Default::default()
        }));
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        let separator_idx = argv.iter().position(|a| a == "--").unwrap();
        assert_eq!(argv[separator_idx + 1], "npm");
        assert_eq!(argv[separator_idx + 2], "install");
        assert_eq!(separator_idx + 3, argv.len());
    }

    #[test]
    fn user_namespace_is_not_in_argv() {
        let plan = make_plan(None);
        let state = make_effective_state(Some(EffectiveOverrides {
            namespaces: Some(vec!["user".to_string(), "pid".to_string()]),
            ..Default::default()
        }));
        let argv = build(&plan, &state);
        assert!(!argv.contains(&"--unshare-user".to_string()));
        assert!(argv.contains(&"--unshare-pid".to_string()));
    }

    #[test]
    fn pid_ipc_uts_cgroup_namespaces() {
        let plan = make_plan(None);
        let state = make_effective_state(Some(EffectiveOverrides {
            namespaces: Some(vec![
                "pid".to_string(),
                "ipc".to_string(),
                "uts".to_string(),
                "cgroup-try".to_string(),
            ]),
            ..Default::default()
        }));
        let argv = build(&plan, &state);
        assert!(argv.contains(&"--unshare-pid".to_string()));
        assert!(argv.contains(&"--unshare-ipc".to_string()));
        assert!(argv.contains(&"--unshare-uts".to_string()));
        assert!(argv.contains(&"--unshare-cgroup-try".to_string()));
    }

    #[test]
    fn file_mount_uses_ro_bind_or_bind() {
        let plan = make_plan(Some(PlanOverrides {
            mounts: Some(vec![
                Mount {
                    mount_type: "file".to_string(),
                    source: Some("/etc/resolv.conf".to_string()),
                    target: "/etc/resolv.conf".to_string(),
                    writable: false,
                },
            ]),
            ..Default::default()
        }));
        let state = make_effective_state(None);
        let argv = build(&plan, &state);
        let idx = argv.iter().position(|a| a == "--ro-bind").unwrap();
        assert_eq!(argv[idx + 1], "/etc/resolv.conf");
        assert_eq!(argv[idx + 2], "/etc/resolv.conf");
    }
}
```

- [ ] **Step 2: Register the module in main.rs**

In `crates/pi-sandbox-runtime/src/main.rs`, add `mod plan_builder;` to the module declarations. The top should now be:

```rust
mod bubblewrap;
mod contract;
mod observer;
mod plan_builder;
mod supervisor;
mod timestamps;
mod validator;
```

- [ ] **Step 3: Run Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test 2>&1`
Expected: All `plan_builder::tests::*` tests pass (12 tests). All `bubblewrap::tests::*` tests also pass.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-sandbox-runtime/src/plan_builder.rs crates/pi-sandbox-runtime/src/main.rs
git commit -m "feat: add plan_builder module for bwrap argv construction"
```

---

### Task 4: Update validator.rs — namespace resolution and NAMESPACE_DEGRADED warnings

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/validator.rs`

- [ ] **Step 1: Update the `validate` function signature to accept bwrap availability**

In `crates/pi-sandbox-runtime/src/validator.rs`, change the imports and function signature. Replace the entire file with:

```rust
use crate::bubblewrap::BwrapAvailability;
use crate::contract::{
    EffectiveNetwork, EffectiveState, PlanPayload, ValidationError, ValidationPayload,
    ValidationWarning, PROTOCOL_VERSION,
};

/// Validate a PlanPayload and resolve effective state.
pub fn validate(plan: &PlanPayload, bwrap: &BwrapAvailability) -> ValidationPayload {
    // 1. Version check — early return
    if plan.version != PROTOCOL_VERSION {
        return ValidationPayload {
            ok: false,
            errors: vec![ValidationError {
                code: "VERSION_MISMATCH".to_string(),
                message: format!(
                    "Protocol version mismatch: expected {PROTOCOL_VERSION}, got {}",
                    plan.version
                ),
                field: Some("payload.version".to_string()),
            }],
            warnings: vec![],
            effective_state: None,
        };
    }

    let mut errors: Vec<ValidationError> = Vec::new();
    let mut warnings: Vec<ValidationWarning> = Vec::new();

    // 2. Empty command check
    if plan.command.is_empty() {
        errors.push(ValidationError {
            code: "MISSING_REQUIRED_FIELD".to_string(),
            message: "command must not be empty".to_string(),
            field: Some("payload.command".to_string()),
        });
    }

    // 3. Writable mounts against allowedWritableTargets
    for mount in &plan.manifest.mounts {
        if mount.writable
            && !plan
                .policy
                .allowed_writable_targets
                .iter()
                .any(|t| t == &mount.target)
        {
            errors.push(ValidationError {
                code: "RW_TARGET_NOT_ALLOWED".to_string(),
                message: format!(
                    "Writable mount target '{}' is not in allowedWritableTargets",
                    mount.target
                ),
                field: Some("payload.manifest.mounts".to_string()),
            });
        }
    }

    // 4. Denied commands check (only if command is non-empty)
    if !plan.command.is_empty() {
        if let Some(deny) = &plan.policy.deny_commands {
            if deny.iter().any(|d| d == &plan.command[0]) {
                errors.push(ValidationError {
                    code: "COMMAND_DENIED".to_string(),
                    message: format!("Command '{}' is denied by policy", plan.command[0]),
                    field: Some("payload.command".to_string()),
                });
            }
        }
    }

    // 5. Resolve effective network
    let effective_network = match plan.policy.network.mode.as_str() {
        "off" => EffectiveNetwork {
            requested: "off".to_string(),
            actual: "off".to_string(),
            enforcement: "enforced".to_string(),
            degraded: false,
        },
        "full" => EffectiveNetwork {
            requested: "full".to_string(),
            actual: "full".to_string(),
            enforcement: "none".to_string(),
            degraded: false,
        },
        "allowlist" => EffectiveNetwork {
            requested: "allowlist".to_string(),
            actual: "full".to_string(),
            enforcement: "observed".to_string(),
            degraded: true,
        },
        _ => EffectiveNetwork {
            requested: plan.policy.network.mode.clone(),
            actual: "full".to_string(),
            enforcement: "none".to_string(),
            degraded: false,
        },
    };

    // 6. Allowlist-degraded warning
    if effective_network.degraded {
        warnings.push(ValidationWarning {
            code: "ALLOWLIST_NOT_ENFORCED".to_string(),
            message:
                "Network allowlist requested but cannot be enforced; running in observed mode"
                    .to_string(),
        });
    }

    // 7. Resolve namespaces based on bwrap availability
    let namespaces_applied = match bwrap {
        BwrapAvailability::Available { .. } => {
            // All requested namespaces can be applied (bwrap handles them)
            plan.policy.namespaces.clone()
        }
        BwrapAvailability::Unavailable { .. } => {
            // No namespaces can be applied — emit warnings
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

    // 8. Resolve applied environment keys
    let env_applied: Vec<String> = if let Some(allowlist) = &plan.policy.env_allowlist {
        plan.manifest
            .env
            .keys()
            .filter(|k| allowlist.contains(k))
            .cloned()
            .collect()
    } else {
        plan.manifest.env.keys().cloned().collect()
    };

    let effective_state = Some(EffectiveState {
        network: effective_network,
        namespaces_applied,
        env_applied,
    });

    ValidationPayload {
        ok: errors.is_empty(),
        errors,
        warnings,
        effective_state,
    }
}
```

- [ ] **Step 2: Update the `validate` call site in main.rs**

In `crates/pi-sandbox-runtime/src/main.rs`, after the plan is extracted (around line 55), add bwrap detection and update the validate call. Replace from line 57 onward (starting at `// 4. Validate the plan.`):

The full `main.rs` should now be:

```rust
mod bubblewrap;
mod contract;
mod observer;
mod plan_builder;
mod supervisor;
mod timestamps;
mod validator;

use std::io::{self, BufRead};
use std::sync::mpsc;

use contract::{
    emit, InboundMessage, ReconciliationHints, ResultEnvelope, ResultPayload, ValidationEnvelope,
    ValidationError, ValidationPayload,
};

fn main() {
    let stdin = io::stdin();
    let mut first_line = String::new();

    // 1. Read exactly one line from stdin and parse it as an InboundMessage.
    if stdin.lock().read_line(&mut first_line).is_err() {
        eprintln!("pi-sandbox-runtime: failed to read from stdin");
        std::process::exit(1);
    }

    let first_line = first_line.trim();

    // 2. On parse error: emit PARSE_ERROR validation, exit.
    let message: InboundMessage = match serde_json::from_str(first_line) {
        Ok(m) => m,
        Err(e) => {
            emit(&ValidationEnvelope::new(ValidationPayload {
                ok: false,
                errors: vec![ValidationError {
                    code: "PARSE_ERROR".to_string(),
                    message: format!("Failed to parse inbound message: {e}"),
                    field: None,
                }],
                warnings: vec![],
                effective_state: None,
            }));
            std::process::exit(0);
        }
    };

    // 3. On Cancel before Plan: log to stderr, exit.
    let plan = match message {
        InboundMessage::Plan { payload } => payload,
        InboundMessage::Cancel { payload } => {
            eprintln!(
                "pi-sandbox-runtime: received Cancel before Plan: reason={:?}",
                payload.reason
            );
            std::process::exit(0);
        }
    };

    // 4. Detect Bubblewrap availability.
    let bwrap = bubblewrap::detect();

    // 5. Validate the plan.
    let validation = validator::validate(&plan, &bwrap);

    // 6. Emit validation message.
    emit(&ValidationEnvelope::new(validation.clone()));

    // 7. If validation failed: exit(0).
    if !validation.ok {
        std::process::exit(0);
    }

    // 8. Clone effectiveState from validation (safe: ok == true guarantees it's Some).
    let effective_state = validation
        .effective_state
        .expect("effectiveState must be Some when ok=true");

    // 9. Spawn background thread to read remaining stdin lines for cancel signal.
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(text) = line else { break };
            let text = text.trim().to_string();
            if text.is_empty() {
                continue;
            }
            match serde_json::from_str::<InboundMessage>(&text) {
                Ok(InboundMessage::Cancel { .. }) => {
                    let _ = cancel_tx.send(());
                    break;
                }
                _ => {
                    // Ignore unknown messages during execution.
                }
            }
        }
    });

    // 10. Run the supervisor.
    let result = supervisor::supervise(&plan, &effective_state, cancel_rx, &bwrap);

    // 11. Emit final ResultEnvelope from supervision result.
    emit(&ResultEnvelope::new(ResultPayload {
        exit_code: result.exit_code,
        signal: result.signal,
        timed_out: result.timed_out,
        duration_ms: result.duration_ms,
        effective_network: result.effective_network,
        observed_connections: result.observed_connections,
        would_have_blocked: result.would_have_blocked,
        resource_peaks: None,
        reconciliation_hints: ReconciliationHints {
            terminal_state: result.terminal_state,
            workspace_modified: result.workspace_modified,
            cleanup_succeeded: true,
        },
    }));

    // 12. Exit.
    std::process::exit(0);
}
```

- [ ] **Step 3: Verify compilation (will fail — supervisor signature not updated yet)**

Run: `cd crates/pi-sandbox-runtime && cargo build --release 2>&1`
Expected: Compilation fails with error about `supervisor::supervise` not accepting `&BwrapAvailability`. This is expected — Task 5 fixes it.

Note: If implementing tasks sequentially, proceed to Task 5 to fix the supervisor. If this task is being implemented standalone, you may stub the bwrap parameter temporarily to get compilation passing.

- [ ] **Step 4: Commit (partial — validator and main updated, supervisor fix in next task)**

```bash
git add crates/pi-sandbox-runtime/src/validator.rs crates/pi-sandbox-runtime/src/main.rs
git commit -m "feat: update validator for namespace resolution and NAMESPACE_DEGRADED warnings"
```

---

### Task 5: Update supervisor.rs — bwrap dispatch with direct-execution fallback

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/supervisor.rs`

- [ ] **Step 1: Update supervisor to accept bwrap availability and dispatch accordingly**

Replace the entire contents of `crates/pi-sandbox-runtime/src/supervisor.rs` with:

```rust
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::bubblewrap::BwrapAvailability;
use crate::contract::{
    emit, EffectiveNetwork, EffectiveState, LifecycleEnvelope, ObservedConnection,
    StderrEnvelope, StdoutEnvelope, PlanPayload,
};
use crate::observer::{compute_would_have_blocked, observe_connections};
use crate::plan_builder;

pub struct SupervisionResult {
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub timed_out: bool,
    pub duration_ms: f64,
    pub effective_network: EffectiveNetwork,
    pub observed_connections: Vec<ObservedConnection>,
    pub would_have_blocked: Vec<crate::contract::BlockedConnection>,
    pub terminal_state: String,
    pub workspace_modified: bool,
}

/// Supervise execution of the plan's command.
pub fn supervise(
    plan: &PlanPayload,
    effective_state: &EffectiveState,
    cancel_rx: Receiver<()>,
    bwrap: &BwrapAvailability,
) -> SupervisionResult {
    // Shared atomic sequence number across all output threads.
    let seq = Arc::new(AtomicU64::new(0));

    // Helper: fetch-and-increment sequence number.
    let next_seq = |counter: &AtomicU64| counter.fetch_add(1, Ordering::SeqCst);

    // Emit lifecycle: started
    emit(&LifecycleEnvelope::new(
        next_seq(&seq),
        "started".to_string(),
    ));

    let start = Instant::now();

    // Build the child process — either via bwrap or direct execution.
    let mut cmd = match bwrap {
        BwrapAvailability::Available { path } => {
            let argv = plan_builder::build(plan, effective_state);
            let mut c = Command::new(path);
            c.args(&argv);
            c
        }
        BwrapAvailability::Unavailable { .. } => {
            let mut c = Command::new(&plan.command[0]);
            if plan.command.len() > 1 {
                c.args(&plan.command[1..]);
            }
            c.current_dir(&plan.manifest.cwd)
                .envs(&plan.manifest.env);
            c
        }
    };

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            // Emit an error lifecycle and return immediately.
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
    };

    // Take ownership of stdout/stderr pipes.
    let child_stdout = child.stdout.take().expect("stdout was piped");
    let child_stderr = child.stderr.take().expect("stderr was piped");

    // Spawn stdout reader thread.
    let seq_stdout = Arc::clone(&seq);
    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(child_stdout);
        for line in reader.lines() {
            match line {
                Ok(text) => {
                    let s = seq_stdout.fetch_add(1, Ordering::SeqCst);
                    emit(&StdoutEnvelope::new(s, text));
                }
                Err(_) => break,
            }
        }
    });

    // Spawn stderr reader thread.
    let seq_stderr = Arc::clone(&seq);
    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(child_stderr);
        for line in reader.lines() {
            match line {
                Ok(text) => {
                    let s = seq_stderr.fetch_add(1, Ordering::SeqCst);
                    emit(&StderrEnvelope::new(s, text));
                }
                Err(_) => break,
            }
        }
    });

    // Poll the cancel channel while the child is running.
    let mut cancelled = false;
    let exit_status = loop {
        // Check for cancel signal (non-blocking).
        if cancel_rx.try_recv().is_ok() {
            cancelled = true;
            let s = seq.fetch_add(1, Ordering::SeqCst);
            emit(&LifecycleEnvelope::new(s, "cancel_requested".to_string()));

            // Kill the process.
            #[cfg(unix)]
            {
                let pid = child.id();
                unsafe {
                    libc::kill(-(pid as libc::pid_t), libc::SIGTERM);
                }
            }
            #[cfg(not(unix))]
            {
                let _ = child.kill();
            }

            let s2 = seq.fetch_add(1, Ordering::SeqCst);
            emit(&LifecycleEnvelope::new(s2, "killing".to_string()));

            // Wait for the child to exit after signaling.
            break child.wait().ok();
        }

        // Check if the child has already exited (non-blocking).
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => break None,
        }
    };

    // Wait for I/O threads to finish draining output.
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Parse exit code / signal.
    let (exit_code, signal) = match exit_status {
        Some(status) => {
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
                (status.code(), None)
            }
        }
        None => (None, None),
    };

    // Determine terminal state.
    let terminal_state = if cancelled {
        "killed_on_cancel".to_string()
    } else if signal.is_some() {
        "killed_on_timeout".to_string()
    } else {
        "clean_exit".to_string()
    };

    // Emit lifecycle: exited
    let s = seq.fetch_add(1, Ordering::SeqCst);
    emit(&LifecycleEnvelope::new(s, "exited".to_string()));

    // Network observation (stub for now — Phase 10 replaces this).
    let observed = observe_connections();
    let would_have_blocked =
        compute_would_have_blocked(&observed, &plan.policy.network.allowlist);

    SupervisionResult {
        exit_code,
        signal,
        timed_out: false,
        duration_ms,
        effective_network: effective_state.network.clone(),
        observed_connections: observed,
        would_have_blocked,
        terminal_state,
        workspace_modified: false,
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd crates/pi-sandbox-runtime && cargo build --release 2>&1`
Expected: Builds successfully.

- [ ] **Step 3: Run all Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test 2>&1`
Expected: All plan_builder and bubblewrap tests pass.

- [ ] **Step 4: Run protocol tests**

Run: `cd tests/protocol && npx vitest run 2>&1`
Expected: All 6 tests pass (7 individual tests). On macOS, supervisor falls back to direct execution (same behavior as before). The validation messages now include `namespacesApplied: []` and `envApplied: [...]` but existing tests don't assert these fields negatively.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-sandbox-runtime/src/supervisor.rs
git commit -m "feat: update supervisor for bwrap dispatch with direct-execution fallback"
```

---

### Task 6: Add bwrap-integration protocol test (Linux-only)

**Files:**
- Create: `tests/protocol/bwrap-integration.test.ts`

- [ ] **Step 1: Write the bwrap integration test**

Create `tests/protocol/bwrap-integration.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

describe("Protocol Test 7: Bwrap Integration (Linux only)", () => {
  const isLinux = process.platform === "linux";

  it.skipIf(!isLinux)("runs command via bwrap with namespaces applied", async () => {
    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "bwrap-test"],
        manifest: {
          mounts: [
            { type: "tmpfs", target: "/tmp", writable: true },
          ],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid", "ipc"],
          network: { mode: "full" },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    // Validation must succeed
    const validation = events[0];
    expect(validation.type).toBe("validation");
    const validationPayload = validation.payload as any;
    expect(validationPayload.ok).toBe(true);

    // namespacesApplied must include the requested namespaces (bwrap available)
    expect(validationPayload.effectiveState.namespacesApplied).toContain("user");
    expect(validationPayload.effectiveState.namespacesApplied).toContain("pid");
    expect(validationPayload.effectiveState.namespacesApplied).toContain("ipc");

    // envApplied must include the env keys
    expect(validationPayload.effectiveState.envApplied).toContain("HOME");
    expect(validationPayload.effectiveState.envApplied).toContain("PATH");

    // Execution must succeed
    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    const resultPayload = result.payload as any;
    expect(resultPayload.exitCode).toBe(0);
    expect(resultPayload.reconciliationHints.terminalState).toBe("clean_exit");

    // Find stdout with "bwrap-test"
    const stdoutEvents = events.filter((e) => e.type === "stdout");
    const bwrapOutput = stdoutEvents.find(
      (e) => ((e.payload as any).data as string).includes("bwrap-test"),
    );
    expect(bwrapOutput).toBeDefined();

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });

  it.skipIf(isLinux)("falls back to direct execution on non-Linux with NAMESPACE_DEGRADED warnings", async () => {
    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "fallback-test"],
        manifest: {
          mounts: [
            { type: "tmpfs", target: "/tmp", writable: true },
          ],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid"],
          network: { mode: "full" },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    const validation = events[0];
    expect(validation.type).toBe("validation");
    const validationPayload = validation.payload as any;
    expect(validationPayload.ok).toBe(true);

    // namespacesApplied must be empty (no bwrap)
    expect(validationPayload.effectiveState.namespacesApplied).toEqual([]);

    // Must have NAMESPACE_DEGRADED warnings
    const nsWarnings = (validationPayload.warnings as any[]).filter(
      (w: any) => w.code === "NAMESPACE_DEGRADED",
    );
    expect(nsWarnings.length).toBe(2); // one for "user", one for "pid"

    // envApplied must still be populated
    expect(validationPayload.effectiveState.envApplied).toContain("HOME");
    expect(validationPayload.effectiveState.envApplied).toContain("PATH");

    // Execution must still succeed (direct execution fallback)
    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    expect((result.payload as any).exitCode).toBe(0);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
```

- [ ] **Step 2: Run protocol tests**

Run: `cd tests/protocol && npx vitest run 2>&1`
Expected: On macOS, the Linux test is skipped, the fallback test passes. All original tests still pass.

- [ ] **Step 3: Commit**

```bash
git add tests/protocol/bwrap-integration.test.ts
git commit -m "test: add bwrap integration protocol test with macOS fallback"
```

---

## Phase 9: Real Build Flows (Integration Tests)

### Task 7: Create integration test scaffolding

**Files:**
- Create: `tests/integration/package.json`
- Create: `tests/integration/tsconfig.json`
- Create: `tests/integration/vitest.config.ts`
- Create: `tests/integration/globalSetup.ts`

- [ ] **Step 1: Create package.json**

Create `tests/integration/package.json`:

```json
{
  "name": "@pi-sandbox/integration-tests",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest",
    "test:network": "RUN_NETWORK_TESTS=1 vitest run network-smoke.test.ts"
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
    "module": "ES2022",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "outDir": "dist",
    "rootDir": ".",
    "declaration": false
  },
  "include": ["*.ts", "**/*.ts"],
  "exclude": ["node_modules", "dist", "fixtures"]
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
    testTimeout: 120000, // 2 minutes — build commands can be slow
  },
});
```

- [ ] **Step 4: Create globalSetup.ts**

Create `tests/integration/globalSetup.ts`:

```typescript
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const CRATE_DIR = resolve(import.meta.dirname, "../../crates/pi-sandbox-runtime");

export async function setup() {
  console.log("Building pi-sandbox-runtime for integration tests...");
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

- [ ] **Step 5: Install dependencies**

Run: `cd tests/integration && npm install 2>&1`
Expected: `node_modules/` created with vitest and typescript.

- [ ] **Step 6: Commit**

```bash
git add tests/integration/package.json tests/integration/tsconfig.json tests/integration/vitest.config.ts tests/integration/globalSetup.ts
git commit -m "feat: scaffold integration test infrastructure"
```

---

### Task 8: Create integration test helpers and fixture repos

**Files:**
- Create: `tests/integration/helpers.ts`
- Create: `tests/integration/fixtures/tiny-npm/package.json`
- Create: `tests/integration/fixtures/tiny-python/setup.py`
- Create: `tests/integration/fixtures/tiny-python/mypackage/__init__.py`
- Create: `tests/integration/fixtures/tiny-rust/Cargo.toml`
- Create: `tests/integration/fixtures/tiny-rust/src/main.rs`

- [ ] **Step 1: Create integration test helpers**

Create `tests/integration/helpers.ts`:

```typescript
import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdtempSync, cpSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// ---------------------------------------------------------------------------
// TestRuntime — identical to protocol test helpers
// ---------------------------------------------------------------------------

export interface TestRuntime {
  send(message: Record<string, unknown>): void;
  readline(): Promise<Record<string, unknown>>;
  readAllEvents(): Promise<Record<string, unknown>[]>;
  kill(signal?: NodeJS.Signals): void;
  waitForExit(): Promise<{ code: number | null; signal: string | null }>;
  stderr: string;
  process: ChildProcess;
}

export function spawnRuntime(): TestRuntime {
  const binaryPath = process.env.RUNTIME_BINARY_PATH;
  if (!binaryPath) {
    throw new Error("RUNTIME_BINARY_PATH not set. Did globalSetup run?");
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

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

const FIXTURES_DIR = resolve(import.meta.dirname, "fixtures");

export interface FixtureWorkspace {
  /** Absolute path to the temp workspace directory (contains the fixture files) */
  workspaceDir: string;
  /** Clean up the temp directory */
  cleanup: () => void;
}

/**
 * Copy a fixture into a fresh temp directory.
 * Returns the temp dir path and a cleanup function.
 */
export function copyFixture(fixtureName: string): FixtureWorkspace {
  const fixtureDir = join(FIXTURES_DIR, fixtureName);
  const tempDir = mkdtempSync(join(tmpdir(), `pi-sandbox-integ-${fixtureName}-`));
  cpSync(fixtureDir, tempDir, { recursive: true });
  return {
    workspaceDir: tempDir,
    cleanup: () => rmSync(tempDir, { recursive: true, force: true }),
  };
}

/**
 * Build a plan message for integration tests.
 *
 * Sets up:
 * - workspace mounted writable at the temp dir
 * - /tmp as tmpfs
 * - build-install profile defaults
 * - inherits current process PATH so commands (npm, pip, cargo) are found
 */
export function makeIntegrationPlan(opts: {
  workspaceDir: string;
  command: string[];
  networkMode?: string;
}): Record<string, unknown> {
  const currentPath = process.env.PATH ?? "/usr/bin:/bin";
  return {
    type: "plan",
    payload: {
      version: 1,
      sessionId: "integ-session-001",
      executionId: "integ-exec-001",
      requestedProfile: "build-install",
      runtimeBaseName: "host-derived",
      manifest: {
        mounts: [
          {
            type: "directory",
            source: opts.workspaceDir,
            target: opts.workspaceDir,
            writable: true,
          },
          {
            type: "tmpfs",
            target: "/tmp",
            writable: true,
          },
        ],
        env: {
          HOME: opts.workspaceDir,
          PATH: currentPath,
        },
        cwd: opts.workspaceDir,
      },
      policy: {
        namespaces: ["user", "pid"],
        network: {
          mode: opts.networkMode ?? "full",
        },
        allowedWritableTargets: [opts.workspaceDir, "/tmp"],
        strictWritePolicy: false,
        envAllowlist: ["HOME", "PATH"],
        denyCommands: [],
      },
      command: opts.command,
    },
  };
}
```

- [ ] **Step 2: Create tiny-npm fixture**

Create `tests/integration/fixtures/tiny-npm/package.json`:

```json
{
  "name": "tiny-npm-fixture",
  "version": "1.0.0",
  "private": true,
  "dependencies": {}
}
```

- [ ] **Step 3: Create tiny-python fixture**

Create `tests/integration/fixtures/tiny-python/setup.py`:

```python
from setuptools import setup

setup(
    name="tiny-python-fixture",
    version="1.0.0",
    packages=["mypackage"],
)
```

Create `tests/integration/fixtures/tiny-python/mypackage/__init__.py`:

```python
"""Tiny fixture package."""
```

- [ ] **Step 4: Create tiny-rust fixture**

Create `tests/integration/fixtures/tiny-rust/Cargo.toml`:

```toml
[package]
name = "tiny-rust-fixture"
version = "0.1.0"
edition = "2021"
```

Create `tests/integration/fixtures/tiny-rust/src/main.rs`:

```rust
fn main() {
    println!("built");
}
```

- [ ] **Step 5: Commit**

```bash
git add tests/integration/helpers.ts tests/integration/fixtures/
git commit -m "feat: add integration test helpers and fixture repos"
```

---

### Task 9: Write build-npm integration test

**Files:**
- Create: `tests/integration/build-npm.test.ts`

- [ ] **Step 1: Write the test**

Create `tests/integration/build-npm.test.ts`:

```typescript
import { describe, expect, it, afterEach } from "vitest";
import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  spawnRuntime,
  copyFixture,
  makeIntegrationPlan,
  type FixtureWorkspace,
} from "./helpers.js";

describe("Integration: npm install", () => {
  let fixture: FixtureWorkspace | null = null;

  afterEach(() => {
    fixture?.cleanup();
    fixture = null;
  });

  it("runs npm install on tiny-npm fixture and exits cleanly", async () => {
    fixture = copyFixture("tiny-npm");
    const rt = spawnRuntime();

    rt.send(
      makeIntegrationPlan({
        workspaceDir: fixture.workspaceDir,
        command: ["npm", "install"],
      }),
    );

    const events = await rt.readAllEvents();

    // Validation must pass
    const validation = events[0];
    expect(validation.type).toBe("validation");
    expect((validation.payload as any).ok).toBe(true);

    // Result must be clean exit
    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    const resultPayload = result.payload as any;
    expect(resultPayload.exitCode).toBe(0);
    expect(resultPayload.reconciliationHints.terminalState).toBe("clean_exit");

    // npm install should have created a package-lock.json
    expect(
      existsSync(join(fixture.workspaceDir, "package-lock.json")),
    ).toBe(true);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
```

- [ ] **Step 2: Run the test**

Run: `cd tests/integration && npx vitest run build-npm.test.ts 2>&1`
Expected: PASS — npm install on an empty project completes quickly with exit 0.

- [ ] **Step 3: Commit**

```bash
git add tests/integration/build-npm.test.ts
git commit -m "test: add npm install integration test"
```

---

### Task 10: Write build-python integration test

**Files:**
- Create: `tests/integration/build-python.test.ts`

- [ ] **Step 1: Write the test**

Create `tests/integration/build-python.test.ts`:

```typescript
import { describe, expect, it, afterEach } from "vitest";
import { execFileSync } from "node:child_process";
import {
  spawnRuntime,
  copyFixture,
  makeIntegrationPlan,
  type FixtureWorkspace,
} from "./helpers.js";

// Check if python3 and pip are available
function hasPython(): boolean {
  try {
    execFileSync("python3", ["--version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

describe("Integration: pip install", () => {
  let fixture: FixtureWorkspace | null = null;

  afterEach(() => {
    fixture?.cleanup();
    fixture = null;
  });

  it.skipIf(!hasPython())(
    "runs pip install -e . on tiny-python fixture and exits cleanly",
    async () => {
      fixture = copyFixture("tiny-python");
      const rt = spawnRuntime();

      rt.send(
        makeIntegrationPlan({
          workspaceDir: fixture.workspaceDir,
          command: ["pip", "install", "-e", ".", "--break-system-packages"],
        }),
      );

      const events = await rt.readAllEvents();

      // Validation must pass
      const validation = events[0];
      expect(validation.type).toBe("validation");
      expect((validation.payload as any).ok).toBe(true);

      // Result must be clean exit
      const result = events[events.length - 1];
      expect(result.type).toBe("result");
      const resultPayload = result.payload as any;
      expect(resultPayload.exitCode).toBe(0);
      expect(resultPayload.reconciliationHints.terminalState).toBe(
        "clean_exit",
      );

      const exit = await rt.waitForExit();
      expect(exit.code).toBe(0);
    },
  );
});
```

- [ ] **Step 2: Run the test**

Run: `cd tests/integration && npx vitest run build-python.test.ts 2>&1`
Expected: PASS if python3/pip are available, SKIP if not.

- [ ] **Step 3: Commit**

```bash
git add tests/integration/build-python.test.ts
git commit -m "test: add pip install integration test"
```

---

### Task 11: Write build-rust integration test

**Files:**
- Create: `tests/integration/build-rust.test.ts`

- [ ] **Step 1: Write the test**

Create `tests/integration/build-rust.test.ts`:

```typescript
import { describe, expect, it, afterEach } from "vitest";
import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  spawnRuntime,
  copyFixture,
  makeIntegrationPlan,
  type FixtureWorkspace,
} from "./helpers.js";

describe("Integration: cargo build", () => {
  let fixture: FixtureWorkspace | null = null;

  afterEach(() => {
    fixture?.cleanup();
    fixture = null;
  });

  it("runs cargo build on tiny-rust fixture and exits cleanly", async () => {
    fixture = copyFixture("tiny-rust");
    const rt = spawnRuntime();

    rt.send(
      makeIntegrationPlan({
        workspaceDir: fixture.workspaceDir,
        command: ["cargo", "build"],
      }),
    );

    const events = await rt.readAllEvents();

    // Validation must pass
    const validation = events[0];
    expect(validation.type).toBe("validation");
    expect((validation.payload as any).ok).toBe(true);

    // Result must be clean exit
    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    const resultPayload = result.payload as any;
    expect(resultPayload.exitCode).toBe(0);
    expect(resultPayload.reconciliationHints.terminalState).toBe("clean_exit");

    // cargo build should have created target/ directory
    expect(existsSync(join(fixture.workspaceDir, "target"))).toBe(true);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
```

- [ ] **Step 2: Run the test**

Run: `cd tests/integration && npx vitest run build-rust.test.ts 2>&1`
Expected: PASS — cargo builds a tiny no-dep crate successfully.

- [ ] **Step 3: Commit**

```bash
git add tests/integration/build-rust.test.ts
git commit -m "test: add cargo build integration test"
```

---

### Task 12: Write network smoke test (optional, skipped by default)

**Files:**
- Create: `tests/integration/network-smoke.test.ts`

- [ ] **Step 1: Write the test**

Create `tests/integration/network-smoke.test.ts`:

```typescript
import { describe, expect, it, afterEach } from "vitest";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { spawnRuntime, makeIntegrationPlan } from "./helpers.js";

const RUN_NETWORK_TESTS = process.env.RUN_NETWORK_TESTS === "1";

describe("Integration: Network Smoke Tests", () => {
  let tempDir: string | null = null;

  afterEach(() => {
    if (tempDir) {
      rmSync(tempDir, { recursive: true, force: true });
      tempDir = null;
    }
  });

  it.skipIf(!RUN_NETWORK_TESTS)(
    "npm install with real network fetches a dependency",
    async () => {
      // Create a temp workspace with a real dependency
      tempDir = mkdtempSync(join(tmpdir(), "pi-sandbox-network-"));
      writeFileSync(
        join(tempDir, "package.json"),
        JSON.stringify({
          name: "network-smoke-test",
          version: "1.0.0",
          private: true,
          dependencies: {
            "is-odd": "3.0.1",
          },
        }),
      );

      const rt = spawnRuntime();

      rt.send(
        makeIntegrationPlan({
          workspaceDir: tempDir,
          command: ["npm", "install"],
          networkMode: "full",
        }),
      );

      const events = await rt.readAllEvents();

      // Validation must pass
      const validation = events[0];
      expect(validation.type).toBe("validation");
      expect((validation.payload as any).ok).toBe(true);

      // Result must be clean exit
      const result = events[events.length - 1];
      expect(result.type).toBe("result");
      const resultPayload = result.payload as any;
      expect(resultPayload.exitCode).toBe(0);
      expect(resultPayload.reconciliationHints.terminalState).toBe(
        "clean_exit",
      );

      // Dependency must have been installed
      expect(existsSync(join(tempDir, "node_modules", "is-odd"))).toBe(true);

      const exit = await rt.waitForExit();
      expect(exit.code).toBe(0);
    },
  );
});
```

- [ ] **Step 2: Run the test (should skip)**

Run: `cd tests/integration && npx vitest run network-smoke.test.ts 2>&1`
Expected: Test is skipped (RUN_NETWORK_TESTS not set).

- [ ] **Step 3: Commit**

```bash
git add tests/integration/network-smoke.test.ts
git commit -m "test: add optional network smoke test (skipped by default)"
```

---

### Task 13: Run full integration test suite

**Files:** None (validation only)

- [ ] **Step 1: Run all integration tests**

Run: `cd tests/integration && npx vitest run 2>&1`
Expected: build-npm passes, build-python passes (or skips if no python), build-rust passes, network-smoke skips.

- [ ] **Step 2: Run protocol tests to confirm no regressions**

Run: `cd tests/protocol && npx vitest run 2>&1`
Expected: All 8 tests pass (6 original + 2 new from bwrap-integration on macOS where one runs and one skips).

- [ ] **Step 3: Commit (no files — this is a verification step)**

No commit needed. If tests fail, fix the issue in the relevant file and re-run.

---

## Phase 10: Network Observation

### Task 14: Replace observer.rs stub with NetworkObserver and `/proc/net/tcp` parser

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/observer.rs`

- [ ] **Step 1: Rewrite observer.rs with the NetworkObserver implementation**

Replace the entire contents of `crates/pi-sandbox-runtime/src/observer.rs` with:

```rust
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::contract::{emit, BlockedConnection, NetworkEnvelope, ObservedConnection};

/// Background network observer that polls /proc/net/tcp for outbound connections.
///
/// On Linux: polls at ~500ms intervals, deduplicates, emits network events.
/// On non-Linux: no-op (returns empty results immediately).
pub struct NetworkObserver {
    handle: Option<JoinHandle<Vec<ObservedConnection>>>,
    stop_flag: Arc<AtomicBool>,
}

impl NetworkObserver {
    /// Start the observer. On Linux, spawns a polling thread.
    /// On non-Linux, returns a no-op observer.
    pub fn start(seq: Arc<AtomicU64>) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));

        #[cfg(target_os = "linux")]
        {
            let flag = Arc::clone(&stop_flag);
            let handle = thread::spawn(move || poll_loop(flag, seq));
            NetworkObserver {
                handle: Some(handle),
                stop_flag,
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = seq; // unused on non-Linux
            NetworkObserver {
                handle: None,
                stop_flag,
            }
        }
    }

    /// Stop the observer and return all observed connections.
    pub fn stop(self) -> Vec<ObservedConnection> {
        self.stop_flag.store(true, Ordering::Relaxed);
        match self.handle {
            Some(h) => h.join().unwrap_or_default(),
            None => vec![],
        }
    }
}

/// The polling loop (Linux only).
#[cfg(target_os = "linux")]
fn poll_loop(stop_flag: Arc<AtomicBool>, seq: Arc<AtomicU64>) -> Vec<ObservedConnection> {
    let mut seen: HashSet<(String, u16)> = HashSet::new();
    let mut results: Vec<ObservedConnection> = Vec::new();

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        if let Ok(connections) = parse_proc_net_tcp("/proc/net/tcp") {
            for conn in connections {
                if seen.insert((conn.host.clone(), conn.port)) {
                    let s = seq.fetch_add(1, Ordering::SeqCst);
                    emit(&NetworkEnvelope::new(
                        s,
                        "outbound".to_string(),
                        conn.host.clone(),
                        conn.port,
                        Some("tcp".to_string()),
                    ));
                    results.push(conn);
                }
            }
        }

        thread::sleep(Duration::from_millis(500));
    }

    results
}

/// Parse /proc/net/tcp and return outbound established connections.
///
/// Each line after the header has format:
///   sl  local_address rem_address   st ...
///   0: 0100007F:1F90 0100007F:C000 01 ...
///
/// We extract rem_address (field 2), filter to state 01 (ESTABLISHED),
/// and exclude loopback (127.x.x.x) and unspecified (0.0.0.0).
#[cfg(target_os = "linux")]
fn parse_proc_net_tcp(path: &str) -> std::io::Result<Vec<ObservedConnection>> {
    use std::io::{BufRead, BufReader};
    use std::fs::File;

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut connections = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        // Skip header (line 0)
        if i == 0 {
            continue;
        }

        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 {
            continue;
        }

        // State must be 01 (ESTABLISHED)
        let state = fields[3];
        if state != "01" {
            continue;
        }

        // Parse remote address (field 2): "HEXIP:HEXPORT"
        let rem_addr = fields[2];
        let parts: Vec<&str> = rem_addr.split(':').collect();
        if parts.len() != 2 {
            continue;
        }

        let ip_hex = parts[0];
        let port_hex = parts[1];

        // Parse IP (little-endian on x86)
        let ip_u32 = match u32::from_str_radix(ip_hex, 16) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let a = (ip_u32 & 0xFF) as u8;
        let b = ((ip_u32 >> 8) & 0xFF) as u8;
        let c = ((ip_u32 >> 16) & 0xFF) as u8;
        let d = ((ip_u32 >> 24) & 0xFF) as u8;

        // Filter loopback and unspecified
        if a == 127 || (a == 0 && b == 0 && c == 0 && d == 0) {
            continue;
        }

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
///
/// The allowlist contains entries in "host:port" format.
/// A connection is "would-have-blocked" if it is not matched by any allowlist entry.
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
    #[cfg(target_os = "linux")]
    fn parse_proc_net_tcp_on_linux() {
        // This test reads the actual /proc/net/tcp
        let result = parse_proc_net_tcp("/proc/net/tcp");
        assert!(result.is_ok());
        // We can't assert specific connections, just that parsing doesn't panic
    }

    #[test]
    fn network_observer_noop_on_stop() {
        // Start and immediately stop — should return empty on all platforms
        let seq = Arc::new(AtomicU64::new(0));
        let observer = NetworkObserver::start(seq);
        let connections = observer.stop();
        // On non-Linux, always empty. On Linux, may or may not be empty depending on timing.
        // Just assert it doesn't panic.
        let _ = connections;
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd crates/pi-sandbox-runtime && cargo build --release 2>&1`
Expected: Builds successfully.

- [ ] **Step 3: Run Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test 2>&1`
Expected: All observer tests pass. On macOS, the Linux-specific `parse_proc_net_tcp_on_linux` test is compiled out.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-sandbox-runtime/src/observer.rs
git commit -m "feat: replace observer stub with NetworkObserver and /proc/net/tcp parser"
```

---

### Task 15: Wire NetworkObserver into supervisor.rs

**Files:**
- Modify: `crates/pi-sandbox-runtime/src/supervisor.rs`

- [ ] **Step 1: Replace the stub `observe_connections()` call with `NetworkObserver`**

In `crates/pi-sandbox-runtime/src/supervisor.rs`, make these changes:

Replace the import line:

```rust
use crate::observer::{compute_would_have_blocked, observe_connections};
```

With:

```rust
use crate::observer::{compute_would_have_blocked, NetworkObserver};
```

Then, after the child process is spawned (after the `let mut child = match cmd.spawn()` block succeeds), add the observer start. Find the line:

```rust
    // Take ownership of stdout/stderr pipes.
    let child_stdout = child.stdout.take().expect("stdout was piped");
```

Insert before it:

```rust
    // Start network observer (Linux: polls /proc/net/tcp; non-Linux: no-op).
    let observer = NetworkObserver::start(Arc::clone(&seq));

```

Then, replace the network observation block near the end (after the `// Emit lifecycle: exited` block). Find:

```rust
    // Network observation (stub for now — Phase 10 replaces this).
    let observed = observe_connections();
    let would_have_blocked =
        compute_would_have_blocked(&observed, &plan.policy.network.allowlist);
```

Replace with:

```rust
    // Stop observer and collect observed connections.
    let observed = observer.stop();
    let would_have_blocked =
        compute_would_have_blocked(&observed, &plan.policy.network.allowlist);
```

- [ ] **Step 2: Verify compilation**

Run: `cd crates/pi-sandbox-runtime && cargo build --release 2>&1`
Expected: Builds successfully.

- [ ] **Step 3: Run all Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test 2>&1`
Expected: All pass.

- [ ] **Step 4: Run protocol tests**

Run: `cd tests/protocol && npx vitest run 2>&1`
Expected: All pass. On macOS, observer is no-op so behavior is identical.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-sandbox-runtime/src/supervisor.rs
git commit -m "feat: wire NetworkObserver into supervisor for live connection tracking"
```

---

### Task 16: Add network observation protocol test (Linux-only)

**Files:**
- Create: `tests/protocol/network-observation.test.ts`

- [ ] **Step 1: Write the test**

Create `tests/protocol/network-observation.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

const isLinux = process.platform === "linux";

describe("Protocol Test 8: Network Observation (Linux only)", () => {
  it.skipIf(!isLinux)(
    "observes outbound connections during execution",
    async () => {
      const rt = spawnRuntime();

      // Use curl or python to make an outbound connection
      rt.send(
        makePlan({
          command: [
            "python3",
            "-c",
            "import urllib.request; urllib.request.urlopen('http://example.com')",
          ],
          manifest: {
            mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
            env: {
              HOME: "/home/sandbox",
              PATH: "/usr/bin:/bin:/usr/local/bin",
            },
            cwd: "/tmp",
          },
          policy: {
            namespaces: ["user", "pid"],
            network: { mode: "full" },
            allowedWritableTargets: ["/workspace", "/tmp"],
            strictWritePolicy: false,
          },
        }),
      );

      const events = await rt.readAllEvents();

      // Validation must succeed
      const validation = events[0];
      expect(validation.type).toBe("validation");
      expect((validation.payload as any).ok).toBe(true);

      // Should have at least one network event
      const networkEvents = events.filter((e) => e.type === "network");
      expect(networkEvents.length).toBeGreaterThanOrEqual(1);

      // Network event should have expected shape
      const firstNet = networkEvents[0];
      expect((firstNet.payload as any).direction).toBe("outbound");
      expect((firstNet.payload as any).port).toBeGreaterThan(0);
      expect(typeof (firstNet.payload as any).host).toBe("string");

      // Result should have observedConnections
      const result = events[events.length - 1];
      expect(result.type).toBe("result");
      const resultPayload = result.payload as any;
      expect(resultPayload.observedConnections.length).toBeGreaterThanOrEqual(
        1,
      );

      expect(resultPayload.exitCode).toBe(0);

      const exit = await rt.waitForExit();
      expect(exit.code).toBe(0);
    },
  );

  it.skipIf(isLinux)(
    "returns empty observations on non-Linux (no-op observer)",
    async () => {
      const rt = spawnRuntime();

      rt.send(
        makePlan({
          command: ["echo", "no-network-needed"],
          manifest: {
            mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
            env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
            cwd: "/tmp",
          },
        }),
      );

      const events = await rt.readAllEvents();

      // No network events on non-Linux
      const networkEvents = events.filter((e) => e.type === "network");
      expect(networkEvents.length).toBe(0);

      // Result should have empty observedConnections
      const result = events[events.length - 1];
      expect(result.type).toBe("result");
      const resultPayload = result.payload as any;
      expect(resultPayload.observedConnections).toEqual([]);

      const exit = await rt.waitForExit();
      expect(exit.code).toBe(0);
    },
  );
});
```

- [ ] **Step 2: Run protocol tests**

Run: `cd tests/protocol && npx vitest run 2>&1`
Expected: On macOS, the Linux test skips and the non-Linux test passes. All other tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/protocol/network-observation.test.ts
git commit -m "test: add network observation protocol test (Linux/macOS)"
```

---

### Task 17: Final verification — all tests pass

**Files:** None (verification only)

- [ ] **Step 1: Run all Rust tests**

Run: `cd crates/pi-sandbox-runtime && cargo test 2>&1`
Expected: All pass (plan_builder, bubblewrap, observer tests).

- [ ] **Step 2: Run protocol tests**

Run: `cd tests/protocol && npx vitest run 2>&1`
Expected: All pass (original 6 + bwrap-integration + network-observation).

- [ ] **Step 3: Run integration tests**

Run: `cd tests/integration && npx vitest run 2>&1`
Expected: All pass (npm, python, rust builds; network-smoke skipped).

- [ ] **Step 4: Verify Rust compilation has no errors**

Run: `cd crates/pi-sandbox-runtime && cargo build --release 2>&1`
Expected: Compiles cleanly (warnings about unused fields are acceptable).

- [ ] **Step 5: Tag the milestone**

```bash
git tag v1-phases-8-10-complete
```
