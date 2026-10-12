// THE HOUSEHOLD JUDGE (ship homes increment 5b, "the household page, and walking into a permitted
// plot", 2026-10-12; docs/design/ship-homes-increment-5b-plan.md section 6, with the orchestrator's
// amendments in its section 8, which win where they differ): did a real game, a real relay and one
// scripted friend give, use, renew, hold, check and take back household permits, and did the
// permitted plot open to the game and only to it?
//
// Pure: it reads one run's record (the manifest scripts/verify-copresence.js --household writes)
// and the evidence that came with it (the game's recording at the picture, the two pictures, the
// relay's log text), and returns checks, so it is tested with made-up runs
// (scripts/tests/household-judge.test.js, in `just rig-tests`) and re-run on a saved run without
// booting anything (`node scripts/verify-copresence.js --dry-verdict <manifest.json>`).
//
// THE RUN, in the plan's steps (verify-copresence.js runHouseholdOnce says how each record is made):
//   1-2  a throwaway relay; walker Ada (TestBotAda) joins first (p1) and builds a wood foundation
//        and a wood wall on her plot; the game joins second (p2): `plots_as_joined`,
//        `her_pieces_on_her_plot`.
//   3    FRIENDS: Ada sends the game a contact request, the game's notice names it, the game
//        accepts (the showcase `accept_request` verb); Ada logs the game's pass and follow and the
//        game lists her as a friend (`friends_made`).
//   4    SHUT BEFORE A PERMIT: the game walks from the Commons into p1's corridor and is stopped at
//        the Commons' wall; p1's corridor doors are for other players only; nothing is walkable or
//        lit (`closed_before_permit`).
//   5    THE GAME GIVES, THROUGH ITS PAGE: "30 days", "Give a permit", "Give"; one given row,
//        taken; Ada receives it and it checks out (`give_through_the_page`, `give_arrived`). No
//        relay permit-check line names anyone (`relay_permit_lines_carry_no_key`, the plan's
//        "no line holding a key" narrowed to those lines: other relay lines name players by
//        design, now by their key's first 16 characters).
//   6    HER PERMIT WORKS, AND ONLY FOR HER OWN PIECES: Ada builds on the game's plot with it and
//        the game draws it, not its own; her take-down of the game's piece is refused
//        not_your_piece; the game takes hers down (`give_lets_her_build`, `her_pieces_only`).
//   7    RENEW: one press; Ada gets a second permit ending later, and since amendment A1 the
//        earlier one is taken back, so one stands (`renew_one_press`).
//   8    THE GAME HOLDS ONE: Ada gives the game a permit for p1; the relay checks it within 2 s and
//        says it works; p1 opens (`held_and_checked`).
//   9    WALKS IN: through p1's corridor to a point inside her home, never corrected, Ada seeing it
//        there (`walks_into_p1`); her home's lights on while it stands there (`lit_inside`).
//   10   SEES WHAT IS THERE: Ada's wall in the picture from inside, wood in its screen rect that is
//        gone in a picture at the same pose after she took it down (`sees_whats_there`).
//   11   BUILDS THERE: the game's foundation on p1, its own, Ada seeing it (`builds_on_p1`).
//   12   TAKES DOWN ONLY ITS OWN: its take-down of Ada's wall refused at its crosshair and never
//        sent; its own foundation comes down, the planks back once (`takes_down_only_its_own`).
//   13   TAKE BACK (amendment A1): the game takes back Ada's permit through its page; her next
//        build on the game's plot with it is refused permit_withdrawn (`take_back_closes_it`).
//   14   OVER THE RUN: no correction of the game or Ada, walks arrived, no panic, no built-in data.
//
// WHAT THE GAME REPORTS: the probe's `household` block (lane 2A, src/engine/ipc.rs, beside
// `shared_build`). Its field names live in ONE place, `PROBE` and `WORDS` below, and every reader
// (this judge, and the rig's waits) goes through `readHousehold`: a rename at merge is a one-line
// change there. The pieces, packs and notices are the --build probe's (shared-build-judge.js).
//
// WHAT THE RELAY LOGS (never a key): "Game: permit for {plot} checked (ok)" or
// "... checked (not_allowed/{why})" (src/relay/handlers/shared_build.rs `permit_check`), and the
// --build piece lines.

"use strict";

const { judgeWalks, relayCorrections } = require("./copresence-judge.js");
// One copy of the piece maths, the wood tint and the relay's piece lines: the --build judge's.
const SB = require("./shared-build-judge.js");

/// THE PROBE'S FIELD NAMES (lane 2A's `household` block; plan section 5, 2A's probe list: `me`
/// the full public key, `me_did`, `server`, `my_plot`, friends, given, held with verdicts, `usable`,
/// `walkable`, `lit`, `open_lights`, and p1 to p12's corridor `mouths` with `others_only` and the
/// open fraction). Written against the plan's names before 2A merged: if 2A spelt one otherwise,
/// change it HERE and nowhere else. A row field missing from the row is also looked for in a nested
/// `permit` object (a held row may carry the whole permit).
const PROBE = {
  block: "household",
  me: "me",
  me_did: "me_did",
  server: "server",
  my_plot: "my_plot",
  friends: "friends",
  friend_key: "key", // a friend row's key (a row may also be the key itself, a string)
  given: "given",
  given_to: "to",
  given_plot: "plot",
  given_serial: "serial",
  given_expiry: "expiry",
  given_state: "state",
  held: "held",
  held_from: "from",
  held_plot: "plot",
  held_serial: "serial",
  held_expiry: "expiry",
  held_verdict: "verdict",
  usable: "usable",
  walkable: "walkable",
  lit: "lit",
  open_lights: "open_lights",
  mouths: "mouths", // { "p1": [row, ...] } or [{ plot, ... }, ...]
  mouth_plot: "plot",
  mouth_others_only: "others_only",
  mouth_open: "open",
};
/// THE WORDS THOSE FIELDS CARRY: a given row's state as src/net/household.rs `GivenState`
/// serializes it (snake_case), and a held row's verdict (`Verdict`, snake_case). A state or verdict
/// written as an object (serde's external tag, {"refused": ...}) reads as its tag.
const WORDS = {
  given_taken: "taken",
  given_taken_back: "taken_back",
  /// A permit the friend may hold now (`GivenState::may_stand`).
  given_standing: ["taken", "on_its_way", "unconfirmed"],
  verdict_works: "works",
};

/// The limits, each with its reason.
const LIMITS = {
  /// How far from "now plus the length" a given permit's end date may be, seconds (plan step 5:
  /// "expiry within 120 s of now plus 30 days"): the rig's clock is read at the confirm click, the
  /// game reads its own a frame or two later.
  EXPIRY_TOL_S: 120,
  /// How soon after the relay stored Ada's permit for the game it must log the game's check of
  /// it, seconds (plan step 8: "within 2 s"). The game asks at once when a permit arrives in session.
  CHECK_WITHIN_S: 2,
  /// How much earlier than Ada's `dm put ok` the rig may see the check line, seconds: the two are
  /// read by polling, the relay's log every 100 ms and Ada's output as it comes.
  CHECK_EARLY_S: 0.5,
  /// How far past the Commons' wall at the corridor's mouth the shut walk may stand, metres, and
  /// how near it it must stop (plan step 4: "never passes ... stands within 1 m of it"). The
  /// player's radius is 0.3 m (wall_collision.rs PLAYER_RADIUS).
  PASS_TOL_M: 0.05,
  STOP_NEAR_M: 1,
  /// How soon after the game acts Ada must hear of it, seconds (as --build: one frame or two).
  SEEN_WITHIN_S: SB.LIMITS.SEEN_WITHIN_S,
  /// How far from where it was built a piece may be drawn, metres (plan: "within 1 cm").
  POSE_TOL_M: SB.LIMITS.POSE_TOL_M,
  /// The wood in the wall's screen rect, measured as --build's step 7 measures it (PROVISIONAL
  /// there until its first run; that run measured 95% with a gain of 95%, median [197,176,143]).
  WOOD_MIN_SHARE: SB.LIMITS.WOOD_MIN_SHARE,
  WOOD_GAIN_SHARE: SB.LIMITS.WOOD_GAIN_SHARE,
};

/// The words the game shows when it may not take a piece down (shared.rs `Why::NotYourPiece`; the
/// judge test reads shared.rs and holds this to it).
const NOT_YOUR_PIECE_SENTENCE = "it was put up by someone else, and only this plot's holder can take it down";

/// The relay's permit-check line (plan section 3.5): the plot as the permit names it, and the
/// verdict, "ok" or "not_allowed/<why>".
const RELAY_PERMIT_CHECK_RE = /Game: permit for (\S+) checked \(([a-z_/]+)\)/;
/// Any line of that kind, read or not, for the key check.
const RELAY_PERMIT_ANY_RE = /Game: permit for .* checked/;

const num = (v) => (v === null || v === undefined || v === "" ? NaN : Number(v));
const fs2 = (x) => (Number.isFinite(x) ? x.toFixed(2) : "?");
const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(none)");
const dist = (a, b) => (Array.isArray(a) && Array.isArray(b) && a.length >= 3 && b.length >= 3 ? Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]) : Infinity);
const key12 = (k) => (typeof k === "string" ? k.slice(0, 12) : "?");

/** The `household` block of a probe (the recorder's one-frame answer), as the game wrote it, or
 *  null when this build reports none. What the rig keeps in the manifest. */
function householdOfProbe(probe) {
  const h = probe && probe[PROBE.block];
  return h && typeof h === "object" && !Array.isArray(h) ? h : null;
}

/** A state or verdict as one lowercase word: a string as it is, serde's tagged object by its tag. */
function word(v) {
  if (typeof v === "string") return v.toLowerCase();
  if (v && typeof v === "object" && !Array.isArray(v)) {
    const k = Object.keys(v)[0];
    return k ? k.toLowerCase() : null;
  }
  return null;
}

/**
 * The game's `household` block (`householdOfProbe`) in the judge's own names, the one reader of
 * `PROBE`: { me, me_did, server, my_plot, friends: [key], given: [{ to, plot, serial, expiry,
 * state }], held: [{ from, plot, serial, expiry, verdict }], usable: [plot], walkable: [plot], lit,
 * open_lights, mouths: { plot: [{ others_only, open }] } }, keys in lowercase. Null when there is no
 * block (then every check that reads it fails as not recorded). Pure.
 */
function readHousehold(raw) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const list = (v) => (Array.isArray(v) ? v : []);
  const str = (v) => (typeof v === "string" && v ? v : null);
  const low = (v) => (typeof v === "string" && v ? v.toLowerCase() : null);
  const pick = (row, k) => {
    if (!row || typeof row !== "object") return undefined;
    if (row[k] !== undefined) return row[k];
    return row.permit && typeof row.permit === "object" ? row.permit[k] : undefined;
  };
  const mouth = (row) => ({
    others_only: row && row[PROBE.mouth_others_only] === true ? true : row && row[PROBE.mouth_others_only] === false ? false : null,
    open: num(row && row[PROBE.mouth_open]),
  });
  const mouths = {};
  const m = raw[PROBE.mouths];
  if (Array.isArray(m)) {
    for (const row of m) {
      const p = str(row && row[PROBE.mouth_plot]);
      if (p) (mouths[p] = mouths[p] || []).push(mouth(row));
    }
  } else if (m && typeof m === "object") {
    for (const [p, rows] of Object.entries(m)) mouths[p] = (Array.isArray(rows) ? rows : [rows]).map(mouth);
  }
  return {
    me: low(raw[PROBE.me]),
    me_did: str(raw[PROBE.me_did]),
    server: str(raw[PROBE.server]),
    my_plot: str(raw[PROBE.my_plot]),
    friends: list(raw[PROBE.friends]).map((f) => low(typeof f === "string" ? f : f && f[PROBE.friend_key])).filter(Boolean),
    given: list(raw[PROBE.given]).map((g) => ({
      to: low(pick(g, PROBE.given_to)),
      plot: str(pick(g, PROBE.given_plot)),
      serial: str(pick(g, PROBE.given_serial)),
      expiry: num(pick(g, PROBE.given_expiry)),
      state: word(pick(g, PROBE.given_state)),
    })),
    held: list(raw[PROBE.held]).map((h) => ({
      from: low(pick(h, PROBE.held_from) ?? pick(h, "issuer")),
      plot: str(pick(h, PROBE.held_plot)),
      serial: str(pick(h, PROBE.held_serial)),
      expiry: num(pick(h, PROBE.held_expiry)),
      verdict: word(pick(h, PROBE.held_verdict)),
    })),
    usable: list(raw[PROBE.usable]).map(String),
    walkable: list(raw[PROBE.walkable]).map(String),
    lit: str(raw[PROBE.lit]),
    open_lights: num(raw[PROBE.open_lights]),
    mouths,
  };
}

/** The rows given to `key` in a read household. */
const givenTo = (h, key) => (h ? h.given.filter((g) => g.to === String(key || "").toLowerCase()) : []);
/** Those that may stand now. */
const standingTo = (h, key) => givenTo(h, key).filter((g) => WORDS.given_standing.includes(g.state));

/** The frame `id` of the run (from the game's door points), or null. */
function frameOf(run, id) {
  return ((run && run.frames) || []).find((f) => f.id === id) || null;
}
/** Is `p` inside frame `f`'s box across the floor? */
function inBox(f, p) {
  return !!f && Array.isArray(p) && p.length >= 3 && p[0] >= f.min[0] && p[0] <= f.max[0] && p[2] >= f.min[2] && p[2] <= f.max[2];
}

/** How far `p` stands past the shut wall of `door` (the zone-side mouth of the permitted plot's
 *  corridor), metres along the corridor, toward the plot: negative on the zone's side. NaN when
 *  either is unknown. `door`: { axis: "x"|"z", mouth_zone: [x,y,z], mouth_plot: [x,y,z] }. */
function pastWall(door, p) {
  if (!door || !Array.isArray(p) || !Array.isArray(door.mouth_zone) || !Array.isArray(door.mouth_plot)) return NaN;
  const i = door.axis === "z" ? 2 : 0;
  const toward = Math.sign(Number(door.mouth_plot[i]) - Number(door.mouth_zone[i]));
  return toward === 0 ? NaN : toward * (Number(p[i]) - Number(door.mouth_zone[i]));
}

/** The relay lines of a window that match `re`, as match arrays. */
function relayHits(lines, re) {
  return (Array.isArray(lines) ? lines : []).map((l) => String(l).match(re)).filter(Boolean);
}
/** Is `why` (a log line's group) the given code, reading "", "-" and none as no why? */
const whyIs = (got, want) => (got === "" || got === "-" || got === undefined ? null : got) === want;
/** Did a refusal record say `action` refused for `reason` and `why`? */
function refusedAs(refusal, action, reason, why) {
  return !!refusal && refusal.action === action && refusal.reason === reason && (refusal.why || null) === why;
}
const refusalText = (r) => (r ? `${r.action} refused: ${r.reason}${r.why ? ` (${r.why})` : ""}` : "no refusal");

/** What the player holds of `blueprint`'s materials, the backpack and home storage together, at
 *  three moments, as [{ item, n, before, after, later }]; null when any record is missing (the
 *  --build judge's count: a take-down's planks land in home storage when the backpack is full). */
function heldMoves(run, blueprint, pack, store) {
  const bp = run && run.blueprints && run.blueprints[blueprint];
  const all = [...pack, ...store];
  if (!bp || !Array.isArray(bp.materials) || !bp.materials.length || all.some((r) => !r || typeof r !== "object")) return null;
  return bp.materials.map(([item, n]) => {
    const p = pack.map((r) => num(r[item]));
    const s = store.map((r) => num(r[item]));
    return { item, n: Number(n), before: p[0] + s[0], after: p[1] + s[1], later: p[2] + s[2] };
  });
}

/** Is `sentence` among the notices recorded? Null when none were recorded. */
function noticed(notices, sentence) {
  if (!Array.isArray(notices)) return null;
  return notices.some((n) => String(n).includes(sentence));
}

/** Does `p` (a permit as Ada logged it) carry these facts? Returns the first fact it gets wrong,
 *  in words, or null. */
function permitFactsWrong(p, want) {
  if (!p || typeof p !== "object") return "no permit";
  for (const [k, v] of Object.entries(want)) {
    if (v === null || v === undefined) return `the run did not record the ${k} it should name`;
    if (String(p[k]).toLowerCase() !== String(v).toLowerCase()) return `its ${k} is ${JSON.stringify(String(p[k]).slice(0, 40))}, not ${JSON.stringify(String(v).slice(0, 40))}`;
  }
  return null;
}

/**
 * Judge one --household run.
 *   run       the manifest (see the top of this file and verify-copresence.js runHouseholdOnce)
 *   evidence  { recordings: { <file>: frames[] }, pictures: { <file>: { width, height, rgba } },
 *             relayLog: the relay's log text }: what the manifest names, read from its folder.
 * Returns { pass, checks: [{ id, ok, detail }] }. A record the run did not make FAILS its check;
 * nothing is ever passed unchecked.
 */
function judgeHousehold(run, evidence = {}) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const m = run || {};
  const s = m.steps_ok || {};
  const recs = evidence.recordings || {};
  const pics = evidence.pictures || {};
  const A = (m.walkers && m.walkers.ada) || {};
  const G = m.game || {};
  const H0 = readHousehold(m.household_joined);
  const me = H0 ? H0.me : null;
  const aKey = typeof A.key === "string" ? A.key.toLowerCase() : null;
  const aFrame = A.plot ? `plot:${A.plot}` : null;
  const gFrame = G.plot ? `plot:${G.plot}` : null;
  const serverDid = (m.relay && m.relay.server_did) || null;

  // ── 1-2. The relay, the game and Ada, and who holds what.
  add("relay_up", s.relay && s.relay.ok, s.relay ? s.relay.detail : "never started");
  add("game_in_world", s.joined && s.joined.ok, s.joined ? s.joined.detail : "never joined");
  add("walker_in_world", s.walker && s.walker.ok, s.walker ? s.walker.detail : "Ada never joined");
  {
    const ok = typeof A.plot === "string" && typeof G.plot === "string" && A.plot !== G.plot && !!H0 && H0.my_plot === G.plot && typeof me === "string" && /^[0-9a-f]{64,}$/.test(me);
    add(
      "plots_as_joined",
      ok,
      !H0
        ? "not recorded: the game's household report after it joined (a build without lane 2A's probe block?)"
        : `${A.name || "Ada"} holds ${A.plot || "no plot"}, the game ${G.plot || "no plot"} by its welcome; its household says my_plot ${H0.my_plot || "none"}, its key ${me ? `${key12(me)}...` : "not reported"}` +
            (ok ? "" : " (two plots of their own, and the household's plot the welcome's)"),
    );
  }
  const herBuilds = m.her_pieces && Array.isArray(m.her_pieces.builds) && m.her_pieces.builds.length ? m.her_pieces.builds : null;
  {
    const ok = !!herBuilds && herBuilds.length === 2 && herBuilds.every((b) => Number.isInteger(b.piece_id) && !b.refusal && b.cmd && b.cmd.frame === aFrame);
    add(
      "her_pieces_on_her_plot",
      ok,
      !herBuilds
        ? "not recorded: Ada's foundation and wall on her plot"
        : herBuilds.map((b) => `${b.cmd ? `${b.cmd.blueprint} on ${b.cmd.frame}` : "?"} -> ${Number.isInteger(b.piece_id) ? `piece ${b.piece_id}` : b.refusal ? refusalText(b.refusal) : b.error || "no answer"}`).join("; ") +
            (ok ? "" : ` (both kept, on ${aFrame || "her plot"})`),
    );
  }
  const wallId = herBuilds && herBuilds[1] && Number.isInteger(herBuilds[1].piece_id) ? herBuilds[1].piece_id : null;

  // ── 3. FRIENDS.
  {
    const f = m.friends || null;
    const H = f ? readHousehold(f.household) : null;
    const noticeOk = !!f && typeof f.notice === "string" && !!A.name && f.notice.includes(A.name) && /contact request/.test(f.notice);
    const passOk = !!f && !!me && f.pass_from === key12(me);
    const followOk = !!f && !!me && f.follow_from === key12(me);
    const listed = !!H && !!aKey && H.friends.includes(aKey);
    const ok = noticeOk && passOk && followOk && listed;
    add(
      "friends_made",
      ok,
      !f
        ? "not recorded: the contact request and its acceptance"
        : `the game's notice ${noticeOk ? `named her request: ${JSON.stringify(f.notice)}` : `did NOT name her request (notices seen ${JSON.stringify(f.notices_seen || [])})`}; ` +
            `Ada logged ${f.pass_from ? `a friend pass from ${f.pass_from}` : "NO friend pass"} and ${f.follow_from ? `a follow from ${f.follow_from}` : "NO follow"} (the game is ${me ? key12(me) : "?"}); ` +
            `the game's household ${!H ? "was not recorded" : listed ? "lists her as a friend" : `does NOT list her (friends ${JSON.stringify(H.friends.map(key12))})`}`,
    );
  }

  // ── 4. SHUT BEFORE A PERMIT.
  const closed = m.closed || null;
  const HC = closed ? readHousehold(closed.household) : null;
  {
    const track = closed && Array.isArray(closed.track) ? closed.track.filter((p) => Array.isArray(p)) : [];
    const end = closed && Array.isArray(closed.end) ? closed.end : null;
    const past = closed ? Math.max(...[...track, ...(end ? [end] : [])].map((p) => pastWall(closed.door, p))) : NaN;
    const endPast = end ? pastWall(closed.door, end) : NaN;
    const stoppedNear = Number.isFinite(endPast) && endPast <= LIMITS.PASS_TOL_M && -endPast <= LIMITS.STOP_NEAR_M;
    const neverPast = Number.isFinite(past) && past <= LIMITS.PASS_TOL_M;
    const mouths = HC && A.plot ? HC.mouths[A.plot] || [] : [];
    const shut = mouths.length > 0 && mouths.every((x) => x.others_only === true);
    const nothingOpen = !!HC && HC.walkable.length === 0 && HC.open_lights === 0 && HC.lit === null && !HC.usable.includes(A.plot);
    const ok = !!closed && closed.arrived === false && track.length > 0 && neverPast && stoppedNear && shut && nothingOpen;
    add(
      "closed_before_permit",
      ok,
      !closed
        ? "not recorded: the walk into her plot before any permit"
        : `walking ${fmt3(closed.from)} -> ${fmt3(closed.to)} through ${closed.door ? `${closed.door.plot}'s corridor` : "(no door recorded)"}: ${closed.arrived === false ? "it never arrived" : "it ARRIVED inside her plot"}; ` +
            `the farthest it got is ${fs2(past)} m past the ${closed.door ? closed.door.zone : "zone"}'s wall at the corridor (${track.length} position(s) read), and it stopped ${fs2(endPast)} m from it at ${fmt3(end)}` +
            `${neverPast ? "" : ": it WALKED THROUGH the shut wall (or the rig's walk_to does not collide: engine/move_check.rs walk_tick)"}${stoppedNear || !neverPast ? "" : `: it did not stop within ${LIMITS.STOP_NEAR_M} m of the wall`}; ` +
            `${HC ? `${A.plot}'s corridor doors ${mouths.length ? (shut ? "open for other players only" : `open for THIS player too (${JSON.stringify(mouths)})`) : "NOT reported"}; walkable ${JSON.stringify(HC.walkable)}, lights ${Number.isFinite(HC.open_lights) ? HC.open_lights : "?"}, lit ${JSON.stringify(HC.lit)}${nothingOpen ? "" : " (nothing may be open before a permit)"}` : "the game's household was not recorded"}`,
    );
  }

  // ── 5. THE GAME GIVES, THROUGH ITS PAGE.
  const give = m.give || null;
  const days = give ? num(give.days) : NaN;
  {
    const H = give ? readHousehold(give.household) : null;
    const rows = H && aKey ? givenTo(H, aKey) : [];
    const want = give ? num(give.epoch_ms) / 1000 + days * 86400 : NaN;
    const r = rows.length === 1 ? rows[0] : null;
    const off = r ? Math.abs(r.expiry - want) : NaN;
    const ok = !!r && r.plot === G.plot && r.state === WORDS.given_taken && off <= LIMITS.EXPIRY_TOL_S;
    add(
      "give_through_the_page",
      ok,
      !give
        ? "not recorded: the game's Give on its Household card"
        : !H
          ? "not recorded: the game's household after Give"
          : `the clicks: ${(give.clicks || []).map((c) => `${JSON.stringify(c.label)} ${c.ok ? "clicked" : `NOT clicked (${c.why || "not found"})`}`).join(", ") || "none"}; ` +
            `${rows.length} given row(s) to Ada${r ? `: plot ${r.plot}, ${r.state}, ending ${fs2(off / 60)} min from ${days} days after the click` : rows.length ? `: ${JSON.stringify(rows)}` : ""}` +
            (ok ? "" : ` (one row, plot ${G.plot || "?"}, ${WORDS.given_taken}, within ${LIMITS.EXPIRY_TOL_S} s of ${Number.isFinite(days) ? days : "?"} days)`),
    );
  }
  const given = give && give.received && give.received.permit ? give.received.permit : null;
  {
    const wrong = given ? permitFactsWrong(given, { plot: G.plot, grantee: A.did, server: serverDid, issuer: me }) : "no permit";
    const checksOut = !!give && !!give.checks_out && !!given && give.checks_out.serial === given.serial && give.checks_out.plot === given.plot && give.checks_out.from === key12(me);
    const ok = !!given && wrong === null && checksOut;
    add(
      "give_arrived",
      ok,
      !give
        ? "not recorded: what Ada received"
        : !given
          ? `Ada never logged a permit from the game${give.refused ? ` (she logged: permit refused: ${give.refused})` : ""}`
          : `Ada received a permit for ${given.plot}, serial ${String(given.serial).slice(0, 8)}..., ending ${given.expiry}${wrong ? `, but ${wrong}` : ", naming her did, this relay's did and the game as its giver"}; ` +
            `${checksOut ? "and it checks out in her own Dilithium check" : `it did NOT check out for her (${give.checks_out ? JSON.stringify(give.checks_out) : give.refused ? `permit refused: ${give.refused}` : "no checks-out line"})`}`,
    );
  }
  {
    // The narrowed key check (the lanes' correction to plan step 5): only the permit-check lines.
    const log = typeof evidence.relayLog === "string" ? evidence.relayLog : null;
    const lines = log === null ? [] : log.split(/\r?\n/).filter((l) => RELAY_PERMIT_ANY_RE.test(l));
    const keys = [me, aKey].filter((k) => typeof k === "string" && k.length >= 16);
    const needles = [...keys, ...keys.map((k) => k.slice(0, 16))];
    const bad = lines.filter((l) => needles.some((n) => l.toLowerCase().includes(n)) || /did:hum:/.test(l));
    const ok = log !== null && keys.length === 2 && lines.length > 0 && bad.length === 0;
    add(
      "relay_permit_lines_carry_no_key",
      ok,
      log === null
        ? "not recorded: the relay's log"
        : keys.length < 2
          ? "not recorded: the game's and Ada's keys, to look for"
          : !lines.length
            ? "the relay logged no permit check at all, so nothing was looked at (the game never asked about the permit it holds?)"
            : bad.length
              ? `${bad.length} of ${lines.length} permit-check line(s) name someone (a key, its first 16 characters, or a did:hum): ${bad[0].slice(0, 160)}`
              : `${lines.length} permit-check line(s), none holding either key, its first 16 characters or a did:hum`,
    );
  }

  // ── 6. HER PERMIT WORKS, AND ONLY FOR HER OWN PIECES.
  const hb = m.her_build || null;
  {
    const want = hb && hb.cmd ? SB.shipPoint(frameOf(m, hb.cmd.frame), hb.cmd.local) : null;
    const row = hb ? hb.game_row : null;
    const pos = row ? dist(row.pos, want) : Infinity;
    const withHers = !!hb && !!given && hb.permit_serial === given.serial;
    const ok = !!hb && Number.isInteger(hb.piece_id) && !hb.refusal && !!hb.cmd && hb.cmd.frame === gFrame && withHers && !!row && Number(row.piece_id) === hb.piece_id && row.mine === false && row.frame === gFrame && pos <= LIMITS.POSE_TOL_M;
    add(
      "give_lets_her_build",
      ok,
      !hb
        ? "not recorded: Ada's build on the game's plot with the permit"
        : !Number.isInteger(hb.piece_id)
          ? `Ada's build on ${hb.cmd ? hb.cmd.frame : "?"} with the permit was NOT kept: ${hb.refusal ? refusalText(hb.refusal) : hb.error || "no answer"}`
          : `Ada's piece ${hb.piece_id} on ${hb.cmd.frame}${withHers ? " with the permit the game gave her" : " NOT with the permit the game gave her"}; the game ${row ? `draws it on ${row.frame}, mine ${row.mine}, ${(pos * 100).toFixed(1)} cm from ${fmt3(want)}` : "NEVER drew it"}` +
            (ok ? "" : ` (on ${gFrame || "the game's plot"}, not the game's own, within ${LIMITS.POSE_TOL_M * 100} cm)`),
    );
  }
  {
    const r = m.her_pieces_only || null;
    const ht = r ? r.her_take_down : null;
    const ot = r ? r.our_take_down : null;
    const logged = ht ? relayHits(ht.relay_lines, SB.RELAY_TAKE_DOWN_REFUSED_RE).filter((h) => h[1] === "not_allowed" && whyIs(h[2], "not_your_piece")) : [];
    const gone = ht ? relayHits(ht.relay_lines, SB.RELAY_TOOK_DOWN_RE) : [];
    const theirsOk = !!ht && refusedAs(ht.refusal, "unbuild", "not_allowed", "not_your_piece") && logged.length === 1 && gone.length === 0 && ht.still_drawn === true;
    const seen = ot && ot.seen_by_walker ? ot.seen_by_walker : null;
    const gap = seen && ot ? (num(seen.epoch_ms) - num(ot.epoch_ms)) / 1000 : NaN;
    const oursOk = !!ot && !!seen && Number(seen.piece_id) === Number(ot.piece_id) && gap >= 0 && gap <= LIMITS.SEEN_WITHIN_S && ot.gone === true;
    add(
      "her_pieces_only",
      theirsOk && oursOk,
      !r
        ? "not recorded: who may take down what on the game's plot"
        : `Ada's take-down of the game's piece ${ht ? ht.piece_id : "?"} with her permit: ${ht ? refusalText(ht.refusal) : "not recorded"}; the relay logged ${logged.length} refusal(s) as not her piece${gone.length ? " and TOOK IT DOWN" : ""}; the game ${ht && ht.still_drawn === true ? "still draws it" : ht && ht.still_drawn === false ? "NO LONGER draws it" : "(not recorded)"}. ` +
            `The game's take-down of her piece ${ot ? ot.piece_id : "?"}: ${seen ? `Ada saw it come down ${fs2(gap)} s later` : "Ada NEVER saw it come down"}; the game ${ot && ot.gone === true ? "no longer has it" : ot && ot.gone === false ? "STILL HAS IT" : "(not recorded)"}` +
            (theirsOk && oursOk ? "" : ` (refused not_allowed / not_your_piece and still standing; hers gone, seen within ${LIMITS.SEEN_WITHIN_S} s)`),
    );
  }

  // ── 7. RENEW.
  const renew = m.renew || null;
  const renewed = renew && renew.received && renew.received.permit ? renew.received.permit : null;
  {
    const H = renew ? readHousehold(renew.household) : null;
    const stand = H && aKey ? standingTo(H, aKey) : [];
    const old = H && given ? H.given.find((g) => g.serial === given.serial) : null;
    const later = !!renewed && !!given && renewed.serial !== given.serial && num(renewed.expiry) > num(given.expiry) && renewed.plot === G.plot;
    const checksOut = !!renew && !!renewed && !!renew.checks_out && renew.checks_out.serial === renewed.serial;
    const one = stand.length === 1 && !!renewed && stand[0].serial === renewed.serial && stand[0].state === WORDS.given_taken;
    const oldBack = !old || old.state === WORDS.given_taken_back;
    const ok = later && checksOut && one && oldBack;
    add(
      "renew_one_press",
      ok,
      !renew
        ? "not recorded: Renew on the game's card"
        : `the click: ${(renew.clicks || []).map((c) => `${JSON.stringify(c.label)} ${c.ok ? "clicked" : "NOT clicked"}`).join(", ") || "none"}; ` +
            `${renewed ? `Ada received a permit for ${renewed.plot} ending ${renewed.expiry}${later ? `, later than ${given ? given.expiry : "?"}, with a new serial` : given ? `, NOT a later one with a new serial than ${given.expiry} / ${String(given.serial).slice(0, 8)}` : ""}${checksOut ? ", and it checks out" : ", which did NOT check out"}` : "Ada received NO second permit"}; ` +
            `${H ? `${stand.length} permit(s) to her may stand${one ? " (the new one, taken)" : `: ${JSON.stringify(stand)}`}; the first ${old ? `reads ${old.state}` : "is no longer listed"}${oldBack ? "" : " (amendment A1: once the new one was taken, the old one is taken back)"}` : "the game's household after Renew was not recorded"}`,
    );
  }

  // ── 8. THE GAME HOLDS ONE.
  const held = m.held || null;
  {
    const H = held ? readHousehold(held.household) : null;
    const sent = held && held.sent ? held.sent.permit : null;
    const row = H && aKey && sent ? H.held.find((h) => h.from === aKey && h.plot === A.plot && h.serial === sent.serial) : null;
    const line = held && held.check_line ? held.check_line : null;
    const lm = line ? String(line.line).match(RELAY_PERMIT_CHECK_RE) : null;
    const gap = line && held.sent ? (num(line.epoch_ms) - num(held.sent.put_ok_epoch_ms)) / 1000 : NaN;
    const lineOk = !!lm && lm[1] === A.plot && lm[2] === "ok" && gap >= -LIMITS.CHECK_EARLY_S && gap <= LIMITS.CHECK_WITHIN_S;
    const mouths = H && A.plot ? H.mouths[A.plot] || [] : [];
    const open = mouths.length > 0 && mouths.every((x) => x.others_only === false);
    const walk = !!H && H.walkable.length === 1 && H.walkable[0] === A.plot && H.usable.includes(A.plot);
    const ok = !!sent && sent.plot === A.plot && !!row && row.verdict === WORDS.verdict_works && lineOk && open && walk;
    add(
      "held_and_checked",
      ok,
      !held
        ? "not recorded: Ada's permit for the game"
        : !sent
          ? "Ada never sent the game a permit"
          : `${H ? (row ? `the game holds Ada's permit for ${row.plot}, the relay's verdict ${JSON.stringify(row.verdict)}` : `the game does NOT hold Ada's permit for ${A.plot} (held ${JSON.stringify(H.held)})`) : "the game's household was not recorded"}; ` +
            `${line ? `the relay logged "${String(line.line).replace(/^.*?Game: /, "Game: ")}" ${fs2(gap)} s after it stored her permit` : "the relay logged NO check of it"}${lineOk || !line ? "" : ` (for ${A.plot}, ok, within ${LIMITS.CHECK_WITHIN_S} s)`}; ` +
            `${H ? `walkable ${JSON.stringify(H.walkable)}, usable ${JSON.stringify(H.usable)}; ${A.plot}'s corridor doors ${mouths.length ? (open ? "open for this player" : `STILL for other players only (${JSON.stringify(mouths)})`) : "NOT reported"}` : ""}`,
    );
  }

  // ── 9. WALKS IN.
  const walkIn = m.walk_in || null;
  const herBox = frameOf(m, aFrame);
  {
    const target = walkIn ? walkIn.target : null;
    const seen = walkIn && Array.isArray(walkIn.seen_by_ada) ? walkIn.seen_by_ada : null;
    const seenIn = seen ? seen.filter((p) => inBox(herBox, p)) : [];
    const c0 = walkIn ? num(walkIn.corrections_before) : NaN;
    const c1 = walkIn ? num(walkIn.corrections_after) : NaN;
    const never = Number.isFinite(c0) && Number.isFinite(c1) && c0 === c1;
    const ok = !!walkIn && walkIn.arrived === true && inBox(herBox, target) && dist(walkIn.at, target) <= 0.05 && never && seenIn.length > 0;
    add(
      "walks_into_p1",
      ok,
      !walkIn
        ? "not recorded: the game's walk into her plot"
        : `the walk through ${A.plot || "her plot"}'s corridor to ${fmt3(target)} (${inBox(herBox, target) ? "inside her plot" : "NOT inside her plot"}) ${walkIn.arrived === true ? "arrived" : `NEVER arrived: it stopped at ${fmt3(walkIn.at)}`}; ` +
            `${never ? "never corrected" : `CORRECTED (${Number.isFinite(c0) ? c0 : "?"} -> ${Number.isFinite(c1) ? c1 : "?"})`}; ` +
            `${seen ? `Ada saw the game at ${seen.length} place(s) on its way, ${seenIn.length} inside her plot` : "what Ada saw was not recorded"}`,
    );
  }
  {
    const H = walkIn ? readHousehold(walkIn.household) : null;
    const ok = !!H && H.lit === A.plot && H.open_lights > 0;
    add(
      "lit_inside",
      ok,
      !H
        ? "not recorded: the game's household inside her plot"
        : `inside: lit ${JSON.stringify(H.lit)}, ${Number.isFinite(H.open_lights) ? H.open_lights : "?"} of her home's lights on (step 4 read ${HC && Number.isFinite(HC.open_lights) ? HC.open_lights : "?"})` +
            (ok ? "" : ` (lit ${A.plot}, and more than none)`),
    );
  }

  // ── 10. SEES WHAT IS THERE.
  {
    const pic = m.picture || null;
    const frames = pic && pic.samples && Array.isArray(recs[pic.samples]) ? recs[pic.samples] : null;
    const withImg = pic && pic.with ? pics[pic.with] : null;
    const without = pic && pic.without ? pics[pic.without] : null;
    const at = frames && wallId !== null ? SB.rowsOf(frames, wallId) : [];
    const near = at.length ? at.reduce((a, b) => (Math.abs(b.epoch - num(pic.with_epoch_ms)) < Math.abs(a.epoch - num(pic.with_epoch_ms)) ? b : a)) : null;
    const rect = near && Array.isArray(near.row.rect) ? near.row.rect.map(Number) : null;
    const size = withImg ? [withImg.width, withImg.height] : null;
    const inView = !!size && !!rect && rect.every(Number.isFinite) && rect[0] < rect[2] && rect[1] < rect[3] && rect[0] >= 0 && rect[1] >= 0 && rect[2] <= size[0] && rect[3] <= size[1];
    const box = inView ? SB.clampRect(rect, size[0], size[1]) : null;
    const a = box ? SB.woodIn(withImg, box) : null;
    const b = box && without && without.width === withImg.width && without.height === withImg.height ? SB.woodIn(without, box) : null;
    const share = a && a.area ? a.count / a.area : NaN;
    const gain = a && b && a.area ? (a.count - b.count) / a.area : NaN;
    // Both pictures of the world view: a page left drawn over the one without her wall would hide
    // the wall's place and pass for a gain.
    const worldView = !!pic && pic.page_with === "None" && pic.page_without === "None";
    const ok = !!pic && Number(pic.wall_piece_id) === wallId && inView && pic.wall_gone === true && worldView && share >= LIMITS.WOOD_MIN_SHARE && gain >= LIMITS.WOOD_GAIN_SHARE;
    add(
      "sees_whats_there",
      ok,
      !pic
        ? "not recorded: the pictures from inside her plot"
        : !withImg || !without
          ? `not recorded: the picture with her wall (${pic.with || "none"}) and the one after she took it down (${pic.without || "none"})`
          : !frames
            ? "not recorded: the game's recording at the picture"
            : !near
              ? `her wall (piece ${wallId === null ? "?" : wallId}) was never in the game's recording at the picture`
              : !inView
                ? `her wall's screen rect ${JSON.stringify(rect)} is NOT inside the ${size ? `${size[0]} x ${size[1]}` : "?"} view`
                : `her wall drawn over x ${rect[0].toFixed(0)}..${rect[2].toFixed(0)}, y ${rect[1].toFixed(0)}..${rect[3].toFixed(0)}: ${a.count} of ${a.area} px wood with it (${(share * 100).toFixed(0)}%, median ${JSON.stringify(a.median)}), ${b ? `${b.count} once she took it down (median ${JSON.stringify(b.median)}): a gain of ${(gain * 100).toFixed(0)}%` : "the picture without it is another size"}` +
                  `${pic.wall_gone === true ? "" : "; the game STILL had the wall when the picture without it was taken (no control)"}` +
                  `${worldView ? "" : `; a page was drawn over a picture (with her wall: ${JSON.stringify(pic.page_with ?? null)}, without: ${JSON.stringify(pic.page_without ?? null)}; both must be the world view, "None")`}` +
                  (ok ? "" : ` (at least ${LIMITS.WOOD_MIN_SHARE * 100}% with it and a gain of ${LIMITS.WOOD_GAIN_SHARE * 100}%)`),
    );
  }

  // ── 11. BUILDS THERE.
  const bp1 = m.build_p1 || null;
  {
    const place = bp1 ? bp1.place : null;
    const p = bp1 ? bp1.piece : null;
    const want = place ? SB.shipPoint(frameOf(m, place.frame), place.local) : null;
    const pos = p ? dist(p.pos, want) : Infinity;
    const kept = bp1 && p ? relayHits(bp1.relay_lines, SB.RELAY_BUILT_RE).filter((h) => Number(h[1]) === Number(p.piece_id) && h[3] === aFrame) : [];
    const seen = bp1 ? bp1.seen_by_walker : null;
    const gap = seen && place ? (num(seen.epoch_ms) - num(place.epoch_ms)) / 1000 : NaN;
    const seenOk = !!seen && !!place && !!p && Number(seen.piece_id) === Number(p.piece_id) && seen.frame === place.frame && dist(seen.local, place.local) <= LIMITS.POSE_TOL_M && Number(seen.turn) === Number(place.turns) && gap >= 0 && gap <= LIMITS.SEEN_WITHIN_S;
    const ok = !!place && place.frame === aFrame && !!p && p.mine === true && p.frame === aFrame && pos <= LIMITS.POSE_TOL_M && kept.length === 1 && seenOk;
    add(
      "builds_on_p1",
      ok,
      !place
        ? "not recorded: the game's place on her plot"
        : !p
          ? `the game's ${place.blueprint} on ${place.frame} never stood in its world${bp1.notices && bp1.notices.length ? ` (on screen: ${JSON.stringify(bp1.notices)})` : ""}`
          : `piece ${p.piece_id} on ${p.frame}, mine ${p.mine === true}, ${(pos * 100).toFixed(1)} cm from ${fmt3(want)}; the relay logged ${kept.length} build of it on ${aFrame}; ` +
            `${seen ? `Ada saw piece ${seen.piece_id} built on ${seen.frame} at ${fmt3(seen.local)} turn ${seen.turn}, ${fs2(gap)} s after the place` : "Ada NEVER saw it built"}` +
            (ok ? "" : ` (the game's own on ${aFrame}, built by the relay, seen at the same pose within ${LIMITS.SEEN_WITHIN_S} s)`),
    );
  }

  // ── 12. TAKES DOWN ONLY ITS OWN.
  {
    const r = m.take_down_p1 || null;
    const th = r ? r.theirs : null;
    const ou = r ? r.ours : null;
    const said = th ? noticed(th.notices, NOT_YOUR_PIECE_SENTENCE) : null;
    const sentLines = th && Array.isArray(th.relay_lines) ? relayHits(th.relay_lines, SB.RELAY_TOOK_DOWN_RE).length + relayHits(th.relay_lines, SB.RELAY_TAKE_DOWN_REFUSED_RE).length : NaN;
    const theirsOk = !!th && Number(th.piece_id) === wallId && said === true && sentLines === 0 && th.still_drawn === true;
    const seen = ou && ou.seen_by_walker ? ou.seen_by_walker : null;
    const gap = seen && ou ? (num(seen.epoch_ms) - num(ou.epoch_ms)) / 1000 : NaN;
    const moves = ou ? heldMoves(m, ou.blueprint, [ou.pack_before, ou.pack_after, ou.pack_later], [ou.store_before, ou.store_after, ou.store_later]) : null;
    const once = !!moves && moves.every((x) => x.after - x.before === x.n && x.later === x.after);
    const oursOk = !!ou && !!seen && Number(seen.piece_id) === Number(ou.piece_id) && gap >= 0 && gap <= LIMITS.SEEN_WITHIN_S && ou.gone === true && once;
    add(
      "takes_down_only_its_own",
      theirsOk && oursOk,
      !r
        ? "not recorded: the game's take-downs on her plot"
        : `her wall (piece ${th ? th.piece_id : "?"}): ${said === null ? "nothing on screen was recorded" : said ? "refused at the crosshair with the not-your-piece words" : `NOT refused at the crosshair (notices ${JSON.stringify((th && th.notices) || [])})`}; ` +
            `${Number.isFinite(sentLines) ? (sentLines ? `${sentLines} take-down line(s) from the relay in that window: it was SENT` : "nothing reached the relay") : "the relay's lines were not recorded"}; ` +
            `the game ${th && th.still_drawn === true ? "still draws it" : th && th.still_drawn === false ? "NO LONGER draws it" : "(not recorded)"}. ` +
            `Its own piece ${ou ? ou.piece_id : "?"}: ${seen ? `Ada saw it come down ${fs2(gap)} s later` : "Ada NEVER saw it come down"}; ${ou && ou.gone === true ? "gone" : "STILL THERE or not recorded"}; ` +
            `${moves ? moves.map((x) => `${x.item} (backpack + home storage) ${x.before} -> ${x.after} -> ${x.later} (gives back ${x.n})`).join("; ") + (once ? "" : ": NOT back exactly once") : "the backpack and home storage were not recorded"}`,
    );
  }

  // ── 13. TAKE BACK (amendment A1).
  {
    const t = m.take_back || null;
    const H = t ? readHousehold(t.household) : null;
    const serial = t ? t.serial : null;
    const row = H && serial ? H.given.find((g) => g.serial === serial) : null;
    const stand = H && aKey ? standingTo(H, aKey) : [];
    const b = t ? t.build : null;
    const logged = b ? relayHits(b.relay_lines, SB.RELAY_BUILD_REFUSED_RE).filter((h) => h[1] === "not_allowed" && whyIs(h[2], "permit_withdrawn") && h[3] === gFrame) : [];
    const kept = b ? relayHits(b.relay_lines, SB.RELAY_BUILT_RE) : [];
    const theLive = !!renewed && serial === renewed.serial;
    const ok = !!t && theLive && !!row && row.state === WORDS.given_taken_back && stand.length === 0 && !!b && b.permit_serial === serial && refusedAs(b.refusal, "build", "not_allowed", "permit_withdrawn") && logged.length === 1 && kept.length === 0;
    add(
      "take_back_closes_it",
      ok,
      !t
        ? "not recorded: Take back on the game's card"
        : `the clicks: ${(t.clicks || []).map((c) => `${JSON.stringify(c.label)} ${c.ok ? "clicked" : `NOT clicked (${c.why || "not found"})`}`).join(", ") || "none"}; ` +
            `the permit taken back ${theLive ? "is the one standing after Renew" : `is NOT the one standing after Renew (${String(serial).slice(0, 8)} for ${renewed ? String(renewed.serial).slice(0, 8) : "?"})`}; ` +
            `${H ? `its row ${row ? `reads ${row.state}` : "is not listed"}, ${stand.length} to Ada may still stand` : "the game's household after it was not recorded"}; ` +
            `${b ? `Ada's next build on ${b.cmd ? b.cmd.frame : "?"} with it: ${Number.isInteger(b.piece_id) ? `KEPT as piece ${b.piece_id}` : refusalText(b.refusal)}${b.permit_serial === serial ? "" : " (NOT with the taken-back permit)"}; the relay logged ${logged.length} refusal(s) as withdrawn${kept.length ? " and BUILT it" : ""}` : "Ada's build after it was not recorded"}` +
            (ok ? "" : " (her build refused not_allowed / permit_withdrawn, the row taken back, none standing)"),
    );
  }

  // ── 14. OVER THE RUN (as --build: this run makes no jump, so no correction may come).
  const relaySent = typeof evidence.relayLog === "string" && Array.isArray(m.walker_keys) ? relayCorrections(evidence.relayLog, m.walker_keys).total : null;
  const gameTotal = num(m.corrections_game);
  add(
    "moves_game_honest_never_corrected",
    gameTotal === 0,
    Number.isFinite(gameTotal) ? `the relay corrected the game ${gameTotal} time(s) over the run (it makes no jump: none may come)` : "not recorded: the game's corrections over the run",
  );
  add(
    "moves_relay_sent_what_the_game_took",
    Number.isFinite(gameTotal) && relaySent !== null && relaySent === gameTotal,
    relaySent === null ? "not recorded: the corrections the relay sent (its relay.log in the run folder)" : `the relay sent the game ${relaySent} correction(s) and the game applied ${Number.isFinite(gameTotal) ? gameTotal : "?"}`,
  );
  const wl = Array.isArray(m.walker_corrections) ? m.walker_corrections : null;
  add("moves_walkers_never_corrected", !!wl && wl.length === 0, !wl ? "not recorded: Ada's corrections" : wl.length ? `Ada was corrected: ${wl[0]}` : "Ada was never corrected");
  for (const c of judgeWalks(m.walks, m.turns).checks) checks.push(c);
  const panics = num(m.panics);
  add("no_panics", panics === 0, Number.isFinite(panics) ? `${panics} PANIC line(s) in run.log` : "not recorded: run.log's PANIC lines");
  add(
    "no_builtin_data",
    Array.isArray(m.builtin_data) && m.builtin_data.length === 0,
    !Array.isArray(m.builtin_data)
      ? "not recorded: which data files came from the copy built into the exe"
      : m.builtin_data.length
        ? `${m.builtin_data.length} line(s) in run.log/relay.log serving a built-in copy: ${String(m.builtin_data[0]).replace(/^.*?\[built-in data copy\]\s*/, "")}`
        : "every data file came from the tree's data/ (no built-in copy served), in the game and the relay",
  );
  return { pass: checks.every((c) => c.ok), checks };
}

module.exports = {
  PROBE,
  WORDS,
  LIMITS,
  NOT_YOUR_PIECE_SENTENCE,
  RELAY_PERMIT_CHECK_RE,
  RELAY_PERMIT_ANY_RE,
  householdOfProbe,
  readHousehold,
  givenTo,
  standingTo,
  pastWall,
  judgeHousehold,
};
