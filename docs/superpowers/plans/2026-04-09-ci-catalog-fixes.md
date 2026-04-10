# CI Additions & Catalog Defensive Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add openclaw and hermes-agent smoke tests plus a catalog count assertion to CI, and fix two latent fragility points in the catalog evaluation pipeline.

**Architecture:** Task 1 extends `.github/workflows/ci.yml` only (matrix rows + a new step in nix-build). Task 2 hardens the Nix expression in `nix.rs` with a `filterAttrs` derivation guard and adds a `builtins.trace` warning to `catalog.nix` for the empty-attrset case.

**Tech Stack:** GitHub Actions YAML, Rust (string formatting), Nix (builtins)

---

## File Map

| File | Task | Change |
|------|------|--------|
| `.github/workflows/ci.yml` | 1 | Add 2 matrix rows; add catalog-count step to nix-build job |
| `crates/nixosandbox/src/nix.rs` | 2 | Add `filterDrvs` to `query_catalog` Nix expression |
| `nix/catalog.nix` | 2 | Add `builtins.trace` guard for empty `llm-agents-pkgs` |

---

### Task 1: CI additions — new agent tests + catalog count assertion

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add openclaw and hermes-agent to the agent smoke test matrix**

  In `.github/workflows/ci.yml`, locate the `matrix.include` block inside `agent-smoke-tests`
  (currently ends at the `pi` entry around line 150). Add two new rows immediately after `pi`:

  ```yaml
          - agent: openclaw
            binary: openclaw
            check: "--help"
          - agent: hermes-agent
            binary: hermes
            check: "--version"
  ```

  The block after adding should look like:

  ```yaml
      strategy:
        fail-fast: false
        matrix:
          include:
            - agent: claude-code
              binary: claude
              check: "--version"
            - agent: codex
              binary: codex
              check: "--help"
            - agent: opencode
              binary: opencode
              check: "--version"
            - agent: amp
              binary: amp
              check: "--help"
            - agent: droid
              binary: droid
              check: "--version"
            - agent: pi
              binary: pi
              check: "--help"
            - agent: openclaw
              binary: openclaw
              check: "--help"
            - agent: hermes-agent
              binary: hermes
              check: "--version"
  ```

- [ ] **Step 2: Add catalog count + presence assertion in the nix-build job**

  In `.github/workflows/ci.yml`, locate the `nix-build` job's "Test catalog subcommand" step
  (around line 96). Add a new step **immediately after** it:

  ```yaml
        - name: Verify catalog agent count and new packages
          run: |
            agent_count=$(NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixosandbox catalog --json | \
              python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d['agents']))")
            echo "Catalog agent count: $agent_count"
            [ "$agent_count" -gt 25 ] || { echo "ERROR: expected >25 agents, got $agent_count"; exit 1; }
            NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixosandbox catalog --json | \
              python3 -c "
  import sys, json
  d = json.load(sys.stdin)
  for name in ['openclaw', 'hermes-agent', 'jules']:
      assert name in d['agents'], f'{name} missing from catalog'
  print(f'All expected agents present in {len(d[\"agents\"])} total')
  "
  ```

- [ ] **Step 3: Verify YAML is valid locally**

  ```bash
  python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo "YAML valid"
  ```

  Expected: `YAML valid` with no errors. If not valid, fix the indentation error.

- [ ] **Step 4: Commit**

  ```bash
  git add .github/workflows/ci.yml
  git commit -m "ci: add openclaw and hermes-agent smoke tests, verify catalog agent count"
  ```

---

### Task 2: Defensive catalog fixes

**Files:**
- Modify: `crates/nixosandbox/src/nix.rs` (lines ~135–137, the `query_catalog` Nix expression)
- Modify: `nix/catalog.nix` (the `agents =` line)

#### Part A — Filter non-derivation attrs in `query_catalog`

- [ ] **Step 1: Read the current `query_catalog` expression**

  ```bash
  sed -n '130,145p' crates/nixosandbox/src/nix.rs
  ```

  Expected: shows the `let expr = format!(r#"..."#, flake_root)` block with `extractMeta`.

- [ ] **Step 2: Update the Nix expression in `query_catalog`**

  In `crates/nixosandbox/src/nix.rs`, find the `query_catalog` function (line ~130).
  Replace the `let expr = format!(...)` block with:

  ```rust
      let expr = format!(
          r#"let flake = builtins.getFlake "{}"; catalog = flake.catalog; filterDrvs = attrs: builtins.filterAttrs (_: pkg: (pkg.type or "") == "derivation") attrs; extractMeta = attrs: builtins.mapAttrs (name: pkg: {{ description = pkg.meta.description or ""; }}) (filterDrvs attrs); in {{ agents = extractMeta catalog.agents; tools = extractMeta catalog.tools; }}"#,
          flake_root
      );
  ```

  The only change from the original is the addition of:
  `filterDrvs = attrs: builtins.filterAttrs (_: pkg: (pkg.type or "") == "derivation") attrs;`
  and wrapping `extractMeta`'s `attrs` argument with `(filterDrvs attrs)`.

- [ ] **Step 3: Verify the Rust still compiles**

  ```bash
  cd crates/nixosandbox && cargo build 2>&1 | tail -5
  ```

  Expected: `Finished` line with no errors.

- [ ] **Step 4: Run the Rust test suite to confirm no regression**

  ```bash
  cd crates/nixosandbox && cargo test 2>&1 | tail -10
  ```

  Expected: all tests pass (`test result: ok. N passed`).

- [ ] **Step 5: Verify `query_catalog` still works end-to-end**

  ```bash
  cd /path/to/repo  # run from repo root, not crates/nixosandbox
  NIXOSANDBOX_FLAKE_ROOT=$PWD cargo run --manifest-path crates/nixosandbox/Cargo.toml -- catalog --json | \
    python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d['agents']), 'agents,', len(d['tools']), 'tools')"
  ```

  Expected: prints something like `88 agents, 24 tools` (no error, no reduction in count — since current llm-agents.nix exposes only derivations anyway, the filter is a no-op in practice).

#### Part B — Trace warning for empty `llm-agents-pkgs`

- [ ] **Step 6: Read the current `catalog.nix`**

  ```bash
  cat nix/catalog.nix
  ```

  Expected: shows `agents = builtins.removeAttrs llm-agents-pkgs [ "default" ];` (set in prior task).

- [ ] **Step 7: Add empty-attrset guard with `builtins.trace`**

  In `nix/catalog.nix`, replace the `agents = ...` line:

  **Before:**
  ```nix
    # All packages from numtide/llm-agents.nix.
    # 'default' is a meta-alias present in every flake packages output; strip it.
    agents = builtins.removeAttrs llm-agents-pkgs [ "default" ];
  ```

  **After:**
  ```nix
    # All packages from numtide/llm-agents.nix.
    # 'default' is a meta-alias present in every flake packages output; strip it.
    # If llm-agents-pkgs is empty (e.g. flake input missing x86_64-linux support),
    # emit a trace warning rather than silently returning an empty catalog.
    agents =
      let filtered = builtins.removeAttrs llm-agents-pkgs [ "default" ];
      in if filtered == {}
         then builtins.trace
                "nixosandbox WARNING: llm-agents-pkgs is empty — catalog will have no agents. Check that the llm-agents.nix flake input exposes x86_64-linux packages."
                {}
         else filtered;
  ```

- [ ] **Step 8: Verify catalog still evaluates correctly (non-empty path)**

  ```bash
  nix eval --accept-flake-config .#catalog.agents --apply 'x: builtins.length (builtins.attrNames x)'
  ```

  Expected: prints the agent count (88 or similar). No warnings should appear because `llm-agents-pkgs` is non-empty.

- [ ] **Step 9: Commit both fixes together**

  ```bash
  git add crates/nixosandbox/src/nix.rs nix/catalog.nix
  git commit -m "fix: guard query_catalog against non-derivation attrs, warn on empty llm-agents-pkgs"
  ```
