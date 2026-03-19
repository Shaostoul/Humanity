# Operations Runbook (Humanity)

## Standard deploy (VPS)

```bash
sudo /usr/local/bin/humanity-deploy-relay
```

This should:
1. back up DB,
2. pull latest main,
3. build relay,
4. sync runtime web client JS,
5. restart service,
6. run smoke checks.

## Smoke check only

```bash
sudo /usr/local/bin/humanity-smoke-check
```

## Last deploy report

```bash
sudo /usr/local/bin/humanity-deploy-last-report
```

Checks:
- relay service active,
- runtime `/var/www/humanity/chat/app.js` hash matches repo `/opt/Humanity/ui/chat/app.js`,
- release binary contains critical command markers (`/channel-edit`, `/channel-delete`).

## Fast rollback

```bash
sudo /usr/local/bin/humanity-restore-latest-db
```

## Gateway restart with auto notify (local PC)

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\Humanity\scripts\restart-with-notify.ps1 -Checkpoint "reason"
```

## OpenClaw config edits (safe flow)
- Always backup before edit.
- Validate after edit.
- Rollback immediately if validation fails.
- See: `knowledge/OPENCLAW_CONFIG_CHANGE_POLICY.md`

## Long-task progress discipline
Use helper script to send explicit status pings in Discord:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\Humanity\scripts\task-progress.ps1 -Mode start -Task "<task>" -Message "begin"
```

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\Humanity\scripts\task-progress.ps1 -Mode update -Task "<task>" -Message "<progress>"
```

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\Humanity\scripts\task-progress.ps1 -Mode done -Task "<task>" -Message "complete"
```

Optional watchdog ping if silent too long:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\Humanity\scripts\task-watchdog.ps1 -MaxSilenceMinutes 10
```
