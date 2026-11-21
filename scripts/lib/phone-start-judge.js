// The verdict of scripts/verify-phone-start.js (Android rung A1, 2026-11-20; docs/design/platforms.md):
// reads each boot's run.log and judges it. Pure: no game, no files, so scripts/tests/
// phone-start-judge.test.js holds every rule, one broken at a time.
//
// THE BOOTS the rig makes, and what each must show:
//   desktop      a plain desktop start: the world's renderer built at start ("pages-only start: no",
//                many render pipelines). The comparison for the start time.
//   phone        the phone preview (HUMANITY_PHONE_PREVIEW=1), pages visited, then idle:
//                  - a pages-only start, the world's renderer never built;
//                  - wgpu holds at most PAGES_MAX_RENDER render pipelines and no compute pipeline
//                    (egui's and the 3D model viewer's: the pages' own), counted by wgpu;
//                  - no stray reach for the world ("[PagesOnly] world touched from ...");
//                  - an idle stretch draws at most IDLE_MAX_FRAMES frames per window.
//   before       the same preview with HUMANITY_REDRAW_ALWAYS=1 (every frame drawn, as before
//                2026-11-20): the frame count must see frames (at least BEFORE_MIN_FRAMES in a window),
//                which proves the count can tell the two apart.
//   pages-first  a desktop started pages-only (HUMANITY_PAGES_FIRST=1), then the world entered: the
//                world's renderer built on demand, more pipelines after than before, no panic.
// Every boot: no PANIC in its log.

"use strict";

/** The pages' own render pipelines at most: egui's one, the 3D model viewer's few (gui/model_view/gpu.rs). */
const PAGES_MAX_RENDER = 8;
/** Frames an idle window may draw (none expected; one or two for a stray wake is not a frame loop). */
const IDLE_MAX_FRAMES = 2;
/** The continuous count must see at least this many frames in a window. */
const BEFORE_MIN_FRAMES = 50;
/** A world renderer holds far more than the pages: at least this many render pipelines. */
const WORLD_MIN_RENDER = 20;

/** The numbers a boot's run.log carries (engine/redraw.rs and engine/renderer_slot.rs write them). */
function parseLog(text) {
  const t = String(text || "");
  const out = { first_frame_ms: null, pages_only: null, counts: [], strays: [], redraw: [], built_for: [], panics: 0 };
  const first = t.match(/\[Boot\] first frame drawn (\d+) ms after the window opened \(pages-only start: (yes|no)\)/);
  if (first) {
    out.first_frame_ms = Number(first[1]);
    out.pages_only = first[2] === "yes";
  }
  for (const m of t.matchAll(/\[PagesOnly\] wgpu holds (\d+) render pipelines, (\d+) compute pipelines, (\d+) shader modules (.*?); the world's renderer built: (yes|no)/g)) {
    out.counts.push({ render: Number(m[1]), compute: Number(m[2]), shader_modules: Number(m[3]), when: m[4], built: m[5] === "yes" });
  }
  for (const m of t.matchAll(/\[PagesOnly\] (world touched from \S+|the world's renderer was built by a reach from \S+)/g)) out.strays.push(m[1]);
  for (const m of t.matchAll(/\[PagesOnly\] the world's renderer built for (.+?): (\d+) ms/g)) out.built_for.push({ reason: m[1], ms: Number(m[2]) });
  for (const m of t.matchAll(/\[Redraw\] (reactive|continuous): (\d+) frames drawn in the last ([\d.]+) s, (\d+) wakes without one/g)) {
    out.redraw.push({ mode: m[1], frames: Number(m[2]), secs: Number(m[3]), wakes: Number(m[4]) });
  }
  out.panics = (t.match(/PANIC/g) || []).length;
  return out;
}

/** Judge a run: { boots: { desktop, phone, before, pages_first } }, each { log } plus, for the phone
 *  boot, `idle_from` (how many [Redraw] lines were logged before its idle stretch began) and, for
 *  pages-first, `entered` (the autopilot's answer). Returns { ok, checks: [{ ok, what }] }. */
function judgePhoneStart(run) {
  const checks = [];
  const check = (ok, what) => checks.push({ ok: !!ok, what });
  const boots = (run && run.boots) || {};
  // `--only` judges the boots it asked for; every other run judges all four.
  const asked = (name) => !(run && Array.isArray(run.only)) || run.only.includes(name);

  const desk = boots.desktop && parseLog(boots.desktop.log);
  if (!asked("desktop")) {
    // not asked for
  } else if (!desk) check(false, "desktop: no boot");
  else {
    check(desk.pages_only === false, `desktop: a full start (pages-only start: ${desk.pages_only === null ? "no first-frame line" : desk.pages_only ? "yes" : "no"})`);
    const c = desk.counts[0];
    check(c && c.built && c.render >= WORLD_MIN_RENDER, `desktop: the world's renderer built at start (${c ? `${c.render} render pipelines` : "no count"})`);
    check(desk.panics === 0, `desktop: no panic (${desk.panics})`);
  }

  const phone = boots.phone && parseLog(boots.phone.log);
  if (!asked("phone")) {
    // not asked for
  } else if (!phone) check(false, "phone: no boot");
  else {
    check(phone.pages_only === true, `phone: a pages-only start (${phone.pages_only === null ? "no first-frame line" : phone.pages_only ? "yes" : "no"})`);
    const last = phone.counts[phone.counts.length - 1];
    check(last && !last.built, `phone: the world's renderer never built (${last ? (last.built ? "BUILT" : "not built") : "no count"})`);
    check(last && last.render <= PAGES_MAX_RENDER, `phone: wgpu holds at most ${PAGES_MAX_RENDER} render pipelines (${last ? last.render : "no count"})`);
    check(last && last.compute === 0, `phone: wgpu holds no compute pipeline (${last ? last.compute : "no count"})`);
    check(phone.strays.length === 0, `phone: no reach for the world (${phone.strays.length ? phone.strays.join("; ") : "none"})`);
    const idle = phone.redraw.slice(boots.phone.idle_from || 0).filter((w) => w.mode === "reactive");
    const win = idle[1] || idle[0];
    check(win && win.frames <= IDLE_MAX_FRAMES, `phone: an idle window draws at most ${IDLE_MAX_FRAMES} frames (${win ? `${win.frames} in ${win.secs} s, ${win.wakes} wakes` : "no idle window"})`);
    check(phone.panics === 0, `phone: no panic (${phone.panics})`);
  }

  const before = boots.before && parseLog(boots.before.log);
  if (!asked("before")) {
    // not asked for
  } else if (!before) check(false, "before: no boot");
  else {
    const w = before.redraw.filter((x) => x.mode === "continuous");
    const win = w[1] || w[0];
    check(win && win.frames >= BEFORE_MIN_FRAMES, `before: drawing every frame, the count sees them (${win ? `${win.frames} in ${win.secs} s` : "no window"})`);
    check(before.panics === 0, `before: no panic (${before.panics})`);
  }

  const pf = boots.pages_first && parseLog(boots.pages_first.log);
  if (!asked("pages_first")) {
    // not asked for
  } else if (!pf) check(false, "pages-first: no boot");
  else {
    check(pf.pages_only === true, `pages-first: a pages-only start (${pf.pages_only === null ? "no first-frame line" : pf.pages_only ? "yes" : "no"})`);
    check(pf.built_for.some((b) => b.reason === "entering the world"), `pages-first: the world's renderer built on entering the world (${pf.built_for.map((b) => `${b.reason}, ${b.ms} ms`).join("; ") || "never"})`);
    check(pf.strays.length === 0, `pages-first: no reach for the world before it was entered (${pf.strays.length ? pf.strays.join("; ") : "none"})`);
    check(boots.pages_first.entered && boots.pages_first.entered.ok === true, `pages-first: the world entered (${JSON.stringify(boots.pages_first.entered || null)})`);
    const after = pf.counts[pf.counts.length - 1];
    check(after && after.built && after.render >= WORLD_MIN_RENDER, `pages-first: the world's pipelines exist after entry (${after ? `${after.render} render pipelines` : "no count"})`);
    check(pf.panics === 0, `pages-first: no panic (${pf.panics})`);
  }
  return { ok: checks.length > 0 && checks.every((c) => c.ok), checks, numbers: numbers(desk, phone, before, pf) };
}

/** The numbers the report states: the start times and the idle frame counts. */
function numbers(desk, phone, before, pf) {
  const idleOf = (p, mode) => {
    const w = p ? p.redraw.filter((x) => x.mode === mode) : [];
    return w.length ? w[w.length - 1] : null;
  };
  return {
    first_frame_ms: { desktop: desk ? desk.first_frame_ms : null, phone: phone ? phone.first_frame_ms : null, pages_first: pf ? pf.first_frame_ms : null },
    idle_window: { before: idleOf(before, "continuous"), after: idleOf(phone, "reactive") },
    world_build_on_entry_ms: pf ? (pf.built_for.find((b) => b.reason === "entering the world") || {}).ms || null : null,
  };
}

function verdictLines(result) {
  const lines = result.checks.map((c) => `${c.ok ? "  ok  " : "  FAIL"} ${c.what}`);
  const n = result.numbers || {};
  if (n.first_frame_ms) lines.push(`  first frame after the window opened: desktop ${n.first_frame_ms.desktop} ms, phone preview ${n.first_frame_ms.phone} ms, pages-first ${n.first_frame_ms.pages_first} ms`);
  if (n.idle_window) {
    const f = (w) => (w ? `${w.frames} frames in ${w.secs} s` : "none");
    lines.push(`  an idle window: before (every frame) ${f(n.idle_window.before)}, after (on change) ${f(n.idle_window.after)}`);
  }
  if (n.world_build_on_entry_ms) lines.push(`  the world's renderer built on entering the world: ${n.world_build_on_entry_ms} ms`);
  lines.push(result.ok ? "PHONE START: PASS" : "PHONE START: FAIL");
  return lines;
}

module.exports = { parseLog, judgePhoneStart, verdictLines, PAGES_MAX_RENDER, IDLE_MAX_FRAMES, BEFORE_MIN_FRAMES, WORLD_MIN_RENDER };
