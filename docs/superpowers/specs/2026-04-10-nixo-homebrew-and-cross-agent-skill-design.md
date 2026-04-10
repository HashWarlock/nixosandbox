---
title: nixo Homebrew Packaging, CLI Naming, and Cross-Agent Skill
date: 2026-04-10
status: proposed
---

# Context

The current install path emphasizes Nix builds from source:

- `nix build github:HashWarlock/nixosandbox`
- Run binary from `./result/bin/nixosandbox`

The goal is to make install and usage easier for mainstream users and agent runtimes:

1. Install through Homebrew without requiring users to build the whole project with Nix.
2. Use `nixo` as the primary CLI command.
3. Keep `nixosandbox` as a compatibility alias.
4. Provide a cross-agent skill using the Agent Skills standard so any compliant runtime can use the CLI reliably.

# Decisions

## Chosen approach

Use a release-artifact + Homebrew tap model:

- Publish prebuilt binaries for supported targets from GitHub Releases.
- Homebrew formula installs `nixo` as the primary executable.
- Homebrew formula installs `nixosandbox` as a symlink alias to preserve compatibility.

## Naming policy

- Canonical CLI name: `nixo`
- Compatibility alias: `nixosandbox`
- Documentation should gradually shift examples to `nixo`, while acknowledging alias compatibility.

## Skill policy

- Create project-level skill at `.agents/skills/nixo-cli/`.
- Follow Agent Skills conventions for discovery and progressive disclosure.
- Keep instructions runtime-agnostic (no Codex/Claude/Gemini-specific tool assumptions).

# Design

## 1) Homebrew packaging architecture

### Release artifacts

Introduce release artifacts for:

- macOS arm64
- macOS x86_64
- Linux x86_64 (and optionally Linux arm64 when release process is stable)

Artifacts should include:

- binary named `nixo`
- checksum files per target

### Homebrew tap formula

Create or use a tap repository with a formula (e.g. `nixo.rb`) that:

- downloads the correct target artifact
- installs the binary as `nixo`
- creates compatibility symlink:
  - `bin.install_symlink "nixo" => "nixosandbox"`

### Versioning and update flow

- Tag release in this repo (semantic versioning recommended).
- CI publishes release artifacts + checksums.
- Tap formula is updated to new version/checksum.

## 2) CLI naming integration

### Runtime behavior

Support both invocation names:

- `nixo` (primary)
- `nixosandbox` (alias compatibility)

Implementation intent:

- Set clap command display name/help to `nixo`.
- Keep behavior identical regardless of invoked binary name.
- Preserve existing flags/subcommands exactly.

### Backward compatibility

- Existing scripts using `nixosandbox ...` continue to work via alias/symlink.
- Existing metadata/env vars remain unchanged for now (`NIXOSANDBOX_*`) to avoid breakage.
- Optional future phase can add mirrored `NIXO_*` env vars if needed.

## 3) Cross-agent skill design

Skill location:

- `.agents/skills/nixo-cli/SKILL.md`

Optional resources:

- `.agents/skills/nixo-cli/references/quick-reference.md`
- `.agents/skills/nixo-cli/references/troubleshooting.md`

Frontmatter requirements:

- `name`: `nixo-cli`
- `description`: trigger-focused, starts with "Use when..."

Skill content scope:

- command workflows:
  - discover packages (`catalog`)
  - create session (`create --with ... --network ... --json`)
  - execute command (`exec <id> -- ...`)
  - inspect (`status`, `list`)
  - cleanup (`destroy`)
- safe defaults:
  - favor `--network off` unless task needs downloads
  - use `--json` for machine-readable flows
- compatibility note:
  - prefer `nixo`, mention `nixosandbox` alias
- failure triage:
  - bwrap unavailable
  - flake root resolution issues
  - invalid package names or mixed `--with/--profile/--spec`

## 4) Documentation updates

Update README install and examples:

- add Homebrew as primary install path
- keep Nix-from-source as developer/advanced path
- switch command examples to `nixo`
- mention compatibility alias explicitly

## 5) Catalog readability and grouping

The current catalog output is functionally correct but hard to scan as agent count grows.

### UX goals

- Make `nixo catalog` easier for humans to scan quickly.
- Preserve existing machine consumers that parse current `--json` output.
- Offer richer grouped JSON for new consumers without forced migration.

### Output behavior

- `nixo catalog` (plain text):
  - grouped by category by default (readability-first).
- `nixo catalog --json`:
  - keep current flat compatibility shape:
    - top-level `agents` map
    - top-level `tools` map
- `nixo catalog --json --grouped`:
  - return grouped JSON view for clients that want category structure.

### Category model

Use category labels aligned with `llm-agents.nix` README conventions where possible (for example `AI Coding Agents`, `AI Assistants`, `Code Review`, `Utilities`).

Implementation note:

- Category metadata is not currently exposed in the catalog query path.
- Add a lightweight category mapping layer in `nixosandbox` (or derive from upstream metadata if later exposed) so grouping remains deterministic and stable.

# Data flow and operations

1. User runs `brew install <tap>/nixo`.
2. Homebrew installs `nixo` binary and `nixosandbox` symlink.
3. User executes `nixo create ...`.
4. CLI behavior remains identical to current `nixosandbox` command handling.
5. Agent runtimes discover `.agents/skills/nixo-cli/SKILL.md` and apply standard skill activation patterns.
6. `nixo catalog` presents grouped human-readable sections while `--json` keeps flat compatibility by default.

# Error handling and edge cases

- Missing release artifact for platform:
  - formula should fail clearly; CI should validate supported platform matrix before release.
- Alias drift:
  - test should assert `nixosandbox --help` works and matches `nixo --help`.
- Existing users pinned to old command:
  - preserved through symlink alias.
- Skill parsing issues across clients:
  - keep YAML simple; avoid malformed frontmatter; keep required fields present.

# Testing strategy

## Packaging tests

- Validate tarball checksums in release workflow.
- Install via Homebrew in CI on macOS runners and run:
  - `nixo --help`
  - `nixosandbox --help`
  - one non-destructive command (e.g., `catalog --json`)

## CLI compatibility tests

- Add tests confirming both executable names function.
- Ensure output parity for key commands.

## Catalog UX tests

- `nixo catalog` renders grouped sections with stable ordering.
- `nixo catalog --json` remains backward-compatible with current shape.
- `nixo catalog --json --grouped` returns grouped schema with deterministic category names.

## Skill validation tests

- Validate skill frontmatter and file layout against Agent Skills expectations.
- Smoke test by loading skill from `.agents/skills/` in at least one runtime that supports Agent Skills discovery.

# Non-goals

- Replacing Nix internals with non-Nix runtime composition.
- Renaming all internal `NIXOSANDBOX_*` env vars in this phase.
- Building a client-specific skill format; this stays Agent Skills standard.

# Rollout plan

1. Add release artifact workflow for `nixo`.
2. Create/maintain Homebrew tap formula with alias symlink.
3. Update CLI branding/help and docs to `nixo`.
4. Add `.agents/skills/nixo-cli` skill and references.
5. Validate install + alias + skill discovery in CI/manual smoke tests.
