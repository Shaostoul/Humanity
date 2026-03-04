param(
  [Parameter(Mandatory=$true)][string]$ConfigPath,
  [Parameter(Mandatory=$true)][string]$MutateScriptPath,
  [switch]$ValidateOnly
)

$ErrorActionPreference = 'Stop'

if (!(Test-Path $ConfigPath)) {
  throw "Config file not found: $ConfigPath"
}
if (!(Test-Path $MutateScriptPath)) {
  throw "Mutate script not found: $MutateScriptPath"
}

$ts = Get-Date -Format "yyyyMMdd-HHmmss"
$backupPath = "$ConfigPath.pre-edit-$ts.bak"
Copy-Item -Path $ConfigPath -Destination $backupPath -Force
Write-Host "backup_created=$backupPath"

try {
  if (-not $ValidateOnly) {
    & powershell -NoProfile -ExecutionPolicy Bypass -File $MutateScriptPath
  }

  openclaw config validate | Out-Host
  Write-Host "result=ok"
}
catch {
  Write-Warning "edit_or_validate_failed=$($_.Exception.Message)"
  Copy-Item -Path $backupPath -Destination $ConfigPath -Force
  Write-Warning "rollback=applied"

  try {
    openclaw config validate | Out-Host
  } catch {
    Write-Warning "post_rollback_validate_failed=$($_.Exception.Message)"
  }

  throw
}
