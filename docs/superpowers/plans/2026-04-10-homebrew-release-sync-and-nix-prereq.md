# Homebrew Release Sync And Nix Prerequisite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automate Homebrew formula version/checksum updates after each tagged release and remove the broken Homebrew `nix` dependency in favor of explicit runtime prerequisite handling.

**Architecture:** Keep the existing tag-driven release flow, then add a post-release formula-sync job that rewrites `Formula/nixo.rb` from published asset digests and commits the result back to `master`. In parallel, remove `depends_on "nix"` from the formula and add a focused CLI-side Nix prerequisite check plus docs updates so Homebrew install succeeds while runtime failures remain actionable.

**Tech Stack:** GitHub Actions, shell scripting, Ruby formula syntax, Rust CLI, Homebrew, GitHub Releases

---

## File Map

- Modify: `.github/workflows/release.yml`
  - Add the post-release formula-sync job.
  - Wire in artifact digest lookup and commit-back behavior.
  - Keep the existing build-and-release jobs intact.
- Create: `scripts/update_homebrew_formula.sh`
  - Deterministically rewrite only `version` and `sha256` lines in `Formula/nixo.rb`.
- Modify: `Formula/nixo.rb`
  - Remove `depends_on "nix"`.
  - Keep package/test behavior aligned with tap installation.
- Modify: `crates/nixosandbox/src/lib.rs`
  - Add a centralized runtime prerequisite check before code paths that shell out to `nix`.
- Modify: `crates/nixosandbox/src/nix.rs`
  - Expose or reuse a targeted Nix detection helper if needed by the runtime prerequisite check.
- Modify: `README.md`
  - Document the two-step install flow: install Nix, then install `nixo` via Homebrew.
  - Document that formula sync is automated after release.
- Modify: `AGENTS.md`
  - Update release packaging guidance and Homebrew prerequisite expectations.
- Modify: `CLAUDE.md`
  - Mirror the release packaging and prerequisite guidance changes.

### Task 1: Add Formula Update Script

**Files:**
- Create: `scripts/update_homebrew_formula.sh`
- Test: `Formula/nixo.rb`

- [ ] **Step 1: Write the script skeleton with strict shell settings**

```bash
#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 <version> <macos-arm64-sha256> <macos-x86_64-sha256> <linux-x86_64-sha256>" >&2
  exit 1
fi

version="$1"
macos_arm64_sha="$2"
macos_x86_64_sha="$3"
linux_x86_64_sha="$4"
formula="Formula/nixo.rb"

if [[ ! -f "$formula" ]]; then
  echo "missing formula: $formula" >&2
  exit 1
fi
```

- [ ] **Step 2: Add deterministic rewrite logic for version and checksums only**

```bash
python3 - <<'PY' "$formula" "$version" "$macos_arm64_sha" "$macos_x86_64_sha" "$linux_x86_64_sha"
import re
import sys
from pathlib import Path

formula_path = Path(sys.argv[1])
version, mac_arm, mac_x86, linux_x86 = sys.argv[2:6]
text = formula_path.read_text()

patterns = {
    r'(^\s*version ")[^"]+("\s*$)': rf'\g<1>{version}\2',
    r'(^\s*sha256 ")[0-9a-f]+("\s*$)': None,
}

text, count = re.subn(r'(^\s*version ")[^"]+("\s*$)', rf'\g<1>{version}\2', text, count=1, flags=re.MULTILINE)
if count != 1:
    raise SystemExit("expected exactly one version line")

sha_lines = re.findall(r'^\s*sha256 "([0-9A-Za-z_]+)"\s*$', text, flags=re.MULTILINE)
if len(sha_lines) != 3:
    raise SystemExit("expected exactly three sha256 lines")

replacement_values = [mac_arm, mac_x86, linux_x86]
for value in replacement_values:
    if not re.fullmatch(r'[0-9a-f]{64}', value):
        raise SystemExit(f"invalid sha256: {value}")

for value in replacement_values:
    text, count = re.subn(r'(^\s*sha256 ")[^"]+("\s*$)', rf'\g<1>{value}\2', text, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit("failed to replace sha256 line")

formula_path.write_text(text)
PY
```

- [ ] **Step 3: Make the script executable and run it against the current formula**

Run: `chmod +x scripts/update_homebrew_formula.sh && scripts/update_homebrew_formula.sh 0.1.1 52e0a8482a4528832a5b95a754f57f818524b4d5fa1c738f3372fc4b6f269879 8e484b841373c619c27d0de72201c6acb81adc3a828ce54dd734ea1a7007c736 d4a51cb17981e947bdfd25bdf126db7e22c2d5bd5058db1caa6819d59555272a`
Expected: exit 0 with no unrelated file changes

- [ ] **Step 4: Verify formula syntax after script execution**

Run: `ruby -c Formula/nixo.rb`
Expected: `Syntax OK`

- [ ] **Step 5: Commit**

```bash
git add scripts/update_homebrew_formula.sh Formula/nixo.rb
git commit -m "build: add homebrew formula update script"
```

### Task 2: Remove Broken Homebrew Nix Dependency

**Files:**
- Modify: `Formula/nixo.rb`
- Test: `Formula/nixo.rb`

- [ ] **Step 1: Delete the Homebrew dependency declaration**

```ruby
class Nixo < Formula
  desc "Reproducible, isolated sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixo"
  version "0.1.1"
```

Expected change: remove the existing `depends_on "nix"` line and nothing else in the formula header.

- [ ] **Step 2: Keep the formula test packaging-only**

```ruby
  test do
    assert_predicate pkgshare/"flake/flake.nix", :exist?
    assert_predicate pkgshare/"flake/flake.lock", :exist?
    assert_match "nixo", shell_output("#{bin}/nixo --help")
    assert_match "nixo", shell_output("#{bin}/nixosandbox --help")
  end
end
```

Expected result: no Nix execution is introduced into `test do`.

- [ ] **Step 3: Verify formula syntax again**

Run: `ruby -c Formula/nixo.rb`
Expected: `Syntax OK`

- [ ] **Step 4: Commit**

```bash
git add Formula/nixo.rb
git commit -m "packaging: remove homebrew nix dependency"
```

### Task 3: Add CLI Nix Runtime Prerequisite Check

**Files:**
- Modify: `crates/nixosandbox/src/lib.rs`
- Modify: `crates/nixosandbox/src/nix.rs`
- Test: `crates/nixosandbox/src/lib.rs`

- [ ] **Step 1: Add a reusable Nix availability helper in `nix.rs`**

```rust
pub fn detect_nix_cli() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("nix"))
            .find(|candidate| candidate.is_file())
    })
}
```

If a helper already exists with compatible behavior, reuse it instead of adding a duplicate.

- [ ] **Step 2: Add a focused runtime prerequisite error in `lib.rs`**

```rust
fn ensure_nix_cli_available() -> Result<()> {
    if crate::nix::detect_nix_cli().is_some() {
        return Ok(());
    }

    anyhow::bail!(
        "nixo requires the Nix CLI to be installed on the host. Install Nix first, then rerun this command. See README.md for the supported install flow."
    );
}
```

- [ ] **Step 3: Call the prerequisite check only on command paths that require Nix**

```rust
match cli.command {
    Commands::Catalog(args) => {
        ensure_nix_cli_available()?;
        cmd_catalog(args)
    }
    Commands::Create(args) => {
        ensure_nix_cli_available()?;
        cmd_create(args)
    }
    Commands::Build(args) => {
        ensure_nix_cli_available()?;
        cmd_build(args)
    }
    // leave purely local commands alone if they do not shell out to nix
    _ => { /* existing dispatch */ }
}
```

Use the actual command enum names and existing dispatch structure in `lib.rs`.

- [ ] **Step 4: Add or update a unit test for the missing-Nix error path**

```rust
#[test]
fn nix_required_commands_fail_with_actionable_message_when_nix_missing() {
    // set PATH to an empty temp dir
    // invoke the prerequisite helper
    // assert the error contains "requires the Nix CLI"
}
```

Place the test near existing `lib.rs` tests or create a small targeted test module if none exists.

- [ ] **Step 5: Run targeted Rust tests**

Run: `cargo test`
Expected: all tests pass, including the new missing-Nix test

- [ ] **Step 6: Commit**

```bash
git add crates/nixosandbox/src/lib.rs crates/nixosandbox/src/nix.rs
git commit -m "feat: add nix runtime prerequisite check"
```

### Task 4: Automate Formula Sync In Release Workflow

**Files:**
- Modify: `.github/workflows/release.yml`
- Test: `.github/workflows/release.yml`, `scripts/update_homebrew_formula.sh`

- [ ] **Step 1: Add job permissions needed for commit-back behavior**

```yaml
permissions:
  contents: write
```

Keep the existing top-level permission if it already satisfies this requirement.

- [ ] **Step 2: Add a follow-up job after release publication**

```yaml
  sync-homebrew-formula:
    name: Sync Homebrew formula
    needs: release
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v6
        with:
          ref: master
      - name: Read release asset digests
        run: |
          set -euo pipefail
          gh release view "${GITHUB_REF_NAME}" --repo "$GITHUB_REPOSITORY" --json assets > release.json
      - name: Update formula
        run: |
          set -euo pipefail
          version="${GITHUB_REF_NAME#v}"
          mac_arm=$(jq -r '.assets[] | select(.name == "nixo-aarch64-apple-darwin.tar.gz") | .digest' release.json | sed 's/^sha256://')
          mac_x86=$(jq -r '.assets[] | select(.name == "nixo-x86_64-apple-darwin.tar.gz") | .digest' release.json | sed 's/^sha256://')
          linux_x86=$(jq -r '.assets[] | select(.name == "nixo-x86_64-unknown-linux-gnu.tar.gz") | .digest' release.json | sed 's/^sha256://')
          scripts/update_homebrew_formula.sh "$version" "$mac_arm" "$mac_x86" "$linux_x86"
      - name: Validate formula
        run: ruby -c Formula/nixo.rb
      - name: Commit formula update
        run: |
          set -euo pipefail
          git config user.name github-actions
          git config user.email github-actions@github.com
          git add Formula/nixo.rb
          git diff --cached --quiet && exit 0
          git commit -m "build: sync homebrew formula for ${GITHUB_REF_NAME}"
          git push origin HEAD:master
```

Use `GH_TOKEN: ${{ github.token }}` or equivalent env wiring for the `gh release view` step.

- [ ] **Step 3: Fail fast if any expected digest is missing**

```bash
for value in "$mac_arm" "$mac_x86" "$linux_x86"; do
  [[ "$value" =~ ^[0-9a-f]{64}$ ]] || {
    echo "missing or invalid release digest" >&2
    exit 1
  }
done
```

- [ ] **Step 4: Validate workflow YAML locally**

Run: `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release.yml"); puts "YAML OK"'`
Expected: `YAML OK`

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml scripts/update_homebrew_formula.sh
git commit -m "ci: sync homebrew formula after release"
```

### Task 5: Update User And Agent Documentation

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update README install instructions to show the two-step prerequisite flow**

```md
### Homebrew

1. Install Nix on the host.
2. Install `nixo` from the tap:

    brew tap HashWarlock/nixo
    brew install nixo
```

Also explain that the formula is synced automatically after tagged releases.

- [ ] **Step 2: Document runtime prerequisite behavior in README**

```md
If `nix` is not installed, `nixo` will fail with a focused runtime error explaining that the host Nix CLI is required.
```

- [ ] **Step 3: Update `AGENTS.md` release and install guidance**

Add concise guidance that:
- the repo is its own tap
- formula release metadata is synced by the release workflow
- Homebrew install does not install Nix for the user

- [ ] **Step 4: Update `CLAUDE.md` with the same release/install guidance**

Mirror the `AGENTS.md` changes so both agent guidance files stay aligned.

- [ ] **Step 5: Commit**

```bash
git add README.md AGENTS.md CLAUDE.md
git commit -m "docs: document homebrew sync and nix prerequisite"
```

### Task 6: Final Verification And Release Rehearsal

**Files:**
- Test: `Formula/nixo.rb`
- Test: `.github/workflows/release.yml`
- Test: `crates/nixosandbox`

- [ ] **Step 1: Run full Rust test suite**

Run: `cd crates/nixosandbox && cargo test`
Expected: all tests pass

- [ ] **Step 2: Verify formula syntax**

Run: `ruby -c Formula/nixo.rb`
Expected: `Syntax OK`

- [ ] **Step 3: Verify workflow YAML syntax**

Run: `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release.yml"); puts "YAML OK"'`
Expected: `YAML OK`

- [ ] **Step 4: Dry-run the formula update script against current release values**

Run: `scripts/update_homebrew_formula.sh 0.1.1 52e0a8482a4528832a5b95a754f57f818524b4d5fa1c738f3372fc4b6f269879 8e484b841373c619c27d0de72201c6acb81adc3a828ce54dd734ea1a7007c736 d4a51cb17981e947bdfd25bdf126db7e22c2d5bd5058db1caa6819d59555272a && git diff --exit-code Formula/nixo.rb`
Expected: exit 0 with no formula drift for the current release metadata

- [ ] **Step 5: Perform manual tap install validation**

Run:

```bash
brew untap HashWarlock/nixo 2>/dev/null || true
brew tap HashWarlock/nixo /absolute/path/to/repo --custom-remote
brew install HashWarlock/nixo/nixo
brew test nixo
nixo --help
nixosandbox --help
```

Expected:
- install succeeds without a Homebrew `nix` dependency
- `brew test nixo` passes
- both CLI names print help output

- [ ] **Step 6: Confirm missing-Nix behavior if Nix is absent in the test environment**

Run a Nix-dependent command with `PATH` excluding `nix`, for example:

```bash
env PATH="/usr/bin:/bin" nixo catalog
```

Expected: command fails with an actionable message containing `requires the Nix CLI`.

- [ ] **Step 7: Commit any final follow-up fixes from verification**

```bash
git add .
git commit -m "test: finalize homebrew release sync validation"
```
