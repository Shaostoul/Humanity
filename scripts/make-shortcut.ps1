$ws = New-Object -ComObject WScript.Shell
$desktop = [Environment]::GetFolderPath('Desktop')
$sc = $ws.CreateShortcut("$desktop\HumanityOS.lnk")
$sc.TargetPath = "cmd.exe"
$sc.Arguments = "/C `"C:\Humanity\scripts\launch.bat`""
$sc.IconLocation = "C:\Humanity\assets\icon.ico,0"
$sc.WorkingDirectory = "C:\Humanity\scripts"
$sc.WindowStyle = 7
$sc.Description = "Launch latest HumanityOS build"
$sc.Save()
Write-Host "Shortcut created on Desktop with H icon, targeting cmd.exe (pinnable)"
