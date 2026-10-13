// THE STARTS JUDGE (ship homes increment 6, "the starts as data, the Cabin, the mess hall",
// 2026-10-12; docs/design/ship-homes-increment-6-plan.md section 6, `just verify-starts`): did a
// real game, a real relay and one scripted walker show that a player who chooses a Cabin on a
// server starts there FRESH (only their name and look come along), eats at the mess hall, and
// finds their own home and things untouched when they go back to it?
//
// Pure: `judgeStarts` reads one run's record (the manifest scripts/verify-copresence.js --starts
// writes) and the relay's log text, and returns checks, so it is tested with made-up runs
// (scripts/tests/starts-judge.test.js, in `just rig-tests`) and re-run on a saved run without
// booting anything (`node scripts/verify-copresence.js --dry-verdict <manifest.json>`).
//
// WHAT IT JUDGES AGAINST: the tree's own data, read when the run is made (`readTreeData`) and kept
// in the manifest's `data` block, so a saved run is judged against the data it ran on. Nothing
// about a start, a plot, the counter or a meal is written in this file: the Cabin's kit, its
// opening quest and every plot kind come from data/homes/starts.ron; the plots' boxes and the
// default plot from data/blueprints/ship_structure.ron; the serving counter from
// data/machines/ship.ron, the mess hall volume it stands in and the food store in front of it
// (data/food/ship_stores.ron); the water bottles from data/containers/fluids.ron; the meal's good
// from data/ship/fleet_ledger.ron; the opening's steps from data/quests/getting_started.ron; and a
// new character's credits from src/ecs/components.rs (`impl Default for Wallet`).
//
// THE RUN, in the plan's steps (verify-copresence.js runStartsOnce says how each record is made):
//   1   SETUP: a throwaway relay (loopback only, the tree's data/ mirrored), ONE game in the
//       .probe-rig/copresence sandbox, paired with its own home ("home:", so it boots there,
//       alone) and keeping its progress between worlds (fresh_world_each_launch false: otherwise
//       going home would build the default home again and nothing could be "untouched").
//   2   MARKED OFFLINE: the game's own home, a homestead, not in the shared world; the Dev
//       `stock` verb fills its backpack, recorded as OFFLINE_MARK (`offline_marked`).
//   3   THE WALKER (TestBotCabin) joins asking for a cabin and gets the first cabin plot
//       (`walker_cabin`); asked where it lives, the relay names its plot and how many homes of
//       each offered start are free (`walker_home_answer`).
//   4   THE GAME CHOOSES A CABIN on the server (`choose_start`), then pairs with it (`pairing`):
//       it arrives with exactly the Cabin's kit, no skills, a new character's credits and the
//       Cabin's opening, none of OFFLINE_MARK (`fresh_on_server`), wearing the same name and look
//       (`look_crossed`).
//   5   ITS HOME: the next cabin plot, kind cabin, the welcome staying; the home's box the ship
//       file's, the camera in it; the relay logged the choice and no key with it (`cabin_plot`).
//   6   TO THE MESS HALL: the game opens Inventory > Status (the opening's first step) and walks
//       through the doors to stand in front of the serving counter, every step passed on to the
//       walker, never corrected, the opening's walk step done (`walks_to_mess_hall`).
//   7   EATS AND DRINKS: a meal from the server, eaten, a line in the fleet ledger
//       (`meal_eaten`); a second refused for now (`one_meal_a_meal_time`); the empty bottles
//       filled (`bottles_filled`); the walker walks there too and takes a meal (`walker_ate`).
//   8   THE OPENING FOLLOWS: its walk and its meal step done (`opening_follows`).
//   9   HOME AGAIN: back on its own home, alone, a homestead on the default plot, OFFLINE_MARK in
//       the backpack (`offline_untouched`); the server's save kept on disk with the cabin's state
//       after the meal (`server_save_kept`).
//   10  BACK TO THE SERVER: the cabin's save comes back and the welcome stays on its plot
//       (`server_home_remembered`).
//   11  OVER THE RUN: no correction of the game or the walker (`no_correction`), no panic
//       (`panics`), no built-in data copy served (`no_builtin_data`).
//
// WHAT THE GAME REPORTS: the probe's `home_session` block (lane 2B, src/engine/ipc.rs
// `home_session_core`): slot, wanted, pairing, path, server, start, household, ship_place, home
// {kind, plot, away, box {min, max}}, backpack {item: count}, skills (a count), credits, quests
// {active [{id, step}], completed}, look {name, appearance, outfit}, vitals {satiation,
// hydration}, own_home, meals_offline, counter (the serving counter's last sentence),
// fleet_meals [{direction, quantity, value}], answer (the last `game_home`), own_save and
// server_save {path, exists, start, household, ship_place, ship_meals, backpack}, last_verb
// {verb, value, ok, said}.
//
// WHAT THE RELAY LOGS (never a key): "Game: home chosen (cabin)" or "Game: home chosen
// (refused/<why>)" (src/relay/handlers/home_plots.rs), and the corrections the --plots judge reads.

"use strict";

const path = require("path");
const { readShipPlots, relayCorrections } = require("./copresence-judge.js");
// The walker's own log lines, one copy each in second-player.js.
const { HOME_RE, MEAL_TAKEN_RE } = require("../second-player.js");

/** The start this run chooses on the server, and asks for in the walker's join. */
const CHOSEN_START = "cabin";
/** Eye height, metres: where the game's camera and the walker stand above the floor. */
const EYE_M = 1.7;
/** How far in front of the serving counter the game stands to use it, metres (plan step 6). */
const STAND_FRONT_M = 1.5;

/// THE GAME'S WORDS the judge reads, each the game's own (the judge test reads the source and holds
/// these to it): what the counter says when it asks the server for a meal (engine/mess_hall.rs
/// `Meal::Asked`), when the server's meal was eaten (engine/fleet.rs `meal_sentence`), when the
/// counter served the ship's stores on this computer instead (which must never happen in a server's
/// world: a meal not on the ledger), when it is not time yet, and when bottles were filled
/// (systems/ship_meals.rs `Filled::sentence`).
const SENTENCES = {
  asked: "Asking the ship's stores for a meal...",
  eaten: "You ate a meal from the ship's stores.",
  eaten_offline: "You ate a crew meal from the ship's stores.",
  not_yet_re: /^Your next meal is in \d+ game minutes\.$/,
  filled_re: /^Filled .+ from the ship's water\.$/,
};

/// The relay's line when a home is chosen (src/relay/handlers/home_plots.rs): the start chosen, or
/// "refused/<why>".
const RELAY_HOME_CHOSEN_RE = /Game: home chosen \(([^)]*)\)/;
/// Something in a log line that names a person: a run of 16 hex digits (a key, or the first 16 of
/// one, which is how the relay names players elsewhere) or a did:hum.
const NAMES_SOMEONE_RE = /[0-9a-f]{16}|did:hum:/i;

/// The limits, each with its reason.
const LIMITS = {
  /// How far from where it was sent the walk may end, metres (`walk_to` stops within 5 cm; a
  /// camera read a frame later may have settled a little).
  ARRIVE_TOL_M: 0.1,
  /// How near a planned step a position the relay passed on must be to count as that step, metres
  /// (the walker logs every 0.25 m of a move, so a step's last logged point is within that; the
  /// --plots judge uses the same 0.5 m for the meeting walk).
  STEP_SEEN_M: 0.5,
  /// How far a home's box may be from the ship file's plot box, metres (floats printed and read).
  BOX_TOL_M: 0.01,
  /// How far outside the plot's footprint the camera may stand and still be "in it", metres.
  IN_PLOT_TOL_M: 0.05,
};

// ── Reading the tree's data (pure: each reader takes a file's text) ─────────────────────────────

/** A RON file's text without its `//` comments (none of the files read here has "//" in a string). */
const uncomment = (text) => String(text || "").replace(/\/\/[^\n]*/g, "");

/** The parenthesised groups at the top level of the list that starts at `open` (the index of its
 *  "["): each group's text, "(" to ")". Strings are skipped, so a ")" in a sentence is not a close. */
function topGroups(text, open) {
  const out = [];
  let depth = 0;
  let start = -1;
  let inStr = false;
  for (let i = open + 1; i < text.length; i++) {
    const c = text[i];
    if (inStr) {
      if (c === "\\") i++;
      else if (c === '"') inStr = false;
      continue;
    }
    if (c === '"') inStr = true;
    else if (c === "(" || c === "[") {
      if (depth === 0 && c === "(") start = i;
      depth++;
    } else if (c === ")" || c === "]") {
      if (depth === 0) break; // the list's own "]"
      depth--;
      if (depth === 0 && c === ")" && start >= 0) {
        out.push(text.slice(start, i + 1));
        start = -1;
      }
    }
  }
  return out;
}

/**
 * The starts of data/homes/starts.ron: { default, order: [id...], starts: { id: { id, name,
 * offered, design, plot_kind, opening, households: [id...], kit: { item: count } } } }, or null
 * when the file names no default or no start.
 */
function readStarts(text) {
  const t = uncomment(text);
  const def = /\bdefault:\s*"([a-z0-9_-]+)"/.exec(t);
  const list = /\bstarts:\s*\[/.exec(t);
  if (!def || !list) return null;
  const starts = {};
  const order = [];
  for (const row of topGroups(t, list.index + list[0].length - 1)) {
    const id = /^\(\s*id:\s*"([^"]+)"/.exec(row);
    if (!id) continue;
    const str = (k) => {
      const m = new RegExp(`\\b${k}:\\s*"([^"]*)"`).exec(row);
      return m ? m[1] : null;
    };
    const opening = /\bopening:\s*(?:Some\(\s*"([^"]+)"\s*\)|None)/.exec(row);
    const kitBlock = /\bkit:\s*\[([\s\S]*?)\]/.exec(row);
    const kit = {};
    if (kitBlock) for (const m of kitBlock[1].matchAll(/\(\s*"([^"]+)"\s*,\s*(\d+)\s*\)/g)) kit[m[1]] = (kit[m[1]] || 0) + Number(m[2]);
    const hh = /\bhouseholds:\s*\[([\s\S]*?)\]/.exec(row);
    starts[id[1]] = {
      id: id[1],
      name: str("name"),
      offered: /\boffered:\s*true\b/.test(row),
      design: str("design"),
      plot_kind: str("plot_kind"),
      opening: opening ? opening[1] || null : null,
      households: hh ? [...hh[1].matchAll(/\bid:\s*"([^"]+)"/g)].map((m) => m[1]) : [],
      kit,
    };
    order.push(id[1]);
  }
  if (!order.length || !starts[def[1]]) return null;
  return { default: def[1], order, starts };
}

/** The ship file's default plot (`default_plot: Some("p1")`), or null. */
function readDefaultPlot(shipText) {
  const m = /\bdefault_plot:\s*Some\(\s*"([^"]+)"\s*\)/.exec(uncomment(shipText));
  return m ? m[1] : null;
}

const nums = (s) => String(s).split(",").map((v) => Number(v.trim()));

/** Where a zone of the ship file stands: its `origin`, or null. */
function zoneOrigin(shipText, zoneId) {
  const m = new RegExp(`\\bid:\\s*"${zoneId}",\\s*label:\\s*"[^"]*",\\s*purpose:\\s*"[^"]*",\\s*origin:\\s*\\(([^)]*)\\)`).exec(uncomment(shipText));
  return m ? nums(m[1]) : null;
}

/** A labelled volume of a zone's body (`id`, `type_id`, `origin`, `size`, zone-local metres), or null. */
function volumeOf(shipText, volumeId) {
  const m = new RegExp(`\\bid:\\s*"${volumeId}",\\s*type_id:\\s*"([^"]+)",\\s*origin:\\s*\\(([^)]*)\\),\\s*size:\\s*\\(([^)]*)\\)`).exec(uncomment(shipText));
  return m ? { type_id: m[1], origin: nums(m[2]), size: nums(m[3]) } : null;
}

/**
 * The mess hall's serving counter the run walks to: the one in the room of the FIRST food store
 * of data/food/ship_stores.ron (the Commons' mess hall), from data/machines/ship.ron (its row and
 * its catalog size) and the ship file (the zone and the volume it stands in). Returns
 *   { id, zone, room, centre [x,y,z] (ship metres, on the floor), size [x,y,z], rotation,
 *     front [x,0,z] (the way its front faces: toward the store, which stands in front of it),
 *     stand [x,EYE_M,z] (STAND_FRONT_M in front of its front face, at eye height),
 *     volume { type_id, min, max } (the room, in ship metres), store { id, place, at [x,0,z] } }
 * or { error } saying what was not found.
 */
function readCounter(shipText, machinesText, storesText) {
  const stores = uncomment(storesText);
  const store = /\(\s*id:\s*"([^"]+)",\s*label:\s*"[^"]*",\s*place:\s*"([^"]+)",\s*spot:\s*\(([^)]*)\)/.exec(stores);
  if (!store) return { error: "data/food/ship_stores.ron lists no store with a place and a spot" };
  const machines = uncomment(machinesText);
  const rowRe = /\(\s*id:\s*"([^"]+)",\s*machine:\s*"serving_counter",\s*room:\s*"([^"]+)",\s*offset:\s*\(([^)]*)\),\s*rotation:\s*([-\d.]+),\s*zone:\s*"([^"]+)"/g;
  const row = [...machines.matchAll(rowRe)].find((r) => r[2] === store[2]);
  if (!row) return { error: `data/machines/ship.ron has no serving_counter row in the room "${store[2]}" (the first food store's)` };
  const cat = /"serving_counter":\s*\(\s*[\s\S]*?\bsize:\s*\(([^)]*)\)/.exec(machines);
  if (!cat) return { error: "data/machines/ship.ron's catalog gives the serving_counter no size" };
  const zo = zoneOrigin(shipText, row[5]);
  const vol = volumeOf(shipText, row[2]);
  if (!zo || !vol) return { error: `the ship file has no zone "${row[5]}" with an origin or no volume "${row[2]}"` };
  const base = [0, 1, 2].map((k) => zo[k] + vol.origin[k]);
  const off = nums(row[3]);
  const centre = [0, 1, 2].map((k) => base[k] + off[k]);
  const spot = nums(store[3]);
  const at = [base[0] + spot[0], base[1], base[2] + spot[1]];
  // Its front faces the store (the file: the store "stands 1.1 m in front of it"): along x or z,
  // whichever the store is farther along.
  const dx = at[0] - centre[0];
  const dz = at[2] - centre[2];
  const front = Math.abs(dz) >= Math.abs(dx) ? [0, 0, Math.sign(dz) || 1] : [Math.sign(dx) || 1, 0, 0];
  const size = nums(cat[1]);
  // Turned a quarter, its footprint turns too.
  const quarter = Math.round(Number(row[4]) / 90) % 2 !== 0;
  const half = (front[0] ? (quarter ? size[2] : size[0]) : quarter ? size[0] : size[2]) / 2;
  const reach = half + STAND_FRONT_M;
  // Rounded to the millimetre, so the walk the rig asks for reads cleanly.
  const mm = (v) => Math.round(v * 1000) / 1000;
  const stand = [mm(centre[0] + front[0] * reach), EYE_M, mm(centre[2] + front[2] * reach)];
  return {
    id: row[1],
    zone: row[5],
    room: row[2],
    centre,
    size,
    rotation: Number(row[4]),
    front,
    stand,
    volume: { type_id: vol.type_id, min: base, max: [0, 1, 2].map((k) => base[k] + vol.size[k]) },
    store: { id: store[1], place: store[2], at },
  };
}

/** The water vessels a counter fills (data/containers/fluids.ron `fillables` of fluid "water"):
 *  [{ empty, full, litres }]. */
function readWaterFillables(text) {
  const re = /\(\s*empty:\s*"([^"]+)",\s*full:\s*"([^"]+)",\s*fluid:\s*"([^"]+)",\s*litres:\s*([\d.]+)\s*\)/g;
  return [...uncomment(text).matchAll(re)].filter((m) => m[3] === "water").map((m) => ({ empty: m[1], full: m[2], litres: Number(m[4]) }));
}

/** The good a meal from the ship's stores is (data/ship/fleet_ledger.ron, the "meal" kind's
 *  `OfGood`), or null. */
function readMealGood(text) {
  const m = /\(\s*id:\s*"meal",[^\n]*?\bprice:\s*OfGood\(\s*"([^"]+)"\s*\)/.exec(uncomment(text));
  return m ? m[1] : null;
}

/** A new character's credits (src/ecs/components.rs, `impl Default for Wallet`), or null. */
function readDefaultCredits(rsText) {
  const m = /impl Default for Wallet \{[\s\S]*?\bcredits:\s*([\d_]+)/.exec(String(rsText || ""));
  return m ? Number(m[1].replace(/_/g, "")) : null;
}

/**
 * The opening quest's steps (data/quests/getting_started.ron): { quest, objectives: [{ kind, arg }],
 * walk (the index of its Travel to the mess hall), eat (the index of the first Eat after it),
 * after_walk, after_meal (the step it stands at once each is done) } or { error }.
 */
function readOpening(questsText, questId) {
  const t = uncomment(questsText);
  const at = t.search(new RegExp(`\\bid:\\s*"${questId}"`));
  if (at < 0) return { error: `data/quests/getting_started.ron has no quest "${questId}"` };
  const rest = t.slice(at + 4);
  const next = rest.search(/\bid:\s*"/);
  const body = next < 0 ? rest : rest.slice(0, next);
  const objectives = [...body.matchAll(/\bobjective:\s*([A-Za-z]+)\(([^)]*)\)/g)].map((m) => ({ kind: m[1], arg: m[2].trim() }));
  const walk = objectives.findIndex((o) => o.kind === "Travel" && /destination:\s*"mess_hall"/.test(o.arg));
  const eat = walk < 0 ? -1 : objectives.findIndex((o, i) => i > walk && o.kind === "Eat");
  if (walk < 0 || eat < 0) return { error: `${questId} has no Travel to the mess hall followed by an Eat`, quest: questId, objectives };
  return { quest: questId, objectives, walk, eat, after_walk: walk + 1, after_meal: eat + 1 };
}

/** Every file `readTreeData` reads, relative to the repository root. */
const TREE_FILES = {
  starts: "data/homes/starts.ron",
  ship: "data/blueprints/ship_structure.ron",
  machines: "data/machines/ship.ron",
  stores: "data/food/ship_stores.ron",
  fluids: "data/containers/fluids.ron",
  ledger: "data/ship/fleet_ledger.ron",
  quests: "data/quests/getting_started.ron",
  wallet: "src/ecs/components.rs",
};

/**
 * The data a --starts run is judged against, read from the tree at `repo` (`readFile(path)` reads
 * one file; fs.readFileSync by default). What is not found is left null and named in `errors`, and
 * every check that needs it then fails as not recorded.
 */
function readTreeData(repo, readFile = (p) => require("fs").readFileSync(p, "utf8")) {
  const errors = [];
  const text = {};
  for (const [k, rel] of Object.entries(TREE_FILES)) {
    try {
      text[k] = readFile(path.join(repo, rel));
    } catch (e) {
      text[k] = "";
      errors.push(`${rel} could not be read (${e.message})`);
    }
  }
  const starts = readStarts(text.starts);
  if (!starts) errors.push(`${TREE_FILES.starts} names no default start or no starts`);
  const chosen = starts && starts.starts[CHOSEN_START];
  if (starts && (!chosen || !chosen.offered)) errors.push(`${TREE_FILES.starts} offers no "${CHOSEN_START}" start`);
  const plots = readShipPlots(text.ship);
  if (!plots.length) errors.push(`${TREE_FILES.ship} has no plots`);
  const defaultPlot = readDefaultPlot(text.ship);
  if (!defaultPlot) errors.push(`${TREE_FILES.ship} names no default plot`);
  const counter = readCounter(text.ship, text.machines, text.stores);
  if (counter.error) errors.push(counter.error);
  const fillables = readWaterFillables(text.fluids);
  if (!fillables.length) errors.push(`${TREE_FILES.fluids} lists no water fillables`);
  const mealGood = readMealGood(text.ledger);
  if (!mealGood) errors.push(`${TREE_FILES.ledger} prices no "meal" kind as a good`);
  const credits = readDefaultCredits(text.wallet);
  if (!Number.isFinite(credits)) errors.push(`${TREE_FILES.wallet} has no Wallet default credits`);
  const opening = chosen && chosen.opening ? readOpening(text.quests, chosen.opening) : { error: `the "${CHOSEN_START}" start names no opening` };
  if (opening.error) errors.push(opening.error);
  return {
    default_start: starts ? starts.default : null,
    chosen: CHOSEN_START,
    start_order: starts ? starts.order : [],
    starts: starts ? starts.starts : {},
    plots,
    default_plot: defaultPlot,
    counter: counter.error ? null : counter,
    fillables,
    meal_good: mealGood,
    default_credits: Number.isFinite(credits) ? credits : null,
    opening: opening.error ? null : opening,
    errors,
  };
}

// ── Small helpers ───────────────────────────────────────────────────────────────────────────────

const num = (x) => (x === null || x === undefined || x === "" ? NaN : Number(x));
const isBag = (b) => !!b && typeof b === "object" && !Array.isArray(b);
const countIn = (b, id) => (isBag(b) && Number.isFinite(Number(b[id])) ? Number(b[id]) : 0);
/** Two backpacks ({ item: count }) hold the same, an item at 0 the same as one not listed. */
function bagEqual(a, b) {
  if (!isBag(a) || !isBag(b)) return false;
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) if (countIn(a, k) !== countIn(b, k)) return false;
  return true;
}
/** A backpack in words, items sorted: "ors_sachet_0 2, water_purified_0 2". */
const bagText = (b, max = 8) => {
  if (!isBag(b)) return "(not recorded)";
  const e = Object.entries(b).filter(([, n]) => Number(n) !== 0).sort(([x], [y]) => (x < y ? -1 : 1));
  if (!e.length) return "nothing";
  return e.slice(0, max).map(([k, n]) => `${k} ${n}`).join(", ") + (e.length > max ? `, and ${e.length - max} more` : "");
};
/** What `b` holds beyond `base` (items, or counts above base's), as a bag. */
function beyond(b, base) {
  const out = {};
  if (!isBag(b)) return out;
  for (const k of Object.keys(b)) if (countIn(b, k) > countIn(base, k)) out[k] = countIn(b, k) - countIn(base, k);
  return out;
}
const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(none)");
const d3 = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(...[0, 1, 2].map((k) => Number(a[k]) - Number(b[k]))) : Infinity);
const word = (v) => (v === undefined || v === null ? "(not recorded)" : JSON.stringify(v));
/** The `home_session` a record carries. */
const sessionOf = (rec) => (rec && rec.session && typeof rec.session === "object" ? rec.session : null);
/** The step an active quest stands at in a session, or null when it is not active. */
function stepOf(s, questId) {
  const active = s && s.quests && Array.isArray(s.quests.active) ? s.quests.active : null;
  const q = active && active.find((x) => x && x.id === questId);
  return q ? num(q.step) : null;
}
const activeIds = (s) => (s && s.quests && Array.isArray(s.quests.active) ? s.quests.active.map((q) => q && q.id) : null);
const completedOf = (s) => (s && s.quests && Array.isArray(s.quests.completed) ? s.quests.completed : null);
/** The meals the fleet ledger counts as used (its "meal" kind's used lines), or NaN unrecorded. */
function usedMeals(fleetMeals) {
  if (!Array.isArray(fleetMeals)) return NaN;
  return fleetMeals.filter((k) => k && k.direction === "used").reduce((a, k) => a + Number(k.quantity || 0), 0);
}
/** The plot box of `id` in the data's plots: { min, max }, or null. */
function plotBox(data, id) {
  const p = data && Array.isArray(data.plots) ? data.plots.find((x) => x.id === id) : null;
  return p ? { min: p.origin.slice(0, 3), max: [0, 1, 2].map((k) => p.origin[k] + p.size[k]), kind: p.kind } : null;
}
/** The plot ids of `kind`, in the ship file's order (the order the relay hands them out). */
const plotsOfKind = (data, kind) => (data && Array.isArray(data.plots) ? data.plots.filter((p) => p.kind === kind).map((p) => p.id) : []);
const boxNear = (a, b, tol) => !!a && !!b && [0, 1, 2].every((k) => Math.abs(Number(a.min[k]) - b.min[k]) <= tol && Math.abs(Number(a.max[k]) - b.max[k]) <= tol);
const inBox = (p, box, tol = 0) => Array.isArray(p) && !!box && [0, 1, 2].every((k) => Number(p[k]) >= box.min[k] - tol && Number(p[k]) <= box.max[k] + tol);

/** The free list of a walker's "home: ...; free cabin 15, homestead 12" line, as { id: n }. */
function freeOf(list) {
  const out = {};
  for (const part of String(list || "").split(",").map((s) => s.trim()).filter(Boolean)) {
    const m = /^([a-z0-9_-]+) (\d+)$/.exec(part);
    if (m) out[m[1]] = Number(m[2]);
  }
  return out;
}

// ── The judge ───────────────────────────────────────────────────────────────────────────────────

/**
 * Judge one --starts run.
 *   run       the manifest (see the top of this file and verify-copresence.js runStartsOnce)
 *   evidence  { relayLog: the relay's log text }
 * Returns { pass, checks: [{ id, ok, detail }] }. A record the run did not make FAILS its check;
 * nothing is ever passed unchecked.
 */
function judgeStarts(run, evidence = {}) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const m = run || {};
  const data = m.data && typeof m.data === "object" ? m.data : null;
  const starts = data && data.starts ? data.starts : {};
  const def = data ? starts[data.default_start] || null : null;
  const cab = data ? starts[data.chosen] || null : null;
  const cabinPlots = cab ? plotsOfKind(data, cab.plot_kind) : [];
  // The walker joins first and is given the first cabin plot; the game, choosing next, the second.
  const walkerPlotWant = cabinPlots[0] || null;
  const gamePlotWant = cabinPlots[1] || null;
  const gameBox = gamePlotWant ? plotBox(data, gamePlotWant) : null;
  const noData = (what) => `not recorded: the tree's data (${what}) this run was judged against`;
  const relayLog = typeof evidence.relayLog === "string" ? evidence.relayLog : null;

  const off = m.offline || null;
  const offS = sessionOf(off);
  const mark = off && isBag(off.mark) ? off.mark : null;

  // ── 2. MARKED OFFLINE.
  {
    const stocked = mark && off ? Object.keys(beyond(mark, off.before)) : [];
    const notKit = mark && cab ? Object.keys(beyond(mark, cab.kit)) : [];
    const ok = !!offS && !!def && offS.slot === "offline" && offS.start === def.id && off.joined === false && !!mark && stocked.length > 0 && notKit.length > 0;
    add(
      "offline_marked",
      ok,
      !off || !offS
        ? "not recorded: the game's home session in its own home before the stock"
        : !def
          ? noData("the default start")
          : `the live save ${word(offS.slot)}, a ${word(offS.start)} (the default start is ${def.id}); ${off.joined === false ? "not in the shared world" : off.joined === true ? "IN the shared world" : "joined not recorded"}; ` +
              (mark ? `OFFLINE_MARK ${bagText(mark, 4)} (the stock added ${stocked.length} item(s), ${notKit.length} not in the ${cab ? cab.id : "?"} kit)` : "OFFLINE_MARK not recorded (the stock never changed the backpack?)") +
              (ok ? "" : " (want: your own home, the default start, alone, and a stocked backpack holding things the Cabin's kit does not)"),
    );
  }

  // ── 3. THE WALKER.
  const w = m.walker || null;
  const hp = w && w.home_plot && typeof w.home_plot === "object" ? w.home_plot : null;
  {
    const ok = !!hp && !!cab && !!walkerPlotWant && hp.id === walkerPlotWant && hp.kind === cab.plot_kind && hp.start === cab.id;
    add(
      "walker_cabin",
      ok,
      !w
        ? "not recorded: the walker's join"
        : !cab
          ? noData(`the ${CHOSEN_START} start and its plots`)
          : !hp
            ? `the walker's welcome named no plot (${word(w.home_plot)}): no ${cab.id} for a join asking for one`
            : `the walker, asking for a ${cab.id}, was given ${hp.id} (kind ${word(hp.kind)}, start ${word(hp.start)}); the first ${cab.plot_kind} plot is ${walkerPlotWant || "(none in the ship file)"}`,
    );
  }
  {
    const line = w && typeof w.home_line === "string" ? w.home_line : null;
    const g = line ? line.match(HOME_RE) : null;
    // Every offered start, with its plots less the one the walker holds (the game has none yet).
    const want = {};
    if (data) {
      for (const id of data.start_order || []) {
        const s = starts[id];
        if (!s || !s.offered) continue;
        want[id] = plotsOfKind(data, s.plot_kind).filter((p) => !hp || p !== hp.id).length;
      }
    }
    const got = g ? freeOf(g[3]) : null;
    const ok = !!g && !!hp && !!cab && g[1] === hp.id && g[2] === cab.plot_kind && Object.keys(want).length > 0 && JSON.stringify(Object.entries(got).sort()) === JSON.stringify(Object.entries(want).sort());
    add(
      "walker_home_answer",
      ok,
      !line
        ? "not recorded: the walker's answer to `home` (game_home_request)"
        : !g
          ? `the walker's answer did not read as a home line: ${line.slice(0, 120)}`
          : `the relay told the walker "${line.slice(line.indexOf("home:"))}"; want ${hp ? hp.id : "?"} ${cab ? cab.plot_kind : "?"} and free ${Object.entries(want).map(([k, n]) => `${k} ${n}`).join(", ") || "(no offered start in the data)"}`,
    );
  }

  // ── 4. FRESH ON THE SERVER, and the look.
  const srv = m.server || null;
  const sS = sessionOf(srv);
  {
    const extra = sS && cab ? beyond(sS.backpack, cab.kit) : {};
    const fromMark = mark ? Object.keys(extra).filter((k) => countIn(mark, k) > 0) : [];
    const ids = activeIds(sS);
    const done = completedOf(sS);
    const credits = data ? data.default_credits : null;
    const questsOk = !!cab && !!cab.opening && Array.isArray(ids) && ids.length === 1 && ids[0] === cab.opening && Array.isArray(done) && done.length === 0;
    const ok = !!sS && !!cab && sS.slot === "server" && sS.start === cab.id && bagEqual(sS.backpack, cab.kit) && num(sS.skills) === 0 && Number.isFinite(credits) && num(sS.credits) === credits && questsOk;
    add(
      "fresh_on_server",
      ok,
      !sS
        ? "not recorded: the game's home session on the server"
        : !cab || !Number.isFinite(credits)
          ? noData(`the ${CHOSEN_START} start's kit and a new character's credits`)
          : `the live save ${word(sS.slot)}, a ${word(sS.start)}; backpack ${bagText(sS.backpack)} (the ${cab.id} kit is ${bagText(cab.kit)}` +
              `${Object.keys(extra).length ? `; MORE: ${bagText(extra)}${fromMark.length ? `, of which ${fromMark.length} from OFFLINE_MARK` : ""}` : ""}); ` +
              `skills ${word(sS.skills)}, credits ${word(sS.credits)} (a new character's ${credits}); active quests ${word(ids)}, completed ${word(done)} (want only ${cab.opening || "(none)"})`,
    );
  }
  {
    // The look right before the swap (`offline.look`), against the look on the server.
    const a = off && off.look && typeof off.look === "object" ? off.look : null;
    const b = sS && sS.look;
    const ok = !!a && !!b && typeof a.name === "string" && JSON.stringify(a) === JSON.stringify(b);
    add(
      "look_crossed",
      ok,
      !a || !b
        ? `not recorded: the look ${!a ? "in the game's own home right before the swap" : "on the server"}`
        : ok
          ? `the same name (${JSON.stringify(a.name)}), appearance and outfit at home and on the server`
          : `at home ${JSON.stringify(a).slice(0, 120)}; on the server ${JSON.stringify(b).slice(0, 120)}: the look did not cross whole`,
    );
  }

  // ── 5. ITS HOME: the next cabin plot, built before the join, the welcome staying.
  {
    const home = sS && sS.home && typeof sS.home === "object" ? sS.home : null;
    const pp = srv && srv.home_plot && typeof srv.home_plot === "object" ? srv.home_plot : null;
    const box = home && home.box && Array.isArray(home.box.min) && Array.isArray(home.box.max) ? home.box : null;
    const cam = srv ? srv.camera : null;
    const chosenLines = relayLog === null ? null : relayLog.split(/\r?\n/).filter((l) => RELAY_HOME_CHOSEN_RE.test(l));
    const named = (chosenLines || []).filter((l) => NAMES_SOMEONE_RE.test(l.slice(l.indexOf("Game: home chosen"))));
    const asCabin = (chosenLines || []).filter((l) => l.match(RELAY_HOME_CHOSEN_RE)[1] === (cab ? cab.id : "?"));
    const ok =
      !!home &&
      !!cab &&
      !!gameBox &&
      home.plot === gamePlotWant &&
      home.kind === cab.plot_kind &&
      home.away === false &&
      !!pp &&
      pp.id === gamePlotWant &&
      pp.kind === cab.plot_kind &&
      srv.last_welcome === "stay" &&
      boxNear(box, gameBox, LIMITS.BOX_TOL_M) &&
      inBox(cam, gameBox, LIMITS.IN_PLOT_TOL_M) &&
      !!chosenLines &&
      asCabin.length === 1 &&
      named.length === 0;
    add(
      "cabin_plot",
      ok,
      !home || !srv
        ? "not recorded: the home the game built on the server"
        : !gameBox
          ? noData(`a second ${cab ? cab.plot_kind : CHOSEN_START} plot in the ship file`)
          : `its home: a ${word(home.kind)} on ${word(home.plot)}${home.away ? ", PUT AWAY" : ""}, the welcome ${word(srv.last_welcome)} on ${pp ? `${pp.id} (${pp.kind})` : "(no plot)"}; want ${gamePlotWant} (${cab.plot_kind}), stay; ` +
              `its box ${box ? `${fmt3(box.min)}..${fmt3(box.max)}` : "(none)"} against the ship file's ${fmt3(gameBox.min)}..${fmt3(gameBox.max)}; the camera at ${fmt3(cam)} ${inBox(cam, gameBox, LIMITS.IN_PLOT_TOL_M) ? "inside it" : "NOT inside it"}; ` +
              (chosenLines === null
                ? "the relay's log was not kept"
                : `the relay logged ${asCabin.length} "home chosen (${cab.id})" line(s) of ${chosenLines.length} home-chosen line(s)${named.length ? `, ${named.length} NAMING SOMEONE: ${named[0].slice(named[0].indexOf("Game:"), named[0].indexOf("Game:") + 120)}` : ", none naming anyone"}`),
    );
  }

  // ── 6. TO THE MESS HALL.
  {
    const wk = m.walk || null;
    const counter = data ? data.counter : null;
    const opening = data ? data.opening : null;
    const steps = wk && Array.isArray(wk.steps) ? wk.steps : [];
    const relayed = wk && Array.isArray(wk.relayed) ? wk.relayed : null;
    let from = 0;
    const missed = [];
    for (const s of steps) {
      const at = relayed ? relayed.findIndex((r, i) => i >= from && d3(r, s) <= LIMITS.STEP_SEEN_M) : -1;
      if (at < 0) missed.push(s);
      else from = at;
    }
    const before = wk ? num(wk.corrections_before) : NaN;
    const after = wk ? num(wk.corrections_after) : NaN;
    const there = !!wk && !!counter && d3(wk.at, counter.stand) <= LIMITS.ARRIVE_TOL_M && inBox(wk.at, { min: [counter.volume.min[0], -Infinity, counter.volume.min[2]], max: [counter.volume.max[0], Infinity, counter.volume.max[2]] });
    const questStep = wk ? num(wk.quest_step) : NaN;
    const ok =
      !!wk &&
      !!counter &&
      !!opening &&
      wk.arrived === true &&
      there &&
      steps.length > 0 &&
      !!relayed &&
      missed.length === 0 &&
      Number.isFinite(before) &&
      after === before &&
      questStep === opening.after_walk;
    add(
      "walks_to_mess_hall",
      ok,
      !wk
        ? "not recorded: the game's walk to the mess hall"
        : !counter || !opening
          ? noData("the serving counter and the opening's steps")
          : `the walk through ${(wk.doors || []).join(", ") || "(no doors)"} in ${steps.length} steps ${wk.arrived === true ? "arrived" : "did NOT arrive"}, at ${fmt3(wk.at)} (${STAND_FRONT_M} m in front of the counter is ${fmt3(counter.stand)}${there ? "" : ": NOT there"}); ` +
              `${!relayed ? "what the relay passed on was not recorded" : missed.length ? `${missed.length} of ${steps.length} steps never passed on to the walker in order (first ${fmt3(missed[0])})` : `every step passed on to the walker (${relayed.length} position(s))`}; ` +
              `corrections ${Number.isFinite(before) ? before : "?"} -> ${Number.isFinite(after) ? after : "?"}; the opening ${opening.quest} at step index ${Number.isFinite(questStep) ? questStep : "(not active)"} ` +
              `(counted from 0; want ${opening.after_walk}: its first ${opening.after_walk} steps done, the last the walk to the mess hall)` +
              (wk.vitals_step !== undefined ? `; after Inventory > Status it stood at index ${word(wk.vitals_step)}` : ""),
    );
  }

  // ── 7. EATS AND DRINKS.
  const meal = m.meal || null;
  {
    const good = data ? data.meal_good : null;
    const bA = meal && meal.before ? meal.before : null;
    const aA = meal && meal.after ? meal.after : null;
    const verb = meal ? meal.verb : null;
    const used = aA ? usedMeals(aA.fleet_meals) : NaN;
    const fedUp = !!bA && !!aA && bA.vitals && aA.vitals && num(aA.vitals.satiation) > num(bA.vitals.satiation);
    const ok =
      !!meal &&
      !!good &&
      !!verb &&
      verb.verb === "meal" &&
      verb.said === SENTENCES.asked &&
      !!aA &&
      aA.counter === SENTENCES.eaten &&
      isBag(aA.backpack) &&
      countIn(aA.backpack, good) === 0 &&
      fedUp &&
      used === 1;
    add(
      "meal_eaten",
      ok,
      !meal
        ? "not recorded: Take a meal at the counter"
        : !good
          ? noData("the meal's good")
          : `the counter said ${word(verb && verb.said)} (want ${JSON.stringify(SENTENCES.asked)}: the server asked), then ${word(aA && aA.counter)} (want ${JSON.stringify(SENTENCES.eaten)}` +
              `${aA && aA.counter === SENTENCES.eaten_offline ? ": THE SHIP'S STORES ON THIS COMPUTER SERVED IT IN A SERVER'S WORLD, a meal not on the ledger" : ""}); ` +
              `${good} in the backpack after: ${aA && isBag(aA.backpack) ? countIn(aA.backpack, good) : "(not recorded)"}; food ${bA && bA.vitals ? num(bA.vitals.satiation).toFixed(1) : "?"} -> ${aA && aA.vitals ? num(aA.vitals.satiation).toFixed(1) : "?"}; ` +
              `the fleet ledger's meals used: ${Number.isFinite(used) ? used : "(no ledger)"} (want 1)`,
    );
  }
  {
    const a = m.meal_again || null;
    const good = data ? data.meal_good : null;
    const used = a ? usedMeals(a.fleet_meals) : NaN;
    const ok = !!a && !!good && typeof a.counter === "string" && SENTENCES.not_yet_re.test(a.counter) && isBag(a.backpack) && countIn(a.backpack, good) === 0 && used === 1;
    add(
      "one_meal_a_meal_time",
      ok,
      !a
        ? "not recorded: a second Take a meal"
        : `asked again at once, the counter said ${word(a.counter)} (want "Your next meal is in N game minutes."); ${good || "the meal good"} in the backpack: ${isBag(a.backpack) ? countIn(a.backpack, good) : "(not recorded)"}; the ledger's meals used ${Number.isFinite(used) ? used : "(no ledger)"} (want still 1)`,
    );
  }
  {
    const wv = m.water || null;
    const fills = data && Array.isArray(data.fillables) ? data.fillables : [];
    const kitEmpties = cab ? fills.reduce((a, f) => a + countIn(cab.kit, f.empty), 0) : NaN;
    const b = wv && wv.before ? wv.before.backpack : null;
    const af = wv && wv.after ? wv.after.backpack : null;
    const emptiesBefore = fills.reduce((a, f) => a + countIn(b, f.empty), 0);
    const emptiesAfter = fills.reduce((a, f) => a + countIn(af, f.empty), 0);
    const fullGain = fills.reduce((a, f) => a + countIn(af, f.full) - countIn(b, f.full), 0);
    const said = wv && wv.verb ? wv.verb.said : null;
    const ok =
      !!wv &&
      fills.length > 0 &&
      kitEmpties > 0 &&
      isBag(b) &&
      isBag(af) &&
      wv.verb.verb === "water" &&
      typeof said === "string" &&
      SENTENCES.filled_re.test(said) &&
      emptiesBefore === kitEmpties &&
      emptiesAfter === 0 &&
      fullGain === kitEmpties;
    add(
      "bottles_filled",
      ok,
      !wv || !isBag(b) || !isBag(af)
        ? "not recorded: Fill your bottles, with the backpack before and after"
        : !fills.length || !(kitEmpties > 0)
          ? noData("the water vessels and the kit's empty ones")
          : `the counter said ${word(said)}; empty vessels ${emptiesBefore} -> ${emptiesAfter}, full ones +${fullGain} (the ${cab.id} kit carries ${kitEmpties} empty; want all of them full)`,
    );
  }
  {
    const wm = m.walker_meal || null;
    const good = data ? data.meal_good : null;
    const line = wm && typeof wm.line === "string" ? wm.line : null;
    const g = line ? line.match(MEAL_TAKEN_RE) : null;
    const ok = !!g && !!good && g[1] === good;
    add(
      "walker_ate",
      ok,
      !wm
        ? "not recorded: the walker's meal"
        : !good
          ? noData("the meal's good")
          : `the walker ${wm.arrived === true ? "reached" : wm.arrived === false ? "did NOT reach" : "(arrival not recorded)"} its place at the counter and logged ${line ? JSON.stringify(line.slice(line.search(/meal |command refused/) >= 0 ? line.search(/meal |command refused/) : 0)) : "nothing for take-meal"} (want "meal taken: ${good}")`,
    );
  }

  // ── 8. THE OPENING FOLLOWS.
  {
    const o = m.opening || null;
    const opening = data ? data.opening : null;
    const s = sessionOf(o);
    const step = opening ? stepOf(s, opening.quest) : null;
    const done = completedOf(s);
    const ok = !!s && !!opening && step === opening.after_meal && Array.isArray(done) && !done.includes(opening.quest);
    add(
      "opening_follows",
      ok,
      !s
        ? "not recorded: the quests after the meal"
        : !opening
          ? noData("the opening's steps")
          : `${opening.quest} stands at step index ${step === null ? "(not active)" : step} after the meal (counted from 0; want ${opening.after_meal}: its walk, step ${opening.walk + 1}, and its meal, step ${opening.eat + 1} counting from 1, done)` +
              (Array.isArray(done) && done.includes(opening.quest) ? "; it is COMPLETED (the rig never visits the rest of it)" : ""),
    );
  }

  // ── 9. HOME AGAIN.
  const ha = m.home_again || null;
  const hS = sessionOf(ha);
  const lastServerBag = m.water && m.water.after && isBag(m.water.after.backpack) ? m.water.after.backpack : null;
  {
    const home = hS && hS.home && typeof hS.home === "object" ? hS.home : null;
    const ok =
      !!hS &&
      !!def &&
      !!data &&
      !!data.default_plot &&
      hS.slot === "offline" &&
      hS.start === def.id &&
      ha.joined === false &&
      !!home &&
      home.kind === def.plot_kind &&
      home.plot === data.default_plot &&
      home.away === false &&
      !!mark &&
      bagEqual(hS.backpack, mark);
    const lost = hS && mark ? beyond(mark, hS.backpack) : {};
    const gained = hS && mark ? beyond(hS.backpack, mark) : {};
    add(
      "offline_untouched",
      ok,
      !hS
        ? "not recorded: the game's home session back in its own home"
        : !def
          ? noData("the default start and the default plot")
          : `the live save ${word(hS.slot)}, a ${word(hS.start)}, ${ha.joined === false ? "out of the shared world" : "STILL IN THE SHARED WORLD"}; its home a ${home ? word(home.kind) : "(none)"} on ${home ? word(home.plot) : "(none)"} (want a ${def.plot_kind} on ${data.default_plot}); ` +
              (!mark
                ? "OFFLINE_MARK was never recorded"
                : bagEqual(hS.backpack, mark)
                  ? "the backpack is OFFLINE_MARK again"
                  : `the backpack is NOT OFFLINE_MARK: missing ${bagText(lost, 4)}; extra ${bagText(gained, 4)}`),
    );
  }
  {
    const ss = hS && hS.server_save && typeof hS.server_save === "object" ? hS.server_save : null;
    const place = ss && ss.ship_place && typeof ss.ship_place === "object" ? ss.ship_place : null;
    const ok = !!ss && !!cab && ss.exists === true && ss.start === cab.id && !!place && place.plot === gamePlotWant && !!lastServerBag && bagEqual(ss.backpack, lastServerBag);
    add(
      "server_save_kept",
      ok,
      !ss
        ? "not recorded: the server's save as the game reported it from its own home"
        : !cab
          ? noData(`the ${CHOSEN_START} start`)
          : `${ss.exists === true ? "kept" : "NOT ON DISK"} (${String(ss.path || "").split(/[\\/]/).slice(-2).join("/")}): a ${word(ss.start)} on ${place ? word(place.plot) : "(no place)"} (want a ${cab.id} on ${gamePlotWant}); ` +
              `its backpack ${bagText(ss.backpack)}${lastServerBag ? (bagEqual(ss.backpack, lastServerBag) ? ", as the cabin held it after the meal and the water" : ` (the cabin held ${bagText(lastServerBag)})`) : " (what the cabin held after the water was not recorded)"}`,
    );
  }

  // ── 10. BACK TO THE SERVER.
  {
    const r = m.server_again || null;
    const s = sessionOf(r);
    const home = s && s.home && typeof s.home === "object" ? s.home : null;
    const pp = r && r.home_plot && typeof r.home_plot === "object" ? r.home_plot : null;
    const ok =
      !!s &&
      !!cab &&
      s.slot === "server" &&
      s.start === cab.id &&
      r.joined === true &&
      r.last_welcome === "stay" &&
      !!home &&
      home.plot === gamePlotWant &&
      home.kind === cab.plot_kind &&
      !!pp &&
      pp.id === gamePlotWant &&
      !!lastServerBag &&
      bagEqual(s.backpack, lastServerBag);
    add(
      "server_home_remembered",
      ok,
      !s
        ? "not recorded: the game's home session back on the server"
        : !cab
          ? noData(`the ${CHOSEN_START} start`)
          : `back on the server: the live save ${word(s.slot)}, a ${word(s.start)}, ${r.joined === true ? "in the shared world" : "NOT in the shared world"}, the welcome ${word(r.last_welcome)} on ${pp ? pp.id : "(no plot)"}; its home a ${home ? word(home.kind) : "(none)"} on ${home ? word(home.plot) : "(none)"} (want a ${cab.plot_kind} on ${gamePlotWant}, stay); ` +
              `the backpack ${lastServerBag ? (bagEqual(s.backpack, lastServerBag) ? "the cabin's again" : `NOT the cabin's: ${bagText(s.backpack, 4)}`) : "(the cabin's was not recorded)"}`,
    );
  }

  // ── 11. OVER THE RUN.
  {
    const game = num(m.corrections_game);
    const wl = Array.isArray(m.walker_corrections) ? m.walker_corrections : null;
    const sent = relayLog === null ? null : relayCorrections(relayLog, []).total;
    const ok = game === 0 && !!wl && wl.length === 0 && sent === 0;
    add(
      "no_correction",
      ok,
      `the game applied ${Number.isFinite(game) ? game : "(not recorded)"} correction(s); the walker logged ${wl ? wl.length : "(not recorded)"}${wl && wl.length ? ` (${wl[0]})` : ""}; the relay sent ${sent === null ? "(its log was not kept)" : sent} in all (nobody jumps in this run: none may come)`,
    );
  }
  const panics = num(m.panics);
  add("panics", panics === 0, Number.isFinite(panics) ? `${panics} PANIC line(s) in run.log` : "not recorded: run.log's PANIC lines");
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
  CHOSEN_START,
  EYE_M,
  STAND_FRONT_M,
  SENTENCES,
  RELAY_HOME_CHOSEN_RE,
  NAMES_SOMEONE_RE,
  LIMITS,
  TREE_FILES,
  readStarts,
  readDefaultPlot,
  readCounter,
  readWaterFillables,
  readMealGood,
  readDefaultCredits,
  readOpening,
  readTreeData,
  bagEqual,
  beyond,
  usedMeals,
  stepOf,
  plotsOfKind,
  plotBox,
  judgeStarts,
};
