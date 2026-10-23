# The PC share guard (2026-10-22): keeps the development work out of the operator's way.
#
# The operator, 2026-10-22: "could we dedicate like 1 CPU core and 1 GPU core to whatever I want
# to do on my PC, like play Helldivers 2 or watch YouTube, use discord, whatever. The rest for
# you but, enough for me to still comfortably use my PC."
#
# Every few seconds this finds the development processes (cargo, rustc, the linker, the C
# compilers and build scripts they start, sccache, test programs, and the copies of HumanityOS
# that the verification rigs boot) and:
#   1. moves them to "below normal" priority, so whenever the operator's programs want the CPU,
#      Windows serves them first;
#   2. keeps them off the LAST physical core (its two logical processors), so that core is the
#      operator's alone however busy the builds are.
# Windows hands both settings on to the processes a build starts after this has seen it, so a
# build is covered from its first few seconds.
#
# The GPU cannot be split this way on a GeForce card under Windows; the GPU side is the rigs'
# machine guard, which waits while one of the operator's listed programs runs
# (scripts/lib/operator-apps.json).
#
# It never touches the operator's own HumanityOS (the repo root's HumanityOS.exe or a
# v<version>_HumanityOS.exe archive): only copies under .probe-rig, a worktree, or a target folder.
#
# Run it by hand with `just share-guard` (Ctrl+C stops it), or install it to start at every
# logon with `just install-share-guard` (one UAC prompt). It writes what it did once a minute to
# %LOCALAPPDATA%\HumanityOS\share-guard.json.

param([int]$IntervalSeconds = 3, [switch]$Once)

$ErrorActionPreference = 'SilentlyContinue'

$logical = [Environment]::ProcessorCount
# Keep two logical processors (one hyperthreaded core) free when there are at least four; on a
# smaller machine keep one.
$keep = if ($logical -ge 4) { 2 } else { 1 }
$mask = [Int64]((([Int64]1) -shl ($logical - $keep)) - 1)

# Development programs by name (without .exe). Test programs are matched by pattern below.
$names = @('cargo', 'rustc', 'sccache', 'link', 'lld-link', 'rust-lld', 'cl', 'cc', 'cmake',
           'ninja', 'nasm', 'build-script-build', 'build-script-main', 'mspdbsrv', 'rustdoc',
           'clippy-driver', 'wasm-bindgen', 'wasm-opt',
           # the build slot (scripts/build-slot/): it runs rustc as its child, so the compile it starts
           # inherits whatever this sets on it
           'hos-build-slot')

$statusDir = Join-Path $env:LOCALAPPDATA 'HumanityOS'
New-Item -ItemType Directory -Force -Path $statusDir | Out-Null
$statusFile = Join-Path $statusDir 'share-guard.json'
# Processes a test starts and must measure untouched (scripts/tests/machine-guard.test.js starts a
# real rustc to prove the capture hold lowers and restores it): it lists their pids here, and
# this leaves them alone. {"pids": [1234, 5678]}; a missing or broken file means none.
# Every share-guard-ignore*.json counts (2026-10-23): each test process writes a file of its own,
# share-guard-ignore-<its pid>.json, because two test runs at once (two sessions' rig-tests)
# shared the one file and the first to finish deleted the other's pids, which raced.
function Get-IgnoredPids {
    $pids = @()
    foreach ($f in Get-ChildItem -Path $statusDir -Filter 'share-guard-ignore*.json' -ErrorAction SilentlyContinue) {
        try {
            $j = Get-Content -Raw -Path $f.FullName | ConvertFrom-Json
            $pids += @($j.pids | ForEach-Object { [int]$_ })
        } catch { }
    }
    return $pids
}

function Test-DevProcess($p) {
    $name = $p.ProcessName
    if ($names -contains $name) { return $true }
    # Cargo's test and bench programs: <crate>-<16 hex>.exe, run from a target folder.
    $path = $p.Path
    if (-not $path) { return $false }
    if ($name -match '-[0-9a-f]{16}$' -and $path -match '\\target\\') { return $true }
    # HumanityOS copies the rigs boot; never the operator's own.
    if ($name -match 'HumanityOS$') {
        if ($path -match '\\\.probe-rig\\' -or $path -match '\\\.claude\\worktrees\\' -or $path -match '\\target\\(debug|release)\\') {
            return $true
        }
    }
    return $false
}

$adjusted = 0
$lastStatus = [DateTime]::MinValue
while ($true) {
    $seen = 0
    $ignored = Get-IgnoredPids
    foreach ($p in Get-Process) {
        if ($ignored -contains $p.Id) { continue }
        if (-not (Test-DevProcess $p)) { continue }
        $seen++
        try {
            if ($p.PriorityClass -ne 'BelowNormal' -and $p.PriorityClass -ne 'Idle') {
                $p.PriorityClass = 'BelowNormal'
                $adjusted++
            }
            if ([Int64]$p.ProcessorAffinity -ne $mask) {
                $p.ProcessorAffinity = [IntPtr]$mask
            }
        } catch { }
    }
    $now = Get-Date
    if (($now - $lastStatus).TotalSeconds -ge 60 -or $Once) {
        $status = @{
            at = $now.ToString('o')
            logical_processors = $logical
            kept_for_the_operator = $keep
            affinity_mask = ('0x{0:X}' -f $mask)
            dev_processes_now = $seen
            priority_changes_since_start = $adjusted
        }
        ($status | ConvertTo-Json) | Set-Content -Path $statusFile -Encoding utf8
        $lastStatus = $now
    }
    if ($Once) { break }
    Start-Sleep -Seconds $IntervalSeconds
}
