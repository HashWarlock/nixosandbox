use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::bubblewrap::BwrapAvailability;
use crate::contract::{
    emit, EffectiveNetwork, EffectiveState, LifecycleEnvelope, ObservedConnection,
    StderrEnvelope, StdoutEnvelope, PlanPayload,
};
use crate::observer::{compute_would_have_blocked, observe_connections};
use crate::plan_builder;

pub struct SupervisionResult {
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub timed_out: bool,
    pub duration_ms: f64,
    pub effective_network: EffectiveNetwork,
    pub observed_connections: Vec<ObservedConnection>,
    pub would_have_blocked: Vec<crate::contract::BlockedConnection>,
    pub terminal_state: String,
    pub workspace_modified: bool,
}

/// Supervise execution of the plan's command.
pub fn supervise(
    plan: &PlanPayload,
    effective_state: &EffectiveState,
    cancel_rx: Receiver<()>,
    bwrap: &BwrapAvailability,
) -> SupervisionResult {
    let seq = Arc::new(AtomicU64::new(0));
    let next_seq = |counter: &AtomicU64| counter.fetch_add(1, Ordering::SeqCst);

    emit(&LifecycleEnvelope::new(
        next_seq(&seq),
        "started".to_string(),
    ));

    let start = Instant::now();

    // Build the child process — either via bwrap or direct execution.
    let mut cmd = match bwrap {
        BwrapAvailability::Available { path } => {
            let argv = plan_builder::build(plan, effective_state);
            let mut c = Command::new(path);
            c.args(&argv);
            c
        }
        BwrapAvailability::Unavailable { .. } => {
            let mut c = Command::new(&plan.command[0]);
            if plan.command.len() > 1 {
                c.args(&plan.command[1..]);
            }
            c.current_dir(&plan.manifest.cwd)
                .envs(&plan.manifest.env);
            c
        }
    };

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let seq_val = next_seq(&seq);
            emit(&LifecycleEnvelope::new(
                seq_val,
                format!("spawn_failed: {e}"),
            ));
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
            return SupervisionResult {
                exit_code: None,
                signal: None,
                timed_out: false,
                duration_ms,
                effective_network: effective_state.network.clone(),
                observed_connections: vec![],
                would_have_blocked: vec![],
                terminal_state: "supervisor_crash".to_string(),
                workspace_modified: false,
            };
        }
    };

    let child_stdout = child.stdout.take().expect("stdout was piped");
    let child_stderr = child.stderr.take().expect("stderr was piped");

    let seq_stdout = Arc::clone(&seq);
    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(child_stdout);
        for line in reader.lines() {
            match line {
                Ok(text) => {
                    let s = seq_stdout.fetch_add(1, Ordering::SeqCst);
                    emit(&StdoutEnvelope::new(s, text));
                }
                Err(_) => break,
            }
        }
    });

    let seq_stderr = Arc::clone(&seq);
    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(child_stderr);
        for line in reader.lines() {
            match line {
                Ok(text) => {
                    let s = seq_stderr.fetch_add(1, Ordering::SeqCst);
                    emit(&StderrEnvelope::new(s, text));
                }
                Err(_) => break,
            }
        }
    });

    let mut cancelled = false;
    let exit_status = loop {
        if cancel_rx.try_recv().is_ok() {
            cancelled = true;
            let s = seq.fetch_add(1, Ordering::SeqCst);
            emit(&LifecycleEnvelope::new(s, "cancel_requested".to_string()));

            #[cfg(unix)]
            {
                let pid = child.id();
                unsafe {
                    libc::kill(-(pid as libc::pid_t), libc::SIGTERM);
                }
            }
            #[cfg(not(unix))]
            {
                let _ = child.kill();
            }

            let s2 = seq.fetch_add(1, Ordering::SeqCst);
            emit(&LifecycleEnvelope::new(s2, "killing".to_string()));

            break child.wait().ok();
        }

        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => break None,
        }
    };

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    let (exit_code, signal) = match exit_status {
        Some(status) => {
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
                (status.code(), None)
            }
        }
        None => (None, None),
    };

    let terminal_state = if cancelled {
        "killed_on_cancel".to_string()
    } else if signal.is_some() {
        "killed_on_timeout".to_string()
    } else {
        "clean_exit".to_string()
    };

    let s = seq.fetch_add(1, Ordering::SeqCst);
    emit(&LifecycleEnvelope::new(s, "exited".to_string()));

    let observed = observe_connections();
    let would_have_blocked =
        compute_would_have_blocked(&observed, &plan.policy.network.allowlist);

    SupervisionResult {
        exit_code,
        signal,
        timed_out: false,
        duration_ms,
        effective_network: effective_state.network.clone(),
        observed_connections: observed,
        would_have_blocked,
        terminal_state,
        workspace_modified: false,
    }
}
