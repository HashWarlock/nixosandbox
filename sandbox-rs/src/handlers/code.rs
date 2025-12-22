use std::sync::Arc;
use std::time::Instant;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::error::{AppError, Result};
use crate::state::AppState;

#[derive(Debug, Clone)]
struct LangConfig {
    ext: &'static str,
    cmd: &'static str,
}

fn get_lang_config(language: &str) -> Option<LangConfig> {
    match language.to_lowercase().as_str() {
        "python" => Some(LangConfig { ext: ".py", cmd: "python3" }),
        "javascript" => Some(LangConfig { ext: ".js", cmd: "node" }),
        "typescript" => Some(LangConfig { ext: ".ts", cmd: "npx tsx" }),
        "go" => Some(LangConfig { ext: ".go", cmd: "go run" }),
        "rust" => Some(LangConfig { ext: ".rs", cmd: "rustc -o /tmp/rust_out && /tmp/rust_out" }),
        "bash" => Some(LangConfig { ext: ".sh", cmd: "bash" }),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
pub struct CodeExecRequest {
    pub code: String,
    pub language: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    30
}

#[derive(Debug, Serialize)]
pub struct CodeExecResponse {
    pub output: String,
    pub error: String,
    pub exit_code: i32,
    pub duration_ms: f64,
}

pub async fn execute_code(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CodeExecRequest>,
) -> Result<Json<CodeExecResponse>> {
    let config = get_lang_config(&req.language)
        .ok_or_else(|| AppError::BadRequest(format!("Unsupported language: {}", req.language)))?;

    let start = Instant::now();

    // Create temp file
    let tmp_path = format!("/tmp/code_{}{}", std::process::id(), config.ext);
    fs::write(&tmp_path, &req.code)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Build command
    let full_cmd = if config.cmd.contains("&&") {
        // Rust special case: compile and run
        config.cmd.replace("/tmp/rust_out", &format!("/tmp/rust_out_{}", std::process::id()))
            + " " + &tmp_path
    } else {
        format!("{} {}", config.cmd, tmp_path)
    };

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(&full_cmd)
        .current_dir(&state.config.workspace);

    let result = timeout(Duration::from_secs(req.timeout), cmd.output()).await;

    // Cleanup temp file
    let _ = fs::remove_file(&tmp_path).await;
    let _ = fs::remove_file(format!("/tmp/rust_out_{}", std::process::id())).await;

    let output = result
        .map_err(|_| AppError::Timeout("Execution timed out".into()))?
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(CodeExecResponse {
        output: String::from_utf8_lossy(&output.stdout).into_owned(),
        error: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
    }))
}
