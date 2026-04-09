use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Plan types (used by docker.rs::rewrite_plan and plan_builder — deleted in Task 6)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPayload {
    pub version: u32,
    pub session_id: String,
    pub execution_id: String,
    pub requested_profile: String,
    pub runtime_base_name: Option<String>,
    pub manifest: Manifest,
    pub policy: Policy,
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub mounts: Vec<Mount>,
    pub env: HashMap<String, String>,
    pub cwd: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(rename = "type")]
    pub mount_type: String,
    pub source: Option<String>,
    pub target: String,
    pub writable: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub namespaces: Vec<String>,
    pub network: NetworkConfig,
    pub resource_limits: Option<ResourceLimits>,
    pub allowed_writable_targets: Vec<String>,
    pub strict_write_policy: bool,
    pub env_allowlist: Option<Vec<String>>,
    pub deny_commands: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub mode: String,
    pub allowlist: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLimits {
    pub max_cpu_seconds: Option<f64>,
    pub max_memory_bytes: Option<u64>,
    pub max_pids: Option<u32>,
    pub max_output_bytes: Option<u64>,
}

// ---------------------------------------------------------------------------
// Effective state types (used by plan_builder.rs — deleted in Task 6)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveState {
    pub network: EffectiveNetwork,
    pub namespaces_applied: Vec<String>,
    pub env_applied: Vec<String>,
    pub resolved_allowlist: Vec<ResolvedAllowlistEntry>,
    pub isolation_backend: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveNetwork {
    pub requested: String,
    pub actual: String,
    pub enforcement: String,
    pub degraded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAllowlistEntry {
    pub hostname: String,
    pub ips: Vec<String>,
    pub resolved: bool,
}

// ---------------------------------------------------------------------------
// Network observation types (used by observer.rs)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedConnection {
    pub direction: String,
    pub host: String,
    pub port: u16,
    pub protocol: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedConnection {
    pub direction: String,
    pub host: String,
    pub port: u16,
    pub protocol: Option<String>,
}
