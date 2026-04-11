# Nix Flake Runtime — Part B Design Spec

## Overview

Part B of the nixosandbox Nix flake runtime redesign. Part A (complete) built the standalone CLI, Nix flake with mkSandboxRootfs, session management, and pivot-root bwrap execution. Part B delivers Docker sidecar support for macOS, legacy cleanup, and integration tests.

## Goals

1. **Docker sidecar with `/nix/store` mount** — macOS users get real bwrap sandboxing via Docker with Nix rootfs, no packages pre-installed in the container.
2. **Legacy cleanup** — Delete `legacy-ndjson` subcommand and all legacy NDJSON protocol inbound types. Enhance `exec --json` to emit full event stream.
3. **Integration tests** — Two independently-gated suites: Linux native (Nix + bwrap) and macOS Docker (Nix + Docker sidecar). Port valuable legacy tests, write new rootfs pipeline tests.

## Out of Scope

- Pi extension simplification (Part C)
- Network allowlist enforcement
- Browser tool
- Nix search fallback for package resolution

---

## 1. Docker Sidecar Changes

### 1.1 Dockerfile

Strip `docker/pi-sandbox-sidecar.Dockerfile` to bare minimum. Rename to `docker/nixosandbox-sidecar.Dockerfile`:

```dockerfile
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    bubblewrap iptables \
    && rm -rf /var/lib/apt/lists/*
```

All runtime packages come from the Nix rootfs via `--pivot-root`. The Dockerfile only provides bwrap (the sandbox primitive) and iptables (for future network enforcement).

### 1.2 Container Creation

`create_sidecar()` in `docker.rs` adds the Nix store volume mount:

```
docker run -d --name nixosandbox-sidecar \
  --cap-add SYS_ADMIN --cap-add NET_ADMIN \
  --security-opt seccomp=unconfined \
  -v /nix/store:/nix/store:ro \
  -v <sessions_dir>:/nixosandbox/sessions:rw \
  <image> sleep infinity
```

Changes from current:
- Add `-v /nix/store:/nix/store:ro` — makes host Nix closures available inside container
- Container sessions dir changes from `/pi-sandbox` to `/nixosandbox/sessions` (matches new naming)
- Sidecar container name changes from `pi-sandbox-sidecar` to `nixosandbox-sidecar`
- Image name changes from `pi-sandbox-base:latest` to `nixosandbox-sidecar:latest`

### 1.3 Path Rewriting Strategy

Nix store paths are absolute and identical on host and container (`/nix/store/abc123-sandbox-strict`), so **rootfs paths need no rewriting**.

Only session directory paths need translation:
- Host: `~/.local/share/nixosandbox/sessions/<id>/workspace`
- Container: `/nixosandbox/sessions/<id>/workspace`

The existing `rewrite_path()` function in `docker.rs` handles this. Update it to use the new container sessions path.

### 1.4 cmd_exec Docker Execution

Replace the warning in `cmd_exec`'s Docker branch with real execution:

1. Rewrite session directory paths (workspace, home, cache) from host to container paths using `docker.rs::rewrite_path()`
2. Build bwrap argv with `plan_builder::build_rootfs()` using the original Nix store rootfs path (no rewriting needed)
3. Construct command: `docker exec -i <container_id> bwrap <rewritten_argv>`
4. Spawn and handle output (inherit stdio for interactive, pipe for --json)

---

## 2. Legacy Cleanup

### 2.1 Delete

| Item | Location |
|------|----------|
| `LegacyNdjson` variant | `crates/nixosandbox/src/cli.rs` |
| `legacy_ndjson_main()` | `crates/nixosandbox/src/main.rs` |
| `InboundMessage` enum (Plan/Cancel types) | `crates/nixosandbox/src/contract.rs` |
| `ValidationEnvelope`, `ValidationPayload` | `crates/nixosandbox/src/contract.rs` (inbound-only types) |
| Legacy protocol tests | `tests/protocol/version-mismatch.test.ts`, `validation-failure.test.ts`, `degraded-allowlist.test.ts`, `network-observation.test.ts`, `allowlist-enforced.test.ts` |

### 2.2 Keep

| Item | Reason |
|------|--------|
| `ResultPayload`, `ResultEnvelope` types | Used by `exec --json` output |
| Streamed event types (stdout, stderr, lifecycle) | Used by `exec --json` output |
| `emit()` function | Used by `exec --json` to write NDJSON |
| `contract.ts` in Pi extension | Untouched (Part C) |
| `supervisor.rs` | Still used by `exec --json` for process supervision |
| `validator.rs` | Still used for plan validation in create/exec |
| `observer.rs` | Still used for network observation |

### 2.3 Enhance exec --json

The current `cmd_exec` JSON mode emits basic stdout events and a result. Enhance to emit the full event stream:

- `lifecycle` event with `stage: "started"` when bwrap spawns
- `stdout` events (already present)
- `stderr` events (add — pipe stderr and stream as separate events)
- `lifecycle` event with `stage: "exited"` before result
- `result` with full payload: exitCode, signal, timedOut, durationMs

Events use the existing `contract.rs` types and `emit()` function. Sequence numbers are strictly increasing across all event types.

### 2.4 Supervisor Reuse

The existing `supervisor::supervise()` function handles process spawning, NDJSON event streaming, cancel handling, and timeout logic. For `cmd_exec --json`, extract the bwrap spawning and NDJSON streaming into a shared function that both the new CLI path and any future callers can use. The supervisor builds a `Command`, spawns it, pipes stdout/stderr, emits events via `emit()`, and returns a `SuperviseResult`. The key change: instead of receiving a `PlanPayload` (legacy protocol type), the supervisor accepts a pre-built `Command` and configuration struct.

### 2.5 Dead Code Cleanup

After removing legacy inbound types, scan for unreferenced code in:
- `contract.rs` — Remove unused inbound message types, keep outbound types
- Any functions only called from `legacy_ndjson_main()`

---

## 3. Integration Tests

### 3.1 Directory Structure

```
tests/
  integration/
    vitest.config.ts
    globalSetup.ts        — cargo build --release, set NIXOSANDBOX_BINARY
    helpers.ts            — CLI wrapper functions
    rootfs-pipeline.test.ts   — Linux native (RUN_INTEGRATION_TESTS=1)
    docker-rootfs.test.ts     — Docker (RUN_DOCKER_TESTS=1)
  protocol/
    vitest.config.ts      — updated
    globalSetup.ts        — updated: cargo build, set binary path
    helpers.ts            — adapted: spawn exec --json, parse NDJSON
    cancel-flow.test.ts   — adapted from legacy
    crash-synthesis.test.ts   — adapted from legacy
    docker-sidecar.test.ts    — adapted for rootfs execution
```

### 3.2 Integration Test Helpers

`tests/integration/helpers.ts` provides CLI wrapper functions:

- `build(args)` — Spawns `nixosandbox build` with args, returns stdout and exit code
- `create(args)` — Spawns `nixosandbox create` with args, parses session ID or JSON
- `execCmd(sessionId, command, opts)` — Spawns `nixosandbox exec`, handles --json and --env
- `list(opts)` — Spawns `nixosandbox list`, parses table or JSON
- `destroy(sessionId)` — Spawns `nixosandbox destroy`, returns exit code

Each function uses `execFile` (not shell execution) to spawn the binary safely.

### 3.3 rootfs-pipeline.test.ts

Gated: `RUN_INTEGRATION_TESTS=1` (requires Nix + bwrap on Linux).

Tests:
1. **build strict profile** — `nixosandbox build --profile strict --json` returns a valid Nix store path
2. **create session** — `nixosandbox create --profile strict --json` returns session ID and metadata
3. **exec echo** — `nixosandbox exec <id> -- echo hello` prints "hello", exits 0
4. **exec verify rootfs** — `nixosandbox exec <id> -- ls /` shows sandbox dirs (bin, etc, workspace), not host dirs
5. **exec verify sandbox user** — `nixosandbox exec <id> -- cat /etc/passwd` contains "sandbox" user
6. **exec json mode** — `nixosandbox exec --json <id> -- echo test` produces NDJSON with lifecycle, stdout, result events
7. **list sessions** — `nixosandbox list --json` shows the session
8. **destroy session** — `nixosandbox destroy <id>` succeeds, session no longer in list

### 3.4 docker-rootfs.test.ts

Gated: `RUN_DOCKER_TESTS=1` (requires Nix + Docker).

Tests:
1. **create + exec through Docker** — Same as rootfs-pipeline tests 2-5 but on macOS with Docker sidecar
2. **verify Nix store accessible** — `nixosandbox exec <id> -- ls /nix/store` succeeds (store is mounted)
3. **verify isolation backend** — JSON mode reports `isolationBackend: "docker"` in events

### 3.5 Adapted Protocol Tests

`tests/protocol/cancel-flow.test.ts`:
- Spawn `nixosandbox exec --json <session_id> -- sleep 60`
- Send SIGTERM to the process
- Verify lifecycle events and result with terminal state

`tests/protocol/crash-synthesis.test.ts`:
- Spawn `nixosandbox exec --json <session_id> -- <command>`
- Kill the nixosandbox process (SIGKILL) mid-execution
- From the test's perspective, verify the process died without a result event
- (Crash synthesis responsibility moves to the consumer — if no result before exit, the consumer synthesizes)

`tests/protocol/docker-sidecar.test.ts`:
- Rewrite to test rootfs execution through Docker
- Verify isolation backend, network enforcement, Nix store access

### 3.6 Test Gating

| Env Var | Suite | Requires |
|---------|-------|----------|
| `RUN_INTEGRATION_TESTS=1` | rootfs-pipeline | Nix, bwrap, Linux |
| `RUN_DOCKER_TESTS=1` | docker-rootfs, docker-sidecar | Nix, Docker |
| (neither) | cancel-flow, crash-synthesis | Just the binary |

---

## 4. Naming Alignment

Part A left some `pi-sandbox` naming artifacts. Part B cleans up:

| Old | New | Location |
|-----|-----|----------|
| `pi-sandbox-sidecar` | `nixosandbox-sidecar` | Container name in docker.rs |
| `pi-sandbox-base:latest` | `nixosandbox-sidecar:latest` | Image name in docker.rs |
| `/pi-sandbox` | `/nixosandbox/sessions` | Container sessions dir in docker.rs |
| `PI_SANDBOX_NO_DOCKER` | `NIXOSANDBOX_NO_DOCKER` | Env var in bubblewrap.rs |
| `PI_SANDBOX_BWRAP_PATH` | `NIXOSANDBOX_BWRAP_PATH` | Env var in bubblewrap.rs |
| `pi-sandbox-sidecar.Dockerfile` | `nixosandbox-sidecar.Dockerfile` | Dockerfile name |
| `RUNTIME_BINARY_PATH` | `NIXOSANDBOX_BINARY` | Test env var |

---

## 5. Files Changed

### Modified
- `crates/nixosandbox/src/docker.rs` — Nix store mount, new naming, updated container paths
- `crates/nixosandbox/src/bubblewrap.rs` — Rename env vars
- `crates/nixosandbox/src/main.rs` — Delete legacy, wire Docker exec, enhance --json
- `crates/nixosandbox/src/cli.rs` — Remove LegacyNdjson
- `crates/nixosandbox/src/contract.rs` — Remove inbound types, keep outbound

### Deleted
- `docker/pi-sandbox-sidecar.Dockerfile` (replaced by renamed version)
- `tests/protocol/version-mismatch.test.ts`
- `tests/protocol/validation-failure.test.ts`
- `tests/protocol/degraded-allowlist.test.ts`
- `tests/protocol/network-observation.test.ts`
- `tests/protocol/allowlist-enforced.test.ts`

### Created
- `docker/nixosandbox-sidecar.Dockerfile`
- `tests/integration/vitest.config.ts`
- `tests/integration/globalSetup.ts`
- `tests/integration/helpers.ts`
- `tests/integration/rootfs-pipeline.test.ts`
- `tests/integration/docker-rootfs.test.ts`

### Adapted
- `tests/protocol/globalSetup.ts`
- `tests/protocol/helpers.ts`
- `tests/protocol/cancel-flow.test.ts`
- `tests/protocol/crash-synthesis.test.ts`
- `tests/protocol/docker-sidecar.test.ts`

### Untouched
- `flake.nix`, `nix/` — Nix flake and profiles
- `packages/pi-sandbox-extension/` — Deferred to Part C
- `crates/nixosandbox/src/spec.rs` — Sandbox spec types
- `crates/nixosandbox/src/session.rs` — Session management
- `crates/nixosandbox/src/nix.rs` — Nix build invocation
- `crates/nixosandbox/src/plan_builder.rs` — bwrap argv construction
- `crates/nixosandbox/src/supervisor.rs` — Process supervision
- `crates/nixosandbox/src/observer.rs` — Network observation
- `crates/nixosandbox/src/timestamps.rs` — Timestamp helper
