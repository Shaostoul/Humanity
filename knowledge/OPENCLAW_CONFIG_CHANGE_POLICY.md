# OpenClaw Config Change Policy

For any edits to `C:\Users\Shaos\.openclaw\openclaw.json`:

1. Create timestamped backup first.
2. Apply change.
3. Run `openclaw config validate`.
4. If validation fails, immediately restore from backup.
5. Retry change with corrected mutation.

## Default safety command pattern

```powershell
# Create a manual backup
Copy-Item C:\Users\Shaos\.openclaw\openclaw.json C:\Users\Shaos\.openclaw\openclaw.json.pre-edit-$(Get-Date -Format yyyyMMdd-HHmmss).bak
```

```powershell
# Validate after changes
openclaw config validate
```

## Automated helper

Use:

`C:\Humanity\scripts\safe-openclaw-config.ps1`

It automatically:
- creates a pre-edit backup,
- runs a mutation script,
- validates config,
- rolls back on failure.
