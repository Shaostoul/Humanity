// The household judge (scripts/lib/household-judge.js): what `just verify-household`
// (verify-copresence.js --household, ship homes increment 5b, 2026-10-12) passes and fails.
//
// Run: node --test scripts/tests/household-judge.test.js   (in `just rig-tests`)
//
// Pure node: nothing is booted. A made-up run that went right passes every check; then one broken
// thing at a time, each failing its own check and no other (the plan's proof for the judge,
// docs/design/ship-homes-increment-5b-plan.md section 6: "Every check is also shown failing alone
// on a doctored copy of a green manifest"). Then the same run written to a folder is judged by the
// rig itself, `node scripts/verify-copresence.js --dry-verdict`, as a saved run would be.
//
// Red first, 2026-10-12, with the judge written and the rig not yet knowing --household: the
// --dry-verdict test judged the saved run as the default rig's ("DRY VERDICT (nothing was
// booted): FAIL  5/12 passed; failed: camera_parked, approach_clear, view_clear, screenshots,
// on_screen, figure_visible, recorded"), and the rig-source tests found no runHouseholdOnce ("the
// walks ... HOUSE_DOOR_MPS is NaN"). Then four breaks made on purpose in the judge, each seen red
// here and put back (the run of each is quoted beside what it broke): take_back_closes_it with its
// refusal test removed, the narrowed key check reading only whole keys, closed_before_permit with
// its wall test removed, and PROBE spelling one field otherwise.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const J = require("../lib/household-judge.js");
const { quarterTurn } = require("../second-player.js");
const png = require("../lib/png.js");

const REPO = path.join(__dirname, "..", "..");

// The two players' identity keys (Dilithium3 public keys are 3,904 hex characters; any 64 or more
// read as one), their did:hum, and the relay's.
const ME = "a1".repeat(40);
const ADA = "b2".repeat(40);
const ADA_DID = "did:hum:4AdaXq9WmZ7r3TfKp2VbN8";
const GAME_DID = "did:hum:7GameYp2QsL5c8HdRu4Ej1";
const SERVER_DID = "did:hum:9RelayKt3MwB6x2NzPq7Fa";

const T0 = 1791240000000; // the game's Give, ms since 1970
const T_SHARE = T0 + 20000; // Ada's build on the game's plot
const T_RENEW = T0 + 60000;
const T_HELD = T0 + 90000; // the relay stored Ada's permit for the game
const T_PIC = T0 + 140000; // the picture from inside her plot
const T_P1 = T0 + 170000; // the game's place on her plot
const T_TD = T0 + 190000; // the game's take-down of its own piece there
const T_BACK = T0 + 220000; // Take back

const FRAMES = [
  { id: "zone:commons", kind: "zone", min: [65, 0, 20], max: [99, 8, 75] },
  { id: "zone:street-1", kind: "zone", min: [65, 0, 85], max: [75, 4, 1185] },
  { id: "plot:p1", kind: "plot", min: [0, 0, 0], max: [55, 3, 89] },
  { id: "plot:p2", kind: "plot", min: [0, 0, 99], max: [55, 3, 188] },
];
// p1's corridor, as door_points.rs reports it (the first --build run's own numbers).
const DOOR = { plot: "plot:p1", zone: "zone:commons", axis: "x", mouth_zone: [65, 0, 40], mouth_plot: [55, 0, 40], step_zone: [66, 1.7, 40], step_plot: [54, 1.7, 40] };
const BLUEPRINTS = {
  wood_foundation: { name: "Wood Foundation", size: [4, 0.2, 4], build_time: 5, materials: [["wood_plank_0", 8]] },
  wood_wall: { name: "Wood Wall", size: [4, 3, 0.2], build_time: 4, materials: [["wood_plank_0", 6]] },
};

const permitGiven = { issuer: ME, server: SERVER_DID, plot: "p2", grantee: ADA_DID, serial: "11".repeat(16), expiry: Math.floor(T0 / 1000) + 30 * 86400 + 2, sig: "c2lnMQ==" };
const permitRenewed = { ...permitGiven, serial: "22".repeat(16), expiry: Math.floor(T_RENEW / 1000) + 30 * 86400 + 1, sig: "c2lnMg==" };
const permitHeld = { issuer: ADA, server: SERVER_DID, plot: "p1", grantee: GAME_DID, serial: "33".repeat(16), expiry: Math.floor(T_HELD / 1000) + 30 * 86400, sig: "c2lnMw==" };

/** The game's household block as lane 2A's probe writes it, in the plan's names. */
function household(over = {}) {
  return {
    me: ME,
    me_did: GAME_DID,
    server: SERVER_DID,
    my_plot: "p2",
    friends: [{ key: ADA, name: "TestBotAda", friend: true }],
    given: [],
    held: [],
    usable: [],
    walkable: [],
    lit: null,
    open_lights: 0,
    mouths: { p1: [{ others_only: true, open: 0 }, { others_only: true, open: 0 }], p2: [{ others_only: true, open: 0 }, { others_only: true, open: 0 }] },
    ...over,
  };
}
const row = (p, state) => ({ to: ADA, plot: p.plot, serial: p.serial, expiry: p.expiry, at: 0, state });
const heldOpen = () => ({
  held: [{ from: ADA, plot: "p1", serial: permitHeld.serial, expiry: permitHeld.expiry, verdict: "works" }],
  usable: ["p1"],
  walkable: ["p1"],
  mouths: { p1: [{ others_only: false, open: 1 }, { others_only: false, open: 1 }], p2: [{ others_only: true, open: 0 }, { others_only: true, open: 0 }] },
});

/** A picture 200 x 120: the home's grey, and with the wall's rect in wood. */
function picture(withWall) {
  const img = { width: 200, height: 120, rgba: Buffer.alloc(200 * 120 * 4) };
  for (let y = 0; y < 120; y++) {
    for (let x = 0; x < 200; x++) {
      const i = (y * 200 + x) * 4;
      const wood = withWall && x >= 100 && x < 150 && y >= 60 && y < 100;
      img.rgba[i] = wood ? 197 : 128;
      img.rgba[i + 1] = wood ? 176 : 128;
      img.rgba[i + 2] = wood ? 143 : 130;
      img.rgba[i + 3] = 255;
    }
  }
  return img;
}

/** The game's recording around the picture: Ada's wall (piece 2) on her plot, its rect on screen. */
function recording(rect = [100, 60, 150, 100]) {
  const frames = [];
  for (let k = 0, e = T_PIC - 1000; e <= T_PIC + 1000; e += 1000 / 15, k++) {
    frames.push({
      t: k / 15,
      dt: 1 / 15,
      epoch_ms: e,
      joined: true,
      cam: [48, 1.7, 44, 0, -0.05],
      players: [],
      crew: [],
      shared: [{ piece_id: 2, frame: "plot:p1", blueprint_id: "wood_wall", pos: [48, 0.2, 34], rot: quarterTurn(0), scale: [4, 3, 0.2], mine: false, built: true, progress: 4, rect }],
    });
  }
  return frames;
}

const pack = (n) => ({ wood_plank_0: n });

/** A --household run that went right, and its evidence. Fresh every call, so a case may break it. */
function goodRun() {
  const run = {
    kind: "verify-copresence-household",
    steps_ok: {
      relay: { ok: true, detail: "http://127.0.0.1:50000 answered /health" },
      joined: { ok: true, detail: "joined and welcomed on p2" },
      walker: { ok: true, detail: "TestBotAda in the world as entity 1, holding p1" },
    },
    relay: { server_did: SERVER_DID },
    frames: FRAMES,
    blueprints: BLUEPRINTS,
    walkers: { ada: { name: "TestBotAda", id: 1, plot: "p1", key: ADA, key16: ADA.slice(0, 16), did: ADA_DID } },
    game: { plot: "p2", entity: 2 },
    household_joined: household({ friends: [] }),
    her_pieces: {
      builds: [
        { cmd: { blueprint: "wood_foundation", frame: "plot:p1", local: [48, 0, 36], turns: 0 }, piece_id: 1, refusal: null },
        { cmd: { blueprint: "wood_wall", frame: "plot:p1", local: [48, 0.2, 34], turns: 0 }, piece_id: 2, refusal: null },
      ],
    },
    friends: {
      notice: "TestBotAda sent you a contact request. Accept or ignore it under Requests, in Chat or Settings > Safety.",
      notices_seen: [],
      pass_from: ME.slice(0, 12),
      follow_from: ME.slice(0, 12),
      household: household(),
    },
    closed: {
      door: DOOR,
      from: DOOR.step_zone,
      to: DOOR.step_plot,
      track: [[66, 1.7, 40], [65.62, 1.7, 40], [65.31, 1.7, 40]],
      end: [65.3, 1.7, 40],
      arrived: false,
      household: household(),
    },
    give: {
      epoch_ms: T0,
      days: 30,
      clicks: [{ label: "30 days", ok: true }, { label: "Give a permit", ok: true }, { label: "Give", ok: true }],
      household: household({ given: [row(permitGiven, "taken")] }),
      received: { permit: permitGiven, epoch_ms: T0 + 400 },
      checks_out: { plot: "p2", serial: permitGiven.serial, expiry: permitGiven.expiry, from: ME.slice(0, 12) },
    },
    ours_p2: { place: { blueprint: "wood_foundation", frame: "plot:p2", local: [48, 0, 36], turns: 0 }, piece: { piece_id: 3, frame: "plot:p2", mine: true, pos: [48, 0, 135] } },
    her_build: {
      cmd: { blueprint: "wood_foundation", frame: "plot:p2", local: [20, 0, 20], turns: 0 },
      permit_serial: permitGiven.serial,
      piece_id: 4,
      refusal: null,
      line_epoch_ms: T_SHARE,
      game_row: { piece_id: 4, frame: "plot:p2", blueprint_id: "wood_foundation", mine: false, pos: [20, 0, 119], rot: quarterTurn(0), built: false },
    },
    her_pieces_only: {
      her_take_down: {
        piece_id: 3,
        refusal: { action: "unbuild", reason: "not_allowed", why: "not_your_piece", line: "unbuild refused: not_allowed (not_your_piece) (req 5): Wood Foundation not taken down." },
        relay_lines: ["Game: take-down refused (not_allowed/not_your_piece)"],
        still_drawn: true,
      },
      our_take_down: { piece_id: 4, epoch_ms: T_SHARE + 5000, seen_by_walker: { epoch_ms: T_SHARE + 5300, piece_id: 4 }, gone: true },
    },
    renew: {
      epoch_ms: T_RENEW,
      clicks: [{ label: "Renew for 30 days", ok: true }],
      received: { permit: permitRenewed, epoch_ms: T_RENEW + 400 },
      checks_out: { plot: "p2", serial: permitRenewed.serial, expiry: permitRenewed.expiry, from: ME.slice(0, 12) },
      household: household({ given: [row(permitGiven, "taken_back"), row(permitRenewed, "taken")] }),
    },
    held: {
      sent: { permit: permitHeld, put_ok_epoch_ms: T_HELD },
      check_line: { line: "2026-10-12T12:01:31.120Z  INFO humanity::relay::handlers::shared_build: Game: permit for p1 checked (ok)", epoch_ms: T_HELD + 300 },
      household: household({ given: [row(permitGiven, "taken_back"), row(permitRenewed, "taken")], ...heldOpen() }),
    },
    walk_in: {
      target: [48, 1.7, 44],
      at: [48, 1.7, 44],
      arrived: true,
      corrections_before: 0,
      corrections_after: 0,
      seen_by_ada: [[66, 1.7, 40], [60, 1.7, 40], [52, 1.7, 41], [48, 1.7, 44]],
      household: household({ given: [row(permitGiven, "taken_back"), row(permitRenewed, "taken")], ...heldOpen(), lit: "p1", open_lights: 35 }),
    },
    picture: { with: "inside_p1.png", without: "inside_p1_without.png", samples: "inside_p1_samples.json", wall_piece_id: 2, with_epoch_ms: T_PIC, without_epoch_ms: T_BACK + 30000, wall_gone: true, page_with: "None", page_without: "None" },
    build_p1: {
      place: { blueprint: "wood_foundation", frame: "plot:p1", local: [20, 0, 20], turns: 0, epoch_ms: T_P1 },
      piece: { piece_id: 5, frame: "plot:p1", blueprint_id: "wood_foundation", mine: true, pos: [20, 0, 20], rot: quarterTurn(0), built: false },
      relay_lines: ["Game: built piece 5 wood_foundation on plot:p1"],
      seen_by_walker: { epoch_ms: T_P1 + 250, piece_id: 5, frame: "plot:p1", local: [20, 0, 20], turn: 0 },
    },
    take_down_p1: {
      theirs: { piece_id: 2, notices: ["Wood Wall not taken down: it was put up by someone else, and only this plot's holder can take it down."], relay_lines: [], still_drawn: true },
      ours: {
        piece_id: 5,
        blueprint: "wood_foundation",
        epoch_ms: T_TD,
        seen_by_walker: { epoch_ms: T_TD + 300, piece_id: 5 },
        gone: true,
        pack_before: pack(12),
        pack_after: pack(12),
        pack_later: pack(12),
        store_before: pack(60),
        store_after: pack(68),
        store_later: pack(68),
      },
    },
    take_back: {
      epoch_ms: T_BACK,
      clicks: [{ label: "Take back", ok: true }, { label: "Take back", ok: true, how: "the confirm" }],
      serial: permitRenewed.serial,
      household: household({ given: [row(permitGiven, "taken_back"), row(permitRenewed, "taken_back")], ...heldOpen(), lit: "p1", open_lights: 35 }),
      build: {
        cmd: { blueprint: "wood_foundation", frame: "plot:p2", local: [20, 0, 20], turns: 0 },
        permit_serial: permitRenewed.serial,
        piece_id: null,
        refusal: { action: "build", reason: "not_allowed", why: "permit_withdrawn", line: "build refused: not_allowed (permit_withdrawn) (req 9): Wood Foundation not built: the plot's holder took this permit back." },
        relay_lines: ["Game: build refused (not_allowed/permit_withdrawn) on plot:p2"],
      },
    },
    corrections_game: 0,
    walker_corrections: [],
    walker_keys: [ADA.slice(0, 16)],
    walks: [{ label: "to_door", to: [66, 1.7, 40], at: [66, 1.7, 40], ok: true }, { label: "into_p1", to: [48, 1.7, 44], at: [48, 1.7, 44], ok: true }],
    turns: [{ pose: "48,1.7,44,0,-0.05", at: [48, 1.7, 44], off: 0, ok: true }],
    panics: 0,
    builtin_data: [],
  };
  const evidence = {
    recordings: { "inside_p1_samples.json": recording() },
    pictures: { "inside_p1.png": picture(true), "inside_p1_without.png": picture(false) },
    // Other lines name players by their key's first 16 characters, by design (cd6cc01d3): only the
    // permit-check lines must not.
    relayLog: [
      `2026-10-12T12:00:01Z  INFO humanity::relay: Game: ${ADA.slice(0, 16)}.. joined the shared world`,
      `2026-10-12T12:00:20Z  INFO humanity::relay: Game: ${ME.slice(0, 16)}.. joined the shared world`,
      "Game: built piece 1 wood_foundation on plot:p1",
      "Game: built piece 2 wood_wall on plot:p1",
      "Game: built piece 3 wood_foundation on plot:p2",
      "Game: built piece 4 wood_foundation on plot:p2",
      "2026-10-12T12:01:31.120Z  INFO humanity::relay::handlers::shared_build: Game: permit for p1 checked (ok)",
      "Game: built piece 5 wood_foundation on plot:p1",
    ].join("\n"),
  };
  // A copy of its own: no case may change another case's run through a shared object.
  return { run: structuredClone(run), evidence };
}

const explain = (r) => r.checks.filter((c) => !c.ok).map((c) => `${c.id}: ${c.detail}`).join("\n");

const ALL_IDS = [
  "relay_up",
  "game_in_world",
  "walker_in_world",
  "plots_as_joined",
  "her_pieces_on_her_plot",
  "friends_made",
  "closed_before_permit",
  "give_through_the_page",
  "give_arrived",
  "relay_permit_lines_carry_no_key",
  "give_lets_her_build",
  "her_pieces_only",
  "renew_one_press",
  "held_and_checked",
  "walks_into_p1",
  "lit_inside",
  "sees_whats_there",
  "builds_on_p1",
  "takes_down_only_its_own",
  "take_back_closes_it",
  "moves_game_honest_never_corrected",
  "moves_relay_sent_what_the_game_took",
  "moves_walkers_never_corrected",
  "walks_all_arrived",
  "walks_turns_in_place",
  "no_panics",
  "no_builtin_data",
];

test("a --household run that went right passes, every check its own", () => {
  const { run, evidence } = goodRun();
  const r = J.judgeHousehold(run, evidence);
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), ALL_IDS);
});

/** The run with `fn` applied to it (and its evidence), judged. */
function doctored(fn) {
  const g = goodRun();
  fn(g.run, g.evidence);
  return J.judgeHousehold(g.run, g.evidence);
}
/** The household block of a record, to break. */
const hh = (rec) => rec.household;

// One broken thing at a time, each failing its own check and no other. Each case's check was SEEN
// failing on its doctored copy (this test asserts exactly that, case by case); where a case's
// check was also broken in the judge on purpose, the red run is quoted beside it.
const CASES = [
  // 1-2: the rig's own steps, and who holds what.
  ["the relay never came up", (r) => (r.steps_ok.relay = { ok: false, detail: "never answered /health" }), "relay_up"],
  ["the game never joined", (r) => (r.steps_ok.joined = { ok: false, detail: "game_joined=false" }), "game_in_world"],
  ["Ada never joined", (r) => delete r.steps_ok.walker, "walker_in_world"],
  ["the household names another plot than the welcome", (r) => (r.household_joined.my_plot = "p3"), "plots_as_joined"],
  ["the household reports no key for the game", (r) => (r.household_joined.me = ""), ["plots_as_joined", "friends_made", "give_arrived", "relay_permit_lines_carry_no_key"]],
  ["Ada and the game handed one plot", (r) => (r.walkers.ada.plot = "p2"), null], // many: see below
  ["Ada's foundation refused by the relay", (r) => Object.assign(r.her_pieces.builds[0], { piece_id: null, refusal: { action: "build", reason: "occupied" } }), "her_pieces_on_her_plot"],
  ["Ada's pieces never recorded", (r) => (r.her_pieces = undefined), ["her_pieces_on_her_plot", "sees_whats_there", "takes_down_only_its_own"]],
  // 3: friends.
  ["no notice named her request", (r) => (r.friends.notice = null), "friends_made"],
  ["the notice names someone else", (r) => (r.friends.notice = "TestBotBo sent you a contact request."), "friends_made"],
  ["Ada never got the game's friend pass", (r) => (r.friends.pass_from = null), "friends_made"],
  ["Ada's follow came from another key", (r) => (r.friends.follow_from = "c3c3c3c3c3c3"), "friends_made"],
  ["the game does not list her as a friend", (r) => (r.friends.household.friends = []), "friends_made"],
  // 4: shut before a permit. Red in the judge 2026-10-12 with its wall test removed (`neverPast`
  // always true): "the game walked through the shut wall into her plot should fail
  // closed_before_permit (only); failed: nothing".
  ["the game walked through the shut wall into her plot", (r) => r.closed.track.push([60, 1.7, 40]), "closed_before_permit"],
  ["the walk arrived inside her plot", (r) => Object.assign(r.closed, { arrived: true, end: [54, 1.7, 40] }), "closed_before_permit"],
  ["the walk stopped 3 m short of the wall (it never reached the corridor)", (r) => Object.assign(r.closed, { track: [[69, 1.7, 40]], end: [68, 1.7, 40] }), "closed_before_permit"],
  ["no position read during the shut walk", (r) => Object.assign(r.closed, { track: [], end: null }), "closed_before_permit"],
  ["her corridor doors open for the game before any permit", (r) => (r.closed.household.mouths.p1[0].others_only = false), "closed_before_permit"],
  ["her corridor doors never reported", (r) => delete r.closed.household.mouths.p1, "closed_before_permit"],
  ["her plot walkable before any permit", (r) => (r.closed.household.walkable = ["p1"]), "closed_before_permit"],
  ["her home's lights on before any permit", (r) => (r.closed.household.open_lights = 35), "closed_before_permit"],
  // 5: the game gives, through its page.
  ["the given row still on its way", (r) => (hh(r.give).given[0].state = "on_its_way"), "give_through_the_page"],
  ["no given row at all", (r) => (hh(r.give).given = []), "give_through_the_page"],
  ["two given rows to Ada", (r) => hh(r.give).given.push(row(permitRenewed, "taken")), "give_through_the_page"],
  ["the permit runs 29 days, not 30", (r) => (hh(r.give).given[0].expiry -= 86400), "give_through_the_page"],
  ["the given row for another plot", (r) => (hh(r.give).given[0].plot = "p1"), "give_through_the_page"],
  ["Ada never received a permit", (r) => (r.give.received = null), ["give_arrived", "give_lets_her_build", "renew_one_press"]],
  ["Ada's permit names another person", (r) => (r.give.received.permit = { ...permitGiven, grantee: GAME_DID }), "give_arrived"],
  ["Ada's permit names another server", (r) => (r.give.received.permit = { ...permitGiven, server: "did:hum:Other" }), "give_arrived"],
  ["Ada's permit from someone other than the game", (r) => (r.give.received.permit = { ...permitGiven, issuer: "c3".repeat(40) }), "give_arrived"],
  ["Ada's permit for another plot", (r) => (r.give.received.permit = { ...permitGiven, plot: "p3" }), "give_arrived"],
  ["Ada's own check refused it", (r) => Object.assign(r.give, { checks_out: null, refused: "does_not_check_out" }), "give_arrived"],
  // The narrowed key check (the lanes' correction to plan step 5). Red in the judge 2026-10-12 with
  // it looking only for whole keys: "the first 16 of the game's key in a permit-check line should
  // fail relay_permit_lines_carry_no_key (only); failed: nothing".
  ["the game's whole key in a permit-check line", (r, e) => (e.relayLog += `\nGame: permit for p1 checked (ok) for ${ME}`), "relay_permit_lines_carry_no_key"],
  ["the first 16 of the game's key in a permit-check line", (r, e) => (e.relayLog += `\nGame: permit for p1 checked (ok) asked by ${ME.slice(0, 16)}..`), "relay_permit_lines_carry_no_key"],
  ["the first 16 of Ada's key in a permit-check line", (r, e) => (e.relayLog += `\nGame: permit for p1 checked (not_allowed/permit_withdrawn) from ${ADA.slice(0, 16)}`), "relay_permit_lines_carry_no_key"],
  ["a did:hum in a permit-check line", (r, e) => (e.relayLog += `\nGame: permit for p1 checked (ok) grantee ${GAME_DID}`), "relay_permit_lines_carry_no_key"],
  ["no permit-check line at all (nothing to look at)", (r, e) => (e.relayLog = e.relayLog.split("\n").filter((l) => !/permit for/.test(l)).join("\n")), "relay_permit_lines_carry_no_key"],
  ["the relay's log never kept", (r, e) => (e.relayLog = undefined), ["relay_permit_lines_carry_no_key", "moves_relay_sent_what_the_game_took"]],
  // 6: her permit works, and only for her own pieces.
  ["Ada's build on the game's plot with its permit refused", (r) => Object.assign(r.her_build, { piece_id: null, refusal: { action: "build", reason: "not_allowed", why: "not_your_plot" } }), "give_lets_her_build"],
  ["her piece drawn as the game's own", (r) => (r.her_build.game_row.mine = true), "give_lets_her_build"],
  ["her piece drawn 5 cm off", (r) => (r.her_build.game_row.pos = [20, 0, 119.05]), "give_lets_her_build"],
  ["her piece never drawn", (r) => (r.her_build.game_row = null), "give_lets_her_build"],
  ["she built with some other permit", (r) => (r.her_build.permit_serial = "44".repeat(16)), "give_lets_her_build"],
  ["Ada took the game's piece down with her permit", (r) => Object.assign(r.her_pieces_only.her_take_down, { refusal: null, relay_lines: ["Game: took down piece 3 on plot:p2"], still_drawn: false }), "her_pieces_only"],
  ["her take-down refused for another reason", (r) => (r.her_pieces_only.her_take_down.refusal.why = "not_your_plot"), "her_pieces_only"],
  ["the game never took her piece down", (r) => Object.assign(r.her_pieces_only.our_take_down, { seen_by_walker: null, gone: false }), "her_pieces_only"],
  // 7: renew.
  ["no second permit came", (r) => (r.renew.received = null), ["renew_one_press", "take_back_closes_it"]],
  ["the renewal ends no later", (r) => (r.renew.received.permit = { ...permitRenewed, expiry: permitGiven.expiry }), "renew_one_press"],
  ["the old permit still stands beside the new (amendment A1 not done)", (r) => (hh(r.renew).given[0].state = "taken"), "renew_one_press"],
  ["the renewal still on its way", (r) => (hh(r.renew).given[1].state = "on_its_way"), "renew_one_press"],
  ["Ada's own check refused the renewal", (r) => (r.renew.checks_out = null), "renew_one_press"],
  // 8: the game holds one.
  ["the relay never said it works", (r) => (hh(r.held).held[0].verdict = "checking"), "held_and_checked"],
  ["the relay said not from the holder", (r) => (hh(r.held).held[0].verdict = { refused: "permit_not_from_holder" }), "held_and_checked"],
  ["the game never kept her permit", (r) => (hh(r.held).held = []), "held_and_checked"],
  ["the check logged 3 s after the permit was stored", (r) => (r.held.check_line.epoch_ms = T_HELD + 3000), "held_and_checked"],
  ["the relay never logged a check", (r) => (r.held.check_line = null), "held_and_checked"],
  ["the check said not_allowed", (r) => (r.held.check_line.line = "Game: permit for p1 checked (not_allowed/permit_bad)"), "held_and_checked"],
  ["her plot not walkable", (r) => (hh(r.held).walkable = []), "held_and_checked"],
  // The plan's red: with door_panels leaving others_only set, the doors stay shut for the game.
  ["her corridor doors still for other players only", (r) => (hh(r.held).mouths.p1 = [{ others_only: true, open: 0 }, { others_only: true, open: 0 }]), "held_and_checked"],
  // 9: walks in. The plan's red again: the doors shut, so the walk never arrives.
  ["the walk into her plot never arrived", (r) => Object.assign(r.walk_in, { arrived: false, at: [65.3, 1.7, 40] }), "walks_into_p1"],
  ["corrected on the way in", (r) => (r.walk_in.corrections_after = 1), "walks_into_p1"],
  ["Ada never saw it inside her plot", (r) => (r.walk_in.seen_by_ada = [[66, 1.7, 40]]), "walks_into_p1"],
  ["the target outside her plot", (r) => Object.assign(r.walk_in, { target: [70, 1.7, 44], at: [70, 1.7, 44] }), "walks_into_p1"],
  ["her home dark inside", (r) => (hh(r.walk_in).open_lights = 0), "lit_inside"],
  ["another plot lit", (r) => (hh(r.walk_in).lit = "p2"), "lit_inside"],
  // 10: sees what is there.
  ["a window over the wall: no wood in the picture", (r, e) => (e.pictures["inside_p1.png"] = picture(false)), "sees_whats_there"],
  ["wood there without her wall too: no gain (an oak panel, not her wall)", (r, e) => (e.pictures["inside_p1_without.png"] = picture(true)), "sees_whats_there"],
  ["her wall's rect off the right of the view", (r, e) => (e.recordings["inside_p1_samples.json"] = recording([100, 60, 210, 100])), "sees_whats_there"],
  ["her wall behind the camera (no rect)", (r, e) => (e.recordings["inside_p1_samples.json"] = recording(null)), "sees_whats_there"],
  ["the control taken while the game still had her wall", (r) => (r.picture.wall_gone = false), "sees_whats_there"],
  // The Home page left over the picture without her wall hides her wall's place: in the picture
  // that is a gain of wood, so the judge must refuse a picture taken with a page drawn. Red in the
  // judge 2026-10-12 with that test removed (`worldView` always true): "the Home page drawn over
  // the picture without her wall should fail sees_whats_there (only); failed: nothing".
  ["the Home page drawn over the picture without her wall", (r) => (r.picture.page_without = "Homes"), "sees_whats_there"],
  ["which page was drawn never asked", (r) => (r.picture.page_with = null), "sees_whats_there"],
  ["the pictures never taken", (r, e) => delete e.pictures["inside_p1_without.png"], "sees_whats_there"],
  // 11: builds there. The plan's red: send_next not attaching the permit, refused not_your_plot.
  ["the game's piece on her plot never stood (refused not_your_plot, the permit not attached)", (r) => Object.assign(r.build_p1, { piece: null, relay_lines: ["Game: build refused (not_allowed/not_your_plot) on plot:p1"] }), "builds_on_p1"],
  ["the game's piece there not its own", (r) => (r.build_p1.piece.mine = false), "builds_on_p1"],
  ["Ada never saw it", (r) => (r.build_p1.seen_by_walker = null), "builds_on_p1"],
  ["Ada saw it 3 s late", (r) => (r.build_p1.seen_by_walker.epoch_ms = T_P1 + 3000), "builds_on_p1"],
  ["the relay never logged it", (r) => (r.build_p1.relay_lines = []), "builds_on_p1"],
  // 12: takes down only its own.
  ["no not-your-piece words at the crosshair", (r) => (r.take_down_p1.theirs.notices = []), "takes_down_only_its_own"],
  ["the take-down of her wall sent to the relay anyway", (r) => (r.take_down_p1.theirs.relay_lines = ["Game: take-down refused (not_allowed/not_your_piece)"]), "takes_down_only_its_own"],
  ["her wall gone from the game's world", (r) => (r.take_down_p1.theirs.still_drawn = false), "takes_down_only_its_own"],
  ["Ada never saw the game's own piece come down", (r) => (r.take_down_p1.ours.seen_by_walker = null), "takes_down_only_its_own"],
  ["the planks given back twice", (r) => (r.take_down_p1.ours.store_later = pack(76)), "takes_down_only_its_own"],
  // 13: take back (amendment A1). Red in the judge 2026-10-12 with the permit_withdrawn refusal
  // test removed: "Ada's build with the taken-back permit KEPT should fail take_back_closes_it
  // (only); failed: nothing".
  ["Ada's build with the taken-back permit KEPT", (r) => Object.assign(r.take_back.build, { piece_id: 7, refusal: null, relay_lines: ["Game: built piece 7 wood_foundation on plot:p2"] }), "take_back_closes_it"],
  ["her build refused, but as someone else's plot", (r) => Object.assign(r.take_back.build, { refusal: { action: "build", reason: "not_allowed", why: "not_your_plot" }, relay_lines: ["Game: build refused (not_allowed/not_your_plot) on plot:p2"] }), "take_back_closes_it"],
  ["the given row not taken back", (r) => (hh(r.take_back).given[1].state = "taken"), "take_back_closes_it"],
  ["the old permit taken back, not the one standing", (r) => (r.take_back.serial = permitGiven.serial), "take_back_closes_it"],
  ["she built with another permit than the one taken back", (r) => (r.take_back.build.permit_serial = permitGiven.serial), "take_back_closes_it"],
  ["the relay never logged the refusal", (r) => (r.take_back.build.relay_lines = []), "take_back_closes_it"],
  // 14: over the run.
  ["the game corrected once", (r, e) => ((r.corrections_game = 1), (e.relayLog += `\nGame: corrected ${ME.slice(0, 16)}.. (correction 1, too_fast): [60.0,1.7,40.0]`)), "moves_game_honest_never_corrected"],
  ["a correction the game never took", (r, e) => (e.relayLog += `\nGame: corrected ${ME.slice(0, 16)}.. (correction 1, too_fast): [60.0,1.7,40.0]`), "moves_relay_sent_what_the_game_took"],
  ["Ada corrected", (r) => (r.walker_corrections = ["[TestBotAda] second-player: corrected to (27.50, 1.70, 44.50) (too_fast, correction 1)"]), "moves_walkers_never_corrected"],
  ["a walk that never arrived", (r) => (r.walks[0].ok = false), "walks_all_arrived"],
  ["a turn sent from somewhere else (a teleport)", (r) => Object.assign(r.turns[0], { at: [40, 1.7, 44], off: 8, ok: false }), "walks_turns_in_place"],
  ["a PANIC", (r) => (r.panics = 1), "no_panics"],
  ["a built-in data copy served", (r) => (r.builtin_data = ["[built-in data copy] ship/household.ron"]), "no_builtin_data"],
];

test("each broken run FAILS its own check and no other", () => {
  for (const [what, fn, id] of CASES) {
    if (id === null) continue; // judged below
    const r = doctored(fn);
    const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
    const ids = Array.isArray(id) ? id : [id];
    assert.deepEqual(failed, ids, `${what} should fail ${ids.join(" and ")} (only); failed: ${failed.join(", ") || "nothing"}`);
  }
  // Every check is shown failing ALONE by some case above (a check no broken run fails on its own
  // may be one that cannot fail).
  const alone = new Set(CASES.filter(([, , id]) => id !== null && (!Array.isArray(id) || id.length === 1)).map(([, , id]) => (Array.isArray(id) ? id[0] : id)));
  assert.deepEqual(ALL_IDS.filter((id) => !alone.has(id)), [], "every check has a broken run that fails it and nothing else");
  assert.equal(J.judgeHousehold(null, {}).pass, false, "a run nobody recorded fails");
});

// Ada and the game on one plot breaks the run from the start: the plot checks fail, and so does
// everything that names the game's plot as other than hers. What matters is that plots_as_joined
// is among them.
test("two players on one plot fails plots_as_joined", () => {
  const r = doctored((run) => (run.walkers.ada.plot = "p2"));
  assert.ok(r.checks.find((c) => c.id === "plots_as_joined").ok === false, explain(r));
});

test("nothing recorded fails every check that has evidence to read", () => {
  const r = J.judgeHousehold({}, {});
  const passed = r.checks.filter((c) => c.ok).map((c) => c.id);
  assert.deepEqual(passed, [], `with nothing recorded nothing may pass; passed: ${passed.join(", ")}`);
});

// THE PROBE'S FIELD NAMES LIVE IN ONE PLACE. Lane 2A's `household` block was written in parallel
// with this judge, against the plan's names; a rename at merge must be a one-line change in
// household-judge.js `PROBE`. So: a block written from PROBE's own values reads back whole, the
// mouths read the same in either shape the plan allows, and the rig never spells a field itself
// (it reads the block through `readHousehold`). That PROBE is load-bearing was seen 2026-10-12 by
// spelling `walkable` as "walkable_plots" in it: the good run then failed held_and_checked
// ("walkable [], usable ["p1"]; p1's corridor doors open for this player"), as a game whose block
// names the field otherwise would.
test("the household block is read through PROBE alone, in either mouths shape", () => {
  const P = J.PROBE;
  const raw = {
    [P.me]: ME.toUpperCase(),
    [P.me_did]: GAME_DID,
    [P.server]: SERVER_DID,
    [P.my_plot]: "p2",
    [P.friends]: [{ [P.friend_key]: ADA }, ADA.toUpperCase()],
    [P.given]: [{ [P.given_to]: ADA, [P.given_plot]: "p2", [P.given_serial]: permitGiven.serial, [P.given_expiry]: permitGiven.expiry, [P.given_state]: "taken" }],
    // A held row that carries its permit whole, its verdict serde's tagged object.
    [P.held]: [{ [P.held_from]: ADA, permit: { plot: "p1", serial: permitHeld.serial, expiry: permitHeld.expiry }, [P.held_verdict]: { refused: "permit_withdrawn" } }],
    [P.usable]: ["p1"],
    [P.walkable]: ["p1"],
    [P.lit]: "p1",
    [P.open_lights]: 35,
    [P.mouths]: { p1: [{ [P.mouth_others_only]: false, [P.mouth_open]: 1 }] },
  };
  const h = J.readHousehold(J.householdOfProbe({ [P.block]: raw }));
  assert.equal(h.me, ME, "keys read in lowercase");
  assert.deepEqual(h.friends, [ADA, ADA]);
  assert.deepEqual(h.given, [{ to: ADA, plot: "p2", serial: permitGiven.serial, expiry: permitGiven.expiry, state: "taken" }]);
  assert.deepEqual(h.held, [{ from: ADA, plot: "p1", serial: permitHeld.serial, expiry: permitHeld.expiry, verdict: "refused" }]);
  assert.deepEqual([h.my_plot, h.lit, h.open_lights, h.usable, h.walkable], ["p2", "p1", 35, ["p1"], ["p1"]]);
  const asList = J.readHousehold({ ...raw, [P.mouths]: [{ [P.mouth_plot]: "p1", [P.mouth_others_only]: false, [P.mouth_open]: 1 }] });
  assert.deepEqual(asList.mouths, h.mouths, "a list of mouths reads as the map");
  assert.deepEqual(h.mouths, { p1: [{ others_only: false, open: 1 }] });
  assert.equal(J.householdOfProbe({}), null, "a probe without the block (a build before lane 2A) reads as none");
  assert.equal(J.readHousehold(null), null);

  // The rig reads the block only through readHousehold: no field of it spelt in the rig's source.
  const rig = fs.readFileSync(path.join(REPO, "scripts", "verify-copresence.js"), "utf8");
  const from = rig.indexOf("async function runHouseholdOnce(");
  assert.ok(from >= 0, "verify-copresence.js has runHouseholdOnce");
  for (const field of ["walkable", "open_lights", "others_only", "my_plot", "me_did"]) {
    assert.ok(!new RegExp(`\\.${field}\\b|\\["${field}"\\]`).test(rig.slice(from).replace(/\bH[A-Za-z0-9]*\.(walkable|open_lights|my_plot|me_did)\b/g, "")), `the rig spells the household's "${field}" itself; read it through readHousehold so PROBE stays the one place`);
  }
});

// The words the judge looks for on the game's screen are the game's own: the one copy lives in
// src/systems/construction/shared.rs (`Why::NotYourPiece`).
test("the not-your-piece words are the contract's own", () => {
  const shared = fs.readFileSync(path.join(REPO, "src", "systems", "construction", "shared.rs"), "utf8");
  assert.ok(shared.includes(J.NOT_YOUR_PIECE_SENTENCE), `shared.rs no longer says "${J.NOT_YOUR_PIECE_SENTENCE}"`);
  assert.ok(shared.includes('"permit_withdrawn"'), "shared.rs no longer names permit_withdrawn, which take_back_closes_it reads");
});

// The relay's permit-check line (src/relay/handlers/shared_build.rs `permit_check`), read in both
// its forms, and the source still writing it that way.
test("the relay's permit-check line is read as the relay writes it", () => {
  assert.deepEqual("x INFO r: Game: permit for p1 checked (ok)".match(J.RELAY_PERMIT_CHECK_RE).slice(1), ["p1", "ok"]);
  assert.deepEqual("Game: permit for p12 checked (not_allowed/permit_withdrawn)".match(J.RELAY_PERMIT_CHECK_RE).slice(1), ["p12", "not_allowed/permit_withdrawn"]);
  const src = fs.readFileSync(path.join(REPO, "src", "relay", "handlers", "shared_build.rs"), "utf8");
  assert.ok(src.includes('"Game: permit for {} checked (ok)"'), "shared_build.rs no longer logs the ok check as the judge reads it");
  assert.ok(src.includes('"Game: permit for {} checked (not_allowed/{})"'), "shared_build.rs no longer logs a refused check as the judge reads it");
});

// Where the shut walk stands against the wall: on the Commons' side is negative, past it positive,
// whichever way the corridor runs.
test("pastWall measures toward the plot along the corridor", () => {
  assert.ok(Math.abs(J.pastWall(DOOR, [65.3, 1.7, 40]) + 0.3) < 1e-9, "0.3 m on the Commons' side");
  assert.ok(Math.abs(J.pastWall(DOOR, [64, 1.7, 40]) - 1) < 1e-9, "1 m past it, into the corridor");
  const east = { axis: "z", mouth_zone: [70, 0, 75], mouth_plot: [70, 0, 85] };
  assert.ok(Math.abs(J.pastWall(east, [70, 1.7, 74]) + 1) < 1e-9);
  assert.ok(Number.isNaN(J.pastWall(null, [0, 0, 0])), "no door, no measure");
});

// THE RIG'S WALKS THROUGH THE CORRIDOR GO AT WALKING PACE. The showcase `walk_to` moves the camera
// itself each frame (engine/move_check.rs walk_tick) and the wall collision then pushes it out of
// a wall it stepped into (lib.rs, wall_collision::resolve). A stride wider than the wall's band
// (its half thickness and the player's 0.3 m, either side) can land past the wall's middle and be
// pushed out the far side: walked through. The step is capped at 0.1 s of walking, so at
// 1.5 m/s or less a stride is at most 0.15 m, the collision's own substep, and the shut leg is
// stopped as a person is (at the --build rig's 6 m/s it is 0.6 m: a shut door could pass for open).
test("the walks against and through her corridor are at walking pace", () => {
  const rig = fs.readFileSync(path.join(REPO, "scripts", "verify-copresence.js"), "utf8");
  const mps = Number((rig.match(/^const HOUSE_DOOR_MPS = ([\d.]+);/m) || [])[1]);
  assert.ok(mps > 0 && mps * 0.1 <= 0.15, `HOUSE_DOOR_MPS is ${mps}: a stride of ${(mps * 0.1).toFixed(2)} m can step through a shut wall`);
  const from = rig.indexOf("async function runHouseholdOnce(");
  const body = rig.slice(from, rig.indexOf("\nasync function ", from + 1));
  assert.match(body, /walk_to: `[^`]*,\$\{HOUSE_DOOR_MPS\}`/, "the shut walk goes at HOUSE_DOOR_MPS");
  assert.match(body, /walkGame\(door\.step_plot, [^)]*HOUSE_DOOR_MPS\)/, "the walk in through the corridor goes at HOUSE_DOOR_MPS");
});

// The rig judges a saved run from its folder, as `just verify-household --dry-verdict` does: the
// run above written out (its recording, its two pictures, the relay's log), then one record
// doctored. Red first, 2026-10-12, before verify-copresence.js knew a --household manifest (see the
// top).
test("--dry-verdict judges a saved --household run from its folder", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hum_household-judge-"));
  try {
    const { run, evidence } = goodRun();
    const write = (name, body) => fs.writeFileSync(path.join(dir, name), body);
    write("inside_p1_samples.json", JSON.stringify({ ok: true, frames: evidence.recordings["inside_p1_samples.json"] }));
    write("inside_p1.png", png.encode(evidence.pictures["inside_p1.png"]));
    write("inside_p1_without.png", png.encode(evidence.pictures["inside_p1_without.png"]));
    write("relay.log", evidence.relayLog);
    const judge = (m) => {
      write("manifest.json", JSON.stringify(m));
      const out = spawnSync(process.execPath, [path.join(REPO, "scripts", "verify-copresence.js"), "--dry-verdict", path.join(dir, "manifest.json")], { encoding: "utf8", timeout: 60000 });
      return { status: out.status, text: `${out.stdout}\n${out.stderr}` };
    };
    const good = judge(run);
    assert.equal(good.status, 0, good.text);
    assert.match(good.text, new RegExp(`DRY VERDICT \\(nothing was booted\\): PASS  ${ALL_IDS.length}/${ALL_IDS.length}`));
    const bad = judge({ ...run, take_back: { ...run.take_back, build: { ...run.take_back.build, piece_id: 7, refusal: null, relay_lines: ["Game: built piece 7 wood_foundation on plot:p2"] } } });
    assert.equal(bad.status, 2, bad.text);
    assert.match(bad.text, /FAIL  take_back_closes_it/);
    assert.match(bad.text, /failed: take_back_closes_it$/m);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
