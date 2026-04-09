# Nix Flake Runtime Redesign — Implementation Plan (Part A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone `nixosandbox` CLI + Nix flake that creates reproducible sandbox environments from Nix derivations and executes commands inside them via bwrap `--pivot-root`.

**Architecture:** A Nix flake at the repo root exports `mkSandboxRootfs` (builds rootfs derivations from package lists) and a Rust CLI binary (`nixosandbox`) that manages sandbox sessions (create/exec/enter/list/destroy/build). The existing bwrap supervision code from `pi-sandbox-runtime` is migrated into the new CLI crate. bwrap uses `--pivot-root` into the Nix-built rootfs instead of `--ro-bind` of individual host paths.

**Tech Stack:** Nix flakes, Rust (clap for CLI), bubblewrap, serde_json

**Design Spec:** `docs/superpowers/specs/2026-04-08-nix-flake-runtime-design.md`

**Scope:** This is Part A — the standalone tool on Linux with Nix. Part B (Docker sidecar updates, Pi extension simplification) follows in a separate plan.

---

## File Map

### New Files

| Path | Responsibility |
|------|----------------|
| `flake.nix` | Flake definition: inputs, mkSandboxRootfs, packages, devShell |
| `nix/mkSandboxRootfs.nix` | Nix function: takes package list, builds rootfs directory tree |
| `nix/profiles/build-install.json` | Built-in profile spec |
| `nix/profiles/offline-review.json` | Built-in profile spec |
| `nix/profiles/strict.json` | Built-in profile spec |
| `nix/profiles/debug-network.json` | Built-in profile spec |
| `nix/packages.json` | Curated package name to nixpkgs attribute mapping |
| `crates/nixosandbox/src/cli.rs` | clap CLI argument parsing and dispatch |
| `crates/nixosandbox/src/session.rs` | Session create/list/destroy + metadata |
| `crates/nixosandbox/src/nix.rs` | Nix build invocation, spec loading, package resolution |
| `crates/nixosandbox/src/spec.rs` | Sandbox spec types + JSON schema validation |

### Modified Files (after crate rename)

| Path | Change |
|------|--------|
| `crates/nixosandbox/Cargo.toml` | Rename package, add `clap` dependency |
| `crates/nixosandbox/src/main.rs` | Replace NDJSON-only entry point with clap CLI dispatch |
| `crates/nixosandbox/src/plan_builder.rs` | Add `build_rootfs()` function for pivot-root bwrap argv |
| `tests/protocol/globalSetup.ts` | Update crate path reference |
| `tests/protocol/helpers.ts` | Pass `legacy-ndjson` subcommand to runtime |

### Deleted Files

| Path | Reason |
|------|--------|
| `nix/shell.nix` | Replaced by `flake.nix` devShell |
| `docker-compose.yml` | Legacy from old server |

---

### Task 1: Nix flake + mkSandboxRootfs + built-in profiles

**Files:**
- Create: `flake.nix`
- Create: `nix/mkSandboxRootfs.nix`
- Create: `nix/profiles/build-install.json`
- Create: `nix/profiles/offline-review.json`
- Create: `nix/profiles/strict.json`
- Create: `nix/profiles/debug-network.json`
- Delete: `nix/shell.nix`

This task builds the Nix side end-to-end: a flake that can build rootfs derivations from profile specs. After this task, `nix build .#sandbox-strict` produces a usable rootfs directory.

- [ ] **Step 1: Create `nix/mkSandboxRootfs.nix`**

```nix
# nix/mkSandboxRootfs.nix
#
# Builds a minimal rootfs directory tree from a list of Nix packages.
# The output is suitable for bwrap --pivot-root.
#
# Usage: mkSandboxRootfs { name = "my-env"; packages = [ pkgs.nodejs pkgs.git ]; }
{ pkgs }:

{ name, packages, env ? {} }:

let
  # Create a merged environment with all requested packages
  mergedEnv = pkgs.buildEnv {
    name = "sandbox-env-${name}";
    paths = packages;
    pathsToLink = [ "/bin" "/lib" "/lib64" "/share" "/etc" "/include" ];
    extraOutputsToInstall = [ "out" ];
  };
in
pkgs.runCommand "sandbox-${name}" {
  passthru = { inherit name env; };
} ''
  mkdir -p $out/{bin,lib,lib64,etc,usr/bin,tmp,dev,proc,workspace,home/sandbox,cache}

  # Symlink all binaries from the merged environment
  if [ -d "${mergedEnv}/bin" ]; then
    for f in ${mergedEnv}/bin/*; do
      ln -sf "$f" "$out/bin/$(basename $f)"
    done
  fi

  # Symlink libraries
  if [ -d "${mergedEnv}/lib" ]; then
    for f in ${mergedEnv}/lib/*; do
      ln -sf "$f" "$out/lib/$(basename $f)"
    done
  fi
  if [ -d "${mergedEnv}/lib64" ]; then
    for f in ${mergedEnv}/lib64/*; do
      ln -sf "$f" "$out/lib64/$(basename $f)"
    done
  fi

  # Symlink share (man pages, etc.)
  if [ -d "${mergedEnv}/share" ]; then
    ln -sf "${mergedEnv}/share" "$out/share"
  fi

  # /usr/bin/env -- needed for #!/usr/bin/env shebangs
  ln -sf "${mergedEnv}/bin/env" "$out/usr/bin/env" 2>/dev/null || \
    ln -sf "${pkgs.coreutils}/bin/env" "$out/usr/bin/env"

  # /etc/ssl/certs -- CA certificates
  mkdir -p $out/etc/ssl/certs
  if [ -e "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" ]; then
    ln -sf "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" "$out/etc/ssl/certs/ca-certificates.crt"
    ln -sf "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" "$out/etc/ssl/certs/ca-bundle.crt"
  fi

  # /etc/passwd and /etc/group -- minimal entries for sandbox user
  cat > $out/etc/passwd <<'PASSWD'
root:x:0:0:root:/root:/bin/bash
sandbox:x:1000:1000:sandbox:/home/sandbox:/bin/bash
nobody:x:65534:65534:nobody:/nonexistent:/usr/bin/nologin
PASSWD

  cat > $out/etc/group <<'GROUP'
root:x:0:
sandbox:x:1000:
nobody:x:65534:
GROUP

  # /etc/nsswitch.conf
  cat > $out/etc/nsswitch.conf <<'NSS'
passwd: files
group: files
hosts: files dns
NSS

  # /etc/hosts -- minimal
  cat > $out/etc/hosts <<'HOSTS'
127.0.0.1 localhost
::1       localhost
HOSTS

  # Nix store reference -- keep a file that references the merged env
  # so nix-collect-garbage knows this rootfs depends on those packages
  echo "${mergedEnv}" > $out/.nix-env-reference
''
```

- [ ] **Step 2: Create the four built-in profile specs**

Create `nix/profiles/build-install.json`:
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

Create `nix/profiles/offline-review.json`:
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

Create `nix/profiles/strict.json`:
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

Create `nix/profiles/debug-network.json`:
```json
{
  "name": "debug-network",
  "packages": ["nodejs_22", "python312", "git", "curl", "cacert", "coreutils", "bash", "inetutils", "netcat-gnu", "dig"],
  "env": {},
  "network": "full",
  "namespaces": ["pid", "mount", "uts", "ipc"],
  "writable": ["/workspace", "/home/sandbox", "/cache", "/tmp"]
}
```

- [ ] **Step 3: Create `flake.nix`**

```nix
{
  description = "nixosandbox -- reproducible, isolated sandbox environments";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
  };

  outputs = { self, nixpkgs }:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    mkSandboxRootfs = import ./nix/mkSandboxRootfs.nix { inherit pkgs; };

    # Helper: load a profile JSON and resolve package names to nixpkgs attrs
    loadProfile = path:
      let
        spec = builtins.fromJSON (builtins.readFile path);
        resolvedPkgs = map (name:
          if builtins.hasAttr name pkgs
          then builtins.getAttr name pkgs
          else throw "nixosandbox: unknown package '${name}' in profile ${spec.name}"
        ) spec.packages;
      in
        mkSandboxRootfs {
          name = spec.name;
          packages = resolvedPkgs;
          env = spec.env or {};
        };
  in
  {
    # Library function for custom rootfs
    lib.mkSandboxRootfs = mkSandboxRootfs;

    packages.${system} = {
      # Pre-built rootfs for each profile
      sandbox-build-install = loadProfile ./nix/profiles/build-install.json;
      sandbox-offline-review = loadProfile ./nix/profiles/offline-review.json;
      sandbox-strict = loadProfile ./nix/profiles/strict.json;
      sandbox-debug-network = loadProfile ./nix/profiles/debug-network.json;

      # Default package is the CLI (wired in Task 10)
      # nixosandbox = ...;
    };

    devShells.${system}.default = pkgs.mkShell {
      name = "nixosandbox-dev";
      buildInputs = with pkgs; [
        rustc
        cargo
        pkg-config
        bubblewrap
        jq
      ];
    };
  };
}
```

- [ ] **Step 4: Delete `nix/shell.nix`**

```bash
rm nix/shell.nix
```

- [ ] **Step 5: Build the strict profile to verify the flake works**

Run: `nix build .#sandbox-strict --no-link --print-out-paths`

Expected: A Nix store path like `/nix/store/...-sandbox-strict`. No errors.

- [ ] **Step 6: Verify the rootfs has expected contents**

Run:
```bash
ROOTFS=$(nix build .#sandbox-strict --no-link --print-out-paths)
ls $ROOTFS/bin/ | head -20
ls $ROOTFS/etc/
cat $ROOTFS/etc/passwd
test -L $ROOTFS/usr/bin/env && echo "env symlink OK"
```

Expected: `bin/` contains bash, coreutils binaries. `etc/` has passwd, group, nsswitch.conf, hosts, ssl/. `usr/bin/env` symlink exists.

- [ ] **Step 7: Build the build-install profile (larger, confirms package resolution)**

Run: `nix build .#sandbox-build-install --no-link --print-out-paths`

Expected: Store path. Takes longer (more packages) but should succeed.

- [ ] **Step 8: Commit**

```bash
git add flake.nix nix/mkSandboxRootfs.nix nix/profiles/
git rm nix/shell.nix
git commit -m "feat: add Nix flake with mkSandboxRootfs and built-in profiles"
```

---

### Task 2: Curated package mapping

**Files:**
- Create: `nix/packages.json`

The curated mapping allows natural-language-style package names to resolve to exact nixpkgs attributes. This file is consumed by the Rust CLI for spec validation and by the future NL skill.

- [ ] **Step 1: Create `nix/packages.json`**

```json
{
  "node": { "attr": "nodejs_22", "aliases": ["nodejs", "node.js", "node22"], "extra": [] },
  "python": { "attr": "python312", "aliases": ["python3", "py", "python3.12"], "extra": ["python312Packages.pip"] },
  "rust": { "attr": "rustc", "aliases": ["rustlang"], "extra": ["cargo", "rustfmt", "clippy"] },
  "go": { "attr": "go", "aliases": ["golang", "go-lang"], "extra": [] },
  "git": { "attr": "git", "aliases": [], "extra": [] },
  "curl": { "attr": "curl", "aliases": ["libcurl"], "extra": [] },
  "wget": { "attr": "wget", "aliases": [], "extra": [] },
  "jq": { "attr": "jq", "aliases": [], "extra": [] },
  "ripgrep": { "attr": "ripgrep", "aliases": ["rg"], "extra": [] },
  "fd": { "attr": "fd", "aliases": ["fd-find"], "extra": [] },
  "tree": { "attr": "tree", "aliases": [], "extra": [] },
  "tmux": { "attr": "tmux", "aliases": [], "extra": [] },
  "vim": { "attr": "vim", "aliases": ["vi"], "extra": [] },
  "neovim": { "attr": "neovim", "aliases": ["nvim"], "extra": [] },
  "postgres": { "attr": "postgresql_16", "aliases": ["postgresql", "pg", "psql"], "extra": [] },
  "redis": { "attr": "redis", "aliases": [], "extra": [] },
  "sqlite": { "attr": "sqlite", "aliases": ["sqlite3"], "extra": [] },
  "make": { "attr": "gnumake", "aliases": ["gmake"], "extra": [] },
  "cmake": { "attr": "cmake", "aliases": [], "extra": [] },
  "gcc": { "attr": "gcc", "aliases": ["gnu-cc"], "extra": [] },
  "clang": { "attr": "clang", "aliases": ["llvm-clang"], "extra": ["llvmPackages.llvm"] },
  "ruby": { "attr": "ruby", "aliases": ["ruby3"], "extra": [] },
  "php": { "attr": "php", "aliases": ["php83"], "extra": [] },
  "java": { "attr": "jdk", "aliases": ["jdk", "openjdk"], "extra": [] },
  "maven": { "attr": "maven", "aliases": ["mvn"], "extra": [] },
  "terraform": { "attr": "terraform", "aliases": ["tf"], "extra": [] },
  "kubectl": { "attr": "kubectl", "aliases": ["kube"], "extra": [] },
  "aws": { "attr": "awscli2", "aliases": ["aws-cli", "awscli"], "extra": [] },
  "ssh": { "attr": "openssh", "aliases": ["openssh"], "extra": [] },
  "openssl": { "attr": "openssl", "aliases": ["libssl"], "extra": [] },
  "htop": { "attr": "htop", "aliases": [], "extra": [] },
  "less": { "attr": "less", "aliases": [], "extra": [] },
  "unzip": { "attr": "unzip", "aliases": [], "extra": [] },
  "zip": { "attr": "zip", "aliases": [], "extra": [] },
  "tar": { "attr": "gnutar", "aliases": ["gtar"], "extra": [] },
  "gzip": { "attr": "gzip", "aliases": ["gz"], "extra": [] },
  "bash": { "attr": "bash", "aliases": [], "extra": [] },
  "zsh": { "attr": "zsh", "aliases": [], "extra": [] },
  "fish": { "attr": "fish", "aliases": [], "extra": [] },
  "coreutils": { "attr": "coreutils", "aliases": [], "extra": [] },
  "findutils": { "attr": "findutils", "aliases": ["find"], "extra": [] },
  "grep": { "attr": "gnugrep", "aliases": ["gnugrep"], "extra": [] },
  "sed": { "attr": "gnused", "aliases": ["gnused"], "extra": [] },
  "awk": { "attr": "gawk", "aliases": ["gawk"], "extra": [] },
  "cacert": { "attr": "cacert", "aliases": ["ca-certificates", "ca-certs"], "extra": [] },
  "netcat": { "attr": "netcat-gnu", "aliases": ["nc", "ncat"], "extra": [] },
  "dig": { "attr": "dig", "aliases": ["bind-tools", "nslookup"], "extra": [] },
  "inetutils": { "attr": "inetutils", "aliases": ["hostname", "ping"], "extra": [] },
  "imagemagick": { "attr": "imagemagick", "aliases": ["convert", "magick"], "extra": [] },
  "ffmpeg": { "attr": "ffmpeg", "aliases": ["ffprobe"], "extra": [] },
  "pandoc": { "attr": "pandoc", "aliases": [], "extra": [] },
  "latex": { "attr": "texliveFull", "aliases": ["texlive", "pdflatex"], "extra": [] }
}
```

- [ ] **Step 2: Verify it is valid JSON**

Run: `cat nix/packages.json | jq . > /dev/null && echo "Valid JSON"`

Expected: `Valid JSON`

- [ ] **Step 3: Commit**

```bash
git add nix/packages.json
git commit -m "feat: add curated package mapping (nix/packages.json)"
```

---

### Task 3: Rename crate + add clap CLI skeleton

**Files:**
- Rename: `crates/pi-sandbox-runtime/` to `crates/nixosandbox/`
- Modify: `crates/nixosandbox/Cargo.toml`
- Create: `crates/nixosandbox/src/cli.rs`
- Modify: `crates/nixosandbox/src/main.rs`

This task renames the crate, adds clap, and creates a CLI skeleton that dispatches to subcommands. The existing NDJSON entry point is preserved as a hidden `legacy-ndjson` subcommand for backward compatibility.

- [ ] **Step 1: Rename the crate directory**

```bash
mv crates/pi-sandbox-runtime crates/nixosandbox
```

- [ ] **Step 2: Update `Cargo.toml`**

Replace the contents of `crates/nixosandbox/Cargo.toml`:

```toml
[package]
name = "nixosandbox"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "nixosandbox"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
uuid = { version = "1", features = ["v4"] }

[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

- [ ] **Step 3: Create `crates/nixosandbox/src/cli.rs`**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nixosandbox", about = "Reproducible, isolated sandbox environments")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new sandbox session
    Create {
        /// Use a built-in profile
        #[arg(long)]
        profile: Option<String>,

        /// Use a custom spec file
        #[arg(long)]
        spec: Option<String>,

        /// Host directory to mount as /workspace
        #[arg(long)]
        workspace: Option<String>,

        /// Human-readable session name
        #[arg(long)]
        name: Option<String>,

        /// Output session info as JSON
        #[arg(long)]
        json: bool,
    },

    /// Execute a command inside a sandbox
    Exec {
        /// Session ID
        session_id: String,

        /// Stream NDJSON events
        #[arg(long)]
        json: bool,

        /// Kill after timeout (seconds)
        #[arg(long)]
        timeout: Option<u64>,

        /// Additional environment variable (KEY=VALUE)
        #[arg(long = "env", value_name = "KEY=VALUE")]
        extra_env: Vec<String>,

        /// Command to execute (after --)
        #[arg(last = true)]
        command: Vec<String>,
    },

    /// Enter a sandbox interactively
    Enter {
        /// Session ID
        session_id: String,
    },

    /// List active sandbox sessions
    List {
        /// Output as JSON array
        #[arg(long)]
        json: bool,
    },

    /// Destroy a sandbox session
    Destroy {
        /// Session ID
        session_id: String,
    },

    /// Build a rootfs without creating a session
    Build {
        /// Use a built-in profile
        #[arg(long)]
        profile: Option<String>,

        /// Use a custom spec file
        #[arg(long)]
        spec: Option<String>,

        /// Output rootfs path as JSON
        #[arg(long)]
        json: bool,
    },

    /// Run in legacy NDJSON subprocess mode (for backward compatibility)
    #[command(hide = true)]
    LegacyNdjson,
}
```

- [ ] **Step 4: Update `main.rs` with CLI dispatch and legacy NDJSON preservation**

Replace the contents of `crates/nixosandbox/src/main.rs`. The file should declare all modules (adding `cli`), parse CLI args, and dispatch to stubs for new commands. The existing NDJSON logic moves into `legacy_ndjson_main()`. Full file content:

```rust
mod bubblewrap;
mod cli;
mod contract;
mod docker;
mod observer;
mod plan_builder;
mod supervisor;
mod timestamps;
mod validator;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create { .. } => {
            eprintln!("nixosandbox: create not yet implemented");
            std::process::exit(1);
        }
        Commands::Exec { .. } => {
            eprintln!("nixosandbox: exec not yet implemented");
            std::process::exit(1);
        }
        Commands::Enter { .. } => {
            eprintln!("nixosandbox: enter not yet implemented");
            std::process::exit(1);
        }
        Commands::List { .. } => {
            eprintln!("nixosandbox: list not yet implemented");
            std::process::exit(1);
        }
        Commands::Destroy { .. } => {
            eprintln!("nixosandbox: destroy not yet implemented");
            std::process::exit(1);
        }
        Commands::Build { .. } => {
            eprintln!("nixosandbox: build not yet implemented");
            std::process::exit(1);
        }
        Commands::LegacyNdjson => {
            legacy_ndjson_main();
        }
    }
}

/// The original NDJSON subprocess entry point (preserved for Pi backward compat).
fn legacy_ndjson_main() {
    use std::io::{self, BufRead};
    use std::sync::mpsc;
    use contract::{
        emit, InboundMessage, ReconciliationHints, ResultEnvelope, ResultPayload,
        ValidationEnvelope, ValidationError, ValidationPayload,
    };

    let stdin = io::stdin();
    let mut first_line = String::new();
    if stdin.lock().read_line(&mut first_line).is_err() {
        eprintln!("nixosandbox: failed to read from stdin");
        std::process::exit(1);
    }
    let first_line = first_line.trim();
    let message: InboundMessage = match serde_json::from_str(first_line) {
        Ok(m) => m,
        Err(e) => {
            emit(&ValidationEnvelope::new(ValidationPayload {
                ok: false,
                errors: vec![ValidationError {
                    code: "PARSE_ERROR".to_string(),
                    message: format!("Failed to parse inbound message: {e}"),
                    field: None,
                }],
                warnings: vec![],
                effective_state: None,
            }));
            std::process::exit(0);
        }
    };
    let plan = match message {
        InboundMessage::Plan { payload } => payload,
        InboundMessage::Cancel { payload } => {
            eprintln!("nixosandbox: received Cancel before Plan: reason={:?}", payload.reason);
            std::process::exit(0);
        }
    };
    let bwrap = bubblewrap::detect();
    let validation = validator::validate(&plan, &bwrap);
    emit(&ValidationEnvelope::new(validation.clone()));
    if !validation.ok {
        std::process::exit(0);
    }
    let effective_state = validation.effective_state.expect("effectiveState must be Some when ok=true");
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(text) = line else { break };
            let text = text.trim().to_string();
            if text.is_empty() { continue; }
            match serde_json::from_str::<InboundMessage>(&text) {
                Ok(InboundMessage::Cancel { .. }) => { let _ = cancel_tx.send(()); break; }
                _ => {}
            }
        }
    });
    let result = supervisor::supervise(&plan, &effective_state, cancel_rx, &bwrap);
    emit(&ResultEnvelope::new(ResultPayload {
        exit_code: result.exit_code,
        signal: result.signal,
        timed_out: result.timed_out,
        duration_ms: result.duration_ms,
        effective_network: result.effective_network,
        observed_connections: result.observed_connections,
        would_have_blocked: result.would_have_blocked,
        resource_peaks: None,
        reconciliation_hints: ReconciliationHints {
            terminal_state: result.terminal_state,
            workspace_modified: result.workspace_modified,
            cleanup_succeeded: true,
        },
    }));
    std::process::exit(0);
}
```

- [ ] **Step 5: Build the crate**

Run: `cd crates/nixosandbox && cargo build`

Expected: Compiles successfully with new `nixosandbox` binary.

- [ ] **Step 6: Verify CLI help works**

Run: `./crates/nixosandbox/target/debug/nixosandbox --help`

Expected: Shows subcommands: create, exec, enter, list, destroy, build.

- [ ] **Step 7: Verify legacy NDJSON mode still works**

Run:
```bash
echo '{"type":"plan","payload":{"version":1,"sessionId":"test","executionId":"test","requestedProfile":"build-install","manifest":{"mounts":[],"env":{},"cwd":"/tmp"},"policy":{"namespaces":[],"network":{"mode":"full"},"allowedWritableTargets":["/tmp"],"strictWritePolicy":false},"command":["echo","hello"]}}' | PI_SANDBOX_NO_DOCKER=1 ./crates/nixosandbox/target/debug/nixosandbox legacy-ndjson 2>/dev/null | head -1 | jq .type
```

Expected: `"validation"`

- [ ] **Step 8: Run existing Rust tests**

Run: `cd crates/nixosandbox && cargo test`

Expected: All 29 existing tests pass.

- [ ] **Step 9: Commit**

```bash
git add -A crates/nixosandbox/
git rm -r --cached crates/pi-sandbox-runtime/ 2>/dev/null || true
git commit -m "feat: rename crate to nixosandbox, add clap CLI skeleton with subcommands"
```

---

### Task 4: Sandbox spec types + validation

**Files:**
- Create: `crates/nixosandbox/src/spec.rs`
- Modify: `crates/nixosandbox/src/main.rs` (add `mod spec;`)

- [ ] **Step 1: Create `crates/nixosandbox/src/spec.rs`**

```rust
use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

/// A sandbox environment specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSpec {
    pub name: String,
    pub packages: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_network")]
    pub network: String,
    #[serde(default = "default_namespaces")]
    pub namespaces: Vec<String>,
    #[serde(default = "default_writable")]
    pub writable: Vec<String>,
}

fn default_network() -> String { "full".to_string() }

fn default_namespaces() -> Vec<String> {
    vec!["pid".to_string(), "mount".to_string(), "uts".to_string(), "ipc".to_string()]
}

fn default_writable() -> Vec<String> {
    vec!["/workspace".to_string(), "/home/sandbox".to_string(), "/cache".to_string(), "/tmp".to_string()]
}

/// Load a spec from a JSON file path.
pub fn load_spec(path: &str) -> Result<SandboxSpec, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read spec file '{}': {}", path, e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse spec file '{}': {}", path, e))
}

/// Load a built-in profile by name.
pub fn load_profile(name: &str, flake_root: &str) -> Result<SandboxSpec, String> {
    let path = format!("{}/nix/profiles/{}.json", flake_root, name);
    if !Path::new(&path).exists() {
        return Err(format!(
            "unknown profile '{}'. Available: build-install, offline-review, strict, debug-network",
            name
        ));
    }
    load_spec(&path)
}

/// Validate a spec for basic correctness.
pub fn validate_spec(spec: &SandboxSpec) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if spec.name.is_empty() {
        errors.push("spec.name must not be empty".to_string());
    }
    if spec.packages.is_empty() {
        errors.push("spec.packages must not be empty".to_string());
    }
    match spec.network.as_str() {
        "off" | "full" => {}
        other => errors.push(format!("spec.network must be 'off' or 'full', got '{}'", other)),
    }
    for ns in &spec.namespaces {
        match ns.as_str() {
            "pid" | "mount" | "uts" | "ipc" | "net" | "user" | "cgroup" => {}
            other => errors.push(format!("unknown namespace '{}' in spec.namespaces", other)),
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_spec() {
        let json = r#"{"name":"test","packages":["bash"]}"#;
        let spec: SandboxSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.name, "test");
        assert_eq!(spec.packages, vec!["bash"]);
        assert_eq!(spec.network, "full");
        assert_eq!(spec.namespaces, vec!["pid", "mount", "uts", "ipc"]);
    }

    #[test]
    fn deserialize_full_spec() {
        let json = r#"{"name":"web","packages":["nodejs_22","git"],"env":{"NODE_ENV":"dev"},"network":"off","namespaces":["pid","net"],"writable":["/tmp"]}"#;
        let spec: SandboxSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.network, "off");
        assert_eq!(spec.env.get("NODE_ENV").unwrap(), "dev");
    }

    #[test]
    fn validate_valid_spec() {
        let spec = SandboxSpec {
            name: "test".to_string(), packages: vec!["bash".to_string()],
            env: HashMap::new(), network: "full".to_string(),
            namespaces: vec!["pid".to_string()], writable: vec!["/tmp".to_string()],
        };
        assert!(validate_spec(&spec).is_ok());
    }

    #[test]
    fn validate_empty_name_fails() {
        let spec = SandboxSpec {
            name: "".to_string(), packages: vec!["bash".to_string()],
            env: HashMap::new(), network: "full".to_string(),
            namespaces: vec![], writable: vec![],
        };
        assert!(validate_spec(&spec).unwrap_err().iter().any(|e| e.contains("name")));
    }

    #[test]
    fn validate_bad_network_fails() {
        let spec = SandboxSpec {
            name: "test".to_string(), packages: vec!["bash".to_string()],
            env: HashMap::new(), network: "allowlist".to_string(),
            namespaces: vec![], writable: vec![],
        };
        assert!(validate_spec(&spec).unwrap_err().iter().any(|e| e.contains("network")));
    }
}
```

- [ ] **Step 2: Add `mod spec;` to main.rs**

Add `mod spec;` in the module declarations (alphabetical order, after `plan_builder`).

- [ ] **Step 3: Run tests**

Run: `cd crates/nixosandbox && cargo test spec`

Expected: All 5 spec tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/nixosandbox/src/spec.rs crates/nixosandbox/src/main.rs
git commit -m "feat: add sandbox spec types with validation and tests"
```

---

### Task 5: Session management

**Files:**
- Create: `crates/nixosandbox/src/session.rs`
- Modify: `crates/nixosandbox/src/main.rs` (add `mod session;`)

- [ ] **Step 1: Create `crates/nixosandbox/src/session.rs`**

```rust
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub session_id: String,
    pub name: String,
    pub profile: String,
    pub rootfs_path: String,
    pub workspace: String,
    pub created_at: String,
    pub last_exec_at: Option<String>,
    pub pid: Option<u32>,
}

pub struct SessionDirs {
    pub root: PathBuf,
    pub workspace: PathBuf,
    pub home: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub metadata_path: PathBuf,
}

pub fn sessions_base_dir() -> PathBuf {
    let data_dir = std::env::var("NIXOSANDBOX_DATA_DIR")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            format!("{}/.local/share/nixosandbox", home)
        });
    PathBuf::from(data_dir).join("sessions")
}

fn generate_session_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

pub fn create_session(
    name: &str, profile: &str, rootfs_path: &str, workspace: Option<&str>,
) -> Result<SessionMetadata, String> {
    let session_id = generate_session_id();
    let base = sessions_base_dir();
    let session_dir = base.join(&session_id);
    fs::create_dir_all(&session_dir).map_err(|e| format!("failed to create session dir: {e}"))?;
    let home_dir = session_dir.join("home");
    let cache_dir = session_dir.join("cache");
    let logs_dir = session_dir.join("logs");
    fs::create_dir_all(&home_dir).map_err(|e| format!("failed to create home dir: {e}"))?;
    fs::create_dir_all(&cache_dir).map_err(|e| format!("failed to create cache dir: {e}"))?;
    fs::create_dir_all(&logs_dir).map_err(|e| format!("failed to create logs dir: {e}"))?;

    let workspace_dir = session_dir.join("workspace");
    let workspace_path = if let Some(ws) = workspace {
        let ws_path = Path::new(ws);
        if !ws_path.exists() {
            return Err(format!("workspace path does not exist: {ws}"));
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(ws_path, &workspace_dir)
            .map_err(|e| format!("failed to symlink workspace: {e}"))?;
        ws.to_string()
    } else {
        fs::create_dir_all(&workspace_dir).map_err(|e| format!("failed to create workspace: {e}"))?;
        workspace_dir.to_string_lossy().to_string()
    };

    let metadata = SessionMetadata {
        session_id: session_id.clone(),
        name: name.to_string(),
        profile: profile.to_string(),
        rootfs_path: rootfs_path.to_string(),
        workspace: workspace_path,
        created_at: crate::timestamps::now_iso8601(),
        last_exec_at: None,
        pid: None,
    };
    let metadata_path = session_dir.join("metadata.json");
    let json = serde_json::to_string_pretty(&metadata).map_err(|e| format!("serialize: {e}"))?;
    fs::write(&metadata_path, json).map_err(|e| format!("write metadata: {e}"))?;
    Ok(metadata)
}

pub fn list_sessions() -> Result<Vec<SessionMetadata>, String> {
    let base = sessions_base_dir();
    if !base.exists() { return Ok(vec![]); }
    let mut sessions = Vec::new();
    let entries = fs::read_dir(&base).map_err(|e| format!("read sessions dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read dir entry: {e}"))?;
        let metadata_path = entry.path().join("metadata.json");
        if metadata_path.exists() {
            let content = fs::read_to_string(&metadata_path).map_err(|e| format!("read metadata: {e}"))?;
            if let Ok(meta) = serde_json::from_str::<SessionMetadata>(&content) {
                sessions.push(meta);
            }
        }
    }
    sessions.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(sessions)
}

pub fn load_session(session_id: &str) -> Result<SessionMetadata, String> {
    let path = sessions_base_dir().join(session_id).join("metadata.json");
    if !path.exists() { return Err(format!("session '{}' not found", session_id)); }
    let content = fs::read_to_string(&path).map_err(|e| format!("read metadata: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("parse metadata: {e}"))
}

pub fn session_dirs(session_id: &str) -> SessionDirs {
    let root = sessions_base_dir().join(session_id);
    SessionDirs {
        workspace: root.join("workspace"), home: root.join("home"),
        cache: root.join("cache"), logs: root.join("logs"),
        metadata_path: root.join("metadata.json"), root,
    }
}

pub fn touch_last_exec(session_id: &str) -> Result<(), String> {
    let mut meta = load_session(session_id)?;
    meta.last_exec_at = Some(crate::timestamps::now_iso8601());
    let dirs = session_dirs(session_id);
    let json = serde_json::to_string_pretty(&meta).map_err(|e| format!("serialize: {e}"))?;
    fs::write(&dirs.metadata_path, json).map_err(|e| format!("write metadata: {e}"))
}

pub fn destroy_session(session_id: &str) -> Result<(), String> {
    let dirs = session_dirs(session_id);
    if !dirs.root.exists() { return Err(format!("session '{}' not found", session_id)); }
    fs::remove_dir_all(&dirs.root).map_err(|e| format!("remove session dir: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_data_dir<F: FnOnce()>(f: F) {
        let dir = std::env::temp_dir().join(format!("nixosandbox-test-{}", uuid::Uuid::new_v4()));
        std::env::set_var("NIXOSANDBOX_DATA_DIR", &dir);
        f();
        let _ = fs::remove_dir_all(&dir);
        std::env::remove_var("NIXOSANDBOX_DATA_DIR");
    }

    #[test]
    fn create_and_list_sessions() {
        with_temp_data_dir(|| {
            let meta = create_session("test-session", "strict", "/nix/store/fake", None).unwrap();
            assert_eq!(meta.name, "test-session");
            let sessions = list_sessions().unwrap();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].session_id, meta.session_id);
        });
    }

    #[test]
    fn load_session_by_id() {
        with_temp_data_dir(|| {
            let meta = create_session("load-test", "strict", "/nix/store/fake", None).unwrap();
            let loaded = load_session(&meta.session_id).unwrap();
            assert_eq!(loaded.name, "load-test");
        });
    }

    #[test]
    fn destroy_session_removes_dir() {
        with_temp_data_dir(|| {
            let meta = create_session("rm-test", "strict", "/nix/store/fake", None).unwrap();
            let dirs = session_dirs(&meta.session_id);
            assert!(dirs.root.exists());
            destroy_session(&meta.session_id).unwrap();
            assert!(!dirs.root.exists());
        });
    }

    #[test]
    fn destroy_nonexistent_errors() {
        with_temp_data_dir(|| {
            assert!(destroy_session("nonexistent").is_err());
        });
    }

    #[test]
    fn create_with_external_workspace() {
        with_temp_data_dir(|| {
            let ws = std::env::temp_dir().join(format!("ws-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&ws).unwrap();
            let meta = create_session("ws-test", "strict", "/nix/store/fake", Some(ws.to_str().unwrap())).unwrap();
            let dirs = session_dirs(&meta.session_id);
            assert!(dirs.workspace.is_symlink());
            destroy_session(&meta.session_id).unwrap();
            assert!(ws.exists()); // external workspace preserved
            let _ = fs::remove_dir_all(&ws);
        });
    }

    #[test]
    fn metadata_roundtrip() {
        let meta = SessionMetadata {
            session_id: "abc".to_string(), name: "test".to_string(),
            profile: "strict".to_string(), rootfs_path: "/nix/store/fake".to_string(),
            workspace: "/tmp/ws".to_string(), created_at: "2026-04-08T12:00:00Z".to_string(),
            last_exec_at: None, pid: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let de: SessionMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(de.session_id, "abc");
    }
}
```

- [ ] **Step 2: Add `mod session;` to main.rs** (alphabetical, after `plan_builder`)

- [ ] **Step 3: Run tests**

Run: `cd crates/nixosandbox && cargo test session`

Expected: All 6 session tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/nixosandbox/src/session.rs crates/nixosandbox/src/main.rs
git commit -m "feat: add session management (create/list/load/destroy) with tests"
```

---

### Task 6: Nix build invocation from Rust

**Files:**
- Create: `crates/nixosandbox/src/nix.rs`
- Modify: `crates/nixosandbox/src/main.rs` (add `mod nix;`)

- [ ] **Step 1: Create `crates/nixosandbox/src/nix.rs`**

```rust
use std::process::{Command, Stdio};
use std::path::Path;

use crate::spec::SandboxSpec;

/// Find the flake root by looking for flake.nix.
pub fn find_flake_root() -> Result<String, String> {
    if let Ok(root) = std::env::var("NIXOSANDBOX_FLAKE_ROOT") {
        if Path::new(&root).join("flake.nix").exists() {
            return Ok(root);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        while let Some(d) = dir {
            if d.join("flake.nix").exists() {
                return Ok(d.to_string_lossy().to_string());
            }
            dir = d.parent().map(|p| p.to_path_buf());
        }
    }
    if Path::new("flake.nix").exists() {
        return Ok(std::env::current_dir().map_err(|e| format!("cwd: {e}"))?.to_string_lossy().to_string());
    }
    Err("Could not find flake.nix. Set NIXOSANDBOX_FLAKE_ROOT or run from repo root.".to_string())
}

/// Build a rootfs for a built-in profile. Returns the Nix store path.
pub fn build_profile(profile_name: &str) -> Result<String, String> {
    let flake_root = find_flake_root()?;
    nix_build(&format!("{}#sandbox-{}", flake_root, profile_name))
}

/// Build a rootfs from a custom spec. Returns the Nix store path.
pub fn build_spec(spec: &SandboxSpec) -> Result<String, String> {
    let flake_root = find_flake_root()?;
    let packages_nix = spec.packages.iter().map(|p| format!("pkgs.{}", p)).collect::<Vec<_>>().join(" ");
    let env_nix = spec.env.iter().map(|(k, v)| format!("\"{}\" = \"{}\";", k, v)).collect::<Vec<_>>().join(" ");
    let expr = format!(
        r#"let pkgs = import (builtins.getFlake "{}").inputs.nixpkgs {{}}; mkSandboxRootfs = import {}/nix/mkSandboxRootfs.nix {{ inherit pkgs; }}; in mkSandboxRootfs {{ name = "{}"; packages = [ {} ]; env = {{ {} }}; }}"#,
        flake_root, flake_root, spec.name, packages_nix, env_nix
    );
    nix_build_expr(&expr)
}

fn nix_build(flake_attr: &str) -> Result<String, String> {
    let output = Command::new("nix")
        .args(["build", flake_attr, "--no-link", "--print-out-paths"])
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .output().map_err(|e| format!("nix build: {e}"))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { Err("nix build produced no output".into()) } else { Ok(path) }
    } else {
        Err(format!("nix build failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

fn nix_build_expr(expr: &str) -> Result<String, String> {
    let output = Command::new("nix")
        .args(["build", "--impure", "--expr", expr, "--no-link", "--print-out-paths"])
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .output().map_err(|e| format!("nix build --expr: {e}"))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { Err("nix build --expr produced no output".into()) } else { Ok(path) }
    } else {
        Err(format!("nix build --expr failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

/// Check if a rootfs path looks valid.
pub fn validate_rootfs(rootfs_path: &str) -> Result<(), String> {
    let root = Path::new(rootfs_path);
    if !root.exists() { return Err(format!("rootfs not found: {rootfs_path}")); }
    if !root.join("bin").exists() { return Err(format!("rootfs missing /bin: {rootfs_path}")); }
    if !root.join("etc").exists() { return Err(format!("rootfs missing /etc: {rootfs_path}")); }
    Ok(())
}
```

- [ ] **Step 2: Add `mod nix;` to main.rs** (alphabetical, after `docker`)

- [ ] **Step 3: Build to verify compilation**

Run: `cd crates/nixosandbox && cargo build`

Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/nixosandbox/src/nix.rs crates/nixosandbox/src/main.rs
git commit -m "feat: add Nix build invocation (build_profile, build_spec, validate_rootfs)"
```

---

### Task 7: build_rootfs() in plan_builder -- pivot-root bwrap argv

**Files:**
- Modify: `crates/nixosandbox/src/plan_builder.rs`

- [ ] **Step 1: Add `SessionDirs` struct and `build_rootfs` function with tests**

Add before the `#[cfg(test)]` module in `crates/nixosandbox/src/plan_builder.rs`:

```rust
/// Session directory paths for rootfs-mode execution.
pub struct RootfsSessionDirs {
    pub workspace: String,
    pub home: String,
    pub cache: String,
}

/// Build bwrap argument vector for pivot-root execution into a Nix rootfs.
pub fn build_rootfs(
    rootfs_path: &str,
    session_dirs: &RootfsSessionDirs,
    command: &[String],
    env: &std::collections::HashMap<String, String>,
    network: &str,
    namespaces: &[String],
) -> Vec<String> {
    let mut argv: Vec<String> = Vec::new();
    argv.extend(["--pivot-root".to_string(), rootfs_path.to_string(), "/oldroot".to_string()]);
    argv.extend(["--tmpfs".to_string(), "/oldroot".to_string()]);
    argv.extend(["--bind".to_string(), session_dirs.workspace.clone(), "/workspace".to_string()]);
    argv.extend(["--bind".to_string(), session_dirs.home.clone(), "/home/sandbox".to_string()]);
    argv.extend(["--bind".to_string(), session_dirs.cache.clone(), "/cache".to_string()]);
    argv.extend(["--tmpfs".to_string(), "/tmp".to_string()]);
    argv.extend(["--dev".to_string(), "/dev".to_string()]);
    argv.extend(["--proc".to_string(), "/proc".to_string()]);
    for ns in namespaces {
        match ns.as_str() {
            "pid" => argv.push("--unshare-pid".to_string()),
            "mount" => {} // implicit with pivot-root
            "uts" => argv.push("--unshare-uts".to_string()),
            "ipc" => argv.push("--unshare-ipc".to_string()),
            "net" => argv.push("--unshare-net".to_string()),
            "user" => argv.push("--unshare-user".to_string()),
            "cgroup" => argv.push("--unshare-cgroup-try".to_string()),
            _ => {}
        }
    }
    argv.push("--clearenv".to_string());
    argv.extend(["--setenv".to_string(), "HOME".to_string(), "/home/sandbox".to_string()]);
    argv.extend(["--setenv".to_string(), "PATH".to_string(), "/bin:/usr/bin".to_string()]);
    argv.extend(["--setenv".to_string(), "TERM".to_string(), "xterm-256color".to_string()]);
    for (key, value) in env {
        argv.extend(["--setenv".to_string(), key.clone(), value.clone()]);
    }
    argv.extend(["--chdir".to_string(), "/workspace".to_string()]);
    argv.push("--".to_string());
    argv.extend(command.iter().cloned());
    argv
}
```

Add to the `#[cfg(test)]` module:

```rust
    #[test]
    fn build_rootfs_produces_pivot_root_argv() {
        let dirs = RootfsSessionDirs {
            workspace: "/tmp/ws".to_string(),
            home: "/tmp/home".to_string(),
            cache: "/tmp/cache".to_string(),
        };
        let cmd = vec!["echo".to_string(), "hello".to_string()];
        let env = std::collections::HashMap::new();
        let argv = build_rootfs("/nix/store/fake", &dirs, &cmd, &env, "full", &["pid".to_string()]);
        assert!(argv.contains(&"--pivot-root".to_string()));
        assert!(argv.contains(&"/nix/store/fake".to_string()));
        assert!(argv.contains(&"--bind".to_string()));
        assert!(argv.contains(&"--tmpfs".to_string()));
        assert!(argv.contains(&"--dev".to_string()));
        assert!(argv.contains(&"--proc".to_string()));
        assert!(argv.contains(&"--clearenv".to_string()));
        let sep = argv.iter().position(|a| a == "--").unwrap();
        assert_eq!(argv[sep + 1], "echo");
        assert_eq!(argv[sep + 2], "hello");
    }

    #[test]
    fn build_rootfs_network_off_adds_unshare_net() {
        let dirs = RootfsSessionDirs {
            workspace: "/tmp/ws".to_string(), home: "/tmp/home".to_string(), cache: "/tmp/cache".to_string(),
        };
        let cmd = vec!["echo".to_string()];
        let env = std::collections::HashMap::new();
        let argv = build_rootfs("/nix/store/fake", &dirs, &cmd, &env, "off", &["pid".to_string(), "net".to_string()]);
        assert!(argv.contains(&"--unshare-net".to_string()));
    }
```

- [ ] **Step 2: Run tests**

Run: `cd crates/nixosandbox && cargo test build_rootfs`

Expected: Both tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/nixosandbox/src/plan_builder.rs
git commit -m "feat: add build_rootfs() for pivot-root bwrap execution"
```

---

### Task 8: Wire CLI subcommands to implementations

**Files:**
- Modify: `crates/nixosandbox/src/main.rs`

This task replaces stub implementations with real logic for all subcommands. This is the largest single task. After this, the CLI is fully functional.

- [ ] **Step 1: Replace main.rs CLI dispatch with full implementations**

Replace the `main()` function and add helper functions. Keep the module declarations and `legacy_ndjson_main()` unchanged. The full new `main()` and helpers are in the plan's Task 8 code block above (from the first Write attempt). Due to the length, refer to the spec for exact behavior of each command:

- `cmd_create`: loads spec/profile, validates, calls `nix::build_profile` or `nix::build_spec`, calls `session::create_session`, prints session ID
- `cmd_exec`: loads session, loads profile for network/namespace config, builds rootfs argv, detects bwrap, spawns with stdio inherit (default) or NDJSON (--json)
- `cmd_list`: calls `session::list_sessions`, prints table or JSON
- `cmd_destroy`: calls `session::destroy_session`
- `cmd_build`: calls `nix::build_profile` or `nix::build_spec`, prints rootfs path
- `Enter` dispatches to `cmd_exec` with `/bin/bash`

Key: The `cmd_exec` function must handle two modes:
1. **Default mode**: `cmd.stdin/stdout/stderr(Stdio::inherit())` for interactive use
2. **JSON mode**: pipe stdout/stderr, stream NDJSON events using `contract::emit`

- [ ] **Step 2: Build and verify compilation**

Run: `cd crates/nixosandbox && cargo build`

Expected: Compiles successfully.

- [ ] **Step 3: Run all Rust tests**

Run: `cd crates/nixosandbox && cargo test`

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/nixosandbox/src/main.rs
git commit -m "feat: wire all CLI subcommands (create, exec, enter, list, destroy, build)"
```

---

### Task 9: Update protocol tests for renamed crate

**Files:**
- Delete: `docker-compose.yml`
- Modify: `tests/protocol/globalSetup.ts`
- Modify: `tests/protocol/helpers.ts`

- [ ] **Step 1: Delete `docker-compose.yml`**

```bash
rm docker-compose.yml
```

- [ ] **Step 2: Update `tests/protocol/globalSetup.ts`**

Change the `CRATE_DIR` path:

```typescript
const CRATE_DIR = resolve(import.meta.dirname, "../../crates/nixosandbox");
```

- [ ] **Step 3: Update `tests/protocol/helpers.ts`**

Update `spawnRuntime` to pass `legacy-ndjson` subcommand:

```typescript
  const child = spawn(binaryPath, ["legacy-ndjson"], {
    stdio: ["pipe", "pipe", "pipe"],
    env: options?.env ?? process.env,
  });
```

- [ ] **Step 4: Run protocol tests**

Run: `cd tests/protocol && npx vitest run`

Expected: All existing protocol tests pass via `legacy-ndjson` subcommand.

- [ ] **Step 5: Commit**

```bash
git rm docker-compose.yml
git add tests/protocol/globalSetup.ts tests/protocol/helpers.ts
git commit -m "chore: delete legacy docker-compose, update tests for renamed crate"
```

---

### Task 10: Wire nixosandbox binary into flake.nix

**Files:**
- Modify: `flake.nix`

- [ ] **Step 1: Update `flake.nix` to build the Rust binary**

Replace the `# nixosandbox = ...;` placeholder with:

```nix
      nixosandbox = pkgs.rustPlatform.buildRustPackage {
        pname = "nixosandbox";
        version = "0.1.0";
        src = ./crates/nixosandbox;
        cargoLock.lockFile = ./crates/nixosandbox/Cargo.lock;
      };

      default = self.packages.${system}.nixosandbox;
```

- [ ] **Step 2: Build the binary via flake**

Run: `nix build .#nixosandbox --no-link --print-out-paths`

Expected: A Nix store path with the binary at `<path>/bin/nixosandbox`.

- [ ] **Step 3: Verify via nix run**

Run: `nix run . -- --help`

Expected: Shows nixosandbox CLI help.

- [ ] **Step 4: Commit**

```bash
git add flake.nix
git commit -m "feat: wire nixosandbox binary into flake.nix as default package"
```

---

## Phase Gate Checklist

After all tasks are complete, verify:

- [ ] `nix build .#sandbox-strict` produces a minimal rootfs with bash, coreutils
- [ ] `nix build .#sandbox-build-install` produces a rootfs with node, python, git, rust
- [ ] `nixosandbox create --profile strict` creates a session with ID
- [ ] `nixosandbox exec <id> -- echo hello` prints "hello", exit 0
- [ ] `nixosandbox exec <id> -- ls /` shows sandbox rootfs, not host
- [ ] `nixosandbox exec --json <id> -- echo test` produces NDJSON event stream
- [ ] `nixosandbox list` shows the session
- [ ] `nixosandbox destroy <id>` cleans up
- [ ] `nixosandbox build --profile strict` outputs a Nix store path
- [ ] `nix run . -- --help` shows CLI help
- [ ] All Rust unit tests pass
- [ ] All protocol tests pass via `legacy-ndjson` subcommand

---

## What Is Next (Part B -- separate plan)

- Docker sidecar updated with `/nix/store` mount for macOS
- Pi extension simplified to thin CLI adapter
- macOS integration tests
- End-to-end integration tests on Linux with Nix + bwrap
