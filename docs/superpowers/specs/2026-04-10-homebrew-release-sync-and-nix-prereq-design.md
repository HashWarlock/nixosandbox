# Homebrew Release Sync And Nix Prerequisite Design

## Context

The repository now acts as its own Homebrew tap at `HashWarlock/nixo` with the formula stored at `Formula/nixo.rb`. The current release workflow publishes versioned tarballs successfully, but the Homebrew formula still requires manual edits for every release because it hardcodes both the version and per-platform `sha256` values.

A tap install test also exposed a second issue: the formula currently declares `depends_on "nix"`, but that dependency is not reliably satisfiable through Homebrew in this environment. That makes `brew install` fail before users even reach the CLI, even though the product requirement is really "a working Nix runtime must exist on the host", not "Homebrew must install a formula named `nix`".

This design addresses both problems together:

1. Keep `Formula/nixo.rb` synchronized automatically with each tagged release.
2. Remove the incorrect Homebrew dependency model for Nix and replace it with explicit runtime prerequisite handling.

## Goals

- Eliminate manual version and checksum edits to `Formula/nixo.rb` for each release.
- Keep the repository itself as the tap: `brew tap HashWarlock/nixo && brew install nixo`.
- Make `brew install` succeed without depending on a Homebrew `nix` formula.
- Fail clearly at runtime when `nix` is missing, with actionable guidance.
- Preserve the current release artifact layout and naming.

## Non-Goals

- Changing the release artifact naming scheme.
- Changing the bundled `flake/` runtime asset strategy.
- Adding bottles to `homebrew/core`.
- Reworking the CLI command set or packaging format.

## Approach Options

### Option 1: Release Workflow Self-Updates Formula After Publishing Assets

After the release job uploads assets and creates the GitHub release, a follow-up job reads the published asset digests, rewrites `Formula/nixo.rb`, and commits the updated formula back to `master`.

Pros:
- Keeps the formula in sync with the actual published release.
- Removes manual release bookkeeping.
- Keeps tap maintenance in a single repo.

Cons:
- Workflow needs permission to commit back to `master`.
- Release pipeline becomes slightly more complex.

### Option 2: Generate Formula From Template During Release

Store a template plus a rendering script, then generate `Formula/nixo.rb` during release and commit the rendered result.

Pros:
- Separates static formula structure from release metadata.

Cons:
- Adds unnecessary indirection for a small file.
- Still requires a commit-back flow.

### Option 3: Keep Manual Formula Updates

Continue updating version and checksums by hand after each release.

Pros:
- Minimal automation.

Cons:
- Error-prone.
- Delays publishability of the tap after every tag.
- Repeats the exact failure mode already observed.

## Recommendation

Adopt Option 1.

The repository already owns both release publishing and tap contents, so the cleanest model is: publish assets first, then update the formula from the definitive release metadata.

## Design

### 1. Formula Sync Pipeline

Extend `.github/workflows/release.yml` with a follow-up job after the GitHub release is published.

That job will:
- check out `master`
- read the release asset metadata for the current tag
- extract the tarball digests for:
  - `nixo-aarch64-apple-darwin.tar.gz`
  - `nixo-x86_64-apple-darwin.tar.gz`
  - `nixo-x86_64-unknown-linux-gnu.tar.gz`
- update `Formula/nixo.rb`
- validate the formula file
- commit and push the change back to `master`

The release remains tag-driven. The formula update is a post-release synchronization step, not a pre-release rendering step.

### 2. Formula Update Script

Add a small script at `scripts/update_homebrew_formula.sh`.

Inputs:
- version without the leading `v`
- macOS arm64 sha256
- macOS x86_64 sha256
- Linux x86_64 sha256

Responsibilities:
- update only the `version` line in `Formula/nixo.rb`
- update only the three `sha256` lines
- fail if expected lines are missing or duplicated
- make no unrelated edits

The script should be deterministic and idempotent for the same inputs.

### 3. Nix Runtime Prerequisite Model

Remove `depends_on "nix"` from `Formula/nixo.rb`.

Reasoning:
- the product requirement is a host Nix runtime, not a Homebrew-managed Nix dependency
- `brew install` should not fail simply because Homebrew cannot satisfy a formula named `nix`
- users may already have Nix installed via the official installer or another supported path

Replace that dependency model with explicit runtime handling:
- add a startup check before CLI paths that shell out to `nix`
- if `nix` is unavailable, return a focused error explaining that `nixo` requires the Nix CLI on the host
- include the docs-directed installation guidance in the error text

Commands that do not require `nix` may continue to run if the implementation naturally allows that, but the design assumes most meaningful sandbox operations depend on Nix.

### 4. README And Formula Messaging

Update README install docs to present the setup as two explicit steps:

1. Install Nix on the host.
2. Install `nixo` from the Homebrew tap.

`Formula/nixo.rb` should remain lightweight:
- install the packaged binaries
- install bundled `flake/` assets
- expose `nixo`
- symlink `nixosandbox`

The formula test should verify packaging and invocation only, not live Nix execution.

### 5. Workflow Failure Behavior

If any expected release asset or digest is missing:
- fail the formula-sync job
- do not push a partially updated formula
- leave the published release intact
- make the failure obvious in Actions output

This preserves release artifacts while preventing a mismatched tap formula from being committed.

## Data Flow

1. Maintainer pushes tag `vX.Y.Z`.
2. Release workflow builds three tarballs and matching checksum sidecar files.
3. Release workflow publishes the GitHub release.
4. Formula-sync job fetches the release metadata for `vX.Y.Z`.
5. Formula-sync job extracts the three tarball digests.
6. `scripts/update_homebrew_formula.sh` rewrites `Formula/nixo.rb` to `version "X.Y.Z"` plus new checksums.
7. Workflow validates `Formula/nixo.rb`.
8. Workflow commits and pushes the formula update to `master`.
9. Users install via `brew tap HashWarlock/nixo && brew install nixo` and get a formula that matches the latest published release.

## Verification Plan

### Automated

- `ruby -c Formula/nixo.rb`
- script-level failure if version or checksum placeholders cannot be updated safely
- workflow failure if expected assets are absent from the release

### Manual Release Validation

After the first automated formula-sync lands:
- `brew untap HashWarlock/nixo && brew tap HashWarlock/nixo`
- `brew install nixo`
- `brew test nixo`
- `nixo --help`
- `nixosandbox --help`
- run one command path that intentionally exercises the missing-Nix guidance if Nix is absent, or a basic successful path if Nix is present

## Risks And Mitigations

### Risk: Workflow Commit Loop

A workflow that commits to `master` could retrigger unrelated workflows.

Mitigation:
- keep the formula-sync commit message stable and narrow
- limit release workflow trigger to tags only, which it already does
- avoid any `push` trigger on `master` for the release workflow

### Risk: Release Published But Formula Sync Fails

This can still leave the tap behind the release.

Mitigation:
- surface the failure clearly in the release workflow
- keep the update script deterministic and small
- allow maintainers to rerun only the failed job after fixing workflow or permissions issues

### Risk: Missing Nix Confuses Users After Install

Without a Homebrew dependency, users may install `nixo` successfully and only discover later that Nix is missing.

Mitigation:
- make README installation explicit
- add focused runtime error messaging at first actual Nix use
- keep formula test limited to packaging so Homebrew install remains stable

## Files Expected To Change

- `.github/workflows/release.yml`
- `Formula/nixo.rb`
- `scripts/update_homebrew_formula.sh`
- Rust CLI startup path for Nix prerequisite checking
- `README.md`
- `AGENTS.md`
- `CLAUDE.md`

## Success Criteria

- A new version tag publishes release assets and then updates `Formula/nixo.rb` automatically.
- `Formula/nixo.rb` on `master` always matches the latest published release version and checksums.
- `brew install nixo` no longer fails because of `depends_on "nix"`.
- Missing Nix is reported as a runtime prerequisite error with actionable guidance.
- Tap installation docs match the actual supported installation flow.
