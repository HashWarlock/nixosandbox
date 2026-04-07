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
        resolved_allowlist: vec![],
    });

    ValidationPayload {
        ok: errors.is_empty(),
        errors,
        warnings,
        effective_state,
    }
}
