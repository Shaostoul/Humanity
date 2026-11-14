// Is a script's game boot hidden from the operator's screen? Read from OUTSIDE the game (2026-11-14).
//
// src/engine/rig_window.rs hides a developer's quiet boot: DWM-cloaked ("not visible to the user",
// still composed, so every frame is drawn), a tool window (no taskbar button, no Alt+Tab entry), the
// ordinary-size client kept. This module asks the game's windows themselves, through
// window-state.ps1 (DwmGetWindowAttribute DWMWA_CLOAKED, GWL_EXSTYLE, the client rect, and with
// `taskbar` the taskbar buttons that name the window through UI Automation), so the check measures
// what Windows reports, not what the game says it asked for.
//
//   node scripts/lib/window-state.js <pid> [--taskbar]   one JSON line and a verdict
//
// Used by probe-sweep after world entry (every rig boot through it says whether it was hidden) and
// for the proof in docs/ai/game-boots.md.

const path = require("path");
const { execFileSync } = require("child_process");

const PS1 = path.join(__dirname, "window-state.ps1");

/** The raw report for `pid`: { pid, windows: [...], taskbar: { read, buttons, error } }, or { error }. */
function readWindowState(pid, { taskbar = false, timeoutMs = 30000 } = {}) {
  const args = ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", PS1, "-ProcessId", String(Number(pid))];
  if (taskbar) args.push("-Taskbar");
  try {
    const out = execFileSync("powershell", args, { encoding: "utf8", timeout: timeoutMs, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    const r = JSON.parse(out.trim().split(/\r?\n/).pop());
    r.windows = [].concat(r.windows || []);
    if (r.taskbar) r.taskbar.buttons = [].concat(r.taskbar.buttons || []);
    return r;
  } catch (e) {
    return { pid: Number(pid), windows: [], taskbar: null, error: String((e && e.message) || e).split(/\r?\n/)[0] };
  }
}

/** The game's main window: the one with a caption (WS_CAPTION 0x00C00000); else the first titled one. */
function mainWindow(report) {
  const ws = (report && report.windows) || [];
  return ws.find((w) => (parseInt(w.style, 16) & 0x00c00000) === 0x00c00000) || ws[0] || null;
}

/**
 * The verdict, pure: { hidden, on_screen, taskbar_button, why }.
 *   hidden          cloaked, a tool window, not forced onto the taskbar (and visible to Windows, so
 *                   still presented: a minimized or SW_HIDE window is not "hidden", it stops drawing)
 *   taskbar_button  true / false when the taskbar was read, null when it was not
 */
function judge(report) {
  const w = mainWindow(report);
  if (!w) return { hidden: null, on_screen: null, taskbar_button: null, why: report && report.error ? `could not read the window (${report.error})` : "no window titled HumanityOS for that process" };
  // A taskbar read counts only when it saw window buttons at all (window_buttons_seen, a count with no
  // names): a read that sees none says nothing about the game's.
  const t = report.taskbar;
  const tb = t && t.read && (t.buttons.length > 0 || Number(t.window_buttons_seen) > 0) ? t.buttons.length > 0 : null;
  const cloaked = Number(w.cloaked) > 0;
  const hidden = cloaked && !!w.tool_window && !w.app_window && !!w.visible && !w.minimized && tb !== true;
  const why = [
    `cloaked=${w.cloaked}`,
    `ex_style=${w.ex_style}${w.tool_window ? " (tool window)" : ""}${w.app_window ? " (app window: forced onto the taskbar)" : ""}`,
    `visible=${w.visible}`,
    `minimized=${w.minimized}`,
    `foreground=${w.foreground}`,
    `client ${w.client[0]}x${w.client[1]}`,
    tb === null
      ? `taskbar not read${t && t.error ? ` (${t.error})` : t && t.read ? " (the read saw no window buttons at all)" : ""}`
      : `taskbar buttons naming it: ${t.buttons.length} (of ${t.window_buttons_seen} window buttons seen)`,
  ].join(", ");
  return { hidden, on_screen: !cloaked && !!w.visible && !w.minimized, taskbar_button: tb, client: w.client, why };
}

if (require.main === module) {
  const pid = Number(process.argv[2]);
  if (!pid) {
    console.error("usage: node scripts/lib/window-state.js <pid> [--taskbar]");
    process.exit(1);
  }
  const r = readWindowState(pid, { taskbar: process.argv.includes("--taskbar") });
  console.log(JSON.stringify(r));
  const v = judge(r);
  console.log(`${v.hidden ? "HIDDEN" : v.on_screen ? "ON SCREEN" : "NOT HIDDEN"}: ${v.why}`);
  process.exit(v.hidden ? 0 : 2);
}

module.exports = { readWindowState, mainWindow, judge, PS1 };
