param([int]$ProcessId = 0, [string]$TitlePrefix = "HumanityOS", [switch]$Taskbar)
# What a game's windows report about themselves, read from OUTSIDE the game (2026-11-14): whether
# each top-level window of the process is visible, minimized, DWM-cloaked (DwmGetWindowAttribute
# DWMWA_CLOAKED), a tool window (GWL_EXSTYLE), and its client size; with -Taskbar, the taskbar
# buttons whose name holds one of those windows' titles (UI Automation over Shell_TrayWnd).
# Called by scripts/lib/window-state.js (the rigs' check that a script's boot is hidden from the
# operator's screen, src/engine/rig_window.rs). Prints one JSON object.
#
# PRIVACY: only windows of the given process are read, and only taskbar buttons that name one of
# them are printed; the operator's other windows and buttons are never listed.
$ErrorActionPreference = "Stop"
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class HosWindowState {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int s);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static List<IntPtr> Of(uint pid) {
    var list = new List<IntPtr>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid) list.Add(h); return true; }, IntPtr.Zero);
    return list;
  }
  public static string Title(IntPtr h) { var s = new StringBuilder(512); GetWindowTextW(h, s, 512); return s.ToString(); }
}
"@
# Every element's name under the taskbar (Shell_TrayWnd), read through UI Automation. The assemblies
# are loaded at SCRIPT level: loaded inside the function (or used through a type held in a variable,
# $A::RootElement) the read saw only 11 of the taskbar's 24 elements and none of the window buttons
# (measured 2026-11-14, a count of "running window" buttons: 0 that way, 2 this way). Names, not a
# ControlType condition: [ControlType]::Button came back empty and then matched nothing.
if ($Taskbar) {
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
}
function Get-TaskbarNames {
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ClassNameProperty, "Shell_TrayWnd")
  $tray = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
  if ($null -eq $tray) { throw "no Shell_TrayWnd (is Explorer running?)" }
  $all = $tray.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
  $out = @()
  foreach ($b in $all) { $n = $b.Current.Name; if ($n) { $out += $n } }
  return $out
}
$wins = @()
foreach ($h in [HosWindowState]::Of([uint32]$ProcessId)) {
  $title = [HosWindowState]::Title($h)
  if (-not $title.StartsWith($TitlePrefix)) { continue }
  $cloaked = 0
  $hr = [HosWindowState]::DwmGetWindowAttribute($h, 14, [ref]$cloaked, 4)
  if ($hr -ne 0) { $cloaked = -1 }
  $style = [HosWindowState]::GetWindowLongW($h, -16)
  $ex = [HosWindowState]::GetWindowLongW($h, -20)
  $c = New-Object HosWindowState+RECT
  [void][HosWindowState]::GetClientRect($h, [ref]$c)
  $r = New-Object HosWindowState+RECT
  [void][HosWindowState]::GetWindowRect($h, [ref]$r)
  $wins += [ordered]@{
    hwnd = $h.ToInt64()
    title = $title
    visible = [HosWindowState]::IsWindowVisible($h)
    minimized = [HosWindowState]::IsIconic($h)
    foreground = ([HosWindowState]::GetForegroundWindow() -eq $h)
    cloaked = $cloaked
    style = ('0x{0:X8}' -f $style)
    ex_style = ('0x{0:X8}' -f $ex)
    tool_window = (($ex -band 0x80) -ne 0)
    app_window = (($ex -band 0x40000) -ne 0)
    client = @(($c.R - $c.L), ($c.B - $c.T))
    rect = @($r.L, $r.T, $r.R, $r.B)
  }
}
$tb = [ordered]@{ read = $false; buttons = @(); window_buttons_seen = 0; error = $null }
if ($Taskbar) {
  try {
    $names = @(Get-TaskbarNames)
    $titles = @($wins | ForEach-Object { $_.title })
    # The positive control: how many window buttons the read saw at all (a COUNT, no names), so "no
    # button names the game" is known to come from a read that sees buttons.
    $windowButtons = 0
    foreach ($n in $names) {
      if ($n -like "*running window*") { $windowButtons++ }
      foreach ($t in $titles) { if ($t -and $n.Contains($t)) { $tb.buttons += $n; break } }
    }
    $tb.window_buttons_seen = $windowButtons
    $tb.read = $true
  } catch {
    $tb.error = $_.Exception.Message
  }
}
[ordered]@{ pid = $ProcessId; windows = $wins; taskbar = $tb } | ConvertTo-Json -Depth 5 -Compress
