mod bubblewrap;
mod cli;
mod docker;
mod nix;
mod plan_builder;
mod session;
mod spec;
mod timestamps;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create { profile, spec: spec_file, with, network, workspace, name, agent, description, json } => {
            cmd_create(profile, spec_file, with, network, workspace, name, agent, description, json);
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
        Commands::Status { session_id, json } => {
            cmd_status(&session_id, json);
        }
        Commands::Build { profile, spec: spec_file, json } => {
            cmd_build(profile, spec_file, json);
        }
        Commands::Catalog { json, filter } => {
            cmd_catalog(json, filter);
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

fn cmd_create(
    profile: Option<String>,
    spec_file: Option<String>,
    with: Option<Vec<String>>,
    network: String,
    workspace: Option<String>,
    name: Option<String>,
    agent: Option<String>,
    description: Option<String>,
    json: bool,
) {
    // Validate mutual exclusivity: --with vs --profile vs --spec
    let source_count = [with.is_some(), profile.is_some(), spec_file.is_some()]
        .iter()
        .filter(|&&b| b)
        .count();
    if source_count > 1 {
        eprintln!("error: specify only one of --profile, --spec, or --with");
        std::process::exit(1);
    }
    if source_count == 0 {
        eprintln!("error: specify --profile, --spec, or --with");
        std::process::exit(1);
    }

    let (rootfs_path, profile_name, session_network) = if let Some(ref packages) = with {
        // Catalog-based composition
        if packages.is_empty() {
            eprintln!("error: --with requires at least one package name");
            std::process::exit(1);
        }
        match network.as_str() {
            "off" | "full" => {}
            other => {
                eprintln!("error: --network must be 'off' or 'full', got '{other}'");
                std::process::exit(1);
            }
        }
        let rootfs = nix::build_with_catalog(packages, &network).unwrap_or_else(|e| {
            eprintln!("nix build failed: {e}");
            std::process::exit(1);
        });
        nix::validate_rootfs(&rootfs).unwrap_or_else(|e| {
            eprintln!("rootfs validation failed: {e}");
            std::process::exit(1);
        });
        (rootfs, format!("custom:{}", packages.join(",")), Some(network.clone()))
    } else {
        // Profile or spec-based
        let sandbox_spec = resolve_spec(profile.clone(), spec_file);
        let rootfs = build_rootfs_for_spec(&sandbox_spec, &profile);
        nix::validate_rootfs(&rootfs).unwrap_or_else(|e| {
            eprintln!("rootfs validation failed: {e}");
            std::process::exit(1);
        });
        (rootfs, sandbox_spec.name.clone(), None)
    };

    let session_name = name.unwrap_or_else(|| profile_name.clone());
    let meta = session::create_session(
        &session_name,
        &profile_name,
        &rootfs_path,
        workspace.as_deref(),
        agent.as_deref(),
        description.as_deref(),
        session_network.as_deref(),
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

    // Load the spec/profile to get network and namespace config.
    // For --with sessions, use the stored network mode from metadata.
    let sandbox_spec = spec::load_profile(&meta.profile, &flake_root).unwrap_or_else(|e| {
        eprintln!("warning: could not load profile '{}': {e}", meta.profile);
        // Fallback spec — use stored network mode from session metadata if available
        let network = meta.network.clone().unwrap_or_else(|| "full".to_string());
        spec::SandboxSpec {
            name: meta.profile.clone(),
            packages: vec![],
            env: std::collections::HashMap::new(),
            network,
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

    // Check bwrap availability
    let bwrap = bubblewrap::detect();
    match &bwrap {
        bubblewrap::BwrapAvailability::Available { .. } => {}
        bubblewrap::BwrapAvailability::DockerAvailable { .. } => {}
        bubblewrap::BwrapAvailability::Unavailable { reason } => {
            eprintln!("error: bwrap is not available: {reason}");
            std::process::exit(1);
        }
    };

    // For Docker, rewrite session directory paths from host to container paths.
    // Nix store paths need no rewriting — identical on host and container.
    let rootfs_dirs = match &bwrap {
        bubblewrap::BwrapAvailability::DockerAvailable {
            host_sessions_dir,
            container_sessions_dir,
            ..
        } => plan_builder::RootfsSessionDirs {
            workspace: docker::rewrite_path(
                &dirs.workspace.to_string_lossy(),
                host_sessions_dir,
                container_sessions_dir,
            ),
            home: docker::rewrite_path(
                &dirs.home.to_string_lossy(),
                host_sessions_dir,
                container_sessions_dir,
            ),
            cache: docker::rewrite_path(
                &dirs.cache.to_string_lossy(),
                host_sessions_dir,
                container_sessions_dir,
            ),
        },
        _ => plan_builder::RootfsSessionDirs {
            workspace: dirs.workspace.to_string_lossy().to_string(),
            home: dirs.home.to_string_lossy().to_string(),
            cache: dirs.cache.to_string_lossy().to_string(),
        },
    };

    let bwrap_argv = plan_builder::build_rootfs(
        &meta.rootfs_path,
        &rootfs_dirs,
        &command,
        &env,
        &sandbox_spec.network,
        &sandbox_spec.namespaces,
    );

    let _ = session::touch_last_exec(session_id);

    if json {
        // NDJSON mode: pipe stdout/stderr, stream lifecycle + data events
        use std::process::{Command, Stdio};
        use std::io::{BufRead, BufReader};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;

        let seq = Arc::new(AtomicU64::new(1));

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

        let start = std::time::Instant::now();

        // Emit lifecycle started
        let started_event = serde_json::json!({
            "type": "lifecycle",
            "sequence": seq.fetch_add(1, Ordering::SeqCst),
            "ts": timestamps::now_iso8601(),
            "payload": { "event": "started" }
        });
        println!("{}", started_event);

        // Stream stdout and stderr in parallel threads
        let child_stdout = child.stdout.take();
        let child_stderr = child.stderr.take();

        let seq_stdout = Arc::clone(&seq);
        let stdout_thread = std::thread::spawn(move || {
            if let Some(stdout) = child_stdout {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let event = serde_json::json!({
                            "type": "stdout",
                            "sequence": seq_stdout.fetch_add(1, Ordering::SeqCst),
                            "ts": timestamps::now_iso8601(),
                            "payload": { "data": line }
                        });
                        println!("{}", event);
                    }
                }
            }
        });

        let seq_stderr = Arc::clone(&seq);
        let stderr_thread = std::thread::spawn(move || {
            if let Some(stderr) = child_stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let event = serde_json::json!({
                            "type": "stderr",
                            "sequence": seq_stderr.fetch_add(1, Ordering::SeqCst),
                            "ts": timestamps::now_iso8601(),
                            "payload": { "data": line }
                        });
                        println!("{}", event);
                    }
                }
            }
        });

        let status = child.wait().unwrap_or_else(|e| {
            eprintln!("error: wait: {e}");
            std::process::exit(1);
        });

        let _ = stdout_thread.join();
        let _ = stderr_thread.join();

        let duration_ms = start.elapsed().as_millis() as u64;

        // Extract exit code and signal
        let (exit_code, signal) = {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                if let Some(sig) = status.signal() {
                    (None, Some(format!("SIG{sig}")))
                } else {
                    (status.code(), None)
                }
            }
            #[cfg(not(unix))]
            {
                (status.code(), None::<String>)
            }
        };

        // Emit lifecycle exited
        let exited_event = serde_json::json!({
            "type": "lifecycle",
            "sequence": seq.fetch_add(1, Ordering::SeqCst),
            "ts": timestamps::now_iso8601(),
            "payload": { "event": "exited" }
        });
        println!("{}", exited_event);

        // Emit result
        let result = serde_json::json!({
            "type": "result",
            "sequence": seq.fetch_add(1, Ordering::SeqCst),
            "ts": timestamps::now_iso8601(),
            "payload": {
                "exitCode": exit_code.unwrap_or(-1),
                "signal": signal,
                "timedOut": false,
                "durationMs": duration_ms,
            }
        });
        println!("{}", result);
        std::process::exit(exit_code.unwrap_or(1));
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

fn cmd_catalog(json: bool, filter: Option<String>) {
    let catalog_json = nix::query_catalog().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    if json && filter.is_none() {
        println!("{}", catalog_json);
        return;
    }

    // Parse for display or filtering
    let catalog: serde_json::Value = serde_json::from_str(&catalog_json).unwrap_or_else(|e| {
        eprintln!("error: failed to parse catalog: {e}");
        std::process::exit(1);
    });

    let filter_lower = filter.as_ref().map(|f| f.to_lowercase());

    if json {
        // Filtered JSON output
        let mut filtered = serde_json::json!({ "agents": {}, "tools": {} });
        for section in ["agents", "tools"] {
            if let Some(entries) = catalog.get(section).and_then(|v| v.as_object()) {
                let filt = filter_lower.as_ref().unwrap();
                let matched: serde_json::Map<String, serde_json::Value> = entries
                    .iter()
                    .filter(|(k, v)| {
                        k.to_lowercase().contains(filt)
                            || v.get("description")
                                .and_then(|d| d.as_str())
                                .map(|d| d.to_lowercase().contains(filt))
                                .unwrap_or(false)
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                filtered[section] = serde_json::Value::Object(matched);
            }
        }
        println!("{}", serde_json::to_string_pretty(&filtered).unwrap());
        return;
    }

    // Human-readable output
    for (section, label) in [("agents", "Agents (from llm-agents.nix)"), ("tools", "Tools (from nixpkgs)")] {
        if let Some(entries) = catalog.get(section).and_then(|v| v.as_object()) {
            println!("{}:", label);
            let mut names: Vec<&String> = entries.keys().collect();
            names.sort();
            for name in names {
                let desc = entries[name]
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                if let Some(ref filt) = filter_lower {
                    if !name.to_lowercase().contains(filt) && !desc.to_lowercase().contains(filt) {
                        continue;
                    }
                }
                println!("  {:<20} {}", name, desc);
            }
            println!();
        }
    }
}

fn cmd_status(session_id: &str, json: bool) {
    let meta = session::load_session(session_id).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    // Derive isolation backend
    let isolation = match bubblewrap::detect() {
        bubblewrap::BwrapAvailability::Available { .. } => "native",
        bubblewrap::BwrapAvailability::DockerAvailable { .. } => "docker",
        bubblewrap::BwrapAvailability::Unavailable { .. } => "unavailable",
    };

    // Derive network mode from profile spec
    let network = {
        let flake_root = nix::find_flake_root().ok();
        if let Some(ref root) = flake_root {
            spec::load_profile(&meta.profile, root)
                .map(|s| s.network.clone())
                .unwrap_or_else(|_| "unknown".to_string())
        } else {
            "unknown".to_string()
        }
    };

    if json {
        let status = serde_json::json!({
            "sessionId": meta.session_id,
            "name": meta.name,
            "profile": meta.profile,
            "rootfsPath": meta.rootfs_path,
            "workspace": meta.workspace,
            "createdAt": meta.created_at,
            "lastExecAt": meta.last_exec_at,
            "agent": meta.agent,
            "description": meta.description,
            "isolation": isolation,
            "network": network,
        });
        println!("{}", serde_json::to_string_pretty(&status).unwrap());
    } else {
        let truncate = |s: &str, max: usize| -> String {
            if s.chars().count() > max {
                let truncated: String = s.chars().take(max - 3).collect();
                format!("{truncated}...")
            } else {
                s.to_string()
            }
        };

        let desc = meta.description.as_deref().unwrap_or("-");
        let agent = meta.agent.as_deref().unwrap_or("-");
        let last_exec = meta.last_exec_at.as_deref().unwrap_or("-");
        let rootfs_display = truncate(&meta.rootfs_path, 36);
        let workspace_display = truncate(&meta.workspace, 36);

        let w = 48;
        println!("╭{}╮", "─".repeat(w));
        println!("│ {:<width$} │", format!("Session: {}", meta.session_id), width = w - 2);
        println!("├{}┤", "─".repeat(w));
        println!("│ {:<13}{:<width$} │", "Name:", meta.name, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Description:", truncate(desc, w - 15), width = w - 15);
        println!("│ {:<13}{:<width$} │", "Agent:", agent, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Profile:", meta.profile, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Created:", meta.created_at, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Last Exec:", last_exec, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Rootfs:", rootfs_display, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Workspace:", workspace_display, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Network:", network, width = w - 15);
        println!("│ {:<13}{:<width$} │", "Isolation:", isolation, width = w - 15);
        println!("╰{}╯", "─".repeat(w));
    }
}

