use serde::{Deserialize, Serialize};

fn default_timeout() -> u64 {
    30
}

fn default_format() -> String {
    "png".into()
}

// POST /browser/goto
#[derive(Debug, Deserialize)]
pub struct GotoRequest {
    pub url: String,
    #[allow(dead_code)] // Reserved for future wait_until support
    #[serde(default)]
    pub wait_until: Option<String>, // "load", "domcontentloaded", "networkidle"
    #[allow(dead_code)] // Reserved for future timeout support
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Serialize)]
pub struct GotoResponse {
    pub url: String,
    pub title: String,
}

// POST /browser/screenshot
#[derive(Debug, Deserialize)]
pub struct ScreenshotRequest {
    pub url: Option<String>,
    pub selector: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct ScreenshotResponse {
    pub data: String, // base64 encoded
    pub format: String,
    pub width: u32,
    pub height: u32,
}

// POST /browser/evaluate
#[derive(Debug, Deserialize)]
pub struct EvaluateRequest {
    pub url: Option<String>,
    pub script: String,
}

#[derive(Debug, Serialize)]
pub struct EvaluateResponse {
    pub result: serde_json::Value,
}

// POST /browser/click
#[derive(Debug, Deserialize)]
pub struct ClickRequest {
    pub url: Option<String>,
    pub selector: String,
}

// POST /browser/type
#[derive(Debug, Deserialize)]
pub struct TypeRequest {
    pub url: Option<String>,
    pub selector: String,
    pub text: String,
}

// GET /browser/status
#[derive(Debug, Serialize)]
pub struct BrowserStatus {
    pub running: bool,
    pub version: Option<String>,
}

// Error types
#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    #[error("Browser failed to launch: {0}")]
    LaunchFailed(String),

    #[error("Navigation failed: {0}")]
    NavigationFailed(String),

    #[error("Element not found: {0}")]
    ElementNotFound(String),

    #[error("JavaScript error: {0}")]
    ScriptError(String),

    #[allow(dead_code)] // Reserved for future timeout support
    #[error("Timeout after {0}s")]
    Timeout(u64),

    #[error("Screenshot failed: {0}")]
    ScreenshotFailed(String),
}
