use crate::config::Config;
use crate::skills::{SkillRegistry, FactorySessions};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub start_time: Instant,
    pub skills: SkillRegistry,
    pub factory: FactorySessions,
}

impl AppState {
    pub fn new(config: Config) -> Arc<Self> {
        let skills = SkillRegistry::new(PathBuf::from(&config.skills_dir));
        let factory = FactorySessions::new();
        Arc::new(Self {
            config,
            start_time: Instant::now(),
            skills,
            factory,
        })
    }

    pub fn uptime_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }
}
