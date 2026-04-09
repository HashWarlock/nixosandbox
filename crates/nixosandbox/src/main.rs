mod bubblewrap;
mod cli;
mod contract;
mod docker;
mod observer;
mod plan_builder;
mod session;
mod spec;
mod supervisor;
mod timestamps;
mod validator;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create { .. } => {
            eprintln!("nixosandbox: create not yet implemented");
            std::process::exit(1);
        }
        Commands::Exec { .. } => {
            eprintln!("nixosandbox: exec not yet implemented");
            std::process::exit(1);
        }
        Commands::Enter { .. } => {
            eprintln!("nixosandbox: enter not yet implemented");
            std::process::exit(1);
        }
        Commands::List { .. } => {
            eprintln!("nixosandbox: list not yet implemented");
            std::process::exit(1);
        }
        Commands::Destroy { .. } => {
            eprintln!("nixosandbox: destroy not yet implemented");
            std::process::exit(1);
        }
        Commands::Build { .. } => {
            eprintln!("nixosandbox: build not yet implemented");
            std::process::exit(1);
        }
        Commands::LegacyNdjson => {
            legacy_ndjson_main();
        }
    }
}

/// The original NDJSON subprocess entry point (preserved for Pi backward compat).
fn legacy_ndjson_main() {
    use std::io::{self, BufRead};
    use std::sync::mpsc;
    use contract::{
        emit, InboundMessage, ReconciliationHints, ResultEnvelope, ResultPayload,
        ValidationEnvelope, ValidationError, ValidationPayload,
    };

    let stdin = io::stdin();
    let mut first_line = String::new();
    if stdin.lock().read_line(&mut first_line).is_err() {
        eprintln!("nixosandbox: failed to read from stdin");
        std::process::exit(1);
    }
    let first_line = first_line.trim();
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
    let plan = match message {
        InboundMessage::Plan { payload } => payload,
        InboundMessage::Cancel { payload } => {
            eprintln!("nixosandbox: received Cancel before Plan: reason={:?}", payload.reason);
            std::process::exit(0);
        }
    };
    let bwrap = bubblewrap::detect();
    let validation = validator::validate(&plan, &bwrap);
    emit(&ValidationEnvelope::new(validation.clone()));
    if !validation.ok {
        std::process::exit(0);
    }
    let effective_state = validation.effective_state.expect("effectiveState must be Some when ok=true");
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(text) = line else { break };
            let text = text.trim().to_string();
            if text.is_empty() { continue; }
            match serde_json::from_str::<InboundMessage>(&text) {
                Ok(InboundMessage::Cancel { .. }) => { let _ = cancel_tx.send(()); break; }
                _ => {}
            }
        }
    });
    let result = supervisor::supervise(&plan, &effective_state, cancel_rx, &bwrap);
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
    std::process::exit(0);
}
