# Troubleshooting

## `nixo` is not found

- Check whether the primary binary is installed.
- Try `nixosandbox` if you are on an older setup that still exposes the compatibility alias.
- Verify the PATH points to the directory that contains the installed binary.

## `catalog` or `create` fails early

- Confirm the command name is correct and the binary is the expected one.
- Re-run with `--json` only if you need structured output; it does not fix command errors.
- If using `create --with`, make sure package names are valid and comma-separated.

## `Could not find flake.nix`

- Set flake root explicitly:
  - `export NIXOSANDBOX_FLAKE_ROOT=/path/to/nixo`
- Verify the path:
  - `test -f "$NIXOSANDBOX_FLAKE_ROOT/flake.nix" && echo ok`
- Retry:
  - `nixo catalog --json`
- Alternative:
  - run commands from the repository root that contains `flake.nix`.

## Sandbox creation problems

- Prefer `--network off` unless the task needs network access.
- If the sandbox setup references a profile, confirm the profile exists and matches the intended package set.
- If the CLI reports a missing package, re-check the package name returned by `catalog`.

## Session execution problems

- Use `status <session-id>` to confirm the session still exists.
- Use `list` to verify the ID before calling `exec` or `destroy`.
- If a command fails inside the sandbox, distinguish between sandbox setup failure and command exit status.

## Compatibility issues

- If instructions mention `nixosandbox`, treat them as valid alias usage, not a different tool.
- When documenting or prompting new workflows, prefer `nixo` so the canonical name stays consistent.
