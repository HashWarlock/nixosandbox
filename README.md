# NixOS Sandbox for AI Agents

A lightweight, self-hosted sandbox environment for AI agents with browser automation, shell access, code execution, and file operations — all controlled via OpenAPI.

## Features

- 🐚 **Shell** — Execute commands, stream output
- 🐍 **Code Execution** — Python, JavaScript, TypeScript, Go, Rust, Bash
- 📁 **File System** — Read, write, list, upload, download
- 🌐 **Browser** — Playwright-based automation with CDP support
- 🖥️ **Desktop** — VNC access, screenshots, mouse/keyboard control
- 🔌 **OpenAPI** — Full REST API with auto-generated docs

## Quick Start

### 1. Clone and setup directory structure

```bash
mkdir -p nixos-sandbox/{nix,sandbox-api}
cd nixos-sandbox

# Copy the configuration files:
# - docker-compose.yml
# - nix/shell.nix
# - sandbox-api/main.py
# - sandbox-api/client.py
```

### 2. Start the sandbox

```bash
docker-compose up -d
```

### 3. Access the services

| Service | URL |
|---------|-----|
| API Docs | http://localhost:8080/docs |
| noVNC | http://localhost:6080 |
| VNC | vnc://localhost:5900 |
| CDP | http://localhost:9222 |

## Usage

### Python SDK

```python
from client import SandboxClient

sandbox = SandboxClient("http://localhost:8080")

# Run shell commands
result = sandbox.shell("ls -la")
print(result.stdout)

# Execute Python code
output = sandbox.run_python("""
import numpy as np
print(np.random.rand(3, 3))
""")
print(output.output)

# Browser automation
sandbox.browser_launch()
sandbox.browser_navigate("https://example.com")
screenshot = sandbox.browser_screenshot()
sandbox.browser_close()

# File operations
sandbox.write_file("data.json", '{"key": "value"}')
content = sandbox.read_file("data.json")
```

### cURL

```bash
# Shell command
curl -X POST http://localhost:8080/shell/exec \
  -H "Content-Type: application/json" \
  -d '{"command": "echo hello"}'

# Execute Python
curl -X POST https://c7712468c1e19db10e63ab2f030914b19ad3766b-8080.dstack-pha-prod3.phala.network/code/execute \
  -H "Content-Type: application/json" \
  -d '{"code": "print(2+2)", "language": "python"}'

# Browser screenshot
curl https://c7712468c1e19db10e63ab2f030914b19ad3766b-8080.dstack-pha-prod3.phala.network/browser/screenshot --output screenshot.png

# Desktop screenshot
curl http://localhost:8080/screen/screenshot --output desktop.png
```

## API Endpoints

### Shell
- `POST /shell/exec` — Execute command
- `POST /shell/stream` — Stream command output (SSE)

### Code
- `POST /code/execute` — Run code (python, javascript, go, rust, bash)

### Files
- `GET /file/read?path=...` — Read file
- `POST /file/write` — Write file
- `GET /file/list?path=...` — List directory
- `POST /file/upload` — Upload file (multipart)
- `GET /file/download?path=...` — Download file

### Browser
- `POST /browser/launch` — Start browser
- `POST /browser/navigate` — Go to URL
- `GET /browser/screenshot` — Capture page
- `POST /browser/click` — Click element/coordinates
- `POST /browser/type` — Type text
- `POST /browser/evaluate` — Run JavaScript
- `POST /browser/close` — Close browser

### Screen (Desktop)
- `GET /screen/screenshot` — Capture desktop
- `POST /screen/mouse` — Mouse actions
- `POST /screen/keyboard` — Keyboard actions
- `POST /screen/record/start` — Start screen recording
- `POST /screen/record/stop` — Stop recording and save
- `GET /screen/record/status` — Get recording status
- `GET /screen/record/download` — Download recorded video

## Testing

A comprehensive test suite is included to validate API functionality and multi-turn task execution.

### Setup

```bash
cd tests
pip install -r requirements.txt
```

### Running Tests

```bash
# Run all tests
./tests/run_tests.sh http://localhost:8080

# Or using pytest directly
pytest tests/test_sandbox_api.py -v --api-url http://localhost:8080

# Run specific test categories
pytest tests/ -v --api-url http://localhost:8080 -k "health"      # Health checks
pytest tests/ -v --api-url http://localhost:8080 -k "shell"       # Shell execution
pytest tests/ -v --api-url http://localhost:8080 -k "browser"     # Browser automation
pytest tests/ -v --api-url http://localhost:8080 -k "screen"      # Desktop/screen
pytest tests/ -v --api-url http://localhost:8080 -k "multi_turn"  # Multi-turn workflows
pytest tests/ -v --api-url http://localhost:8080 -k "goal"        # Goal-oriented tasks

# Verbose output
pytest tests/ -v -s --api-url http://localhost:8080
```

### Test Categories

| Category | Description |
|----------|-------------|
| `TestHealthAndInfo` | Basic connectivity and status checks |
| `TestShellExecution` | Shell commands, env vars, pipelines, timeouts |
| `TestCodeExecution` | Python, bash execution and error handling |
| `TestFileOperations` | Read, write, list, nested directories |
| `TestBrowser` | Launch, navigate, screenshot, click, evaluate |
| `TestScreen` | Desktop screenshot, mouse, keyboard |
| `TestMultiTurnTasks` | Complex multi-step workflows |
| `TestStress` | Rapid commands, large files, concurrency |
| `TestGoalOrientedTasks` | Real-world automation scenarios |

### Multi-Turn Task Examples

The test suite includes realistic multi-turn scenarios:

- **Create and execute scripts** — Write code, run it, verify output
- **Web form interaction** — Navigate, fill forms, submit
- **File processing pipeline** — Create files, process, aggregate results
- **Iterative development** — Write buggy code, test, fix, re-test
- **Visual testing** — Navigate pages, compare screenshots
- **Dev environment setup** — Create project structure, run tests

### Screen Recording

Tests can automatically record the sandbox screen during execution for visual verification:

```bash
# Record all tests
./tests/run_tests.sh http://localhost:8080 --record

# Record specific tests
pytest tests/ -v --api-url http://localhost:8080 --record -k "browser"

# Custom recording directory and framerate
pytest tests/ -v --api-url http://localhost:8080 --record --record-dir ./my-recordings --record-fps 30
```

Recordings are saved as MP4 files organized by test class:
```
recordings/
├── TestBrowser/
│   ├── test_browser_launch_20241212_171500.mp4
│   ├── test_browser_navigate_20241212_171505.mp4
│   └── ...
├── TestMultiTurnTasks/
│   ├── test_web_form_interaction_20241212_171530.mp4
│   └── ...
└── ...
```

#### Manual Recording in Tests

For fine-grained control, use the `record_test` fixture:

```python
def test_custom_workflow(client, record_test):
    # Setup (not recorded)
    client.post("/browser/launch")

    # Record only the important part
    with record_test("my_workflow"):
        client.post("/browser/navigate", json={"url": "https://example.com"})
        client.post("/browser/click", json={"selector": "button"})
        # ... more actions ...

    # Cleanup (not recorded)
    client.post("/browser/close")
```

### Environment Variable

You can also set the API URL via environment variable:

```bash
export SANDBOX_API_URL=http://localhost:8080
pytest tests/ -v
```

## Configuration

Environment variables in `docker-compose.yml`:

| Variable | Default | Description |
|----------|---------|-------------|
| `SANDBOX_API_PORT` | 8080 | API server port |
| `VNC_PORT` | 5900 | VNC server port |
| `NOVNC_PORT` | 6080 | noVNC web port |
| `CDP_PORT` | 9222 | Chrome DevTools port |
| `BROWSER_HEADLESS` | false | Run browser headless |
| `WORKSPACE` | /home/sandbox/workspace | Working directory |

## Architecture

```
┌──────────────────────────────────────────────────┐
│                 Docker Container                 │
│  ┌────────────────────────────────────────────┐  │
│  │         FastAPI Control Server             │  │
│  │              (Port 8080)                   │  │
│  └──────────────────┬─────────────────────────┘  │
│                     │                            │
│    ┌────────────────┼────────────────┐          │
│    │                │                │          │
│    ▼                ▼                ▼          │
│ ┌──────┐      ┌──────────┐      ┌────────┐     │
│ │ PTY  │      │Playwright│      │  Nix   │     │
│ │Shell │      │ Browser  │      │Runtimes│     │
│ └──────┘      └──────────┘      └────────┘     │
│                     │                           │
│              ┌──────┴──────┐                   │
│              │   Xvfb :99  │                   │
│              │  (Virtual)  │                   │
│              └──────┬──────┘                   │
│                     │                           │
│         ┌──────────┴──────────┐               │
│         ▼                     ▼               │
│    ┌─────────┐          ┌─────────┐          │
│    │ x11vnc  │          │  noVNC  │          │
│    │ :5900   │          │  :6080  │          │
│    └─────────┘          └─────────┘          │
└──────────────────────────────────────────────────┘
```

## Extending

### Add more languages

Edit `nix/shell.nix` to add packages:

```nix
buildInputs = with pkgs; [
  # ... existing packages
  ruby
  php
  julia
];
```

Update `LANG_CONFIG` in `sandbox-api/main.py`:

```python
LANG_CONFIG = {
    # ... existing
    "ruby": {"ext": ".rb", "cmd": "ruby"},
    "php": {"ext": ".php", "cmd": "php"},
}
```

### Custom Nix configuration

For a full NixOS VM instead of nix-shell, create `nix/configuration.nix`:

```nix
{ config, pkgs, ... }:
{
  services.xserver.enable = true;
  # ... full NixOS config
}
```

## Security Notes

- The container runs with elevated privileges for Xvfb/VNC
- For production, consider:
  - Adding authentication to the API
  - Running behind a reverse proxy with TLS
  - Using resource limits (CPU/memory)
  - Network isolation

## License

Apache 2.0