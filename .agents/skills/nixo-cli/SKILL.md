---
name: nixo-cli
description: Use when requests involve nixo or nixosandbox sandbox lifecycle operations, catalog queries, session-id workflows, or CLI errors such as 'Could not find flake.nix' and create flag conflicts.
---

# nixo CLI

Use this skill for runtime-agnostic workflows around the `nixo` command line.

## Naming

- Use `nixo` as the default command in all new instructions, prompts, and automation.
- Use `nixosandbox` only when the user explicitly asks for the legacy name or when `nixo` is unavailable.
- Keep flags and behavior identical across both names.

## Core workflow

- Discover available packages with `catalog`.
- Create a sandbox with `create`, using `--with` for package lists or `--profile` for named presets.
- Use `--json` when the output will be parsed by another tool or agent.
- Run commands inside an existing sandbox with `exec <session-id> -- <command...>`.
- Inspect session state with `list` and `status`.
- Remove sessions with `destroy` when they are no longer needed.

## Safe defaults

- Prefer `--network off` unless the task explicitly needs downloads or live network access.
- Prefer narrow package sets over broad sandboxes.
- Prefer machine-readable output when the result feeds another step.
- Use `list` before `destroy` if you need to confirm the active session set.
- For automation, capture `sessionId` from `create --json` output and reuse it for `exec`, `status`, and `destroy`.

## Common failure patterns

- `Could not find flake.nix`:
  - set `NIXOSANDBOX_FLAKE_ROOT` to a directory containing `flake.nix`, or run from the repo root.
- `create` option conflicts:
  - never combine `--with`, `--profile`, and `--spec` in the same command.
- Invalid package names with `--with`:
  - confirm package names through `catalog` first.

## When to read more

- [Quick reference](references/quick-reference.md)
- [Troubleshooting](references/troubleshooting.md)
