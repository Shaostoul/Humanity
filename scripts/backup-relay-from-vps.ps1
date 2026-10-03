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
# 2026-10-02 FINDING -- 39 DAYS OF "pulled OK" ON ONE FOSSIL FILE:
#   Since 2026-08-24 the VPS script (scripts/humanity-backup-db.sh, its
#   "Encryption at rest" block) seals every 30-minute snapshot to
#   relay-<UTC ts>.db.aes and deletes the plain .db. This script still
#   listed only relay-*.db, so the newest match stayed
#   relay-20260824-000317.db for 39 days. Every scheduled pull re-fetched
#   that one file, the SQLite header check passed (it WAS a valid DB),
#   and the log said "pulled OK". All 60 local copies were byte-identical.
#   Same shape as the 2026-05-21 incident written up in
#   humanity-backup-db.sh: a check that cannot fail. Now the script:
#     - pulls the newest relay-*.db.aes (picked by the UTC stamp in its
#       NAME, the same thing the freshness gate judges),
#     - checks the openssl "Salted__" header instead of the SQLite one,
#     - FAILS (ERROR line + non-zero exit) when that snapshot's name
#       stamp is older than $MaxAgeHours (default 2), and when the pull
#       is byte-identical (SHA-256) to the previous pull,
#     - rotates only *.db.aes. The 60 old plain relay-*.db copies in
#       the backup folder are left alone: deleting them, and the 15 the
#       VPS still keeps, is the operator's decision.
#   Prove the gate can fail without touching the VPS:
#     powershell -NoProfile -ExecutionPolicy Bypass -File scripts\backup-relay-from-vps.ps1 -GateTestName relay-20260824-000317.db.aes
#   (exits 7 and writes nothing; -GateTestName runs the same function
#   the real pull uses, then stops).
#
# EXIT CODES (the scheduled task's "Last Run Result" shows them):
#   0 ok, 2 no relay-*.db.aes on the VPS, 3 scp failed,
#   4 not an openssl-sealed file, 5 snapshot name has no UTC stamp,
#   6 byte-identical to the previous pull (duplicate removed),
#   7 newest VPS snapshot is stale (pulled + kept, run still FAILS),
#   8 the copy does not match the VPS file (taken mid-seal; removed).
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
# AT-REST: since 2026-08-24 the pulled file is ciphertext (openssl
# AES-256-CBC + PBKDF2, sealed on the VPS with /opt/Humanity/data/
# backup.key). DMs inside were already E2EE (Kyber-sealed); now the
# profiles/messages are sealed too. The key deliberately does NOT
# travel with the backups, so a restore from THIS copy needs a copy of
# backup.key kept somewhere off the VPS: if the VPS is lost and so is
# the key, these files cannot be opened. Decrypt with
# scripts/decrypt-backup.sh <file.db.aes> <key-file> (Git Bash ships
# openssl).
# =====================================================================

# Named parameters only: a stray argument must never turn the scheduled
# pull into a gate-only test.
[CmdletBinding(PositionalBinding=$false)]
param(
    # Run ONLY the freshness gate against this snapshot file name (no
    # ssh, no scp, nothing written, not even the log) and exit with the
    # code a real run would use. The proof that the gate can fail.
    [string]$GateTestName = "",
    # A newest VPS snapshot older than this (by the UTC stamp in its
    # name) fails the run. The VPS timer makes one every 30 minutes.
    [double]$MaxAgeHours = $(if ($env:HUMANITY_BACKUP_MAX_AGE_HOURS) { [double]$env:HUMANITY_BACKUP_MAX_AGE_HOURS } else { 2 })
)

# -- Config (override via env vars if desired) --
$SshAlias  = if ($env:HUMANITY_VPS_ALIAS)   { $env:HUMANITY_VPS_ALIAS }   else { "humanity-vps" }
$RemoteDir = "/opt/Humanity/backups"
$LocalDir  = if ($env:HUMANITY_BACKUP_DIR)  { $env:HUMANITY_BACKUP_DIR }  else { "$env:USERPROFILE\HumanityBackups" }
$KeepLocal = if ($env:HUMANITY_BACKUP_KEEP) { [int]$env:HUMANITY_BACKUP_KEEP } else { 60 }
$LogFile   = Join-Path $LocalDir "backup-pull.log"

# The one shape a sealed snapshot name has, on the VPS and here:
# relay-YYYYMMDD-HHMMSS.db.aes. The VPS stamp is UTC (`date -u`); the
# local stamp is this PC's pull time, as before.
$SealedName = '^relay-(\d{8}-\d{6})\.db\.aes$'

function Write-Log($msg) {
    $line = "{0}  {1}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss"), $msg
    Write-Output $line
    if ($LogFile) {
        try { Add-Content -Path $LogFile -Value $line -Encoding utf8 } catch {}
    }
}

# -- Freshness gate (2026-10-02) --
# Judges a snapshot by the UTC stamp the VPS script writes into its
# name. Returns 0 = fresh, 5 = the name carries no stamp, 7 = older
# than $MaxAgeHours, and logs its own verdict. The real pull and
# -GateTestName both call THIS function, so the test exercises exactly
# the code that guards the scheduled pull. (Its log lines go to the
# host so the function's output is only the code.)
function Test-SnapshotGate([string]$Name) {
    $Leaf = ($Name -split '/')[-1]
    if ($Leaf -notmatch $SealedName) {
        Write-Log "ERROR: snapshot name '$Leaf' has no relay-YYYYMMDD-HHMMSS.db.aes UTC stamp -- cannot judge its age" | Out-Host
        return 5
    }
    try {
        $Stamp = [DateTime]::ParseExact($Matches[1], "yyyyMMdd-HHmmss",
            [System.Globalization.CultureInfo]::InvariantCulture,
            [System.Globalization.DateTimeStyles]"AssumeUniversal, AdjustToUniversal")
    } catch {
        Write-Log "ERROR: snapshot name '$Leaf' has an unreadable UTC stamp -- cannot judge its age" | Out-Host
        return 5
    }
    $AgeH = ([DateTime]::UtcNow - $Stamp).TotalHours
    if ($AgeH -gt $MaxAgeHours) {
        Write-Log ("ERROR: newest VPS snapshot {0} is {1:N2} h old (limit {2} h): the VPS 30-minute backup timer (humanity-backup-db.timer) has stopped or is failing, or the VPS has stopped sealing to .db.aes (is /opt/Humanity/data/backup.key still there?)" -f $Leaf, $AgeH, $MaxAgeHours) | Out-Host
        return 7
    }
    # A stamp from the future means a clock is wrong somewhere, which would
    # also hide a stale snapshot. Fifteen minutes of slack for clock drift.
    if ($AgeH -lt -0.25) {
        Write-Log ("ERROR: newest VPS snapshot {0} is stamped {1:N2} h in the future: a clock is wrong on this PC or the VPS" -f $Leaf, (-$AgeH)) | Out-Host
        return 7
    }
    Write-Log ("snapshot age OK: {0} is {1:N2} h old (limit {2} h)" -f $Leaf, $AgeH, $MaxAgeHours) | Out-Host
    return 0
}

# Local sealed copies, newest first. Sorted by NAME (the pull stamp),
# not LastWriteTime, so copying the folder elsewhere keeps the order.
function Get-LocalSealed {
    @(Get-ChildItem -Path $LocalDir -Filter "relay-*.db.aes" -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match $SealedName } |
        Sort-Object Name -Descending)
}

if ($GateTestName) {
    $LogFile = $null   # a test must not write into the operator's backup log
    Write-Output ("gate test: name={0} max_age_h={1} (no ssh, no scp, nothing written)" -f $GateTestName, $MaxAgeHours)
    $Code = Test-SnapshotGate $GateTestName
    Write-Output ("gate test: exit {0}" -f $Code)
    exit $Code
}

if (-not (Test-Path $LocalDir)) {
    New-Item -ItemType Directory -Force -Path $LocalDir | Out-Null
}

Write-Log "=== backup pull start (alias=$SshAlias dest=$LocalDir) ==="

# -- Find the newest relay-*.db.aes on the VPS --
# Newest by NAME (the UTC stamp), since that stamp is what the gate
# below judges. Plain relay-*.db files there are pre-2026-08-24 fossils.
# Stamped names only (relay-YYYYMMDD-HHMMSS.db.aes): a hand-made name like
# relay-pre-wipe.db.aes would sort after the digits and win every time.
$Remote = (ssh $SshAlias "ls -1 $RemoteDir/relay-[0-9]*-[0-9]*.db.aes 2>/dev/null | sort | tail -1")
if ($Remote) { $Remote = "$Remote".Trim() }
if ([string]::IsNullOrWhiteSpace($Remote)) {
    Write-Log "ERROR: no relay-*.db.aes found on VPS at $RemoteDir (ssh exit=$LASTEXITCODE) -- aborting"
    exit 2
}
Write-Log "newest remote backup: $Remote"

# -- Freshness gate. A stale snapshot is still pulled (it may be the
# freshest copy that exists), but the run ends in failure, below. --
$Gate = Test-SnapshotGate $Remote
if ($Gate -eq 5) { exit 5 }

# The previous pull, for the byte-identical check (taken BEFORE this
# pull lands so it can never be compared with itself).
$Prev = Get-LocalSealed | Select-Object -First 1

# -- Pull it, named with the LOCAL pull timestamp --
$Ts   = Get-Date -Format "yyyyMMdd-HHmmss"
$Dest = Join-Path $LocalDir "relay-$Ts.db.aes"
scp "${SshAlias}:${Remote}" $Dest
$ScpExit = $LASTEXITCODE
if ($ScpExit -ne 0 -or -not (Test-Path $Dest)) {
    Write-Log "ERROR: scp failed (exit=$ScpExit) -- aborting"
    exit 3
}

# -- Sanity: openssl-sealed (header magic) + non-trivial size --
$Size = (Get-Item $Dest).Length
if ($Size -lt 16384) {
    Write-Log "WARN: pulled file is only $Size bytes -- suspiciously small"
}
# `openssl enc` with a salt (the PBKDF2 default) writes the literal
# ASCII 'Salted__' then the 8-byte salt, then ciphertext. A plain
# SQLite file, an HTML error page or an empty file all fail this.
# .NET byte read is robust across PowerShell 5.1 + 7.
$Buf = New-Object byte[] 8
$Fs = [System.IO.File]::OpenRead($Dest)
try { $Got = $Fs.Read($Buf, 0, 8) } finally { $Fs.Dispose() }
$Header = [System.Text.Encoding]::ASCII.GetString($Buf, 0, $Got)
if ($Header -cne "Salted__") {
    Write-Log "ERROR: not an openssl-sealed backup (header='$Header', expected 'Salted__') -- removing + aborting"
    Remove-Item $Dest -Force
    exit 4
}

# -- Byte-identical to the previous pull? Then nothing new reached this
# PC (the 39-day failure above). Each VPS seal uses a fresh random salt,
# so two DIFFERENT snapshots never hash alike, even of an unchanged DB. --
$Hash = (Get-FileHash -Path $Dest -Algorithm SHA256).Hash

# -- The copy is the whole snapshot: same SHA-256 as the VPS file. The VPS
# seals in place, so a pull that lands mid-seal copies a truncated file that
# still starts with 'Salted__' and differs from the last pull (reviewer,
# 2026-10-02); this is the only check that catches it. --
$RemoteHash = ((ssh $SshAlias "sha256sum '$Remote'") -split '\s+')[0]
if (-not $RemoteHash -or $RemoteHash.ToUpper() -ne $Hash) {
    Write-Log ("ERROR: the copy does not match the VPS file (local sha256 {0}, VPS {1}): taken while it was still being written? -- removing + failing; the next run retries" -f $Hash, $RemoteHash)
    Remove-Item $Dest -Force
    exit 8
}

if ($Prev) {
    $PrevHash = (Get-FileHash -Path $Prev.FullName -Algorithm SHA256).Hash
    if ($Hash -eq $PrevHash) {
        Write-Log ("ERROR: pulled file is byte-identical (SHA-256 {0}) to the previous pull {1}: no new snapshot reached this PC -- removing the duplicate + failing" -f $Hash, $Prev.Name)
        Remove-Item $Dest -Force
        exit 6
    }
} else {
    Write-Log "NOTE: no earlier relay-*.db.aes here to compare with (first sealed pull); the byte-identical check applies from the next run"
}

$SizeMsg = "pulled OK: {0} ({1} bytes, sealed 'Salted__' header, sha256 {2})" -f $Dest, $Size, $Hash
Write-Log $SizeMsg

# -- Rotate local sealed copies: keep newest $KeepLocal --
# Only *.db.aes. The plain relay-*.db files from before 2026-08-24 are
# never touched here (the operator decides whether to delete them).
$All = Get-LocalSealed
if ($All.Count -gt $KeepLocal) {
    $All | Select-Object -Skip $KeepLocal | ForEach-Object {
        Remove-Item $_.FullName -Force
        Write-Log ("rotated out: {0}" -f $_.Name)
    }
}

$Plain = @(Get-ChildItem -Path $LocalDir -Filter "relay-*.db" -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match '^relay-\d{8}-\d{6}\.db$' }).Count
if ($Plain -gt 0) {
    Write-Log ("older plain relay-*.db copies left untouched: {0} (from before the 2026-08-24 switch to sealed snapshots; deleting them is the operator's call)" -f $Plain)
}

$Count = @(Get-LocalSealed).Count
if ($Gate -eq 7) {
    Write-Log ("=== backup pull done WITH ERROR: stale VPS snapshot (local copies: {0}) ===" -f $Count)
    exit 7
}
Write-Log ("=== backup pull done (local copies: {0}) ===" -f $Count)
