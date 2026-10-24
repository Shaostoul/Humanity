# =====================================================================
# backup-drill-pc.ps1 -- the monthly restore drill, run from this PC
# =====================================================================
# WHY: a backup that has never been restored is a hope, not a backup.
# The operator's setup (2026-10-22, docs/admin/continuous-backup.md):
#   copy 1: the live database on each server
#   copy 2: each server streams every change, sealed with its own
#           backup key, to the OTHER server (united-humanity.us to
#           public.guide and back), which keeps ciphertext it cannot read
#   copy 3: this PC pulls a sealed snapshot from the VPS every few hours
#           (scripts/backup-relay-from-vps.ps1, the "HumanityOS Relay
#           Backup Pull" task), the offline copy
# This script drills copies 2 and 3 from here, once a month:
#   - copy 2: HumanityOS.exe --backup-drill restores the newest state
#     from the other server into a temporary file, runs SQLite's full
#     integrity check, checks the copy is no older than -MaxAgeMinutes,
#     lists the rows per table, and deletes the file;
#   - copy 3: the newest pulled relay-*.db.aes is decrypted with openssl
#     (the same tool and settings that sealed it on the VPS) into a
#     temporary file, checked with HumanityOS.exe --check-db, and deleted.
# Nothing is written anywhere but the temp folder and the log.
#
# NEEDS (once, see docs/admin/continuous-backup.md "The PC's monthly drill"):
#   -KeyDir: a folder holding COPIES of the server's data/backup.key and
#   data/backup-destination.secrets.json (default
#   %USERPROFILE%\HumanityBackups\keys\united-humanity.us). Keep it as
#   private as the backups themselves: with those two files, the backups
#   can be read.
#   Git for Windows (its openssl) for the snapshot half.
#
# SCHEDULED TASK (must stay SILENT, like the pull task: S4U + Hidden, so no
# window ever takes the focus):
#   $a=New-ScheduledTaskAction -Execute powershell.exe -Argument '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\Humanity\scripts\backup-drill-pc.ps1'
#   $t=New-ScheduledTaskTrigger -Weekly -WeeksInterval 4 -DaysOfWeek Sunday -At 10am
#   $p=New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType S4U -RunLevel Limited
#   $s=New-ScheduledTaskSettingsSet -Hidden -StartWhenAvailable
#   Register-ScheduledTask -TaskName 'HumanityOS Backup Drill' -Action $a -Trigger $t -Principal $p -Settings $s
#
# EXIT CODES (the task's "Last Run Result"):
#   0 both drills passed, 1 a restore or check failed, 2 setup missing
#   (no exe, no key folder), 7 the newest streamed copy is too old.
#
# Pure ASCII on purpose (PowerShell 5.1 reads scripts in the system
# codepage). No $ErrorActionPreference = "Stop" (native tools' stderr).
# =====================================================================

[CmdletBinding(PositionalBinding=$false)]
param(
    [string]$KeyDir = $(if ($env:HUMANITY_DRILL_KEYS) { $env:HUMANITY_DRILL_KEYS } else { "$env:USERPROFILE\HumanityBackups\keys\united-humanity.us" }),
    [string]$Exe = "",
    [int]$MaxAgeMinutes = 60,
    [string]$SnapshotDir = $(if ($env:HUMANITY_BACKUP_DIR) { $env:HUMANITY_BACKUP_DIR } else { "$env:USERPROFILE\HumanityBackups" }),
    [switch]$SkipSnapshot
)

$LogDir = "$env:USERPROFILE\HumanityBackups"
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Force -Path $LogDir | Out-Null }
$LogFile = Join-Path $LogDir "backup-drill.log"

function Write-Log($msg) {
    $line = "{0}  {1}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss"), $msg
    Write-Output $line
    try { Add-Content -Path $LogFile -Value $line -Encoding utf8 } catch {}
}

# The newest archived exe in the repo root (what `just launch` boots), else
# the release build.
if (-not $Exe) {
    $Repo = Split-Path -Parent $PSScriptRoot
    $Archived = @(Get-ChildItem -Path $Repo -Filter "v*_HumanityOS.exe" -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending)
    if ($Archived.Count -gt 0) { $Exe = $Archived[0].FullName }
    elseif (Test-Path (Join-Path $Repo "target\release\HumanityOS.exe")) { $Exe = Join-Path $Repo "target\release\HumanityOS.exe" }
}
if (-not $Exe -or -not (Test-Path $Exe)) {
    Write-Log "ERROR: no HumanityOS.exe found (pass -Exe <path>)"
    exit 2
}

Write-Log "=== backup drill start (exe=$Exe) ==="
$Result = 0

# The exe is a windowed program on Windows: Start-Process -Wait is what
# waits for it and reads its exit code. It writes its report to the log.
function Invoke-Hos([string[]]$HosArgs) {
    $p = Start-Process -FilePath $Exe -ArgumentList $HosArgs -Wait -PassThru -WindowStyle Hidden
    return $p.ExitCode
}

# -- Copy 2: the continuous backup, from the other server --
if (-not (Test-Path (Join-Path $KeyDir "backup.key")) -or -not (Test-Path (Join-Path $KeyDir "backup-destination.secrets.json"))) {
    Write-Log "ERROR: $KeyDir needs copies of the server's backup.key and backup-destination.secrets.json"
    $Result = 2
} else {
    $Code = Invoke-Hos @("--backup-drill", "--data", "`"$KeyDir`"", "--max-age-minutes", "$MaxAgeMinutes", "--log", "`"$LogFile`"")
    if ($Code -eq 0) { Write-Log "streamed copy: PASSED" }
    else {
        Write-Log "streamed copy: FAILED (exit $Code, the lines above say why)"
        if ($Result -eq 0) { $Result = $Code }
    }
}

# -- Copy 3: the newest snapshot pulled to this PC --
if (-not $SkipSnapshot) {
    $Newest = @(Get-ChildItem -Path $SnapshotDir -Filter "relay-*.db.aes" -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^relay-\d{8}-\d{6}\.db\.aes$' } | Sort-Object Name -Descending) | Select-Object -First 1
    $OpenSsl = @("$env:ProgramFiles\Git\usr\bin\openssl.exe", "$env:ProgramFiles\Git\mingw64\bin\openssl.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $Newest) {
        Write-Log "ERROR: no relay-*.db.aes in $SnapshotDir (is the pull task running?)"
        if ($Result -eq 0) { $Result = 1 }
    } elseif (-not $OpenSsl) {
        Write-Log "ERROR: openssl not found (install Git for Windows), snapshot not drilled"
        if ($Result -eq 0) { $Result = 2 }
    } else {
        $Tmp = Join-Path $env:TEMP ("hos-snapshot-drill-{0}" -f $PID)
        New-Item -ItemType Directory -Force -Path $Tmp | Out-Null
        $Plain = Join-Path $Tmp "snapshot.db"
        & $OpenSsl enc -d -aes-256-cbc -pbkdf2 -iter 200000 -pass ("file:" + (Join-Path $KeyDir "backup.key")) -in $Newest.FullName -out $Plain 2>$null
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path $Plain)) {
            Write-Log ("snapshot {0}: FAILED to decrypt (is the key in {1} the server's backup.key?)" -f $Newest.Name, $KeyDir)
            if ($Result -eq 0) { $Result = 1 }
        } else {
            $Code = Invoke-Hos @("--check-db", "`"$Plain`"", "--log", "`"$LogFile`"")
            if ($Code -eq 0) { Write-Log ("snapshot {0}: PASSED" -f $Newest.Name) }
            else {
                Write-Log ("snapshot {0}: FAILED (exit {1})" -f $Newest.Name, $Code)
                if ($Result -eq 0) { $Result = 1 }
            }
        }
        Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
    }
}

if ($Result -eq 0) { Write-Log "=== backup drill PASSED ===" } else { Write-Log "=== backup drill FAILED (exit $Result) ===" }
exit $Result
