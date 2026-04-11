# Nixo Homebrew, Catalog Grouping, and Cross-Agent Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Homebrew-first install flow with `nixo` as the primary CLI name, preserve `nixosandbox` compatibility, improve catalog readability with grouped output, and add a cross-agent `.agents/skills` skill for runtime portability.

**Architecture:** Keep runtime behavior and JSON compatibility stable while layering UX improvements. Introduce a catalog presentation module for deterministic grouping, dual binary names for migration safety, a release workflow that emits `nixo` artifacts, and a repository-local Agent Skills package under `.agents/skills/nixo-cli`.

**Tech Stack:** Rust (`clap`, `serde_json`), Nix, GitHub Actions, Homebrew formula conventions, Agent Skills (`SKILL.md` frontmatter + progressive disclosure resources).

---

## File Structure

- `crates/nixosandbox/src/catalog.rs` (new): Catalog data model, grouping logic, text/JSON rendering helpers, category mapping.
- `crates/nixosandbox/src/main.rs` (modify): Route `cmd_catalog` through catalog module, add `--grouped` support.
- `crates/nixosandbox/src/cli.rs` (modify): Add `--grouped` to `catalog`, set command branding to `nixo`.
- `crates/nixosandbox/Cargo.toml` (modify): Produce both `nixo` and `nixosandbox` binaries from `src/main.rs`.
- `.github/workflows/release.yml` (new): Tag-driven release artifacts + checksums.
- `packaging/homebrew/nixo.rb` (new): Formula template for tap repository use.
- `README.md` (modify): Homebrew-first install, `nixo` examples, alias notes, grouped catalog docs.
- `CLAUDE.md` (modify): Command examples and architecture guidance updated to `nixo`/alias language and grouped catalog behavior.
- `AGENTS.md` (modify): Agent-facing project instructions updated to `nixo`/alias language and grouped catalog behavior.
- `docs/superpowers/specs/2026-04-10-nixo-homebrew-and-cross-agent-skill-design.md` (modify): Add any implementation-time clarifications discovered during execution.
- `.agents/skills/nixo-cli/SKILL.md` (new): Cross-agent skill instructions.
- `.agents/skills/nixo-cli/references/quick-reference.md` (new): Command recipes.
- `.agents/skills/nixo-cli/references/troubleshooting.md` (new): Error triage paths.
- `.github/workflows/ci.yml` (modify): Add catalog grouped output check and alias parity smoke checks.

### Task 1: Add Catalog Grouping Domain Module

**Files:**
- Create: `crates/nixosandbox/src/catalog.rs`
- Modify: `crates/nixosandbox/src/main.rs`
- Test: `crates/nixosandbox/src/catalog.rs` (unit tests in module)

- [ ] **Step 1: Write failing tests for grouping and rendering**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_known_agents_into_expected_sections() {
        let raw = serde_json::json!({
            "agents": {
                "claude-code": { "description": "Anthropic CLI" },
                "localgpt": { "description": "assistant" },
                "coderabbit-cli": { "description": "review" }
            },
            "tools": { "git": { "description": "git scm" } }
        });
        let grouped = GroupedCatalog::from_catalog_json(&raw).unwrap();
        assert!(grouped.agent_categories.contains_key("AI Coding Agents"));
        assert!(grouped.agent_categories.contains_key("AI Assistants"));
        assert!(grouped.agent_categories.contains_key("Code Review"));
    }

    #[test]
    fn preserves_flat_json_shape_for_default_json_mode() {
        let raw = serde_json::json!({
            "agents": { "claude-code": { "description": "Anthropic CLI" } },
            "tools": { "git": { "description": "git scm" } }
        });
        let flat = CatalogView::from_json(&raw).unwrap().to_flat_json();
        assert!(flat.get("agents").is_some());
        assert!(flat.get("tools").is_some());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/nixosandbox && cargo test catalog::tests -- --nocapture`  
Expected: FAIL because `catalog.rs` does not exist and tests are not wired.

- [ ] **Step 3: Implement catalog module with grouping + stable sort**

```rust
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct CatalogView {
    pub agents: Vec<CatalogEntry>,
    pub tools: Vec<CatalogEntry>,
}

#[derive(Clone, Debug)]
pub struct GroupedCatalog {
    pub agent_categories: BTreeMap<String, Vec<CatalogEntry>>,
    pub tools: Vec<CatalogEntry>,
}

impl CatalogView {
    pub fn from_json(raw: &Value) -> Result<Self, String> {
        let agents = parse_entries(raw, "agents")?;
        let tools = parse_entries(raw, "tools")?;
        Ok(Self { agents, tools })
    }

    pub fn to_flat_json(&self) -> Value {
        serde_json::json!({
            "agents": entries_to_object(&self.agents),
            "tools": entries_to_object(&self.tools),
        })
    }

    pub fn to_grouped(&self) -> GroupedCatalog {
        let mut grouped: BTreeMap<String, Vec<CatalogEntry>> = BTreeMap::new();
        for entry in &self.agents {
            let category = agent_category(&entry.name).to_string();
            grouped.entry(category).or_default().push(entry.clone());
        }
        for entries in grouped.values_mut() {
            entries.sort_by(|a, b| a.name.cmp(&b.name));
        }
        let mut tools = self.tools.clone();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        GroupedCatalog {
            agent_categories: grouped,
            tools,
        }
    }
}

impl GroupedCatalog {
    pub fn from_catalog_json(raw: &Value) -> Result<Self, String> {
        let view = CatalogView::from_json(raw)?;
        Ok(view.to_grouped())
    }
}

fn parse_entries(raw: &Value, section: &str) -> Result<Vec<CatalogEntry>, String> {
    let map = raw
        .get(section)
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!("missing section: {section}"))?;
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.extend(map.keys().cloned());
    Ok(names
        .into_iter()
        .map(|name| CatalogEntry {
            description: map
                .get(&name)
                .and_then(|v| v.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            name,
        })
        .collect())
}

fn entries_to_object(entries: &[CatalogEntry]) -> Map<String, Value> {
    entries
        .iter()
        .map(|e| {
            (
                e.name.clone(),
                serde_json::json!({ "description": e.description }),
            )
        })
        .collect()
}

fn agent_category(name: &str) -> &'static str {
    match name {
        "localgpt" | "hermes-agent" | "openclaw" => "AI Assistants",
        "coderabbit-cli" | "tuicr" => "Code Review",
        _ => "AI Coding Agents",
    }
}
```

- [ ] **Step 4: Wire module into main**

```rust
mod catalog;
```

- [ ] **Step 5: Run tests to verify pass**

Run: `cd crates/nixosandbox && cargo test catalog::tests -- --nocapture`  
Expected: PASS with grouping and flat-shape assertions.

- [ ] **Step 6: Commit**

```bash
git add crates/nixosandbox/src/catalog.rs crates/nixosandbox/src/main.rs
git commit -m "feat: add catalog grouping domain module"
```

### Task 2: Add `catalog --grouped` CLI UX

**Files:**
- Modify: `crates/nixosandbox/src/cli.rs`
- Modify: `crates/nixosandbox/src/main.rs`
- Test: `crates/nixosandbox/src/catalog.rs` (renderer/filter tests)

- [ ] **Step 1: Write failing test for grouped JSON mode**

```rust
#[test]
fn grouped_json_contains_agent_categories() {
    let raw = serde_json::json!({
        "agents": { "claude-code": { "description": "Anthropic CLI" } },
        "tools": { "git": { "description": "git scm" } }
    });
    let grouped = CatalogView::from_json(&raw).unwrap().to_grouped_json();
    assert!(grouped.get("agentCategories").is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/nixosandbox && cargo test grouped_json_contains_agent_categories -- --nocapture`  
Expected: FAIL because `to_grouped_json()` is not implemented.

- [ ] **Step 3: Add `--grouped` flag and command handling**

```rust
// cli.rs
Catalog {
    #[arg(long)]
    json: bool,
    #[arg(long)]
    grouped: bool,
    #[arg(long)]
    filter: Option<String>,
}

// main.rs match arm
Commands::Catalog { json, grouped, filter } => {
    cmd_catalog(json, grouped, filter);
}
```

- [ ] **Step 4: Implement output modes in `cmd_catalog`**

```rust
fn cmd_catalog(json: bool, grouped: bool, filter: Option<String>) {
    let catalog_json = nix::query_catalog().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    let raw: serde_json::Value = serde_json::from_str(&catalog_json).unwrap_or_else(|e| {
        eprintln!("error: failed to parse catalog: {e}");
        std::process::exit(1);
    });
    let view = catalog::CatalogView::from_json(&raw).unwrap_or_else(|e| {
        eprintln!("error: invalid catalog shape: {e}");
        std::process::exit(1);
    });
    if json && !grouped {
        println!("{}", serde_json::to_string_pretty(&view.to_flat_json()).unwrap());
        return;
    }
    if json && grouped {
        println!("{}", serde_json::to_string_pretty(&view.to_grouped_json(filter.as_deref())).unwrap());
        return;
    }
    println!("{}", view.to_grouped_text(filter.as_deref()));
}
```

- [ ] **Step 5: Run tests and smoke command checks**

Run: `cd crates/nixosandbox && cargo test catalog::tests -- --nocapture`  
Expected: PASS  

Run: `NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixosandbox catalog --json --grouped | python3 -m json.tool > /dev/null`  
Expected: command succeeds and JSON parses.

- [ ] **Step 6: Commit**

```bash
git add crates/nixosandbox/src/cli.rs crates/nixosandbox/src/main.rs crates/nixosandbox/src/catalog.rs
git commit -m "feat: add grouped catalog output and grouped json mode"
```

### Task 3: Make `nixo` Primary CLI with `nixosandbox` Compatibility

**Files:**
- Modify: `crates/nixosandbox/Cargo.toml`
- Modify: `crates/nixosandbox/src/cli.rs`
- Modify: `README.md`
- Test: `crates/nixosandbox` build outputs

- [ ] **Step 1: Write failing test/verification command for dual binaries**

```bash
cd crates/nixosandbox
cargo build --release
test -x target/release/nixo
test -x target/release/nixosandbox
```

- [ ] **Step 2: Run to verify it fails**

Run: the command block above  
Expected: FAIL because `target/release/nixo` does not exist.

- [ ] **Step 3: Add dual binary entries and update clap branding**

```toml
[[bin]]
name = "nixo"
path = "src/main.rs"

[[bin]]
name = "nixosandbox"
path = "src/main.rs"
```

```rust
#[derive(Parser)]
#[command(name = "nixo", about = "Reproducible, isolated sandbox environments")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
```

- [ ] **Step 4: Verify both binaries work**

Run: `cd crates/nixosandbox && cargo build --release && ./target/release/nixo --help && ./target/release/nixosandbox --help`  
Expected: both succeed, help text shows `nixo`.

- [ ] **Step 5: Commit**

```bash
git add crates/nixosandbox/Cargo.toml crates/nixosandbox/src/cli.rs README.md
git commit -m "feat: make nixo primary binary and keep nixosandbox alias"
```

### Task 4: Add Release Workflow and Homebrew Formula Template

**Files:**
- Create: `.github/workflows/release.yml`
- Create: `packaging/homebrew/nixo.rb`
- Modify: `README.md`
- Test: workflow lint + formula syntax check

- [ ] **Step 1: Write failing verification for release workflow presence**

Run: `test -f .github/workflows/release.yml`  
Expected: FAIL because file does not exist.

- [ ] **Step 2: Add release workflow**

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

jobs:
  build:
    strategy:
      matrix:
        include:
          - runner: macos-15
            target: aarch64-apple-darwin
          - runner: macos-15-intel
            target: x86_64-apple-darwin
          - runner: ubuntu-latest
            target: x86_64-unknown-linux-gnu
    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release --locked --target ${{ matrix.target }}
        working-directory: crates/nixosandbox
      - run: |
          mkdir -p dist
          cp crates/nixosandbox/target/${{ matrix.target }}/release/nixo dist/nixo-${{ matrix.target }}
          shasum -a 256 dist/nixo-${{ matrix.target }} > dist/nixo-${{ matrix.target }}.sha256
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/*
```

- [ ] **Step 3: Add Homebrew formula template for tap repo**

```bash
TAG="v0.1.0"
ARM64_SHA="$(cut -d' ' -f1 dist/nixo-aarch64-apple-darwin.sha256)"
AMD64_SHA="$(cut -d' ' -f1 dist/nixo-x86_64-apple-darwin.sha256)"
LINUX_SHA="$(cut -d' ' -f1 dist/nixo-x86_64-unknown-linux-gnu.sha256)"

cat > packaging/homebrew/nixo.rb <<EOF
class Nixo < Formula
  desc "Reproducible sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixosandbox"
  version "${TAG#v}"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/HashWarlock/nixosandbox/releases/download/${TAG}/nixo-aarch64-apple-darwin"
      sha256 "${ARM64_SHA}"
    else
      url "https://github.com/HashWarlock/nixosandbox/releases/download/${TAG}/nixo-x86_64-apple-darwin"
      sha256 "${AMD64_SHA}"
    end
  end

  on_linux do
    url "https://github.com/HashWarlock/nixosandbox/releases/download/${TAG}/nixo-x86_64-unknown-linux-gnu"
    sha256 "${LINUX_SHA}"
  end

  def install
    bin.install Dir["nixo-*"].first => "nixo"
    bin.install_symlink "nixo" => "nixosandbox"
  end
end
EOF
```

- [ ] **Step 4: Verify files exist and basic syntax checks**

Run: `test -f .github/workflows/release.yml && test -f packaging/homebrew/nixo.rb`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml packaging/homebrew/nixo.rb README.md
git commit -m "ci: add nixo release workflow and homebrew formula template"
```

### Task 5: Add Cross-Agent Skill in `.agents/skills`

**Files:**
- Create: `.agents/skills/nixo-cli/SKILL.md`
- Create: `.agents/skills/nixo-cli/references/quick-reference.md`
- Create: `.agents/skills/nixo-cli/references/troubleshooting.md`
- Test: frontmatter parse + file layout checks

- [ ] **Step 1: Write failing validation command**

Run: `test -f .agents/skills/nixo-cli/SKILL.md`  
Expected: FAIL because skill does not exist.

- [ ] **Step 2: Create Agent Skills-compliant `SKILL.md`**

```markdown
---
name: nixo-cli
description: Use when creating, managing, and troubleshooting nixo sandbox sessions from any agent runtime, including package catalog discovery and command execution in isolated environments.
---

# Nixo CLI

Load this skill when an agent needs to operate `nixo` sessions end-to-end.

Prefer `nixo` commands. `nixosandbox` is a compatibility alias.

## Core workflow

1. Run `nixo catalog` or `nixo catalog --json` to choose packages.
2. Create sandbox with `nixo create --with ... --network off --json` unless downloads are required.
3. Execute commands via `nixo exec <session-id> -- ...`.
4. Inspect state with `nixo status <session-id> --json`.
5. Destroy sessions when done with `nixo destroy <session-id>`.

## References

- `references/quick-reference.md`
- `references/troubleshooting.md`
```

- [ ] **Step 3: Add reference docs**

```markdown
# Quick Reference

- Catalog: `nixo catalog --json`
- Create: `nixo create --with claude-code,bash --network off --json`
- Exec: `nixo exec <session-id> -- echo hello`
- Status: `nixo status <session-id> --json`
- Destroy: `nixo destroy <session-id>`
```

```markdown
# Troubleshooting

## bwrap unavailable

- Linux: ensure `bwrap` is installed and user namespaces are available.
- macOS: ensure Docker Desktop is running unless `NIXOSANDBOX_NO_DOCKER=1` is intentional.

## Flake root errors

- Set `NIXOSANDBOX_FLAKE_ROOT` to repository root containing `flake.nix`.

## Create argument conflicts

- Do not combine `--with`, `--profile`, and `--spec` in the same `create` command.
```

- [ ] **Step 4: Validate skill shape**

Run: `test -f .agents/skills/nixo-cli/SKILL.md && rg -n "^name:|^description:" .agents/skills/nixo-cli/SKILL.md`  
Expected: PASS with exactly one `name` and one `description` in frontmatter.

- [ ] **Step 5: Commit**

```bash
git add .agents/skills/nixo-cli/SKILL.md .agents/skills/nixo-cli/references/quick-reference.md .agents/skills/nixo-cli/references/troubleshooting.md
git commit -m "feat: add cross-agent nixo cli skill package"
```

### Task 6: Update CI and README for New UX

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: `AGENTS.md`
- Modify: `docs/superpowers/specs/2026-04-10-nixo-homebrew-and-cross-agent-skill-design.md` (only if implementation details changed)
- Test: CI-equivalent local checks

- [ ] **Step 1: Write failing checks for grouped and alias coverage**

Run: `rg -n "catalog --json --grouped|nixo --help|nixosandbox --help" .github/workflows/ci.yml`  
Expected: FAIL because checks are missing.

- [ ] **Step 2: Add CI smoke checks**

```yaml
- name: Catalog grouped JSON shape
  run: |
    NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo catalog --json --grouped | python3 -m json.tool > /dev/null

- name: Alias parity smoke check
  run: |
    ./result/bin/nixo --help > /tmp/nixo-help.txt
    ./result/bin/nixosandbox --help > /tmp/nixosandbox-help.txt
    grep -q "nixo" /tmp/nixo-help.txt
    grep -q "nixo" /tmp/nixosandbox-help.txt
```

- [ ] **Step 3: Update README command examples and install sections**

```markdown
## Install

### Homebrew (recommended)

```bash
brew tap HashWarlock/homebrew-nixo
brew install nixo
```

`nixosandbox` remains available as a compatibility alias.
```

- [ ] **Step 4: Update `CLAUDE.md` and `AGENTS.md` command guidance**

```markdown
### CLI smoke test (Linux only, requires bwrap)
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo catalog --json
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo create --with bash,coreutils --network off --json
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo exec <session-id> -- echo hello
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo destroy <session-id>

Compatibility note: `nixosandbox` remains an alias for `nixo`.
```

- [ ] **Step 5: Reconcile spec doc if implementation changed details**

Run: `rg -n "nixo|catalog --json --grouped|homebrew" docs/superpowers/specs/2026-04-10-nixo-homebrew-and-cross-agent-skill-design.md`  
Expected: PASS and aligned with final implementation behavior.

- [ ] **Step 6: Run verification commands**

Run: `cd crates/nixosandbox && cargo test && cargo build --release`  
Expected: PASS

Run: `NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo catalog --json --grouped | python3 -m json.tool > /dev/null`  
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/ci.yml README.md CLAUDE.md AGENTS.md docs/superpowers/specs/2026-04-10-nixo-homebrew-and-cross-agent-skill-design.md
git commit -m "docs(ci): align nixo homebrew guidance across project docs"
```

## Final Verification Checklist

- [ ] `cd crates/nixosandbox && cargo test`
- [ ] `cd crates/nixosandbox && cargo build --release`
- [ ] `./crates/nixosandbox/target/release/nixo --help`
- [ ] `./crates/nixosandbox/target/release/nixosandbox --help`
- [ ] `NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo catalog`
- [ ] `NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo catalog --json`
- [ ] `NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo catalog --json --grouped`
- [ ] `test -f .agents/skills/nixo-cli/SKILL.md`

## Self-Review

### Spec coverage check

- Homebrew-first install path: Task 4 + README updates in Task 6.
- `nixo` primary with `nixosandbox` alias: Task 3 + CI checks in Task 6.
- Catalog grouping UX: Task 1 and Task 2 + CI grouped checks.
- Cross-agent skill package: Task 5.

### Placeholder scan

- No `TBD`, `TODO`, or unresolved placeholder tokens remain.

### Type/signature consistency

- `cmd_catalog(json, grouped, filter)` signature is introduced consistently in `cli.rs` and `main.rs`.
- `CatalogView`/`GroupedCatalog` naming is consistent across module tests and command routing.
