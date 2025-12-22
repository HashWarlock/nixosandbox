use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{extract::State, Json};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::error::{AppError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ShellExecRequest {
    pub command: String,
    pub cwd: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    pub env: Option<HashMap<String, String>>,
}

fn default_timeout() -> u64 {
    30
}

#[derive(Debug, Serialize)]
pub struct ShellExecResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: f64,
}

pub async fn exec_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ShellExecRequest>,
) -> Result<Json<ShellExecResponse>> {
    let start = Instant::now();
    let cwd = req.cwd.unwrap_or_else(|| state.config.workspace.clone());

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&req.command).current_dir(&cwd);

    // Merge environment
    if let Some(env) = req.env {
        for (key, value) in env {
            cmd.env(key, value);
        }
    }

    let output = timeout(Duration::from_secs(req.timeout), cmd.output())
        .await
        .map_err(|_| AppError::Timeout("Command timed out".into()))?
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ShellExecResponse {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
    }))
}

pub async fn stream_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ShellExecRequest>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    let cwd = req.cwd.unwrap_or_else(|| state.config.workspace.clone());

    let stream = async_stream::stream! {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&req.command)
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Merge environment
        if let Some(env) = &req.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        match cmd.spawn() {
            Ok(mut child) => {
                let stdout = child.stdout.take();
                let _stderr = child.stderr.take();

                if let Some(stdout) = stdout {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        yield Ok(Event::default().data(line));
                    }
                }

                match child.wait().await {
                    Ok(status) => {
                        let code = status.code().unwrap_or(-1);
                        yield Ok(Event::default().data(format!("[exit_code:{}]", code)));
                    }
                    Err(e) => {
                        yield Ok(Event::default().data(format!("[error:{}]", e)));
                    }
                }
            }
            Err(e) => {
                yield Ok(Event::default().data(format!("[error:{}]", e)));
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
