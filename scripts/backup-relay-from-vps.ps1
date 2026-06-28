# =====================================================================
# backup-relay-from-vps.ps1 -- pull the live relay DB backup to this PC
# =====================================================================
# WHY: off-site backup using the operator's OWN devices instead of a
# third-party cloud (sovereignty -- see docs/design/device-mesh.md).
# The VPS is public + always-on; this PC is behind home NAT, so the
# flow is PC-PULLS-FROM-VPS (the VPS never reaches into the house).
#
# This is the "immediate" half of the device-mesh vision (TIER 0 #1
# off-site backup). The full device-mesh feature (dashboard, roles,
# device-to-device, restore) is the design doc; this is the zero-new-
# app-code stopgap that gives real 3-2-1 backup today:
#   copy 1: live DB on the VPS (/opt/Humanity/data/relay.db)
#   copy 2: VPS-local snapshots (humanity-backup-db.timer, every 30m)
#   copy 3: THIS PC (off-site, different failure domain) <-- this
#
# Relies on the `humanity-vps` SSH alias in ~/.ssh/config. No secrets
# in this script.
#
# SCHEDULED-TASK CONFIG (must stay SILENT): the Windows task "HumanityOS Relay
# Backup Pull" MUST run with LogonType **S4U** ("Run whether user is logged on
# or not") + Settings.Hidden = $true. Under the Interactive logon type,
# powershell.exe flashes a console window every run that STEALS FOCUS and kicks
# the operator out of whatever app is in front (reported 2026-06-28). S4U runs
# it non-interactively (no window) and key-based SSH still works headless. To
# re-apply after any task re-create (needs admin):
#   $p=New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType S4U -RunLevel Limited
#   $s=(Get-ScheduledTask -TaskName 'HumanityOS Relay Backup Pull').Settings; $s.Hidden=$true
#   Set-ScheduledTask -TaskName 'HumanityOS Relay Backup Pull' -Principal $p -Settings $s
#
# IMPLEMENTATION NOTES:
#  - Pure ASCII on purpose. PowerShell 5.1 reads script files in the
#    system codepage, not UTF-8, so any em-dash / box-drawing glyph
#    gets mangled into garbage that breaks the parser. Keep it ASCII.
#  - Deliberately does NOT set $ErrorActionPreference = "Stop": under
#    5.1 that turns native-command (ssh/scp) stderr into terminating
#    errors that abort unpredictably. We check $LASTEXITCODE +
#    Test-Path explicitly instead -- the robust pattern for native
#    tools.
#
# AT-REST: the pulled DB has all relay data. DMs are E2EE (Kyber-
# sealed; even this backup cannot expose DM plaintext), but profiles/
# messages are plaintext in the DB. Rely on this PC's disk encryption
# (BitLocker). An explicit age / 7-Zip-AES layer is a Phase B TODO.
# =====================================================================

# -- Config (override via env vars if desired) --
$SshAlias  = if ($env:HUMANITY_VPS_ALIAS)   { $env:HUMANITY_VPS_ALIAS }   else { "humanity-vps" }
$RemoteDir = "/opt/Humanity/backups"
$LocalDir  = if ($env:HUMANITY_BACKUP_DIR)  { $env:HUMANITY_BACKUP_DIR }  else { "$env:USERPROFILE\HumanityBackups" }
$KeepLocal = if ($env:HUMANITY_BACKUP_KEEP) { [int]$env:HUMANITY_BACKUP_KEEP } else { 60 }
$LogFile   = Join-Path $LocalDir "backup-pull.log"

function Write-Log($msg) {
    $line = "{0}  {1}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss"), $msg
    Write-Output $line
    try { Add-Content -Path $LogFile -Value $line -Encoding utf8 } catch {}
}

if (-not (Test-Path $LocalDir)) {
    New-Item -ItemType Directory -Force -Path $LocalDir | Out-Null
}

Write-Log "=== backup pull start (alias=$SshAlias dest=$LocalDir) ==="

# -- Find the newest relay-*.db on the VPS --
$Remote = (ssh $SshAlias "ls -1t $RemoteDir/relay-*.db 2>/dev/null | head -1")
if ($Remote) { $Remote = $Remote.Trim() }
if ([string]::IsNullOrWhiteSpace($Remote)) {
    Write-Log "ERROR: no relay-*.db found on VPS at $RemoteDir (ssh exit=$LASTEXITCODE) -- aborting"
    exit 2
}
Write-Log "newest remote backup: $Remote"

# -- Pull it, named with the LOCAL pull timestamp --
$Ts   = Get-Date -Format "yyyyMMdd-HHmmss"
$Dest = Join-Path $LocalDir "relay-$Ts.db"
scp "${SshAlias}:${Remote}" $Dest
$ScpExit = $LASTEXITCODE
if ($ScpExit -ne 0 -or -not (Test-Path $Dest)) {
    Write-Log "ERROR: scp failed (exit=$ScpExit) -- aborting"
    exit 3
}

# -- Sanity: real SQLite DB (header magic) + non-trivial size --
$Size = (Get-Item $Dest).Length
if ($Size -lt 16384) {
    Write-Log "WARN: pulled file is only $Size bytes -- suspiciously small"
}
# .NET byte read is robust across PowerShell 5.1 + 7. SQLite files
# begin with the literal ASCII 'SQLite format 3' + NUL.
$HeaderBytes = [System.IO.File]::ReadAllBytes($Dest)[0..15]
$Header = [System.Text.Encoding]::ASCII.GetString($HeaderBytes)
if ($Header -notlike "SQLite format 3*") {
    Write-Log "ERROR: not a SQLite DB (header='$Header') -- removing + aborting"
    Remove-Item $Dest -Force
    exit 4
}
$SizeMsg = "pulled OK: {0} ({1} bytes, valid SQLite header)" -f $Dest, $Size
Write-Log $SizeMsg

# -- Rotate local copies: keep newest $KeepLocal --
$All = @(Get-ChildItem (Join-Path $LocalDir "relay-*.db") | Sort-Object LastWriteTime -Descending)
if ($All.Count -gt $KeepLocal) {
    $All | Select-Object -Skip $KeepLocal | ForEach-Object {
        Remove-Item $_.FullName -Force
        Write-Log ("rotated out: {0}" -f $_.Name)
    }
}

$Count = @(Get-ChildItem (Join-Path $LocalDir "relay-*.db")).Count
Write-Log ("=== backup pull done (local copies: {0}) ===" -f $Count)
