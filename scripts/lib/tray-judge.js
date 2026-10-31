// The judge of `just verify-tray` (scripts/verify-tray.js): the hidden-window chain of
// notifications rung 2, part 2 (docs/design/notifications.md, "The tray"), from what the game
// answered in debug/tray_done.json and wrote in run.log. Pure, so scripts/tests/tray-judge.test.js
// holds it to a good run and to one broken link at a time.
//
// THE CHAIN, each link a check:
//   tray_up            the game made its icon (a rig asks with HUMANITY_TRAY=1)
//   hides              the close button (with "closing keeps HumanityOS running" on) hid the window
//   world_left_saved   the world was saved as it went, and the main menu is in front
//   voice_left         a voice room and a call it was in were left, no voice wanted, no session
//                      (the critic's review of part 2: hiding kept the microphone open)
//   no_frames_hidden   while hidden no frame was drawn, and the slow tick still answered
//   shows_again        the icon brought the window back and frames came again
//   quit_hidden_saves  the tray's Quit, while hidden, saved and shut down cleanly
//   taskkill_hidden    `taskkill` without /F, while hidden, closed it gracefully (saved), not forced
//   no_panics          no PANIC in either boot's run.log

"use strict";

/// The least time with no frame that counts as "nothing drawn while hidden", ms (the rig waits
/// HIDDEN_WAIT_MS before it asks).
const HIDDEN_MS = 2000;
/// Frames are back when the last one is younger than this, ms (a background rig draws at 15 to 30
/// a second, low impact or not).
const SHOWN_MS = 1000;

const has = (text, needle) => typeof text === "string" && text.includes(needle);

/// Judge one run. `run`:
///   first          tray_done after boot and world entry
///   hide           { done: tray_done of the voice_close, log: run.log written during it }
///   hidden_later   tray_done HIDDEN_WAIT_MS after hiding
///   shown          tray_done after the icon, a moment later
///   quit           { exited: bool, log: run.log since the Quit }
///   taskkill       { exited_gracefully: bool, log: run.log of the second boot since the taskkill }
///   panics         PANIC lines in both boots' run.log
/// Returns { ok, checks: [{ id, ok, why }] }.
function judgeTray(run) {
  const checks = [];
  const check = (id, ok, why) => checks.push({ id, ok: !!ok, why });
  const first = run.first || {};
  check("tray_up", first.allowed === true && first.icon_shown === true, `allowed ${first.allowed}, icon shown ${first.icon_shown}`);

  const hide = (run.hide && run.hide.done) || {};
  const vc = hide.voice_close || {};
  check("hides", vc.hid === true && hide.hidden === true && hide.visible === false, `close hid it ${vc.hid}, hidden ${hide.hidden}, visible ${hide.visible}`);
  const hideLog = (run.hide && run.hide.log) || "";
  check(
    "world_left_saved",
    first.world_loaded === true && has(hideLog, "World saved to") && hide.page === "MainMenu",
    `world loaded first ${first.world_loaded}, saved as it hid ${has(hideLog, "World saved to")}, page ${hide.page}`
  );
  const v = hide.voice || {};
  check(
    "voice_left",
    vc.left_by_close === true && v.room == null && v.call == null && v.wanted === false && v.session_running === false && has(hideLog, "voice: the call or voice room was left"),
    `left by the close ${vc.left_by_close}, room ${v.room}, call ${v.call}, wanted ${v.wanted}, session ${v.session_running}, logged ${has(hideLog, "voice: the call or voice room was left")}`
  );

  const later = run.hidden_later || {};
  check(
    "no_frames_hidden",
    later.hidden === true && Number(later.ms_since_last_frame) >= HIDDEN_MS,
    `hidden ${later.hidden}, no frame for ${later.ms_since_last_frame} ms (at least ${HIDDEN_MS})`
  );
  const shown = run.shown || {};
  check(
    "shows_again",
    shown.hidden === false && shown.visible === true && Number(shown.ms_since_last_frame) < SHOWN_MS,
    `hidden ${shown.hidden}, visible ${shown.visible}, last frame ${shown.ms_since_last_frame} ms ago (under ${SHOWN_MS})`
  );

  const quit = run.quit || {};
  const ql = quit.log || "";
  const quitAt = ql.indexOf("tray: Quit");
  check(
    "quit_hidden_saves",
    quit.exited === true && quitAt >= 0 && ql.indexOf("World saved to", quitAt) > quitAt && has(ql, "clean shutdown"),
    `exited ${quit.exited}, Quit logged ${quitAt >= 0}, saved after it ${quitAt >= 0 && ql.indexOf("World saved to", quitAt) > quitAt}, clean shutdown ${has(ql, "clean shutdown")}`
  );
  const tk = run.taskkill || {};
  const tl = tk.log || "";
  check(
    "taskkill_hidden",
    tk.hidden_first === true && tk.exited_gracefully === true && has(tl, "another program asked HumanityOS to close") && has(tl, "clean shutdown"),
    `hidden first ${tk.hidden_first}, closed without force ${tk.exited_gracefully}, asked-to-close logged ${has(tl, "another program asked HumanityOS to close")}, clean shutdown ${has(tl, "clean shutdown")}`
  );
  check("no_panics", Number(run.panics || 0) === 0, `${run.panics || 0} PANIC line(s)`);
  return { ok: checks.every((c) => c.ok), checks };
}

/// The verdict's lines, one per check, then the result.
function verdictLines(result) {
  const lines = result.checks.map((c) => `${c.ok ? "PASS" : "FAIL"}  ${c.id.padEnd(18)} ${c.why}`);
  const failed = result.checks.filter((c) => !c.ok).length;
  lines.push(result.ok ? `RESULT: PASS  ${result.checks.length}/${result.checks.length} links of the hidden-window chain held` : `RESULT: FAIL  ${failed} of ${result.checks.length} links broke`);
  return lines;
}

module.exports = { judgeTray, verdictLines, HIDDEN_MS, SHOWN_MS };
