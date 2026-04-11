# nixo

Reproducible, isolated sandbox environments for AI coding agents. Compose sandboxes from 88+ agents and 24+ tools using Nix, run them in Bubblewrap containers with configurable network and filesystem policies.
Primary CLI: `nixo`. The legacy `nixosandbox` binary remains available as a compatibility alias.

## What it does

nixo creates lightweight Linux sandboxes from a catalog of Nix packages. You pick the agent and tools you need, and it builds an isolated rootfs with just those packages — no Docker images, no VMs, no manual setup.

```bash
# Create a sandbox with Claude Code + Git + Python
nixo create --with claude-code,git,python312 --network off --json

# Run a command inside it
nixo exec <session-id> -- claude --version

# Or drop into an interactive shell
nixo enter <session-id>
```

## Architecture

```
nixo CLI (Rust)
  ├── Nix: builds rootfs from catalog packages
  ├── Bubblewrap: creates isolated mount/pid/net namespaces
  ├── Session manager: tracks sandbox lifecycle
  └── Catalog: 88+ AI agents (llm-agents.nix) + 24+ dev tools (nixpkgs)
```

**On Linux:** uses bubblewrap directly (setuid or user namespaces).
**On macOS:** uses a Docker sidecar container with bwrap inside.

## Install

### Homebrew

`nixo` is published in the `HashWarlock/nixo` tap, but Homebrew does not install Nix for you. Install the host Nix CLI first, then install the formula:

1. Install Nix on the host.
2. Install `nixo` from the tap.

```bash
brew tap HashWarlock/nixo
brew install nixo
nixo --help
nixosandbox --help  # compatibility alias
```

[`Formula/nixo.rb`](Formula/nixo.rb) is the in-repo Homebrew formula for the `HashWarlock/nixo` tap. It installs `nixo` as the primary executable and `nixosandbox` as a compatibility symlink. The packaged release ships `bin/` plus `flake/` assets, so the installed command does not require a checkout of this repository. At runtime, `nixo` still shells out to the host `nix` CLI; if `nix` is missing, the command exits with a focused error telling the user that the host Nix CLI is required.

Before publishing or updating the formula, run:

```bash
brew audit --strict nixo
brew test nixo
```

To publish the first tap-backed release from this repo:

1. Merge the formula and release workflow to `master`.
2. Create a version tag such as `v0.1.0`.
3. Wait for the release workflow to upload the tarballs.
4. Compute the real archive checksums and replace the placeholder `sha256` values in [`Formula/nixo.rb`](Formula/nixo.rb).
5. Commit that formula update on `master`, then users can install with `brew tap HashWarlock/nixo && brew install nixo`.

After tagged releases, the formula metadata in the tap is synced automatically by the release workflow.

### From source (requires Nix with flakes)

```bash
nix build github:HashWarlock/nixo
./result/bin/nixo --help
./result/bin/nixosandbox --help
```

### Development shell

```bash
nix develop
cargo build
```

## Quick start

### 1. Browse the catalog

```bash
nixo catalog
nixo catalog --filter claude
nixo catalog --json | jq '.agents | keys'
nixo catalog --json --grouped | jq '.agentCategories | keys'
```

### 2. Create a sandbox

```bash
# From catalog packages (compose what you need)
nixo create --with claude-code,bash,git --network off --name my-sandbox --json

# From a built-in profile
nixo create --profile strict --json

# With a host workspace mounted
nixo create --with opencode,bash --workspace ~/projects/myapp --json
```

### 3. Execute commands

```bash
# Run a single command
nixo exec <session-id> -- echo "Hello from sandbox"

# Stream NDJSON events (for programmatic use)
nixo exec <session-id> --json -- python3 -c "print('hello')"

# With extra environment variables
nixo exec <session-id> --env API_KEY=test -- node script.js
```

### 4. Interactive shell

```bash
nixo enter <session-id>
```

### 5. Manage sessions

```bash
nixo list                    # list all sessions
nixo list --json             # as JSON
nixo status <session-id>     # detailed session info
nixo destroy <session-id>    # clean up
```

## AgentSkills support

This repo ships a reusable AgentSkills-compatible skill at [`.agents/skills/nixo-cli/SKILL.md`](.agents/skills/nixo-cli/SKILL.md).

- Agents should prefer `nixo` in new instructions and treat `nixosandbox` as a compatibility alias.
- Use the bundled skill when the task involves sandbox lifecycle operations, catalog queries, session-id workflows, or common CLI errors.
- If your runtime supports the AgentSkills directory convention, copy the `nixo-cli` folder into that runtime's skills directory or install it from this repository using the runtime's skill-import flow.

## CLI reference

| Command | Description |
|---------|-------------|
| `create` | Create a new sandbox session |
| `exec` | Execute a command inside a sandbox |
| `enter` | Enter a sandbox interactively (bash) |
| `list` | List active sandbox sessions |
| `destroy` | Destroy a sandbox session |
| `status` | Show detailed session status |
| `build` | Build a rootfs without creating a session |
| `catalog` | List available packages from the catalog |

### `create` flags

| Flag | Description |
|------|-------------|
| `--with <pkg1,pkg2,...>` | Compose from catalog packages |
| `--profile <name>` | Use a built-in profile |
| `--spec <file>` | Use a custom JSON spec file |
| `--network <off\|full>` | Network mode (default: `off`) |
| `--workspace <path>` | Mount host directory as `/workspace` |
| `--name <name>` | Human-readable session name |
| `--agent <id>` | Agent identifier (e.g. `claude:opus-4-6`) |
| `--description <text>` | Purpose of this sandbox |
| `--json` | Output as JSON |

`--with`, `--profile`, and `--spec` are mutually exclusive.

### `catalog` flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON (flat compatibility by default) |
| `--grouped` | Group agent catalog JSON by category |
| `--filter <text>` | Filter package names and descriptions by substring |

## Catalog

The catalog merges two sources:

**AI agents** from [numtide/llm-agents.nix](https://github.com/numtide/llm-agents.nix) — all 88+ packages exposed dynamically, automatically updated when the flake input is bumped:

| Agent | Description |
|-------|-------------|
| `amp` | Sourcegraph coding agent |
| `claude-code` | Anthropic's CLI coding agent |
| `codex` | OpenAI's coding agent |
| `copilot-cli` | GitHub Copilot CLI |
| `cursor-agent` | Cursor's headless agent |
| `droid` | Factory AI's development agent |
| `forge` | Code forging agent |
| `gemini-cli` | Google Gemini CLI |
| `goose-cli` | Block's coding agent |
| `hermes-agent` | Nous Research self-improving agent |
| `jules` | Google's async coding agent |
| `openclaw` | OpenClaw AI assistant |
| `opencode` | Open-source coding agent |
| `pi` | Pi coding agent |
| `qwen-code` | Alibaba's coding agent |
| ... | 88+ agents total — run `nixo catalog` to see all |

**Development tools** from nixpkgs:

`python312` `nodejs_22` `rustc` `cargo` `go` `git` `coreutils` `bash` `findutils` `gnugrep` `gnused` `gawk` `gnumake` `gcc` `gnutar` `gzip` `curl` `cacert` `ripgrep` `fd` `jq` `less` `zsh` `nix`

Catalog output supports two JSON views:

- `nixo catalog --json` keeps the current flat compatibility shape with top-level `agents` and `tools`
- `nixo catalog --json --grouped` returns grouped agent categories under `agentCategories`

## Built-in profiles

| Profile | Network | Packages | Use case |
|---------|---------|----------|----------|
| `strict` | off | coreutils, bash, cacert | Minimal, locked-down |
| `offline-review` | off | git, coreutils, bash, grep, sed, jq | Code review without network |
| `build-install` | full | node, python, rust, git, gcc, make | Building and installing |
| `debug-network` | full | node, python, curl, netcat, dig | Network debugging |

## How it works

### Rootfs composition

When you run `nixo create --with claude-code,bash`, the CLI:

1. Resolves `claude-code` and `bash` from the catalog (agents first, then tools)
2. Calls `mkAgentSandbox` which delegates to `mkSandboxRootfs`
3. Nix builds a merged environment with all requested packages
4. Creates a minimal rootfs directory tree with symlinks into `/nix/store`

The rootfs contains: `/bin`, `/lib`, `/etc` (passwd, hosts, certs), `/usr/bin/env`, and mount points for `/workspace`, `/home/sandbox`, `/cache`, `/tmp`, `/dev`, `/proc`, `/nix/store`.

### Sandbox execution

When you run `nixo exec <id> -- command`:

1. Loads session metadata (rootfs path, network mode, profile)
2. Detects bubblewrap (native Linux or Docker sidecar on macOS)
3. Builds bwrap arguments:
   - `--ro-bind <rootfs> /` (read-only root)
   - `--ro-bind /nix/store /nix/store` (symlink targets)
   - `--bind <workspace> /workspace` (writable)
   - `--bind <home> /home/sandbox` (writable)
   - `--bind <cache> /cache` (writable)
   - `--tmpfs /tmp`, `--dev /dev`, `--proc /proc`
   - Namespace isolation (`--unshare-pid`, `--unshare-uts`, etc.)
   - `--unshare-net` when network is `off`
   - `--die-with-parent`, `--new-session` (lifecycle safety)
4. Spawns bwrap with the constructed arguments
5. In `--json` mode, streams NDJSON lifecycle/stdout/stderr/result events

### Session storage

Sessions live in `~/.local/share/nixosandbox/sessions/<session-id>/`:

```
<session-id>/
  metadata.json    # session config, profile, rootfs path
  workspace/       # working directory (or symlink to --workspace)
  home/            # persistent home directory
  cache/           # persistent cache
```

## Custom spec files

Create a JSON spec for full control:

```json
{
  "name": "my-environment",
  "packages": ["nodejs_22", "python312", "git"],
  "env": { "NODE_ENV": "development" },
  "network": "off",
  "namespaces": ["pid", "mount", "uts", "ipc", "net"],
  "writable": ["/workspace", "/home/sandbox", "/tmp"]
}
```

```bash
nixo create --spec my-env.json --json
```

## Environment variables

| Variable | Description |
|----------|-------------|
| `NIXOSANDBOX_FLAKE_ROOT` | Path to the nixosandbox flake (for development) |
| `NIXOSANDBOX_DATA_DIR` | Override session storage directory |
| `NIXOSANDBOX_BWRAP_PATH` | Explicit path to bwrap binary |
| `NIXOSANDBOX_NO_DOCKER` | Set to `1` to disable Docker fallback on macOS |

## Nix flake outputs

```nix
{
  # CLI binary
  packages.x86_64-linux.nixosandbox
  packages.x86_64-linux.default

  # Pre-built profile rootfs
  packages.x86_64-linux.sandbox-strict
  packages.x86_64-linux.sandbox-build-install
  packages.x86_64-linux.sandbox-offline-review
  packages.x86_64-linux.sandbox-debug-network

  # Library functions
  lib.mkSandboxRootfs   # { name, packages, env? } -> rootfs derivation
  lib.mkAgentSandbox    # { name, packages } -> rootfs (catalog-aware)

  # Queryable catalog
  catalog.agents        # attrset of agent packages
  catalog.tools         # attrset of tool packages
}
```

## Project structure

```
nixosandbox/
  crates/nixosandbox/       # Rust CLI
    src/
      main.rs               # Command handlers
      cli.rs                # Argument parsing (clap)
      session.rs            # Session CRUD and metadata
      nix.rs                # Nix build integration
      bubblewrap.rs         # bwrap detection
      docker.rs             # Docker sidecar (macOS)
      plan_builder.rs       # bwrap argv construction
      spec.rs               # Profile/spec loading
  nix/
    catalog.nix             # Unified agent + tool catalog
    mkSandboxRootfs.nix     # Rootfs builder
    mkAgentSandbox.nix      # Catalog-aware composition
    profiles/               # Built-in profile specs (JSON)
  packages/
    pi-sandbox-extension/   # TypeScript Pi agent integration
  .github/workflows/
    ci.yml                  # CI: Rust, TypeScript, Nix, agent smoke tests
```

## Pi extension

nixosandbox includes a [Pi coding agent](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent) extension that registers 7 sandbox tools: `sandbox_run`, `sandbox_read_file`, `sandbox_write_file`, `sandbox_list_files`, `sandbox_session_info`, `sandbox_catalog`, and `sandbox_browser`.

### Setup

1. Build the extension:

```bash
cd packages/pi-sandbox-extension
npm install
npm run build
```

2. Create an extension wrapper at `.pi/extensions/sandbox.ts` (project-local) or `~/.pi/agent/extensions/sandbox.ts` (global):

```typescript
import sandboxExtension from "<path-to-repo>/packages/pi-sandbox-extension/dist/index.js";

export default function (pi: any) {
  sandboxExtension(pi, {
    // Absolute path to the nixo binary (cargo build --release)
    // The nixosandbox alias remains available if your setup still expects it.
    binaryPath: "<path-to-repo>/crates/nixosandbox/target/release/nixo",
  });
}
```

3. Or load it directly with the `-e` flag:

```bash
pi -e .pi/extensions/sandbox.ts
```

### What the tools do

| Tool | Description |
|------|-------------|
| `sandbox_run` | Execute commands in an isolated sandbox (creates session on first use) |
| `sandbox_read_file` | Read a file from the sandbox workspace |
| `sandbox_write_file` | Write a file to the sandbox workspace |
| `sandbox_list_files` | List files in the sandbox workspace |
| `sandbox_session_info` | Show session details or list all sessions |
| `sandbox_catalog` | List available agent and tool packages |
| `sandbox_browser` | Browser automation (goto, screenshot, evaluate, click, type) |

### Environment

Set `NIXOSANDBOX_FLAKE_ROOT` to the repo root if the binary can't find `flake.nix` automatically:

```bash
export NIXOSANDBOX_FLAKE_ROOT=/path/to/nixo
```

## Testing

```bash
# Rust unit tests
cd crates/nixosandbox && cargo test

# Nix evaluation
nix flake check

# TypeScript typecheck
cd packages/pi-sandbox-extension && npx tsc --noEmit
```

CI runs agent smoke tests that create sandboxes with each agent (claude-code, codex, opencode, amp, droid, pi, openclaw, hermes-agent, jules) and verify the binary launches inside bwrap.

## License

Apache 2.0
