# Skills System & TEE Integration Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Skills system (Phase 3) and TEE/dstack integration (Phase 4) to the Rust Sandbox API.

**Architecture:** Skills stored in filesystem with YAML frontmatter parsing. Factory dialogue uses in-memory sessions. TEE uses official dstack-sdk with feature-gated compilation.

**Tech Stack:** Rust, Axum 0.8, serde_yaml, DashMap, dstack-sdk

---

## Phase 3: Skills System

### Module Structure

```
src/skills/
├── mod.rs          # Module exports
├── types.rs        # Skill, SkillMeta, validation
├── registry.rs     # Filesystem CRUD operations
└── factory.rs      # Dialogue state machine
```

### Types (`src/skills/types.rs`)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub compatibility: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    #[serde(flatten)]
    pub meta: SkillMeta,
    pub body: String,
    pub scripts: Vec<String>,
    pub references: Vec<String>,
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
}

// Validation: name 1-64 chars, lowercase alphanumeric + hyphens
// No consecutive hyphens, no start/end with hyphen
pub fn validate_skill_name(name: &str) -> Result<(), String>;

// Validation: description 1-1024 chars
pub fn validate_description(desc: &str) -> Result<(), String>;
```

### Registry (`src/skills/registry.rs`)

```rust
use std::path::PathBuf;

pub struct SkillRegistry {
    skills_dir: PathBuf,
}

impl SkillRegistry {
    pub fn new(skills_dir: PathBuf) -> Self;

    pub async fn list(&self) -> Result<Vec<SkillSummary>>;
    pub async fn get(&self, name: &str) -> Result<Skill>;
    pub async fn create(&self, skill: CreateSkillRequest) -> Result<Skill>;
    pub async fn update(&self, name: &str, skill: UpdateSkillRequest) -> Result<Skill>;
    pub async fn delete(&self, name: &str) -> Result<()>;
    pub async fn search(&self, query: &str) -> Result<Vec<SkillSummary>>;
}
```

**SKILL.md parsing:**
- Split on `---` to extract YAML frontmatter
- Parse frontmatter with `serde_yaml`
- Remainder is body content
- List `scripts/`, `references/`, `assets/` directories

### Factory (`src/skills/factory.rs`)

```rust
use dashmap::DashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum FactoryStep {
    Goal,
    Trigger,
    Example,
    Complexity,
    EdgeCases,
    Confirm,
    Done,
}

#[derive(Debug, Clone, Default)]
pub struct FactoryAnswers {
    pub goal: Option<String>,
    pub triggers: Option<Vec<String>>,
    pub example_input: Option<String>,
    pub example_output: Option<String>,
    pub complexity: Option<Complexity>,
    pub edge_cases: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Complexity {
    Simple,
    Complex,
}

#[derive(Debug, Clone)]
pub struct FactorySession {
    pub id: String,
    pub step: FactoryStep,
    pub answers: FactoryAnswers,
    pub created_at: Instant,
}

pub struct FactorySessions {
    sessions: DashMap<String, FactorySession>,
}

impl FactorySessions {
    pub fn new() -> Self;
    pub fn start(&self, initial_input: Option<String>) -> FactorySession;
    pub fn continue_session(&self, id: &str, input: &str) -> Result<FactorySession>;
    pub fn get(&self, id: &str) -> Option<FactorySession>;
    pub fn cleanup_expired(&self, max_age_secs: u64);
}
```

**Step prompts:**
- Goal: "What task do you want me to help with? Give me the high-level goal."
- Trigger: "When should I use this skill? What words or situations should activate it?"
- Example: "Walk me through a real example. What would you give me as input, and what should I produce?"
- Complexity: "Is this a simple skill (text instructions only) or complex (needs scripts, templates)?"
- EdgeCases: "What should I do if something's missing or goes wrong?"
- Confirm: Summary + "Does this capture what you want? Say 'yes' to create."

### Skills API Endpoints

**Handlers** in `src/handlers/skills.rs`:

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/skills` | `list_skills` | List all skills |
| GET | `/skills/search` | `search_skills` | Search by query |
| GET | `/skills/:name` | `get_skill` | Get full skill |
| POST | `/skills` | `create_skill` | Create new skill |
| PUT | `/skills/:name` | `update_skill` | Update existing |
| DELETE | `/skills/:name` | `delete_skill` | Delete skill |
| POST | `/skills/:name/scripts/:script` | `execute_script` | Run script |

**Request/Response types:**

```rust
// GET /skills
#[derive(Serialize)]
pub struct ListSkillsResponse {
    pub skills: Vec<SkillSummary>,
}

// GET /skills/:name
// Returns: Skill

// POST /skills
#[derive(Deserialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    pub body: String,
    #[serde(default)]
    pub scripts: HashMap<String, String>,  // filename -> content
    #[serde(default)]
    pub references: HashMap<String, String>,
    #[serde(default)]
    pub assets: HashMap<String, String>,
}

// POST /skills/:name/scripts/:script
#[derive(Deserialize)]
pub struct ExecuteScriptRequest {
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct ExecuteScriptResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
```

### Factory API Endpoints

**Handlers** in `src/handlers/factory.rs`:

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/factory/start` | `start_factory` | Begin dialogue |
| POST | `/factory/continue` | `continue_factory` | Advance step |
| POST | `/factory/check` | `check_trigger` | Check if input triggers factory |

**Request/Response types:**

```rust
// POST /factory/start
#[derive(Deserialize)]
pub struct StartFactoryRequest {
    pub initial_input: Option<String>,
}

// POST /factory/continue
#[derive(Deserialize)]
pub struct ContinueFactoryRequest {
    pub session_id: String,
    pub input: String,
}

// Response for start/continue
#[derive(Serialize)]
pub struct FactoryResponse {
    pub session_id: String,
    pub step: String,
    pub prompt: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<SkillSummary>,
}

// POST /factory/check
#[derive(Deserialize)]
pub struct CheckTriggerRequest {
    pub input: String,
}

#[derive(Serialize)]
pub struct CheckTriggerResponse {
    pub triggers_factory: bool,
    pub matched_phrases: Vec<String>,
}
```

**Trigger phrases:**
- "teach me", "teach you"
- "learn this", "learn how"
- "create a skill"
- "remember how to"
- "automate this"

---

## Phase 4: TEE Integration

### Feature Flag

```toml
# Cargo.toml
[features]
default = []
tee = ["dstack-sdk"]

[dependencies]
dstack-sdk = { git = "https://github.com/Dstack-TEE/dstack", optional = true }
```

### Module Structure

```
src/tee/
├── mod.rs          # Feature-gated exports
├── types.rs        # Our request types (SDK provides responses)
└── client.rs       # Wrapper around DstackClient
```

### Client Wrapper (`src/tee/client.rs`)

```rust
use dstack_sdk::DstackClient;

pub struct TeeService {
    client: DstackClient,
}

impl TeeService {
    pub fn new(endpoint: Option<&str>) -> Self {
        Self {
            client: DstackClient::new(endpoint),
        }
    }

    pub async fn info(&self) -> Result<InfoResponse> {
        self.client.info().await
    }

    pub async fn get_quote(&self, report_data: &[u8]) -> Result<GetQuoteResponse> {
        self.client.get_quote(report_data).await
    }

    pub async fn derive_key(&self, path: Option<&str>, purpose: Option<&str>) -> Result<GetKeyResponse> {
        self.client.get_key(path, purpose).await
    }

    pub async fn sign(&self, algorithm: &str, data: &[u8]) -> Result<SignResponse> {
        self.client.sign(algorithm, data).await
    }

    pub async fn verify(
        &self,
        algorithm: &str,
        data: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<VerifyResponse> {
        self.client.verify(algorithm, data, signature, public_key).await
    }

    pub async fn emit_event(&self, event: &str, payload: &str) -> Result<()> {
        self.client.emit_event(event, payload).await
    }
}
```

### TEE API Endpoints

**Handlers** in `src/handlers/tee.rs` (only compiled with `--features tee`):

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/tee/info` | `tee_info` | CVM instance metadata |
| POST | `/tee/quote` | `generate_quote` | TDX attestation quote |
| POST | `/tee/derive-key` | `derive_key` | Derive key with path/purpose |
| POST | `/tee/sign` | `sign_data` | Sign with derived key |
| POST | `/tee/verify` | `verify_signature` | Verify signature |
| POST | `/tee/emit-event` | `emit_event` | Emit runtime event |

**Request types:**

```rust
#[derive(Deserialize)]
pub struct GenerateQuoteRequest {
    pub report_data: String,  // hex-encoded
}

#[derive(Deserialize)]
pub struct DeriveKeyRequest {
    pub path: Option<String>,
    pub purpose: Option<String>,
}

#[derive(Deserialize)]
pub struct SignRequest {
    pub algorithm: String,  // "secp256k1"
    pub data: String,       // hex-encoded
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub algorithm: String,
    pub data: String,
    pub signature: String,
    pub public_key: String,
}

#[derive(Deserialize)]
pub struct EmitEventRequest {
    pub event: String,
    pub payload: String,
}
```

**Response types** come directly from `dstack-sdk-types`:
- `InfoResponse` - app_id, instance_id, tcb_info, device_id, etc.
- `GetQuoteResponse` - quote, event_log, report_data
- `GetKeyResponse` - key, signature_chain
- `SignResponse` - signature, signature_chain, public_key
- `VerifyResponse` - verified (bool)

### Conditional Compilation

In `src/main.rs`:

```rust
#[cfg(feature = "tee")]
use handlers::tee::*;

fn build_router(state: AppState) -> Router {
    let router = Router::new()
        // ... existing routes ...
        .route("/skills", get(list_skills).post(create_skill))
        // ... skills routes ...
        ;

    #[cfg(feature = "tee")]
    let router = router
        .route("/tee/info", get(tee_info))
        .route("/tee/quote", post(generate_quote))
        .route("/tee/derive-key", post(derive_key))
        .route("/tee/sign", post(sign_data))
        .route("/tee/verify", post(verify_signature))
        .route("/tee/emit-event", post(emit_event));

    router.with_state(state)
}
```

---

## Configuration Updates

Add to `src/config.rs`:

```rust
pub struct Config {
    // Existing...
    pub host: String,
    pub port: u16,
    pub workspace: String,
    pub display: String,
    pub cdp_port: u16,

    // New
    pub skills_dir: String,
    #[cfg(feature = "tee")]
    pub dstack_endpoint: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let workspace = env::var("WORKSPACE")
            .unwrap_or_else(|_| "/home/sandbox/workspace".into());

        Self {
            // Existing...
            skills_dir: env::var("SKILLS_DIR")
                .unwrap_or_else(|_| format!("{}/.skills", workspace)),
            #[cfg(feature = "tee")]
            dstack_endpoint: env::var("DSTACK_ENDPOINT").ok(),
        }
    }
}
```

---

## AppState Updates

Add to `src/state.rs`:

```rust
use crate::skills::{SkillRegistry, FactorySessions};
#[cfg(feature = "tee")]
use crate::tee::TeeService;

pub struct AppState {
    pub config: Config,
    pub start_time: Instant,
    pub skills: SkillRegistry,
    pub factory: FactorySessions,
    #[cfg(feature = "tee")]
    pub tee: TeeService,
}
```

---

## New Dependencies

```toml
[dependencies]
# Existing
axum = { version = "0.8", features = ["multipart"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"
anyhow = "1"
hostname = "0.4"
chrono = { version = "0.4", features = ["serde"] }
async-stream = "0.3"
futures = "0.3"

# New for Skills
serde_yaml = "0.9"
dashmap = "6"
uuid = { version = "1", features = ["v4"] }
regex = "1"

# TEE (optional)
dstack-sdk = { git = "https://github.com/Dstack-TEE/dstack", optional = true }

[features]
default = []
tee = ["dstack-sdk"]
```

---

## Testing

### Test Files

```
tests/
├── skills_test.rs      # Skills CRUD
├── factory_test.rs     # Factory dialogue
└── tee_test.rs         # TEE endpoints (feature-gated)
```

### Skills Tests (`tests/skills_test.rs`)

- `test_list_skills_empty` - Empty directory returns empty list
- `test_create_and_get_skill` - Create skill, verify retrieval
- `test_create_skill_invalid_name` - Validation errors for bad names
- `test_update_skill` - Modify existing skill
- `test_delete_skill` - Remove skill
- `test_search_skills` - Query matching
- `test_execute_script` - Run script and get output

### Factory Tests (`tests/factory_test.rs`)

- `test_factory_full_flow` - Complete 6-step dialogue
- `test_factory_creates_skill` - Skill exists after confirmation
- `test_factory_session_not_found` - Error for invalid session
- `test_check_trigger_phrases` - Trigger detection

### TEE Tests (`tests/tee_test.rs`)

```rust
#[cfg(feature = "tee")]
mod tee_tests {
    // Only run with real dstack socket or mock
    #[tokio::test]
    async fn test_tee_info() { ... }

    #[tokio::test]
    async fn test_derive_key() { ... }
}
```

---

## Implementation Order

### Phase 3: Skills (Tasks 1-8)

1. Add new dependencies to Cargo.toml
2. Create `src/skills/types.rs` with validation
3. Create `src/skills/registry.rs` with CRUD
4. Create `src/skills/factory.rs` state machine
5. Create `src/handlers/skills.rs` endpoints
6. Create `src/handlers/factory.rs` endpoints
7. Update state.rs and main.rs
8. Add skills and factory tests

### Phase 4: TEE (Tasks 9-12)

9. Add dstack-sdk dependency with feature flag
10. Create `src/tee/client.rs` wrapper
11. Create `src/handlers/tee.rs` endpoints
12. Add conditional compilation in main.rs
13. Add TEE tests

### Final (Task 14)

14. Final test run and documentation
