param(
  [string]$Checkpoint = "unspecified task",
  [string]$NotifyTarget = "119318268300361728",
  [int]$TimeoutSeconds = 120
)

$ErrorActionPreference = 'Stop'
$watcher = @"
`$ErrorActionPreference = 'SilentlyContinue'
`$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
while ((Get-Date) -lt `$deadline) {
  `$s = openclaw status 2>&1 | Out-String
  `$line = (`$s -split "`r?`n") | Where-Object { `$_ -match "Discord" } | Select-Object -First 1
  if (`$line -and `$line -match "\bON\b" -and `$line -match "\bOK\b") {
    $msg = "Reply exactly with: Restart complete. I'm back online and running. Resuming: $Checkpoint"
    openclaw agent --to $NotifyTarget --channel discord --message $msg --deliver | Out-Null
    break
  }
  Start-Sleep -Seconds 3
}
"@

$watcherPath = "C:\Humanity\scripts\_restart_notify_watcher.ps1"
Set-Content -Path $watcherPath -Value $watcher -Encoding UTF8
Start-Process powershell -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',$watcherPath) -WindowStyle Hidden

openclaw gateway restart | Out-Host
