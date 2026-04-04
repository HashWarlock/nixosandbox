mod bubblewrap;
mod contract;
mod observer;
mod supervisor;
mod timestamps;
mod validator;

use std::io::{self, BufRead};
use std::sync::mpsc;

use contract::{
    emit, InboundMessage, ReconciliationHints, ResultEnvelope, ResultPayload, ValidationEnvelope,
    ValidationError, ValidationPayload,
};

fn main() {
    let stdin = io::stdin();
    let mut first_line = String::new();

    // 1. Read exactly one line from stdin and parse it as an InboundMessage.
    if stdin.lock().read_line(&mut first_line).is_err() {
        eprintln!("pi-sandbox-runtime: failed to read from stdin");
        std::process::exit(1);
    }

    let first_line = first_line.trim();

    // 2. On parse error: emit PARSE_ERROR validation, exit.
    let message: InboundMessage = match serde_json::from_str(first_line) {
        Ok(m) => m,
        Err(e) => {
            emit(&ValidationEnvelope::new(ValidationPayload {
                ok: false,
                errors: vec![ValidationError {
                    code: "PARSE_ERROR".to_string(),
                    message: format!("Failed to parse inbound message: {e}"),
                    field: None,
                }],
                warnings: vec![],
                effective_state: None,
            }));
            std::process::exit(0);
        }
    };

    // 3. On Cancel before Plan: log to stderr, exit.
    let plan = match message {
        InboundMessage::Plan { payload } => payload,
        InboundMessage::Cancel { payload } => {
            eprintln!(
                "pi-sandbox-runtime: received Cancel before Plan: reason={:?}",
                payload.reason
            );
            std::process::exit(0);
        }
    };

    // 4. Validate the plan.
    let validation = validator::validate(&plan);

    // 5. Emit validation message.
    emit(&ValidationEnvelope::new(validation.clone()));

    // 6. If validation failed: exit(0).
    if !validation.ok {
        std::process::exit(0);
    }

    // 7. Clone effectiveState from validation (safe: ok == true guarantees it's Some).
    let effective_state = validation
        .effective_state
        .expect("effectiveState must be Some when ok=true");

    // 8. Spawn background thread to read remaining stdin lines for cancel signal.
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(text) = line else { break };
            let text = text.trim().to_string();
            if text.is_empty() {
                continue;
            }
            match serde_json::from_str::<InboundMessage>(&text) {
                Ok(InboundMessage::Cancel { .. }) => {
                    let _ = cancel_tx.send(());
                    break;
                }
                _ => {
                    // Ignore unknown messages during execution.
                }
            }
        }
    });

    // 9. Run the supervisor.
    let result = supervisor::supervise(&plan, &effective_state, cancel_rx);

    // 10. Emit final ResultEnvelope from supervision result.
    emit(&ResultEnvelope::new(ResultPayload {
        exit_code: result.exit_code,
        signal: result.signal,
        timed_out: result.timed_out,
        duration_ms: result.duration_ms,
        effective_network: result.effective_network,
        observed_connections: result.observed_connections,
        would_have_blocked: result.would_have_blocked,
        resource_peaks: None,
        reconciliation_hints: ReconciliationHints {
            terminal_state: result.terminal_state,
            workspace_modified: result.workspace_modified,
            cleanup_succeeded: true,
        },
    }));

    // 11. Exit.
    std::process::exit(0);
}
