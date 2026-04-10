# Quick Reference

## Common commands

```bash
nixo catalog
nixo catalog --json
nixo create --with bash,coreutils --network off --json
nixo create --profile strict --json
nixo exec <session-id> -- echo hello
nixo exec <session-id> --json -- python3 -c "print('hello')"
nixo list
nixo status <session-id>
nixo destroy <session-id>
```

## Typical patterns

- Package discovery:
  - use `catalog` to find names before creating a sandbox
- New sandbox:
  - `create --with <packages> --network off --json`
- Reusing a session:
  - `exec <session-id> -- <command...>`
- Cleanup:
  - `destroy <session-id>` after the work is finished

## Alias note

- `nixosandbox` is the compatibility alias.
- Prefer `nixo` in new instructions and examples.
