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
