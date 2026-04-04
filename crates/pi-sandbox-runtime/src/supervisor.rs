use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::contract::{
    emit, EffectiveNetwork, EffectiveState, LifecycleEnvelope, ObservedConnection,
    StderrEnvelope, StdoutEnvelope, PlanPayload,
};
use crate::observer::{compute_would_have_blocked, observe_connections};

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
) -> SupervisionResult {
    // Shared atomic sequence number across all output threads.
    let seq = Arc::new(AtomicU64::new(0));

    // Helper: fetch-and-increment sequence number.
    let next_seq = |counter: &AtomicU64| counter.fetch_add(1, Ordering::SeqCst);

    // Emit lifecycle: started
    emit(&LifecycleEnvelope::new(
        next_seq(&seq),
        "started".to_string(),
    ));

    let start = Instant::now();

    // Build the child process.
    let mut cmd = Command::new(&plan.command[0]);
    if plan.command.len() > 1 {
        cmd.args(&plan.command[1..]);
    }
    cmd.current_dir(&plan.manifest.cwd)
        .envs(&plan.manifest.env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            // Emit an error lifecycle and return immediately.
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
                terminal_state: "spawn_failed".to_string(),
                workspace_modified: false,
            };
        }
    };

    // Take ownership of stdout/stderr pipes.
    let child_stdout = child.stdout.take().expect("stdout was piped");
    let child_stderr = child.stderr.take().expect("stderr was piped");

    // Spawn stdout reader thread.
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

    // Spawn stderr reader thread.
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

    // Poll the cancel channel while the child is running.
    // We use a try_recv loop combined with try_wait on the child.
    let mut cancelled = false;
    let exit_status = loop {
        // Check for cancel signal (non-blocking).
        if cancel_rx.try_recv().is_ok() {
            cancelled = true;
            let s = seq.fetch_add(1, Ordering::SeqCst);
            emit(&LifecycleEnvelope::new(s, "cancel_requested".to_string()));

            // Kill the process.
            #[cfg(unix)]
            {
                let pid = child.id();
                // Send SIGTERM to the process group.
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

            // Wait for the child to exit after signaling.
            break child.wait().ok();
        }

        // Check if the child has already exited (non-blocking).
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                // Still running — sleep a short interval and poll again.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => break None,
        }
    };

    // Wait for I/O threads to finish draining output.
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Parse exit code / signal.
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

    // Determine terminal state.
    let terminal_state = if cancelled {
        "cancelled".to_string()
    } else if signal.is_some() {
        "killed".to_string()
    } else if exit_code == Some(0) {
        "clean_exit".to_string()
    } else {
        "error_exit".to_string()
    };

    // Emit lifecycle: exited
    let s = seq.fetch_add(1, Ordering::SeqCst);
    emit(&LifecycleEnvelope::new(s, "exited".to_string()));

    // Network observation (stub).
    let observed = observe_connections();
    let would_have_blocked =
        compute_would_have_blocked(&observed, &effective_state.network.allowlist);

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
