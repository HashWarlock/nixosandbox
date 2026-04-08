use crate::contract::PlanPayload;

/// Rewrite a single host path to its container-side equivalent.
///
/// If the path starts with `host_prefix`, replace that prefix with `container_prefix`.
/// Otherwise return the path unchanged.
pub fn rewrite_path(path: &str, host_prefix: &str, container_prefix: &str) -> String {
    if path.starts_with(host_prefix) {
        path.replacen(host_prefix, container_prefix, 1)
    } else {
        path.to_string()
    }
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
