# Browser Automation Design (Phase 2)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add browser automation to the Rust Sandbox API using chromiumoxide for CDP-based Chromium control.

**Architecture:** Single shared Chromium instance with lazy initialization. Each request creates a fresh page, executes actions, and closes the page.

**Tech Stack:** Rust, Axum 0.8, chromiumoxide, Tokio

---

## Scope

**Included:**
- Basic navigation: goto, screenshot, evaluate JavaScript, click, type
- Single shared browser instance (lazy-initialized)
- Action-based REST endpoints

**Excluded (future phases):**
- Screen automation (xdotool, scrot)
- Playwright fallback
- Full automation (forms, downloads, cookies, PDF)

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│                   AppState                       │
│  ┌─────────────────────────────────────────┐    │
│  │          BrowserService                  │    │
│  │  ┌─────────────────────────────────┐    │    │
│  │  │   OnceCell<Browser>             │    │    │
│  │  │   - Single Chromium instance    │    │    │
│  │  │   - Launched on first use       │    │    │
│  │  │   - Shared across all requests  │    │    │
│  │  └─────────────────────────────────┘    │    │
│  └─────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

**Request Flow:**
1. Request arrives at `/browser/goto`
2. Handler gets `BrowserService` from `AppState`
3. `BrowserService.get_browser()` lazy-inits Chromium if needed
4. Create new page, execute action
5. Close page, return result

**Module Structure:**
```
src/browser/
├── mod.rs          # Module exports
├── service.rs      # BrowserService (manages Chromium)
└── types.rs        # Request/response types
```

---

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/browser/goto` | Navigate to URL, return page info |
| POST | `/browser/screenshot` | Take screenshot, return base64 PNG |
| POST | `/browser/evaluate` | Execute JavaScript, return result |
| POST | `/browser/click` | Click element by selector |
| POST | `/browser/type` | Type text into element |
| GET | `/browser/status` | Check if browser is running |

---

## Request/Response Types

```rust
// POST /browser/goto
#[derive(Debug, Deserialize)]
pub struct GotoRequest {
    pub url: String,
    #[serde(default)]
    pub wait_until: Option<String>,  // "load", "domcontentloaded", "networkidle"
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
    pub url: Option<String>,         // Navigate first if provided
    pub selector: Option<String>,    // Element to capture (full page if none)
    #[serde(default = "default_format")]
    pub format: String,              // "png" or "jpeg"
}

#[derive(Debug, Serialize)]
pub struct ScreenshotResponse {
    pub data: String,                // base64 encoded
    pub format: String,
    pub width: u32,
    pub height: u32,
}

// POST /browser/evaluate
#[derive(Debug, Deserialize)]
pub struct EvaluateRequest {
    pub url: Option<String>,         // Navigate first if provided
    pub script: String,              // JavaScript to execute
}

#[derive(Debug, Serialize)]
pub struct EvaluateResponse {
    pub result: serde_json::Value,   // JS return value
}

// POST /browser/click
#[derive(Debug, Deserialize)]
pub struct ClickRequest {
    pub url: Option<String>,         // Navigate first if provided
    pub selector: String,            // CSS selector
}

// POST /browser/type
#[derive(Debug, Deserialize)]
pub struct TypeRequest {
    pub url: Option<String>,         // Navigate first if provided
    pub selector: String,            // CSS selector
    pub text: String,                // Text to type
}

// GET /browser/status
#[derive(Debug, Serialize)]
pub struct BrowserStatus {
    pub running: bool,
    pub version: Option<String>,
}
```

---

## BrowserService

```rust
use chromiumoxide::{Browser, BrowserConfig};
use tokio::sync::OnceCell;

pub struct BrowserService {
    browser: OnceCell<Browser>,
    config: BrowserServiceConfig,
}

#[derive(Debug, Clone)]
pub struct BrowserServiceConfig {
    pub headless: bool,
    pub executable_path: Option<String>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub timeout: u64,
}

impl BrowserService {
    pub fn new(config: BrowserServiceConfig) -> Self;

    /// Lazy-init browser on first call
    async fn get_browser(&self) -> Result<&Browser>;

    /// Public methods for each action
    pub async fn goto(&self, req: GotoRequest) -> Result<GotoResponse>;
    pub async fn screenshot(&self, req: ScreenshotRequest) -> Result<ScreenshotResponse>;
    pub async fn evaluate(&self, req: EvaluateRequest) -> Result<EvaluateResponse>;
    pub async fn click(&self, req: ClickRequest) -> Result<()>;
    pub async fn type_text(&self, req: TypeRequest) -> Result<()>;
    pub async fn status(&self) -> BrowserStatus;
}
```

**Chrome Launch Args:**
```rust
let mut args = vec![
    "--disable-gpu",
    "--disable-dev-shm-usage",
    "--disable-setuid-sandbox",
];

// Detect container environment - disable Chrome sandbox
if std::path::Path::new("/.dockerenv").exists()
   || std::env::var("CONTAINER").is_ok() {
    args.push("--no-sandbox");
}
```

---

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `BROWSER_HEADLESS` | `true` | Run headless (false for debugging) |
| `BROWSER_EXECUTABLE` | auto-detect | Path to Chrome/Chromium binary |
| `CDP_URL` | none | Connect to existing Chrome instead of launching |
| `BROWSER_TIMEOUT` | `30` | Default timeout for operations (seconds) |
| `BROWSER_VIEWPORT_WIDTH` | `1280` | Default viewport width |
| `BROWSER_VIEWPORT_HEIGHT` | `720` | Default viewport height |

---

## Error Handling

```rust
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

    #[error("Timeout after {0}s")]
    Timeout(u64),

    #[error("Screenshot failed: {0}")]
    ScreenshotFailed(String),
}

impl From<BrowserError> for AppError {
    fn from(e: BrowserError) -> Self {
        match e {
            BrowserError::ElementNotFound(_) => AppError::NotFound(e.to_string()),
            BrowserError::Timeout(_) => AppError::Timeout(e.to_string()),
            _ => AppError::Internal(e.to_string()),
        }
    }
}
```

---

## Implementation Tasks

### Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml`

Add:
```toml
chromiumoxide = { version = "0.7", features = ["tokio-runtime"] }
base64 = "0.22"
```

### Task 2: Create Browser Types

**Files:**
- Create: `src/browser/types.rs`
- Create: `src/browser/mod.rs`

Request/response structs, BrowserError enum.

### Task 3: Create BrowserService

**Files:**
- Create: `src/browser/service.rs`

BrowserService with lazy init, all action methods.

### Task 4: Create Browser Handlers

**Files:**
- Create: `src/handlers/browser.rs`
- Modify: `src/handlers/mod.rs`

Endpoint handlers using BrowserService.

### Task 5: Update AppState and Config

**Files:**
- Modify: `src/config.rs`
- Modify: `src/state.rs`

Add BrowserServiceConfig and BrowserService to state.

### Task 6: Wire Up Routes

**Files:**
- Modify: `src/main.rs`

Add browser routes.

### Task 7: Add Integration Tests

**Files:**
- Create: `tests/browser_test.rs`

Tests for goto, screenshot, evaluate, click, type, error cases.

### Task 8: Final Verification

Build, run tests, manual API verification.

---

## Testing

```rust
#[tokio::test]
async fn test_browser_goto() {
    // Navigate to example.com, verify title returned
}

#[tokio::test]
async fn test_browser_screenshot() {
    // Take screenshot, verify base64 PNG returned
}

#[tokio::test]
async fn test_browser_evaluate() {
    // Execute JS, verify result
}

#[tokio::test]
async fn test_browser_click_and_type() {
    // Navigate to form page, type text, verify
}

#[tokio::test]
async fn test_browser_element_not_found() {
    // Click non-existent selector, expect 404
}

#[tokio::test]
async fn test_browser_status() {
    // Check status before/after first use
}
```
