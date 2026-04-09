# llm-agents.nix Integration Design

## Goal

Integrate `numtide/llm-agents.nix` as a flake input to provide a unified package catalog of 80+ AI agent tools alongside nixpkgs utilities. This enables agent runtimes to compose custom sandboxes by name — turning natural language task descriptions into concrete, isolated execution environments.

## Architecture

nixosandbox gains a **catalog-driven composition layer**. The Nix flake merges two package sources into a queryable catalog. The Rust CLI exposes this catalog and a new `--with` flag. Agent-facing tool schemas guide LLMs to pick the right packages for any task.

```
Agent (LLM) reasons about task
  → calls sandbox_catalog to see available packages
  → calls sandbox_create --with claude-code,python312,git --network off
  → nixosandbox resolves names from catalog
  → mkAgentSandbox → mkSandboxRootfs → rootfs in /nix/store
  → session ready for exec
```

## Key Decisions

- **Approach 1 (Catalog-Driven Composition)** over profiles-only or dynamic Nix expressions
- **Rootfs over OCI** — near-instant from cached Nix store paths, no registry/daemon overhead
- **Agent is the intelligence** — no NLP in the CLI; the LLM reasons about what to install from good tool descriptions and a queryable catalog
- **Two nixpkgs sources, no follows** — our base tools from nixos-25.11 (stable), agent packages from llm-agents.nix's nixpkgs-unstable (pre-built via numtide binary cache)
- **Nix library + CLI + tool schemas** — composable at all three layers

---

## Part 1: Flake Integration & Package Catalog

### Flake inputs

```nix
inputs = {
  nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
  llm-agents.url = "github:numtide/llm-agents.nix";
  # No follows — they keep their own nixpkgs-unstable + binary cache
};
```

Add `nixConfig` to include numtide's binary cache:

```nix
nixConfig = {
  extra-substituters = [ "https://cache.numtide.com" ];
  extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
};
```

### Package catalog (`nix/catalog.nix`)

Builds a unified catalog from both sources:

```nix
{ pkgs, llm-agents-pkgs }:
{
  agents = {
    inherit (llm-agents-pkgs)
      claude-code pi codex amp goose-cli gemini-cli
      opencode copilot-cli droid forge cursor-agent
      /* all available agent packages */;
  };

  tools = {
    inherit (pkgs)
      python312 nodejs_22 rustc cargo git curl
      coreutils bash gnugrep gnused jq ripgrep fd
      /* standard dev tools */;
  };
}
```

Two namespaces (`agents` and `tools`) prevent accidental conflicts between sources and make it clear to LLMs what's an agent runtime vs a utility.

### Flake output

```nix
catalog = forAllSystems (system: import ./nix/catalog.nix {
  pkgs = nixpkgs.legacyPackages.${system};
  llm-agents-pkgs = llm-agents.packages.${system};
});
```

### nixpkgs version strategy

| Layer | Source | Rationale |
|-------|--------|-----------|
| Base tools (coreutils, bash, git, etc.) | nixos-25.11 (ours) | Stable, predictable |
| Agent packages (claude-code, pi, codex, etc.) | nixpkgs-unstable (theirs, via binary cache) | Pre-built, self-contained |
| nixosandbox Rust CLI | nixos-25.11 (ours) | Our code, our stability |

Agent packages are mostly binary downloads, npm bundles, or Go static binaries — they don't share libraries with base tools. The rule: don't mix same-domain packages from both sources in one rootfs (e.g., don't request their `python` alongside our `python312`).

---

## Part 2: `mkAgentSandbox` Nix Function

New file `nix/mkAgentSandbox.nix` — a catalog-aware composition layer over `mkSandboxRootfs`:

```nix
{ pkgs, catalog, mkSandboxRootfs }:

{ name
, packages ? []        # names resolved against agents-first, then tools
, extraPackages ? []   # raw Nix derivations for advanced users
, env ? {}
, network ? "off"
, namespaces ? ["pid" "mount" "uts" "ipc"]
, writable ? ["/workspace" "/home/sandbox" "/tmp"]
}:

let
  resolvePackage = pname:
    if catalog.agents ? ${pname} then catalog.agents.${pname}
    else if catalog.tools ? ${pname} then catalog.tools.${pname}
    else throw "unknown package '${pname}' — not in agents or tools catalog";

  resolvedPackages = map resolvePackage packages;
  allPackages = resolvedPackages ++ extraPackages;
in
  mkSandboxRootfs {
    inherit name env;
    packages = allPackages;
  }
```

### Composition paths

Existing JSON profiles are unchanged — they go through `loadProfile` directly:

```
Profiles path:    profiles/*.json → loadProfile → mkSandboxRootfs → rootfs
Catalog path:     package names   → mkAgentSandbox → mkSandboxRootfs → rootfs
```

### Flake library output

```nix
lib = {
  inherit mkSandboxRootfs;    # existing — raw derivation lists
  inherit mkAgentSandbox;     # new — catalog-aware, name-based
};
```

---

## Part 3: CLI Changes

### New `--with` flag on `create`

```
nixosandbox create --name "code-review" \
  --with claude-code,git,ripgrep \
  --network off \
  --agent "claude:opus-4-6" \
  --json
```

Accepts a comma-separated list of names from the unified catalog (the Pi extension tool schema uses a native array; the CLI parses commas). Mutually exclusive with `--profile` and `--spec`:

| Flag | Source | Use case |
|------|--------|----------|
| `--profile strict` | Built-in JSON profile | Known, curated environments |
| `--spec ./my-spec.json` | Custom JSON file | User-defined specs |
| `--with claude-code,git` | Catalog resolution | Dynamic, agent-driven composition |

### Resolution in `nix.rs`

New function `build_with_catalog()` generates a Nix expression that calls `mkAgentSandbox`:

```rust
pub fn build_with_catalog(
    flake_root: &str,
    names: &[String],
    network: &str,
    env: &HashMap<String, String>,
) -> Result<String, String> {
    // Generates and evaluates:
    // let flake = builtins.getFlake "<flake_root>";
    //     mkAgentSandbox = flake.lib.mkAgentSandbox;
    // in mkAgentSandbox {
    //   name = "custom-<hash>";
    //   packages = [ "claude-code" "git" "ripgrep" ];
    //   network = "off";
    //   env = { ... };
    // }
}
```

### New `catalog` subcommand

```
$ nixosandbox catalog
Agents (from llm-agents.nix):
  claude-code    Anthropic's Claude Code CLI
  pi             Terminal-based coding agent
  codex          OpenAI Codex CLI
  amp            Sourcegraph coding agent
  ...

Tools (from nixpkgs):
  python312      Python 3.12 interpreter
  nodejs_22      Node.js 22 LTS
  git            Distributed version control
  ...

$ nixosandbox catalog --json
{
  "agents": { "claude-code": { "description": "..." }, ... },
  "tools": { "python312": { "description": "..." }, ... }
}
```

The JSON output is what agents consume to reason about available packages.

### Implementation in `nix.rs`

New function `query_catalog()` evaluates the catalog flake output:

```rust
pub fn query_catalog(flake_root: &str) -> Result<CatalogOutput, String> {
    // nix eval '.#catalog.<system>' --json
    // Returns structured catalog with names and descriptions
}
```

---

## Part 4: Agent Tool Schema

### Updated `sandbox_create` tool

```typescript
{
  name: "sandbox_create",
  description: `Create an isolated sandbox environment for executing tasks.
    First call 'sandbox_catalog' to see available agents and tools,
    then compose a sandbox by picking what you need for the task.
    
    Agents are AI coding tools (claude-code, pi, codex, etc.).
    Tools are utilities (python312, git, nodejs_22, ripgrep, etc.).
    
    Choose 'network: off' for review/analysis tasks.
    Choose 'network: full' for build/install tasks that need downloads.`,
  parameters: {
    name: { type: "string", description: "Human-readable session name" },
    with: { 
      type: "array", items: { type: "string" },
      description: "Package names from the catalog (agents + tools)"
    },
    profile: { type: "string", description: "OR use a built-in profile instead of --with" },
    network: { type: "string", enum: ["off", "full"], default: "off" },
    workspace: { type: "string", description: "Host directory to mount as /workspace" },
    agent: { type: "string", description: "Agent runtime identifier" },
    description: { type: "string", description: "What this sandbox is for" },
  }
}
```

### New `sandbox_catalog` tool

```typescript
{
  name: "sandbox_catalog",
  description: `List all available packages for sandbox_create --with.
    Returns agents (AI coding tools) and tools (utilities) with descriptions.
    Call this before sandbox_create to see what's available.`,
  parameters: {
    filter: { type: "string", description: "Optional search term to filter results" },
    json: { type: "boolean", default: true }
  }
}
```

### Natural language flow

The agent IS the intelligence. No NLP in the CLI. The flow:

1. Agent receives task description from user
2. Agent calls `sandbox_catalog` to see available packages
3. Agent reasons about what packages the task needs
4. Agent calls `sandbox_create` with concrete `--with` names
5. Agent calls `sandbox_exec` to do the work

---

## Part 5: What Changes Where

### New files

| File | Purpose |
|------|---------|
| `nix/catalog.nix` | Unified package catalog from both sources |
| `nix/mkAgentSandbox.nix` | Catalog-aware composition layer |

### Modified files

| File | Change |
|------|--------|
| `flake.nix` | Add `llm-agents` input, `nixConfig`, expose `catalog` and `mkAgentSandbox` |
| `crates/nixosandbox/src/cli.rs` | Add `--with` flag on `create`, add `Catalog` subcommand |
| `crates/nixosandbox/src/main.rs` | Wire `cmd_catalog`, wire `--with` into create flow |
| `crates/nixosandbox/src/nix.rs` | Add `build_with_catalog()`, add `query_catalog()` |
| `packages/pi-sandbox-extension/src/extension.ts` | Add `sandbox_catalog` tool, update `sandbox_create` with `with` param |
| `packages/pi-sandbox-extension/src/cli-client.ts` | Add `catalogPackages()`, update `createSession()` for `--with` |

### Unchanged files

- `nix/mkSandboxRootfs.nix` — untouched foundation
- `nix/profiles/*.json` — backward compatible
- `crates/nixosandbox/src/session.rs` — no session model changes
- `crates/nixosandbox/src/plan_builder.rs` — bwrap argv unchanged
- `crates/nixosandbox/src/bubblewrap.rs`, `docker.rs` — isolation layer unchanged
- `packages/pi-sandbox-extension/src/contract.ts`, `crash-synthesis.ts` — protocol unchanged

### Testing strategy

1. **Nix eval test** — `nix eval .#catalog.x86_64-linux` succeeds, contains expected names
2. **Catalog CLI test** — `nixosandbox catalog --json` returns valid JSON with both namespaces
3. **`--with` integration test** — `nixosandbox create --with git,bash --network off --json` produces a valid session
4. **Error cases** — `--with nonexistent` errors clearly; `--with` + `--profile` together rejected
5. **Existing tests** — all current profile-based tests pass unchanged

### Out of scope

- OCI image output
- Overriding llm-agents.nix's nixpkgs (`follows`)
- Cross-namespace conflict detection (Nix's `buildEnv` collision errors are sufficient for v1)
- MCP server wrapper (can add later over the CLI)
- Auto-updating llm-agents.nix input (manual `nix flake update llm-agents`)

---

## Comparison: nixosandbox vs agent-images

For reference, here is how this design compares to `nothingnesses/agent-images`:

| Aspect | nixosandbox (this design) | agent-images |
|--------|--------------------------|--------------|
| Isolation | bubblewrap + Docker fallback | OCI containers (Podman/Docker) |
| Image format | Nix store rootfs (local, instant) | OCI layered image (distributable) |
| Composition | `mkAgentSandbox` with name-based catalog | `mkAgentImage` with Nix attrsets |
| Agent catalog | llm-agents.nix (80+ agents) + nixpkgs tools | llm-agents.nix (30 agents) |
| Runtime | Rust CLI with session lifecycle | None (delegates to agent-box / podman) |
| Session state | Built-in (create/exec/status/destroy) | Stateless |
| Network control | off/full per profile, namespace-level | Container networking |
| macOS | Docker sidecar fallback | Linux-only |
| Natural language | Agent tool schemas + queryable catalog | Not supported |
| Distribution | Local Nix store only | OCI registries |

Our advantages: lightweight isolation, session management, macOS support, structured event streaming, agent-driven composition via catalog + tool schemas.

Their advantages: OCI distribution, Nix-inside-container, nix-ld for foreign binaries, simpler mental model for manual use.
