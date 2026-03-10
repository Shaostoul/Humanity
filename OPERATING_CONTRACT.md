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

## 7) Mandatory VPS sync after web/relay/runtime changes
This is required every time runtime-facing files change (web client, relay, desktop runtime assets, shared shell/CSS/JS, or deployment config):

1. Push latest local `main` to GitHub.
2. On VPS, force sync to `origin/main` (`git fetch` + `git reset --hard origin/main`).
3. Rebuild relay in release mode.
4. Restart relay service.
5. Verify VPS HEAD commit equals local HEAD commit.
6. Verify service health is active/running.

Do not report "done" or ask the user to test until all six steps are complete and explicitly reported.
