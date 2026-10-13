// The starts judge (scripts/lib/starts-judge.js): what `just verify-starts`
// (verify-copresence.js --starts, ship homes increment 6, 2026-10-12) passes and fails.
//
// Run: node --test scripts/tests/starts-judge.test.js   (in `just rig-tests`)
//
// Pure node: nothing is booted. A made-up run that went right passes every check; then one broken
// thing at a time, each failing its own check and no other (the plan's proof for the judge,
// docs/design/ship-homes-increment-6-plan.md section 6: "Every check is also shown failing alone
// on a doctored copy of a green manifest"). Then the same run written to a folder is judged by the
// rig itself, `node scripts/verify-copresence.js --dry-verdict`, as a saved run would be.
//
// THE MADE-UP RUN IS BUILT FROM THE TREE'S OWN DATA (`readTreeData`): the Cabin's kit, the plots,
// the counter, the meal's good, as the rig reads them when it runs. So the readers are tested on
// the real files too, and a change to the data (another kit, another cabin row) moves the run with
// it instead of breaking it.
//
// Red first, 2026-10-12: with the rig's --dry-verdict not knowing a --starts manifest, the saved run
// was judged as the default rig's ("DRY VERDICT (nothing was booted): FAIL  2/12 passed; failed:
// relay_up, game_in_world, camera_parked, walker_ran, approach_clear, view_clear, screenshots,
// on_screen, figure_visible, recorded"). Then three breaks made on purpose in the judge, each seen
// red here and put back (quoted beside what each broke): fresh_on_server comparing only the kit's
// own items (not the whole backpack), cabin_plot looking for a key only as a whole key, and
// walks_to_mess_hall without its relayed-steps test.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const SJ = require("../lib/starts-judge.js");
const SP = require("../second-player.js");

const REPO = path.join(__dirname, "..", "..");
const DATA = SJ.readTreeData(REPO);

// ── The made-up run ─────────────────────────────────────────────────────────────────────────────

const CAB = DATA.starts[DATA.chosen];
const DEF = DATA.starts[DATA.default_start];
const CABINS = DATA.plots.filter((p) => p.kind === CAB.plot_kind);
const C1 = CABINS[0];
const C2 = CABINS[1];
const P1 = DATA.plots.find((p) => p.id === DATA.default_plot);
const boxOf = (p) => ({ min: p.origin.slice(), max: [0, 1, 2].map((k) => p.origin[k] + p.size[k]) });
const plotJson = (p) => ({ id: p.id, kind: p.kind, origin: p.origin.slice(), size: p.size.slice() });
const STAND = DATA.counter.stand;
const LOOK = { name: "CopresenceStarts", appearance: { skin: [0.6, 0.45, 0.35], hair: [0.1, 0.08, 0.05], height: 1.0 }, outfit: { top: "shirt_basic", bottom: "trousers_basic" } };
const SERVER = "http://127.0.0.1:61234";
const GAME_KEY = "a1".repeat(40);
const WALKER_KEY = "b2".repeat(40);

/** What the Dev stock adds: things no Cabin carries, and more of the homestead's own. */
const STOCK = { iron_ingot_0: 99, wood_plank_0: 99, copper_wire_0: 50, ration_basic_0: 99 };
const MARK = (() => {
  const b = { ...DEF.kit };
  for (const [k, n] of Object.entries(STOCK)) b[k] = (b[k] || 0) + n;
  return b;
})();
/** The Cabin's kit with its empty water vessels filled (what the counter's Fill your bottles does). */
const AFTER_WATER = (() => {
  const b = { ...CAB.kit };
  for (const f of DATA.fillables) {
    const n = b[f.empty] || 0;
    if (!n) continue;
    delete b[f.empty];
    b[f.full] = (b[f.full] || 0) + n;
  }
  return b;
})();
const FILLED = DATA.fillables.reduce((a, f) => a + (CAB.kit[f.empty] || 0), 0);

/** A `home_session` block as lane 2B's probe writes it (src/engine/ipc.rs home_session_core). */
function session(over = {}) {
  return {
    slot: "offline",
    wanted: "offline",
    pairing: "home:",
    path: "saves/offline_home.json",
    server: "",
    start: DEF.id,
    household: DEF.households[0],
    ship_place: null,
    home: { kind: DEF.plot_kind, plot: P1.id, away: false, box: boxOf(P1) },
    backpack: { ...DEF.kit },
    skills: 0,
    credits: DATA.default_credits,
    quests: { active: [{ id: DEF.opening, step: 0 }], completed: [] },
    look: JSON.parse(JSON.stringify(LOOK)),
    vitals: { satiation: 80, hydration: 80 },
    own_home: false,
    meals_offline: { taken: 0, next_at_game_s: 0, water_l: 0 },
    counter: "",
    fleet_meals: null,
    answer: null,
    own_save: { path: "saves/offline_home.json", exists: false },
    server_save: null,
    last_verb: null,
    ...over,
  };
}
/** The server's cabin as a session reports it. */
function cabinSession(over = {}) {
  return session({
    slot: "server",
    wanted: "server",
    pairing: "server:__connected__",
    path: "saves/servers/0123456789abcdef-a1a1a1a1a1a1a1a1.json",
    server: SERVER,
    start: CAB.id,
    household: CAB.households[0],
    ship_place: { ship_id: "mothership-1", ship_hash: "abc", plot: C2.id },
    home: { kind: CAB.plot_kind, plot: C2.id, away: false, box: boxOf(C2) },
    backpack: { ...CAB.kit },
    quests: { active: [{ id: CAB.opening, step: 0 }], completed: [] },
    ...over,
  });
}
const verb = (v, said, ok = true) => ({ verb: v, value: v === "pairing" ? "server" : v === "choose_start" ? CAB.id : "1", ok, said });
/** "cabin 15, homestead 12": every offered start's plots, less the walker's cabin. */
const FREE = DATA.start_order
  .filter((id) => DATA.starts[id].offered)
  .map((id) => `${id} ${DATA.plots.filter((p) => p.kind === DATA.starts[id].plot_kind && p.id !== C1.id).length}`)
  .join(", ");
// The game's walk to the counter: its door, the doors on the way, the counter (made-up points; the
// judge only asks that each was passed on, in order, and the last is the counter's place).
const STEPS = [
  [C2.origin[0] + 1, 1.7, C2.origin[2] + 1.25],
  [78.5, 1.7, 92],
  [70, 1.7, 92],
  [70, 1.7, 76],
  STAND.slice(),
];

function goodRun() {
  const run = {
    kind: "verify-copresence-starts",
    stamp: "20261012-230000",
    data: JSON.parse(JSON.stringify(DATA)),
    // `look`: the look right before the swap, what must cross into the server's world.
    offline: { session: session(), joined: false, ws_identified: true, before: { ...DEF.kit }, mark: { ...MARK }, look: JSON.parse(JSON.stringify(LOOK)) },
    walker: {
      name: "TestBotCabin",
      plot: C1.id,
      home_plot: { ...plotJson(C1), start: CAB.id },
      home_line: `second-player: home: ${C1.id} ${CAB.plot_kind}; free ${FREE}`,
    },
    choose: { verb: verb("choose_start", "asked the server"), answer: { type: "game_home", plot: { id: C2.id, kind: CAB.plot_kind, start: CAB.id }, starts: [], chose: true } },
    server: {
      verb: verb("pairing", "paired"),
      session: cabinSession(),
      joined: true,
      welcomed: true,
      last_welcome: "stay",
      home_plot: plotJson(C2),
      camera: [C2.origin[0] + 1, 1.7, C2.origin[2] + 1.25],
    },
    walk: {
      from: STEPS[0],
      stand: STAND.slice(),
      steps: STEPS.map((p) => p.slice()),
      doors: [`plot:${C2.id}->zone:cabins-1`, "zone:cabins-1->zone:street-1", "zone:street-1->zone:commons"],
      arrived: true,
      at: STAND.slice(),
      // The walker logs a move every 0.25 m: each step's last point is near it, with others between.
      relayed: [STEPS[0], [80, 1.7, 91.8], STEPS[1], [74, 1.7, 92], STEPS[2], [70, 1.7, 84], STEPS[3], [68, 1.7, 50], [STAND[0], 1.7, STAND[2] + 0.2]],
      corrections_before: 0,
      corrections_after: 0,
      vitals_step: DATA.opening.walk,
      quest_step: DATA.opening.after_walk,
    },
    meal: {
      verb: verb("meal", SJ.SENTENCES.asked),
      before: { backpack: { ...CAB.kit }, vitals: { satiation: 72.5, hydration: 78 } },
      after: { backpack: { ...CAB.kit }, vitals: { satiation: 91.5, hydration: 78 }, counter: SJ.SENTENCES.eaten, fleet_meals: [{ direction: "used", quantity: 1, value: 10 }] },
    },
    meal_again: { verb: verb("meal", SJ.SENTENCES.asked), counter: "Your next meal is in 480 game minutes.", backpack: { ...CAB.kit }, fleet_meals: [{ direction: "used", quantity: 1, value: 10 }], vitals: { satiation: 91.4, hydration: 78 } },
    water: { verb: verb("water", `Filled ${FILLED} x Water bottle (1 L) from the ship's water.`), before: { backpack: { ...CAB.kit } }, after: { backpack: { ...AFTER_WATER } } },
    walker_meal: { arrived: true, route_m: 84.2, line: `second-player: meal taken: ${DATA.meal_good}`, error: null },
    opening: { session: cabinSession({ backpack: { ...AFTER_WATER }, quests: { active: [{ id: CAB.opening, step: DATA.opening.after_meal }], completed: [] } }) },
    home_again: {
      verb: verb("pairing", "paired"),
      session: session({
        backpack: { ...MARK },
        server_save: { path: "saves/servers/0123456789abcdef-a1a1a1a1a1a1a1a1.json", exists: true, start: CAB.id, household: CAB.households[0], ship_place: { ship_id: "mothership-1", ship_hash: "abc", plot: C2.id }, ship_meals: { taken: 0, next_at_game_s: 0, water_l: 1 }, backpack: { ...AFTER_WATER } },
      }),
      joined: false,
      home_plot: plotJson(P1),
    },
    server_again: {
      verb: verb("pairing", "paired"),
      session: cabinSession({ backpack: { ...AFTER_WATER }, quests: { active: [{ id: CAB.opening, step: DATA.opening.after_meal }], completed: [] } }),
      joined: true,
      last_welcome: "stay",
      home_plot: plotJson(C2),
      camera: [C2.origin[0] + 1, 1.7, C2.origin[2] + 1.25],
    },
    walks: [{ label: "to_mess_hall", to: STAND.slice(), at: STAND.slice(), ok: true }],
    turns: [],
    corrections_game: 0,
    walker_corrections: [],
    walker_keys: [WALKER_KEY.slice(0, 16)],
    panics: 0,
    builtin_data: [],
  };
  const relayLog = [
    "2026-10-12T23:00:01.000Z  INFO humanity_engine::relay::handlers::game_state: Game: ship mothership-1 (abc) has 28 plot(s)",
    `2026-10-12T23:00:05.000Z  INFO humanity_engine::relay::handlers::msg_handlers: Game: player 'TestBotCabin' (${WALKER_KEY.slice(0, 16)}...) joined as entity 1`,
    "2026-10-12T23:00:09.000Z  INFO humanity_engine::relay::handlers::home_plots: Game: home chosen (cabin)",
    `2026-10-12T23:00:12.000Z  INFO humanity_engine::relay::handlers::msg_handlers: Game: player 'CopresenceStarts' (${GAME_KEY.slice(0, 16)}...) joined as entity 2`,
  ].join("\n");
  return { run, evidence: { relayLog } };
}

const ALL_IDS = [
  "offline_marked",
  "walker_cabin",
  "walker_home_answer",
  "fresh_on_server",
  "look_crossed",
  "cabin_plot",
  "walks_to_mess_hall",
  "meal_eaten",
  "one_meal_a_meal_time",
  "bottles_filled",
  "walker_ate",
  "opening_follows",
  "offline_untouched",
  "server_save_kept",
  "server_home_remembered",
  "no_correction",
  "panics",
  "no_builtin_data",
];

const explain = (r) => r.checks.filter((c) => !c.ok).map((c) => `${c.id}: ${c.detail}`).join("\n");

// ── The tree's data, as the rig reads it ────────────────────────────────────────────────────────

test("the rig reads the tree's data it judges against, and every piece of it is there", () => {
  assert.deepEqual(DATA.errors, [], "readTreeData found everything");
  assert.ok(CAB && CAB.offered, `the ${SJ.CHOSEN_START} start is offered`);
  assert.ok(CAB.opening && DATA.opening && DATA.opening.quest === CAB.opening, "the Cabin's opening is read with its steps");
  assert.ok(Object.keys(CAB.kit).length > 0, "the Cabin's kit is read");
  assert.ok(FILLED > 0, "the Cabin carries empty water vessels to fill (data/containers/fluids.ron)");
  assert.ok(DEF && DEF.id !== CAB.id && DEF.plot_kind !== CAB.plot_kind, "the default start is another kind of home");
  assert.ok(C1 && C2, "the ship file has at least two plots of the Cabin's kind");
  assert.equal(P1 && P1.kind, DEF.plot_kind, "the ship's default plot is the default start's kind");
  // The counter: on the floor of the mess hall volume, its store in front of it, the place the
  // game stands in reach of both (5 m, the store's reach and the counter card's).
  const c = DATA.counter;
  assert.equal(c.volume.type_id, "mess_hall");
  for (const p of [c.centre, c.stand, c.store.at]) assert.ok(p[0] >= c.volume.min[0] && p[0] <= c.volume.max[0] && p[2] >= c.volume.min[2] && p[2] <= c.volume.max[2], `${JSON.stringify(p)} is in the mess hall`);
  const gap = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
  assert.ok(gap(c.stand, c.store.at) < SP.STORE_REACH_M - 1, "the game's place is well in reach of the food store");
  assert.ok(Math.abs(gap(c.stand, c.centre) - (c.size[2] / 2 + SJ.STAND_FRONT_M)) < 1e-6 || Math.abs(gap(c.stand, c.centre) - (c.size[0] / 2 + SJ.STAND_FRONT_M)) < 1e-6, "1.5 m in front of its front face");
  assert.ok(Number.isFinite(DATA.default_credits) && DATA.default_credits > 0, "a new character's credits");
  assert.ok(typeof DATA.meal_good === "string" && DATA.meal_good.length > 0, "the meal's good");
  assert.ok(DATA.opening.after_meal === DATA.opening.after_walk + 1, "the opening's meal step comes right after its walk");
});

test("a data file that cannot be read is named, and its pieces are null", () => {
  const d = SJ.readTreeData(REPO, (p) => {
    if (/ship_structure\.ron$/.test(p)) throw new Error("gone");
    return fs.readFileSync(p, "utf8");
  });
  assert.ok(d.errors.some((e) => /ship_structure\.ron could not be read/.test(e)), d.errors.join("; "));
  assert.deepEqual(d.plots, []);
  assert.equal(d.counter, null);
  assert.equal(d.default_plot, null);
});

test("the readers read the shapes the files are written in", () => {
  const starts = SJ.readStarts(`// a comment with (parens) and "quotes"
(
    default: "b",
    starts: [
        (
            id: "a", name: "A (small)", sentence: "x)", offered: true, design: "a", plot_kind: "ka",
            households: [(id: "solo", name: "Just you", machines: "a.ron")],
            kit: [("w_0", 2), ("w_0", 1), ("e_0", 4)],
            opening: Some("q_a"),
        ),
        (id: "b", name: "B", offered: false, design: "b", plot_kind: "kb", households: [], kit: [], opening: None),
    ],
)`);
  assert.deepEqual(starts.order, ["a", "b"]);
  assert.equal(starts.default, "b");
  assert.deepEqual(starts.starts.a.kit, { w_0: 3, e_0: 4 });
  assert.equal(starts.starts.a.name, "A (small)");
  assert.equal(starts.starts.a.opening, "q_a");
  assert.equal(starts.starts.b.opening, null);
  assert.equal(starts.starts.b.offered, false);
  assert.deepEqual(starts.starts.a.households, ["solo"]);
  assert.equal(SJ.readStarts("(starts: [])"), null, "no default: null");
  assert.equal(SJ.readDefaultCredits("impl Default for Wallet {\n fn default() -> Self { Self { credits: 12_500 } }\n}"), 12500);
  assert.equal(SJ.readMealGood('(id: "meal", direction: Used, label: "M", unit: "meal", price: OfGood("r_0"), per_day: false),'), "r_0");
  assert.deepEqual(SJ.readWaterFillables('(empty: "a_e", full: "a_f", fluid: "water", litres: 0.5),\n(empty: "m_e", full: "m_f", fluid: "milk", litres: 1.0),'), [{ empty: "a_e", full: "a_f", litres: 0.5 }]);
  const o = SJ.readOpening('(id: "q", steps: [(objective: View(view: "vitals")), (objective: Travel(destination: "mess_hall")), (objective: Eat(quantity: 1))]), (id: "next", steps: [(objective: Eat(quantity: 1))])', "q");
  assert.deepEqual([o.walk, o.eat, o.after_walk, o.after_meal], [1, 2, 2, 3]);
  assert.ok(SJ.readOpening('(id: "q", steps: [(objective: Eat(quantity: 1))])', "q").error, "no walk to the mess hall: an error");
});

// ── The judge ───────────────────────────────────────────────────────────────────────────────────

test("a --starts run that went right passes, every check its own", () => {
  const { run, evidence } = goodRun();
  const r = SJ.judgeStarts(run, evidence);
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), ALL_IDS);
});

/** The run with `fn` applied to it (and its evidence), judged. */
function doctored(fn) {
  const g = goodRun();
  fn(g.run, g.evidence);
  return SJ.judgeStarts(g.run, g.evidence);
}

// One broken thing at a time, each failing its own check and no other. Each case's check was SEEN
// failing on its doctored copy (this test asserts exactly that, case by case); where a case's
// check was also broken in the judge on purpose, the red run is quoted beside it.
const CASES = [
  // 2: marked offline.
  ["the game booted into the server's world", (r) => (r.offline.joined = true), "offline_marked"],
  ["its own home's save is a cabin", (r) => (r.offline.session.start = CAB.id), "offline_marked"],
  ["the live save at boot is a server's", (r) => (r.offline.session.slot = "server"), "offline_marked"],
  ["the stock added only things a Cabin carries", (r) => (r.offline.before = { ...MARK }), "offline_marked"],
  ["the stock never landed", (r) => (r.offline.mark = null), ["offline_marked", "offline_untouched"]],
  // 3: the walker.
  ["the walker's welcome names no start", (r) => delete r.walker.home_plot.start, "walker_cabin"],
  ["the walker came in a guest", (r) => (r.walker.home_plot = null), ["walker_cabin", "walker_home_answer"]],
  // The plan's red: assign_home ignoring `start` gives the walker the first homestead.
  ["the walker given a homestead (assign_home ignoring start)", (r) => (r.walker.home_plot = { ...plotJson(P1), start: DEF.id }), ["walker_cabin", "walker_home_answer"]],
  ["a held plot counted as free", (r) => (r.walker.home_line = `home: ${C1.id} ${CAB.plot_kind}; free ${FREE.replace(/(\d+)/, (n) => String(Number(n) + 1))}`), "walker_home_answer"],
  ["the not-offered Apartment listed as free", (r) => (r.walker.home_line += ", apartment 0"), "walker_home_answer"],
  ["the walker never asked where it lives", (r) => (r.walker.home_line = null), "walker_home_answer"],
  // 4: fresh on the server. The plan's red: save_slot::follow doing nothing leaves OFFLINE_MARK in
  // the backpack. Red in the judge 2026-10-12 with fresh_on_server comparing only the kit's own
  // items (so anything MORE passed): "7 gold ingots from home in the cabin should fail
  // fresh_on_server (only); failed: nothing". (The whole OFFLINE_MARK still failed that broken
  // judge, by the homestead kit's larger count of a water item the Cabin's kit also has: a whole
  // backpack is the weaker case.)
  ["the offline backpack came into the server's save", (r) => (r.server.session.backpack = { ...MARK }), "fresh_on_server"],
  ["7 gold ingots from home in the cabin", (r) => (r.server.session.backpack.gold_ingot_0 = 7), "fresh_on_server"],
  ["a kit bottle missing", (r) => (r.server.session.backpack.water_bottle_empty_0 = 1), "fresh_on_server"],
  ["the skills came along", (r) => (r.server.session.skills = 3), "fresh_on_server"],
  ["the home's credits came along", (r) => (r.server.session.credits = 50000), "fresh_on_server"],
  ["the homestead's opening on the server", (r) => (r.server.session.quests.active = [{ id: DEF.opening, step: 2 }]), "fresh_on_server"],
  ["no opening at all", (r) => (r.server.session.quests.active = []), "fresh_on_server"],
  ["the live save still your own home's", (r) => (r.server.session.slot = "offline"), "fresh_on_server"],
  ["a homestead save on the server", (r) => (r.server.session.start = DEF.id), "fresh_on_server"],
  ["the look left at home", (r) => (r.server.session.look = { ...LOOK, name: "Player" }), "look_crossed"],
  ["another outfit on the server", (r) => (r.server.session.look.outfit = { top: "vest" }), "look_crossed"],
  ["no look recorded before the swap", (r) => (r.offline.look = null), "look_crossed"],
  // 5: its home.
  ["the welcome moved the home", (r) => (r.server.last_welcome = "move"), "cabin_plot"],
  ["the home on another cabin", (r) => (r.server.session.home.plot = CABINS[2].id), "cabin_plot"],
  ["a homestead built on the cabin plot", (r) => (r.server.session.home.kind = DEF.plot_kind), "cabin_plot"],
  ["the home put away (a guest)", (r) => (r.server.session.home.away = true), "cabin_plot"],
  ["the welcome named another plot", (r) => (r.server.home_plot = plotJson(CABINS[2])), "cabin_plot"],
  ["the home's box 5 cm off the ship file's", (r) => (r.server.session.home.box.max[0] += 0.05), "cabin_plot"],
  ["the camera outside the cabin", (r) => (r.server.camera = [70, 1.7, 92]), "cabin_plot"],
  // Red in the judge 2026-10-12 with the key looked for only as a whole key: "the first 16 of a
  // key on the home-chosen line should fail cabin_plot (only); failed: nothing".
  ["the first 16 of a key on the home-chosen line", (r, e) => (e.relayLog = e.relayLog.replace("Game: home chosen (cabin)", `Game: home chosen (cabin) by ${GAME_KEY.slice(0, 16)}...`)), "cabin_plot"],
  ["a did:hum on the home-chosen line", (r, e) => (e.relayLog = e.relayLog.replace("Game: home chosen (cabin)", "Game: home chosen (cabin) did:hum:7GameYp2QsL5c8HdRu4Ej1")), "cabin_plot"],
  ["the relay never logged the choice", (r, e) => (e.relayLog = e.relayLog.replace(/^.*home chosen.*$/m, "")), "cabin_plot"],
  ["the choice logged refused", (r, e) => (e.relayLog = e.relayLog.replace("home chosen (cabin)", "home chosen (refused/full)")), "cabin_plot"],
  ["the relay's log never kept", (r, e) => (e.relayLog = undefined), ["cabin_plot", "no_correction"]],
  // 6: to the mess hall.
  ["the walk never arrived", (r) => (r.walk.arrived = false), "walks_to_mess_hall"],
  ["it stopped 2 m short of the counter", (r) => (r.walk.at = [STAND[0], 1.7, STAND[2] + 2]), "walks_to_mess_hall"],
  // Red in the judge 2026-10-12 with the relayed-steps test removed: "a step never passed on to
  // the walker should fail walks_to_mess_hall (only); failed: nothing".
  ["a step never passed on to the walker", (r) => (r.walk.relayed = r.walk.relayed.filter((p) => !(p[2] === 76))), "walks_to_mess_hall"],
  ["the steps passed on out of order", (r) => (r.walk.relayed = [...r.walk.relayed].reverse()), "walks_to_mess_hall"],
  ["what the relay passed on not recorded", (r) => (r.walk.relayed = null), "walks_to_mess_hall"],
  ["corrected on the way", (r) => (r.walk.corrections_after = 1), "walks_to_mess_hall"],
  // The plan's red for 1G: NearestVolume unresolved, so the walk to the mess hall never counts.
  ["the opening's walk step not done", (r) => (r.walk.quest_step = DATA.opening.walk), "walks_to_mess_hall"],
  // 7: eats and drinks. The plan's red: the counter's offline path taken while joined, a meal
  // not on the ledger.
  [
    "the ship's stores on this computer served it in the server's world",
    (r) => {
      r.meal.verb.said = SJ.SENTENCES.eaten_offline;
      r.meal.after.counter = SJ.SENTENCES.eaten_offline;
      r.meal.after.fleet_meals = [];
    },
    "meal_eaten",
  ],
  ["no ledger line for the meal", (r) => (r.meal.after.fleet_meals = []), "meal_eaten"],
  ["the ledger never came", (r) => (r.meal.after.fleet_meals = null), "meal_eaten"],
  ["the meal left in the backpack, not eaten", (r) => (r.meal.after.backpack[DATA.meal_good] = 1), "meal_eaten"],
  ["food not up", (r) => (r.meal.after.vitals.satiation = r.meal.before.vitals.satiation), "meal_eaten"],
  ["the server refused the meal", (r) => (r.meal.after.counter = "Walk closer to the stores to take a meal."), "meal_eaten"],
  ["a second meal served at once", (r) => Object.assign(r.meal_again, { counter: SJ.SENTENCES.eaten, fleet_meals: [{ direction: "used", quantity: 2, value: 20 }] }), "one_meal_a_meal_time"],
  ["the second ask never answered", (r) => (r.meal_again.counter = SJ.SENTENCES.asked), "one_meal_a_meal_time"],
  ["no second ask", (r) => (r.meal_again = null), "one_meal_a_meal_time"],
  ["the counter says no bottle to fill", (r) => (r.water.verb.said = "You carry no empty bottle to fill."), "bottles_filled"],
  ["the bottles were full already", (r) => (r.water.before.backpack = { ...AFTER_WATER }), "bottles_filled"],
  ["Fill your bottles never pressed", (r) => (r.water.verb = { verb: "meal", value: "1", ok: true, said: SJ.SENTENCES.asked }), "bottles_filled"],
  ["the walker's meal refused not_yet", (r) => (r.walker_meal.line = "second-player: meal refused: not_yet (the next in 28800 game s)"), "walker_ate"],
  ["the walker never reached the counter", (r) => Object.assign(r.walker_meal, { arrived: false, line: null, error: "the walker never reached its place at the counter" }), "walker_ate"],
  ["the walker ate something else", (r) => (r.walker_meal.line = "second-player: meal taken: a_meal"), "walker_ate"],
  // 8: the opening.
  ["the meal step not counted", (r) => (r.opening.session.quests.active[0].step = DATA.opening.after_walk), "opening_follows"],
  ["the opening gone", (r) => (r.opening.session.quests.active = []), "opening_follows"],
  ["the quests after the meal not recorded", (r) => (r.opening = null), "opening_follows"],
  // 9: home again. The plan's red: save_slot::follow doing nothing, so going home keeps the
  // cabin's things (and the server's save is never written).
  ["home again with the cabin's things", (r) => (r.home_again.session.backpack = { ...AFTER_WATER }), "offline_untouched"],
  ["home again to the default home (its progress dropped)", (r) => (r.home_again.session.backpack = { ...DEF.kit }), "offline_untouched"],
  ["still in the shared world at home", (r) => (r.home_again.joined = true), "offline_untouched"],
  ["the live save still the server's", (r) => (r.home_again.session.slot = "server"), "offline_untouched"],
  ["a cabin built at home", (r) => (r.home_again.session.home = { kind: CAB.plot_kind, plot: P1.id, away: false, box: boxOf(P1) }), "offline_untouched"],
  ["the home on another plot", (r) => (r.home_again.session.home.plot = CABINS[0].id), "offline_untouched"],
  ["the server's save not on disk", (r) => (r.home_again.session.server_save.exists = false), "server_save_kept"],
  ["the server's save holds the home's things", (r) => (r.home_again.session.server_save.backpack = { ...MARK }), "server_save_kept"],
  ["the server's save before the meal and water", (r) => (r.home_again.session.server_save.backpack = { ...CAB.kit }), "server_save_kept"],
  ["the server's save on another plot", (r) => (r.home_again.session.server_save.ship_place.plot = CABINS[2].id), "server_save_kept"],
  ["the server's save never reported", (r) => (r.home_again.session.server_save = null), "server_save_kept"],
  // 10: back to the server.
  ["the welcome moved the cabin on the way back", (r) => (r.server_again.last_welcome = "move"), "server_home_remembered"],
  ["back on the server, a fresh save again", (r) => (r.server_again.session.backpack = { ...CAB.kit }), "server_home_remembered"],
  ["never back in the shared world", (r) => (r.server_again.joined = false), "server_home_remembered"],
  ["back on another plot", (r) => Object.assign(r.server_again, { home_plot: plotJson(CABINS[2]) }), "server_home_remembered"],
  ["back as your own home", (r) => (r.server_again.session.slot = "offline"), "server_home_remembered"],
  // 11: over the run.
  ["the game corrected once", (r) => (r.corrections_game = 1), "no_correction"],
  ["the walker corrected", (r) => (r.walker_corrections = ["[TestBotCabin] second-player: corrected to (70.00, 1.70, 90.00) (too_fast, correction 1)"]), "no_correction"],
  ["a correction the relay sent", (r, e) => (e.relayLog += `\nGame: corrected ${GAME_KEY.slice(0, 16)}.. (correction 1, too_fast): [70.0,1.7,90.0]`), "no_correction"],
  ["corrections never recorded", (r) => delete r.corrections_game, "no_correction"],
  ["a PANIC", (r) => (r.panics = 1), "panics"],
  ["panics never counted", (r) => delete r.panics, "panics"],
  ["a built-in data copy served", (r) => (r.builtin_data = ["[built-in data copy] homes/starts.ron"]), "no_builtin_data"],
];

test("each broken run FAILS its own check and no other", () => {
  for (const [what, fn, id] of CASES) {
    const r = doctored(fn);
    const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
    const ids = Array.isArray(id) ? id : [id];
    assert.deepEqual(failed, ids, `${what} should fail ${ids.join(" and ")} (only); failed: ${failed.join(", ") || "nothing"}`);
  }
  // Every check is shown failing ALONE by some case above (a check no broken run fails on its own
  // may be one that cannot fail).
  const alone = new Set(CASES.filter(([, , id]) => !Array.isArray(id) || id.length === 1).map(([, , id]) => (Array.isArray(id) ? id[0] : id)));
  assert.deepEqual(ALL_IDS.filter((id) => !alone.has(id)), [], "every check has a broken run that fails it and nothing else");
  assert.equal(SJ.judgeStarts(null, {}).pass, false, "a run nobody recorded fails");
});

test("nothing recorded fails every check", () => {
  const r = SJ.judgeStarts({}, {});
  const passed = r.checks.filter((c) => c.ok).map((c) => c.id);
  assert.deepEqual(passed, [], `with nothing recorded nothing may pass; passed: ${passed.join(", ")}`);
  assert.deepEqual(r.checks.map((c) => c.id), ALL_IDS, "every check is still reported");
});

test("a run whose data block is missing fails every check that judges against it", () => {
  const r = doctored((run) => delete run.data);
  const passed = r.checks.filter((c) => c.ok).map((c) => c.id);
  assert.deepEqual(passed, ["look_crossed", "no_correction", "panics", "no_builtin_data"], "only the checks that need none of the tree's data may pass");
});

// ── The contract: the game's words, the probe's field names, the relay's line ──────────────────

test("the counter's words are the game's own", () => {
  const read = (p) => fs.readFileSync(path.join(REPO, ...p.split("/")), "utf8");
  const mess = read("src/engine/mess_hall.rs");
  const fleet = read("src/engine/fleet.rs");
  const meals = read("src/systems/ship_meals.rs");
  assert.ok(mess.includes(`"${SJ.SENTENCES.asked}"`), "mess_hall.rs no longer asks in the words the judge reads");
  assert.ok(mess.includes(`"${SJ.SENTENCES.eaten_offline}"`), "mess_hall.rs no longer serves the ship's stores in the words the judge reads");
  assert.ok(fleet.includes(`"${SJ.SENTENCES.eaten}"`), "fleet.rs no longer says a server's meal was eaten in the words the judge reads");
  assert.ok(fleet.includes('"Your next meal is in {:.0} game minutes."'), "fleet.rs no longer says not yet in the words the judge reads");
  assert.ok(SJ.SENTENCES.not_yet_re.test("Your next meal is in 480 game minutes."));
  assert.ok(meals.includes(`"Filled {} ({}) from the ship's water."`), "ship_meals.rs no longer says bottles were filled in the words the judge reads");
  assert.ok(SJ.SENTENCES.filled_re.test("Filled 2 x Water bottle (1 L) from the ship's water."));
});

test("the relay's home-chosen line is read as the relay writes it", () => {
  const src = fs.readFileSync(path.join(REPO, "src", "relay", "handlers", "home_plots.rs"), "utf8");
  assert.ok(src.includes('"Game: home chosen ({})"'), "home_plots.rs no longer logs a choice as the judge reads it");
  assert.ok(src.includes('"Game: home chosen (refused/{})"'), "home_plots.rs no longer logs a refusal as the judge reads it");
  assert.equal("x INFO r: Game: home chosen (cabin)".match(SJ.RELAY_HOME_CHOSEN_RE)[1], "cabin");
});

// The probe's field names live in one place in the game (src/engine/ipc.rs home_session_core and
// save_file_json); every one the judge reads must be written there by that name.
test("every home_session field the judge reads is one the probe writes", () => {
  const ipc = fs.readFileSync(path.join(REPO, "src", "engine", "ipc.rs"), "utf8");
  const from = ipc.indexOf("fn save_file_json(");
  const to = ipc.indexOf("// ── Door points", from);
  assert.ok(from > 0 && to > from, "ipc.rs has save_file_json and home_session_core before the door points");
  const body = ipc.slice(from, to);
  for (const f of ["slot", "start", "household", "ship_place", "home", "kind", "plot", "away", "box", "backpack", "skills", "credits", "quests", "active", "completed", "id", "step", "look", "name", "appearance", "outfit", "vitals", "satiation", "counter", "fleet_meals", "direction", "quantity", "answer", "server_save", "exists", "last_verb"]) {
    assert.ok(body.includes(`"${f}"`), `the probe no longer writes "${f}" (ipc.rs home_session_core / save_file_json)`);
  }
  const verbs = ipc.slice(ipc.indexOf("fn note_home_verb("), ipc.indexOf("fn note_home_verb(") + 600);
  for (const f of ["verb", "value", "ok", "said"]) assert.ok(verbs.includes(`"${f}"`), `last_verb no longer carries "${f}"`);
});

test("the walker's lines are read as it writes them", () => {
  const g = `second-player: home: ${C1.id} ${CAB.plot_kind}; free ${FREE}`.match(SP.HOME_RE);
  assert.deepEqual(g.slice(1, 3), [C1.id, CAB.plot_kind]);
  assert.equal("second-player: meal taken: ration_basic_0".match(SP.MEAL_TAKEN_RE)[1], "ration_basic_0");
});

// ── The rig ─────────────────────────────────────────────────────────────────────────────────────

test("the rig's --starts run boots its own home, keeps progress, and sends each home verb alone", () => {
  const rig = fs.readFileSync(path.join(REPO, "scripts", "verify-copresence.js"), "utf8");
  const from = rig.indexOf("async function runStartsOnce(");
  assert.ok(from >= 0, "verify-copresence.js has runStartsOnce");
  const body = rig.slice(from, rig.indexOf("\nasync function mainStarts(", from));
  // Paired with its own home before it boots, so it boots there alone.
  assert.match(rig, /cfg\.last_world = "home:";/, "the sandbox is paired with its own home");
  assert.match(body, /pairSandboxWithHome\(\);[\s\S]*GL\.spawnGame\(/, "the pairing is written before the game starts");
  // Keeping progress, or going home would build the default home again and nothing could be judged
  // untouched.
  assert.match(body, /gameplay: \{ server_url: relay\.httpUrl, fresh_world_each_launch: false \}/);
  // choose_start and pairing in separate requests (the probe keeps only the last verb).
  assert.match(body, /showcase\(\{ choose_start: data\.chosen \}\)/);
  assert.match(body, /showcase\(\{ pairing: "server" \}\)/);
  assert.ok(!/choose_start: [^}]*pairing/.test(body), "a choice and a pairing are never sent in one request");
  // The run forgets earlier runs' server saves and its own home's save first.
  assert.match(body, /forgetSandboxPlots\(\);\s*forgetSandboxOwnHome\(\);/);
  // Never the operator's focus opt-in.
  assert.ok(!/HUMANITY_TAKE_FOCUS/.test(rig), "the rig never names HUMANITY_TAKE_FOCUS");
});

// The rig judges a saved run from its folder, as `just verify-starts --dry-verdict` does: the run
// above written out (with the relay's log), then one record doctored. Red first, 2026-10-12, before
// verify-copresence.js knew a --starts manifest (see the top).
test("--dry-verdict judges a saved --starts run from its folder", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hum_starts-judge-"));
  try {
    const { run, evidence } = goodRun();
    fs.writeFileSync(path.join(dir, "relay.log"), evidence.relayLog);
    const judge = (m) => {
      fs.writeFileSync(path.join(dir, "manifest.json"), JSON.stringify(m));
      const out = spawnSync(process.execPath, [path.join(REPO, "scripts", "verify-copresence.js"), "--dry-verdict", path.join(dir, "manifest.json")], { encoding: "utf8", timeout: 60000 });
      return { status: out.status, text: `${out.stdout}\n${out.stderr}` };
    };
    const good = judge(run);
    assert.equal(good.status, 0, good.text);
    assert.match(good.text, new RegExp(`DRY VERDICT \\(nothing was booted\\): PASS  ${ALL_IDS.length}/${ALL_IDS.length}`));
    const bad = judge({ ...run, server: { ...run.server, session: { ...run.server.session, backpack: { ...MARK } } } });
    assert.equal(bad.status, 2, bad.text);
    assert.match(bad.text, /FAIL  fresh_on_server/);
    assert.match(bad.text, /failed: fresh_on_server$/m);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
