# Nix Flake Runtime Redesign — Design Spec

**Date:** 2026-04-08
**Branch:** `pi-sandbox-refactor`
**Prerequisite:** Docker fallback for macOS complete

---

## Overview

The nixosandbox project was intended to be a NixOS sandbox flake for easy agent integration. The current refactor (Phases 0–12 + Docker fallback) built solid infrastructure — bwrap supervision, NDJSON protocol, session management, Docker sidecar — but moved away from Nix toward host-derived binaries and Debian Docker images.

This spec puts Nix back at the center. Sandbox environments become Nix derivations — complete rootfs closures that bwrap pivots into. A standalone Rust CLI (`nixosandbox`) owns the full sandbox lifecycle. Agent frameworks (Pi, Claude Code, custom) are thin consumers of the CLI.

---

## Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Product shape | Standalone CLI + Nix flake | Agent-runtime-agnostic. Any framework can shell out to `nixosandbox`. |
| Rootfs strategy | `mkSandboxRootfs` builds minimal rootfs from package list | Not a full NixOS system — no systemd, no init. Fast to build, small on disk. |
| LLM integration | LLM generates JSON spec, not Nix code | Constrained schema is reliable; raw Nix is not. Verification via `nix build`. |
| Package resolution | Curated mapping (~200 entries) with `nix search` fallback (future) | Predictable for common tools; extensible for the long tail. |
| Execution model | `bwrap --pivot-root` into Nix rootfs | Complete filesystem isolation. Host is invisible from inside the sandbox. |
| CLI interface | Subcommands: create, exec, enter, list, destroy, build | Full lifecycle management. `--json` flag for programmatic consumers. |
| NDJSON protocol | Preserved as `--json` mode | Backward compatible with existing Pi integration; reuses event types. |
| Existing Rust code | Migrated into `nixosandbox` CLI binary | supervisor, plan_builder, validator, observer reused — not rewritten. |
| Docker fallback | Updated — mounts `/nix/store` into sidecar | Fastest option: host Nix store mounted read-only. No rebuild inside Docker. |
| Pi extension | Becomes thin CLI adapter | Keeps tool registration, approvals, UX. Delegates sessions/execution to CLI. |

---

## Architecture

```text
User / Agent Framework / Orchestrator Skill
  │
  │  shell out or --json
  ▼
┌──────────────────────────────────────────────┐
│ nixosandbox CLI (Rust binary)                │
│                                              │
│  create ──► nix build mkSandboxRootfs        │
│  exec   ──► bwrap --pivot-root + supervise   │
│  enter  ──► exec -- $SHELL (interactive)     │
│  list   ──► read session metadata            │
│  destroy ──► kill + cleanup session dirs     │
│  build  ──► nix build only (no session)      │
│                                              │
│  Internals (migrated from pi-sandbox-runtime)│
│  ├─ plan_builder.rs (gains build_rootfs)     │
│  ├─ supervisor.rs (unchanged)                │
│  ├─ validator.rs (gains rootfs validation)   │
│  ├─ observer.rs (unchanged)                  │
│  ├─ docker.rs (updated mount paths)          │
│  └─ bubblewrap.rs (unchanged)               │
└──────────────┬───────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────────┐
│ bwrap --pivot-root /nix/store/...-rootfs     │
│                                              │
│  /bin/node    ─┐                             │
│  /bin/python3  ├─ from Nix closure (ro)      │
│  /bin/git     ─┘                             │
│  /workspace   ──── session dir (rw)          │
│  /home/sandbox ─── session dir (rw)          │
│  /cache       ──── session dir (rw)          │
│  /tmp         ──── tmpfs                     │
│  /dev         ──── devtmpfs                  │
│  /proc        ──── procfs                    │
│                                              │
│  Host filesystem: invisible                  │
└──────────────────────────────────────────────┘
```

### On macOS (Docker fallback)

```text
nixosandbox CLI (macOS native)
  │
  │  docker exec -i pi-sandbox-sidecar bwrap --pivot-root ...
  ▼
┌──────────────────────────────────────────────┐
│ Docker sidecar (pi-sandbox-sidecar)          │
│  -v /nix/store:/nix/store:ro                 │
│  -v <sessions_dir>:/nixosandbox:rw           │
│  --cap-add SYS_ADMIN --cap-add NET_ADMIN     │
│  --security-opt seccomp=unconfined           │
│                                              │
│  bwrap --pivot-root /nix/store/...-rootfs    │
│  (same as Linux, runs inside Docker)         │
└──────────────────────────────────────────────┘
```

The host's `/nix/store` is mounted read-only into the sidecar. No need to build anything inside Docker. This is the fastest path — instant if the Nix closure is already built.

---

## Flake Structure

### `flake.nix` outputs

```nix
{
  outputs = { self, nixpkgs, ... }: {
    # Library function for building custom rootfs
    lib.mkSandboxRootfs = { name, packages, env ? {}, ... }: ...;

    # Pre-built rootfs for each built-in profile
    packages.x86_64-linux = {
      nixosandbox = ...;               # The CLI binary
      sandbox-build-install = ...;     # Rootfs derivation
      sandbox-offline-review = ...;
      sandbox-strict = ...;
      sandbox-debug-network = ...;
    };

    # Development shell for working on nixosandbox itself
    devShells.x86_64-linux.default = ...;
  };
}
```

### `mkSandboxRootfs` behavior

Input: a sandbox spec (name, packages, env, etc.)

Output: a derivation producing a directory tree:

```text
/nix/store/<hash>-sandbox-<name>/
  bin/           → symlinks to package bins (via buildEnv)
  lib/           → shared libraries
  etc/
    ssl/certs/   → CA certificates
    passwd       → sandbox user entry
    group        → sandbox group entry
    nsswitch.conf
  usr/
    bin/env      → for #!/usr/bin/env shebangs
```

Built using `pkgs.buildEnv` + `pkgs.runCommand` to assemble the tree. Fast to build (seconds if packages are cached). Small on disk (symlinks into Nix store).

Mountpoints (`/tmp`, `/dev`, `/proc`, `/workspace`, `/home`) are NOT in the derivation — bwrap creates them at runtime.

---

## Sandbox Spec Format

The intermediate representation between natural language and Nix. Validated against a JSON schema.

```json
{
  "name": "web-dev",
  "packages": ["nodejs_22", "postgresql_16", "git", "curl", "jq", "python312"],
  "env": {
    "NODE_ENV": "development"
  },
  "network": "full",
  "namespaces": ["pid", "mount", "uts", "ipc"],
  "writable": ["/workspace", "/home/sandbox", "/cache", "/tmp"]
}
```

### Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Unique name for the environment |
| `packages` | string[] | yes | Exact nixpkgs attribute names |
| `env` | object | no | Environment variables set inside sandbox |
| `network` | `"off"` \| `"full"` | no (default: `"full"`) | Network isolation mode |
| `namespaces` | string[] | no (default: `["pid","mount","uts","ipc"]`) | Linux namespaces to unshare |
| `writable` | string[] | no (default: `["/workspace","/home/sandbox","/cache","/tmp"]`) | Paths writable inside sandbox |

### Built-in profiles

Shipped as spec files in `nix/profiles/`:

**`build-install.json`** (default):
```json
{
  "name": "build-install",
  "packages": ["nodejs_22", "python312", "rustc", "cargo", "git", "curl", "cacert", "coreutils", "bash", "gnugrep", "gnused", "gawk", "findutils", "gnutar", "gzip", "gnumake", "gcc"],
  "env": {},
  "network": "full",
  "namespaces": ["pid", "mount", "uts", "ipc"],
  "writable": ["/workspace", "/home/sandbox", "/cache", "/tmp"]
}
```

**`offline-review.json`**:
```json
{
  "name": "offline-review",
  "packages": ["git", "cacert", "coreutils", "bash", "gnugrep", "gnused", "gawk", "findutils", "jq"],
  "env": {},
  "network": "off",
  "namespaces": ["pid", "mount", "uts", "ipc", "net"],
  "writable": ["/workspace", "/home/sandbox", "/tmp"]
}
```

**`strict.json`**:
```json
{
  "name": "strict",
  "packages": ["coreutils", "bash", "cacert"],
  "env": {},
  "network": "off",
  "namespaces": ["pid", "mount", "uts", "ipc", "net"],
  "writable": ["/tmp"]
}
```

**`debug-network.json`**:
```json
{
  "name": "debug-network",
  "packages": ["nodejs_22", "python312", "git", "curl", "cacert", "coreutils", "bash", "inetutils", "netcat", "dig"],
  "env": {},
  "network": "full",
  "namespaces": ["pid", "mount", "uts", "ipc"],
  "writable": ["/workspace", "/home/sandbox", "/cache", "/tmp"]
}
```

---

## Package Resolution

### Curated Mapping (`nix/packages.json`)

Maps common tool names and aliases to exact nixpkgs attributes:

```json
{
  "node": {
    "attr": "nodejs_22",
    "aliases": ["nodejs", "node.js", "node22", "node-22"],
    "extra": []
  },
  "python": {
    "attr": "python312",
    "aliases": ["python3", "py", "python3.12"],
    "extra": ["python312Packages.pip"]
  },
  "rust": {
    "attr": "rustc",
    "aliases": ["rustlang"],
    "extra": ["cargo", "rustfmt", "clippy"]
  },
  "go": {
    "attr": "go",
    "aliases": ["golang"],
    "extra": []
  },
  "postgres": {
    "attr": "postgresql_16",
    "aliases": ["postgresql", "pg", "psql"],
    "extra": []
  },
  "git": {
    "attr": "git",
    "aliases": [],
    "extra": []
  }
}
```

~200 entries covering common development tools, languages, databases, and utilities.

### Resolution chain (v1)

1. Look up in curated mapping (exact match or alias match)
2. If match found, expand to `attr` + `extra` packages
3. If no match, treat as a literal nixpkgs attribute name (user knows what they want)
4. Validate all resolved attributes exist via `nix eval`

### Resolution chain (future, out of scope for v1)

Steps 1–3 above, plus:
4. If literal attribute doesn't exist, fall back to `nix search nixpkgs#<term>`
5. If still unresolved, ask user for clarification

---

## CLI Interface

### `nixosandbox create`

```
nixosandbox create [OPTIONS]

Options:
  --profile <NAME>       Use a built-in profile (build-install, offline-review, strict, debug-network)
  --spec <PATH>          Use a custom spec file
  --workspace <PATH>     Host directory to mount as /workspace (default: current directory)
  --name <NAME>          Human-readable session name (default: auto-generated)
  --json                 Output session info as JSON

Creates a sandbox session. Builds the rootfs if not cached.
Prints the session ID (or JSON with --json).
```

### `nixosandbox exec`

```
nixosandbox exec [OPTIONS] <SESSION-ID> -- <COMMAND...>

Options:
  --json                 Stream NDJSON events (lifecycle, stdout, stderr, result)
  --timeout <SECONDS>    Kill after timeout (default: none)
  --env <KEY=VALUE>      Additional environment variable (repeatable)

Executes a command inside the sandbox. Returns the command's exit code.
With --json, streams the same NDJSON event types as the current pi-sandbox-runtime.
```

### `nixosandbox enter`

```
nixosandbox enter <SESSION-ID>

Shorthand for: nixosandbox exec <SESSION-ID> -- /bin/bash
Opens an interactive shell inside the sandbox.
```

### `nixosandbox list`

```
nixosandbox list [OPTIONS]

Options:
  --json                 Output as JSON array

Lists active sandbox sessions with ID, name, profile, workspace, and created timestamp.
```

### `nixosandbox destroy`

```
nixosandbox destroy <SESSION-ID>

Kills any running processes in the sandbox, removes session directories.
Does NOT remove the Nix rootfs (it's in /nix/store, managed by nix-collect-garbage).
```

### `nixosandbox build`

```
nixosandbox build [OPTIONS]

Options:
  --profile <NAME>       Build rootfs for a built-in profile
  --spec <PATH>          Build rootfs from a custom spec file
  --json                 Output rootfs path as JSON

Builds the rootfs derivation without creating a session.
Useful for CI, caching, or pre-warming.
```

---

## bwrap Execution with Nix Rootfs

### New `build_rootfs()` in plan_builder.rs

The existing `build()` and `build_with_allowlist()` functions stay for backward compatibility. A new `build_rootfs()` function produces bwrap argv for the pivot-root model:

```rust
pub fn build_rootfs(
    rootfs_path: &str,
    session_dirs: &SessionDirs,
    effective_state: &EffectiveState,
) -> Vec<String> {
    // --pivot-root <rootfs> /oldroot
    // --tmpfs /oldroot (hide host)
    // --bind <session_workspace> /workspace
    // --bind <session_home> /home/sandbox
    // --bind <session_cache> /cache
    // --tmpfs /tmp
    // --dev /dev
    // --proc /proc
    // --unshare-pid --unshare-ipc --unshare-uts
    // --unshare-net (if network == "off")
    // --clearenv
    // --setenv HOME /home/sandbox
    // --setenv PATH /bin
    // --chdir /workspace
}
```

### SessionDirs struct

```rust
pub struct SessionDirs {
    pub workspace: String,      // host path → /workspace
    pub home: String,           // host path → /home/sandbox
    pub cache: String,          // host path → /cache
    pub logs: String,           // host path (not mounted, for CLI logs)
    pub metadata: String,       // host path (session metadata file)
}
```

### Docker path (macOS)

On macOS with Docker:
1. Sidecar is started with `-v /nix/store:/nix/store:ro -v <sessions_dir>:/nixosandbox:rw`
2. `build_rootfs()` produces the same argv
3. Supervisor prefixes with `docker exec -i <container_id> bwrap`
4. Path rewriting: session dirs are rewritten from host to container paths (existing logic)
5. Nix store paths need NO rewriting — `/nix/store` is the same path on host and in container

---

## Session Management

### Session directory layout

```text
~/.local/share/nixosandbox/sessions/<session-id>/
  metadata.json
  workspace/         # writable (or symlink to --workspace path)
  home/              # writable /home/sandbox
  cache/             # writable /cache (npm, pip, cargo caches)
  logs/              # CLI execution logs
```

### metadata.json

```json
{
  "sessionId": "a1b2c3d4",
  "name": "my-project",
  "profile": "build-install",
  "rootfsPath": "/nix/store/abc123-sandbox-build-install",
  "workspace": "/home/user/projects/myapp",
  "createdAt": "2026-04-08T12:00:00Z",
  "lastExecAt": "2026-04-08T12:05:00Z",
  "pid": null
}
```

### Lifecycle

- **create**: Build rootfs (if not cached), create session dirs, write metadata. If `--workspace` points to an existing directory, symlink `sessions/<id>/workspace` to it. If omitted, create an empty `workspace/` directory inside the session.
- **exec**: Read metadata, build bwrap argv, supervise process, update lastExecAt
- **destroy**: Kill pid if running, remove session directory. Does NOT remove the `--workspace` directory if it was an external symlink. Rootfs stays in Nix store.
- **Garbage collection**: `nix-collect-garbage` removes unused rootfs derivations. The CLI does not manage Nix store directly.

---

## Migration Path

### Renames and moves

| Current | Becomes |
|---|---|
| `crates/pi-sandbox-runtime/` | `crates/nixosandbox/` |
| `pi-sandbox-runtime` binary | `nixosandbox` binary |
| `nix/shell.nix` | Deleted (replaced by `flake.nix` devShell) |
| `docker-compose.yml` | Deleted (legacy) |

### New files

| Path | Responsibility |
|---|---|
| `flake.nix` | Flake definition: mkSandboxRootfs, packages, devShell |
| `flake.lock` | Pinned nixpkgs |
| `nix/mkSandboxRootfs.nix` | Rootfs builder function |
| `nix/packages.json` | Curated package mapping |
| `nix/profiles/build-install.json` | Built-in profile spec |
| `nix/profiles/offline-review.json` | Built-in profile spec |
| `nix/profiles/strict.json` | Built-in profile spec |
| `nix/profiles/debug-network.json` | Built-in profile spec |
| `crates/nixosandbox/src/cli.rs` | CLI argument parsing (clap) |
| `crates/nixosandbox/src/session.rs` | Session create/list/destroy |
| `crates/nixosandbox/src/nix.rs` | Nix build invocation, spec validation |

### Modified files

| Path | Change |
|---|---|
| `crates/nixosandbox/src/plan_builder.rs` | Add `build_rootfs()` function |
| `crates/nixosandbox/src/main.rs` | Replace NDJSON-only entry with clap CLI |
| `crates/nixosandbox/src/validator.rs` | Add rootfs validation path |
| `crates/nixosandbox/src/docker.rs` | Update sidecar to mount `/nix/store`, update path rewriting |
| `crates/nixosandbox/src/supervisor.rs` | Accept rootfs-mode argv (minimal change) |
| `crates/nixosandbox/Cargo.toml` | Add `clap` dependency |
| `packages/pi-sandbox-extension/src/runtime-base.ts` | Simplify to call `nixosandbox build` |
| `packages/pi-sandbox-extension/src/session-manager.ts` | Delegate to `nixosandbox create`/`exec` |
| `packages/pi-sandbox-extension/src/profiles.ts` | Read from spec files or delegate to CLI |

### Deleted files

| Path | Reason |
|---|---|
| `nix/shell.nix` | Replaced by flake.nix devShell |
| `docker-compose.yml` | Legacy from old server |

---

## Testing

### Unit tests (Rust, any platform)

- `build_rootfs()` produces correct bwrap argv for a given rootfs path + session dirs
- Session metadata serialization/deserialization
- Spec validation (valid spec passes, invalid spec rejected)
- Package resolution from curated mapping

### Integration tests (Linux with Nix)

- `nix build .#sandbox-build-install` produces a rootfs with expected binaries
- `nixosandbox create --profile build-install` creates session dirs + metadata
- `nixosandbox exec <id> -- echo hello` produces stdout "hello", exit code 0
- `nixosandbox exec <id> -- which node` returns `/bin/node` (Nix closure, not host)
- `nixosandbox exec <id> -- ls /` shows sandbox rootfs, not host
- `nixosandbox destroy <id>` cleans up session dirs
- `nixosandbox list` shows/hides sessions correctly
- `nixosandbox exec --json <id> -- echo test` produces valid NDJSON stream

### Docker tests (macOS with Docker + Nix, gated behind env var)

- Sidecar starts with `/nix/store` mount
- `nixosandbox exec` works through Docker sidecar
- Nix store paths are accessible inside sidecar without rebuilding

### Backward compatibility

- `nixosandbox exec --json` output matches existing NDJSON protocol event types
- Pi extension can create and exec sandboxes via CLI

---

## What Is NOT in This Spec

- **Natural language spec generation** — Future skill that wraps the CLI. CLI only accepts spec files and profiles in v1.
- **`nix search` fallback** — v1 uses curated mapping only. Users specify exact attrs for unmapped packages.
- **Background services** — No PostgreSQL/Redis running as daemons inside sandboxes. Packages are installed but not started.
- **Resource limits via cgroups** — Deferred. bwrap namespace isolation only.
- **Sandbox snapshots** — No checkpoint/restore.
- **Remote execution** — No SSH to remote NixOS hosts.
- **Multi-architecture** — x86_64-linux only in v1. aarch64-linux added later.
- **NixOS module profiles** — Profiles are simple package lists, not NixOS configurations.
- **Orchestrator skill** — The skill that manages multiple sandboxes for subagents is built on top of this CLI, not part of it.

---

## Phase Gate

| Criteria |
|----------|
| `nix build .#sandbox-build-install` produces a rootfs with node, python, git, rust, curl |
| `nixosandbox create --profile build-install --workspace /tmp/test` creates a session |
| `nixosandbox exec <id> -- node -e "console.log('hello')"` prints "hello", exit 0 |
| `nixosandbox exec <id> -- which git` returns `/bin/git` (Nix, not host) |
| `nixosandbox exec --json <id> -- echo test` produces valid NDJSON event stream |
| `nixosandbox list` shows the session |
| `nixosandbox destroy <id>` cleans up |
| On macOS with Docker: same commands work via sidecar with `/nix/store` mount |
| Host filesystem is invisible from inside the sandbox (`ls /` shows rootfs, not host) |
| Pi extension can create and exec sandboxes via `nixosandbox` CLI |
| All existing Rust unit tests pass (supervisor, plan_builder, validator, observer) |
