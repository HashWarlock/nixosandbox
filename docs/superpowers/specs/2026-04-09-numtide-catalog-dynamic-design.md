# Design: Dynamic numtide/llm-agents.nix Catalog

**Date:** 2026-04-09  
**Status:** Approved  
**Scope:** `nix/catalog.nix` only

## Problem

The catalog uses a hardcoded `pickExisting` whitelist of ~25 agent names from
`numtide/llm-agents.nix`. The upstream flake currently exposes 65+ packages
across 8 categories. Any new package added by numtide is invisible to
`nixosandbox catalog` and cannot be used with `--with` until someone manually
adds its name to the whitelist.

## Solution

Replace the whitelist with a dynamic passthrough: assign `agents` directly from
`llm-agents-pkgs`, stripping only the `default` meta-attribute that every Nix
flake packages output includes.

## Change

**File:** `nix/catalog.nix`

Remove: the `pickExisting` helper function and the 25-name whitelist.  
Add: `agents = builtins.removeAttrs llm-agents-pkgs [ "default" ];`

No other files are modified. The `tools` section (nixpkgs packages) is unchanged.

## Impact

| Surface | Before | After |
|---------|--------|-------|
| `nixosandbox catalog` agent count | ~25 | 65+ |
| `nixosandbox create --with <name>` | only whitelisted names | any llm-agents.nix package |
| Maintenance on numtide upstream update | manual name addition required | zero — auto-picks up new packages on `nix flake update` |

## Out of scope

- Category-structured display in `nixosandbox catalog` (future enhancement)
- Changes to `tools` (nixpkgs packages)
- Changes to Rust CLI, flake.nix, mkAgentSandbox.nix, or profiles
