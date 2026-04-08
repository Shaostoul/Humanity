#!/usr/bin/env node
// Creates a Windows shortcut (.lnk) for HumanityOS launcher with the H icon.
// Usage: node create-shortcut.js [target-folder]
// Default target: Desktop

const { execSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const targetDir = process.argv[2] || path.join(process.env.USERPROFILE, "Desktop");
const shortcutPath = path.join(targetDir, "HumanityOS.lnk");
const batPath = path.resolve(__dirname, "launch.bat");
const icoPath = path.resolve(__dirname, "..", "assets", "icon.ico");

if (!fs.existsSync(batPath)) {
  console.error("ERROR: launch.bat not found at", batPath);
  process.exit(1);
}
if (!fs.existsSync(icoPath)) {
  console.error("ERROR: icon.ico not found at", icoPath);
  process.exit(1);
}

// Use PowerShell to create a .lnk shortcut (only reliable way on Windows)
// TargetPath = cmd.exe so Windows allows pinning to taskbar
// /C runs the bat then closes the cmd window
const ps = `
$ws = New-Object -ComObject WScript.Shell
$sc = $ws.CreateShortcut('${shortcutPath.replace(/'/g, "''")}')
$sc.TargetPath = 'cmd.exe'
$sc.Arguments = '/C ""${batPath.replace(/'/g, "''")}""'
$sc.IconLocation = '${icoPath.replace(/'/g, "''")},0'
$sc.WorkingDirectory = '${path.dirname(batPath).replace(/'/g, "''")}'
$sc.WindowStyle = 7
$sc.Description = 'Launch latest HumanityOS build'
$sc.Save()
`.trim();

try {
  execSync(`powershell -Command "${ps.replace(/"/g, '\\"')}"`, { stdio: "inherit" });
  console.log(`Shortcut created: ${shortcutPath}`);
  console.log(`Icon: ${icoPath}`);
  console.log(`\nPin this to your taskbar: right-click → Pin to taskbar`);
} catch (e) {
  console.error("Failed to create shortcut:", e.message);
  process.exit(1);
}
