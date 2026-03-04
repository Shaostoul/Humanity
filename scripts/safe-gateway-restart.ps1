param(
  [string]$Checkpoint = "unspecified task",
  [int]$TimeoutSeconds = 90,
  [string]$NotifyChannel = "discord",
  [string]$NotifyTarget = "119318268300361728"
)

$ErrorActionPreference = 'Stop'
$configPath = "C:\Users\Shaos\.openclaw\openclaw.json"
$ts = Get-Date -Format "yyyyMMdd-HHmmss"
$checkpointPath = "C:\Humanity\memory\gateway-restart-checkpoint.txt"

function Write-Info($msg) { Write-Host "[safe-restart] $msg" }

function Test-DiscordHealthy {
  try {
    $status = openclaw status 2>&1 | Out-String
    $discordLine = ($status -split "`r?`n") | Where-Object { $_ -match "Discord" } | Select-Object -First 1
    if ($discordLine -and $discordLine -match "\bON\b" -and $discordLine -match "\bOK\b") {
      return $true
    }
  } catch {}
  return $false
}

Write-Info "checkpoint=$Checkpoint"
New-Item -ItemType Directory -Force -Path (Split-Path $checkpointPath) | Out-Null
Set-Content -Path $checkpointPath -Value ("{0}`n{1}" -f (Get-Date).ToString('s'), $Checkpoint) -Encoding UTF8

# Backup config before restart as a safety net.
if (Test-Path $configPath) {
  $backupPath = "$configPath.pre-restart-$ts.bak"
  Copy-Item -Path $configPath -Destination $backupPath -Force
  Write-Info "backup_created=$backupPath"
}

# Validate config before touching gateway.
openclaw config validate | Out-Host

Write-Info "restarting_gateway"
openclaw gateway restart | Out-Host

$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$healthy = $false
while ((Get-Date) -lt $deadline) {
  if (Test-DiscordHealthy) {
    $healthy = $true
    break
  }
  Start-Sleep -Seconds 3
}

if (-not $healthy) {
  Write-Info "health_check_failed_after_restart; trying stop/start"
  openclaw gateway stop | Out-Host
  Start-Sleep -Seconds 2
  openclaw gateway start | Out-Host

  $deadline2 = (Get-Date).AddSeconds($TimeoutSeconds)
  while ((Get-Date) -lt $deadline2) {
    if (Test-DiscordHealthy) {
      $healthy = $true
      break
    }
    Start-Sleep -Seconds 3
  }
}

if (-not $healthy) {
  Write-Info "gateway_not_healthy; manual intervention required"
  exit 2
}

Write-Info "gateway_healthy_discord_ok"
Write-Info "resume_checkpoint=$Checkpoint"

# Proactive notification so user never has to ask "are you there?"
try {
  if ($NotifyChannel -eq 'discord') {
    openclaw message send --channel discord --account default --target ("user:" + $NotifyTarget) --message ("Restart complete. I'm back online and running. Resuming: " + $Checkpoint) | Out-Host
  }
  Write-Info "proactive_notify_sent"
}
catch {
  Write-Info "proactive_notify_failed=$($_.Exception.Message)"
}
