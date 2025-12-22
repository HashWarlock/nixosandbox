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
