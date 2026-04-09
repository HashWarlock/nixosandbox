use std::net::ToSocketAddrs;
use std::path::PathBuf;

use crate::bubblewrap::BwrapAvailability;
use crate::contract::{
    EffectiveNetwork, EffectiveState, PlanPayload, ResolvedAllowlistEntry,
    ValidationError, ValidationPayload, ValidationWarning, PROTOCOL_VERSION,
};

/// Resolve a hostname to IP addresses using system DNS.
fn resolve_hostname(hostname: &str) -> Vec<String> {
    let addr = format!("{hostname}:0");
    match addr.to_socket_addrs() {
        Ok(addrs) => addrs.map(|a| a.ip().to_string()).collect::<Vec<_>>(),
        Err(_) => vec![],
    }
}

/// Check if iptables binary is available on the host.
fn detect_iptables() -> Option<PathBuf> {
    #[cfg(not(target_os = "linux"))]
    {
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        match std::process::Command::new("which")
            .arg("iptables")
            .output()
        {
            Ok(output) if output.status.success() => {
                let path_str = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();
                let path = PathBuf::from(&path_str);
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

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
    let has_net_namespace = plan.policy.namespaces.iter().any(|ns| ns == "net");
    let bwrap_available = matches!(
        bwrap,
        BwrapAvailability::Available { .. } | BwrapAvailability::DockerAvailable { .. }
    );

    let (effective_network, resolved_allowlist) = match plan.policy.network.mode.as_str() {
        "off" => {
            let enforcement = if bwrap_available && has_net_namespace {
                "enforced"
            } else {
                "best_effort"
            };
            let degraded = enforcement != "enforced";
            (
                EffectiveNetwork {
                    requested: "off".to_string(),
                    actual: "off".to_string(),
                    enforcement: enforcement.to_string(),
                    degraded,
                },
                vec![],
            )
        }
        "full" => (
            EffectiveNetwork {
                requested: "full".to_string(),
                actual: "full".to_string(),
                enforcement: "observed".to_string(),
                degraded: false,
            },
            vec![],
        ),
        "allowlist" => {
            let allowlist_hosts = plan
                .policy
                .network
                .allowlist
                .as_deref()
                .unwrap_or(&[]);

            // Resolve DNS for each hostname
            let mut entries: Vec<ResolvedAllowlistEntry> = Vec::new();
            for hostname in allowlist_hosts {
                let ips = resolve_hostname(hostname);
                let resolved = !ips.is_empty();
                if !resolved {
                    warnings.push(ValidationWarning {
                        code: "DNS_RESOLUTION_PARTIAL".to_string(),
                        message: format!(
                            "Failed to resolve allowlist hostname '{hostname}'"
                        ),
                    });
                }
                entries.push(ResolvedAllowlistEntry {
                    hostname: hostname.clone(),
                    ips,
                    resolved,
                });
            }

            let any_resolved = entries.iter().any(|e| e.resolved);
            let iptables_path = detect_iptables();

            let can_enforce = bwrap_available
                && has_net_namespace
                && any_resolved
                && iptables_path.is_some();

            if !bwrap_available || !has_net_namespace {
                warnings.push(ValidationWarning {
                    code: "ALLOWLIST_NOT_ENFORCED".to_string(),
                    message:
                        "Network allowlist requested but cannot be enforced; running in observed mode"
                            .to_string(),
                });
            } else if iptables_path.is_none() {
                warnings.push(ValidationWarning {
                    code: "IPTABLES_NOT_FOUND".to_string(),
                    message:
                        "iptables binary not found on host; allowlist degraded to full/observed"
                            .to_string(),
                });
            } else if !any_resolved {
                warnings.push(ValidationWarning {
                    code: "ALLOWLIST_DNS_FAILED".to_string(),
                    message:
                        "All allowlist hostnames failed DNS resolution; degraded to full/observed"
                            .to_string(),
                });
            }

            if can_enforce {
                (
                    EffectiveNetwork {
                        requested: "allowlist".to_string(),
                        actual: "allowlist".to_string(),
                        enforcement: "enforced".to_string(),
                        degraded: false,
                    },
                    entries,
                )
            } else {
                (
                    EffectiveNetwork {
                        requested: "allowlist".to_string(),
                        actual: "full".to_string(),
                        enforcement: "observed".to_string(),
                        degraded: true,
                    },
                    entries,
                )
            }
        }
        _ => (
            EffectiveNetwork {
                requested: plan.policy.network.mode.clone(),
                actual: "full".to_string(),
                enforcement: "none".to_string(),
                degraded: false,
            },
            vec![],
        ),
    };

    // 6. Resolve namespaces based on bwrap availability
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

    // 7. Resolve applied environment keys
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

    let isolation_backend = match bwrap {
        BwrapAvailability::Available { .. } => "native".to_string(),
        BwrapAvailability::DockerAvailable { .. } => "docker".to_string(),
        BwrapAvailability::Unavailable { .. } => "none".to_string(),
    };

    // On non-Linux, if bwrap is unavailable (Docker not found), emit a warning
    #[cfg(not(target_os = "linux"))]
    if matches!(bwrap, BwrapAvailability::Unavailable { .. }) {
        warnings.push(ValidationWarning {
            code: "DOCKER_NOT_AVAILABLE".to_string(),
            message: "macOS detected but Docker not available; running without isolation"
                .to_string(),
        });
    }

    let effective_state = Some(EffectiveState {
        network: effective_network,
        namespaces_applied,
        env_applied,
        resolved_allowlist,
        isolation_backend,
    });

    ValidationPayload {
        ok: errors.is_empty(),
        errors,
        warnings,
        effective_state,
    }
}
