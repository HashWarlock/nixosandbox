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

/// Create and start a new sidecar container.
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
