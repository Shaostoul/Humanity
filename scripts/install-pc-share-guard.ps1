# Installs the "HumanityOS PC Share Guard" scheduled task (2026-10-22): starts
# scripts/pc-share-guard.ps1 at every logon and keeps it running, silently, so the development
# builds and rigs always run at below-normal priority and off the last CPU core (the operator's).
#
# Needs admin (a UAC prompt) for the same reason as the disk guard: LogonType S4U and Hidden is
# the only combination that does not flash a console window over whatever the operator is doing.
#
# Run it with `just install-share-guard`. Running it again replaces the task.
# Remove it with: Unregister-ScheduledTask -TaskName 'HumanityOS PC Share Guard'

$ErrorActionPreference = 'Stop'
$name = 'HumanityOS PC Share Guard'
$repo = Split-Path -Parent $PSScriptRoot
$script = Join-Path $PSScriptRoot 'pc-share-guard.ps1'
$ps = (Get-Command powershell -ErrorAction Stop).Source

$action = New-ScheduledTaskAction -Execute $ps -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $script + '"') -WorkingDirectory $repo
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
# No time limit: it runs for the whole session. Restart it if it ever stops.
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Seconds 0) -MultipleInstances IgnoreNew -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
$settings.Hidden = $true
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType S4U -RunLevel Limited

Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
Start-ScheduledTask -TaskName $name
Start-Sleep -Seconds 5
$info = Get-ScheduledTaskInfo -TaskName $name
Write-Host ("Installed '{0}': last result {1} (267009 means it is running)" -f $name, $info.LastTaskResult)
Start-Sleep -Seconds 3
