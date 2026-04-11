# Pi Sandbox Phases 8-10 Design Spec: Make the Runtime Real

**Date:** 2026-04-03
**Status:** Approved
**Prerequisite:** Phases 0-7 complete (tag `v1-protocol-passing`)
**Branch:** `pi-sandbox-refactor`

## Overview

Replace the stub execution and observation in the Pi Sandbox runtime with real Bubblewrap isolation and network observation. Validate with real-world build flows.

**What this spec covers:**
- Phase 8: Bubblewrap integration (real isolation on Linux, graceful macOS fallback)
- Phase 9: Integration tests with real build workflows (npm, Python, Rust)
- Phase 10: Network observation via `/proc/net/tcp` polling

**What this spec does NOT cover:**
- Legacy server deprecation (Phase 11)
- Browser, real allowlist enforcement, Nix runtime bases (Phase 12)

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Platform strategy | Linux bwrap + macOS fallback | Truthful reporting via effectiveState; dev on macOS, isolate on Linux |
| Bwrap argv construction | Dedicated `plan_builder.rs` | Testable pure function; bwrap flag logic separate from process lifecycle |
| Bwrap binary discovery | `PI_SANDBOX_BWRAP_PATH` env var, fallback `which bwrap` | Supports NixOS store paths; simple case stays simple |
| Network observation | Poll `/proc/net/tcp` at ~500ms | Pure Rust, no external deps, Linux-only (macOS returns empty) |
| Integration test strategy | Fixture repos + optional network smoke tests | Deterministic CI gate; optional real-network validation |

---

## Section 1: Bubblewrap Integration (Phase 8)

### Architecture

The supervisor currently runs commands directly:

```rust
Command::new(&plan.command[0]).args(&plan.command[1..])
```

Phase 8 replaces this with:

```rust
Command::new(bwrap_path).args(plan_builder::build(plan, effective_state))
```

On macOS or when bwrap is unavailable, the supervisor keeps direct execution and emits degraded warnings.

### New Files

#### `crates/pi-sandbox-runtime/src/bubblewrap.rs`

Bwrap binary discovery and platform detection.

Responsibilities:
- Check if running on Linux (`cfg(target_os = "linux")`)
- Resolve bwrap path: `PI_SANDBOX_BWRAP_PATH` env var first, then `which bwrap` on PATH
- Validate the resolved binary exists and is executable
- Expose `BwrapAvailability` enum: `Available { path: PathBuf }` or `Unavailable { reason: String }`
- Public function: `detect() -> BwrapAvailability`

On non-Linux platforms, `detect()` always returns `Unavailable { reason: "not Linux" }`.

#### `crates/pi-sandbox-runtime/src/plan_builder.rs`

Translates `PlanPayload` + `EffectiveState` into bwrap argv (`Vec<String>`).

Construction order:
1. **Mounts:** For each mount in `manifest.mounts`:
   - `type = "directory"`, `writable = false` → `--ro-bind <source> <target>`
   - `type = "directory"`, `writable = true` → `--bind <source> <target>`
   - `type = "file"`, `writable = false` → `--ro-bind <source> <target>`
   - `type = "file"`, `writable = true` → `--bind <source> <target>`
   - `type = "tmpfs"` → `--tmpfs <target>`
2. **Devices:** Hardcoded minimal set (not configurable in v1):
   - `--dev-bind /dev/null /dev/null`
   - `--dev-bind /dev/zero /dev/zero`
   - `--dev-bind /dev/urandom /dev/urandom`
   - `--dev-bind /dev/random /dev/random`
3. **Proc filesystem:** `--proc /proc`
4. **Namespaces:** For each namespace in `effective_state.namespaces_applied`:
   - `pid` → `--unshare-pid`
   - `ipc` → `--unshare-ipc`
   - `uts` → `--unshare-uts`
   - `net` → `--unshare-net` (only when `network.actual = "off"`)
   - `cgroup-try` → `--unshare-cgroup-try`
   - `user` → omitted (bwrap implicitly creates a user namespace when any other namespace is unshared; do not pass `--unshare-user` explicitly)
5. **Environment:** `--clearenv` then `--setenv KEY VALUE` for each entry in `manifest.env`
6. **Working directory:** `--chdir <manifest.cwd>`
7. **Command:** Appended last: `-- <command[0]> <command[1]> ...`

Public function: `build(plan: &PlanPayload, effective_state: &EffectiveState) -> Vec<String>`

This is a pure function with no side effects. Testable in isolation.

### Modified Files

#### `crates/pi-sandbox-runtime/src/contract.rs`

Extend `EffectiveState` to include fields that were in the v1 spec but not yet implemented:

```rust
pub struct EffectiveState {
    pub network: EffectiveNetwork,
    pub namespaces_applied: Vec<String>,
    pub env_applied: Vec<String>,
}
```

These fields are populated by the validator and reported in the validation message. Existing protocol tests that check `effectiveState` must be updated to expect these new fields (both will be present in every validation response where `effectiveState` is non-null).

#### `crates/pi-sandbox-runtime/src/validator.rs`

Changes:
- Resolve which namespaces can actually be applied based on platform and bwrap availability
- Populate `namespaces_applied` in `EffectiveState` (only namespaces that will actually be created)
- Populate `env_applied` in `EffectiveState` (keys from `manifest.env`, filtered by `env_allowlist` if set)
- Emit `NAMESPACE_DEGRADED` warning for each requested namespace that cannot be applied
- Accept bwrap availability as input (passed from main.rs)

#### `crates/pi-sandbox-runtime/src/supervisor.rs`

Changes:
- Accept bwrap availability as input
- When bwrap is available: call `plan_builder::build()` to get argv, spawn `Command::new(bwrap_path).args(argv)`
- When bwrap is unavailable: keep current direct execution (`Command::new(&plan.command[0])`)
- All streaming, cancel, and result logic remains identical regardless of execution mode
- The only branching point is the `Command` construction

#### `crates/pi-sandbox-runtime/src/main.rs`

Changes:
- Call `bubblewrap::detect()` at startup
- Pass bwrap availability to `validator::validate()` and `supervisor::supervise()`

### Platform Detection Flow

```
main.rs startup:
  bwrap = bubblewrap::detect()

validator::validate(plan, bwrap):
  if bwrap is Available:
    namespaces_applied = plan.policy.namespaces (all requested)
  else:
    namespaces_applied = [] (none applied)
    emit NAMESPACE_DEGRADED warning per requested namespace

supervisor::supervise(plan, effective_state, cancel_rx, bwrap):
  if bwrap is Available:
    argv = plan_builder::build(plan, effective_state)
    child = Command::new(bwrap.path).args(argv)
  else:
    child = Command::new(plan.command[0]).args(plan.command[1..])
  // everything else identical
```

### Rust Unit Tests

`plan_builder.rs` tests (pure Rust, no bwrap needed):
- Given a plan with read-only directory mounts → argv contains `--ro-bind`
- Given a plan with writable mounts → argv contains `--bind`
- Given a plan with tmpfs mount → argv contains `--tmpfs`
- Given a plan with network mode off → argv contains `--unshare-net`
- Given a plan with network mode full → no `--unshare-net`
- Given a plan with env entries → argv contains `--clearenv --setenv K V`
- Given a plan with cwd → argv contains `--chdir`
- Device mounts are always present
- Command is always last after `--`

`bubblewrap.rs` tests:
- `PI_SANDBOX_BWRAP_PATH` set → uses that path
- On non-Linux → returns Unavailable

### Protocol Test Updates

Existing protocol tests run on macOS (no bwrap). They continue to pass because:
- Supervisor falls back to direct execution
- Validation reports degraded namespaces (tests may need minor assertions updated)
- All NDJSON contract behavior is identical

New test: `tests/protocol/bwrap-integration.test.ts`
- Skipped on non-Linux
- Sends a plan, asserts `namespacesApplied` is non-empty
- Asserts bwrap actually ran (check lifecycle events, validate isolation)

---

## Section 2: Real Build Flows (Phase 9)

### Purpose

Validate that the sandbox can run real-world build workflows, not just `echo hello`.

### Directory Structure

```
tests/integration/
  fixtures/
    tiny-npm/
      package.json
    tiny-python/
      setup.py
      mypackage/
        __init__.py
    tiny-rust/
      Cargo.toml
      src/
        main.rs
  helpers.ts
  globalSetup.ts
  vitest.config.ts
  build-npm.test.ts
  build-python.test.ts
  build-rust.test.ts
  network-smoke.test.ts
```

### Fixture Repos

#### `tiny-npm/`

```json
{
  "name": "tiny-npm-fixture",
  "version": "1.0.0",
  "private": true,
  "dependencies": {}
}
```

An empty npm project. `npm install` creates `node_modules/` and `package-lock.json` with zero network needed. This validates that Node.js tooling works inside the sandbox (correct PATH, writable workspace, etc.).

#### `tiny-python/`

```python
# setup.py
from setuptools import setup
setup(
    name="tiny-python-fixture",
    version="1.0.0",
    packages=["mypackage"],
)
```

```python
# mypackage/__init__.py
"""Tiny fixture package."""
```

`pip install -e .` with no external deps. Validates Python tooling works inside the sandbox.

#### `tiny-rust/`

```toml
# Cargo.toml
[package]
name = "tiny-rust-fixture"
version = "0.1.0"
edition = "2021"
```

```rust
// src/main.rs
fn main() {
    println!("built");
}
```

`cargo build` with no external deps. Validates Rust toolchain access inside the sandbox.

### Integration Test Helpers

`tests/integration/helpers.ts`:
- Reuses `spawnRuntime()` and `makePlan()` from protocol test helpers (import or copy)
- Adds `copyFixture(name: string) -> { tempDir: string, cleanup: () => void }` — copies fixture into a temp directory that simulates a session workspace
- Adds `makeIntegrationPlan(fixture, command, profile?)` — builds a plan with the fixture's temp dir as workspace, correct mounts, and the specified profile

`tests/integration/globalSetup.ts`:
- Builds Rust binary (same as protocol tests)
- Sets `RUNTIME_BINARY_PATH`

### CI Gate Tests

#### `build-npm.test.ts`

1. Copy `tiny-npm` fixture to temp workspace
2. Build plan: command = `["npm", "install"]`, profile = `build-install`, workspace mounted writable
3. Send plan via NDJSON protocol
4. Assert: `validation.ok = true`
5. Assert: `result.exitCode = 0`
6. Assert: `result.reconciliationHints.terminalState = "clean_exit"`
7. Assert: `node_modules/` directory exists in temp workspace (or `package-lock.json` was created)
8. Cleanup temp dir

#### `build-python.test.ts`

1. Copy `tiny-python` fixture to temp workspace
2. Build plan: command = `["pip", "install", "-e", "."]`, profile = `build-install`
3. Send plan, assert exit code 0, clean_exit

#### `build-rust.test.ts`

1. Copy `tiny-rust` fixture to temp workspace
2. Build plan: command = `["cargo", "build"]`, profile = `build-install`
3. Send plan, assert exit code 0, clean_exit
4. Assert: `target/` directory exists in temp workspace

### Optional Network Smoke Tests

`network-smoke.test.ts` — skipped unless `RUN_NETWORK_TESTS=1`:

1. Create a temp workspace with a `package.json` that has one real dependency (e.g., `is-odd@3.0.1`)
2. Build plan: command = `["npm", "install"]`, profile = `build-install`, network mode = `full`
3. Send plan, assert exit code 0
4. Assert: `node_modules/is-odd/` exists
5. If Phase 10 is complete: assert `observedConnections` is non-empty

### What Changes in Existing Code

Nothing. Phase 9 is purely tests. The production code from Phases 0-8 is validated, not modified.

---

## Section 3: Network Observation (Phase 10)

### Architecture

A background observer thread runs during child process execution, polling `/proc/net/tcp` for outbound TCP connections.

```
supervisor::supervise()
  ├─ spawn child
  ├─ start observer thread (Linux only)
  ├─ stream stdout/stderr (existing)
  ├─ poll cancel (existing)
  ├─ child exits
  ├─ stop observer → Vec<ObservedConnection>
  └─ build result with observed connections
```

### Modified File: `crates/pi-sandbox-runtime/src/observer.rs`

Replace the stub with a real implementation.

#### `NetworkObserver` struct

```rust
pub struct NetworkObserver {
    handle: Option<JoinHandle<Vec<ObservedConnection>>>,
    stop_flag: Arc<AtomicBool>,
}
```

#### Public API

- `NetworkObserver::start() -> NetworkObserver` — spawns a polling thread (Linux only). On non-Linux, returns a no-op observer that produces empty results.
- `observer.stop() -> Vec<ObservedConnection>` — sets the stop flag, joins the thread, returns deduplicated connections.
- `observer.emit_events(seq: &AtomicU64)` — during polling, emits `network` streamed events for newly discovered connections.

#### `/proc/net/tcp` Parser

Parses `/proc/net/tcp` (and optionally `/proc/net/tcp6` for IPv6).

Each line format:
```
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:0035 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 ...
```

Parsing:
1. Skip header line (starts with whitespace + "sl")
2. Split by whitespace, extract field index 2 (`rem_address`)
3. Split `rem_address` on `:` → hex IP and hex port
4. Convert hex IP to `u32`, then to dotted decimal (little-endian on x86)
5. Convert hex port to `u16`
6. Extract field index 3 (`st` = state), filter to `01` (ESTABLISHED)
7. Filter out loopback (127.0.0.0/8) and unspecified (0.0.0.0)

#### Deduplication

The observer maintains a `HashSet<(String, u16)>` of (host, port) pairs seen so far. A connection is only emitted as a `network` event and added to the result list on first sight.

#### Polling Loop

```rust
loop {
    if stop_flag.load(Ordering::Relaxed) { break; }
    let connections = parse_proc_net_tcp();
    for conn in connections {
        if seen.insert((conn.host.clone(), conn.port)) {
            // New connection — emit network event
            let s = seq.fetch_add(1, Ordering::SeqCst);
            emit(&NetworkEnvelope::new(s, "outbound", conn.host, conn.port, Some("tcp")));
            results.push(conn);
        }
    }
    thread::sleep(Duration::from_millis(500));
}
```

#### Platform Behavior

- **Linux:** Real polling. Streamed `network` events. Populated `observedConnections` and `wouldHaveBlocked` in result.
- **macOS/other:** `NetworkObserver::start()` returns a no-op observer. `stop()` returns empty vec. No `network` events emitted. `observedConnections` and `wouldHaveBlocked` are empty. This is truthful.

No warning is emitted for missing network observation — this is a best-effort diagnostic feature, not a security claim.

### Modified File: `crates/pi-sandbox-runtime/src/supervisor.rs`

Changes:
- After spawning child, call `NetworkObserver::start()`
- Pass shared `Arc<AtomicU64>` sequence counter to observer (same one used by stdout/stderr threads)
- After child exits: call `observer.stop()` to get final connections
- Pass connections to result builder (replaces the current `observe_connections()` call)

### Existing Code That Lights Up

`compute_would_have_blocked()` in `observer.rs` already works correctly. Once `observe_connections()` returns real data, `wouldHaveBlocked` is automatically populated for allowlist scenarios. No changes needed.

### New Protocol Test

`tests/protocol/network-observation.test.ts`:
- Skip on non-Linux (`process.platform !== 'linux'`)
- Run a command that makes an outbound TCP connection (e.g., `curl -s http://example.com` or `python3 -c "import urllib.request; urllib.request.urlopen('http://example.com')"`)
- Assert: at least one `network` streamed event was received
- Assert: `result.observedConnections` is non-empty
- Assert: observed connections contain expected host

### Integration With Phase 9 Network Smoke Tests

Once Phase 10 lands, the optional `network-smoke.test.ts` from Phase 9 can add assertions:
- After `npm install` with real network: `observedConnections` contains registry.npmjs.org
- With allowlist mode: `wouldHaveBlocked` is computed correctly

---

## File Map Summary

### New Files

| File | Phase | Purpose |
|------|-------|---------|
| `crates/pi-sandbox-runtime/src/bubblewrap.rs` | 8 | Bwrap binary discovery and platform detection |
| `crates/pi-sandbox-runtime/src/plan_builder.rs` | 8 | Manifest+policy → bwrap argv construction |
| `tests/integration/fixtures/tiny-npm/package.json` | 9 | NPM build fixture |
| `tests/integration/fixtures/tiny-python/setup.py` | 9 | Python build fixture |
| `tests/integration/fixtures/tiny-python/mypackage/__init__.py` | 9 | Python fixture package |
| `tests/integration/fixtures/tiny-rust/Cargo.toml` | 9 | Rust build fixture |
| `tests/integration/fixtures/tiny-rust/src/main.rs` | 9 | Rust fixture source |
| `tests/integration/helpers.ts` | 9 | Integration test utilities |
| `tests/integration/globalSetup.ts` | 9 | Build Rust binary for integration tests |
| `tests/integration/vitest.config.ts` | 9 | Vitest config for integration tests |
| `tests/integration/build-npm.test.ts` | 9 | NPM build integration test |
| `tests/integration/build-python.test.ts` | 9 | Python build integration test |
| `tests/integration/build-rust.test.ts` | 9 | Rust build integration test |
| `tests/integration/network-smoke.test.ts` | 9 | Optional network smoke test |
| `tests/protocol/bwrap-integration.test.ts` | 8 | Bwrap-specific protocol test (Linux only) |
| `tests/protocol/network-observation.test.ts` | 10 | Network observation protocol test (Linux only) |

### Modified Files

| File | Phase | Changes |
|------|-------|---------|
| `crates/pi-sandbox-runtime/src/contract.rs` | 8 | Add `namespaces_applied`, `env_applied` to EffectiveState |
| `crates/pi-sandbox-runtime/src/validator.rs` | 8 | Namespace resolution, env resolution, NAMESPACE_DEGRADED warnings |
| `crates/pi-sandbox-runtime/src/supervisor.rs` | 8, 10 | Bwrap dispatch, observer integration |
| `crates/pi-sandbox-runtime/src/main.rs` | 8 | Bwrap detection at startup, pass to validator/supervisor |
| `crates/pi-sandbox-runtime/src/observer.rs` | 10 | Replace stub with /proc/net/tcp polling |
| `crates/pi-sandbox-runtime/Cargo.toml` | 8 | No new deps expected (uses std only) |

### Unchanged Files

All TypeScript files in `packages/pi-sandbox-extension/` remain unchanged. The TS runtime client already handles all NDJSON message types including `network` events and `namespacesApplied`. No TS changes needed.

---

## Migration Phase Gates

| Phase | Gate Criteria |
|-------|--------------|
| 8 | `plan_builder.rs` unit tests pass. On Linux: bwrap protocol test passes with real isolation. On macOS: existing protocol tests pass with degraded warnings. |
| 9 | All 3 CI gate integration tests pass (npm, Python, Rust builds complete in sandbox). |
| 10 | On Linux: network observation test passes (connections detected during execution). `wouldHaveBlocked` correctly computed for allowlist scenarios. |

---

## Engineering Rules (carried from v1)

- Do not fake namespace application. Report what was actually applied.
- Do not claim allowlist enforcement if only observing.
- Do not bind host `/dev` wholesale. Use hardcoded minimal device set.
- Do not make network observation a security claim. It is diagnostic.
- Platform fallback must be truthful: macOS reports empty namespaces and empty observations.
- `plan_builder.rs` is a pure function. No side effects, no I/O.
