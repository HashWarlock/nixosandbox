use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    #[allow(dead_code)]
    pub host: String,
    pub port: u16,
    pub workspace: String,
    pub display: String,
    pub cdp_port: u16,
    pub skills_dir: String,
    pub browser_headless: bool,
    pub browser_executable: Option<String>,
    pub browser_viewport_width: u32,
    pub browser_viewport_height: u32,
    pub browser_timeout: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let workspace = env::var("WORKSPACE")
            .unwrap_or_else(|_| "/home/sandbox/workspace".into());

        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            workspace: workspace.clone(),
            display: env::var("DISPLAY").unwrap_or_else(|_| ":99".into()),
            cdp_port: env::var("CDP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(9222),
            skills_dir: env::var("SKILLS_DIR")
                .unwrap_or_else(|_| format!("{}/.skills", workspace)),
            browser_headless: env::var("BROWSER_HEADLESS")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            browser_executable: env::var("BROWSER_EXECUTABLE").ok(),
            browser_viewport_width: env::var("BROWSER_VIEWPORT_WIDTH")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(1280),
            browser_viewport_height: env::var("BROWSER_VIEWPORT_HEIGHT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(720),
            browser_timeout: env::var("BROWSER_TIMEOUT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(30),
        }
    }
}
