mod bubblewrap;
mod cli;
mod contract;
mod docker;
mod nix;
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
        Commands::Create { profile, spec: spec_file, workspace, name, json } => {
            cmd_create(profile, spec_file, workspace, name, json);
        }
        Commands::Exec { session_id, json, timeout: _timeout, extra_env, command } => {
            cmd_exec(&session_id, json, extra_env, command);
        }
        Commands::Enter { session_id } => {
            cmd_enter(&session_id);
        }
        Commands::List { json } => {
            cmd_list(json);
        }
        Commands::Destroy { session_id } => {
            cmd_destroy(&session_id);
        }
        Commands::Build { profile, spec: spec_file, json } => {
            cmd_build(profile, spec_file, json);
        }
        Commands::LegacyNdjson => {
            legacy_ndjson_main();
        }
    }
}

fn resolve_spec(profile: Option<String>, spec_file: Option<String>) -> spec::SandboxSpec {
    match (profile, spec_file) {
        (Some(p), None) => {
            let flake_root = nix::find_flake_root().unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });
            spec::load_profile(&p, &flake_root).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            })
        }
        (None, Some(s)) => {
            spec::load_spec(&s).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            })
        }
        (Some(_), Some(_)) => {
            eprintln!("error: specify --profile or --spec, not both");
            std::process::exit(1);
        }
        (None, None) => {
            eprintln!("error: specify --profile or --spec");
            std::process::exit(1);
        }
    }
}

fn build_rootfs_for_spec(spec: &spec::SandboxSpec, profile: &Option<String>) -> String {
    if let Err(errors) = spec::validate_spec(spec) {
        for e in &errors {
            eprintln!("validation error: {e}");
        }
        std::process::exit(1);
    }
    let rootfs = if let Some(p) = profile {
        nix::build_profile(p)
    } else {
        nix::build_spec(spec)
    };
    rootfs.unwrap_or_else(|e| {
        eprintln!("nix build failed: {e}");
        std::process::exit(1);
    })
}

fn cmd_create(profile: Option<String>, spec_file: Option<String>, workspace: Option<String>, name: Option<String>, json: bool) {
    let sandbox_spec = resolve_spec(profile.clone(), spec_file);
    let rootfs_path = build_rootfs_for_spec(&sandbox_spec, &profile);

    nix::validate_rootfs(&rootfs_path).unwrap_or_else(|e| {
        eprintln!("rootfs validation failed: {e}");
        std::process::exit(1);
    });

    let session_name = name.unwrap_or_else(|| sandbox_spec.name.clone());
    let meta = session::create_session(
        &session_name,
        &sandbox_spec.name,
        &rootfs_path,
        workspace.as_deref(),
    ).unwrap_or_else(|e| {
        eprintln!("session creation failed: {e}");
        std::process::exit(1);
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&meta).unwrap());
    } else {
        println!("{}", meta.session_id);
    }
}

fn cmd_exec(session_id: &str, json: bool, extra_env: Vec<String>, command: Vec<String>) {
    if command.is_empty() {
        eprintln!("error: no command specified (use -- <command>)");
        std::process::exit(1);
    }

    let meta = session::load_session(session_id).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    let flake_root = nix::find_flake_root().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    // Load the spec/profile to get network and namespace config
    let sandbox_spec = spec::load_profile(&meta.profile, &flake_root).unwrap_or_else(|e| {
        eprintln!("warning: could not load profile '{}': {e}", meta.profile);
        // Fallback spec
        spec::SandboxSpec {
            name: meta.profile.clone(),
            packages: vec![],
            env: std::collections::HashMap::new(),
            network: "full".to_string(),
            namespaces: vec!["pid".to_string(), "mount".to_string(), "uts".to_string(), "ipc".to_string()],
            writable: vec!["/workspace".to_string(), "/home/sandbox".to_string(), "/cache".to_string(), "/tmp".to_string()],
        }
    });

    let dirs = session::session_dirs(session_id);

    // Merge extra env vars
    let mut env = sandbox_spec.env.clone();
    for kv in &extra_env {
        if let Some((k, v)) = kv.split_once('=') {
            env.insert(k.to_string(), v.to_string());
        } else {
            eprintln!("warning: ignoring invalid --env value: {kv}");
        }
    }

    let rootfs_dirs = plan_builder::RootfsSessionDirs {
        workspace: dirs.workspace.to_string_lossy().to_string(),
        home: dirs.home.to_string_lossy().to_string(),
        cache: dirs.cache.to_string_lossy().to_string(),
    };

    let bwrap_argv = plan_builder::build_rootfs(
        &meta.rootfs_path,
        &rootfs_dirs,
        &command,
        &env,
        &sandbox_spec.network,
        &sandbox_spec.namespaces,
    );

    // Check bwrap availability
    let bwrap = bubblewrap::detect();
    match &bwrap {
        bubblewrap::BwrapAvailability::Available { .. } => {}
        bubblewrap::BwrapAvailability::DockerAvailable { .. } => {
            eprintln!("warning: Docker execution with rootfs not yet fully supported");
        }
        bubblewrap::BwrapAvailability::Unavailable { reason } => {
            eprintln!("error: bwrap is not available: {reason}");
            std::process::exit(1);
        }
    };

    let _ = session::touch_last_exec(session_id);

    if json {
        // NDJSON mode: pipe stdout/stderr, stream events
        use std::process::{Command, Stdio};
        let mut child = match &bwrap {
            bubblewrap::BwrapAvailability::DockerAvailable { container_id, .. } => {
                let mut cmd_args = vec!["exec".to_string(), "-i".to_string(), container_id.clone(), "bwrap".to_string()];
                cmd_args.extend(bwrap_argv);
                Command::new("docker")
                    .args(&cmd_args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap_or_else(|e| {
                        eprintln!("error: failed to spawn docker+bwrap: {e}");
                        std::process::exit(1);
                    })
            }
            _ => {
                Command::new("bwrap")
                    .args(&bwrap_argv)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap_or_else(|e| {
                        eprintln!("error: failed to spawn bwrap: {e}");
                        std::process::exit(1);
                    })
            }
        };

        use std::io::{BufRead, BufReader};
        let start = std::time::Instant::now();
        let mut seq = 1u64;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let event = serde_json::json!({
                        "type": "stdout",
                        "sequence": seq,
                        "ts": timestamps::now_iso8601(),
                        "payload": { "data": line }
                    });
                    println!("{}", event);
                    seq += 1;
                }
            }
        }

        let status = child.wait().unwrap_or_else(|e| {
            eprintln!("error: wait: {e}");
            std::process::exit(1);
        });

        let duration_ms = start.elapsed().as_millis() as u64;
        let result = serde_json::json!({
            "type": "result",
            "payload": {
                "exitCode": status.code().unwrap_or(-1),
                "timedOut": false,
                "durationMs": duration_ms,
            }
        });
        println!("{}", result);
        std::process::exit(status.code().unwrap_or(1));
    } else {
        // Interactive mode: inherit stdio
        use std::process::Command;
        let status = match &bwrap {
            bubblewrap::BwrapAvailability::DockerAvailable { container_id, .. } => {
                let mut cmd_args = vec!["exec".to_string(), "-i".to_string(), container_id.clone(), "bwrap".to_string()];
                cmd_args.extend(bwrap_argv);
                Command::new("docker")
                    .args(&cmd_args)
                    .status()
                    .unwrap_or_else(|e| {
                        eprintln!("error: failed to run docker+bwrap: {e}");
                        std::process::exit(1);
                    })
            }
            _ => {
                Command::new("bwrap")
                    .args(&bwrap_argv)
                    .status()
                    .unwrap_or_else(|e| {
                        eprintln!("error: failed to run bwrap: {e}");
                        std::process::exit(1);
                    })
            }
        };
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn cmd_enter(session_id: &str) {
    cmd_exec(session_id, false, vec![], vec!["/bin/bash".to_string()]);
}

fn cmd_list(json: bool) {
    let sessions = session::list_sessions().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&sessions).unwrap());
    } else {
        if sessions.is_empty() {
            println!("No active sessions.");
            return;
        }
        println!("{:<12} {:<20} {:<16} {}", "SESSION", "NAME", "PROFILE", "CREATED");
        for s in &sessions {
            println!("{:<12} {:<20} {:<16} {}", s.session_id, s.name, s.profile, s.created_at);
        }
    }
}

fn cmd_destroy(session_id: &str) {
    session::destroy_session(session_id).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    eprintln!("Session {} destroyed.", session_id);
}

fn cmd_build(profile: Option<String>, spec_file: Option<String>, json: bool) {
    let sandbox_spec = resolve_spec(profile.clone(), spec_file);
    let rootfs_path = build_rootfs_for_spec(&sandbox_spec, &profile);

    if json {
        println!("{}", serde_json::json!({ "rootfsPath": rootfs_path }));
    } else {
        println!("{}", rootfs_path);
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
