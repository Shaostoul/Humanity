// The verdict of scripts/verify-phone-start.js (Android rung A1, 2026-11-20; docs/design/platforms.md):
// reads each boot's run.log and judges it. Pure: no game, no files, so scripts/tests/
// phone-start-judge.test.js holds every rule, one broken at a time.
//
// THE BOOTS the rig makes, and what each must show:
//   desktop      a plain desktop start, which since 2026-11-20 is PAGES-FIRST and prepares the 3D world
//                only when Play is pressed (the default, the operator's; engine/world_prep.rs), then
//                entered by the dev autopilot (Play):
//                  - a pages-only start, and at the first frame wgpu holds at most PAGES_MAX_RENDER
//                    render pipelines and no compute pipeline, the world's renderer not built: the
//                    pages were drawn before any world pipeline existed (counted by wgpu);
//                  - Play waited for the world ("[WorldPrep] Play pressed before the world was
//                    ready"), which was built for entering it and became ready;
//                  - the frame's share of that preparation (looking whether it is done, installing
//                    it) never over ON_FRAME_BUDGET_MS on any frame, as the game measured it;
//                  - no stray reach for the world, the world entered, its pipelines there after.
//                The numbers: its first frame (the pages ready), the world ready after Play, Play to
//                the world loaded.
//   background   HUMANITY_WORLD_PREP=background, the world prepared beside the pages after the first
//                frame: the pages before any world pipeline, the world ready, the frame's share in
//                its budget, no stray reach, then entered once ready. The numbers: when the world
//                was ready, the page frames' spacing while it was prepared and after, Play after it.
//   old_order    HUMANITY_PAGES_FIRST=0, the order of every start before 2026-11-20: the world's
//                renderer built at start ("pages-only start: no", many render pipelines). The
//                comparison for the start time.
//   play_during  HUMANITY_WORLD_PREP=background, entered by the dev autopilot right after its first
//                frame, while the world is still being prepared: Play waited for it, the world opened
//                by itself once ready, and the world entered. The number: how long Play took then.
//   phone        the phone preview (HUMANITY_PHONE_PREVIEW=1), pages visited, then idle:
//                  - a pages-only start, the world's renderer never built;
//                  - wgpu holds at most PAGES_MAX_RENDER render pipelines and no compute pipeline
//                    (egui's and the 3D model viewer's: the pages' own), counted by wgpu;
//                  - no stray reach for the world ("[PagesOnly] world touched from ...");
//                  - an idle stretch draws at most IDLE_MAX_FRAMES frames per window.
//   before       the same preview with HUMANITY_REDRAW_ALWAYS=1 (every frame drawn, as before
//                2026-11-20): the frame count must see frames (at least BEFORE_MIN_FRAMES in a window),
//                which proves the count can tell the two apart.
// (The pages-first boot of the first phone-start rig, HUMANITY_PAGES_FIRST=1, is the desktop boot now:
// that switch names the default since 2026-11-20.)
// Every boot: no PANIC in its log.
//
// THE LONG GAPS between two page frames (2026-11-21): engine/world_prep.rs logs each gap of 100 ms
// or more with its parts (the frame before's whole handler and its present, the time between the
// frames, this frame's time so far) and each frame's longest stretches (its laps: the frame's
// stages, the present, the work after it). `classifyGap` tells IDLE (a phone that drew nothing
// because nothing asked: the time between frames, on reactive drawing) from SLOW (a stretch that
// held the frame, named). The desktop and background boots must keep every page frame before Play
// (nothing being built) under IDLE_STRETCH_MAX_MS in its longest stretch: the v0.1567.0 run had a
// 3,051 ms gap there, the boot frame's file work (the instruments' first read, the settings' save
// with its flush to disk) on a busy disk. The slowest gap before Play, on Play and beside the pages
// is printed with its cause.

"use strict";

/** The pages' own render pipelines at most: egui's one, the 3D model viewer's few (gui/model_view/gpu.rs). */
const PAGES_MAX_RENDER = 8;
/** Frames an idle window may draw (none expected; one or two for a stray wake is not a frame loop). */
const IDLE_MAX_FRAMES = 2;
/** The continuous count must see at least this many frames in a window. */
const BEFORE_MIN_FRAMES = 50;
/** A world renderer holds far more than the pages: at least this many render pipelines. */
const WORLD_MIN_RENDER = 20;
/** The frame's share of the world's preparation, at most, on any frame (engine/world_prep.rs ON_FRAME_BUDGET). */
const ON_FRAME_BUDGET_MS = 2;
/** Every boot the rig knows. */
const BOOTS = ["desktop", "background", "play_during", "old_order", "phone", "before"];
/** A page frame's longest single stretch (a stage, its present, the work after it, or the time
 *  between two frames) at most while nothing is being built (before Play): such a frame only draws
 *  pages, so anything longer is a hitch with a cause of ours, named in the check. */
const IDLE_STRETCH_MAX_MS = 100;
/** While the world is built (on Play, beside the pages), at most this share of the page frames may
 *  come 100 ms or more after the one before. Measured 2026-11-21 (other agents compiling): with the
 *  build's threads on every core, 19 of 188 frames on Play (10%) and 17 of 209 in play_during; with
 *  the frame's core left to it (renderer/world_build.rs `build_thread`), 7 of 301 and 2 of 278. A
 *  share, not a worst: one frame on a PC busy with other programs is not the build's doing, a
 *  frame in ten is. (The game logs at most 24 long gaps a run, so a count past that reads as 24.) */
const BUSY_LONG_GAP_SHARE_MAX = 0.05;
/** While the world is built, no page frame at all this long after the one before (2026-11-21,
 *  BUG-338's third cause). The share above lets a few slow frames of a busy PC through, and so it
 *  let through the one frame of every build that froze the progress card for 440 to 940 ms while
 *  the star sky made its GPU memory beside the renderer's (renderer/world_build.rs
 *  `compiles_running`): a freeze that long is seen, however rare. Measured that day (other agents
 *  compiling): the stalls 391, 440, 607 and 849 ms; every other slow frame of the build 120 to 214
 *  ms (a stage or the present waiting on a busy PC), so the cap sits between. */
const BUSY_WORST_GAP_MAX_MS = 300;

/** The numbers a boot's run.log carries (engine/redraw.rs, engine/renderer_slot.rs and engine/world_prep.rs write them). */
function parseLog(text) {
  const t = String(text || "");
  const out = { first_frame_ms: null, pages_only: null, counts: [], strays: [], redraw: [], built_for: [], panics: 0, ready_ms: null, prep_ms: null, on_frame: null, during: null, windows: [], gaps: [], pressed: false, opened_ms: null, loaded_ms: null };
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
  const ready = t.match(/\[WorldPrep\] the 3D world is ready: (\d+) ms after the window opened, (\d+) ms after its preparation began/);
  if (ready) {
    out.ready_ms = Number(ready[1]);
    out.prep_ms = Number(ready[2]);
  }
  const onFrame = t.match(/\[WorldPrep\] on-frame work while it was prepared: worst ([\d.]+) ms over (\d+) frames/);
  if (onFrame) out.on_frame = { worst_ms: Number(onFrame[1]), frames: Number(onFrame[2]) };
  const during = t.match(/\[WorldPrep\] page frames while it was prepared: (\d+), ([\d.]+) ms apart on average, worst ([\d.]+) ms/);
  if (during) out.during = { count: Number(during[1]), mean_ms: Number(during[2]), worst_ms: Number(during[3]) };
  for (const m of t.matchAll(/\[WorldPrep\] page frames in the last ([\d.]+) s: (\d+), ([\d.]+) ms apart on average, worst ([\d.]+) ms; the world: (ready|being prepared|not started)/g)) {
    out.windows.push({ secs: Number(m[1]), count: Number(m[2]), mean_ms: Number(m[3]), worst_ms: Number(m[4]), world: m[5] });
  }
  out.gaps = parseGaps(t);
  out.pressed = /\[WorldPrep\] Play pressed before the world was ready/.test(t);
  const opened = t.match(/\[WorldPrep\] opening the world (\d+) ms after Play was pressed/);
  if (opened) out.opened_ms = Number(opened[1]);
  const loaded = t.match(/3D world loaded in (\d+)ms/);
  if (loaded) out.loaded_ms = Number(loaded[1]);
  out.panics = (t.match(/PANIC/g) || []).length;
  return out;
}

/** "network 151 ms, load 2 ms" (engine/world_prep.rs `laps_words`) as [{ name, ms }]; "none" is []. */
function parseLaps(s) {
  if (!s || s === "none") return [];
  return s
    .split(", ")
    .map((p) => {
      const m = p.match(/^(\S+) ([\d.]+) ms$/);
      return m ? { name: m[1], ms: Number(m[2]) } : null;
    })
    .filter(Boolean);
}

/** The long gaps between two page frames (engine/world_prep.rs `note_spacing`), each with its parts:
 *  the frame before's whole handler and its present, the time between the two frames, this frame's
 *  time to the line, the longest stretches (laps) of each frame, the world's state and the drawing
 *  (continuous on a desktop, reactive on a phone). A line from before 2026-11-21 has no stretches and
 *  no drawing, and its "frame before" ended at its present (the work after it read as between). */
function parseGaps(t) {
  const re =
    /\[WorldPrep\] a page frame (\d+) ms after the last \((\d+) ms after the window opened; the frame before took (\d+) ms, (\d+) of them presenting, then (\d+) ms between frames, and this one (\d+) ms to here(?:; the frame before's longest: (.*?); this one's: (.*?))?\); the world: (.*?)(?:; drawing: (continuous|reactive))?(?=\r?$)/gm;
  const out = [];
  for (const m of t.matchAll(re)) {
    out.push({
      gap_ms: Number(m[1]),
      at_ms: Number(m[2]),
      before_ms: Number(m[3]),
      present_ms: Number(m[4]),
      between_ms: Number(m[5]),
      this_ms: Number(m[6]),
      before_longest: parseLaps(m[7]),
      this_longest: parseLaps(m[8]),
      world: m[9],
      drawing: m[10] || null,
    });
  }
  return out;
}

/** What a long gap was: IDLE (a reactive app that drew nothing because nothing asked, the time
 *  between frames) or a SLOW frame, with the stretch that took the most of it: a lap of the frame
 *  before or of this one ("instrument_packs", "present", "network"), or "between frames" (the event
 *  loop's own time on a desktop that draws every frame). A line without laps names the frame's
 *  parts instead ("the frame before's work", "present", "this frame"). */
function classifyGap(g) {
  if (g.drawing === "reactive" && g.between_ms * 2 >= g.gap_ms) return { kind: "idle", cause: { name: "nothing asked for a frame", ms: g.between_ms } };
  const c = [];
  // The frame-rate cap's sleep is pacing (Settings' caps, 30 a second in a window without focus),
  // never a hitch's cause: it is left out.
  const work = (l) => l.name !== "fps_cap_sleep";
  if (g.before_longest.length || g.this_longest.length) {
    for (const l of g.before_longest.filter(work)) c.push({ name: l.name, ms: l.ms, in: "the frame before" });
    for (const l of g.this_longest.filter(work)) c.push({ name: l.name, ms: l.ms, in: "this frame" });
  } else {
    c.push({ name: "the frame before's work", ms: g.before_ms - g.present_ms, in: "the frame before" });
    c.push({ name: "present", ms: g.present_ms, in: "the frame before" });
    c.push({ name: "this frame", ms: g.this_ms, in: "this frame" });
  }
  c.push({ name: "between frames", ms: g.between_ms, in: "between frames" });
  c.sort((a, b) => b.ms - a.ms);
  return { kind: "slow", cause: c[0] };
}

/** The slowest of a boot's long gaps that `pred` keeps, by its cause's length: { ...gap, kind,
 *  cause }, or null when none was slow (no long gap, or only idle ones). */
function worstSlow(gaps, pred) {
  let w = null;
  for (const g of gaps || []) {
    if (!pred(g)) continue;
    const c = classifyGap(g);
    if (c.kind === "slow" && (!w || c.cause.ms > w.cause.ms)) w = { ...g, ...c };
  }
  return w;
}

/** "instrument_packs 3020 ms in the frame before; a gap of 3051 ms, 4078 ms after the window opened". */
function gapWords(w) {
  const where = w.cause.in && w.cause.in !== w.cause.name ? ` in ${w.cause.in}` : "";
  return `${w.cause.name} ${Math.round(w.cause.ms)} ms${where}; a gap of ${w.gap_ms} ms, ${w.at_ms} ms after the window opened`;
}

/** Judge a run: { boots: { desktop, background, play_during, old_order, phone, before } }, each { log }
 *  plus, for the phone boot, `idle_from` (how many [Redraw] lines were logged before its idle stretch
 *  began) and, for desktop, background and play_during, `entered` (the autopilot's answer, written
 *  once the world has loaded). Returns { ok, checks: [{ ok, what }], numbers }. */
function judgePhoneStart(run) {
  const checks = [];
  const check = (ok, what) => checks.push({ ok: !!ok, what });
  const boots = (run && run.boots) || {};
  // `--only` judges the boots it asked for; every other run judges all of them.
  const asked = (name) => !(run && Array.isArray(run.only)) || run.only.includes(name);
  const enteredOk = (b) => b.entered && b.entered.ok === true;

  // The first frame's count: the pages drawn before any world pipeline existed.
  const pagesFirst = (name, p) => {
    check(p.pages_only === true, `${name}: a pages-first start (pages-only start: ${p.pages_only === null ? "no first-frame line" : p.pages_only ? "yes" : "no"})`);
    const c = p.counts[0];
    check(
      c && !c.built && c.render <= PAGES_MAX_RENDER && c.compute === 0,
      `${name}: the pages drawn before any world pipeline existed (${c ? `${c.render} render, ${c.compute} compute, the world built: ${c.built ? "yes" : "no"}` : "no count"})`,
    );
  };
  // Before Play, with nothing being built, no page frame holds the window: its longest stretch, the
  // cause the long-gap line names, stays under IDLE_STRETCH_MAX_MS (an idle gap of a reactive app
  // is no hitch and is never judged here).
  const idleStretch = (name, p) => {
    const w = worstSlow(p.gaps, (g) => g.world === "not started");
    check(!w || w.cause.ms <= IDLE_STRETCH_MAX_MS, `${name}: before Play, no stretch of a page frame over ${IDLE_STRETCH_MAX_MS} ms (${w ? gapWords(w) : "no long gap"})`);
  };
  // While the world is built, the page frames keep coming: long gaps at most a small share of them
  // (BUSY_LONG_GAP_SHARE_MAX), the slowest named.
  const busyShare = (name, p) => {
    const building = (g) => g.world.startsWith("being prepared");
    const long = p.gaps.filter((g) => building(g) && classifyGap(g).kind === "slow").length;
    const n = p.during ? p.during.count : 0;
    const w = worstSlow(p.gaps, building);
    check(
      n > 0 && long <= Math.floor(n * BUSY_LONG_GAP_SHARE_MAX),
      `${name}: while the world was built, at most ${Math.round(BUSY_LONG_GAP_SHARE_MAX * 100)}% of the page frames 100 ms or more after the one before (${n > 0 ? `${long} of ${n}` : "no count of the page frames"}${w ? `; the slowest: ${gapWords(w)}` : ""})`,
    );
    // And none of them a freeze (BUSY_WORST_GAP_MAX_MS): the game's own worst over every page frame
    // of the build, the logged long gaps capped at 24 a run or not.
    const worst = p.during ? p.during.worst_ms : null;
    check(
      worst !== null && worst <= BUSY_WORST_GAP_MAX_MS,
      `${name}: while the world was built, no page frame more than ${BUSY_WORST_GAP_MAX_MS} ms after the one before (${worst !== null ? `worst ${worst} ms${w ? `; the slowest named: ${gapWords(w)}` : ""}` : "no count of the page frames"})`,
    );
  };
  // The frame's share of the preparation, as the game measured it.
  const budget = (name, p) => {
    const of = p.on_frame;
    check(
      of && of.worst_ms <= ON_FRAME_BUDGET_MS,
      `${name}: the frame's share of the preparation never over ${ON_FRAME_BUDGET_MS} ms (${of ? `worst ${of.worst_ms} ms over ${of.frames} frames` : "no measurement"})`,
    );
  };

  const desk = boots.desktop && parseLog(boots.desktop.log);
  if (!asked("desktop")) {
    // not asked for
  } else if (!desk) check(false, "desktop: no boot");
  else {
    pagesFirst("desktop", desk);
    idleStretch("desktop", desk);
    check(desk.pressed,`desktop: Play waited for the world it prepares (${desk.pressed ? "yes" : "no such line"})`);
    check(desk.built_for.some((b) => b.reason === "entering the world"), `desktop: the world's renderer built for entering the world (${desk.built_for.map((b) => `${b.reason}, ${b.ms} ms`).join("; ") || "never"})`);
    check(desk.prep_ms !== null, `desktop: the world ready after Play (${desk.prep_ms !== null ? `${desk.prep_ms} ms` : "never"})`);
    budget("desktop", desk);
    busyShare("desktop", desk);
    check(desk.strays.length === 0, `desktop: no reach for the world before it was ready (${desk.strays.length ? desk.strays.join("; ") : "none"})`);
    check(enteredOk(boots.desktop), `desktop: the world entered (${JSON.stringify(boots.desktop.entered || null)})`);
    const after = desk.counts[desk.counts.length - 1];
    check(after && after.built && after.render >= WORLD_MIN_RENDER, `desktop: the world's pipelines exist after entry (${after ? `${after.render} render pipelines` : "no count"})`);
    check(desk.panics === 0, `desktop: no panic (${desk.panics})`);
  }

  const bg = boots.background && parseLog(boots.background.log);
  if (!asked("background")) {
    // not asked for
  } else if (!bg) check(false, "background: no boot");
  else {
    pagesFirst("background", bg);
    idleStretch("background", bg);
    check(bg.ready_ms !== null && !bg.pressed, `background: the world prepared beside the pages, before Play (${bg.ready_ms !== null ? `ready ${bg.ready_ms} ms after the window opened` : "never"}${bg.pressed ? "; Play had to wait" : ""})`);
    budget("background", bg);
    busyShare("background", bg);
    check(bg.strays.length === 0, `background: no reach for the world before it was ready (${bg.strays.length ? bg.strays.join("; ") : "none"})`);
    check(enteredOk(boots.background), `background: the world entered once it was ready (${JSON.stringify(boots.background.entered || null)})`);
    check(bg.panics === 0, `background: no panic (${bg.panics})`);
  }

  const old = boots.old_order && parseLog(boots.old_order.log);
  if (!asked("old_order")) {
    // not asked for
  } else if (!old) check(false, "old_order: no boot");
  else {
    check(old.pages_only === false, `old_order: a full start (pages-only start: ${old.pages_only === null ? "no first-frame line" : old.pages_only ? "yes" : "no"})`);
    const c = old.counts[0];
    check(c && c.built && c.render >= WORLD_MIN_RENDER, `old_order: the world's renderer built at start (${c ? `${c.render} render pipelines` : "no count"})`);
    check(old.panics === 0, `old_order: no panic (${old.panics})`);
  }

  const pd = boots.play_during && parseLog(boots.play_during.log);
  if (!asked("play_during")) {
    // not asked for
  } else if (!pd) check(false, "play_during: no boot");
  else {
    busyShare("play_during", pd);
    check(pd.pressed, `play_during: Play pressed before the world was ready, and waited for it (${pd.pressed ? "yes" : "no such line"})`);
    check(pd.opened_ms !== null, `play_during: the world opened by itself once ready (${pd.opened_ms !== null ? `${pd.opened_ms} ms after Play` : "never"})`);
    check(enteredOk(boots.play_during), `play_during: the world entered (${JSON.stringify(boots.play_during.entered || null)})`);
    check(pd.strays.length === 0, `play_during: no reach for the world before it was ready (${pd.strays.length ? pd.strays.join("; ") : "none"})`);
    check(pd.panics === 0, `play_during: no panic (${pd.panics})`);
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

  return { ok: checks.length > 0 && checks.every((c) => c.ok), checks, numbers: numbers({ desk, bg, old, phone, before }, boots) };
}

/** The numbers the report states: the start times, the world's preparation, the idle frame counts. */
function numbers({ desk, bg, old, phone, before }, boots) {
  const idleOf = (p, mode) => {
    const w = p ? p.redraw.filter((x) => x.mode === mode) : [];
    return w.length ? w[w.length - 1] : null;
  };
  const waited = (b) => (b && b.entered && Number.isFinite(b.entered.waited_for_world_ms) ? b.entered.waited_for_world_ms : null);
  // The page frames once the world was ready (and before it was entered): the worst of those windows.
  const afterWins = bg ? bg.windows.filter((w) => w.world === "ready") : [];
  const after = afterWins.length
    ? { count: afterWins.reduce((s, w) => s + w.count, 0), mean_ms: afterWins[afterWins.length - 1].mean_ms, worst_ms: Math.max(...afterWins.map((w) => w.worst_ms)) }
    : null;
  return {
    first_frame_ms: { old_order: old ? old.first_frame_ms : null, desktop: desk ? desk.first_frame_ms : null, background: bg ? bg.first_frame_ms : null, phone: phone ? phone.first_frame_ms : null },
    idle_window: { before: idleOf(before, "continuous"), after: idleOf(phone, "reactive") },
    world_build_on_entry_ms: desk ? (desk.built_for.find((b) => b.reason === "entering the world") || {}).ms || null : null,
    world_ready_after_play_ms: desk ? desk.prep_ms : null,
    world_ready_ms: bg ? bg.ready_ms : null,
    on_frame_worst_ms: { desktop: desk && desk.on_frame ? desk.on_frame.worst_ms : null, background: bg && bg.on_frame ? bg.on_frame.worst_ms : null },
    page_frames_during: bg ? bg.during : null,
    page_frames_during_play: desk ? desk.during : null,
    // The page frames with nothing being built (the default start before Play): the baseline.
    page_frames_idle: (() => {
      const w = desk ? desk.windows.filter((x) => x.world === "not started") : [];
      return w.length ? { count: w.reduce((s, x) => s + x.count, 0), mean_ms: w[w.length - 1].mean_ms, worst_ms: Math.max(...w.map((x) => x.worst_ms)) } : null;
    })(),
    page_frames_after: after,
    // The slowest long gap of each stretch, with the cause its line names (idle gaps left out).
    worst_gaps: {
      before_play: desk ? worstSlow(desk.gaps, (g) => g.world === "not started") : null,
      on_play: desk ? worstSlow(desk.gaps, (g) => g.world.startsWith("being prepared")) : null,
      background: bg ? worstSlow(bg.gaps, (g) => g.world.startsWith("being prepared")) : null,
    },
    play:{ on_play_ms: waited(boots.desktop), during_ms: waited(boots.play_during), after_ms: waited(boots.background) },
  };
}

function verdictLines(result) {
  const lines = result.checks.map((c) => `${c.ok ? "  ok  " : "  FAIL"} ${c.what}`);
  const n = result.numbers || {};
  const ms = (v) => (v === null || v === undefined ? "n/a" : `${v} ms`);
  if (n.first_frame_ms) {
    const f = n.first_frame_ms;
    lines.push(`  first frame after the window opened (the pages ready): old order ${ms(f.old_order)}, desktop now ${ms(f.desktop)}, background ${ms(f.background)}, phone preview ${ms(f.phone)}`);
  }
  const of = n.on_frame_worst_ms || {};
  if (n.world_ready_after_play_ms) lines.push(`  the world ready after Play: ${n.world_ready_after_play_ms} ms (renderer ${ms(n.world_build_on_entry_ms)}); the frame's share worst ${ms(of.desktop)} (budget ${ON_FRAME_BUDGET_MS} ms)`);
  if (n.world_ready_ms) lines.push(`  background: the world ready ${n.world_ready_ms} ms after the window opened; the frame's share worst ${ms(of.background)} (budget ${ON_FRAME_BUDGET_MS} ms)`);
  const pfr = (p) => (p ? `${p.count} frames, ${p.mean_ms} ms apart on average, worst ${p.worst_ms} ms` : "n/a");
  if (n.page_frames_during || n.page_frames_after) lines.push(`  background: page frames while the world was prepared: ${pfr(n.page_frames_during)}; after: ${pfr(n.page_frames_after)}`);
  if (n.page_frames_idle) lines.push(`  page frames with nothing being built (before Play): ${pfr(n.page_frames_idle)}`);
  if (n.page_frames_during_play) lines.push(`  on Play: page frames (the progress card) while the world was prepared: ${pfr(n.page_frames_during_play)}`);
  const wg = n.worst_gaps || {};
  for (const [k, words] of [["before_play", "before Play"], ["on_play", "on Play"], ["background", "background"]]) {
    if (wg[k]) lines.push(`  the slowest page frame ${words}: ${gapWords(wg[k])}`);
  }
  if (n.play) lines.push(`  Play to the world loaded: the default start ${ms(n.play.on_play_ms)}; background, during the preparation ${ms(n.play.during_ms)}, after it ${ms(n.play.after_ms)}`);
  if (n.idle_window) {
    const f = (w) => (w ? `${w.frames} frames in ${w.secs} s` : "none");
    lines.push(`  an idle window: before (every frame) ${f(n.idle_window.before)}, after (on change) ${f(n.idle_window.after)}`);
  }
  lines.push(result.ok ? "PHONE START: PASS" : "PHONE START: FAIL");
  return lines;
}

module.exports = { parseLog, classifyGap, judgePhoneStart, verdictLines, PAGES_MAX_RENDER, IDLE_MAX_FRAMES, BEFORE_MIN_FRAMES, WORLD_MIN_RENDER, ON_FRAME_BUDGET_MS, IDLE_STRETCH_MAX_MS, BUSY_LONG_GAP_SHARE_MAX, BUSY_WORST_GAP_MAX_MS, BOOTS };
