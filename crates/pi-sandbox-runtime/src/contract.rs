use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::timestamps::now_iso8601;

pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Inbound types (TypeScript -> Rust)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InboundMessage {
    Plan { payload: PlanPayload },
    Cancel { payload: CancelPayload },
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub mounts: Vec<Mount>,
    pub env: HashMap<String, String>,
    pub cwd: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(rename = "type")]
    pub mount_type: String,
    pub source: Option<String>,
    pub target: String,
    pub writable: bool,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub mode: String,
    pub allowlist: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLimits {
    pub max_cpu_seconds: Option<f64>,
    pub max_memory_bytes: Option<u64>,
    pub max_pids: Option<u32>,
    pub max_output_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelPayload {
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Outbound types (Rust -> TypeScript)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum OutboundMessage {
    Validation(ValidationEnvelope),
    Stdout(StdoutEnvelope),
    Stderr(StderrEnvelope),
    Lifecycle(LifecycleEnvelope),
    Network(NetworkEnvelope),
    Warning(WarningEnvelope),
    Result(ResultEnvelope),
}

// --- Validation ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub v: u32,
    pub payload: ValidationPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationPayload {
    pub ok: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub effective_state: Option<EffectiveState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveState {
    pub network: EffectiveNetwork,
    pub namespaces_applied: Vec<String>,
    pub env_applied: Vec<String>,
    pub resolved_allowlist: Vec<ResolvedAllowlistEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAllowlistEntry {
    pub hostname: String,
    pub ips: Vec<String>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveNetwork {
    pub requested: String,
    pub actual: String,
    pub enforcement: String,
    pub degraded: bool,
}

impl ValidationEnvelope {
    pub fn new(payload: ValidationPayload) -> OutboundMessage {
        OutboundMessage::Validation(ValidationEnvelope {
            msg_type: "validation",
            v: PROTOCOL_VERSION,
            payload,
        })
    }
}

// --- Stdout ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StdoutEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: DataPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataPayload {
    pub data: String,
}

impl StdoutEnvelope {
    pub fn new(sequence: u64, data: String) -> OutboundMessage {
        OutboundMessage::Stdout(StdoutEnvelope {
            msg_type: "stdout",
            sequence,
            ts: now_iso8601(),
            payload: DataPayload { data },
        })
    }
}

// --- Stderr ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StderrEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: DataPayload,
}

impl StderrEnvelope {
    pub fn new(sequence: u64, data: String) -> OutboundMessage {
        OutboundMessage::Stderr(StderrEnvelope {
            msg_type: "stderr",
            sequence,
            ts: now_iso8601(),
            payload: DataPayload { data },
        })
    }
}

// --- Lifecycle ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: LifecyclePayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecyclePayload {
    pub event: String,
}

impl LifecycleEnvelope {
    pub fn new(sequence: u64, event: String) -> OutboundMessage {
        OutboundMessage::Lifecycle(LifecycleEnvelope {
            msg_type: "lifecycle",
            sequence,
            ts: now_iso8601(),
            payload: LifecyclePayload { event },
        })
    }
}

// --- Network ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: NetworkEventPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkEventPayload {
    pub direction: String,
    pub host: String,
    pub port: u16,
    pub protocol: Option<String>,
}

impl NetworkEnvelope {
    pub fn new(
        sequence: u64,
        direction: String,
        host: String,
        port: u16,
        protocol: Option<String>,
    ) -> OutboundMessage {
        OutboundMessage::Network(NetworkEnvelope {
            msg_type: "network",
            sequence,
            ts: now_iso8601(),
            payload: NetworkEventPayload {
                direction,
                host,
                port,
                protocol,
            },
        })
    }
}

// --- Warning ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub sequence: u64,
    pub ts: String,
    pub payload: WarningPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningPayload {
    pub code: String,
    pub message: String,
}

impl WarningEnvelope {
    pub fn new(sequence: u64, code: String, message: String) -> OutboundMessage {
        OutboundMessage::Warning(WarningEnvelope {
            msg_type: "warning",
            sequence,
            ts: now_iso8601(),
            payload: WarningPayload { code, message },
        })
    }
}

// --- Result ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultEnvelope {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub v: u32,
    pub payload: ResultPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultPayload {
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub timed_out: bool,
    pub duration_ms: f64,
    pub effective_network: EffectiveNetwork,
    pub observed_connections: Vec<ObservedConnection>,
    pub would_have_blocked: Vec<BlockedConnection>,
    pub resource_peaks: Option<ResourcePeaks>,
    pub reconciliation_hints: ReconciliationHints,
}

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePeaks {
    pub cpu_seconds: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub pids: Option<u32>,
    pub output_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationHints {
    pub terminal_state: String,
    pub workspace_modified: bool,
    pub cleanup_succeeded: bool,
}

impl ResultEnvelope {
    pub fn new(payload: ResultPayload) -> OutboundMessage {
        OutboundMessage::Result(ResultEnvelope {
            msg_type: "result",
            v: PROTOCOL_VERSION,
            payload,
        })
    }
}

// ---------------------------------------------------------------------------
// Emit helper
// ---------------------------------------------------------------------------

/// Serialize an outbound message to JSON and print it to stdout (NDJSON).
pub fn emit(message: &OutboundMessage) {
    println!("{}", serde_json::to_string(message).expect("serialization must not fail"));
}
