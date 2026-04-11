# Docker Fallback for macOS — Design Spec

**Date:** 2026-04-08
**Branch:** `pi-sandbox-refactor`
**Prerequisite:** Phases 0-12 complete (tag `v1-phases-11-12-complete`)

---

## Overview

On macOS, bwrap is unavailable because it depends on Linux kernel namespaces (`unshare(2)`). Today, the runtime degrades to running commands directly on the host with no isolation. This spec adds a Docker-based fallback: when macOS is detected and Docker Desktop is available, the runtime starts a lightweight Linux sidecar container and runs bwrap inside it, giving macOS users real kernel-level isolation.

---

## Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Container lifecycle | Hybrid sidecar — long-running container, bwrap isolates per-execution | Container is just a Linux kernel host; bwrap provides per-execution isolation. Fast startup (~50ms per exec). |
| Image strategy | Minimal debian-slim base + Rust binary mounted in | No cross-compile toolchain needed on host. Image is small (~150-200MB). Binary updates are instant. |
| Host↔container IPC | `docker exec -i` into sidecar, NDJSON over stdio | Same protocol as native bwrap. RuntimeClient barely changes. |
| Workspace mounting | Broad mount of pi-sandbox sessions directory | Sessions dir is sandbox-managed. Avoids per-session container restarts. |
| Base image contents | Debian-slim + bwrap + iptables + python3 + node + git + curl | Matches build-install profile expectations. Covers common sandboxed commands. |
| Missing Docker | Silent degradation with `DOCKER_NOT_AVAILABLE` warning + `PI_SANDBOX_NO_DOCKER=1` opt-out | Protocol already supports degradation warnings. Opt-out for CI or users who prefer no Docker. |

---

## Detection Chain

The detection order in `bubblewrap.rs` becomes:

1. `PI_SANDBOX_NO_DOCKER=1` set? → skip Docker, go to step 4
2. Linux? → check for bwrap binary (existing logic) → `Available` or `Unavailable`
3. macOS? → check `docker info` succeeds
   - Yes → start/find sidecar container → `DockerAvailable { container_id, host_sessions_dir, container_sessions_dir }`
   - No → `Unavailable { "Docker not found" }` with `DOCKER_NOT_AVAILABLE` warning
4. Other platform → `Unavailable`

### BwrapAvailability Enum

```rust
pub enum BwrapAvailability {
    Available { path: PathBuf },
    DockerAvailable {
        container_id: String,
        host_sessions_dir: String,
        container_sessions_dir: String,
    },
    Unavailable { reason: String },
}
```

`DockerAvailable` carries the path mapping so the supervisor can rewrite host paths to container paths without additional lookups.

---

## Sidecar Container Lifecycle

### Container Name

`pi-sandbox-sidecar` — well-known name for detection and lifecycle management.

### Startup (lazy, on first detection)

```
detect() on macOS:
  1. docker ps --filter name=pi-sandbox-sidecar --format '{{.ID}}'
     → running? return DockerAvailable
  2. docker ps -a --filter name=pi-sandbox-sidecar --format '{{.ID}}'
     → exists but stopped? docker start pi-sandbox-sidecar → return DockerAvailable
  3. doesn't exist?
     a. Ensure image exists (docker images pi-sandbox-base:latest)
        → missing? docker build -t pi-sandbox-base:latest -f docker/pi-sandbox-sidecar.Dockerfile .
     b. Ensure Linux runtime binary exists
        → missing? docker run --rm -v <crate_src>:/src -v <output>:/out pi-sandbox-base:latest
          sh -c "apt-get update && apt-get install -y cargo && cd /src && cargo build --release && cp target/release/pi-sandbox-runtime /out/"
     c. docker run -d --name pi-sandbox-sidecar \
          --cap-add SYS_ADMIN --cap-add NET_ADMIN \
          -v <sessions_dir>:/pi-sandbox \
          -v <runtime_binary>:/usr/local/bin/pi-sandbox-runtime:ro \
          pi-sandbox-base:latest \
          sleep infinity
     → return DockerAvailable
```

### Shutdown

- **Extension shutdown** (`session_shutdown` event): `docker stop pi-sandbox-sidecar` with 10s grace period. Container is NOT removed — next session reuses it.
- **Manual cleanup**: `docker rm -f pi-sandbox-sidecar` to reclaim resources.

### Crash Recovery

If `docker exec` fails because the container died:
1. Detect failure (non-zero exit + specific error pattern)
2. Restart sidecar: `docker start pi-sandbox-sidecar` or recreate if container was removed
3. Retry the execution once
4. If retry fails, degrade to `Unavailable` and run naked with `DOCKER_SIDECAR_FAILED` warning

### Security: No `--privileged`

The sidecar uses `--cap-add SYS_ADMIN --cap-add NET_ADMIN` instead of `--privileged`. These are the minimum capabilities needed for:
- `SYS_ADMIN`: bwrap's `unshare(2)` calls (mount, pid, uts, ipc namespaces)
- `NET_ADMIN`: `--unshare-net` and iptables rule injection

---

## Supervisor Execution Path

The supervisor gains a third branch:

```rust
match bwrap {
    BwrapAvailability::Available { path } => {
        // Linux: existing bwrap path (unchanged)
    }
    BwrapAvailability::DockerAvailable { container_id, host_sessions_dir, container_sessions_dir } => {
        // macOS+Docker: rewrite paths, docker exec bwrap
        let rewritten_plan = rewrite_paths(&plan, &host_sessions_dir, &container_sessions_dir);
        let argv = plan_builder::build_with_allowlist(&rewritten_plan, effective_state, iptables_path);
        let mut cmd = Command::new("docker");
        cmd.args(["exec", "-i", &container_id, "bwrap"]);
        cmd.args(&argv);
        cmd
    }
    BwrapAvailability::Unavailable { .. } => {
        // No isolation: existing naked execution (unchanged)
    }
}
```

### Key properties

- **`plan_builder.rs` is unchanged.** It produces bwrap argv. The supervisor just prefixes it with `docker exec`.
- **`-i` flag** keeps stdin open for cancel messages over NDJSON.
- **No `-t` flag** — no TTY. We're piping structured data.
- **Observer works.** The Rust runtime (supervisor) runs on the macOS host. On macOS, the observer is a no-op (`#[cfg(not(target_os = "linux"))]`). This is acceptable because bwrap+iptables inside the container provides kernel-level enforcement — the observer is a safety net, not the enforcement mechanism.
- **iptables wrapper script** is written to the host temp dir, then bind-mounted into the container via the existing sessions dir mount (write it to `<sessions_dir>/tmp/`).

---

## Path Rewriting

Session manager creates paths like:
```
/Users/hashwarlock/.local/share/pi-sandbox/sessions/<id>/workspace
```

Sidecar mounts as:
```
-v /Users/hashwarlock/.local/share/pi-sandbox:/pi-sandbox
```

Inside the container:
```
/pi-sandbox/sessions/<id>/workspace
```

### Rewrite function

```rust
fn rewrite_host_to_container(
    host_path: &str,
    host_sessions_dir: &str,
    container_sessions_dir: &str,
) -> String {
    host_path.replacen(host_sessions_dir, container_sessions_dir, 1)
}
```

### Applied to

- `manifest.mounts[].source` — for directory/file bind mounts
- `manifest.cwd` — working directory
- iptables wrapper script path

### plan_builder stays untouched

The supervisor clones the plan, rewrites paths in the clone, passes the rewritten plan to plan_builder. The original plan is preserved for the result envelope.

---

## Docker Image

### Dockerfile: `docker/pi-sandbox-sidecar.Dockerfile`

```dockerfile
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    bubblewrap \
    iptables \
    python3 \
    nodejs \
    git \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
```

### Image build strategy

The sidecar detection code checks if `pi-sandbox-base:latest` exists locally. If not, it builds from the Dockerfile. This is a one-time ~30 second cost.

### Rust binary build strategy

The Rust binary must be a Linux binary (not macOS). Two-stage process:
1. Check for cached Linux binary at `<project>/target/docker-linux/pi-sandbox-runtime`
2. If missing, build inside Docker: mount the crate source into a container, run `cargo build --release`, copy the binary out to the cache path
3. Sidecar mounts the cached binary at `/usr/local/bin/pi-sandbox-runtime:ro`

Binary rebuilds happen when the user explicitly rebuilds or when the cached binary is missing. The binary update is instant for the sidecar — next `docker exec` picks it up.

---

## Effective States

The validator treats `DockerAvailable` identically to `Available`. Bwrap is genuinely available inside the container.

| Requested | Docker? | Actual | Enforcement | Degraded |
|-----------|---------|--------|-------------|----------|
| `off` | Yes | `off` | `enforced` | `false` |
| `full` | Yes | `full` | `observed` | `false` |
| `allowlist` | Yes + iptables | `allowlist` | `enforced` | `false` |
| Any | No Docker | (same as today's macOS behavior) | `best_effort`/`observed` | `true` |

### New field: `isolationBackend`

Added to `EffectiveState` for observability:

```rust
pub isolation_backend: String,  // "native" | "docker" | "none"
```

- `"native"` — Linux with bwrap directly
- `"docker"` — macOS with Docker sidecar + bwrap
- `"none"` — no isolation (naked execution)

Purely informational. Not a policy input.

### New warning codes

| Code | When |
|------|------|
| `DOCKER_NOT_AVAILABLE` | macOS, Docker not found, degrading to naked execution |
| `DOCKER_SIDECAR_RESTARTED` | Sidecar was dead, successfully restarted before execution |

---

## File Map

### New Files

| Path | Responsibility |
|------|----------------|
| `docker/pi-sandbox-sidecar.Dockerfile` | Debian-slim image with bwrap + iptables + common tools |
| `crates/pi-sandbox-runtime/src/docker.rs` | Docker detection, sidecar lifecycle, image build, binary build |

### Modified Files

| Path | Change |
|------|--------|
| `crates/pi-sandbox-runtime/src/bubblewrap.rs` | Add `DockerAvailable` variant, update `detect()` to try Docker on macOS |
| `crates/pi-sandbox-runtime/src/supervisor.rs` | Add `DockerAvailable` execution branch with path rewriting, sidecar crash recovery |
| `crates/pi-sandbox-runtime/src/validator.rs` | Treat `DockerAvailable` same as `Available` |
| `crates/pi-sandbox-runtime/src/contract.rs` | Add `isolation_backend` to `EffectiveState`, add warning codes |
| `crates/pi-sandbox-runtime/src/main.rs` | Add `mod docker;` |
| `packages/pi-sandbox-extension/src/contract.ts` | Add `isolationBackend` to EffectiveState schema, add warning codes |

### New Test Files

| Path | Responsibility |
|------|----------------|
| `tests/protocol/docker-sidecar.test.ts` | Docker sidecar lifecycle and execution tests (env-gated) |

---

## Testing

- **Path rewriting unit tests** — pure function, runs on any platform
- **Docker sidecar lifecycle tests** — gated behind `RUN_DOCKER_TESTS=1`. Tests start, detect, stop, restart-on-crash.
- **Docker execution integration test** — gated behind `RUN_DOCKER_TESTS=1`. Runs echo through Docker+bwrap, verifies `enforcement: "enforced"`, `isolationBackend: "docker"`.
- **Existing tests unchanged** — Linux native bwrap tests unaffected. macOS degradation tests without Docker still pass as before.

---

## What Is NOT in This Spec

- **Cross-compilation toolchain on macOS** — Uses Docker to build the Linux binary instead
- **Custom image registry / image publishing** — Image is built locally from Dockerfile
- **Container resource limits** — cgroups inside Docker deferred (same as native cgroups)
- **Docker Compose integration** — Sidecar is managed programmatically by `docker.rs`
- **Observer inside Docker** — Observer remains a macOS no-op. Enforcement is via bwrap+iptables inside the container.
- **Nix-based image** — Debian-slim keeps it simple. Nix image is an option for a future enhancement.

---

## Phase Gate

| Criteria |
|----------|
| Dockerfile builds successfully |
| Linux Rust binary builds inside Docker |
| Sidecar starts, stops, and recovers from crash |
| `sandbox_run` on macOS with Docker produces `enforcement: "enforced"` and `isolationBackend: "docker"` |
| `sandbox_run` on macOS without Docker still degrades gracefully (same as today + `DOCKER_NOT_AVAILABLE` warning) |
| `PI_SANDBOX_NO_DOCKER=1` skips Docker detection |
| All existing Linux and macOS tests continue to pass |
| Path rewriting unit tests pass on any platform |
