param(
  [ValidateSet('start','update','block','done')][string]$Mode = 'update',
  [string]$Task = '',
  [string]$Message = '',
  [string]$TargetUserId = '119318268300361728'
)

$ErrorActionPreference = 'Stop'
$statePath = 'C:\Humanity\memory\active-task.json'
New-Item -ItemType Directory -Force -Path (Split-Path $statePath) | Out-Null

$now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$state = @{
  status = $Mode
  task = $Task
  message = $Message
  targetUserId = $TargetUserId
  updatedAt = $now
}
$state | ConvertTo-Json -Depth 8 | Set-Content -Path $statePath -Encoding UTF8

$prefix = switch ($Mode) {
  'start' { '⏳ Started' }
  'update' { '🔄 Progress' }
  'block' { '⚠️ Blocked' }
  'done' { '✅ Done' }
  default { 'ℹ️ Update' }
}

$txt = "$prefix";
if ($Task) { $txt += ": $Task" }
if ($Message) { $txt += " — $Message" }

openclaw message send --channel discord --account default --target "user:$TargetUserId" --message $txt | Out-Null
Write-Host "sent=$txt"
