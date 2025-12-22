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
