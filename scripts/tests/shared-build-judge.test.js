// The shared-build judge (scripts/lib/shared-build-judge.js): what `just verify-shared-build`
// (verify-copresence.js --build, ship homes increment 5, 2026-10-05) passes and fails.
//
// Run: node --test scripts/tests/shared-build-judge.test.js   (in `just rig-tests`)
//
// Pure node: nothing is booted. A made-up run that went right passes every check; then one broken
// thing at a time, each failing its own check and no other (the plan's proof for the judge,
// docs/design/ship-homes-increment-5-plan.md section 5: "Every judge check is also shown failing
// alone on a doctored copy of a green manifest"). Last, the same run written to a folder is judged
// by the rig itself, `node scripts/verify-copresence.js --dry-verdict`, as a saved run would be.
//
// Red first, 2026-10-05, before the judge existed:
//   Error: Cannot find module '../lib/shared-build-judge.js'
// and the --dry-verdict test, before verify-copresence.js knew a --build manifest (it judged it
// as the default rig's): "DRY VERDICT (nothing was booted): FAIL  4/12 passed; failed:
// camera_parked, walker_ran, approach_clear, view_clear, screenshots, on_screen, figure_visible,
// recorded".

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const J = require("../lib/shared-build-judge.js");
const { quarterTurn } = require("../second-player.js");
const png = require("../lib/png.js");

const REPO = path.join(__dirname, "..", "..");

const T0 = 1759680000000; // when the builder heard its foundation kept, ms since 1970
const T1 = T0 + 30000; // the game's own place
const T2 = T0 + 60000; // the game's take-down
const T3 = T0 + 120000; // the rank holder's wall in the Commons

const FRAMES = [
  { id: "zone:commons", kind: "zone", min: [65, 0, 20], max: [99, 8, 75] },
  { id: "zone:street-1", kind: "zone", min: [65, 0, 85], max: [71, 4, 1185] },
  { id: "plot:p1", kind: "plot", min: [0, 0, 0], max: [55, 3, 89] },
  { id: "plot:p2", kind: "plot", min: [0, 0, 99], max: [55, 3, 188] },
  { id: "plot:p3", kind: "plot", min: [0, 0, 198], max: [55, 3, 287] },
];
const BLUEPRINTS = {
  wood_foundation: { name: "Wood Foundation", size: [4, 0.2, 4], build_time: 5, materials: [["wood_plank_0", 8]] },
  wood_wall: { name: "Wood Wall", size: [4, 3, 0.2], build_time: 4, materials: [["wood_plank_0", 6]] },
};
const at = (frame, local) => {
  const f = FRAMES.find((x) => x.id === frame);
  return [0, 1, 2].map((k) => f.min[k] + local[k]);
};

/** A piece the game has, as the recorder lists it, at frame time `e` (ms). */
function row(p, e) {
  const built = e >= p.placed_at * 1000 + BLUEPRINTS[p.cmd.blueprint].build_time * 1000;
  return {
    piece_id: p.piece_id,
    frame: p.cmd.frame,
    blueprint_id: p.cmd.blueprint,
    pos: at(p.cmd.frame, p.cmd.local),
    rot: quarterTurn(p.cmd.turns),
    scale: BLUEPRINTS[p.cmd.blueprint].size.slice(),
    mine: false,
    built,
    progress: Math.min(BLUEPRINTS[p.cmd.blueprint].build_time, e / 1000 - p.placed_at),
    rect: p.rect || null,
  };
}

/** A recording at 15 frames a second from `from` to `to` (ms), each piece listed from `seenAt`. */
function recording(from, to, pieces) {
  const frames = [];
  for (let e = from, k = 0; e <= to; e += 1000 / 15, k++) {
    frames.push({ t: k / 15, dt: 1 / 15, epoch_ms: e, joined: true, cam: [76, 1.7, 64, Math.PI, -0.05], players: [], crew: [], shared: pieces.filter((p) => e >= p.seenAt).map((p) => row(p, e)) });
  }
  return frames;
}

/** Pictures 200 x 120: the Commons' grey, and the same with the wall's rect in wood. */
function picture(withWall) {
  const img = { width: 200, height: 120, rgba: Buffer.alloc(200 * 120 * 4) };
  for (let y = 0; y < 120; y++) {
    for (let x = 0; x < 200; x++) {
      const i = (y * 200 + x) * 4;
      const wood = withWall && x >= 100 && x < 150 && y >= 60 && y < 100;
      img.rgba[i] = wood ? 150 : 128;
      img.rgba[i + 1] = wood ? 125 : 128;
      img.rgba[i + 2] = wood ? 92 : 130;
      img.rgba[i + 3] = 255;
    }
  }
  return img;
}

/** A --build run that went right, and its evidence. Fresh every call, so a case may break it. */
function goodRun() {
  const foundation = { cmd: { blueprint: "wood_foundation", frame: "plot:p1", local: [48, 0, 36], turns: 0 }, req: 1, piece_id: 1, line_epoch_ms: T0, placed_at: (T0 - 50) / 1000, refusal: null, seenAt: T0 + 100 };
  const wallP1 = { cmd: { blueprint: "wood_wall", frame: "plot:p1", local: [46, 0.2, 36], turns: 1 }, req: 2, piece_id: 2, line_epoch_ms: T0 + 600, placed_at: (T0 + 550) / 1000, refusal: null, seenAt: T0 + 700 };
  const rankWall = { cmd: { blueprint: "wood_wall", frame: "zone:commons", local: [11, 0, 50], turns: 0 }, req: 1, piece_id: 4, line_epoch_ms: T3, placed_at: (T3 - 50) / 1000, refusal: null, seenAt: T3 + 150, rect: [100, 60, 150, 100] };
  const strip = ({ seenAt, rect, ...b }) => b;
  const pack = (n) => ({ wood_plank_0: n });
  const run = {
    kind: "verify-copresence-build",
    steps_ok: {
      relay: { ok: true, detail: "http://127.0.0.1:50000 answered /health" },
      joined: { ok: true, detail: "joined and welcomed on p2" },
      walkers: { ok: true, detail: "TestBotBuilder on p1, TestBotShipwright on p3" },
    },
    frames: FRAMES,
    blueprints: BLUEPRINTS,
    walkers: {
      builder: { name: "TestBotBuilder", id: 1, plot: "p1", ranks: { can_edit_ship: false, take_down_any: false } },
      rank: { name: "TestBotShipwright", id: 3, plot: "p3", ranks: { can_edit_ship: true, take_down_any: true } },
    },
    game: { plot: "p2", ranks: { can_edit_ship: false, take_down_any: false } },
    theirs: { samples: "theirs_samples.json", save_before: 3, save_after: 3, builds: [strip(foundation), strip(wallP1)] },
    ours: {
      place: { blueprint: "wood_foundation", frame: "plot:p2", local: [48, 0, 36], turns: 0, at: [48, 0, 135], epoch_ms: T1 },
      pack_before: pack(99),
      pack_after: pack(91),
      pack_later: pack(91),
      piece: { piece_id: 3, frame: "plot:p2", mine: true, pos: [48, 0, 135], rot: quarterTurn(0), built: false },
      seen_by_walker: { epoch_ms: T1 + 400, piece_id: 3, frame: "plot:p2", local: [48, 0, 36], turn: 0 },
      save: 3,
    },
    named: {
      theirs_in_ours: {
        cmd: { blueprint: "wood_foundation", frame: "plot:p2", local: [20, 0, 20], turns: 0 },
        refusal: { action: "build", reason: "not_allowed", why: "not_your_plot", line: "build refused: not_allowed (not_your_plot) (req 3): Wood Foundation not built: this is someone else's plot." },
        relay_lines: ["2026-10-05T12:00:31Z  INFO humanity::relay: Game: build refused (not_allowed/not_your_plot) on plot:p2"],
        drawn_others: [],
      },
      ours_in_theirs: {
        place: { blueprint: "wood_foundation", frame: "plot:p1", local: [20, 0, 20], turns: 0, at: [20, 0, 20], epoch_ms: T1 + 20000 },
        notices: [],
        hint: "Placing Wood Foundation: this is someone else's plot; you build only on your own plot, or where its holder has given you a household permit   [Esc] done",
        pack_before: pack(91),
        pack_after: pack(91),
        relay_lines: [],
        walker_lines: [],
        pending: { builds: 0, unbuilds: 0, intents: 0 },
      },
    },
    take_down: {
      theirs: {
        piece_id: 3,
        refusal: { action: "unbuild", reason: "not_allowed", why: "not_your_plot", line: "unbuild refused: not_allowed (not_your_plot) (req 4): Wood Foundation not taken down." },
        relay_lines: ["Game: take-down refused (not_allowed/not_your_plot)"],
        still_drawn: true,
      },
      ours: {
        piece_id: 3,
        blueprint: "wood_foundation",
        epoch_ms: T2,
        seen_by_walker: { epoch_ms: T2 + 300, piece_id: 3 },
        gone: true,
        pack_before: pack(91),
        pack_after: pack(99),
        pack_later: pack(99),
        store_before: pack(5),
        store_after: pack(5),
        store_later: pack(5),
        relay_lines: ["Game: took down piece 3 on plot:p2"],
      },
    },
    zone: {
      parked: { ok: true, detail: "at (76.00, 1.70, 64.00) yaw 3.142, holding still, in the shared world" },
      game_place: {
        place: { blueprint: "wood_foundation", frame: "zone:commons", local: [23, 0, 33], turns: 0, at: [88, 0, 53], epoch_ms: T3 - 20000 },
        notices: ["Placing Wood Foundation: the ship's shared spaces are built by people this server has given that rank"],
        hint: null,
        pack_before: pack(99),
        pack_after: pack(99),
        relay_lines: [],
        walker_lines: [],
        pending: { builds: 0, unbuilds: 0, intents: 0 },
      },
      walker_refused: {
        cmd: { blueprint: "wood_foundation", frame: "zone:commons", local: [23, 0, 33], turns: 0 },
        refusal: { action: "build", reason: "not_allowed", why: "ship_rank", line: "build refused: not_allowed (ship_rank) (req 5): ..." },
        relay_lines: ["Game: build refused (not_allowed/ship_rank) on zone:commons"],
      },
      wall: { ...strip(rankWall), samples: "zone_samples.json", before: "zone_before.png", after: "zone_after.png", after_epoch_ms: T3 + 5500 },
    },
    dev: {
      joined: { ship_editing: false, editor_open: true, editor_zone: "home" },
      solo: { joined: false, ship_editing: true, pieces: [] },
      back: { joined: true, pieces: [{ piece_id: 1, built: true }, { piece_id: 2, built: true }, { piece_id: 4, built: true }] },
      expect_ids: [1, 2, 4],
    },
    corrections_game: 0,
    walker_corrections: [],
    walker_keys: ["aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb"],
    walks: [{ label: "zone", to: [76, 1.7, 64], at: [76, 1.7, 64], ok: true }],
    turns: [{ pose: "76,1.7,64,3.14159265,-0.05", at: [76, 1.7, 64], off: 0, ok: true }],
    panics: 0,
    builtin_data: [],
  };
  const evidence = {
    recordings: {
      "theirs_samples.json": recording(T0 - 1000, T0 + 9000, [foundation, wallP1]),
      "zone_samples.json": recording(T3 - 1000, T3 + 7000, [rankWall]),
    },
    pictures: { "zone_before.png": picture(false), "zone_after.png": picture(true) },
    relayLog: ["Game: built piece 1 wood_foundation on plot:p1", "Game: built piece 2 wood_wall on plot:p1", "Game: built piece 3 wood_foundation on plot:p2", "Game: built piece 4 wood_wall on zone:commons"].join("\n"),
  };
  return { run, evidence };
}

const explain = (r) => r.checks.filter((c) => !c.ok).map((c) => `${c.id}: ${c.detail}`).join("\n");

const ALL_IDS = [
  "relay_up",
  "game_in_world",
  "walkers_in_world",
  "three_plots_held",
  "ranks_from_the_server",
  "theirs_built",
  "theirs_drawn_in_time",
  "theirs_where_built",
  "theirs_scaffold_finishes",
  "shared_pieces_not_saved",
  "ours_mine",
  "ours_spent_once",
  "ours_reaches_them",
  "refused_their_build_in_our_plot",
  "refused_our_build_in_their_plot",
  "their_take_down_refused",
  "our_take_down_reaches_them",
  "our_take_down_refunds_once",
  "zone_camera_parked",
  "zone_refused_without_rank",
  "zone_walker_refused_without_rank",
  "zone_rank_wall_drawn",
  "zone_wall_in_view",
  "zone_wall_visible",
  "dev_off_while_joined",
  "dev_on_offline",
  "solo_back_same_ids",
  "solo_back_finished",
  "moves_game_honest_never_corrected",
  "moves_relay_sent_what_the_game_took",
  "moves_walkers_never_corrected",
  "walks_all_arrived",
  "walks_turns_in_place",
  "no_panics",
  "no_builtin_data",
];

test("a --build run that went right passes, every check its own", () => {
  const { run, evidence } = goodRun();
  const r = J.judgeSharedBuild(run, evidence);
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), ALL_IDS);
});

/** The run with `fn` applied to it (and its evidence), judged. */
function doctored(fn) {
  const g = goodRun();
  fn(g.run, g.evidence);
  return J.judgeSharedBuild(g.run, g.evidence);
}
/** Change every recorded row of piece `id` in a recording. */
const eachRow = (frames, id, fn) => frames.forEach((fr, i) => (fr.shared = fr.shared.map((r) => (r.piece_id === id ? fn(r, fr, i) : r))));

// One broken thing at a time, each failing its own check and no other (or, where one fault must
// show in several, exactly those). Red first, 2026-10-05: the module did not exist (see the top).
const CASES = [
  // 1-2: the rig's own steps.
  ["the relay never came up", (r) => (r.steps_ok.relay = { ok: false, detail: "never answered /health" }), "relay_up"],
  ["the game never joined", (r) => (r.steps_ok.joined = { ok: false, detail: "game_joined=false" }), "game_in_world"],
  ["the builders never joined", (r) => delete r.steps_ok.walkers, "walkers_in_world"],
  // Who holds what, and what the server says each may do.
  ["the rank holder told it may not edit the ship (ADMIN_KEYS never reached the relay)", (r) => (r.walkers.rank.ranks = { can_edit_ship: false, take_down_any: false }), "ranks_from_the_server"],
  ["a plain builder told it may edit the ship", (r) => (r.walkers.builder.ranks.can_edit_ship = true), "ranks_from_the_server"],
  ["the game's ranks never recorded (a probe from before increment 5)", (r) => (r.game.ranks = null), "ranks_from_the_server"],
  ["two players handed one plot", (r) => (r.game.plot = "p1"), "three_plots_held"],
  ["the game a guest", (r) => (r.game.plot = null), "three_plots_held"],
  // 3: theirs reaches us.
  ["the builder's wall refused by the relay", (r) => Object.assign(r.theirs.builds[1], { piece_id: null, refusal: { action: "build", reason: "occupied" } }), ["theirs_built", "theirs_drawn_in_time", "theirs_where_built", "theirs_scaffold_finishes"]],
  ["the builder's builds never recorded", (r) => (r.theirs.builds = undefined), ["theirs_built", "theirs_drawn_in_time", "theirs_where_built", "theirs_scaffold_finishes"]],
  // The plan's red: with the client's game_built handling removed, steps 3 and 7 fail.
  ["the game never draws the builder's foundation (its game_built handling removed)", (r, e) => e.recordings["theirs_samples.json"].forEach((fr) => (fr.shared = fr.shared.filter((x) => x.piece_id !== 1))), ["theirs_drawn_in_time", "theirs_where_built", "theirs_scaffold_finishes"]],
  ["the wall drawn 2.5 s after the builder heard it kept", (r, e) => e.recordings["theirs_samples.json"].forEach((fr) => (fr.shared = fr.shared.filter((x) => x.piece_id !== 2 || fr.epoch_ms >= T0 + 600 + 2500))), "theirs_drawn_in_time"],
  ["the foundation drawn 5 cm off where it was built", (r, e) => eachRow(e.recordings["theirs_samples.json"], 1, (x) => ({ ...x, pos: [48, 0, 36.05] })), "theirs_where_built"],
  ["the wall drawn turned the other way (3 quarter turns for 1)", (r, e) => eachRow(e.recordings["theirs_samples.json"], 2, (x) => ({ ...x, rot: quarterTurn(3) })), "theirs_where_built"],
  ["the wall drawn the size of a foundation", (r, e) => eachRow(e.recordings["theirs_samples.json"], 2, (x) => ({ ...x, scale: [4, 0.2, 4] })), "theirs_where_built"],
  ["the foundation drawn in the wrong frame (its plot taken for the game's)", (r, e) => eachRow(e.recordings["theirs_samples.json"], 1, (x) => ({ ...x, frame: "plot:p2" })), "theirs_where_built"],
  ["the foundation's scaffold finishing 1.5 s late", (r, e) => eachRow(e.recordings["theirs_samples.json"], 1, (x, fr) => ({ ...x, built: fr.epoch_ms >= T0 - 50 + 6500 })), "theirs_scaffold_finishes"],
  ["the foundation drawn finished at once (the build time ignored)", (r, e) => eachRow(e.recordings["theirs_samples.json"], 1, (x) => ({ ...x, built: true })), "theirs_scaffold_finishes"],
  ["the wall still a scaffold when the recording ended", (r, e) => eachRow(e.recordings["theirs_samples.json"], 2, (x) => ({ ...x, built: false })), "theirs_scaffold_finishes"],
  ["the builder's pieces in the game's save", (r) => (r.theirs.save_after = 5), "shared_pieces_not_saved"],
  ["the game's own shared piece in its save", (r) => (r.ours.save = 4), "shared_pieces_not_saved"],
  ["the save never counted", (r) => (r.theirs.save_before = undefined), "shared_pieces_not_saved"],
  // 4: ours reaches them.
  ["the game's piece not marked its own", (r) => (r.ours.piece.mine = false), "ours_mine"],
  ["the game's piece never kept as a shared one", (r) => (r.ours.piece = null), "ours_mine"],
  ["the game's piece placed 1 m off where it was aimed", (r) => (r.ours.piece.pos = [49, 0, 135]), "ours_mine"],
  ["the planks spent twice", (r) => (r.ours.pack_later = { wood_plank_0: 83 }), "ours_spent_once"],
  ["nothing spent", (r) => (r.ours.pack_after = { wood_plank_0: 99 }), "ours_spent_once"],
  ["the pack never recorded", (r) => (r.ours.pack_before = undefined), "ours_spent_once"],
  ["the builder never saw the game's piece", (r) => (r.ours.seen_by_walker = null), "ours_reaches_them"],
  ["the builder saw it 3 s after the place", (r) => (r.ours.seen_by_walker.epoch_ms = T1 + 3000), "ours_reaches_them"],
  ["the builder saw it somewhere else", (r) => (r.ours.seen_by_walker.local = [48, 0, 37]), "ours_reaches_them"],
  ["the builder saw another piece than the game holds", (r) => (r.ours.seen_by_walker.piece_id = 9), "ours_reaches_them"],
  // 5: the named case. The plan's red: with may_build returning Ok, exactly this check fails.
  [
    "the relay KEPT the builder's build inside the game's plot (may_build returning Ok)",
    (r) => Object.assign(r.named.theirs_in_ours, { refusal: null, relay_lines: ["Game: built piece 9 wood_foundation on plot:p2"], drawn_others: [9] }),
    "refused_their_build_in_our_plot",
  ],
  ["the builder's build refused for another reason", (r) => (r.named.theirs_in_ours.refusal = { action: "build", reason: "occupied", why: null }), "refused_their_build_in_our_plot"],
  ["the relay never logged the refusal", (r) => (r.named.theirs_in_ours.relay_lines = []), "refused_their_build_in_our_plot"],
  ["the game drew the refused piece anyway", (r) => (r.named.theirs_in_ours.drawn_others = [9]), "refused_their_build_in_our_plot"],
  ["no plot sentence on the game's screen", (r) => (r.named.ours_in_theirs.hint = null), "refused_our_build_in_their_plot"],
  ["the game's screen never recorded", (r) => Object.assign(r.named.ours_in_theirs, { hint: undefined, notices: undefined }), "refused_our_build_in_their_plot"],
  ["planks spent on a build the game refused itself", (r) => (r.named.ours_in_theirs.pack_after = { wood_plank_0: 83 }), "refused_our_build_in_their_plot"],
  ["the game sent it to the relay anyway", (r) => (r.named.ours_in_theirs.relay_lines = ["Game: build refused (not_allowed/not_your_plot) on plot:p1"]), "refused_our_build_in_their_plot"],
  ["the builder saw a build in that window", (r) => (r.named.ours_in_theirs.walker_lines = ["second-player: saw built: piece 9 wood_foundation on plot:p1 at (20.000, 0.000, 20.000) turn 0 (seq 3)"]), "refused_our_build_in_their_plot"],
  ["a build left waiting to be sent", (r) => (r.named.ours_in_theirs.pending = { builds: 0, unbuilds: 0, intents: 1 }), "refused_our_build_in_their_plot"],
  ["what the game has waiting never recorded", (r) => (r.named.ours_in_theirs.pending = undefined), "refused_our_build_in_their_plot"],
  // 6: who takes down.
  ["the builder took the game's piece down", (r) => Object.assign(r.take_down.theirs, { refusal: null, relay_lines: ["Game: took down piece 3 on plot:p2"], still_drawn: false }), "their_take_down_refused"],
  ["the builder's take-down refused for another reason", (r) => (r.take_down.theirs.refusal = { action: "unbuild", reason: "no_such_piece", why: null }), "their_take_down_refused"],
  ["the game stopped drawing a piece nobody took down", (r) => (r.take_down.theirs.still_drawn = false), "their_take_down_refused"],
  ["the relay never logged the take-down refusal", (r) => (r.take_down.theirs.relay_lines = []), "their_take_down_refused"],
  ["the builder never saw the game's take-down", (r) => (r.take_down.ours.seen_by_walker = null), "our_take_down_reaches_them"],
  ["the builder saw it come down 2 s late", (r) => (r.take_down.ours.seen_by_walker.epoch_ms = T2 + 2000), "our_take_down_reaches_them"],
  ["the game still has the piece it took down", (r) => (r.take_down.ours.gone = false), "our_take_down_reaches_them"],
  ["the planks given back twice", (r) => (r.take_down.ours.pack_later = { wood_plank_0: 107 }), "our_take_down_refunds_once"],
  ["nothing given back", (r) => (r.take_down.ours.pack_after = { wood_plank_0: 91 }), "our_take_down_refunds_once"],
  ["the planks given back twice, into the backpack and into home storage", (r) => Object.assign(r.take_down.ours, { store_after: { wood_plank_0: 13 }, store_later: { wood_plank_0: 13 } }), "our_take_down_refunds_once"],
  ["home storage never counted (a game whose probe reports only the backpack)", (r) => (r.take_down.ours.store_before = undefined), "our_take_down_refunds_once"],
  // 7: the ship's shared spaces.
  ["the game never stood in the Commons", (r) => (r.zone.parked = { ok: false, detail: "stopped short" }), "zone_camera_parked"],
  ["no rank sentence on the game's screen", (r) => (r.zone.game_place.notices = ["Placing Wood Foundation: this is someone else's plot; you build only on your own plot"]), "zone_refused_without_rank"],
  ["the game sent a build in the Commons it may not make", (r) => (r.zone.game_place.relay_lines = ["Game: build refused (not_allowed/ship_rank) on zone:commons"]), "zone_refused_without_rank"],
  ["the builder's build in the Commons refused as someone else's plot", (r) => (r.zone.walker_refused.refusal.why = "not_your_plot"), "zone_walker_refused_without_rank"],
  ["the builder's build in the Commons KEPT", (r) => Object.assign(r.zone.walker_refused, { refusal: null, relay_lines: ["Game: built piece 8 wood_foundation on zone:commons"] }), "zone_walker_refused_without_rank"],
  ["the relay never logged the rank refusal", (r) => (r.zone.walker_refused.relay_lines = []), "zone_walker_refused_without_rank"],
  // The plan's red again: step 7's wall never drawn takes its view and picture with it.
  ["the rank holder's wall never in the game's world", (r, e) => e.recordings["zone_samples.json"].forEach((fr) => (fr.shared = [])), ["zone_rank_wall_drawn", "zone_wall_in_view", "zone_wall_visible"]],
  ["the wall drawn 2 s after the rank holder heard it kept", (r, e) => e.recordings["zone_samples.json"].forEach((fr) => (fr.shared = fr.shared.filter(() => fr.epoch_ms >= T3 + 2000))), "zone_rank_wall_drawn"],
  ["the wall drawn a metre to the side", (r, e) => eachRow(e.recordings["zone_samples.json"], 4, (x) => ({ ...x, pos: [77, 0, 70] })), "zone_rank_wall_drawn"],
  ["the wall's rect running off the right of the view", (r, e) => eachRow(e.recordings["zone_samples.json"], 4, (x) => ({ ...x, rect: [100, 60, 210, 100] })), "zone_wall_in_view"],
  ["the wall behind the camera (no rect)", (r, e) => eachRow(e.recordings["zone_samples.json"], 4, (x) => ({ ...x, rect: null })), ["zone_wall_in_view", "zone_wall_visible"]],
  ["a window drawn over the wall: no wood in the picture", (r, e) => (e.pictures["zone_after.png"] = picture(false)), "zone_wall_visible"],
  ["oak behind the wall's place already: no gain", (r, e) => (e.pictures["zone_before.png"] = picture(true)), "zone_wall_visible"],
  ["the pictures never taken", (r, e) => delete e.pictures["zone_before.png"], "zone_wall_visible"],
  // 8: Dev keeps everything only offline.
  ["ship editing allowed while joined", (r) => (r.dev.joined.ship_editing = true), "dev_off_while_joined"],
  ["the build editor never opened", (r) => (r.dev.joined.editor_open = false), "dev_off_while_joined"],
  ["the build editor open on the Commons", (r) => (r.dev.joined.editor_zone = "commons"), "dev_off_while_joined"],
  ["no ship editing offline", (r) => (r.dev.solo.ship_editing = false), "dev_on_offline"],
  ["shared pieces left in the world offline", (r) => (r.dev.solo.pieces = [{ piece_id: 1 }]), "dev_on_offline"],
  ["a piece back under a new id", (r) => (r.dev.back.pieces[2].piece_id = 5), "solo_back_same_ids"],
  ["a piece missing after stepping back in", (r) => r.dev.back.pieces.pop(), ["solo_back_same_ids"]],
  ["the game's taken-down piece back again", (r) => r.dev.back.pieces.push({ piece_id: 3, built: true }), "solo_back_same_ids"],
  ["pieces back as scaffolds growing again", (r) => (r.dev.back.pieces[0].built = false), "solo_back_finished"],
  // 9: over the run.
  ["the game corrected once", (r, e) => ((r.corrections_game = 1), (e.relayLog += "\nGame: corrected 0123456789abcdef.. (correction 1, too_fast)")), "moves_game_honest_never_corrected"],
  ["a correction the game never took", (r, e) => (e.relayLog += "\nGame: corrected 0123456789abcdef.. (correction 1, too_fast)"), "moves_relay_sent_what_the_game_took"],
  ["a walker corrected", (r) => (r.walker_corrections = ["[TestBotBuilder] second-player: corrected to (27.50, 1.70, 44.50) (too_fast, correction 1)"]), "moves_walkers_never_corrected"],
  ["a walk that never arrived", (r) => (r.walks[0].ok = false), "walks_all_arrived"],
  ["a turn sent from somewhere else (a teleport)", (r) => Object.assign(r.turns[0], { at: [70, 1.7, 64], off: 6, ok: false }), "walks_turns_in_place"],
  ["a PANIC", (r) => (r.panics = 1), "no_panics"],
  ["a built-in data copy served", (r) => (r.builtin_data = ["[built-in data copy] blueprints/basic.ron"]), "no_builtin_data"],
];

test("each broken run FAILS its own check and no other", () => {
  for (const [what, fn, id] of CASES) {
    const r = doctored(fn);
    const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
    const ids = Array.isArray(id) ? id : [id];
    assert.deepEqual(failed, ids, `${what} should fail ${ids.join(" and ")} (only); failed: ${failed.join(", ") || "nothing"}`);
  }
  // Every check is shown failing ALONE by some case above (a check no broken run fails on its own
  // may be one that cannot fail), except theirs_built: the builder's builds being kept is what
  // the rest of step 3 judges, so a run where one was refused fails those too (shown above).
  const alone = new Set(CASES.filter(([, , id]) => !Array.isArray(id) || id.length === 1).map(([, , id]) => (Array.isArray(id) ? id[0] : id)));
  const takesOthersAlong = ["theirs_built"];
  assert.deepEqual(ALL_IDS.filter((id) => !alone.has(id) && !takesOthersAlong.includes(id)), [], "every check has a broken run that fails it and nothing else");
  assert.equal(J.judgeSharedBuild(null, {}).pass, false, "a run nobody recorded fails");
});

// WHAT COMES BACK IS COUNTED WHERE THE GAME PUTS IT. A take-down's materials come back through the
// "Take to backpack" channel, so what the backpack has no room for lands in home storage
// (src/engine/shared_build.rs `took_down`, as F gives back a piece of the player's own,
// build_place.rs `apply_take_down`). The rig fills the backpack far past its 65 L ("stock all
// materials": a stack of every recipe input), and a Wood Foundation's 8 planks are 65.4 L, more
// than even an empty backpack holds, so the planks of the game's own foundation land in home
// storage. The first --build run (2026-10-05) read the backpack alone: "wood_plank_0: 12 before, 12
// once it came down, 12 a few seconds later (the piece gives back 8): NOT given back exactly once".
// The judge counts the backpack and home storage together (the probe's `pack` and `storage`), and
// says where the planks landed.
//
// Seen red 2026-10-05 against the judge that read the backpack alone:
//   AssertionError [ERR_ASSERTION]: all into home storage (the rig's stocked backpack):
//   wood_plank_0: 12 before, 12 once it came down, 12 a few seconds later (the piece gives back
//   8): NOT given back exactly once
test("what comes back is counted in the backpack and home storage together", () => {
  const counts = (ns) => ns.map((n) => ({ wood_plank_0: n }));
  for (const [what, packs, stores, said] of [
    ["all into home storage (the rig's stocked backpack)", [12, 12, 12], [0, 8, 8], "12 + 0 before, 12 + 8 once it came down, 12 + 8 a few seconds later (the piece gives back 8: 0 into the backpack, 8 into home storage)"],
    ["7 into an empty backpack, 1 into home storage", [0, 7, 7], [3, 4, 4], "(the piece gives back 8: 7 into the backpack, 1 into home storage)"],
    ["all into a backpack with room", [91, 99, 99], [5, 5, 5], "(the piece gives back 8: 8 into the backpack, 0 into home storage)"],
  ]) {
    const r = doctored((run) => {
      const [pb, pa, pl] = counts(packs);
      const [sb, sa, sl] = counts(stores);
      Object.assign(run.take_down.ours, { pack_before: pb, pack_after: pa, pack_later: pl, store_before: sb, store_after: sa, store_later: sl });
    });
    const c = r.checks.find((x) => x.id === "our_take_down_refunds_once");
    assert.ok(c.ok, `${what}: ${c.detail}`);
    assert.ok(c.detail.includes(said), `${what}: the detail says where they landed: ${c.detail}`);
  }
});

// BUG-165 IN THE --build RIG TOO (2026-10-05): its walk from the game's door into the Commons faces
// each leg as a person walks it and turns to the meeting pose's facing only at the end
// (copresence-judge.js `routeFacings`, `walkYaw`), as --plots' meeting walk does. It had a walk loop
// of its own that asked every door for the facing the camera started with, so the camera turned
// back to it at each door. The rig boots the game, so this reads its source, as
// second-player.test.js reads the rig's use of CORRECTED_RE.
//
// Red first, 2026-10-05, on that loop:
//   AssertionError [ERR_ASSERTION]: the --build walk asks a door for the facing the camera started
//   with (yaw0): walkGame(p, yaw0, pitch0, "zone")
test("the --build walk into the Commons faces its legs and turns only at its end", () => {
  const rig = fs.readFileSync(path.join(REPO, "scripts", "verify-copresence.js"), "utf8");
  const from = rig.indexOf("async function runBuildOnce(");
  const to = rig.indexOf("\nasync function ", from + 1);
  assert.ok(from >= 0 && to > from, "verify-copresence.js has runBuildOnce");
  const build = rig.slice(from, to);
  const fixed = build.match(/walkGame\([^)]*\byaw0\b[^)]*\)/);
  assert.equal(fixed, null, `the --build walk asks a door for the facing the camera started with (yaw0): ${fixed && fixed[0]}`);
  assert.match(build, /routeFacings\(points\.length, yaw, pitch\)/, "its walkRoute takes each walk's facing from routeFacings");
  assert.match(build, /yaw === null \? walkYaw\(/, "and a door's facing is the way that leg walks");
  assert.match(build, /walkRoute\(\[door, \.\.\.route\.points\], myaw, mpitch, "zone"\)/, "the walk into the Commons is one route, ending at the meeting pose's facing");
});

test("nothing recorded fails every check that has evidence to read", () => {
  const r = J.judgeSharedBuild({}, {});
  const passed = r.checks.filter((c) => c.ok).map((c) => c.id);
  assert.deepEqual(passed, [], `with nothing recorded nothing may pass; passed: ${passed.join(", ")}`);
});

// The sentences the judge looks for on the game's screen are the game's own words: the one copy
// of each lives in src/systems/construction/shared.rs (`Why::NotYourPlot`, `Why::ShipRank`).
test("the sentences on screen are the contract's own words", () => {
  const shared = fs.readFileSync(path.join(REPO, "src", "systems", "construction", "shared.rs"), "utf8");
  assert.ok(shared.includes(J.PLOT_SENTENCE), `shared.rs no longer says "${J.PLOT_SENTENCE}"`);
  assert.ok(shared.includes(J.RANK_SENTENCE), `shared.rs no longer says "${J.RANK_SENTENCE}"`);
});

// The relay's log lines (plan section 3.3), in every way a refusal with no `why` may be written.
test("the relay's build lines are read in each form", () => {
  assert.deepEqual("x Game: built piece 12 wood_wall on zone:commons".match(J.RELAY_BUILT_RE).slice(1), ["12", "wood_wall", "zone:commons"]);
  for (const [line, why] of [
    ["Game: build refused (not_allowed/not_your_plot) on plot:p2", "not_your_plot"],
    ["Game: build refused (occupied) on plot:p2", undefined],
    ["Game: build refused (occupied/) on plot:p2", ""],
    ["Game: build refused (occupied/-) on plot:p2", "-"],
  ]) {
    const m = line.match(J.RELAY_BUILD_REFUSED_RE);
    assert.ok(m, line);
    assert.equal(m[2], why, line);
    assert.equal(m[3], "plot:p2");
  }
  assert.deepEqual("Game: take-down refused (not_allowed/not_your_piece)".match(J.RELAY_TAKE_DOWN_REFUSED_RE).slice(1), ["not_allowed", "not_your_piece"]);
  assert.deepEqual("Game: took down piece 3 on plot:p2".match(J.RELAY_TOOK_DOWN_RE).slice(1), ["3", "plot:p2"]);
});

// PROVISIONAL wood (LIMITS.WOOD_*, measured off the first real run): the wall's material lit, and
// nothing the Commons is known to hold when it is not there. Each pinned, so a change is seen.
test("wood is the wall's material, never the floor, the crew or a scaffold", () => {
  assert.ok(J.isWoodTint(150, 125, 92), "the expected lit wood");
  assert.ok(J.isWoodTint(110, 90, 66), "and in shade");
  for (const [what, c] of [
    ["the vehicle bay's floor", [181, 186, 192]],
    ["a grey wall", [61, 72, 87]],
    ["a crew member's amber body (copresence-judge isFigureAmber)", [176, 151, 71]],
    ["a scaffold's amber", [200, 160, 80]],
    ["a player's teal", [80, 193, 204]],
    ["black", [10, 8, 6]],
  ]) {
    assert.ok(!J.isWoodTint(...c), `${what} ${JSON.stringify(c)} is not wood`);
  }
});

// The rig judges a saved run from its folder, as `just verify-shared-build --dry-verdict` does:
// the run above written out (its recordings, its two pictures, the relay's log), then one record
// doctored. Red first, 2026-10-05, before verify-copresence.js knew a --build manifest (see the top).
test("--dry-verdict judges a saved --build run from its folder", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hum_shared-build-judge-"));
  try {
    const { run, evidence } = goodRun();
    const write = (name, body) => fs.writeFileSync(path.join(dir, name), body);
    write("theirs_samples.json", JSON.stringify({ ok: true, frames: evidence.recordings["theirs_samples.json"] }));
    write("zone_samples.json", JSON.stringify({ ok: true, frames: evidence.recordings["zone_samples.json"] }));
    write("zone_before.png", png.encode(evidence.pictures["zone_before.png"]));
    write("zone_after.png", png.encode(evidence.pictures["zone_after.png"]));
    write("relay.log", evidence.relayLog);
    const judge = (m) => {
      write("manifest.json", JSON.stringify(m));
      const out = spawnSync(process.execPath, [path.join(REPO, "scripts", "verify-copresence.js"), "--dry-verdict", path.join(dir, "manifest.json")], { encoding: "utf8", timeout: 60000 });
      return { status: out.status, text: `${out.stdout}\n${out.stderr}` };
    };
    const good = judge(run);
    assert.equal(good.status, 0, good.text);
    assert.match(good.text, new RegExp(`DRY VERDICT \\(nothing was booted\\): PASS  ${ALL_IDS.length}/${ALL_IDS.length}`));
    const bad = judge({ ...run, named: { ...run.named, theirs_in_ours: { ...run.named.theirs_in_ours, refusal: null, relay_lines: ["Game: built piece 9 wood_foundation on plot:p2"], drawn_others: [9] } } });
    assert.equal(bad.status, 2, bad.text);
    assert.match(bad.text, /FAIL  refused_their_build_in_our_plot/);
    assert.match(bad.text, /failed: refused_their_build_in_our_plot$/m);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
