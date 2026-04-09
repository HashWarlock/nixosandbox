use std::process::{Command, Stdio};

const SIDECAR_NAME: &str = "nixosandbox-sidecar";
const IMAGE_NAME: &str = "nixosandbox-sidecar:latest";
/// Mount point for the host data dir inside the container.
/// Note: this is the data dir mount point, not the sessions subdir.
/// Sessions live at `<CONTAINER_DATA_MOUNT>/sessions/<id>/...` inside the container.
const CONTAINER_DATA_MOUNT: &str = "/nixosandbox/sessions";

/// Information about a running Docker sidecar container.
pub struct DockerSidecar {
    pub container_id: String,
    /// Host data directory (e.g. `~/.local/share/nixosandbox`), mounted into the container.
    pub host_sessions_dir: String,
    /// Container-side mount point for the host data dir.
    pub container_sessions_dir: String,
}

/// Get the nixosandbox data directory on the host.
///
/// Uses `NIXOSANDBOX_DATA_DIR` env var if set, otherwise `$HOME/.local/share/nixosandbox`.
fn get_data_dir() -> Result<String, String> {
    if let Ok(dir) = std::env::var("NIXOSANDBOX_DATA_DIR") {
        return Ok(dir);
    }
    let home = std::env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    Ok(format!("{home}/.local/share/nixosandbox"))
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

    eprintln!("nixosandbox: building Docker sidecar image (one-time setup)...");
    let status = Command::new("docker")
        .args([
            "build", "-t", IMAGE_NAME,
            "-f", "docker/nixosandbox-sidecar.Dockerfile", ".",
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
    let sessions_volume = format!("{host_sessions_dir}:{CONTAINER_DATA_MOUNT}");
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
            container_sessions_dir: CONTAINER_DATA_MOUNT.to_string(),
        });
    }

    // 2. Check if container exists but is stopped
    if let Some(id) = find_stopped_sidecar() {
        start_container(&id)?;
        return Ok(DockerSidecar {
            container_id: id,
            host_sessions_dir,
            container_sessions_dir: CONTAINER_DATA_MOUNT.to_string(),
        });
    }

    // 3. Container doesn't exist — build image and create it
    ensure_image()?;
    let id = create_sidecar(&host_sessions_dir)?;

    Ok(DockerSidecar {
        container_id: id,
        host_sessions_dir,
        container_sessions_dir: CONTAINER_DATA_MOUNT.to_string(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_path_replaces_matching_prefix() {
        let result = rewrite_path(
            "/Users/me/.local/share/nixosandbox/sessions/abc/workspace",
            "/Users/me/.local/share/nixosandbox/sessions",
            "/nixosandbox/sessions",
        );
        assert_eq!(result, "/nixosandbox/sessions/abc/workspace");
    }

    #[test]
    fn rewrite_path_leaves_non_matching_path_unchanged() {
        let result = rewrite_path(
            "/nix/store/abc123-sandbox-strict",
            "/Users/me/.local/share/nixosandbox/sessions",
            "/nixosandbox/sessions",
        );
        assert_eq!(result, "/nix/store/abc123-sandbox-strict");
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

}
