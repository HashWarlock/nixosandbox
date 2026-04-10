---
name: nixo-cli
description: Use when working with the nixo CLI to discover packages, create sandboxes, execute commands, inspect sessions, or troubleshoot compatibility. Prefer `nixo`; `nixosandbox` remains a compatibility alias.
---

# nixo CLI

Use this skill for runtime-agnostic workflows around the `nixo` command line.

## Naming

- Prefer `nixo` in examples, prompts, and automation.
- Treat `nixosandbox` as a compatibility alias for older scripts and environments.
- Keep command behavior and flag usage identical regardless of which binary name is invoked.

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

## When to read more

- [Quick reference](references/quick-reference.md)
- [Troubleshooting](references/troubleshooting.md)
