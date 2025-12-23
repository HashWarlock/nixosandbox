use chromiumoxide::{Browser, BrowserConfig};
use tokio::sync::OnceCell;
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures::StreamExt;

use crate::browser::types::*;

#[derive(Debug, Clone)]
pub struct BrowserServiceConfig {
    pub headless: bool,
    pub executable_path: Option<String>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    #[allow(dead_code)] // Reserved for future timeout support
    pub timeout: u64,
}

impl Default for BrowserServiceConfig {
    fn default() -> Self {
        Self {
            headless: true,
            executable_path: None,
            viewport_width: 1280,
            viewport_height: 720,
            timeout: 30,
        }
    }
}

#[derive(Clone)]
pub struct BrowserService {
    browser: Arc<OnceCell<Browser>>,
    config: BrowserServiceConfig,
}

impl BrowserService {
    pub fn new(config: BrowserServiceConfig) -> Self {
        Self {
            browser: Arc::new(OnceCell::new()),
            config,
        }
    }

    /// Lazy-init browser on first call
    async fn get_browser(&self) -> Result<&Browser, BrowserError> {
        self.browser.get_or_try_init(|| async {
            let mut builder = BrowserConfig::builder();

            if self.config.headless {
                builder = builder.arg("--headless=new");
            }

            // Container-safe args
            builder = builder
                .arg("--disable-gpu")
                .arg("--disable-dev-shm-usage")
                .arg("--disable-setuid-sandbox");

            // Detect container and disable sandbox
            if std::path::Path::new("/.dockerenv").exists()
               || std::env::var("CONTAINER").is_ok() {
                builder = builder.arg("--no-sandbox");
            }

            builder = builder
                .viewport(chromiumoxide::handler::viewport::Viewport {
                    width: self.config.viewport_width,
                    height: self.config.viewport_height,
                    ..Default::default()
                });

            if let Some(ref path) = self.config.executable_path {
                builder = builder.chrome_executable(path);
            }

            let config = builder.build()
                .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;

            let (browser, mut handler) = Browser::launch(config)
                .await
                .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;

            // Spawn handler task (required by chromiumoxide)
            tokio::spawn(async move {
                while let Some(event) = handler.next().await {
                    // Handle browser events (required by chromiumoxide)
                    let _ = event;
                }
            });

            Ok(browser)
        }).await
    }

    pub async fn goto(&self, req: GotoRequest) -> Result<GotoResponse, BrowserError> {
        let browser = self.get_browser().await?;
        let page = browser.new_page("about:blank")
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        page.goto(&req.url)
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        let title = page.get_title()
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?
            .unwrap_or_default();

        let url = page.url()
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?
            .map(|u| u.to_string())
            .unwrap_or_else(|| req.url.clone());

        page.close().await.ok();

        Ok(GotoResponse { url, title })
    }

    pub async fn screenshot(&self, req: ScreenshotRequest) -> Result<ScreenshotResponse, BrowserError> {
        let browser = self.get_browser().await?;
        let page = browser.new_page("about:blank")
            .await
            .map_err(|e| BrowserError::ScreenshotFailed(e.to_string()))?;

        // Navigate if URL provided
        if let Some(ref url) = req.url {
            page.goto(url)
                .await
                .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;
        }

        // Take screenshot
        let screenshot_data = if let Some(ref selector) = req.selector {
            // Element screenshot
            let element = page.find_element(selector)
                .await
                .map_err(|_| BrowserError::ElementNotFound(selector.clone()))?;
            element.screenshot(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
                .await
                .map_err(|e| BrowserError::ScreenshotFailed(e.to_string()))?
        } else {
            // Full page screenshot
            page.screenshot(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::default())
                .await
                .map_err(|e| BrowserError::ScreenshotFailed(e.to_string()))?
        };

        let data = BASE64.encode(&screenshot_data);

        page.close().await.ok();

        Ok(ScreenshotResponse {
            data,
            format: req.format,
            width: self.config.viewport_width,
            height: self.config.viewport_height,
        })
    }

    pub async fn evaluate(&self, req: EvaluateRequest) -> Result<EvaluateResponse, BrowserError> {
        let browser = self.get_browser().await?;
        let page = browser.new_page("about:blank")
            .await
            .map_err(|e| BrowserError::ScriptError(e.to_string()))?;

        if let Some(ref url) = req.url {
            page.goto(url)
                .await
                .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;
        }

        let eval_result = page.evaluate(req.script)
            .await
            .map_err(|e| BrowserError::ScriptError(e.to_string()))?;

        let result = eval_result.into_value()
            .map_err(|e| BrowserError::ScriptError(e.to_string()))?;

        page.close().await.ok();

        Ok(EvaluateResponse { result })
    }

    pub async fn click(&self, req: ClickRequest) -> Result<(), BrowserError> {
        let browser = self.get_browser().await?;
        let page = browser.new_page("about:blank")
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        if let Some(ref url) = req.url {
            page.goto(url)
                .await
                .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;
        }

        let element = page.find_element(&req.selector)
            .await
            .map_err(|_| BrowserError::ElementNotFound(req.selector.clone()))?;

        element.click()
            .await
            .map_err(|e| BrowserError::ScriptError(e.to_string()))?;

        page.close().await.ok();

        Ok(())
    }

    pub async fn type_text(&self, req: TypeRequest) -> Result<(), BrowserError> {
        let browser = self.get_browser().await?;
        let page = browser.new_page("about:blank")
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        if let Some(ref url) = req.url {
            page.goto(url)
                .await
                .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;
        }

        let element = page.find_element(&req.selector)
            .await
            .map_err(|_| BrowserError::ElementNotFound(req.selector.clone()))?;

        element.type_str(&req.text)
            .await
            .map_err(|e| BrowserError::ScriptError(e.to_string()))?;

        page.close().await.ok();

        Ok(())
    }

    pub fn status(&self) -> BrowserStatus {
        BrowserStatus {
            running: self.browser.get().is_some(),
            version: None,  // Could query browser for version if needed
        }
    }
}
