use std::io::Write;
use std::process::{Command, Stdio};

const SIDECAR_NAME: &str = "nixosandbox-sidecar";
const IMAGE_NAME: &str = "nixosandbox-sidecar:latest";
/// Mount point for the host data dir inside the container.
/// Note: this is the data dir mount point, not the sessions subdir.
/// Sessions live at `<CONTAINER_DATA_MOUNT>/sessions/<id>/...` inside the container.
const CONTAINER_DATA_MOUNT: &str = "/nixosandbox/sessions";

const BUILDER_IMAGE_NAME: &str = "nixosandbox-builder:latest";
/// Persistent Docker volume storing the Nix store for macOS builds.
/// Shared between the builder (writes) and the sidecar (reads).
const NIX_VOLUME_NAME: &str = "nixosandbox-nix";

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
    let flake_root = crate::nix::find_flake_root()
        .unwrap_or_else(|_| ".".to_string());
    let dockerfile = format!("{}/docker/nixosandbox-sidecar.Dockerfile", flake_root);
    let output = Command::new("docker")
        .args([
            "build",
            "--platform", "linux/amd64",
            "-t", IMAGE_NAME,
            "-f", &dockerfile,
            &flake_root,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("docker build failed: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("docker build failed: {}", stderr.trim()))
    }
}

/// Create and start a new sidecar container.
///
/// Mounts the shared `nixosandbox-nix` Docker volume at `/nix` so bwrap can
/// access rootfs derivations built by the Docker-based builder.
fn create_sidecar(host_sessions_dir: &str) -> Result<String, String> {
    let sessions_volume = format!("{host_sessions_dir}:{CONTAINER_DATA_MOUNT}");
    let nix_volume = format!("{NIX_VOLUME_NAME}:/nix:ro");
    let output = Command::new("docker")
        .args([
            "run", "-d",
            "--platform", "linux/amd64",
            "--name", SIDECAR_NAME,
            "--cap-add", "SYS_ADMIN",
            "--cap-add", "NET_ADMIN",
            "--security-opt", "seccomp=unconfined",
            "-v", &sessions_volume,
            "-v", &nix_volume,
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

// ---------------------------------------------------------------------------
// Docker-based Nix builder (macOS)
// ---------------------------------------------------------------------------

/// Build the builder Docker image if it doesn't already exist.
/// Uses an inline Dockerfile piped via stdin to avoid path-resolution issues.
fn ensure_builder_image() -> Result<(), String> {
    let output = Command::new("docker")
        .args(["images", BUILDER_IMAGE_NAME, "--format", "{{.ID}}"])
        .output()
        .map_err(|e| format!("docker images check failed: {e}"))?;

    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !id.is_empty() {
        return Ok(());
    }

    eprintln!("nixosandbox: building Docker builder image (one-time setup)...");

    // Inline Dockerfile — avoids needing to locate the file on disk.
    let dockerfile = concat!(
        "FROM nixos/nix:latest\n",
        "RUN echo 'experimental-features = nix-command flakes' >> /etc/nix/nix.conf \\\n",
        " && echo 'extra-substituters = https://cache.numtide.com' >> /etc/nix/nix.conf \\\n",
        " && echo 'extra-trusted-public-keys = niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=' >> /etc/nix/nix.conf \\\n",
        " && echo 'filter-syscalls = false' >> /etc/nix/nix.conf\n",
    );

    let mut child = Command::new("docker")
        .args([
            "build",
            "--platform", "linux/amd64",
            "-t", BUILDER_IMAGE_NAME,
            "-f", "-",   // read Dockerfile from stdin
            "/tmp",       // build context (unused — Dockerfile has no COPY)
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("docker build (builder): {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(dockerfile.as_bytes())
            .map_err(|e| format!("writing builder Dockerfile to stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("docker build (builder) wait: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("docker build (builder) failed: {}", stderr.trim()))
    }
}

/// Run `nix build <flake-attr>` inside a Docker container.
///
/// The builder uses a persistent Docker volume (`nixosandbox-nix`) for `/nix`,
/// so subsequent builds are incremental. The flake root is bind-mounted read-only.
pub fn nix_build_in_docker(flake_attr: &str, flake_root: &str) -> Result<String, String> {
    if !is_docker_available() {
        return Err(
            "Docker not available. On macOS, Docker Desktop is required to build Linux sandboxes.\n\
             Install Docker Desktop from https://www.docker.com/products/docker-desktop/".to_string()
        );
    }

    ensure_builder_image()?;

    let flake_mount = format!("{}:{}:ro", flake_root, flake_root);
    let nix_volume = format!("{}:/nix", NIX_VOLUME_NAME);

    let output = Command::new("docker")
        .args([
            "run", "--rm",
            "--platform", "linux/amd64",
            "-v", &flake_mount,
            "-v", &nix_volume,
            BUILDER_IMAGE_NAME,
            "nix", "build", flake_attr,
            "--no-link", "--print-out-paths",
            "--accept-flake-config",
            "--extra-experimental-features", "nix-command flakes",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("docker run (nix build): {e}"))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Err("nix build in Docker produced no output".into())
        } else {
            Ok(path)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("nix build in Docker failed: {}", stderr))
    }
}

/// Run `nix build --impure --expr <expr>` inside a Docker container.
pub fn nix_build_expr_in_docker(expr: &str, flake_root: &str) -> Result<String, String> {
    if !is_docker_available() {
        return Err(
            "Docker not available. On macOS, Docker Desktop is required to build Linux sandboxes.\n\
             Install Docker Desktop from https://www.docker.com/products/docker-desktop/".to_string()
        );
    }

    ensure_builder_image()?;

    let flake_mount = format!("{}:{}:ro", flake_root, flake_root);
    let nix_volume = format!("{}:/nix", NIX_VOLUME_NAME);

    let output = Command::new("docker")
        .args([
            "run", "--rm",
            "--platform", "linux/amd64",
            "-v", &flake_mount,
            "-v", &nix_volume,
            BUILDER_IMAGE_NAME,
            "nix", "build", "--impure", "--expr", expr,
            "--no-link", "--print-out-paths",
            "--accept-flake-config",
            "--extra-experimental-features", "nix-command flakes",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("docker run (nix build --expr): {e}"))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Err("nix build --expr in Docker produced no output".into())
        } else {
            Ok(path)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("nix build --expr in Docker failed: {}", stderr))
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
