use std::sync::Arc;
use axum::{extract::State, Json};
use crate::state::AppState;
use crate::error::{AppError, Result};
use crate::browser::{
    GotoRequest, GotoResponse,
    ScreenshotRequest, ScreenshotResponse,
    EvaluateRequest, EvaluateResponse,
    ClickRequest, TypeRequest,
    BrowserStatus, BrowserError,
};

// Convert BrowserError to AppError
impl From<BrowserError> for AppError {
    fn from(e: BrowserError) -> Self {
        match e {
            BrowserError::ElementNotFound(msg) => AppError::NotFound(msg),
            BrowserError::Timeout(secs) => AppError::Timeout(format!("Timeout after {}s", secs)),
            BrowserError::LaunchFailed(msg) => AppError::Internal(format!("Browser launch failed: {}", msg)),
            BrowserError::NavigationFailed(msg) => AppError::Internal(format!("Navigation failed: {}", msg)),
            BrowserError::ScriptError(msg) => AppError::BadRequest(format!("Script error: {}", msg)),
            BrowserError::ScreenshotFailed(msg) => AppError::Internal(format!("Screenshot failed: {}", msg)),
        }
    }
}

// POST /browser/goto - Navigate to a URL
pub async fn browser_goto(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GotoRequest>,
) -> Result<Json<GotoResponse>> {
    let response = state.browser.goto(req).await?;
    Ok(Json(response))
}

// POST /browser/screenshot - Take a screenshot
pub async fn browser_screenshot(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ScreenshotRequest>,
) -> Result<Json<ScreenshotResponse>> {
    let response = state.browser.screenshot(req).await?;
    Ok(Json(response))
}

// POST /browser/evaluate - Evaluate JavaScript
pub async fn browser_evaluate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EvaluateRequest>,
) -> Result<Json<EvaluateResponse>> {
    let response = state.browser.evaluate(req).await?;
    Ok(Json(response))
}

// POST /browser/click - Click an element
pub async fn browser_click(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClickRequest>,
) -> Result<Json<serde_json::Value>> {
    state.browser.click(req).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

// POST /browser/type - Type text into an element
pub async fn browser_type(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TypeRequest>,
) -> Result<Json<serde_json::Value>> {
    state.browser.type_text(req).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

// GET /browser/status - Get browser status
pub async fn browser_status(
    State(state): State<Arc<AppState>>,
) -> Json<BrowserStatus> {
    Json(state.browser.status())
}
