# OPERATING_CONTRACT.md

Operational behaviors that must persist across sessions (including after `/new`).

## 1) Progress updates during active development
- If a task is marked active, send a user progress update every 10 minutes max.
- If blocked for >5 minutes, send a blocker update immediately.
- Never go silent during active work.

## 2) Test-gate rule (no premature handoff)
Do not ask the user to test until ALL are green:
- runtime sync verifier passes,
- smoke checks pass,
- service health is OK,
- deploy succeeded,
- touched runtime assets hash-match repo sources.

## 3) Drift prevention
- Use manifest-driven runtime sync (`runtime-sync-manifest.txt`).
- Run `humanity-verify-runtime-sync` in smoke checks.
- Fail deploy on drift.

## 4) Safety for OpenClaw config edits
Before editing `openclaw.json`:
- backup first,
- validate after change,
- rollback on failure.

## 5) Restart reliability
When gateway restarts are triggered:
- health-check Discord ON/OK,
- proactively notify user restart is complete.

## 6) Reporting format
Every technical update includes:
- edit status,
- build status,
- checks status,
- deploy status,
- runtime sync status,
- exact verify target.
