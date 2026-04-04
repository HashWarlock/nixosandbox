mod contract;
mod timestamps;

use std::io::{self, BufRead};

use contract::{
    emit, InboundMessage, EffectiveNetwork, EffectiveState, ValidationEnvelope, ValidationError,
    ValidationPayload, PROTOCOL_VERSION,
};

fn main() {
    let stdin = io::stdin();
    let mut line = String::new();

    if stdin.lock().read_line(&mut line).is_err() {
        eprintln!("Failed to read from stdin");
        std::process::exit(1);
    }

    let line = line.trim();

    // Parse the inbound message.
    let message: InboundMessage = match serde_json::from_str(line) {
        Ok(m) => m,
        Err(e) => {
            let msg = ValidationEnvelope::new(ValidationPayload {
                ok: false,
                errors: vec![ValidationError {
                    code: "PARSE_ERROR".to_string(),
                    message: format!("Failed to parse inbound message: {e}"),
                    field: None,
                }],
                warnings: vec![],
                effective_state: None,
            });
            emit(&msg);
            return;
        }
    };

    match message {
        InboundMessage::Plan { payload } => {
            // Check protocol version.
            if payload.version != PROTOCOL_VERSION {
                let msg = ValidationEnvelope::new(ValidationPayload {
                    ok: false,
                    errors: vec![ValidationError {
                        code: "VERSION_MISMATCH".to_string(),
                        message: format!(
                            "Protocol version mismatch: expected {PROTOCOL_VERSION}, got {}",
                            payload.version
                        ),
                        field: Some("payload.version".to_string()),
                    }],
                    warnings: vec![],
                    effective_state: None,
                });
                emit(&msg);
                return;
            }

            // Resolve effective network mode.
            let effective_mode = if payload.policy.network.mode == "off" {
                "off".to_string()
            } else {
                "full".to_string()
            };

            let effective_network = EffectiveNetwork {
                mode: effective_mode,
                allowlist: payload.policy.network.allowlist,
            };

            let msg = ValidationEnvelope::new(ValidationPayload {
                ok: true,
                errors: vec![],
                warnings: vec![],
                effective_state: Some(EffectiveState {
                    network: effective_network,
                }),
            });
            emit(&msg);
        }

        InboundMessage::Cancel { payload } => {
            eprintln!(
                "Received Cancel before Plan: reason={:?}",
                payload.reason
            );
        }
    }
}
