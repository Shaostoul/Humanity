// The falls judge (scripts/lib/falls-judge.js): what `just verify-falls` passes and fails.
//
// Run: node --test scripts/tests/falls-judge.test.js   (in `just rig-tests`)
//
// Pure node: nothing is booted. A made-up run that went right passes every check; then one broken
// thing at a time, each failing its own checks. The made-up run is built from the tree's own data
// (falls.ron's curve, game.csv's gravity, the homestead's stairs), so a change to the data moves the
// run with it, and the readers are tested on the real files too. Last, the same runs written to a
// folder are judged by the rig itself, `node scripts/verify-falls.js --dry-verdict`, as a saved run
// would be.
//
// THE SHAPE THAT MATTERS MOST is `handOffRemoved`: what the running game reports when the walk's
// landing never reaches the body (lib.rs's `take_landing` -> `falls::player_landed` hand-off taken
// out, or a FallWatch that swallows every landing). The camera still drops 3 m, the health and the
// wounds do not move, and the landing count stays where it was. Every unit test of falls stays
// green in that build; this judge must not.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const FJ = require("../lib/falls-judge.js");

const REPO = path.join(__dirname, "..", "..");
const DATA = FJ.readTreeData(REPO);
const G = DATA.gravity_m_s2;
const ST = DATA.stairs;
const EYE = 1.7;
const V3 = Math.sqrt(2 * G * ST.size[1]); // the walk's landing off the top step
const SPRAIN = DATA.falls.steps[0].effect;
const HARM3 = FJ.expectFall(DATA.falls, V3, "realistic").harm;
const LD = DATA.ladder;
const VL = Math.sqrt(2 * G * (LD.hang_eye_m - EYE)); // off the ladder's top
const LADDER = FJ.expectFall(DATA.falls, VL, "realistic");
const HANG_S = 1.5;

// ── The made-up run ─────────────────────────────────────────────────────────────────────────────

/** A camera track at 60 frames a second: standing at the start, walking up the stairs, along the
 *  top step, off it (falling under G) and standing where it landed; or, `placed`, standing, then
 *  put at the top's height and falling; or, `ladder`, climbing the ladder at 3 m/s, hanging at
 *  its top for `hangS`, stepping back off it and falling. `health(t, landedT)` gives each frame's
 *  health. */
function track({ placed = false, ladder = false, hangS = HANG_S + 0.7, health = () => 100, climb = true } = {}) {
  const dt = 1 / 60;
  const frames = [];
  let t = 0;
  let [x, y, z] = ladder ? LD.stand : ST.foot;
  const push = () => {
    frames.push({ t, cam: [x, y, z, ST.yaw, 0], health: null });
    t += dt;
  };
  for (let i = 0; i < 18; i++) push();
  const top = ladder ? LD.hang_eye_m : ST.top_m + EYE;
  if (ladder) {
    while (y < top) {
      y = Math.min(top, y + 3 * dt);
      push();
    }
    for (let k = 0; k < hangS / dt; k++) push();
    for (let k = 0; k < 10; k++) {
      z += 3.75 * dt;
      push();
    }
  } else if (placed) {
    y = top;
  } else {
    const climbFrames = 45;
    for (let i = 1; i <= climbFrames; i++) {
      z += (ST.walk_m / climbFrames) * 1.0;
      y = climb ? EYE + (top - EYE) * Math.min(1, i / (climbFrames - 6)) : EYE;
      push();
    }
  }
  // The fall: v = g t from rest at the top (a walk off the edge has no vertical speed).
  let v = 0;
  while (y > EYE) {
    push();
    v += G * dt;
    y = Math.max(EYE, y - v * dt);
    if (!placed) z += 5 * dt;
  }
  const landedT = t;
  for (let i = 0; i < 180; i++) push();
  for (const f of frames) f.health = health(f.t, landedT);
  return frames;
}

const fallsBlock = (over = {}) => ({ landings: 0, last_m_s: null, mode: "forgiving", gravity_m_s2: G, flying: false, aboard: true, ...over });
const sprain = () => ({ id: SPRAIN, name: DATA.effects[SPRAIN].name, remaining_s: 1209600 });
const vitals = (health, wounds = []) => ({ satiation: 80, hydration: 80, health, health_max: 100, wounds, registry: true });
const snap = (falls, v) => ({ falls, vitals: v, home_box: { min: [0, 0, 0], max: [55, 3, 89] }, frames: [{ t: 0, cam: [...ST.foot, ST.yaw, 0], health: v.health }] });

const broken = () => ({ id: LADDER.wound, name: DATA.effects[LADDER.wound].name, remaining_s: 3628800 });

/** A run that went right: nothing landed getting into the world; the walks land at V3, Forgiving
 *  hurts nobody, Realistic sprains and takes HARM3 two frames after the landing; the ladder holds
 *  on at its top and stepping off it lands at VL, its wound and harm; and the placement's drop
 *  reaches no body. */
function goodRun() {
  const after = 100 - HARM3;
  const afterL = after - LADDER.harm;
  const landedHurt = (t, landedT) => (t >= landedT + 2 / 60 ? after : 100);
  const ladderHurt = (t, landedT) => (t >= landedT + 2 / 60 ? afterL : after);
  return {
    data: DATA,
    entry: snap(fallsBlock(), vitals(100)),
    legs: [
      {
        id: "forgiving_walk",
        mode: "forgiving",
        kind: "walk",
        start: ST.foot.slice(),
        before: snap(fallsBlock(), vitals(100)),
        after: { falls: fallsBlock({ landings: 1, last_m_s: V3 }), vitals: vitals(100), frames: track() },
      },
      {
        id: "realistic_walk",
        mode: "realistic",
        kind: "walk",
        start: ST.foot.slice(),
        before: snap(fallsBlock({ landings: 1, last_m_s: V3, mode: "realistic" }), vitals(100)),
        after: { falls: fallsBlock({ landings: 2, last_m_s: V3, mode: "realistic" }), vitals: vitals(after, [sprain()]), frames: track({ health: landedHurt }) },
      },
      {
        id: "realistic_ladder",
        mode: "realistic",
        kind: "ladder",
        start: LD.stand.slice(),
        hang_s: HANG_S,
        before: snap(fallsBlock({ landings: 2, last_m_s: V3, mode: "realistic" }), vitals(after, [sprain()])),
        after: {
          falls: fallsBlock({ landings: 3, last_m_s: VL, mode: "realistic" }),
          vitals: vitals(afterL, [sprain(), broken()]),
          frames: track({ ladder: true, health: ladderHurt }),
        },
      },
      {
        id: "placed_drop",
        mode: "realistic",
        kind: "placement",
        start: ST.placed_eye.slice(),
        before: snap(fallsBlock({ landings: 3, last_m_s: VL, mode: "realistic" }), vitals(afterL, [sprain(), broken()])),
        after: {
          falls: fallsBlock({ landings: 3, last_m_s: VL, mode: "realistic" }),
          vitals: vitals(afterL, [sprain(), broken()]),
          frames: track({ placed: true, health: () => afterL }),
        },
      },
    ],
    panics: 0,
    builtin_data: [],
  };
}
const clone = (x) => JSON.parse(JSON.stringify(x));
const leg = (run, id) => run.legs.find((l) => l.id === id);
const failed = (run) =>
  FJ.judgeFalls(run)
    .checks.filter((c) => !c.ok)
    .map((c) => c.id)
    .sort();

/** What the game reports with the lib.rs hand-off taken out: every landing stays in the watch. */
function handOffRemoved() {
  const run = goodRun();
  for (const l of run.legs) {
    for (const r of [l.before, l.after]) {
      r.falls.landings = 0;
      r.falls.last_m_s = null;
      r.vitals = vitals(100);
    }
    for (const f of l.after.frames) f.health = 100;
  }
  return run;
}

// ── The readers, on the tree's own files ────────────────────────────────────────────────────────

test("the readers read the tree: falls.ron's curve, game.csv's gravity, the effects' names, the homestead's stairs", () => {
  assert.strictEqual(DATA.falls.speed_share, 0.7);
  assert.deepStrictEqual(DATA.falls.harm[0], { m_s: 6.26, health: 2 });
  assert.deepStrictEqual(
    DATA.falls.steps.map((s) => s.effect),
    ["sprained_ankle", "broken_ankle", "broken_leg"]
  );
  assert.strictEqual(DATA.falls.landed, "You land hard.");
  assert.strictEqual(G, 9.81);
  assert.strictEqual(DATA.effects.sprained_ankle.name, "Sprained Ankle");
  assert.strictEqual(DATA.effects.sprained_ankle.kind, "wound");
  // The tallest staircase aboard: stairs-1, 3 m, climbed toward +z (south), so facing yaw pi.
  assert.strictEqual(ST.id, "stairs-1");
  assert.strictEqual(ST.top_m, 3);
  assert.ok(Math.abs(ST.yaw - Math.PI) < 1e-9);
  assert.ok(ST.foot[2] < ST.pos[2] - ST.size[2] / 2, "the walk starts off the stairs, before the first step");
  assert.strictEqual(ST.placed_eye[1], ST.top_m + EYE);
});

test("the judge's own reading of falls.ron gives what falls.ron's comments say", () => {
  // A 3 m drop on the walk's gravity: 7.67 m/s, a sprain and 9.7 health in Realistic, nothing in
  // Forgiving (read at 70%: 5.37, under the first harm at 6.26).
  assert.ok(Math.abs(V3 - 7.672) < 0.001, `${V3}`);
  const real = FJ.expectFall(DATA.falls, V3, "realistic");
  const soft = FJ.expectFall(DATA.falls, V3, "forgiving");
  assert.strictEqual(real.wound, "sprained_ankle");
  assert.ok(Math.abs(real.harm - 9.685) < 0.01, `${real.harm}`);
  assert.deepStrictEqual([soft.wound, soft.harm], [null, 0]);
  assert.ok(Math.abs(soft.read - 5.370) < 0.001);
  // The file's points exactly, and its "six storeys (18.94 m/s) takes about 117".
  for (const p of DATA.falls.harm) assert.ok(Math.abs(FJ.harmAt(DATA.falls, p.m_s) - p.health) < 1e-9, `${p.m_s}`);
  assert.strictEqual(Math.round(FJ.harmAt(DATA.falls, 18.94)), 117);
  assert.strictEqual(FJ.harmAt(DATA.falls, 6.25), 0);
  assert.strictEqual(FJ.woundAt(DATA.falls, 7.73), "broken_ankle");
  assert.strictEqual(FJ.woundAt(DATA.falls, 10.93), "broken_leg");
});

// ── The judgement ────────────────────────────────────────────────────────────────────────────────

test("a run that went right passes every check, the landing-time check included", () => {
  const r = FJ.judgeFalls(goodRun());
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  const ids = r.checks.map((c) => c.id);
  for (const want of [
    "forgiving_walk.dropped",
    "forgiving_walk.landing_handed",
    "forgiving_walk.landing_speed",
    "forgiving_walk.wound",
    "forgiving_walk.health",
    "realistic_walk.wound",
    "realistic_walk.wound_named",
    "realistic_walk.health",
    "realistic_walk.harm_at_landing",
    "realistic_ladder.held_on",
    "realistic_ladder.landing_handed",
    "realistic_ladder.wound",
    "realistic_ladder.health",
    "realistic_ladder.harm_at_landing",
    "world_entry",
    "placed_drop.dropped",
    "placed_drop.no_landing",
    "placed_drop.no_wound",
    "placed_drop.health",
    "panics",
    "no_builtin_data",
  ]) {
    assert.ok(ids.includes(want), `${want} was never judged`);
  }
  assert.ok(FJ.verdictLines(r).at(-1).startsWith("PASS"));
});

test("RED: the landing never reaches the body (the lib.rs hand-off taken out)", () => {
  assert.deepStrictEqual(failed(handOffRemoved()), [
    "forgiving_walk.landing_handed",
    "realistic_ladder.health",
    "realistic_ladder.landing_handed",
    "realistic_ladder.wound",
    "realistic_walk.health",
    "realistic_walk.landing_handed",
    "realistic_walk.wound",
  ]);
  const r = FJ.judgeFalls(handOffRemoved());
  const handed = r.checks.find((c) => c.id === "realistic_walk.landing_handed");
  assert.match(handed.detail, /0 landings reached the body for a 3\.00 m drop/);
  assert.match(r.checks.find((c) => c.id === "realistic_walk.wound").detail, /gives Sprained Ankle \(sprained_ankle\); the body has no wound/);
});

test("RED: a walk that never went off the top is not a pass for Forgiving", () => {
  const run = handOffRemoved();
  // No drop at all in the Forgiving leg (the walk never climbed): "no wound" would be vacuous.
  leg(run, "forgiving_walk").after.frames = track({ climb: false }).map((f) => ({ ...f, cam: [f.cam[0], EYE, f.cam[2], f.cam[3], f.cam[4]] }));
  const r = FJ.judgeFalls(run);
  const d = r.checks.find((c) => c.id === "forgiving_walk.dropped");
  assert.strictEqual(d.ok, false);
  assert.match(d.detail, /says nothing about falls/);
  assert.ok(!r.checks.some((c) => c.id === "forgiving_walk.wound"), "nothing about wounds is judged on a leg that never fell");
});

test("RED: Forgiving read at full speed (the mode's share not applied)", () => {
  const run = goodRun();
  const f = leg(run, "forgiving_walk");
  f.after.vitals = vitals(100 - HARM3, [sprain()]);
  for (const fr of f.after.frames) fr.health = 100 - HARM3;
  assert.deepStrictEqual(failed(run).filter((id) => id.startsWith("forgiving_walk")), ["forgiving_walk.health", "forgiving_walk.wound"]);
});

test("RED: a placement's drop reaches the body (FallWatch's exemption broken)", () => {
  const run = goodRun();
  const p = leg(run, "placed_drop");
  const h = p.before.vitals.health;
  p.after.falls = fallsBlock({ landings: 4, last_m_s: V3, mode: "realistic" });
  p.after.vitals = vitals(h - HARM3, [sprain(), broken(), { id: "broken_leg", name: "Broken Leg", remaining_s: 7257600 }]);
  assert.deepStrictEqual(failed(run), ["placed_drop.health", "placed_drop.no_landing", "placed_drop.no_wound"]);
});

test("RED: the ladder lets go of a body when the climb key comes up", () => {
  const run = goodRun();
  const l = leg(run, "realistic_ladder");
  // At the top only while the key was still held after the climb (0.4 s), then straight down.
  const after = l.before.vitals.health - LADDER.harm;
  l.after.frames = track({ ladder: true, hangS: 0.4, health: (t, landedT) => (t >= landedT + 2 / 60 ? after : l.before.vitals.health) });
  assert.deepStrictEqual(failed(run), ["realistic_ladder.held_on"]);
});

test("RED: a landing reached the body on the way into the world", () => {
  const run = goodRun();
  run.entry = snap(fallsBlock({ landings: 1, last_m_s: 9.0 }), vitals(88, [sprain()]));
  assert.deepStrictEqual(failed(run), ["world_entry"]);
});

// ── Real captures from the running game (tests/fixtures/falls/) ──────────────────────────────────
// Two runs of `just verify-falls` on 2026-10-20, trimmed to what the judge reads. GREEN is this
// tree's build. RED is a build with lib.rs's `take_landing` -> `falls::player_landed` hand-off
// commented out: in the running game the camera dropped 3.00 m off the stairs and 3.20 m off the
// ladder, and the body's health, wounds and landing count never moved. Each carries the data it ran
// on, so it is judged against that, not against today's falls.ron.

const FIXTURES = path.join(REPO, "tests", "fixtures", "falls");
const fixture = (name) => JSON.parse(fs.readFileSync(path.join(FIXTURES, name), "utf8"));

test("the real green run passes every check (its walk skipped the top step at 54 ms frames, and the judge followed the real drop)", () => {
  const run = fixture("green.json");
  const r = FJ.judgeFalls(run);
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  // The realistic walk left from the 13th step: 2.79 m, 7.39 m/s, a sprain and 8.17 health, which
  // a judge expecting the nominal 3 m (9.68 health) would have failed.
  const w = r.observed.realistic_walk;
  assert.ok(Math.abs(w.drop_m - 2.79) < 0.01 && Math.abs(w.landing_m_s - 7.39) < 0.01, JSON.stringify(w));
  assert.deepStrictEqual(r.observed.realistic_ladder.new_wounds, ["broken_ankle"]);
  assert.strictEqual(run.falls_log.length, 3, "one [Falls] line a landing that reached the body");
});

test("the real red run (the lib.rs hand-off taken out) fails on every leg a landing should have reached", () => {
  assert.deepStrictEqual(failed(fixture("red-hand-off-removed.json")), [
    "forgiving_walk.landing_handed",
    "realistic_ladder.health",
    "realistic_ladder.landing_handed",
    "realistic_ladder.wound",
    "realistic_walk.health",
    "realistic_walk.landing_handed",
    "realistic_walk.wound",
  ]);
});

test("RED: the walk lands at a speed its own drop does not give", () => {
  const run = goodRun();
  leg(run, "realistic_walk").after.falls.last_m_s = 3.0; // the frame's velocity, say, not sqrt(2 g h)
  const f = failed(run);
  assert.ok(f.includes("realistic_walk.landing_speed"), f.join(", "));
});

test("RED: the mode verb not taken, dev flight on, the wrong gravity", () => {
  const a = goodRun();
  leg(a, "realistic_walk").after.falls.mode = "forgiving";
  assert.ok(failed(a).includes("realistic_walk.mode"));
  const b = goodRun();
  leg(b, "forgiving_walk").before.falls.flying = true;
  assert.ok(failed(b).includes("forgiving_walk.on_foot_aboard"));
  const c = goodRun();
  leg(c, "forgiving_walk").after.falls.gravity_m_s2 = 1.62;
  assert.ok(failed(c).includes("forgiving_walk.gravity"));
});

test("RED: the health fell before the eye landed, or long after", () => {
  const early = goodRun();
  for (const fr of leg(early, "realistic_walk").after.frames) fr.health = 100 - HARM3;
  assert.deepStrictEqual(failed(early), ["realistic_walk.harm_at_landing"]);
  const late = goodRun();
  const frames = leg(late, "realistic_walk").after.frames;
  const landedT = FJ.dropOf(frames).landed_t;
  for (const fr of frames) fr.health = fr.t >= landedT + 2.5 ? 100 - HARM3 : 100;
  assert.deepStrictEqual(failed(late), ["realistic_walk.harm_at_landing"]);
});

test("RED: a build older than the body probe, a missing leg, a panic, built-in data", () => {
  const old = goodRun();
  delete leg(old, "realistic_walk").after.vitals.health;
  assert.ok(failed(old).includes("realistic_walk.body_reported"));
  const short = goodRun();
  short.legs = short.legs.filter((l) => l.id !== "placed_drop");
  assert.ok(failed(short).includes("placed_drop.ran"));
  const loud = goodRun();
  loud.panics = 1;
  loud.builtin_data = ["[built-in data copy] medical/falls.ron"];
  assert.deepStrictEqual(failed(loud), ["no_builtin_data", "panics"]);
});

// ── The rig's --dry-verdict over saved runs ──────────────────────────────────────────────────────

test("the rig judges a saved run: green passes, the hand-off shape fails, nothing is booted", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hos-falls-judge-"));
  try {
    for (const [name, run, code, word] of [
      ["green", goodRun(), 0, /DRY VERDICT \(nothing was booted\): PASS/],
      ["red", handOffRemoved(), 2, /DRY VERDICT \(nothing was booted\): FAIL .*realistic_walk\.landing_handed/],
    ]) {
      const p = path.join(dir, `${name}.json`);
      fs.writeFileSync(p, JSON.stringify(run));
      const out = spawnSync(process.execPath, [path.join(REPO, "scripts", "verify-falls.js"), "--dry-verdict", p], { encoding: "utf8", timeout: 60000 });
      const text = `${out.stdout}\n${out.stderr}`;
      assert.strictEqual(out.status, code, text);
      assert.match(text, word);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
