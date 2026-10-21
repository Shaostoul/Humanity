// THE FALLS JUDGE (2026-10-20, `just verify-falls`): in the RUNNING game, does a fall the player
// walks into hurt the way the code and data/medical/falls.ron say, in both Illness and injury
// modes, and does a drop the player did not walk into hurt nobody?
//
// WHY. Falls shipped on 2026-10-20 (v0.1527.0, src/systems/falls.rs) with unit tests over the
// controller, the watch and the body pass, and nothing that looked at the game: `player_landed`
// logged nothing and no probe said what health or wounds the player had. A landing that never
// reached the body (the lib.rs hand-off missing, the watch swallowing every landing, the walk
// reporting 0) would have left every unit test green. The house rule (memory
// feedback_verify_on_player_state): verify on the state the player actually reaches.
//
// Pure: `judgeFalls` reads one run's record (the manifest scripts/verify-falls.js writes) and
// returns checks, so it is tested with made-up runs (scripts/tests/falls-judge.test.js, in
// `just rig-tests`) and re-run on a saved run without booting anything
// (`node scripts/verify-falls.js --dry-verdict <manifest.json>`).
//
// WHAT IT JUDGES AGAINST: the tree's own data, read when the run is made (`readTreeData`) and kept
// in the manifest's `data` block, so a saved run is judged against the data it ran on: the harm
// curve, the wound steps and Forgiving's share of the speed from data/medical/falls.ron, the
// walk's gravity from data/game.csv (`gravity_m_s2`), each wound's name from
// data/status_effects.csv, and the home's stairs, ladder, floor slabs and rooms from
// data/blueprints/structure_types.ron and data/homes/homestead.ron. The harm curve is evaluated
// HERE, by its own code, from the file: a second reading of falls.ron, not the game's.
//
// THE RUN (scripts/verify-falls.js says how each record is made). Aboard the homestead, in the
// world the autopilot enters (offline, the rig's own home). Since ship homes increment 7
// (2026-10-21) the homestead has an upper floor: its stairs (stairs-1, 16 risers to 3 m) lead onto
// it, its ladder (ladder-1) climbs through a guarded hatch with a gate onto it, and no unguarded
// 3 m edge is left anywhere in the home by design. So the walks off a raised floor go off a DECK
// the rig places for itself (`pickAboardDeck`; docs/design/ship-homes-increment-7-plan.md
// decision 10): a `deck` piece with its top at the upper floor's height (3.0 m), placed with the
// build editor's console (the showcase `console` verb) in the middle of the largest room open to
// the roof (the greenhouse), the player stood on it a metre from its south edge facing south:
//   forgiving_walk  Illness and injury set to Forgiving; forward held: off the deck's edge. A 3 m
//                   drop lands at 7.67 m/s, read at 70% (5.37), under the first harm (6.26): a
//                   landing, and nothing else. (Until 2026-10-21 this walked up the stairs and off
//                   their top; the saved runs of then carry kind "walk".)
//   realistic_stairs_down  (BUG-208, run right after forgiving_walk) Realistic, stood on the top
//                   tread facing down, forward held down the whole flight: every riser is a step
//                   down, so no landing reaches the body, no wound, no health lost. Before
//                   BUG-208 the walk flew off the top tread and landed from 3.0 m at 7.67 m/s.
//   realistic_walk  forgiving_walk again in Realistic: 7.67 m/s, past the sprain (6.26) and short of the
//                   broken ankle (7.73): a Sprained Ankle and 9.7 health.
//   realistic_ladder  still Realistic, in the hatch's hole at the foot of ladder-1 facing it:
//                   Space held climbs it to the top (the eye stops 0.2 m over the top rung), then
//                   forward held steps off the top across the hatch's gate onto the upper floor
//                   (`judgeLadderHatch`): the gate lets a climber through, so the body ends
//                   standing on the floor, and nothing lands harder than the 0.2 m step down from
//                   the top rung's hold (1.98 m/s, which cannot hurt): no wound, no health lost.
//                   (Until 2026-10-21 the top was open and this let go there and stepped back off
//                   it, a 3.2 m fall; the saved runs of then carry kind "ladder".)
//   placed_drop     still Realistic, the player PLACED at the deck's height beside it (the
//                   showcase `cam` verb, a teleport) and left to fall: the same drop, and no
//                   landing may reach the body, because something other than the walk moved it
//                   (FallWatch).
// And before any leg, `world_entry`: the first look after the autopilot entered the world has
// had no landing reach the body, no wound and full health; and `deck_placed`, what the console
// answered the line that placed the deck.
// The forgiving leg goes first so each walk starts from a clean body (no wound, full health); a
// wound already on the body is not a new one, so each later leg is judged by what it added.
//
// THE PLANET LEGS (2026-10-20, second increment). A planet's ground has its own walk
// (`surface_move::radial_step`, handed to the body through `CameraController::surface_step` in
// lib.rs), its own gravity (the body's data file, `PlanetDef::gravity_at`) and its own
// exemptions (dev flight, the flight band, water, a camera park's `probe_hold`), none of which the
// legs aboard touch. They run on open Sahara sand (`PLANET` below: the probe sweep's own walk
// vantage, `planet-open-noon-walk` in tests/visual/vantages.json: flat, dry, nothing built), with
// the player stood on the ground by the dev `stand` verb and on foot (`walk`):
//   planet_forgiving_fall  Forgiving; Space held (on a planet Space is not a one-shot jump but a
//                   climb at walking speed, `radial_step`'s walking branch) for long enough to
//                   rise about 2.6 m, then let go: the player falls back to the ground from
//                   wherever the climb and its coast took them. The climb is the walk's own
//                   speed, limp included (the ankles the legs aboard hurt climb at 30%), so the
//                   rig holds Space for as long as the rate the game reports (`climb_m_s`) needs.
//                   Read at 70% the landing is under the first harm: a landing, and nothing else.
//   planet_realistic_fall  the same in Realistic: about 7.1 m/s, a Sprained Ankle and its harm.
//   planet_placed_drop  still Realistic, the player PLACED 3 m over the ground (`stand`, a
//                   teleport) and left to fall: no landing may reach the body (`surface_step`'s
//                   placement test, the planet's twin of FallWatch's).
//   planet_deck_fall  (2026-10-20, the third increment) still Realistic: a deck BUILT on the sand
//                   (`DECK`: a Wood Roof on three Wood Walls, 3.2 m, put up by the dev `build` verb
//                   40 m east of the spot, open to the south), the player stood on its roof and
//                   walked off its open edge: the eye must stand on the roof (`stood_on_deck`, the
//                   deck's height over the ground it lands on), and the landing must reach the body
//                   at the speed of that drop with what falls.ron says of it (about 7.8 m/s, a hair
//                   either side of falls.ron's 7.73 m/s Broken Ankle, so the wound is worked out from
//                   the game's own landing, as for every leg).
// THE DECK. Until 2026-10-20 the planet walk did not stand on built pieces: its floor was the
// terrain alone (lib.rs fed `radial_step` the drawn ground's radius, and built pieces only stopped
// a step sideways, `planet_build::collide_on_site`), so a player stood on a built deck sank through
// it to the ground, and the first planet legs fell from the climb on Space instead. The walk's floor
// is now the higher of the terrain and a built piece's top under the body
// (`planet_build::built_floor_radius`, `surface_move::walk_rest`), and the deck leg holds it to
// that. The judge reads every planet drop from the eye's distance from the planet's centre
// (`planet_r`, every recorder frame).
// The game's own gravity on the planet (the `falls` block's, the planet walk's this frame) is the
// one the camera's speed is computed with, checked against the body's data file.
//
// WHAT THE GAME REPORTS (src/engine/body_probe.rs): each record is the dev IPC recorder's done
// JSON (debug/remote_players_done.json) trimmed to what is judged: `falls` {landings, last_m_s,
// mode, gravity_m_s2, flying, aboard}, the home session's `vitals` {health, health_max, wounds
// [{id, name, remaining_s}], registry}, and per frame {t, cam [x, y, z, yaw, pitch], health}.
// Every leg has a `before` (a zero-second recording) and an `after` (the recording of the walk or
// the drop).

"use strict";

const path = require("path");

/// The limits, each with its reason.
const LIMITS = {
  /// The least drop the camera must show for a walk leg to count as "walked off the edge", metres.
  /// The stairs are 3 m; anything under this means the walk never reached the top (a wall, a
  /// wrong pose, the hold not moving the body), and the leg says nothing about falls.
  MIN_DROP_M: 2.5,
  /// How far the game's landing speed may sit from the camera's own drop's (v = sqrt(2 g h)),
  /// m/s or share, whichever is larger: the walk computes the speed from the same top and floor
  /// the camera shows, so they agree to float precision on a healthy build.
  SPEED_TOL_M_S: 0.15,
  SPEED_TOL_SHARE: 0.03,
  /// How far the health the landing took may be from what falls.ron says, health points: the
  /// body heals by itself between frames (100 over days, a hundredth of a point in a few
  /// seconds) and nothing else takes health in the rig's sealed home.
  HEALTH_TOL: 0.5,
  /// How long after the landing frame the health must have fallen, seconds of the frame clock:
  /// the body pass takes a landing on its next tick, a frame or two.
  HARM_WITHIN_S: 1.5,
  /// How near the requested start the first recorded camera must stand, metres.
  START_TOL_M: 0.6,
  /// The share of the rig's hang (the seconds between letting go of the climb key and stepping
  /// away) the eye must stay at the ladder's top: a body that does not hold on drops the moment
  /// the key comes up, and stays up there only as long as the climb overshot.
  HANG_SHARE: 0.8,
  /// A planet fall leg must drop at least this share over the height whose landing reaches
  /// falls.ron's first harm at the planet's gravity (h = v^2 / 2 g: 2.0 m at 9.81 m/s^2), so
  /// Realistic must hurt and Forgiving's "no wound" is not the vacuous "no fall".
  PLANET_MIN_DROP_SHARE: 1.05,
  /// How far above where the eye comes to rest a planet landing is taken, metres: the walk's
  /// settle glue (`surface_move::SETTLE_GLUE_M`, 0.5 m) catches a fall within half a metre of the
  /// floor and eases the eye down the rest of the way over most of a second, so the landing is
  /// the first frame inside that half metre, not the frame the eye finally stops.
  PLANET_LAND_BAND_M: 0.52,
  /// How much a wound's time left must rise between a leg's before and after for the leg to have
  /// given it again (renewed), seconds: between the two records it otherwise only runs down, by
  /// about the seconds between them (a wound counts game seconds, 1x in the rig's Normal mode).
  RENEWED_S: 1.0,
  /// Within this of the rest height the planet walk counts as standing
  /// (`surface_move::GROUNDED_REACH_M`): a planet fall must start there, on the ground.
  PLANET_STAND_M: 0.35,
  /// How far the eye's standing height on the deck, over where it came to rest on the ground after
  /// walking off, may be from the deck's own height, metres: the sand's clearance under a standing
  /// eye (`surface_walk::DRAWN_CLEARANCE_M`, 0.05 m) and the ground's own rise between the hut and
  /// where the walk ends. A walk that goes through the deck stands on the ground, 3 m off.
  DECK_TOL_M: 0.3,
};

/// The planet ground the planet legs stand on: open Sahara sand at the probe sweep's own walk
/// vantage (tests/visual/vantages.json `planet-open-noon-walk`), flat, dry and with nothing
/// built, parked at 30 m and stood on by the dev `stand` verb.
const PLANET = { body: "earth", lat: 23.5, lon: 13.08, park_altitude_km: 0.03 };

/// The deck the planet deck leg is stood on and walks off (2026-10-20): `roof` resting on three
/// `wall`s (north, west and east: open to the south), built by the dev `build` verb `dx` metres
/// east of the planet legs' spot (so nothing built stands where they ran), the player stood on its
/// roof `stand_from_edge_m` north of its south edge, facing south.
const DECK = { wall: "wood_wall", roof: "roof", dx: 40, stand_from_edge_m: 1.0 };

/** The deck from data/blueprints/basic.ron (`readStructureTypes` reads its sizes too): the dev
 *  `build` verb's spec, its height (the walls' and the roof's, metres: what the roof rests on and
 *  how thick it is), and where the player is stood on it (site metres east and south of the spot,
 *  facing south). null if either blueprint is missing. */
function pickDeck(blueprints) {
  const wall = blueprints[DECK.wall];
  const roof = blueprints[DECK.roof];
  if (!wall || !roof || !Array.isArray(wall.size) || !Array.isArray(roof.size)) return null;
  const half = roof.size[0] / 2;
  const x = DECK.dx;
  return {
    spec: [`${DECK.wall}@${x},${-half},0`, `${DECK.wall}@${x - half},0,1`, `${DECK.wall}@${x + half},0,1`, `${DECK.roof}@${x},0,0`].join(";"),
    top_m: wall.size[1] + roof.size[1],
    stand: [x, half - DECK.stand_from_edge_m],
    edge_m: DECK.stand_from_edge_m,
    heading_deg: 180,
  };
}

// ── Reading the tree's data (pure: each reader takes a file's text) ─────────────────────────────

/** A RON file's text without its `//` comments (none of the files read here has "//" in a string). */
const uncomment = (text) => String(text || "").replace(/\/\/[^\n]*/g, "");

/** The text of the `[...]` list that follows `key:` in `text` (brackets balanced), or "". */
function listAfter(text, key) {
  const at = text.search(new RegExp(`\\b${key}\\s*:\\s*\\[`));
  if (at < 0) return "";
  const open = text.indexOf("[", at);
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    if (text[i] === "[") depth++;
    else if (text[i] === "]" && --depth === 0) return text.slice(open + 1, i);
  }
  return "";
}

/** data/medical/falls.ron: { speed_share, harm [{m_s, health}], steps [{from_m_s, effect}], landed, cause }. */
function readFalls(text) {
  const t = uncomment(text);
  const share = t.match(/forgiving\s*:\s*\(\s*speed_share\s*:\s*([-\d.]+)\s*\)/);
  const harm = [...listAfter(t, "harm").matchAll(/\(\s*m_s\s*:\s*([-\d.]+)\s*,\s*health\s*:\s*([-\d.]+)\s*\)/g)].map((m) => ({
    m_s: Number(m[1]),
    health: Number(m[2]),
  }));
  const steps = [...listAfter(t, "steps").matchAll(/\(\s*from_m_s\s*:\s*([-\d.]+)\s*,\s*effect\s*:\s*"([^"]+)"\s*\)/g)].map((m) => ({
    from_m_s: Number(m[1]),
    effect: m[2],
  }));
  const str = (k) => (t.match(new RegExp(`\\b${k}\\s*:\\s*"([^"]*)"`)) || [])[1];
  if (!share || !harm.length || !steps.length) {
    throw new Error("falls.ron: could not read forgiving.speed_share, harm and steps (did its shape change?)");
  }
  return { speed_share: Number(share[1]), harm, steps, landed: str("landed"), cause: str("cause") };
}

/** data/game.csv: the `gravity_m_s2` row's value. */
function readGravity(csvText) {
  const row = String(csvText || "")
    .split(/\r?\n/)
    .find((l) => l.startsWith("gravity_m_s2,"));
  const g = row ? Number(row.split(",")[1]) : NaN;
  if (!Number.isFinite(g) || g <= 0) throw new Error("game.csv: no gravity_m_s2 row with a positive number");
  return g;
}

/** data/status_effects.csv: id -> { name, kind } (the first three columns). */
function readEffects(csvText) {
  const out = {};
  for (const line of String(csvText || "").split(/\r?\n/).slice(1)) {
    const [id, name, kind] = line.split(",");
    if (id && name) out[id.trim()] = { name: name.trim(), kind: (kind || "").trim() };
  }
  return out;
}

/** data/blueprints/structure_types.ron: id -> { kind, shape, size [w, h, d], steps }. */
function readStructureTypes(text) {
  const t = uncomment(text);
  const out = {};
  const starts = [...t.matchAll(/\bid\s*:\s*"([^"]+)"/g)];
  starts.forEach((m, i) => {
    const body = t.slice(m.index, i + 1 < starts.length ? starts[i + 1].index : t.length);
    const size = body.match(/\bsize\s*:\s*\(([^)]*)\)/);
    out[m[1]] = {
      kind: (body.match(/\bkind\s*:\s*(\w+)/) || [])[1],
      shape: (body.match(/\bshape\s*:\s*(\w+)/) || [])[1],
      size: size ? size[1].split(",").map((v) => Number(v.trim())) : null,
      steps: Number((body.match(/\bsteps\s*:\s*(\d+)/) || [])[1] || 0),
    };
  });
  return out;
}

/** A home design's placed structures: [{ id, type_id, pos [x, y, z], rot_deg }]. */
function readHomeStructures(text) {
  const t = uncomment(text);
  return [...listAfter(t, "structures").matchAll(/\bid\s*:\s*"([^"]+)"\s*,\s*type_id\s*:\s*"([^"]+)"\s*,\s*pos\s*:\s*\(([^)]*)\)\s*,\s*rot_deg\s*:\s*([-\d.]+)/g)].map(
    (m) => ({ id: m[1], type_id: m[2], pos: m[3].split(",").map((v) => Number(v.trim())), rot_deg: Number(m[4]) })
  );
}

const num2 = (s) => s.split(",").map((v) => Number(v.trim()));

/** A home design's floor slabs (ship homes increment 7): [{ a [x, z], b [x, z], top }], home metres.
 *  A slab's top is a floor a person stands on (src/ship/home_structure.rs `FloorSlab`). */
function readHomeFloors(text) {
  const t = uncomment(text);
  return [...listAfter(t, "floors").matchAll(/\ba\s*:\s*\(([^)]*)\)\s*,\s*b\s*:\s*\(([^)]*)\)\s*,\s*top\s*:\s*([-\d.]+)/g)].map((m) => ({
    a: num2(m[1]),
    b: num2(m[2]),
    top: Number(m[3]),
  }));
}

/** A home design's interior walls: [{ a [x, z], b [x, z], base, height }], home metres (a wall with
 *  no `base` stands on the ground, 0). */
function readHomeWalls(text) {
  const t = uncomment(text);
  return [...listAfter(t, "walls").matchAll(/\ba\s*:\s*\(([^)]*)\)\s*,\s*b\s*:\s*\(([^)]*)\)\s*,\s*(?:base\s*:\s*([-\d.]+)\s*,\s*)?height\s*:\s*([-\d.]+)/g)].map((m) => ({
    a: num2(m[1]),
    b: num2(m[2]),
    base: m[3] === undefined ? 0 : Number(m[3]),
    height: Number(m[4]),
  }));
}

/** A home design's room zones: [{ id, origin [x, y, z], size [x, y, z] }], home metres. */
function readHomeZones(text) {
  const t = uncomment(text);
  return [...listAfter(t, "zones").matchAll(/\bid\s*:\s*"([^"]+)"\s*,\s*type_id\s*:\s*"[^"]*"\s*,\s*origin\s*:\s*\(([^)]*)\)\s*,\s*size\s*:\s*\(([^)]*)\)/g)].map((m) => ({
    id: m[1],
    origin: num2(m[2]),
    size: num2(m[3]),
  }));
}

/** A home design's body height, metres (its first `height:`, the body's, before its walls). */
function readHomeHeight(text) {
  const t = uncomment(text);
  const body = t.slice(t.search(/\bbody\s*:/));
  const h = body.match(/\bheight\s*:\s*([-\d.]+)/);
  return h ? Number(h[1]) : NaN;
}

/** Whether the slab `s` covers the floor point (x, z), home metres. */
const slabCovers = (s, x, z) => x >= Math.min(s.a[0], s.b[0]) && x <= Math.max(s.a[0], s.b[0]) && z >= Math.min(s.a[1], s.b[1]) && z <= Math.max(s.a[1], s.b[1]);
/** Whether any of `floors` at height `top` (within a centimetre) covers (x, z). */
const floorAt = (floors, top, x, z) => (floors || []).some((s) => Math.abs(s.top - top) < 0.01 && slabCovers(s, x, z));

/**
 * The staircase a walk leg goes up and off: the tallest Steps-shaped Stairs in the home, with the
 * pose to start from (home metres, eye height; 1 m before its first step, facing up it) and the
 * drop off its top. Mirrors src/ship/structure.rs: a piece's local +z maps to world
 * (sin r, cos r), and its steps rise along local +z; forward_xz is (sin yaw, -cos yaw), so facing
 * up the stairs is yaw = pi - r.
 */
function pickStairs(types, structures, eyeM = 1.7) {
  const stairs = structures
    .map((s) => ({ s, ty: types[s.type_id] }))
    .filter(({ ty }) => ty && ty.kind === "Stairs" && ty.shape === "Steps" && Array.isArray(ty.size))
    .sort((a, b) => b.ty.size[1] - a.ty.size[1]);
  if (!stairs.length) return null;
  const { s, ty } = stairs[0];
  const [w, h, d] = ty.size;
  const r = (s.rot_deg * Math.PI) / 180;
  const along = (lz) => [s.pos[0] + lz * Math.sin(r), s.pos[2] + lz * Math.cos(r)];
  const [fx, fz] = along(-d / 2 - 1.0);
  const tread = ty.steps > 0 ? d / ty.steps : d;
  const [tx, tz] = along(d / 2 - tread / 2);
  return {
    /// How many risers the flight has (the type's `steps`), and one tread's depth, metres (the
    /// stairs' depth over their steps).
    steps: ty.steps,
    tread_m: tread,
    /// The stairs-down leg's start: the eye standing on the middle of the top tread.
    top_tread_eye: [tx, s.pos[1] + h + eyeM, tz],
    /// Facing DOWN the stairs (the opposite of `yaw`).
    down_yaw: 0 - r,
    id: s.id,
    type_id: s.type_id,
    size: [w, h, d],
    rot_deg: s.rot_deg,
    pos: s.pos.slice(),
    top_m: s.pos[1] + h,
    foot: [fx, s.pos[1] + eyeM, fz],
    yaw: Math.PI - r,
    /// The placement leg's eye: over the start (off the stairs' footprint), as high as the eye on
    /// the top step, so it drops the same height the walk does.
    placed_eye: [fx, s.pos[1] + h + eyeM, fz],
    /// Metres from the start to the top step's far edge (1 m to the first step, then its depth).
    walk_m: 1.0 + d,
  };
}

/** The yaw that looks along the floor direction (dx, dz): the camera at yaw a looks along
 *  (sin a, -cos a) (src/renderer/camera.rs). */
const yawAlong = (dx, dz) => Math.atan2(dx, -dz);

/**
 * The ladder a ladder leg climbs: the tallest Ladder in the home, the pose to stand at it (home
 * metres, eye height, STAND_M out from its face, facing it) and how high the eye hangs at its top.
 * Mirrors src/renderer/camera.rs: climbing clamps the eye at the ladder's top + eye height + 0.2 m,
 * and the climb zone (engine/footing.rs `ladder_at`) is within half the ladder's widest side +
 * 0.9 m of its foot, so a step of more than that away from it lets go.
 *
 * THE HATCH (ship homes increment 7, 2026-10-21). When a floor slab of the home stands at the
 * ladder's top (`floors`), the ladder climbs through a hatch onto an upper floor
 * (engine/footing.rs `ladder_gates`): one side of its top is the slab (where a climber steps off),
 * the other the hatch's hole (where a climber stands to climb, under no ceiling). `hatch` then says
 * which is which: the climber stands on the hole's side facing the slab, and stepping forward off
 * the top crosses the gate across the ladder's top (the gate lets a body ON the ladder through) onto
 * the floor. `step_m` is how far along that way from the ladder's foot the walk ends: out of the
 * ladder's reach (`reach_m`) by STEP_PAST_M, standing on the floor. null with no slab at the top.
 */
function pickLadder(types, structures, eyeM = 1.7, floors = []) {
  const STAND_M = 0.6;
  const STEP_PAST_M = 0.3;
  const ladders = structures
    .map((s) => ({ s, ty: types[s.type_id] }))
    .filter(({ ty }) => ty && ty.kind === "Ladder" && Array.isArray(ty.size))
    .sort((a, b) => b.ty.size[1] - a.ty.size[1]);
  if (!ladders.length) return null;
  const { s, ty } = ladders[0];
  const [w, h, d] = ty.size;
  const r = (s.rot_deg * Math.PI) / 180;
  const top = s.pos[1] + h;
  const reach = Math.max(w, d) * 0.5 + 0.9;
  // Local +z of the piece in the home: (sin r, cos r).
  const side = (k) => [s.pos[0] + k * STAND_M * Math.sin(r), s.pos[2] + k * STAND_M * Math.cos(r)];
  const onFloor = (k) => floorAt(floors, top, ...side(k));
  // The slab's side: +1 (local +z) or -1; null when the top is not a floor on exactly one side.
  const slabSide = onFloor(1) && !onFloor(-1) ? 1 : onFloor(-1) && !onFloor(1) ? -1 : null;
  let hatch = null;
  if (slabSide !== null) {
    const dir = [slabSide * Math.sin(r), slabSide * Math.cos(r)];
    const [sx, sz] = side(-slabSide);
    const stepM = reach + STEP_PAST_M;
    hatch = {
      /// The floor at the ladder's top (the slab's top), metres, and the eye standing on it.
      floor_m: top,
      floor_eye_m: top + eyeM,
      /// Where the climber stands (the hole's side, eye height on the ground) and faces (the slab).
      stand: [sx, s.pos[1] + eyeM, sz],
      yaw: yawAlong(dir[0], dir[1]),
      /// The way off the top, across the floor, and where the walk ends along it from the foot.
      dir,
      step_m: stepM,
      step_to: [s.pos[0] + dir[0] * stepM, top + eyeM, s.pos[2] + dir[1] * stepM],
      /// From the stand to the walk's end, metres across the floor.
      walk_m: STAND_M + stepM,
    };
  }
  return {
    id: s.id,
    type_id: s.type_id,
    size: [w, h, d],
    rot_deg: s.rot_deg,
    pos: s.pos.slice(),
    top_m: top,
    stand: [s.pos[0] + STAND_M * Math.sin(r), s.pos[1] + eyeM, s.pos[2] + STAND_M * Math.cos(r)],
    yaw: -r,
    hang_eye_m: top + eyeM + 0.2,
    reach_m: reach,
    hatch,
  };
}

/** The deck the walk legs aboard step off (ship homes increment 7, docs/design/
 *  ship-homes-increment-7-plan.md decision 10): placed by the rig itself with the build editor's
 *  console (`add_structure deck x y z 0`, the showcase `console` verb), its top at the home's upper
 *  floor's height, in the middle of the largest ground room no floor slab lies over (the
 *  homestead's greenhouse, 6 m to its roof), the player stood on it `EDGE_M` from its south edge
 *  facing south and walked `WALK_ON_M` past the edge. Placed beside it (`PLACED_GAP_M` north of its
 *  north edge) at the same height, the placement leg's drop. */
const ABOARD_DECK = { type_id: "deck", EDGE_M: 1.0, WALK_ON_M: 1.5, PLACED_GAP_M: 1.0, HEADROOM_M: 0.3 };

/** Whether the segment (a, b) crosses or touches the rectangle [x0, x1] x [z0, z1]. */
function segmentHitsRect(a, b, x0, x1, z0, z1) {
  const inside = (p) => p[0] >= x0 && p[0] <= x1 && p[1] >= z0 && p[1] <= z1;
  if (inside(a) || inside(b)) return true;
  const o = (p, q, r) => (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
  const cross = (p, q, r, s) => o(p, q, r) * o(p, q, s) < 0 && o(r, s, p) * o(r, s, q) < 0;
  const c = [[x0, z0], [x1, z0], [x1, z1], [x0, z1]];
  return c.some((p, i) => cross(a, b, p, c[(i + 1) % 4]));
}

/**
 * Where the rig's deck goes (`ABOARD_DECK`), from the tree: { type_id, size, top_m, room, pos
 * (home metres, the piece's base: its top less its thickness), console (the console line that
 * places it), stand (the eye on it, home metres), yaw (south), edge_m, walk_on_m, placed_eye } or
 * { why } when it cannot go anywhere (no deck type, no upper floor to take the height from, no
 * open room with the headroom, or a wall across the walk).
 */
function pickAboardDeck(types, zones, floors, walls, bodyHeight, eyeM = 1.7) {
  const D = ABOARD_DECK;
  const ty = types[D.type_id];
  if (!ty || ty.kind !== "Deck" || !Array.isArray(ty.size)) return { why: `structure_types.ron has no Deck called "${D.type_id}"` };
  const tops = (floors || []).map((s) => s.top).filter((t) => t > 0.01 && t < bodyHeight - 0.01);
  if (!tops.length) return { why: "the home has no upper floor (no floor slab over the ground) to take the deck's height from" };
  const top = Math.max(...tops);
  const [w, h, d] = ty.size;
  if (bodyHeight - (top + eyeM) < D.HEADROOM_M) return { why: `the home is ${bodyHeight} m tall: no room for an eye ${eyeM} m over a ${top} m deck` };
  // Ground rooms no slab lies over: open to the roof.
  const overlaps = (z, s) => {
    const [x0, x1] = [Math.min(s.a[0], s.b[0]), Math.max(s.a[0], s.b[0])];
    const [z0, z1] = [Math.min(s.a[1], s.b[1]), Math.max(s.a[1], s.b[1])];
    return Math.min(x1, z.origin[0] + z.size[0]) - Math.max(x0, z.origin[0]) > 0.01 && Math.min(z1, z.origin[2] + z.size[2]) - Math.max(z0, z.origin[2]) > 0.01;
  };
  const open = (zones || [])
    .filter((z) => Math.abs(z.origin[1]) < 0.01 && !(floors || []).some((s) => overlaps(z, s)))
    .sort((a, b) => b.size[0] * b.size[2] - a.size[0] * a.size[2]);
  const room = open[0];
  if (!room) return { why: "no ground room is open to the roof (every one has a floor slab over it)" };
  const cx = room.origin[0] + room.size[0] / 2;
  const cz = room.origin[2] + room.size[2] / 2;
  // The ground the legs use: the deck, the walk off its south edge and on, and the placement north of it.
  const area = [cx - w / 2, cx + w / 2, cz - d / 2 - D.PLACED_GAP_M - 0.5, cz + d / 2 + D.WALK_ON_M + 1.0];
  if (area[2] < room.origin[2] || area[3] > room.origin[2] + room.size[2] || area[0] < room.origin[0] || area[1] > room.origin[0] + room.size[0]) {
    return { why: `${room.id} is too small for the deck and its walk` };
  }
  const across = (walls || []).find((wl) => wl.base < top + eyeM && wl.base + wl.height > 0.4 && segmentHitsRect(wl.a, wl.b, ...area));
  if (across) return { why: `a wall stands across the deck's walk in ${room.id}: (${across.a}) to (${across.b})` };
  const pos = [cx, top - h, cz];
  const f = (v) => Number(v.toFixed(3));
  return {
    type_id: D.type_id,
    size: [w, h, d],
    top_m: top,
    room: room.id,
    pos,
    console: `add_structure ${D.type_id} ${f(pos[0])} ${f(pos[1])} ${f(pos[2])} 0`,
    stand: [cx, top + eyeM, cz + d / 2 - D.EDGE_M],
    yaw: yawAlong(0, 1),
    edge_m: D.EDGE_M,
    walk_on_m: D.WALK_ON_M,
    placed_eye: [cx, top + eyeM, cz - d / 2 - D.PLACED_GAP_M],
  };
}

/** data/planets/<body>.ron: { gravity_m_s2, curved } (the top-level `gravity:`, and whether a
 *  `gravity_curve` makes it vary with altitude, in which case the game's own reading stands). */
function readPlanetGravity(text) {
  const t = uncomment(text);
  const g = t.match(/^\s*gravity\s*:\s*([-\d.]+)/m);
  if (!g || !(Number(g[1]) > 0)) throw new Error("planet file: no top-level gravity with a positive number (did its shape change?)");
  return { gravity_m_s2: Number(g[1]), curved: /\bgravity_curve\s*:\s*Some/.test(t) };
}

/** Everything the judge needs from the tree. `readFile` is injectable for tests. */
function readTreeData(repo, readFile = (p) => require("fs").readFileSync(p, "utf8")) {
  const rd = (...p) => readFile(path.join(repo, ...p));
  const types = readStructureTypes(rd("data", "blueprints", "structure_types.ron"));
  const home = rd("data", "homes", "homestead.ron");
  const placed = readHomeStructures(home);
  const floors = readHomeFloors(home);
  const deck = pickAboardDeck(types, readHomeZones(home), floors, readHomeWalls(home), readHomeHeight(home));
  return {
    falls: readFalls(rd("data", "medical", "falls.ron")),
    gravity_m_s2: readGravity(rd("data", "game.csv")),
    effects: readEffects(rd("data", "status_effects.csv")),
    stairs: pickStairs(types, placed),
    ladder: pickLadder(types, placed, 1.7, floors),
    /// The homestead's floor slabs (ship homes increment 7): where an upper floor is.
    floors,
    /// The deck the walk legs aboard step off, or { why } it cannot be placed.
    deck,
    planet: {
      ...PLANET,
      ...readPlanetGravity(rd("data", "planets", `${PLANET.body}.ron`)),
      deck: pickDeck(readStructureTypes(rd("data", "blueprints", "basic.ron"))),
    },
  };
}

// ── The rules, evaluated here from the file (a second reading of falls.ron, not the game's) ──────

/** The speed a landing at `v` m/s is read at in `mode` ("realistic" | "forgiving"). */
function readSpeed(falls, v, mode) {
  const s = Math.max(0, Number(v) || 0);
  return mode === "forgiving" ? s * Math.min(1, Math.max(0, falls.speed_share)) : s;
}

/** Health a landing read at `v` takes: none below the first point, straight lines between, the
 *  last line carried on past the end. */
function harmAt(falls, v) {
  const p = falls.harm;
  if (!p.length || v < p[0].m_s) return 0;
  let i = p.findIndex((pt, k) => k > 0 && v < pt.m_s);
  i = i < 0 ? p.length - 2 : i - 1;
  if (i < 0) return Math.max(0, p[0].health);
  const a = p[i];
  const b = p[i + 1];
  if (!b || b.m_s <= a.m_s) return Math.max(0, p[0].health);
  return Math.max(0, a.health + ((v - a.m_s) / (b.m_s - a.m_s)) * (b.health - a.health));
}

/** The wound a landing read at `v` leaves: the last step it reaches, or null. */
function woundAt(falls, v) {
  let w = null;
  for (const s of falls.steps) if (v >= s.from_m_s) w = s.effect;
  return w;
}

/** What falls.ron says a landing at `v` m/s does in `mode`: { read, harm, wound }. */
function expectFall(falls, v, mode) {
  const read = readSpeed(falls, v, mode);
  return { read, harm: harmAt(falls, read), wound: woundAt(falls, read) };
}

// ── Reading a record ─────────────────────────────────────────────────────────────────────────────

const num = (x) => (x === null || x === undefined || x === "" ? NaN : Number(x));
const f2 = (x) => (Number.isFinite(num(x)) ? num(x).toFixed(2) : "(none)");
const woundIds = (rec) => (rec && rec.vitals && Array.isArray(rec.vitals.wounds) ? rec.vitals.wounds.map((w) => w && w.id).filter(Boolean) : null);
const healthOf = (rec) => (rec && rec.vitals ? num(rec.vitals.health) : NaN);
const landingsOf = (rec) => (rec && rec.falls ? num(rec.falls.landings) : NaN);

/** How a drop is read aboard: the camera's y is up, and the walk stops the eye at the floor. */
const ABOARD_DROP = { height: (f) => (f && Array.isArray(f.cam) ? num(f.cam[1]) : NaN), landBand: 0.02 };
/** On a planet: the eye's distance from the body's centre is up (`planet_r`; the camera's y is
 *  not), and the settle glue catches the last half metre (LIMITS.PLANET_LAND_BAND_M). */
const PLANET_DROP = { height: (f) => (f ? num(f.planet_r) : NaN), landBand: LIMITS.PLANET_LAND_BAND_M };

/**
 * The eye's drop in a recording: the highest the eye was, where it came to rest (the last
 * frame's height), the drop between, and the frame clock of the first frame back within
 * `landBand` of rest after the highest point (the landing). Heights are `how.height` of a frame
 * (ABOARD_DROP, the default, or PLANET_DROP). null with fewer than two frames.
 */
function dropOf(frames, how = ABOARD_DROP) {
  const h = how.height;
  const fs = (Array.isArray(frames) ? frames : []).filter((f) => f && Array.isArray(f.cam) && Number.isFinite(h(f)));
  if (fs.length < 2) return null;
  let top = 0;
  fs.forEach((f, i) => {
    if (h(f) > h(fs[top])) top = i;
  });
  const rest = h(fs[fs.length - 1]);
  const peak = h(fs[top]);
  let landed = -1;
  for (let i = top + 1; i < fs.length; i++) {
    if (h(fs[i]) <= rest + how.landBand) {
      landed = i;
      break;
    }
  }
  // The longest stretch the eye stayed within 5 cm of its highest (on a ladder: the hang).
  let held = 0;
  let from = null;
  for (const f of fs) {
    if (h(f) >= peak - 0.05) {
      if (from === null) from = num(f.t);
      held = Math.max(held, num(f.t) - from);
    } else from = null;
  }
  return {
    peak_y: peak,
    peak_held_s: held,
    rest_y: rest,
    first_y: h(fs[0]),
    drop_m: peak - rest,
    landed_t: landed >= 0 ? num(fs[landed].t) : NaN,
    first_cam: fs[0].cam.slice(0, 3),
    frames: fs,
  };
}

/// ── The judgement ────────────────────────────────────────────────────────────────────────────────

/** The legs every run must have: aboard (the first rig), and on a planet when the run planned
 *  them (its manifest carries the `planet` block the rig writes before it boots; the runs saved
 *  before the planet legs existed, tests/fixtures/falls/green.json and red-hand-off-removed.json,
 *  carry none). */
const ABOARD_LEGS = ["forgiving_walk", "realistic_walk", "realistic_ladder", "placed_drop"];
const PLANET_LEGS = ["planet_forgiving_fall", "planet_realistic_fall", "planet_placed_drop"];
/** Walking DOWN the stairs (BUG-208, 2026-10-20). */
const STAIRS_LEGS = ["realistic_stairs_down"];
/** Standing on a deck built on a planet's ground and walking off it (2026-10-20). */
const DECK_LEGS = ["planet_deck_fall"];
/** Every leg the rig runs now. A run whose manifest carries the rig's `plan` (every run since the
 *  stairs-down leg) must have every leg of that plan; one without a plan is an older saved run,
 *  judged by the legs that rig had. The rig plans all of these (scripts/tests/falls-judge.test.js
 *  holds it to that), so only a run saved before the deck leg is judged by fewer. */
const ALL_LEGS = [...ABOARD_LEGS.slice(0, 1), ...STAIRS_LEGS, ...ABOARD_LEGS.slice(1), ...PLANET_LEGS, ...DECK_LEGS];

/**
 * Judge one run. `run` = { data (readTreeData's), entry (the first look after world entry),
 * planet (where the planet legs ran, when they were planned), origin (the home's corner in the
 * camera's frame), deck_console ({ line, said }: the console's answer for the rig's deck aboard),
 * legs: [{ id, mode, kind: "deck"|"stairs_down"|"ladder_hatch"|"placement"|"planet_fall"|
 * "planet_placement"|"planet_deck" (and, in runs saved before 2026-10-21, "walk"|"ladder"), start
 * [x, y, z] (aboard), deck_m (a deck's), hang_s (an old ladder leg's), body (a planet leg's),
 * before, after }], panics, builtin_data [lines], other_build }. Returns { checks: [{ id, ok,
 * detail }], ok, observed: { <leg>: what was seen, for the report } }.
 */
function judgeFalls(run) {
  const checks = [];
  const observed = {};
  const check = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const data = run && run.data;
  if (!data || !data.falls || !Number.isFinite(num(data.gravity_m_s2))) {
    check("tree_data", false, "the manifest carries no tree data (falls.ron, gravity): nothing to judge against");
    return { checks, ok: false, observed };
  }
  const gShip = num(data.gravity_m_s2);
  const effects = data.effects || {};
  const legs = Array.isArray(run.legs) ? run.legs : [];
  for (const want of Array.isArray(run.plan) ? run.plan : [...ABOARD_LEGS, ...(run.planet ? PLANET_LEGS : [])]) {
    if (!legs.some((l) => l && l.id === want)) check(`${want}.ran`, false, "this leg never ran (see the rig's output for where it stopped)");
  }

  // World entry (PRIORITIES: "check no landing hurts at world entry"): the first look after the
  // autopilot's entry and the home's few seconds to build has had no landing reach the body.
  const entry = run.entry || null;
  if (!entry || !entry.falls || !entry.vitals) {
    check("world_entry", false, "no record of the first look after world entry");
  } else {
    const n = num(entry.falls.landings);
    const w = woundIds(entry) || [];
    const full = num(entry.vitals.health) >= num(entry.vitals.health_max) - LIMITS.HEALTH_TOL;
    check(
      "world_entry",
      n === 0 && w.length === 0 && full,
      `after entering the world: ${n} landing(s) handed to the body, wounds [${w.join(", ")}], health ${f2(entry.vitals.health)} of ${f2(entry.vitals.health_max)}`
    );
  }

  // The rig's deck aboard (ship homes increment 7): what the build editor's console answered the
  // line that places it (the showcase `console` verb's run.log line). A diagnosis, not the proof:
  // each walk leg's `stood_on_deck` is the eye measured standing on it.
  if (legs.some((l) => l && l.kind === "deck")) {
    const dc = run.deck_console || null;
    const said = dc && typeof dc.said === "string" ? dc.said : null;
    check(
      "deck_placed",
      !!said && /^added structure #\d+ \(deck\)/.test(said),
      !dc
        ? "the rig recorded no console answer for its deck (the run.log has no \"Showcase: console\" line: the request was never read, or the build is older than the console verb)"
        : `the console answered "${said === null ? "(nothing)" : said}" to "${dc.line}"` + (said && /^added structure/.test(said) ? "" : ": the deck was not placed (engine/ipc.rs console_this_session, gui/pages/construction.rs exec_construction_command)")
    );
  }

  for (const leg of legs) {
    const id = leg.id;
    const before = leg.before || null;
    const after = leg.after || null;
    if (!before || !after) {
      check(`${id}.recorded`, false, `no ${!before ? "before" : "after"} record: the game never answered the recorder`);
      continue;
    }
    const onPlanet = leg.kind === "planet_fall" || leg.kind === "planet_placement" || leg.kind === "planet_deck";
    // A deck walk: on a planet (built of blueprint pieces) or aboard (the rig's own deck piece).
    const isDeck = leg.kind === "planet_deck" || leg.kind === "deck";
    const deckM = isDeck ? num(leg.deck_m) : NaN;
    const isPlacement = leg.kind === "placement" || leg.kind === "planet_placement";
    const drop = dropOf(after.frames, onPlanet ? PLANET_DROP : ABOARD_DROP);
    const hb = healthOf(before);
    const ha = healthOf(after);
    const wb = woundIds(before) || [];
    const wa = woundIds(after);
    // What this leg gave the body: a wound not on it before, or one it already had whose time left
    // went UP (StatusEffects::apply keeps the longer of the two, so a sprain on a sprained ankle
    // starts its healing over: the planet's Realistic fall sprains an ankle the walk aboard already
    // sprained). A wound's time only ever runs down on its own.
    const left = (rec, id) => {
      const w = rec && rec.vitals && Array.isArray(rec.vitals.wounds) ? rec.vitals.wounds.find((x) => x && x.id === id) : null;
      return w ? num(w.remaining_s) : NaN;
    };
    const newWounds = [...new Set(wa || [])].filter((w) => !wb.includes(w) || left(after, w) > left(before, w) + LIMITS.RENEWED_S);
    const lb = landingsOf(before);
    const la = landingsOf(after);
    const last = after.falls ? num(after.falls.last_m_s) : NaN;
    const gGame = after.falls ? num(after.falls.gravity_m_s2) : NaN;
    const seen = (observed[id] = {
      mode: after.falls && after.falls.mode,
      where: onPlanet ? `on ${leg.body}` : "aboard",
      ...(leg.kind === "planet_fall" ? { climb_m_s: num(leg.climb_m_s), hold_s: num(leg.hold_s) } : {}),
      ...(isDeck ? { deck_m: deckM, walk_m_s: num(leg.walk_m_s), hold_s: num(leg.hold_s) } : {}),
      gravity_m_s2: gGame,
      health_before: hb,
      health_after: ha,
      wounds_after: (after.vitals && after.vitals.wounds) || [],
      new_wounds: newWounds,
      landings: [lb, la],
      landing_m_s: Number.isFinite(last) && la > lb ? last : null,
      drop_m: drop ? drop.drop_m : null,
      peak_y: drop ? drop.peak_y : null,
      rest_y: drop ? drop.rest_y : null,
    });

    // The state the leg asked for: the mode, on foot, aboard or on the planet's ground.
    const modeOk = after.falls && after.falls.mode === leg.mode && before.falls && before.falls.mode === leg.mode;
    check(`${id}.mode`, modeOk, `asked ${leg.mode}; the game said ${before.falls && before.falls.mode} before, ${after.falls && after.falls.mode} after`);
    if (onPlanet) {
      const onGround = [before, after].every((r) => r.falls && r.falls.flying === false && r.falls.aboard === false && r.falls.planet === leg.body);
      check(
        `${id}.on_foot_on_planet`,
        onGround,
        onGround
          ? `walking (no dev flight) on ${leg.body}'s ground`
          : `flying ${before.falls && before.falls.flying}/${after.falls && after.falls.flying}, aboard ${before.falls && before.falls.aboard}/${after.falls && after.falls.aboard}, ` +
              `planet walk on ${before.falls && before.falls.planet}/${after.falls && after.falls.planet} (before/after), asked ${leg.body}: ` +
              "not the planet walk on foot, so this leg proves nothing about it (a fall in flight never hurts)"
      );
      // The camera park's pin is one of the planet walk's exemptions: while it holds, every fall
      // is exempt (and the walk cannot move the body at all). A rig's held key must let go of it,
      // as a person's does (engine/rig_walk.rs `hold`); a placement must start without it too.
      const pinned = [after, ...(isPlacement ? [before] : [])].some((r) => !r.falls || r.falls.probe_hold !== false);
      check(
        `${id}.park_released`,
        !pinned,
        pinned
          ? "the camera park still pinned the eye (probe_hold) " +
              (isPlacement ? "around the placement" : "after the rig held its key") +
              ": every planet fall is exempt while it holds, so this leg proves nothing. engine/rig_walk.rs `hold` must let go of the park the way a key press does"
          : "no camera park pinned the eye"
      );
      // The planet walk's own gravity, against the body's data file.
      const pg = data.planet || {};
      const gOk = Number.isFinite(gGame) && gGame > 0 && (pg.curved || Math.abs(gGame - num(pg.gravity_m_s2)) < 1e-3);
      check(
        `${id}.gravity`,
        gOk,
        `the planet walk's gravity ${f2(gGame)} m/s^2, data/planets/${leg.body}.ron ${f2(pg.gravity_m_s2)}` + (pg.curved ? " (a gravity curve: the game's reading stands)" : "")
      );
    } else {
      const onFoot = [before, after].every((r) => r.falls && r.falls.flying === false && r.falls.aboard === true);
      check(
        `${id}.on_foot_aboard`,
        onFoot,
        onFoot
          ? "walking (no dev flight), aboard"
          : `flying ${before.falls && before.falls.flying}/${after.falls && after.falls.flying}, aboard ${before.falls && before.falls.aboard}/${after.falls && after.falls.aboard} (before/after): a fall in flight never hurts, so this leg proves nothing`
      );
      check(`${id}.gravity`, Math.abs(gGame - gShip) < 1e-3, `the walk's gravity ${f2(gGame)} m/s^2, data/game.csv ${f2(gShip)}`);
    }
    // The gravity the camera's own speed is worked out with: aboard data/game.csv's (checked equal
    // above), on a planet the game's own there (checked against the planet's file above).
    const g = onPlanet ? gGame : gShip;
    if (!Number.isFinite(hb) || !Number.isFinite(ha) || !wa) {
      check(`${id}.body_reported`, false, "the probe reported no health or no wounds list: is the build older than src/engine/body_probe.rs?");
      continue;
    }
    if (after.vitals && after.vitals.registry === false) {
      check(`${id}.body_reported`, false, "the game had no status effect registry: no wound could be named, so 'no wound' means nothing");
      continue;
    }

    if (leg.kind === "ladder_hatch") {
      judgeLadderHatch({ check, id, leg, drop, data, origin: run.origin, lb, la, last, g, newWounds, hb, ha });
      if (drop) seen.peak_held_s = drop.peak_held_s;
      continue;
    }

    // The drop happened: without it, "no wound" is a pass that proves nothing.
    const minDrop = onPlanet && !isPlacement ? planetMinDrop(data.falls, g) : LIMITS.MIN_DROP_M;
    const dropped = drop && drop.drop_m >= minDrop;
    let startOk = isPlacement;
    let startNote = "";
    if (isDeck) {
      // Stood ON the deck before walking off it: the eye began at its highest (standing, not
      // falling), the deck's height over the floor it came to rest on after. The drop itself is
      // judged below like any fall's. Aboard the start is also where the rig stood it.
      const over = drop ? drop.first_y - drop.rest_y : NaN;
      const standing = drop && Math.abs(drop.peak_y - drop.first_y) <= LIMITS.PLANET_STAND_M;
      const onDeck = standing && Number.isFinite(deckM) && Math.abs(over - deckM) <= LIMITS.DECK_TOL_M;
      check(
        `${id}.stood_on_deck`,
        onDeck,
        !drop
          ? `the recording has under two frames${onPlanet ? " on the planet walk" : ""}: no camera track to read`
          : `before walking off, the eye stood ${f2(over)} m over the ${onPlanet ? "ground" : "floor"} it came to rest on (the deck is ${f2(deckM)} m, at most ${LIMITS.DECK_TOL_M} off)` +
              (onDeck
                ? ": on the deck"
                : !standing
                  ? `, but it was not standing there: it rose ${f2(drop.peak_y - drop.first_y)} m after the recording began`
                  : onPlanet
                    ? ": the walk did not stand on the built deck, it went through it to the ground (the planet walk's floor: planet_build::built_floor_radius, surface_move::walk_rest)"
                    : ": the body was not standing on the deck the rig placed (no deck there: see deck_placed; or the floor under the body does not count it: engine/footing.rs floor_under, ship::structure::walk_surface)")
      );
      startOk = onPlanet || (drop && Array.isArray(leg.start) && Math.hypot(drop.first_cam[0] - leg.start[0], drop.first_cam[2] - leg.start[2]) <= LIMITS.START_TOL_M);
      if (!onPlanet && drop) startNote = `, starting at (${drop.first_cam.map(f2).join(", ")}) for (${(leg.start || []).map(f2).join(", ")})`;
    } else if (drop && !isPlacement && onPlanet) {
      // Standing on the ground when the key went down, and back on the same ground after.
      startOk = Math.abs(drop.first_y - drop.rest_y) <= LIMITS.PLANET_STAND_M;
      startNote = `, from standing ${f2(drop.first_y - drop.rest_y)} m off where it came to rest (at most ${LIMITS.PLANET_STAND_M})`;
    } else if (drop && !isPlacement) {
      startOk = Array.isArray(leg.start) && Math.hypot(drop.first_cam[0] - leg.start[0], drop.first_cam[2] - leg.start[2]) <= LIMITS.START_TOL_M;
      startNote = `, starting at (${drop.first_cam.map(f2).join(", ")}) for (${(leg.start || []).map(f2).join(", ")})`;
    }
    check(
      `${id}.dropped`,
      dropped && startOk,
      !drop
        ? `the recording has under two frames${onPlanet ? " on the planet walk (no planet_r: a build older than the planet legs, or the player not on the ground)" : ""}: no camera track to read`
        : `the eye fell ${f2(drop.drop_m)} m (from ${onPlanet ? "r" : "y"} ${f2(drop.peak_y)} to ${f2(drop.rest_y)})` +
            startNote +
            (dropped
              ? ""
              : `: under ${f2(minDrop)} m, so ` +
                (isDeck
                  ? `the walk never went off the deck from standing on it (it stood on the ${onPlanet ? "ground" : "floor"} under the deck, or never reached its edge)`
                  : onPlanet
                    ? `the climb never lifted the body that high (Space held ${f2(leg.hold_s)} s at the ${f2(leg.climb_m_s)} m/s the game reported: a slower climb than reported, the key never reaching the walk, or a camera park's pin)`
                    : leg.kind === "stairs_down"
                      ? "the walk never went down the stairs (it never stood on the top tread, or never moved)"
                      : "the walk never went off the top") +
                " and this leg says nothing about falls")
    );
    if (!dropped) continue;
    const vCam = Math.sqrt(2 * g * drop.drop_m);
    seen.camera_m_s = vCam;
    seen.peak_held_s = drop.peak_held_s;

    if (leg.kind === "ladder") {
      // Let go of the climb key at the top: the body holds on there until it steps away.
      const need = LIMITS.HANG_SHARE * num(leg.hang_s);
      check(
        `${id}.held_on`,
        Number.isFinite(need) && drop.peak_held_s >= need,
        `the eye stayed at the top (y ${f2(drop.peak_y)}) for ${f2(drop.peak_held_s)} s; the rig let go of the climb key ${f2(leg.hang_s)} s before stepping away (at least ${f2(need)} s needed): ` +
          (drop.peak_held_s >= need ? "it held on" : "it dropped when the key came up, so the ladder does not hold a body that stops climbing")
      );
    }

    if (leg.kind === "stairs_down") {
      // Walked DOWN the stairs from the top tread: every riser is a step down, not a fall
      // (BUG-208), so nothing lands hard enough to count, and nothing hurts.
      check(
        `${id}.no_landing`,
        la === lb,
        la === lb
          ? `no landing reached the body (${lb} before and after) walking down ${f2(drop.drop_m)} m of stairs`
          : `a landing reached the body (${lb} -> ${la}, the last at ${f2(last)} m/s) walking DOWN the stairs: the walk left a tread instead of stepping down it ` +
              `(a ${f2(drop.drop_m)} m flight lands at ${f2(vCam)} m/s if it is all one fall). The aboard walk's step down (BUG-208, renderer/camera.rs, STEP_DOWN_M)`
      );
      check(`${id}.no_wound`, newWounds.length === 0, newWounds.length ? `new wound(s) ${newWounds.join(", ")} from walking down stairs` : "no new wound");
      check(`${id}.health`, Math.abs(hb - ha) <= LIMITS.HEALTH_TOL, `health ${f2(hb)} -> ${f2(ha)}`);
      continue;
    }

    if (isPlacement) {
      // Placed, then fell: the watch must keep this landing from the body.
      check(
        `${id}.no_landing`,
        la === lb,
        la === lb
          ? `no landing reached the body (${lb} before and after) for a ${f2(drop.drop_m)} m drop that would land at ${f2(vCam)} m/s`
          : `a landing reached the body (${lb} -> ${la}, the last at ${f2(last)} m/s) after a PLACEMENT: ` +
              (onPlanet ? "the planet walk's placement test (CameraController::surface_step, SURFACE_PLACED_M) let a teleport's drop hurt" : "FallWatch let a teleport's drop hurt (src/systems/falls.rs)")
      );
      check(`${id}.no_wound`, newWounds.length === 0, newWounds.length ? `new wound(s) ${newWounds.join(", ")} from a placement` : "no new wound");
      check(`${id}.health`, Math.abs(hb - ha) <= LIMITS.HEALTH_TOL, `health ${f2(hb)} -> ${f2(ha)}`);
      continue;
    }

    // A fall the player made: one landing, at the camera's speed, and what falls.ron says of it.
    const handed = la === lb + 1 && Number.isFinite(last);
    check(
      `${id}.landing_handed`,
      handed,
      handed
        ? `one landing reached the body (${lb} -> ${la}) at ${f2(last)} m/s`
        : `${Number.isFinite(la) && Number.isFinite(lb) ? `${la - lb} landings` : "no landing count"} reached the body for a ${f2(drop.drop_m)} m drop (expected exactly 1). ` +
            (onPlanet
              ? "The planet walk's landing never got to player_landed: the lib.rs hand-off after radial_step (CameraController::surface_step), an exemption left on (dev flight, the flight band, the camera park, water), or radial_step reporting no impact"
              : "The walk's landing never got to player_landed: the lib.rs hand-off (take_landing -> falls::player_landed), FallWatch treating the walk as a placement, or the walk reporting no impact")
    );
    if (!handed) {
      // Still judge the body against what the drop should have done, so the report says what it cost.
      const e = expectFall(data.falls, vCam, leg.mode);
      check(`${id}.wound`, sameSet(newWounds, e.wound ? [e.wound] : []), woundDetail(newWounds, e, effects));
      check(`${id}.health`, Math.abs(hb - ha - e.harm) <= LIMITS.HEALTH_TOL, healthDetail(hb, ha, e));
      continue;
    }
    const tol = Math.max(LIMITS.SPEED_TOL_M_S, vCam * LIMITS.SPEED_TOL_SHARE);
    check(`${id}.landing_speed`, Math.abs(last - vCam) <= tol, `the game landed at ${f2(last)} m/s; the camera's ${f2(drop.drop_m)} m drop at ${f2(g)} m/s^2 is ${f2(vCam)} m/s (tolerance ${f2(tol)})`);
    // What falls.ron says that landing does, from the speed the body was given.
    const e = expectFall(data.falls, last, leg.mode);
    seen.expected = e;
    check(`${id}.wound`, sameSet(newWounds, e.wound ? [e.wound] : []), woundDetail(newWounds, e, effects));
    const named = (after.vitals.wounds || []).filter((w) => newWounds.includes(w.id)).every((w) => !effects[w.id] || w.name === effects[w.id].name);
    check(`${id}.wound_named`, named, named ? "each new wound carries its name from status_effects.csv" : `a wound's name differs from status_effects.csv: ${JSON.stringify(after.vitals.wounds)}`);
    check(`${id}.health`, Math.abs(hb - ha - e.harm) <= LIMITS.HEALTH_TOL, healthDetail(hb, ha, e));
    if (e.harm > LIMITS.HEALTH_TOL) {
      // The health fell AT the landing, not before it and not much later.
      const fell = drop.frames.find((f) => Number.isFinite(num(f.health)) && num(f.health) < hb - e.harm / 2);
      const at = fell ? num(fell.t) : NaN;
      const ok = fell && Number.isFinite(drop.landed_t) && at >= drop.landed_t - 1e-6 && at - drop.landed_t <= LIMITS.HARM_WITHIN_S;
      check(
        `${id}.harm_at_landing`,
        ok,
        fell
          ? `the health fell at t ${f2(at)} s; the eye landed at t ${f2(drop.landed_t)} s (within ${LIMITS.HARM_WITHIN_S} s after, never before)`
          : "no recorded frame shows the health falling (is the build older than the per-frame health?)"
      );
    }
  }

  check("panics", num(run.panics) === 0, `${num(run.panics)} PANIC line(s) in run.log`);
  const builtin = Array.isArray(run.builtin_data) ? run.builtin_data : [];
  check("no_builtin_data", builtin.length === 0, builtin.length ? `served from the copy built into the exe: ${builtin.slice(0, 3).join(" | ")}` : "every data file read from the tree");
  return { checks, ok: checks.every((c) => c.ok), observed };
}

/**
 * The ladder through the hatch (ship homes increment 7, `realistic_ladder` since 2026-10-21): the
 * climber stood in the hatch's hole facing the ladder, Space held to its top, then forward held off
 * the top across the gate (engine/footing.rs `ladder_gates`, open to a body ON the ladder) onto the
 * upper floor. Judged on the camera track and the body:
 *   climbed         the eye reached the ladder's top (its hold, top + eye + 0.2 m);
 *   on_upper_floor  the last frame stands on the upper floor: the eye at the floor's height + eye,
 *                   over a slab of that height, past the ladder's line (the gate's) the way off it.
 *                   A gate that does not let the climber through leaves it hanging at the top over
 *                   the hole; a body that falls down the hole ends on the floor below;
 *   no_fall         no landing harder than the step from the top rung's hold down onto the floor
 *                   (0.2 m: 1.98 m/s, which the walk hands to the body as a landing that cannot
 *                   hurt). A fall from the top down the hole lands at about 7.9 m/s;
 *   no_wound, health  nothing hurt.
 * `origin`: the home's corner in the camera's frame (the manifest's).
 */
function judgeLadderHatch({ check, id, leg, drop, data, origin, lb, la, last, g, newWounds, hb, ha }) {
  const L = data.ladder || null;
  const H = L && L.hatch;
  if (!H) {
    check(`${id}.hatch`, false, "the run's tree data has no ladder climbing through a hatch to a floor at its top (data/homes/homestead.ron's floors and ladder-1)");
    return;
  }
  const o = Array.isArray(origin) ? origin.map(Number) : [0, 0, 0];
  if (!drop) {
    check(`${id}.climbed`, false, "the recording has under two frames: no camera track to read");
    return;
  }
  const hang = o[1] + L.hang_eye_m;
  const floorEye = o[1] + H.floor_eye_m;
  check(
    `${id}.climbed`,
    drop.peak_y >= hang - 0.1,
    `the eye rose to y ${f2(drop.peak_y)} at its highest; the ladder's top holds it at ${f2(hang)}` +
      (drop.peak_y >= hang - 0.1 ? "" : `: the climb never reached the top (Space held ${f2(leg.climb_s)} s at the ladder, from (${(leg.start || []).map(f2).join(", ")}))`)
  );
  const end = drop.frames[drop.frames.length - 1].cam;
  const [hx, hz] = [end[0] - o[0], end[2] - o[2]];
  const past = (hx - L.pos[0]) * H.dir[0] + (hz - L.pos[2]) * H.dir[1];
  const standing = Math.abs(end[1] - floorEye) <= 0.05;
  const overFloor = floorAt(data.floors, H.floor_m, hx, hz);
  const ok = standing && overFloor && past > 0;
  const at = `(${end.slice(0, 3).map(f2).join(", ")})`;
  check(
    `${id}.on_upper_floor`,
    ok,
    ok
      ? `it ended standing on the upper floor at ${at}, ${f2(past)} m past the ladder's top the way off it: the hatch's gate let the climber through`
      : Math.abs(end[1] - hang) <= 0.05 && !overFloor
        ? `it ended hanging at the ladder's top over the hatch at ${at}: the gate across the ladder's top did not let the climber through onto the floor (engine/footing.rs ladder_gates; shut_gates leaves it out only for a body on the ladder)`
        : Math.abs(end[1] - (o[1] + L.pos[1] + EYE_DEFAULT_M)) <= 0.1
          ? `it ended back on the floor below at ${at}: it fell down the hatch, or let go of the ladder`
          : `it ended at ${at}, not standing on the upper floor (the eye at y ${f2(floorEye)}, over a slab at ${f2(H.floor_m)} m, past the ladder's line the way off it)`
  );
  const stepM = L.hang_eye_m - H.floor_eye_m;
  const vStep = Math.sqrt(2 * g * Math.max(0, stepM));
  const tol = Math.max(LIMITS.SPEED_TOL_M_S, vStep * LIMITS.SPEED_TOL_SHARE);
  const n = la - lb;
  const soft = n === 0 || (n === 1 && Number.isFinite(last) && last <= vStep + tol);
  const vHole = Math.sqrt(2 * g * (L.hang_eye_m - (L.pos[1] + EYE_DEFAULT_M)));
  check(
    `${id}.no_fall`,
    soft,
    n === 0
      ? `no landing reached the body (${lb} before and after)`
      : soft
        ? `one landing reached the body, at ${f2(last)} m/s: the ${f2(stepM)} m step from the top rung's hold down onto the floor (at most ${f2(vStep + tol)} m/s), which cannot hurt`
        : `${Number.isFinite(n) ? n : "?"} landing(s) reached the body, the last at ${f2(last)} m/s: harder than the ${f2(stepM)} m step off the top (at most ${f2(vStep + tol)} m/s). A fall from the top down the hatch lands at ${f2(vHole)} m/s`
  );
  check(`${id}.no_wound`, newWounds.length === 0, newWounds.length ? `new wound(s) ${newWounds.join(", ")} from climbing a ladder` : "no new wound");
  check(`${id}.health`, Math.abs(hb - ha) <= LIMITS.HEALTH_TOL, `health ${f2(hb)} -> ${f2(ha)}`);
}

/** The eye's height over the feet the judge assumes when the data does not say (src/renderer/camera.rs `eye_height`). */
const EYE_DEFAULT_M = 1.7;

/** The least drop a planet fall leg must make at gravity `g`: LIMITS.PLANET_MIN_DROP_SHARE over the
 *  height whose landing reaches falls.ron's first harm (v^2 / 2 g). */
function planetMinDrop(falls, g) {
  const v = falls.harm.length ? falls.harm[0].m_s : 0;
  return (LIMITS.PLANET_MIN_DROP_SHARE * v * v) / (2 * Math.max(1e-6, g));
}

const sameSet = (a, b) => a.length === b.length && a.every((x) => b.includes(x));
const nameOf = (effects, id) => (effects[id] ? `${effects[id].name} (${id})` : id);

function woundDetail(newWounds, e, effects) {
  const want = e.wound ? nameOf(effects, e.wound) : "no wound";
  const got = newWounds.length ? newWounds.map((w) => nameOf(effects, w)).join(", ") : "no wound";
  return `read at ${f2(e.read)} m/s falls.ron gives ${want}; the body has ${got}`;
}

function healthDetail(hb, ha, e) {
  return `health ${f2(hb)} -> ${f2(ha)}, took ${f2(hb - ha)}; falls.ron at ${f2(e.read)} m/s takes ${f2(e.harm)} (tolerance ${LIMITS.HEALTH_TOL})`;
}

/** The verdict lines a rig prints. */
function verdictLines(result) {
  const lines = [];
  for (const c of result.checks) lines.push(`${c.ok ? "PASS" : "FAIL"}  ${c.id.padEnd(32)} ${c.detail}`);
  const failed = result.checks.filter((c) => !c.ok).map((c) => c.id);
  lines.push("");
  lines.push(
    failed.length
      ? `FAIL  ${result.checks.length - failed.length}/${result.checks.length} passed; failed: ${failed.join(", ")}`
      : `PASS  ${result.checks.length}/${result.checks.length}: falls hurt in the running game the way falls.ron says, in both modes, ` +
          `${Object.keys(result.observed).some((k) => k.startsWith("planet_")) ? "aboard and on a planet's ground, " : ""}and a placement's drop hurt nobody`
  );
  return lines;
}

module.exports = {
  LIMITS,
  PLANET,
  ABOARD_LEGS,
  PLANET_LEGS,
  STAIRS_LEGS,
  DECK_LEGS,
  DECK,
  ALL_LEGS,
  ABOARD_DROP,
  PLANET_DROP,
  planetMinDrop,
  readPlanetGravity,
  readFalls,
  readGravity,
  readEffects,
  readStructureTypes,
  readHomeStructures,
  readHomeFloors,
  readHomeWalls,
  readHomeZones,
  readHomeHeight,
  floorAt,
  pickStairs,
  pickLadder,
  pickDeck,
  ABOARD_DECK,
  pickAboardDeck,
  readTreeData,
  readSpeed,
  harmAt,
  woundAt,
  expectFall,
  dropOf,
  judgeFalls,
  verdictLines,
};
