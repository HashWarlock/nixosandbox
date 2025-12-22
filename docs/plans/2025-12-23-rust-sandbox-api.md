# Rust Sandbox API Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite the NixOS AI Agent Sandbox from Python/FastAPI to Rust/Axum for improved performance and type safety.

**Architecture:** Axum 0.8 async web server with modular handlers and services. Each domain (shell, code, file, browser, screen) lives in its own module. Shared state via `Arc<AppState>`. Browser sessions shared per user via `DashMap`.

**Tech Stack:** Rust, Axum 0.8, Tokio, Serde, chromiumoxide, portable-pty

---

## Phase 1: Project Setup and Health Endpoints

### Task 1: Initialize Rust Project

**Files:**
- Create: `sandbox-rs/Cargo.toml`
- Create: `sandbox-rs/src/main.rs`
- Create: `sandbox-rs/src/lib.rs`

**Step 1: Create project directory**

```bash
mkdir -p /home/gem/nixos-sandbox/sandbox-rs/src
```

**Step 2: Create Cargo.toml with dependencies**

Create `sandbox-rs/Cargo.toml`:

```toml
[package]
name = "sandbox-api"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"
anyhow = "1"

[dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
tokio-test = "0.4"
```

**Step 3: Create minimal main.rs**

Create `sandbox-rs/src/main.rs`:

```rust
use std::net::SocketAddr;
use axum::{Router, routing::get};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "sandbox_api=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/health", get(|| async { "ok" }));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 4: Create lib.rs for module exports**

Create `sandbox-rs/src/lib.rs`:

```rust
pub mod config;
pub mod error;
pub mod handlers;
pub mod state;
```

**Step 5: Verify project compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles with warnings about missing modules

**Step 6: Commit**

```bash
cd /home/gem/nixos-sandbox
git add sandbox-rs/
git commit -m "feat: initialize Rust sandbox-api project structure"
```

---

### Task 2: Add Configuration Module

**Files:**
- Create: `sandbox-rs/src/config.rs`

**Step 1: Create config.rs**

Create `sandbox-rs/src/config.rs`:

```rust
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub workspace: String,
    pub display: String,
    pub cdp_port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            workspace: env::var("WORKSPACE")
                .unwrap_or_else(|_| "/home/sandbox/workspace".into()),
            display: env::var("DISPLAY").unwrap_or_else(|_| ":99".into()),
            cdp_port: env::var("CDP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(9222),
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add sandbox-rs/src/config.rs
git commit -m "feat: add configuration module"
```

---

### Task 3: Add Error Types

**Files:**
- Create: `sandbox-rs/src/error.rs`

**Step 1: Create error.rs**

Create `sandbox-rs/src/error.rs`:

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Timeout(msg) => (StatusCode::REQUEST_TIMEOUT, msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Io(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
```

**Step 2: Verify it compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add sandbox-rs/src/error.rs
git commit -m "feat: add error types with IntoResponse"
```

---

### Task 4: Add Application State

**Files:**
- Create: `sandbox-rs/src/state.rs`

**Step 1: Create state.rs**

Create `sandbox-rs/src/state.rs`:

```rust
use std::sync::Arc;
use std::time::Instant;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub start_time: Instant,
}

impl AppState {
    pub fn new(config: Config) -> Arc<Self> {
        Arc::new(Self {
            config,
            start_time: Instant::now(),
        })
    }

    pub fn uptime_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }
}
```

**Step 2: Verify it compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add sandbox-rs/src/state.rs
git commit -m "feat: add application state"
```

---

### Task 5: Add Handlers Module Structure

**Files:**
- Create: `sandbox-rs/src/handlers/mod.rs`
- Create: `sandbox-rs/src/handlers/health.rs`

**Step 1: Create handlers directory**

```bash
mkdir -p /home/gem/nixos-sandbox/sandbox-rs/src/handlers
```

**Step 2: Create handlers/mod.rs**

Create `sandbox-rs/src/handlers/mod.rs`:

```rust
pub mod health;

pub use health::*;
```

**Step 3: Create handlers/health.rs**

Create `sandbox-rs/src/handlers/health.rs`:

```rust
use std::sync::Arc;
use axum::{extract::State, Json};
use serde::Serialize;
use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime: f64,
    pub services: Services,
}

#[derive(Serialize)]
pub struct Services {
    pub display: bool,
    pub browser: bool,
}

pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let display_exists = std::path::Path::new("/tmp/.X11-unix/X99").exists();

    Json(HealthResponse {
        status: "healthy".into(),
        uptime: state.uptime_secs(),
        services: Services {
            display: display_exists,
            browser: false, // Will be updated when browser manager is added
        },
    })
}

#[derive(Serialize)]
pub struct SandboxInfo {
    pub hostname: String,
    pub workspace: String,
    pub display: String,
    pub cdp_url: String,
    pub vnc_url: String,
}

pub async fn sandbox_info(State(state): State<Arc<AppState>>) -> Json<SandboxInfo> {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".into());

    Json(SandboxInfo {
        hostname,
        workspace: state.config.workspace.clone(),
        display: state.config.display.clone(),
        cdp_url: format!("http://localhost:{}", state.config.cdp_port),
        vnc_url: "vnc://localhost:5900".into(),
    })
}
```

**Step 4: Add hostname dependency to Cargo.toml**

Add to `sandbox-rs/Cargo.toml` under `[dependencies]`:

```toml
hostname = "0.4"
```

**Step 5: Verify it compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles successfully

**Step 6: Commit**

```bash
git add sandbox-rs/src/handlers/
git add sandbox-rs/Cargo.toml
git commit -m "feat: add health and sandbox info handlers"
```

---

### Task 6: Wire Up Health Endpoints in Main

**Files:**
- Modify: `sandbox-rs/src/main.rs`

**Step 1: Update main.rs with full routing**

Replace `sandbox-rs/src/main.rs`:

```rust
mod config;
mod error;
mod handlers;
mod state;

use std::net::SocketAddr;
use axum::{Router, routing::get};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use state::AppState;
use handlers::{health_check, sandbox_info};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "sandbox_api=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(config);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/sandbox/info", get(sandbox_info))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 2: Remove lib.rs (using main.rs as entry point)**

```bash
rm /home/gem/nixos-sandbox/sandbox-rs/src/lib.rs
```

**Step 3: Build and run**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo build`
Expected: Compiles successfully

**Step 4: Test manually**

Run in background: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo run &`
Test: `curl http://localhost:8080/health`
Expected: `{"status":"healthy","uptime":...,"services":{"display":false,"browser":false}}`
Kill: `pkill sandbox-api`

**Step 5: Commit**

```bash
git add sandbox-rs/
git commit -m "feat: wire up health endpoints"
```

---

### Task 7: Add Integration Tests for Health

**Files:**
- Create: `sandbox-rs/tests/health_test.rs`

**Step 1: Create tests directory**

```bash
mkdir -p /home/gem/nixos-sandbox/sandbox-rs/tests
```

**Step 2: Create health_test.rs**

Create `sandbox-rs/tests/health_test.rs`:

```rust
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

async fn wait_for_server(base_url: &str) {
    let client = Client::new();
    for _ in 0..50 {
        if client.get(format!("{}/health", base_url)).send().await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server did not start in time");
}

#[tokio::test]
async fn test_health_check() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["status"], "healthy");
    assert!(body["uptime"].as_f64().unwrap() >= 0.0);
}

#[tokio::test]
async fn test_sandbox_info() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/sandbox/info", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert!(body["workspace"].as_str().is_some());
    assert!(body["display"].as_str().is_some());
}
```

**Step 3: Run tests (requires server running)**

Terminal 1: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo run`
Terminal 2: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo test`
Expected: 2 tests pass

**Step 4: Commit**

```bash
git add sandbox-rs/tests/
git commit -m "test: add integration tests for health endpoints"
```

---

## Phase 2: Shell Execution

### Task 8: Add Shell Types

**Files:**
- Create: `sandbox-rs/src/handlers/shell.rs`
- Modify: `sandbox-rs/src/handlers/mod.rs`

**Step 1: Create shell.rs with request/response types**

Create `sandbox-rs/src/handlers/shell.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
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
```

**Step 2: Update handlers/mod.rs**

Replace `sandbox-rs/src/handlers/mod.rs`:

```rust
pub mod health;
pub mod shell;

pub use health::*;
pub use shell::*;
```

**Step 3: Verify it compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add sandbox-rs/src/handlers/
git commit -m "feat: add shell execution handler"
```

---

### Task 9: Wire Up Shell Endpoint

**Files:**
- Modify: `sandbox-rs/src/main.rs`

**Step 1: Add shell route to main.rs**

Update `sandbox-rs/src/main.rs` router:

```rust
mod config;
mod error;
mod handlers;
mod state;

use std::net::SocketAddr;
use axum::{Router, routing::{get, post}};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use state::AppState;
use handlers::{health_check, sandbox_info, exec_command};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "sandbox_api=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(config);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/sandbox/info", get(sandbox_info))
        .route("/shell/exec", post(exec_command))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 2: Build**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add sandbox-rs/src/main.rs
git commit -m "feat: wire up shell exec endpoint"
```

---

### Task 10: Add Shell Tests

**Files:**
- Create: `sandbox-rs/tests/shell_test.rs`

**Step 1: Write failing test**

Create `sandbox-rs/tests/shell_test.rs`:

```rust
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

async fn wait_for_server(base_url: &str) {
    let client = Client::new();
    for _ in 0..50 {
        if client.get(format!("{}/health", base_url)).send().await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server did not start in time");
}

#[tokio::test]
async fn test_shell_exec_simple() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/shell/exec", base_url))
        .json(&json!({
            "command": "echo hello"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["stdout"].as_str().unwrap().trim(), "hello");
    assert_eq!(body["exit_code"], 0);
}

#[tokio::test]
async fn test_shell_exec_with_env() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/shell/exec", base_url))
        .json(&json!({
            "command": "echo $MY_VAR",
            "env": { "MY_VAR": "test_value" }
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["stdout"].as_str().unwrap().trim(), "test_value");
}

#[tokio::test]
async fn test_shell_exec_stderr() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/shell/exec", base_url))
        .json(&json!({
            "command": "echo error >&2"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["stderr"].as_str().unwrap().trim(), "error");
}

#[tokio::test]
async fn test_shell_exec_nonzero_exit() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/shell/exec", base_url))
        .json(&json!({
            "command": "exit 42"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["exit_code"], 42);
}
```

**Step 2: Run tests (with server running)**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo test shell`
Expected: 4 tests pass

**Step 3: Commit**

```bash
git add sandbox-rs/tests/shell_test.rs
git commit -m "test: add shell execution tests"
```

---

## Phase 3: Code Execution

### Task 11: Add Code Execution Handler

**Files:**
- Create: `sandbox-rs/src/handlers/code.rs`
- Modify: `sandbox-rs/src/handlers/mod.rs`

**Step 1: Create code.rs**

Create `sandbox-rs/src/handlers/code.rs`:

```rust
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
```

**Step 2: Update handlers/mod.rs**

Replace `sandbox-rs/src/handlers/mod.rs`:

```rust
pub mod health;
pub mod shell;
pub mod code;

pub use health::*;
pub use shell::*;
pub use code::*;
```

**Step 3: Verify it compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add sandbox-rs/src/handlers/
git commit -m "feat: add code execution handler"
```

---

### Task 12: Wire Up Code Endpoint and Test

**Files:**
- Modify: `sandbox-rs/src/main.rs`
- Create: `sandbox-rs/tests/code_test.rs`

**Step 1: Add code route to main.rs**

Update router in `sandbox-rs/src/main.rs`:

```rust
mod config;
mod error;
mod handlers;
mod state;

use std::net::SocketAddr;
use axum::{Router, routing::{get, post}};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use state::AppState;
use handlers::{health_check, sandbox_info, exec_command, execute_code};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "sandbox_api=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(config);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/sandbox/info", get(sandbox_info))
        .route("/shell/exec", post(exec_command))
        .route("/code/execute", post(execute_code))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 2: Create code_test.rs**

Create `sandbox-rs/tests/code_test.rs`:

```rust
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

async fn wait_for_server(base_url: &str) {
    let client = Client::new();
    for _ in 0..50 {
        if client.get(format!("{}/health", base_url)).send().await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server did not start in time");
}

#[tokio::test]
async fn test_code_python() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/code/execute", base_url))
        .json(&json!({
            "code": "print('hello from python')",
            "language": "python"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["output"].as_str().unwrap().trim(), "hello from python");
    assert_eq!(body["exit_code"], 0);
}

#[tokio::test]
async fn test_code_bash() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/code/execute", base_url))
        .json(&json!({
            "code": "echo 'hello from bash'",
            "language": "bash"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["output"].as_str().unwrap().trim(), "hello from bash");
}

#[tokio::test]
async fn test_code_unsupported_language() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/code/execute", base_url))
        .json(&json!({
            "code": "print('hello')",
            "language": "cobol"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 400);
}
```

**Step 3: Run tests**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo test code`
Expected: 3 tests pass

**Step 4: Commit**

```bash
git add sandbox-rs/
git commit -m "feat: wire up code execution with tests"
```

---

## Phase 4: File Operations

### Task 13: Add File Handler Types

**Files:**
- Create: `sandbox-rs/src/handlers/file.rs`
- Modify: `sandbox-rs/src/handlers/mod.rs`

**Step 1: Create file.rs**

Create `sandbox-rs/src/handlers/file.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use axum::{
    body::Body,
    extract::{Multipart, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::error::{AppError, Result};
use crate::state::AppState;

fn resolve_path(base: &str, path: &str) -> PathBuf {
    if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        PathBuf::from(base).join(path)
    }
}

// Read file
#[derive(Debug, Deserialize)]
pub struct FileReadQuery {
    pub path: String,
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

fn default_encoding() -> String {
    "utf-8".into()
}

#[derive(Debug, Serialize)]
pub struct FileReadResponse {
    pub content: String,
    pub size: u64,
    pub mime_type: String,
}

pub async fn read_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileReadQuery>,
) -> Result<Json<FileReadResponse>> {
    let full_path = resolve_path(&state.config.workspace, &query.path);

    if !full_path.exists() {
        return Err(AppError::NotFound("File not found".into()));
    }

    let content = fs::read_to_string(&full_path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let metadata = fs::metadata(&full_path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(FileReadResponse {
        content,
        size: metadata.len(),
        mime_type: "text/plain".into(),
    }))
}

// Write file
#[derive(Debug, Deserialize)]
pub struct FileWriteRequest {
    pub path: String,
    pub content: String,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "644".into()
}

#[derive(Debug, Serialize)]
pub struct FileWriteResponse {
    pub path: String,
    pub size: u64,
}

pub async fn write_file(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FileWriteRequest>,
) -> Result<Json<FileWriteResponse>> {
    let full_path = resolve_path(&state.config.workspace, &req.path);

    // Create parent directories
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    fs::write(&full_path, &req.content)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Set file mode (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = u32::from_str_radix(&req.mode, 8).unwrap_or(0o644);
        let perms = std::fs::Permissions::from_mode(mode);
        fs::set_permissions(&full_path, perms)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    let size = req.content.len() as u64;

    Ok(Json(FileWriteResponse {
        path: full_path.to_string_lossy().into_owned(),
        size,
    }))
}

// List directory
#[derive(Debug, Deserialize)]
pub struct FileListQuery {
    pub path: String,
    #[serde(default)]
    pub recursive: bool,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Serialize)]
pub struct FileListResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

pub async fn list_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileListQuery>,
) -> Result<Json<FileListResponse>> {
    let full_path = resolve_path(&state.config.workspace, &query.path);

    if !full_path.exists() {
        return Err(AppError::NotFound("Path not found".into()));
    }

    let mut entries = Vec::new();

    if query.recursive {
        collect_entries_recursive(&full_path, &mut entries).await?;
    } else {
        let mut dir = fs::read_dir(&full_path)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        while let Some(entry) = dir.next_entry().await.map_err(|e| AppError::Internal(e.to_string()))? {
            if let Some(file_entry) = entry_to_file_entry(&entry).await {
                entries.push(file_entry);
            }
        }
    }

    Ok(Json(FileListResponse {
        path: full_path.to_string_lossy().into_owned(),
        entries,
    }))
}

async fn collect_entries_recursive(path: &PathBuf, entries: &mut Vec<FileEntry>) -> Result<()> {
    let mut dir = fs::read_dir(path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    while let Some(entry) = dir.next_entry().await.map_err(|e| AppError::Internal(e.to_string()))? {
        if let Some(file_entry) = entry_to_file_entry(&entry).await {
            let is_dir = file_entry.file_type == "directory";
            entries.push(file_entry);

            if is_dir {
                Box::pin(collect_entries_recursive(&entry.path(), entries)).await?;
            }
        }
    }

    Ok(())
}

async fn entry_to_file_entry(entry: &fs::DirEntry) -> Option<FileEntry> {
    let metadata = entry.metadata().await.ok()?;
    let modified = metadata.modified().ok()?;
    let datetime: chrono::DateTime<chrono::Utc> = modified.into();

    Some(FileEntry {
        name: entry.file_name().to_string_lossy().into_owned(),
        path: entry.path().to_string_lossy().into_owned(),
        file_type: if metadata.is_dir() { "directory" } else { "file" }.into(),
        size: metadata.len(),
        modified: datetime.to_rfc3339(),
    })
}

// Upload file (multipart)
pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<FileWriteResponse>> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_path: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                file_data = Some(field.bytes().await.map_err(|e| AppError::Internal(e.to_string()))?.to_vec());
            }
            "path" => {
                file_path = Some(field.text().await.map_err(|e| AppError::Internal(e.to_string()))?);
            }
            _ => {}
        }
    }

    let data = file_data.ok_or_else(|| AppError::BadRequest("Missing file field".into()))?;
    let path = file_path.ok_or_else(|| AppError::BadRequest("Missing path field".into()))?;

    let full_path = resolve_path(&state.config.workspace, &path);

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    fs::write(&full_path, &data)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(FileWriteResponse {
        path: full_path.to_string_lossy().into_owned(),
        size: data.len() as u64,
    }))
}

// Download file
#[derive(Debug, Deserialize)]
pub struct FileDownloadQuery {
    pub path: String,
}

pub async fn download_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileDownloadQuery>,
) -> Result<Response> {
    let full_path = resolve_path(&state.config.workspace, &query.path);

    if !full_path.exists() {
        return Err(AppError::NotFound("File not found".into()));
    }

    let mut file = fs::File::open(&full_path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let filename = full_path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".into());

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CONTENT_DISPOSITION, &format!("attachment; filename=\"{}\"", filename)),
        ],
        contents,
    ).into_response())
}
```

**Step 2: Add chrono dependency to Cargo.toml**

Add to `sandbox-rs/Cargo.toml` under `[dependencies]`:

```toml
chrono = { version = "0.4", features = ["serde"] }
```

**Step 3: Update handlers/mod.rs**

Replace `sandbox-rs/src/handlers/mod.rs`:

```rust
pub mod health;
pub mod shell;
pub mod code;
pub mod file;

pub use health::*;
pub use shell::*;
pub use code::*;
pub use file::*;
```

**Step 4: Verify it compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add sandbox-rs/
git commit -m "feat: add file operations handler"
```

---

### Task 14: Wire Up File Endpoints and Test

**Files:**
- Modify: `sandbox-rs/src/main.rs`
- Create: `sandbox-rs/tests/file_test.rs`

**Step 1: Add file routes to main.rs**

Update `sandbox-rs/src/main.rs`:

```rust
mod config;
mod error;
mod handlers;
mod state;

use std::net::SocketAddr;
use axum::{Router, routing::{get, post}};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use state::AppState;
use handlers::{
    health_check, sandbox_info,
    exec_command, execute_code,
    read_file, write_file, list_files, upload_file, download_file,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "sandbox_api=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(config);

    let app = Router::new()
        // Health
        .route("/health", get(health_check))
        .route("/sandbox/info", get(sandbox_info))
        // Shell
        .route("/shell/exec", post(exec_command))
        // Code
        .route("/code/execute", post(execute_code))
        // Files
        .route("/file/read", get(read_file))
        .route("/file/write", post(write_file))
        .route("/file/list", get(list_files))
        .route("/file/upload", post(upload_file))
        .route("/file/download", get(download_file))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 2: Create file_test.rs**

Create `sandbox-rs/tests/file_test.rs`:

```rust
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

async fn wait_for_server(base_url: &str) {
    let client = Client::new();
    for _ in 0..50 {
        if client.get(format!("{}/health", base_url)).send().await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server did not start in time");
}

#[tokio::test]
async fn test_file_write_and_read() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Write file
    let write_resp = client
        .post(format!("{}/file/write", base_url))
        .json(&json!({
            "path": "/tmp/test_file.txt",
            "content": "hello world"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(write_resp.status(), 200);

    // Read file back
    let read_resp = client
        .get(format!("{}/file/read?path=/tmp/test_file.txt", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(read_resp.status(), 200);

    let body: Value = read_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["content"], "hello world");
}

#[tokio::test]
async fn test_file_list() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Ensure /tmp exists and list it
    let resp = client
        .get(format!("{}/file/list?path=/tmp", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert!(body["entries"].is_array());
}

#[tokio::test]
async fn test_file_not_found() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/file/read?path=/nonexistent/file.txt", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_file_download() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // First write a file
    client
        .post(format!("{}/file/write", base_url))
        .json(&json!({
            "path": "/tmp/download_test.txt",
            "content": "download content"
        }))
        .send()
        .await
        .expect("Failed to write file");

    // Download it
    let resp = client
        .get(format!("{}/file/download?path=/tmp/download_test.txt", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("content-disposition").is_some());

    let content = resp.text().await.expect("Failed to get body");
    assert_eq!(content, "download content");
}
```

**Step 3: Run tests**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo test file`
Expected: 4 tests pass

**Step 4: Commit**

```bash
git add sandbox-rs/
git commit -m "feat: wire up file operations with tests"
```

---

## Phase 5: Shell Streaming (SSE)

### Task 15: Add Shell Stream Endpoint

**Files:**
- Modify: `sandbox-rs/src/handlers/shell.rs`

**Step 1: Add SSE streaming to shell.rs**

Add to `sandbox-rs/src/handlers/shell.rs`:

```rust
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use std::convert::Infallible;
use tokio::io::{AsyncBufReadExt, BufReader};

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
                let stderr = child.stderr.take();

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
```

**Step 2: Add dependencies to Cargo.toml**

Add to `sandbox-rs/Cargo.toml`:

```toml
async-stream = "0.3"
futures = "0.3"
```

**Step 3: Verify it compiles**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo check`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add sandbox-rs/
git commit -m "feat: add shell streaming endpoint"
```

---

### Task 16: Wire Up Stream Endpoint

**Files:**
- Modify: `sandbox-rs/src/main.rs`
- Modify: `sandbox-rs/src/handlers/mod.rs`

**Step 1: Export stream_command in mod.rs**

The `stream_command` should already be public, but ensure it's exported.

**Step 2: Add route to main.rs**

Add this line to the router in `sandbox-rs/src/main.rs`:

```rust
.route("/shell/stream", post(handlers::stream_command))
```

Full main.rs:

```rust
mod config;
mod error;
mod handlers;
mod state;

use std::net::SocketAddr;
use axum::{Router, routing::{get, post}};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use state::AppState;
use handlers::{
    health_check, sandbox_info,
    exec_command, stream_command, execute_code,
    read_file, write_file, list_files, upload_file, download_file,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "sandbox_api=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(config);

    let app = Router::new()
        // Health
        .route("/health", get(health_check))
        .route("/sandbox/info", get(sandbox_info))
        // Shell
        .route("/shell/exec", post(exec_command))
        .route("/shell/stream", post(stream_command))
        // Code
        .route("/code/execute", post(execute_code))
        // Files
        .route("/file/read", get(read_file))
        .route("/file/write", post(write_file))
        .route("/file/list", get(list_files))
        .route("/file/upload", post(upload_file))
        .route("/file/download", get(download_file))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 3: Build and test manually**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo build`
Manual test: `curl -X POST http://localhost:8080/shell/stream -H "Content-Type: application/json" -d '{"command":"echo line1; sleep 0.1; echo line2"}'`
Expected: SSE events with "line1", "line2", "[exit_code:0]"

**Step 4: Commit**

```bash
git add sandbox-rs/
git commit -m "feat: wire up shell stream endpoint"
```

---

## Phase 6: Final Integration

### Task 17: Add Makefile

**Files:**
- Create: `sandbox-rs/Makefile`

**Step 1: Create Makefile**

Create `sandbox-rs/Makefile`:

```makefile
.PHONY: build run test check clean

build:
	cargo build --release

run:
	cargo run

test:
	cargo test

check:
	cargo check
	cargo clippy

clean:
	cargo clean

dev:
	RUST_LOG=debug cargo run

fmt:
	cargo fmt
```

**Step 2: Commit**

```bash
git add sandbox-rs/Makefile
git commit -m "chore: add Makefile for common commands"
```

---

### Task 18: Final Test Run

**Step 1: Build release**

Run: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo build --release`
Expected: Compiles successfully

**Step 2: Run all tests**

Terminal 1: `cd /home/gem/nixos-sandbox/sandbox-rs && WORKSPACE=/tmp cargo run`
Terminal 2: `cd /home/gem/nixos-sandbox/sandbox-rs && cargo test`
Expected: All tests pass (health: 2, shell: 4, code: 3, file: 4 = 13 tests)

**Step 3: Manual API verification**

```bash
# Health
curl http://localhost:8080/health | jq

# Shell
curl -X POST http://localhost:8080/shell/exec \
  -H "Content-Type: application/json" \
  -d '{"command":"uname -a"}' | jq

# Code
curl -X POST http://localhost:8080/code/execute \
  -H "Content-Type: application/json" \
  -d '{"code":"print(1+1)","language":"python"}' | jq

# File write
curl -X POST http://localhost:8080/file/write \
  -H "Content-Type: application/json" \
  -d '{"path":"/tmp/test.txt","content":"hello"}' | jq

# File read
curl "http://localhost:8080/file/read?path=/tmp/test.txt" | jq
```

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete MVP Phase 1 - health, shell, code, files"
```

---

## Summary

**Endpoints Implemented (MVP Core):**

| Endpoint | Method | Status |
|----------|--------|--------|
| `/health` | GET | ✅ |
| `/sandbox/info` | GET | ✅ |
| `/shell/exec` | POST | ✅ |
| `/shell/stream` | POST | ✅ |
| `/code/execute` | POST | ✅ |
| `/file/read` | GET | ✅ |
| `/file/write` | POST | ✅ |
| `/file/list` | GET | ✅ |
| `/file/upload` | POST | ✅ |
| `/file/download` | GET | ✅ |

**Next Phases (Future Plans):**
- Phase 2: Browser & Screen (chromiumoxide, xdotool, scrot)
- Phase 3: Skills System (registry, factory)
- Phase 4: TEE/dstack Integration
