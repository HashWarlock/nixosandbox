# Pi Sandbox Phases 11-12 Design Spec

**Date:** 2026-04-06
**Branch:** `pi-sandbox-refactor`
**Prerequisite:** Phases 0-10 complete (tag `v1-phases-8-10-complete`)

---

## Overview

Phase 11 removes the legacy `sandbox-rs/` Axum REST server. Phase 12 adds two Phase 2 capabilities: session-based browser automation (Playwright, TS-side) and real allowlist network enforcement (iptables inside bwrap network namespace, Rust-side). Nix runtime bases are deferred to a future phase.

---

## Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Legacy server removal | Hard delete `sandbox-rs/` | `v0-legacy-server` tag preserves history; dead code adds confusion |
| Browser automation | TS-side Playwright | Browser doesn't benefit from namespace isolation; Pi already has TS tool infra |
| Browser tool shape | Single `sandbox_browser` tool with sub-commands | Session-scoped persistent page state; reduces tool registration noise |
| Allowlist enforcement | iptables inside bwrap network namespace | Kernel-level enforcement for all protocols; standard Linux approach |
| DNS resolution | Pre-resolve at plan time in validator | Bounded problem; avoids DNS inside sandbox |
| Nix runtime bases | Deferred | High complexity, low immediate value; `RuntimeBase` interface already supports future swap |
| Testing pattern | Same as Phases 8-10 | Linux-only enforcement tests, macOS degradation tests, optional env-gated smoke tests |

---

## Phase 11: Legacy Server Deprecation

### What Gets Deleted

The entire `sandbox-rs/` directory (~1500 lines):

| Path | Description |
|---|---|
| `sandbox-rs/src/handlers/shell.rs` | Shell execution handler |
| `sandbox-rs/src/handlers/code.rs` | Code execution handler |
| `sandbox-rs/src/handlers/file.rs` | File operations handler |
| `sandbox-rs/src/handlers/browser.rs` | Browser route handler |
| `sandbox-rs/src/handlers/skills.rs` | Skills CRUD handler |
| `sandbox-rs/src/handlers/factory.rs` | Factory/session dialogue handler |
| `sandbox-rs/src/handlers/tee.rs` | TEE operations handler |
| `sandbox-rs/src/handlers/health.rs` | Health check handler |
| `sandbox-rs/src/handlers/mod.rs` | Handler module index |
| `sandbox-rs/src/browser/` | BrowserService (chromiumoxide) |
| `sandbox-rs/src/skills/` | Skill registry and factory |
| `sandbox-rs/src/tee/` | TEE client |
| `sandbox-rs/src/main.rs` | Axum router and startup |
| `sandbox-rs/src/state.rs` | Shared app state |
| `sandbox-rs/src/config.rs` | Server configuration |
| `sandbox-rs/src/error.rs` | Error types |
| `sandbox-rs/Cargo.toml` | Heavy dependency tree (axum, tokio, chromiumoxide, etc.) |

### What Changes

- Root `Cargo.toml`: remove `sandbox-rs` from workspace members.
- Root `Cargo.lock`: regenerated without sandbox-rs dependencies.

### What Stays Untouched

- `crates/pi-sandbox-runtime/` -- the new Rust runtime
- `packages/pi-sandbox-extension/` -- the new TS extension
- `tests/` -- all protocol and integration tests

### Verification Gate

After deletion:
1. `cargo build -p pi-sandbox-runtime` succeeds
2. All protocol tests pass (`npm test` in `tests/protocol/`)
3. All integration tests pass (`npm test` in `tests/integration/`)

---

## Phase 12a: Session-Based Browser Tool

### Architecture

Browser automation lives entirely in the TS extension. No NDJSON protocol changes. The browser runs on the host, outside bwrap -- browser automation does not benefit from namespace isolation.

```
Pi agent
  -> sandbox_browser({ sessionId, action: "goto", url: "..." })
    -> Pi extension (BrowserManager)
      -> Playwright browser context (persistent per session)
```

### New Files

| File | Responsibility |
|---|---|
| `packages/pi-sandbox-extension/src/browser.ts` | BrowserManager class: lifecycle, session-scoped page management |

### Modified Files

| File | Change |
|---|---|
| `packages/pi-sandbox-extension/src/extension.ts` | Register `sandbox_browser` tool, wire BrowserManager |
| `packages/pi-sandbox-extension/src/session-manager.ts` | Session cleanup calls `BrowserManager.closePage(sessionId)` |
| `packages/pi-sandbox-extension/package.json` | Add `playwright-core` dependency |

### BrowserManager Design

```typescript
class BrowserManager {
  // Lazy-initialized Chromium instance (shared across sessions)
  private browser: Browser | null = null;

  // One persistent page per sandbox session
  private pages: Map<string, Page> = new Map();

  async getOrCreatePage(sessionId: string): Promise<Page>;
  async closePage(sessionId: string): Promise<void>;
  async shutdown(): Promise<void>;
}
```

- **Lazy launch:** Browser starts on first `sandbox_browser` call, not at extension init.
- **Session-scoped pages:** Each sandbox session gets one persistent page. Navigation, clicks, and evaluations operate on the same page -- like a real browsing session.
- **Cleanup:** `closePage()` is called when a session is torn down or reconciled after crash. `shutdown()` closes the browser entirely.

### Tool Interface

Single tool: `sandbox_browser`

Parameters:
- `sessionId: string` -- sandbox session to operate within
- `action: "goto" | "screenshot" | "evaluate" | "click" | "type" | "close"`
- `url?: string` -- for `goto`
- `selector?: string` -- for `click` and `type`
- `text?: string` -- for `type`
- `script?: string` -- for `evaluate`

Return values by action:
- `goto` -- page title + truncated text content
- `screenshot` -- base64 PNG string
- `evaluate` -- JSON-serialized result
- `click` -- confirmation string
- `type` -- confirmation string
- `close` -- confirmation string

### Dependency

`playwright-core` (not `playwright`). Uses `playwright-core` to avoid bundling browser binaries. Expects Chromium to be available on the host via `PLAYWRIGHT_CHROMIUM_PATH` env var or system-installed Chrome/Chromium.

### Testing

- Unit tests for BrowserManager lifecycle (launch, getOrCreatePage, closePage, shutdown)
- Integration test: `sandbox_browser` goto + screenshot on a local HTML fixture
- Test that session cleanup closes browser pages

---

## Phase 12b: Real Allowlist Network Enforcement

### Architecture

When bwrap is available and creates a network namespace (`--unshare-net`), the runtime injects iptables rules to enforce the allowlist at the kernel level. On platforms without bwrap, behavior remains degraded (unchanged from v1).

```
Plan arrives with policy.network.mode = "allowlist"
  -> Validator resolves hostnames to IPs (DNS pre-resolution)
  -> plan_builder generates iptables wrapper script
  -> bwrap --unshare-net runs wrapper script
    -> wrapper: iptables -P OUTPUT DROP
    -> wrapper: iptables -A OUTPUT -d <resolved_ip> -j ACCEPT (per host)
    -> wrapper: iptables -A OUTPUT -o lo -j ACCEPT
    -> wrapper: iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
    -> wrapper: exec <user command>
  -> Observer cross-checks: wouldHaveBlocked should be empty
```

### DNS Pre-Resolution

The validator resolves each hostname in `policy.network.allowlist` to IP addresses before execution begins.

Rules:
- Resolution uses system DNS (std::net in Rust)
- Each hostname may resolve to multiple IPs; all are allowed
- If a hostname fails to resolve: emit `DNS_RESOLUTION_PARTIAL` warning, skip that host
- If ALL hostnames fail to resolve: degrade to `actual=full, enforcement=observed`, emit `ALLOWLIST_DNS_FAILED` warning
- Resolved IPs are stored in `EffectiveState.resolved_allowlist: Vec<ResolvedAllowlistEntry>`

```rust
pub struct ResolvedAllowlistEntry {
    pub hostname: String,
    pub ips: Vec<String>,
    pub resolved: bool,
}
```

### Effective Network States

| Requested | Bwrap? | Net NS? | DNS OK? | Actual | Enforcement | Degraded |
|---|---|---|---|---|---|---|
| `allowlist` | Yes | Yes | Yes | `allowlist` | `enforced` | `false` |
| `allowlist` | Yes | Yes | All fail | `full` | `observed` | `true` |
| `allowlist` | Yes | No | Any | `full` | `observed` | `true` |
| `allowlist` | No | N/A | Any | `full` | `observed` | `true` |
| `off` | Yes | Yes | N/A | `off` | `enforced` | `false` |
| `off` | No | N/A | N/A | `off` | `best_effort` | `true` |
| `full` | Any | Any | N/A | `full` | `observed` | `false` |

### Modified Files

| File | Change |
|---|---|
| `crates/pi-sandbox-runtime/src/contract.rs` | Add `ResolvedAllowlistEntry`, add `resolved_allowlist` to `EffectiveState` |
| `crates/pi-sandbox-runtime/src/validator.rs` | DNS resolution logic, new effective states for enforced allowlist |
| `crates/pi-sandbox-runtime/src/plan_builder.rs` | Generate iptables wrapper script when allowlist is enforced |
| `crates/pi-sandbox-runtime/src/supervisor.rs` | Pass resolved allowlist through; detect enforcement leaks via observer cross-check |

### Wrapper Script Generation

When `effective_state.network.actual == "allowlist"` and bwrap is available, `plan_builder.rs` generates a wrapper shell script:

```bash
#!/bin/sh
set -e
iptables -P OUTPUT DROP
iptables -A OUTPUT -o lo -j ACCEPT
iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -d 93.184.216.34 -j ACCEPT
iptables -A OUTPUT -d 2606:2800:220:1:... -j ACCEPT
# ... one rule per resolved IP
exec "$@"
```

The plan builder writes this to a temp file inside the sandbox and adjusts the bwrap command to run the wrapper with the user command as arguments.

**Prerequisite mount:** The wrapper script requires `iptables` to be available inside the sandbox. `plan_builder.rs` must add a read-only bind mount for the iptables binary (resolved via `which iptables` on the host) when generating an allowlist-enforced plan. If iptables is not found on the host, the validator degrades allowlist to `full/observed` and emits an `IPTABLES_NOT_FOUND` warning.

### Observer Cross-Check

The existing `/proc/net/tcp` observer continues running during allowlist-enforced executions. After execution:
- `wouldHaveBlocked` is computed as before
- If enforcement was `enforced` and `wouldHaveBlocked` is non-empty, emit an `ENFORCEMENT_LEAK` warning in the result -- this indicates an iptables race condition or misconfiguration
- This is a safety net, not the enforcement mechanism

### New Warning Codes

| Code | When |
|---|---|
| `DNS_RESOLUTION_PARTIAL` | Some allowlist hostnames failed to resolve |
| `ALLOWLIST_DNS_FAILED` | All allowlist hostnames failed to resolve; degraded to full |
| `ENFORCEMENT_LEAK` | Observer saw a connection that should have been blocked by iptables |
| `IPTABLES_NOT_FOUND` | iptables binary not found on host; allowlist degraded to full/observed |

### Testing

Protocol tests (same pattern as Phases 8-10):
- **Linux test:** allowlist with bwrap -> enforcement=enforced, attempt blocked connection -> connection fails
- **macOS test:** existing degraded-allowlist test already covers this (enforcement=observed, degraded=true)
- **Optional smoke test:** `RUN_ALLOWLIST_TESTS=1` env var gates a test that verifies real iptables enforcement with external host

---

## File Map (All Changes)

### Phase 11 -- Delete

| Path | Action |
|---|---|
| `sandbox-rs/` (entire directory) | Delete |
| Root `Cargo.toml` | Remove sandbox-rs from workspace members |

### Phase 12a -- New/Modified (Browser)

| Path | Action |
|---|---|
| `packages/pi-sandbox-extension/src/browser.ts` | Create |
| `packages/pi-sandbox-extension/src/extension.ts` | Modify (add sandbox_browser tool) |
| `packages/pi-sandbox-extension/src/session-manager.ts` | Modify (browser cleanup on session teardown) |
| `packages/pi-sandbox-extension/package.json` | Modify (add playwright-core) |
| `tests/extension/browser.test.ts` | Create |

### Phase 12b -- Modified (Allowlist Enforcement)

| Path | Action |
|---|---|
| `crates/pi-sandbox-runtime/src/contract.rs` | Modify (ResolvedAllowlistEntry, resolved_allowlist) |
| `crates/pi-sandbox-runtime/src/validator.rs` | Modify (DNS resolution, enforced allowlist states) |
| `crates/pi-sandbox-runtime/src/plan_builder.rs` | Modify (iptables wrapper script generation) |
| `crates/pi-sandbox-runtime/src/supervisor.rs` | Modify (enforcement leak detection) |
| `tests/protocol/allowlist-enforced.test.ts` | Create |

---

## What Is NOT in This Spec

- **Nix runtime bases** -- Deferred. `RuntimeBase` interface already supports future `NixComposedBase`.
- **TEE support** -- Removed in Phase 11, not replaced.
- **Skills/Factory** -- Removed in Phase 11, not replaced. Pi's native tool system replaces these.
- **Resource limits enforcement** -- `ResourceLimits` exists in the contract but enforcement (cgroups) is deferred.
- **Browser inside bwrap** -- Explicitly not done. Browser runs on host.

---

## Phase Gates

| Gate | Criteria |
|---|---|
| Phase 11 complete | `sandbox-rs/` deleted, `cargo build` succeeds, all existing tests pass |
| Phase 12a complete | `sandbox_browser` tool works with goto/screenshot/evaluate/click/type/close, browser tests pass |
| Phase 12b complete | Allowlist enforcement works on Linux with iptables, DNS resolution in validator, observer cross-check, enforcement tests pass on Linux, degradation tests pass on macOS |

---

## Engineering Rules (Carried from Spec A)

1. **Truthfulness** -- Never claim enforcement not actually applied.
2. **Platform honesty** -- macOS reports degraded; Linux reports enforced only when kernel mechanisms are active.
3. **No protocol changes for browser** -- Browser is entirely TS-side.
4. **Test gating** -- Linux-only tests skip on macOS. Optional smoke tests behind env vars.
5. **YAGNI** -- No Nix bases, no cgroups, no browser-in-bwrap.
