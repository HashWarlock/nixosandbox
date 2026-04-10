# Dynamic numtide Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hardcoded 25-name whitelist in `nix/catalog.nix` with a dynamic passthrough that exposes every package from `numtide/llm-agents.nix` automatically.

**Architecture:** Remove the `pickExisting` helper and the explicit name list. Assign `agents = builtins.removeAttrs llm-agents-pkgs [ "default" ]` — Nix evaluates this lazily so all 65+ upstream packages become resolvable by name with zero ongoing maintenance.

**Tech Stack:** Nix (flakes), nixosandbox CLI (`nix eval` for catalog query)

---

### Task 1: Replace the whitelist in catalog.nix

**Files:**
- Modify: `nix/catalog.nix`

- [ ] **Step 1: Read the current file to confirm starting state**

```bash
cat nix/catalog.nix
```

Expected: file contains `pickExisting` helper + list of ~25 agent names.

- [ ] **Step 2: Rewrite catalog.nix**

Replace the entire file contents with:

```nix
# nix/catalog.nix
#
# Unified package catalog merging AI agents from llm-agents.nix
# and standard development tools from nixpkgs.
#
# Usage: import ./catalog.nix { pkgs = ...; llm-agents-pkgs = ...; }
{ pkgs, llm-agents-pkgs }:
{
  # All packages from numtide/llm-agents.nix.
  # 'default' is a meta-alias present in every flake packages output; strip it.
  agents = builtins.removeAttrs llm-agents-pkgs [ "default" ];

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

- [ ] **Step 3: Verify the file evaluates without error**

```bash
nix eval --accept-flake-config .#catalog.agents --apply 'x: builtins.length (builtins.attrNames x)'
```

Expected: a number greater than 25 (should be 60+). If this errors, check `flake.nix` still passes `llm-agents-pkgs = llm-agents.packages.${linuxSystem} or {}` — it should, since flake.nix is unchanged.

- [ ] **Step 4: Spot-check a previously missing package resolves**

```bash
nix eval --accept-flake-config '.#catalog.agents.jules.meta.description'
```

Expected: a string like `"Jules, the asynchronous coding agent from Google, in the terminal"` (not an error).

- [ ] **Step 5: Spot-check that the old packages still resolve**

```bash
nix eval --accept-flake-config '.#catalog.agents.claude-code.meta.description'
nix eval --accept-flake-config '.#catalog.agents.codex.meta.description'
```

Expected: both return description strings without error.

- [ ] **Step 6: Verify `default` is absent**

```bash
nix eval --accept-flake-config '.#catalog.agents' --apply 'x: x ? "default"'
```

Expected: `false`

- [ ] **Step 7: Commit**

```bash
git add nix/catalog.nix
git commit -m "feat: expose all llm-agents.nix packages dynamically in catalog"
```

---

### Task 2: Verify nixosandbox catalog CLI output

**Files:** (read-only verification, no changes)

- [ ] **Step 1: Build the nixosandbox CLI if not already built**

```bash
nix build --accept-flake-config .#nixosandbox
```

Expected: `./result/bin/nixosandbox` exists.

- [ ] **Step 2: Run catalog command and check count**

```bash
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixosandbox catalog | grep -c '  '
```

Expected: line count significantly higher than 25 (the old whitelist size).

- [ ] **Step 3: Confirm a new package appears in catalog output**

```bash
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixosandbox catalog | grep jules
```

Expected: `  jules   Jules, the asynchronous coding agent from Google, in the terminal`

- [ ] **Step 4: Confirm JSON mode works**

```bash
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixosandbox catalog --json | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d['agents']), 'agents')"
```

Expected: `60+ agents` (exact number depends on current llm-agents.nix upstream).

- [ ] **Step 5: Commit verification note (optional)**

No code change in this task — skip commit if nothing changed.
