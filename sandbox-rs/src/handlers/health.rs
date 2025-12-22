use crate::state::AppState;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

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
