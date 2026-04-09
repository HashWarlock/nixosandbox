# llm-agents.nix Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate `numtide/llm-agents.nix` as a flake input to provide a unified package catalog (80+ AI agents + nixpkgs tools) and enable agent-driven sandbox composition via `--with` CLI flag.

**Architecture:** Add `llm-agents.nix` as a flake input with its own binary cache. A new `nix/catalog.nix` merges agent packages and nixpkgs tools into a queryable catalog. A new `nix/mkAgentSandbox.nix` resolves package names from the catalog and delegates to the existing `mkSandboxRootfs`. The Rust CLI gains a `--with` flag on `create` and a new `catalog` subcommand. The Pi extension adds a `sandbox_catalog` tool and updates `sandbox_run` with a `with` parameter.

**Tech Stack:** Nix flakes, Rust (clap, serde_json), TypeScript (Node.js child_process)

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `nix/catalog.nix` | Build unified `{ agents, tools }` attrset from llm-agents.nix + nixpkgs |
| `nix/mkAgentSandbox.nix` | Resolve package names from catalog, delegate to mkSandboxRootfs |

### Modified files

| File | Change |
|------|--------|
| `flake.nix` | Add `llm-agents` input, `nixConfig`, expose `catalog` and `lib.mkAgentSandbox` |
| `crates/nixosandbox/src/cli.rs` | Add `--with` on Create, add `Catalog` variant to Commands |
| `crates/nixosandbox/src/main.rs` | Wire `--with` into `cmd_create`, add `cmd_catalog`, update `resolve_spec` |
| `crates/nixosandbox/src/nix.rs` | Add `build_with_catalog()` and `query_catalog()` |
| `packages/pi-sandbox-extension/src/cli-client.ts` | Add `catalogPackages()`, update `CreateOptions` with `withPackages` |
| `packages/pi-sandbox-extension/src/extension.ts` | Add `sandbox_catalog` tool, update `sandbox_run` with `with` param |
| `packages/pi-sandbox-extension/src/index.ts` | Re-export `catalogPackages` and `CatalogResponse` |

### Unchanged files

- `nix/mkSandboxRootfs.nix` -- untouched foundation
- `nix/profiles/*.json` -- backward compatible
- `crates/nixosandbox/src/session.rs`, `plan_builder.rs`, `bubblewrap.rs`, `docker.rs`
- `packages/pi-sandbox-extension/src/contract.ts`, `crash-synthesis.ts`, `browser.ts`

---

### Task 1: Add llm-agents.nix flake input and nixConfig

**Files:**
- Modify: `flake.nix`

- [ ] **Step 1: Add llm-agents input and nixConfig to flake.nix**

Open `flake.nix` and make these changes:

1. Add `nixConfig` block before `inputs`:

```nix
{
  description = "nixosandbox -- reproducible, isolated sandbox environments";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    llm-agents.url = "github:numtide/llm-agents.nix";
  };
```

2. Update the `outputs` function signature to include `llm-agents`:

```nix
  outputs = { self, nixpkgs, llm-agents }:
```

No other changes to flake.nix yet -- the catalog and mkAgentSandbox outputs come in later tasks.

- [ ] **Step 2: Lock the new input**

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix flake lock --accept-flake-config
```

Expected: `flake.lock` updates with a new entry for `llm-agents` and its transitive inputs (`blueprint`, `bun2nix`, `treefmt-nix`, `flake-parts`, `systems`). No errors.

- [ ] **Step 3: Verify the flake evaluates**

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix flake show --accept-flake-config 2>&1 | head -30
```

Expected: Shows `packages`, `devShells`, `lib` outputs without errors. The llm-agents input should be locked but not yet used in outputs.

- [ ] **Step 4: Commit**

```bash
git add flake.nix flake.lock
git commit -m "feat: add llm-agents.nix flake input with numtide binary cache"
```

---

### Task 2: Create nix/catalog.nix

**Files:**
- Create: `nix/catalog.nix`

- [ ] **Step 1: Create the catalog module**

Create `nix/catalog.nix` with the full agent and tool listings:

```nix
# nix/catalog.nix
#
# Unified package catalog merging AI agents from llm-agents.nix
# and standard development tools from nixpkgs.
#
# Usage: import ./catalog.nix { pkgs = ...; llm-agents-pkgs = ...; }
{ pkgs, llm-agents-pkgs }:

let
  # Helper: only inherit a package if it exists in the source attrset.
  # llm-agents.nix package availability varies by platform.
  pickExisting = src: names:
    builtins.listToAttrs (
      builtins.filter (x: x != null) (
        map (name:
          if src ? ${name}
          then { inherit name; value = src.${name}; }
          else null
        ) names
      )
    );
in
{
  agents = pickExisting llm-agents-pkgs [
    # AI Coding Agents
    "amp"
    "claude-code"
    "codex"
    "copilot-cli"
    "crush"
    "cursor-agent"
    "droid"
    "forge"
    "gemini-cli"
    "goose-cli"
    "iflow-cli"
    "kilocode-cli"
    "mistral-vibe"
    "nanocoder"
    "opencode"
    "pi"
    "qoder-cli"
    "qwen-code"
    # Claude Code Ecosystem
    "claudebox"
    "catnip"
    "claude-code-router"
    # ACP Ecosystem
    "claude-code-acp"
    "codex-acp"
    # Utilities
    "sidecar"
    "sandbox-runtime"
  ];

  tools = {
    # Languages & runtimes
    inherit (pkgs) python312 nodejs_22 rustc cargo go;
    # Version control
    inherit (pkgs) git;
    # Core utilities
    inherit (pkgs) coreutils bash findutils gnugrep gnused gawk;
    # Build tools
    inherit (pkgs) gnumake gcc gnutar gzip;
    # Network
    inherit (pkgs) curl cacert;
    # Search & text
    inherit (pkgs) ripgrep fd jq less;
    # Shells
    inherit (pkgs) zsh;
    # Nix itself
    inherit (pkgs) nix;
  };
}
```

- [ ] **Step 2: Verify the catalog evaluates**

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix eval --accept-flake-config --impure --expr '
  let
    flake = builtins.getFlake (toString ./.);
    pkgs = flake.inputs.nixpkgs.legacyPackages.x86_64-linux;
    agents-pkgs = flake.inputs.llm-agents.packages.x86_64-linux;
    catalog = import ./nix/catalog.nix { inherit pkgs; llm-agents-pkgs = agents-pkgs; };
  in builtins.attrNames catalog.agents
' 2>&1 | head -5
```

Expected: A list of agent names like `[ "amp" "catnip" "claude-code" ... ]`. Some may be missing if not available for x86_64-linux -- that's fine, `pickExisting` handles it.

- [ ] **Step 3: Commit**

```bash
git add nix/catalog.nix
git commit -m "feat: create unified package catalog from llm-agents.nix + nixpkgs"
```

---

### Task 3: Create nix/mkAgentSandbox.nix

**Files:**
- Create: `nix/mkAgentSandbox.nix`

- [ ] **Step 1: Create the mkAgentSandbox function**

Create `nix/mkAgentSandbox.nix`:

```nix
# nix/mkAgentSandbox.nix
#
# Catalog-aware sandbox composition layer.
# Resolves package names from the unified catalog and delegates to mkSandboxRootfs.
#
# Usage:
#   mkAgentSandbox = import ./mkAgentSandbox.nix { inherit catalog mkSandboxRootfs; };
#   mkAgentSandbox { name = "review"; packages = [ "claude-code" "git" "ripgrep" ]; }
{ catalog, mkSandboxRootfs }:

{ name
, packages ? []
, extraPackages ? []
, env ? {}
}:

let
  resolvePackage = pname:
    if catalog.agents ? ${pname} then catalog.agents.${pname}
    else if catalog.tools ? ${pname} then catalog.tools.${pname}
    else throw "nixosandbox: unknown package '${pname}' -- not found in agents or tools catalog. Run 'nixosandbox catalog' to see available packages.";

  resolvedPackages = map resolvePackage packages;
  allPackages = resolvedPackages ++ extraPackages;
in
  mkSandboxRootfs {
    inherit name env;
    packages = allPackages;
  }
```

- [ ] **Step 2: Verify mkAgentSandbox resolves a known tool**

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix eval --accept-flake-config --impure --expr '
  let
    flake = builtins.getFlake (toString ./.);
    pkgs = flake.inputs.nixpkgs.legacyPackages.x86_64-linux;
    agents-pkgs = flake.inputs.llm-agents.packages.x86_64-linux;
    catalog = import ./nix/catalog.nix { inherit pkgs; llm-agents-pkgs = agents-pkgs; };
    mkSandboxRootfs = import ./nix/mkSandboxRootfs.nix { inherit pkgs; };
    mkAgentSandbox = import ./nix/mkAgentSandbox.nix { inherit catalog mkSandboxRootfs; };
    result = mkAgentSandbox { name = "test"; packages = [ "bash" "git" ]; };
  in result.name
'
```

Expected: `"sandbox-test"` (the name prefix from mkSandboxRootfs).

- [ ] **Step 3: Verify unknown package throws**

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix eval --accept-flake-config --impure --expr '
  let
    flake = builtins.getFlake (toString ./.);
    pkgs = flake.inputs.nixpkgs.legacyPackages.x86_64-linux;
    agents-pkgs = flake.inputs.llm-agents.packages.x86_64-linux;
    catalog = import ./nix/catalog.nix { inherit pkgs; llm-agents-pkgs = agents-pkgs; };
    mkSandboxRootfs = import ./nix/mkSandboxRootfs.nix { inherit pkgs; };
    mkAgentSandbox = import ./nix/mkAgentSandbox.nix { inherit catalog mkSandboxRootfs; };
    result = mkAgentSandbox { name = "fail"; packages = [ "nonexistent-pkg" ]; };
  in result.name
' 2>&1
```

Expected: Error containing `"unknown package 'nonexistent-pkg'"`.

- [ ] **Step 4: Commit**

```bash
git add nix/mkAgentSandbox.nix
git commit -m "feat: create mkAgentSandbox -- catalog-aware composition layer"
```

---

### Task 4: Wire catalog and mkAgentSandbox into flake.nix outputs

**Files:**
- Modify: `flake.nix`

- [ ] **Step 1: Add catalog, mkAgentSandbox, and lib exports to flake.nix**

In `flake.nix`, update the `let` block to add catalog and mkAgentSandbox, then add them to outputs. The full updated file:

```nix
{
  description = "nixosandbox -- reproducible, isolated sandbox environments";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    llm-agents.url = "github:numtide/llm-agents.nix";
  };

  outputs = { self, nixpkgs, llm-agents }:
  let
    # Sandbox rootfs is always Linux
    linuxSystem = "x86_64-linux";
    linuxPkgs = nixpkgs.legacyPackages.${linuxSystem};
    mkSandboxRootfs = import ./nix/mkSandboxRootfs.nix { pkgs = linuxPkgs; };

    # Unified catalog: agents from llm-agents.nix + tools from nixpkgs
    linuxCatalog = import ./nix/catalog.nix {
      pkgs = linuxPkgs;
      llm-agents-pkgs = llm-agents.packages.${linuxSystem} or {};
    };

    # Catalog-aware sandbox builder
    mkAgentSandbox = import ./nix/mkAgentSandbox.nix {
      catalog = linuxCatalog;
      inherit mkSandboxRootfs;
    };

    # Helper: load a profile JSON and resolve package names to nixpkgs attrs
    loadProfile = path:
      let
        spec = builtins.fromJSON (builtins.readFile path);
        resolvedPkgs = map (name:
          if builtins.hasAttr name linuxPkgs
          then builtins.getAttr name linuxPkgs
          else throw "nixosandbox: unknown package '${name}' in profile ${spec.name}"
        ) spec.packages;
      in
        mkSandboxRootfs {
          name = spec.name;
          packages = resolvedPkgs;
          env = spec.env or {};
        };

    # Rootfs derivations (always x86_64-linux, buildable from any host)
    sandboxPackages = {
      sandbox-build-install = loadProfile ./nix/profiles/build-install.json;
      sandbox-offline-review = loadProfile ./nix/profiles/offline-review.json;
      sandbox-strict = loadProfile ./nix/profiles/strict.json;
      sandbox-debug-network = loadProfile ./nix/profiles/debug-network.json;
    };

    # All systems that can build/use nixosandbox
    supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];

    forAllSystems = f: nixpkgs.lib.genAttrs supportedSystems f;
  in
  {
    # Library functions for custom rootfs and catalog composition
    lib = {
      inherit mkSandboxRootfs;
      inherit mkAgentSandbox;
    };

    # Catalog: queryable package listing (always x86_64-linux for rootfs)
    catalog = linuxCatalog;

    packages = forAllSystems (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
        sandboxPackages // {
          nixosandbox = pkgs.rustPlatform.buildRustPackage {
            pname = "nixosandbox";
            version = "0.1.0";
            src = ./crates/nixosandbox;
            cargoLock.lockFile = ./crates/nixosandbox/Cargo.lock;
          };

          default = self.packages.${system}.nixosandbox;
        }
    );

    devShells = forAllSystems (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        default = pkgs.mkShell {
          name = "nixosandbox-dev";
          buildInputs = with pkgs; [
            rustc
            cargo
            pkg-config
            jq
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            bubblewrap
          ];
        };
      }
    );
  };
}
```

- [ ] **Step 2: Verify flake outputs include catalog and lib**

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix flake show --accept-flake-config 2>&1 | head -20
```

Expected: Output includes `catalog` and `lib` entries alongside existing `packages` and `devShells`.

- [ ] **Step 3: Verify catalog is queryable via nix eval**

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix eval --accept-flake-config .#catalog.agents --apply 'x: builtins.attrNames x' 2>&1 | head -3
```

Expected: A list of agent names.

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix eval --accept-flake-config .#catalog.tools --apply 'x: builtins.attrNames x' 2>&1 | head -3
```

Expected: A list of tool names like `[ "bash" "cacert" "cargo" ... ]`.

- [ ] **Step 4: Verify existing profiles still build**

Run:
```bash
NIX_SSL_CERT_FILE=/etc/ssl/cert.pem nix eval --accept-flake-config .#packages.x86_64-linux.sandbox-strict.name
```

Expected: `"sandbox-strict"` -- confirms existing profile path is unbroken.

- [ ] **Step 5: Commit**

```bash
git add flake.nix
git commit -m "feat: wire catalog and mkAgentSandbox into flake outputs"
```

---

### Task 5: Add `--with` flag and `Catalog` subcommand to CLI

**Files:**
- Modify: `crates/nixosandbox/src/cli.rs`

- [ ] **Step 1: Add `--with` to Create variant and new Catalog variant**

Replace the full contents of `crates/nixosandbox/src/cli.rs`:

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

        /// Compose from catalog packages (comma-separated, e.g. claude-code,git,python312)
        #[arg(long, value_delimiter = ',')]
        with: Option<Vec<String>>,

        /// Network mode for --with sandboxes
        #[arg(long, default_value = "off")]
        network: String,

        /// Host directory to mount as /workspace
        #[arg(long)]
        workspace: Option<String>,

        /// Human-readable session name
        #[arg(long)]
        name: Option<String>,

        /// Agent runtime identifier (e.g. 'claude:opus-4-6')
        #[arg(long)]
        agent: Option<String>,

        /// Purpose of this sandbox session
        #[arg(long)]
        description: Option<String>,

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

    /// Show detailed session status (battlecard)
    Status {
        /// Session ID
        session_id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
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

    /// List available packages from the catalog
    Catalog {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Filter by name substring
        #[arg(long)]
        filter: Option<String>,
    },
}
```

- [ ] **Step 2: Verify it compiles**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo check 2>&1
```

Expected: Compilation errors in `main.rs` because the new `Create` fields (`with`, `network`) and `Catalog` variant are not yet handled. This is expected -- we wire them in the next tasks.

- [ ] **Step 3: Commit**

```bash
git add crates/nixosandbox/src/cli.rs
git commit -m "feat: add --with flag on create and catalog subcommand to CLI"
```

---

### Task 6: Add `build_with_catalog()` and `query_catalog()` to nix.rs

**Files:**
- Modify: `crates/nixosandbox/src/nix.rs`

- [ ] **Step 1: Add the two new functions to nix.rs**

Append these functions to the end of `crates/nixosandbox/src/nix.rs` (after the existing `validate_rootfs` function):

```rust
/// Build a rootfs from catalog package names using mkAgentSandbox.
/// Returns the Nix store path of the resulting rootfs.
pub fn build_with_catalog(names: &[String], network: &str) -> Result<String, String> {
    let flake_root = find_flake_root()?;
    let packages_nix = names
        .iter()
        .map(|n| format!("\"{}\"", n))
        .collect::<Vec<_>>()
        .join(" ");

    // Generate a deterministic name from the sorted package list
    let mut sorted_names = names.to_vec();
    sorted_names.sort();
    let hash_input = sorted_names.join(",");
    let mut h: u64 = 0;
    for b in hash_input.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u64);
    }
    let name_hash = format!("{:08x}", h);

    let expr = format!(
        r#"let flake = builtins.getFlake "{}"; in flake.lib.mkAgentSandbox {{ name = "custom-{}"; packages = [ {} ]; }}"#,
        flake_root, name_hash, packages_nix
    );
    nix_build_expr(&expr)
}

/// Query the flake catalog and return JSON with agent/tool names and descriptions.
pub fn query_catalog() -> Result<String, String> {
    let flake_root = find_flake_root()?;

    // Evaluate a Nix expression that extracts names and meta.description
    // from both catalog.agents and catalog.tools.
    let expr = format!(
        r#"let flake = builtins.getFlake "{}"; catalog = flake.catalog; extractMeta = attrs: builtins.mapAttrs (name: pkg: {{ description = pkg.meta.description or ""; }}) attrs; in {{ agents = extractMeta catalog.agents; tools = extractMeta catalog.tools; }}"#,
        flake_root
    );

    let output = std::process::Command::new("nix")
        .args(["eval", "--impure", "--expr", &expr, "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("nix eval: {e}"))?;

    if output.status.success() {
        let json = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if json.is_empty() {
            Err("nix eval produced no output".into())
        } else {
            Ok(json)
        }
    } else {
        Err(format!(
            "nix eval failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo check 2>&1
```

Expected: Still has errors from `main.rs` not handling new CLI variants, but `nix.rs` itself should compile cleanly (no errors from this file).

- [ ] **Step 3: Commit**

```bash
git add crates/nixosandbox/src/nix.rs
git commit -m "feat: add build_with_catalog() and query_catalog() to nix.rs"
```

---

### Task 7: Wire everything together in main.rs

**Files:**
- Modify: `crates/nixosandbox/src/main.rs`

- [ ] **Step 1: Update the match arm in main() to handle new fields**

In `main()`, replace the `Commands::Create` match arm:

```rust
        Commands::Create { profile, spec: spec_file, with, network, workspace, name, agent, description, json } => {
            cmd_create(profile, spec_file, with, network, workspace, name, agent, description, json);
        }
```

Add the `Catalog` match arm after the `Build` arm:

```rust
        Commands::Catalog { json, filter } => {
            cmd_catalog(json, filter);
        }
```

- [ ] **Step 2: Replace cmd_create with version that handles --with**

Replace the existing `cmd_create` function:

```rust
fn cmd_create(
    profile: Option<String>,
    spec_file: Option<String>,
    with: Option<Vec<String>>,
    network: String,
    workspace: Option<String>,
    name: Option<String>,
    agent: Option<String>,
    description: Option<String>,
    json: bool,
) {
    // Validate mutual exclusivity: --with vs --profile vs --spec
    let source_count = [with.is_some(), profile.is_some(), spec_file.is_some()]
        .iter()
        .filter(|&&b| b)
        .count();
    if source_count > 1 {
        eprintln!("error: specify only one of --profile, --spec, or --with");
        std::process::exit(1);
    }
    if source_count == 0 {
        eprintln!("error: specify --profile, --spec, or --with");
        std::process::exit(1);
    }

    let (rootfs_path, profile_name) = if let Some(ref packages) = with {
        // Catalog-based composition
        if packages.is_empty() {
            eprintln!("error: --with requires at least one package name");
            std::process::exit(1);
        }
        match network.as_str() {
            "off" | "full" => {}
            other => {
                eprintln!("error: --network must be 'off' or 'full', got '{other}'");
                std::process::exit(1);
            }
        }
        let rootfs = nix::build_with_catalog(packages, &network).unwrap_or_else(|e| {
            eprintln!("nix build failed: {e}");
            std::process::exit(1);
        });
        nix::validate_rootfs(&rootfs).unwrap_or_else(|e| {
            eprintln!("rootfs validation failed: {e}");
            std::process::exit(1);
        });
        (rootfs, format!("custom:{}", packages.join(",")))
    } else {
        // Profile or spec-based
        let sandbox_spec = resolve_spec(profile.clone(), spec_file);
        let rootfs = build_rootfs_for_spec(&sandbox_spec, &profile);
        nix::validate_rootfs(&rootfs).unwrap_or_else(|e| {
            eprintln!("rootfs validation failed: {e}");
            std::process::exit(1);
        });
        (rootfs, sandbox_spec.name.clone())
    };

    let session_name = name.unwrap_or_else(|| profile_name.clone());
    let meta = session::create_session(
        &session_name,
        &profile_name,
        &rootfs_path,
        workspace.as_deref(),
        agent.as_deref(),
        description.as_deref(),
    ).unwrap_or_else(|e| {
        eprintln!("session creation failed: {e}");
        std::process::exit(1);
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&meta).unwrap());
    } else {
        println!("{}", meta.session_id);
    }
}
```

- [ ] **Step 3: Add cmd_catalog function**

Add this function at the end of `main.rs` (after `cmd_status`):

```rust
fn cmd_catalog(json: bool, filter: Option<String>) {
    let catalog_json = nix::query_catalog().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    if json && filter.is_none() {
        println!("{}", catalog_json);
        return;
    }

    // Parse for display or filtering
    let catalog: serde_json::Value = serde_json::from_str(&catalog_json).unwrap_or_else(|e| {
        eprintln!("error: failed to parse catalog: {e}");
        std::process::exit(1);
    });

    let filter_lower = filter.as_ref().map(|f| f.to_lowercase());

    if json {
        // Filtered JSON output
        let mut filtered = serde_json::json!({ "agents": {}, "tools": {} });
        for section in ["agents", "tools"] {
            if let Some(entries) = catalog.get(section).and_then(|v| v.as_object()) {
                let filt = filter_lower.as_ref().unwrap();
                let matched: serde_json::Map<String, serde_json::Value> = entries
                    .iter()
                    .filter(|(k, v)| {
                        k.to_lowercase().contains(filt)
                            || v.get("description")
                                .and_then(|d| d.as_str())
                                .map(|d| d.to_lowercase().contains(filt))
                                .unwrap_or(false)
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                filtered[section] = serde_json::Value::Object(matched);
            }
        }
        println!("{}", serde_json::to_string_pretty(&filtered).unwrap());
        return;
    }

    // Human-readable output
    for (section, label) in [("agents", "Agents (from llm-agents.nix)"), ("tools", "Tools (from nixpkgs)")] {
        if let Some(entries) = catalog.get(section).and_then(|v| v.as_object()) {
            println!("{}:", label);
            let mut names: Vec<&String> = entries.keys().collect();
            names.sort();
            for name in names {
                let desc = entries[name]
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                if let Some(ref filt) = filter_lower {
                    if !name.to_lowercase().contains(filt) && !desc.to_lowercase().contains(filt) {
                        continue;
                    }
                }
                println!("  {:<20} {}", name, desc);
            }
            println!();
        }
    }
}
```

- [ ] **Step 4: Build and verify compilation**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo build 2>&1
```

Expected: Clean compilation with at most the existing `BwrapAvailability::Available` platform-conditional warning.

- [ ] **Step 5: Run existing tests**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo test 2>&1
```

Expected: All 19 existing tests pass.

- [ ] **Step 6: Verify --help shows new options**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo run -- create --help 2>&1
```

Expected: Help output includes `--with`, `--network`, and all existing flags.

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo run -- catalog --help 2>&1
```

Expected: Help output shows `--json` and `--filter` options.

- [ ] **Step 7: Verify mutual exclusivity error**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo run -- create --profile strict --with bash 2>&1
```

Expected: `error: specify only one of --profile, --spec, or --with`

- [ ] **Step 8: Commit**

```bash
git add crates/nixosandbox/src/main.rs
git commit -m "feat: wire --with catalog creation and catalog subcommand into main"
```

---

### Task 8: Update Pi extension cli-client.ts

**Files:**
- Modify: `packages/pi-sandbox-extension/src/cli-client.ts`

- [ ] **Step 1: Add CatalogEntry, CatalogResponse types and catalogPackages function**

Add these types after the existing `CreateOptions` interface (around line 44):

```typescript
export interface CatalogEntry {
  description: string;
}

export interface CatalogResponse {
  agents: Record<string, CatalogEntry>;
  tools: Record<string, CatalogEntry>;
}
```

- [ ] **Step 2: Add withPackages and network to CreateOptions**

Update the `CreateOptions` interface to include the new fields:

```typescript
export interface CreateOptions {
  profile?: string;
  workspace?: string;
  name?: string;
  agent?: string;
  description?: string;
  withPackages?: string[];
  network?: string;
}
```

- [ ] **Step 3: Update createSession to handle withPackages**

Replace the `createSession` function:

```typescript
export function createSession(binary: string, opts: CreateOptions): SessionMetadata {
  const args = ["create", "--json"];
  if (opts.withPackages && opts.withPackages.length > 0) {
    args.push("--with", opts.withPackages.join(","));
    if (opts.network) {
      args.push("--network", opts.network);
    }
  } else if (opts.profile) {
    args.push("--profile", opts.profile);
  }
  if (opts.workspace) { args.push("--workspace", opts.workspace); }
  if (opts.name) { args.push("--name", opts.name); }
  if (opts.agent) { args.push("--agent", opts.agent); }
  if (opts.description) { args.push("--description", opts.description); }

  const stdout = execFileSync(binary, args, { encoding: "utf-8" });
  return JSON.parse(stdout.trim()) as SessionMetadata;
}
```

- [ ] **Step 4: Add catalogPackages function**

Add this function after `destroySession`:

```typescript
export function catalogPackages(binary: string, filter?: string): CatalogResponse {
  const args = ["catalog", "--json"];
  if (filter) { args.push("--filter", filter); }
  const stdout = execFileSync(binary, args, { encoding: "utf-8" });
  return JSON.parse(stdout.trim()) as CatalogResponse;
}
```

- [ ] **Step 5: Verify TypeScript compiles**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/packages/pi-sandbox-extension && npx tsc --noEmit 2>&1
```

Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add packages/pi-sandbox-extension/src/cli-client.ts
git commit -m "feat: add catalogPackages() and withPackages to cli-client"
```

---

### Task 9: Add sandbox_catalog tool and update sandbox_run in extension.ts

**Files:**
- Modify: `packages/pi-sandbox-extension/src/extension.ts`

- [ ] **Step 1: Add catalogPackages to imports**

Change the import from `./cli-client.js` to include `catalogPackages`:

```typescript
import {
  createSession,
  statusSession,
  listSessions,
  execCommand,
  catalogPackages,
} from "./cli-client.js";
```

- [ ] **Step 2: Update sandbox_run tool description and parameters**

Replace the `sandboxRun` tool definition inside `createSandboxTools`:

```typescript
  const sandboxRun: ToolDefinition = {
    name: "sandbox_run",
    description:
      "Run a command inside an isolated sandbox. " +
      "Use 'with' to compose from catalog packages (call sandbox_catalog first to see available), " +
      "or 'profile' for a built-in profile. Returns combined stdout/stderr and execution metadata.",
    parameters: Type.Object({
      command: Type.Array(Type.String(), {
        description: "Command and arguments to execute, e.g. [\"bash\", \"-c\", \"echo hello\"]",
        minItems: 1,
      }),
      sessionId: Type.Optional(
        Type.String({ description: "Reuse an existing session. Omit to create a new one." }),
      ),
      with: Type.Optional(
        Type.Array(Type.String(), {
          description: "Package names from the catalog (agents + tools). Mutually exclusive with profile.",
        }),
      ),
      profile: Type.Optional(
        Type.String({ description: "Built-in profile name. Defaults to build-install. Mutually exclusive with 'with'." }),
      ),
      network: Type.Optional(
        Type.String({ description: "Network mode: 'off' for review/analysis, 'full' for build/install. Default: 'off'. Only used with 'with'." }),
      ),
      agent: Type.Optional(
        Type.String({ description: "Agent runtime identifier, e.g. 'claude:opus-4-6'" }),
      ),
      description: Type.Optional(
        Type.String({ description: "Purpose of this sandbox session" }),
      ),
      timeoutMs: Type.Optional(
        Type.Number({ description: "Execution timeout in milliseconds." }),
      ),
    }),
    async execute(args: unknown): Promise<string> {
      const {
        command,
        sessionId: maybeSessionId,
        with: withPackages,
        profile = withPackages ? undefined : "build-install",
        network,
        agent,
        description,
        timeoutMs,
      } = args as {
        command: string[];
        sessionId?: string;
        with?: string[];
        profile?: string;
        network?: string;
        agent?: string;
        description?: string;
        timeoutMs?: number;
      };

      let sid = maybeSessionId;
      if (!sid) {
        const meta = createSession(binaryPath, {
          withPackages,
          profile,
          network,
          agent,
          description,
        });
        sid = meta.sessionId;
      }

      const result = await execCommand(binaryPath, sid, command, { timeoutMs });
      return formatExecResult(result);
    },
  };
```

- [ ] **Step 3: Add sandbox_catalog tool**

Add this tool definition after `sandboxSessionInfo` and before the `return` statement:

```typescript
  // -------------------------------------------------------------------------
  // Tool: sandbox_catalog
  // -------------------------------------------------------------------------
  const sandboxCatalog: ToolDefinition = {
    name: "sandbox_catalog",
    description:
      "List available packages for sandbox composition. " +
      "Returns agents (AI coding tools like claude-code, pi, codex) and tools (utilities like python312, git, ripgrep). " +
      "Call this before sandbox_run with 'with' to see what packages are available.",
    parameters: Type.Object({
      filter: Type.Optional(
        Type.String({ description: "Filter results by name or description substring." }),
      ),
    }),
    async execute(args: unknown): Promise<string> {
      const { filter } = args as { filter?: string };
      const catalog = catalogPackages(binaryPath, filter);

      const lines: string[] = [];

      const agentNames = Object.keys(catalog.agents).sort();
      if (agentNames.length > 0) {
        lines.push("Agents (AI coding tools):");
        for (const name of agentNames) {
          lines.push(`  ${name}  ${catalog.agents[name].description}`);
        }
        lines.push("");
      }

      const toolNames = Object.keys(catalog.tools).sort();
      if (toolNames.length > 0) {
        lines.push("Tools (utilities):");
        for (const name of toolNames) {
          lines.push(`  ${name}  ${catalog.tools[name].description}`);
        }
      }

      return lines.join("\n");
    },
  };
```

- [ ] **Step 4: Update the return array to include sandboxCatalog**

Change the return statement to:

```typescript
  return [
    sandboxRun,
    sandboxReadFile,
    sandboxWriteFile,
    sandboxListFiles,
    sandboxSessionInfo,
    sandboxCatalog,
    sandboxBrowser,
  ];
```

- [ ] **Step 5: Verify TypeScript compiles**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/packages/pi-sandbox-extension && npx tsc --noEmit 2>&1
```

Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add packages/pi-sandbox-extension/src/extension.ts
git commit -m "feat: add sandbox_catalog tool and 'with' param to sandbox_run"
```

---

### Task 10: Update Pi extension index.ts re-exports

**Files:**
- Modify: `packages/pi-sandbox-extension/src/index.ts`

- [ ] **Step 1: Add CatalogEntry, CatalogResponse, and catalogPackages to re-exports**

Update the cli-client type re-exports:

```typescript
export type {
  SessionMetadata,
  StatusResponse,
  ExecResult,
  CreateOptions,
  CatalogEntry,
  CatalogResponse,
} from "./cli-client.js";
export {
  createSession,
  statusSession,
  listSessions,
  destroySession,
  execCommand,
  catalogPackages,
} from "./cli-client.js";
```

- [ ] **Step 2: Verify TypeScript compiles**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/packages/pi-sandbox-extension && npx tsc --noEmit 2>&1
```

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add packages/pi-sandbox-extension/src/index.ts
git commit -m "feat: re-export catalogPackages and CatalogResponse from index"
```

---

### Task 11: Rust unit tests

**Files:**
- Modify: `crates/nixosandbox/src/spec.rs`

- [ ] **Step 1: Add validation test for empty packages**

In `crates/nixosandbox/src/spec.rs`, inside the `#[cfg(test)] mod tests` block, add:

```rust
    #[test]
    fn validate_empty_packages_fails() {
        let spec = SandboxSpec {
            name: "test".to_string(), packages: vec![],
            env: HashMap::new(), network: "full".to_string(),
            namespaces: vec![], writable: vec![],
        };
        assert!(validate_spec(&spec).unwrap_err().iter().any(|e| e.contains("packages")));
    }
```

- [ ] **Step 2: Run all Rust tests**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo test 2>&1
```

Expected: All tests pass including the new one (20 total).

- [ ] **Step 3: Commit**

```bash
git add crates/nixosandbox/src/spec.rs
git commit -m "test: add validation test for empty packages"
```

---

### Task 12: End-to-end smoke test

**Files:** None (manual verification)

This task verifies the full integration works end-to-end. It requires network access to fetch packages.

- [ ] **Step 1: Build the CLI**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo build --release 2>&1
```

Expected: Clean build.

- [ ] **Step 2: Test catalog subcommand**

Run:
```bash
NIXOSANDBOX_FLAKE_ROOT=/Users/hashwarlock/Projects/nixosandbox \
  NIX_SSL_CERT_FILE=/etc/ssl/cert.pem \
  ./target/release/nixosandbox catalog 2>&1 | head -20
```

Expected: Human-readable catalog listing with "Agents (from llm-agents.nix):" and "Tools (from nixpkgs):" sections.

- [ ] **Step 3: Test catalog --json**

Run:
```bash
NIXOSANDBOX_FLAKE_ROOT=/Users/hashwarlock/Projects/nixosandbox \
  NIX_SSL_CERT_FILE=/etc/ssl/cert.pem \
  ./target/release/nixosandbox catalog --json 2>&1 | python3 -m json.tool | head -20
```

Expected: Valid JSON with `agents` and `tools` keys.

- [ ] **Step 4: Test catalog --filter**

Run:
```bash
NIXOSANDBOX_FLAKE_ROOT=/Users/hashwarlock/Projects/nixosandbox \
  NIX_SSL_CERT_FILE=/etc/ssl/cert.pem \
  ./target/release/nixosandbox catalog --filter claude 2>&1
```

Expected: Only claude-related entries shown.

- [ ] **Step 5: Test --with creates a session (requires Nix package downloads)**

Run:
```bash
NIXOSANDBOX_FLAKE_ROOT=/Users/hashwarlock/Projects/nixosandbox \
  NIX_SSL_CERT_FILE=/etc/ssl/cert.pem \
  ./target/release/nixosandbox create --with bash,coreutils --network off --name test-catalog --json 2>&1
```

Expected: JSON session metadata with `profile` containing `"custom:bash,coreutils"`.

Note: This step requires Nix to download packages. If VPN causes issues, disconnect ProtonVPN first or skip this step.

- [ ] **Step 6: Clean up test session (if Step 5 succeeded)**

Use the session ID from Step 5:

```bash
NIXOSANDBOX_FLAKE_ROOT=/Users/hashwarlock/Projects/nixosandbox \
  ./target/release/nixosandbox destroy <session-id> 2>&1
```

- [ ] **Step 7: Verify all Rust tests still pass**

Run:
```bash
cd /Users/hashwarlock/Projects/nixosandbox/crates/nixosandbox && cargo test 2>&1
```

Expected: All tests pass.
