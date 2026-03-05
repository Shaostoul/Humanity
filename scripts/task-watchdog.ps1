param(
  [int]$MaxSilenceMinutes = 10
)

$ErrorActionPreference = 'Stop'
$statePath = 'C:\Humanity\memory\active-task.json'
if (!(Test-Path $statePath)) { exit 0 }

$state = Get-Content $statePath -Raw | ConvertFrom-Json
if ($state.status -eq 'done' -or [string]::IsNullOrWhiteSpace($state.task)) { exit 0 }

$now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$ageMin = ($now - [int64]$state.updatedAt) / 60.0
if ($ageMin -lt $MaxSilenceMinutes) { exit 0 }

$msg = "🔔 Watchdog: no progress update for {0:N0} min on '{1}'." -f $ageMin, $state.task
openclaw message send --channel discord --account default --target ("user:" + $state.targetUserId) --message $msg | Out-Null

$state.updatedAt = $now
$state.message = 'watchdog-ping'
$state | ConvertTo-Json -Depth 8 | Set-Content -Path $statePath -Encoding UTF8
