// The judge of `just verify-tray` (scripts/lib/tray-judge.js): a good run passes, and each link of
// the hidden-window chain, broken alone, fails its own check and only that one.
//
// Run: node --test scripts/tests/tray-judge.test.js   (in `just rig-tests`)
//
// Red first, 2026-10-31: every "broken" case below is one link broken on purpose; each must FAIL
// its named check (the test asserts the failing ids exactly), so a judge that stopped looking at
// a link goes red here. The good run is the shape of the real run of the same day.

const test = require("node:test");
const assert = require("node:assert");
const TJ = require("../lib/tray-judge.js");

function good() {
  return {
    first: { allowed: true, icon_shown: true, world_loaded: true, hidden: false, visible: true },
    hide: {
      done: {
        hidden: true,
        visible: false,
        page: "MainMenu",
        voice_close: { hid: true, left_by_close: true },
        voice: { room: null, call: null, wanted: false, session_running: false, mic_test: false },
      },
      log: "persistence] World saved to C:\\rig\\saves\\offline_home.json\nvoice: the call or voice room was left and the microphone closed (the window went to the tray)\nwindow: hidden in the tray",
    },
    hidden_later: { hidden: true, visible: false, ms_since_last_frame: 3100 },
    shown: { hidden: false, visible: true, ms_since_last_frame: 40 },
    quit: { exited: true, log: "tray: Quit\nWorld saved to C:\\rig\\saves\\offline_home.json\n===== HumanityOS clean shutdown =====" },
    taskkill: { hidden_first: true, exited_gracefully: true, log: "window: another program asked HumanityOS to close: quitting\nWorld saved to x\n===== HumanityOS clean shutdown =====" },
    panics: 0,
  };
}

const failed = (r) => r.checks.filter((c) => !c.ok).map((c) => c.id);

test("a good run passes every link", () => {
  const r = TJ.judgeTray(good());
  assert.deepStrictEqual(failed(r), []);
  assert.strictEqual(r.ok, true);
  assert.match(TJ.verdictLines(r).at(-1), /RESULT: PASS/);
});

const broken = [
  ["no icon (the rig did not ask for the tray)", (r) => (r.first.icon_shown = false), ["tray_up"]],
  ["the close quit instead of hiding", (r) => (r.hide.done.voice_close.hid = false), ["hides"]],
  ["the window stayed on screen (winit's hide alone, a background launch)", (r) => (r.hide.done.visible = true), ["hides"]],
  ["hiding did not save the world", (r) => (r.hide.log = r.hide.log.replace("World saved to", "World kept in")), ["world_left_saved"]],
  ["the world stayed in front", (r) => (r.hide.done.page = "None"), ["world_left_saved"]],
  ["the call stayed up (part 2 as merged)", (r) => {
    r.hide.done.voice_close.left_by_close = false;
    r.hide.done.voice.call = "rig-peer";
    r.hide.done.voice.wanted = true;
  }, ["voice_left"]],
  ["a voice session still running", (r) => (r.hide.done.voice.session_running = true), ["voice_left"]],
  ["frames still drawn while hidden", (r) => (r.hidden_later.ms_since_last_frame = 30), ["no_frames_hidden"]],
  ["the icon did not bring it back", (r) => (r.shown.hidden = true), ["shows_again"]],
  ["back, but no frames", (r) => (r.shown.ms_since_last_frame = 5000), ["shows_again"]],
  ["Quit while hidden never ended it", (r) => (r.quit.exited = false), ["quit_hidden_saves"]],
  ["Quit while hidden did not save", (r) => (r.quit.log = "tray: Quit\n===== HumanityOS clean shutdown ====="), ["quit_hidden_saves"]],
  ["taskkill had to force it (the second rig run of 2026-10-31)", (r) => (r.taskkill.exited_gracefully = false), ["taskkill_hidden"]],
  ["taskkill on a window that was not hidden proves nothing here", (r) => (r.taskkill.hidden_first = false), ["taskkill_hidden"]],
  ["a panic", (r) => (r.panics = 1), ["no_panics"]],
];

for (const [what, breakIt, expect] of broken) {
  test(`one broken link fails its own check: ${what}`, () => {
    const run = good();
    breakIt(run);
    const r = TJ.judgeTray(run);
    assert.deepStrictEqual(failed(r), expect, what);
    assert.strictEqual(r.ok, false);
    assert.match(TJ.verdictLines(r).at(-1), /RESULT: FAIL/);
  });
}
