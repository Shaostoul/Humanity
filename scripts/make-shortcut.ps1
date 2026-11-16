$ws = New-Object -ComObject WScript.Shell
$desktop = [Environment]::GetFolderPath('Desktop')
$sc = $ws.CreateShortcut("$desktop\HumanityOS.lnk")
$sc.TargetPath = "cmd.exe"
$repo = Split-Path -Parent $PSScriptRoot
$sc.Arguments = "/C `"$repo\scripts\launch.bat`""
$sc.IconLocation = "$repo\assets\icon.ico,0"
$sc.WorkingDirectory = "$repo\scripts"
$sc.WindowStyle = 7
$sc.Description = "Launch latest HumanityOS build"
$sc.Save()
Write-Host "Shortcut created on Desktop with H icon, targeting cmd.exe (pinnable)"
