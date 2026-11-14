// The verdict on a script boot's window (scripts/lib/window-state.js judge, 2026-11-14): hidden only
// when Windows reports it cloaked AND a tool window AND not forced onto the taskbar AND still shown
// to Windows (a minimized window stops presenting, so it is not "hidden", it is broken). Pure node.
//
// RED FIRST (2026-11-14): with judge() reduced to "cloaked > 0" (the first idea: a DWM cloak alone,
// which the measurement showed still leaves a taskbar button), the test failed with
//   AssertionError: cloaked but forced onto the taskbar is NOT hidden
// and with the window_buttons_seen guard taken out of judge():
//   AssertionError: a taskbar read that saw no window buttons at all proves nothing

const { test } = require("node:test");
const assert = require("node:assert");
const WS = require("../lib/window-state.js");

const win = (o) => ({ title: "HumanityOS v0.1", style: "0x16CF0000", visible: true, minimized: false, foreground: false, cloaked: 1, ex_style: "0x00000190", tool_window: true, app_window: false, client: [2560, 1369], ...o });
const report = (w, taskbar = { read: true, buttons: [], window_buttons_seen: 5 }) => ({ pid: 1, windows: [w], taskbar });

test("cloaked, a tool window, no taskbar button: hidden", () => {
  const v = WS.judge(report(win({})));
  assert.strictEqual(v.hidden, true, v.why);
  assert.strictEqual(v.on_screen, false);
  assert.strictEqual(v.taskbar_button, false);
});

test("cloaked but forced onto the taskbar is NOT hidden", () => {
  assert.strictEqual(WS.judge(report(win({ ex_style: "0x00040110", tool_window: false, app_window: true }))).hidden, false, "cloaked but forced onto the taskbar is NOT hidden");
});

test("not cloaked: on the screen", () => {
  const v = WS.judge(report(win({ cloaked: 0 })));
  assert.strictEqual(v.hidden, false);
  assert.strictEqual(v.on_screen, true);
});

test("minimized is not hidden (it stops presenting)", () => {
  assert.strictEqual(WS.judge(report(win({ minimized: true }))).hidden, false);
});

test("a taskbar button that names the window is NOT hidden", () => {
  const v = WS.judge(report(win({}), { read: true, buttons: ["HumanityOS v0.1 - 1 running window"], window_buttons_seen: 1 }));
  assert.strictEqual(v.hidden, false);
  assert.strictEqual(v.taskbar_button, true);
});

test("a taskbar read that saw no window buttons at all proves nothing", () => {
  const v = WS.judge(report(win({}), { read: true, buttons: [], window_buttons_seen: 0 }));
  assert.strictEqual(v.taskbar_button, null, "a taskbar read that saw no window buttons at all proves nothing");
});

test("the main window is the captioned one, not a helper window with the same title prefix", () => {
  const helper = win({ style: "0x84000000", cloaked: 0, tool_window: false });
  const main = win({});
  assert.strictEqual(WS.mainWindow({ windows: [helper, main] }), main);
});

test("no window: no verdict", () => {
  const v = WS.judge({ pid: 1, windows: [], taskbar: null, error: "boom" });
  assert.strictEqual(v.hidden, null);
  assert.match(v.why, /boom/);
});
