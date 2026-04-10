# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and test commands

### Rust CLI
```bash
cd crates/nixosandbox
cargo build                          # build
cargo test                           # run all 20 tests
cargo test session::tests            # run tests in one module
cargo test metadata_roundtrip        # run a single test by name
```

### TypeScript extension
```bash
cd packages/pi-sandbox-extension
npm install
npx tsc --noEmit                     # typecheck only
npm run build                        # compile to dist/
```

### Nix
```bash
nix flake check --accept-flake-config
nix build --accept-flake-config .#nixosandbox          # build CLI as Nix package
nix eval --accept-flake-config .#catalog.agents --apply 'x: builtins.attrNames x'
nix eval --accept-flake-config .#catalog.tools --apply 'x: builtins.attrNames x'
```

### CLI smoke test (Linux only, requires bwrap)
```bash
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo catalog --json --grouped
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo create --with bash,coreutils --network off --json
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo exec <session-id> -- echo hello
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixosandbox --help  # compatibility alias
NIXOSANDBOX_FLAKE_ROOT=$PWD ./result/bin/nixo destroy <session-id>
```

## Architecture

### Core data flow

1. **User runs** `nixo create --with claude-code,bash --network off`
2. **nix.rs** resolves package names via `build_with_catalog()` which generates a Nix expression calling `mkAgentSandbox`
3. **mkAgentSandbox.nix** resolves names from the catalog (agents first, then tools) and delegates to `mkSandboxRootfs`
4. **mkSandboxRootfs.nix** uses `pkgs.buildEnv` to merge packages, then creates a rootfs directory with symlinks into `/nix/store`
5. **session.rs** creates a session directory under `~/.local/share/nixosandbox/sessions/<id>/` with metadata, workspace, home, and cache dirs
6. **User runs** `nixo exec <id> -- command`
7. **plan_builder.rs** constructs bwrap argv: `--ro-bind <rootfs> /`, `--ro-bind /nix/store /nix/store`, writable bind mounts for workspace/home/cache, namespace flags, env vars
8. **lib.rs** spawns bwrap (detected path from `bubblewrap::detect()`) with the constructed argv

### Key design decisions

- **Primary CLI name is nixo**: `src/bin/nixo.rs` is the canonical entrypoint, and `src/bin/nixosandbox.rs` invokes the same app for compatibility.
- **Profile field is overloaded**: `session.profile` stores either a built-in profile name (e.g., `"strict"`) which maps to `nix/profiles/<name>.json`, or `"custom:<pkg1>,<pkg2>"` for `--with` sessions. Code must check `meta.profile.starts_with("custom:")` before calling `load_profile()`.
- **Rootfs symlinks require Nix store**: The rootfs contains absolute symlinks into `/nix/store`. bwrap must bind-mount `/nix/store` read-only, and the rootfs must have `/nix/store` as an empty mount point directory.
- **macOS support via Docker sidecar**: On non-Linux, `bubblewrap::detect()` tries a Docker sidecar container (`nixosandbox-sidecar`) with bwrap inside. Session paths are rewritten from host to container paths via `docker::rewrite_path()`.
- **Package name validation**: `nix::validate_package_name()` rejects names not matching `[a-zA-Z0-9_.-]+` to prevent Nix expression injection via `--with`.
- **Network mode storage**: For `--with` sessions, the network mode is stored in `session.network` (not derivable from a profile file). For built-in profiles, it's in the profile JSON.

### Rust module responsibilities

| Module | Owns |
|--------|------|
| `cli.rs` | clap argument parsing |
| `lib.rs` | command dispatch, `cmd_create`, `cmd_exec`, `cmd_catalog`, etc. |
| `session.rs` | session CRUD, metadata serialization, directory layout |
| `nix.rs` | `find_flake_root()`, `build_profile()`, `build_with_catalog()`, `query_catalog()` (filters non-derivation attrs via `filterDrvs` before reading `.meta.description`) |
| `plan_builder.rs` | bwrap argv construction (`--ro-bind`, `--bind`, `--unshare-*`, `--setenv`) |
| `bubblewrap.rs` | bwrap detection: `NIXOSANDBOX_BWRAP_PATH` env var, then `which bwrap`, Docker fallback on macOS |
| `docker.rs` | Docker sidecar lifecycle (find, start, create, image build) |
| `spec.rs` | profile/spec loading from JSON, validation |

### Nix module responsibilities

| File | Owns |
|------|------|
| `nix/catalog.nix` | Unified `{ agents, tools }` attrset — agents is a full dynamic passthrough of `llm-agents-pkgs` (no whitelist), tools from nixpkgs |
| `nix/mkSandboxRootfs.nix` | Builds rootfs directory tree from package list (symlinks, /etc, certs) |
| `nix/mkAgentSandbox.nix` | Resolves catalog names to packages, delegates to mkSandboxRootfs |
| `nix/profiles/*.json` | Built-in profile specs (strict, build-install, offline-review, debug-network) |

### TypeScript extension (packages/pi-sandbox-extension)

Provides Pi coding agent integration. Key files:
- `cli-client.ts` — spawns the `nixo` CLI as a subprocess (`nixosandbox` remains a compatibility alias), provides `createSession()`, `execCommand()`, `catalogPackages()`
- `extension.ts` — registers Pi tools: `sandboxRun`, `sandboxReadFile`, `sandboxWriteFile`, `sandboxListFiles`, `sandboxSessionInfo`, `sandboxBrowser`, `sandboxCatalog`
- `contract.ts` — TypeScript type definitions for the NDJSON protocol

## Session tests use env var serialization

Tests in `session.rs` mutate `NIXOSANDBOX_DATA_DIR` to use temp directories. They serialize via `static ENV_LOCK: Mutex<()>` acquired in `with_temp_data_dir()`. If adding session tests, always use this helper.

## CI runs on nixos-25.11

The flake pins `nixpkgs` to `nixos-25.11`. CI uses `cachix/install-nix-action@v30` with the numtide binary cache. Agent smoke tests use system apt bwrap (setuid) because Nix-built bwrap lacks setuid and fails on Ubuntu 24.04's AppArmor user namespace restrictions.
