# NixOS Sandbox for AI Agents

A lightweight, self-hosted sandbox environment for AI agents with browser automation, shell access, code execution, and file operations — all controlled via REST API.

## Features

- **Shell** — Execute commands with streaming output (SSE)
- **Code Execution** — Python, JavaScript, TypeScript, Go, Rust, Bash
- **File System** — Read, write, list, upload, download
- **Browser** — CDP-based Chromium automation (goto, screenshot, evaluate, click, type)
- **Skills** — Filesystem-based skill registry with CRUD + search
- **TEE** — Optional Trusted Execution Environment support (dstack integration)

## Tech Stack

- **Rust** with Axum 0.8 + Tokio async runtime
- **chromiumoxide** for browser automation via CDP
- **Nix** for reproducible runtime environments

## Quick Start

### 1. Build the Rust API server

```bash
cd sandbox-rs
cargo build --release
```

### 2. Run the server

```bash
# Default port 8080
cargo run --release

# Custom port
PORT=9090 cargo run --release

# With TEE support
cargo run --release --features tee
```

### 3. Verify it's running

```bash
curl http://localhost:8080/health
# {"status":"healthy","uptime":1.23,"services":{"display":false,"browser":false}}
```

## API Endpoints

### Health & Info

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check with uptime and service status |
| GET | `/sandbox/info` | Sandbox environment info |

### Shell

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/shell/exec` | Execute command, return stdout/stderr |
| POST | `/shell/stream` | Stream command output via SSE |

### Code Execution

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/code/execute` | Run code (python, javascript, typescript, go, rust, bash) |

### Files

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/file/read?path=...` | Read file content |
| POST | `/file/write` | Write file content |
| GET | `/file/list?path=...` | List directory contents |
| POST | `/file/upload` | Upload file (multipart) |
| GET | `/file/download?path=...` | Download file |

### Browser (chromiumoxide)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/browser/goto` | Navigate to URL, return title |
| POST | `/browser/screenshot` | Take screenshot, return base64 PNG |
| POST | `/browser/evaluate` | Execute JavaScript, return result |
| POST | `/browser/click` | Click element by CSS selector |
| POST | `/browser/type` | Type text into element |
| GET | `/browser/status` | Check if browser is running |

### Skills

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/skills` | List all skills |
| POST | `/skills` | Create a new skill |
| GET | `/skills/search?q=...` | Search skills by name/description |
| GET | `/skills/{name}` | Get skill by name |
| PUT | `/skills/{name}` | Update skill |
| DELETE | `/skills/{name}` | Delete skill |
| POST | `/skills/{name}/scripts/{script}` | Execute skill script |

### Factory (Skill Creation Dialogue)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/factory/start` | Start skill creation session |
| POST | `/factory/continue` | Continue with user input |
| POST | `/factory/check` | Check for trigger phrases |

### TEE (Trusted Execution Environment)

*Requires `--features tee` build flag*

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/tee/info` | Get TEE environment info |
| POST | `/tee/quote` | Generate attestation quote |
| POST | `/tee/derive-key` | Derive key from path |
| POST | `/tee/sign` | Sign data with TEE key |
| POST | `/tee/verify` | Verify signature |
| POST | `/tee/emit-event` | Emit TEE event |

## Usage Examples

### Shell Execution

```bash
curl -X POST http://localhost:8080/shell/exec \
  -H "Content-Type: application/json" \
  -d '{"command": "echo hello && uname -a"}'
```

### Code Execution

```bash
curl -X POST http://localhost:8080/code/execute \
  -H "Content-Type: application/json" \
  -d '{"code": "print(2 + 2)", "language": "python"}'
```

### Browser Automation

```bash
# Navigate and get title
curl -X POST http://localhost:8080/browser/goto \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com"}'

# Take screenshot
curl -X POST http://localhost:8080/browser/screenshot \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com"}' | jq -r '.data' | base64 -d > screenshot.png

# Execute JavaScript
curl -X POST http://localhost:8080/browser/evaluate \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com", "script": "document.title"}'
```

### File Operations

```bash
# Write file
curl -X POST http://localhost:8080/file/write \
  -H "Content-Type: application/json" \
  -d '{"path": "/tmp/test.txt", "content": "Hello, World!"}'

# Read file
curl "http://localhost:8080/file/read?path=/tmp/test.txt"

# List directory
curl "http://localhost:8080/file/list?path=/tmp"
```

### Skills

```bash
# Create a skill
curl -X POST http://localhost:8080/skills \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-helper",
    "description": "A helpful skill",
    "body": "Instructions for the skill..."
  }'

# Search skills
curl "http://localhost:8080/skills/search?q=helper"
```

## Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | API server port |
| `WORKSPACE` | `/home/sandbox/workspace` | Default working directory |
| `DISPLAY` | `:99` | X11 display for browser |
| `CDP_PORT` | `9222` | Chrome DevTools Protocol port |
| `SKILLS_DIR` | `./skills` | Skills storage directory |
| `BROWSER_HEADLESS` | `true` | Run browser in headless mode |
| `BROWSER_EXECUTABLE` | (auto-detect) | Path to Chromium binary |
| `BROWSER_VIEWPORT_WIDTH` | `1280` | Default viewport width |
| `BROWSER_VIEWPORT_HEIGHT` | `720` | Default viewport height |
| `BROWSER_TIMEOUT` | `30` | Default operation timeout (seconds) |

## Testing

```bash
cd sandbox-rs

# Run unit tests
cargo test --bin sandbox-api

# Run integration tests (requires running server)
PORT=9090 cargo run &
TEST_BASE_URL=http://localhost:9090 cargo test

# Run browser tests (requires Chromium)
TEST_BASE_URL=http://localhost:9090 cargo test --test browser_test -- --ignored
```

## Project Structure

```
sandbox-rs/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point, router setup
│   ├── config.rs         # Environment configuration
│   ├── error.rs          # Error types
│   ├── state.rs          # Application state
│   ├── browser/          # Browser automation
│   │   ├── mod.rs
│   │   ├── service.rs    # BrowserService with lazy init
│   │   └── types.rs      # Request/response types
│   ├── handlers/         # HTTP handlers
│   │   ├── mod.rs
│   │   ├── health.rs
│   │   ├── shell.rs
│   │   ├── code.rs
│   │   ├── file.rs
│   │   ├── browser.rs
│   │   ├── skills.rs
│   │   ├── factory.rs
│   │   └── tee.rs
│   ├── skills/           # Skills system
│   │   ├── mod.rs
│   │   ├── registry.rs   # Filesystem-based registry
│   │   ├── types.rs      # Skill types
│   │   └── factory.rs    # Skill creation dialogue
│   └── tee/              # TEE integration (feature-gated)
│       └── mod.rs
└── tests/
    ├── health_test.rs
    ├── shell_test.rs
    ├── code_test.rs
    ├── file_test.rs
    ├── browser_test.rs
    ├── skills_test.rs
    ├── factory_test.rs
    └── tee_test.rs
```

## License

Apache 2.0
