use std::path::PathBuf;

/// Whether Bubblewrap is available for sandboxed execution.
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

/// Detect whether Bubblewrap is available on this platform.
///
/// Resolution order:
/// 1. `NIXOSANDBOX_BWRAP_PATH` env var (if set and file exists)
/// 2. `which bwrap` on PATH (Linux only)
/// 3. Unavailable
///
/// On non-Linux platforms, always returns Unavailable.
pub fn detect() -> BwrapAvailability {
    #[cfg(not(target_os = "linux"))]
    {
        // Check opt-out env var
        if std::env::var("NIXOSANDBOX_NO_DOCKER").map_or(false, |v| v == "1") {
            return BwrapAvailability::Unavailable {
                reason: "Docker fallback disabled via NIXOSANDBOX_NO_DOCKER=1".to_string(),
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

    #[cfg(target_os = "linux")]
    {
        // 1. Check env var
        if let Ok(path_str) = std::env::var("NIXOSANDBOX_BWRAP_PATH") {
            let path = PathBuf::from(&path_str);
            if path.exists() {
                return BwrapAvailability::Available { path };
            }
            return BwrapAvailability::Unavailable {
                reason: format!(
                    "NIXOSANDBOX_BWRAP_PATH set to '{}' but file does not exist",
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
}
