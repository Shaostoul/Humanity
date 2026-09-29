# Installs the "HumanityOS Dev Disk Guard" scheduled task (2026-09-29): runs
# scripts/pc-disk-guard.js every hour, silently, so build output can never fill
# C: again (it reached 91% on 2026-09-29 with 1.13 TB of cargo build caches).
#
# Needs admin (a UAC prompt): the task runs with LogonType S4U ("run whether
# the user is logged on or not") and Hidden, the only combination that does
# not flash a console window over whatever the operator is doing (the same
# fix as the "HumanityOS Relay Backup Pull" task, 2026-06-28).
#
# Run it with `just install-disk-guard`. Running it again replaces the task.
# Remove it with: Unregister-ScheduledTask -TaskName 'HumanityOS Dev Disk Guard'

$ErrorActionPreference = 'Stop'
$name = 'HumanityOS Dev Disk Guard'
$repo = Split-Path -Parent $PSScriptRoot
$script = Join-Path $PSScriptRoot 'pc-disk-guard.js'
$node = (Get-Command node -ErrorAction Stop).Source

$action = New-ScheduledTaskAction -Execute $node -Argument ('"' + $script + '"') -WorkingDirectory $repo
# Every hour, every day: a daily trigger repeating hourly for a day (the form
# Windows PowerShell 5.1 accepts; an "indefinitely" duration does not register).
$trigger = New-ScheduledTaskTrigger -Daily -At '00:05'
$trigger.Repetition = (New-ScheduledTaskTrigger -Once -At '00:05' -RepetitionInterval (New-TimeSpan -Hours 1) -RepetitionDuration (New-TimeSpan -Days 1)).Repetition
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 2) -MultipleInstances IgnoreNew
$settings.Hidden = $true
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType S4U -RunLevel Limited

Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
Start-ScheduledTask -TaskName $name
Start-Sleep -Seconds 5
$info = Get-ScheduledTaskInfo -TaskName $name
Write-Host ("Installed '{0}': last result {1}, next run {2}" -f $name, $info.LastTaskResult, $info.NextRunTime)
Start-Sleep -Seconds 3
