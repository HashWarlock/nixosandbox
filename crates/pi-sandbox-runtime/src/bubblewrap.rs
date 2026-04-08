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
    fn non_linux_always_unavailable() {
        let result = detect();
        match result {
            BwrapAvailability::Unavailable { reason } => {
                assert!(reason.contains("Linux"), "reason: {}", reason);
            }
            BwrapAvailability::Available { .. } => {
                panic!("Should not be available on non-Linux");
            }
            BwrapAvailability::DockerAvailable { .. } => {
                panic!("DockerAvailable should not be returned by detect() on non-Linux (Task 7 not yet wired)");
            }
        }
    }
}
