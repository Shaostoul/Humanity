// THE STOREYS JUDGE (2026-10-21, `just verify-storeys`): in the RUNNING game, does the homestead's
// upper floor work the way ship homes increment 7 built it? You walk in at the front door and up
// the stair to a floor you stand on, the HUD names the room upstairs, a wall upstairs stops you
// upstairs and not under it, a jump under the floor bumps its head, the elevator rides between
// the storeys and comes when called, the ladder climbs through its hatch, and a guard holds you
// at the stairwell's edge.
//
// WHY. Increment 7 (docs/design/ship-homes-increment-7-plan.md; the layout as built:
// docs/design/homestead-upper-floor.md) landed in five lanes with unit tests over a small
// synthetic two-storey home (engine/footing.rs `test_aboard`), and nothing that walked the real
// homestead in the running game. A stair that ends in air, slabs that never reach the walk, a HUD
// that names the room downstairs, a landing door that opens onto an empty shaft: every one of
// them leaves those tests green. The house rule (memory feedback_verify_on_player_state): verify
// on the state the player actually reaches. The class this closes: a feature proven on a
// synthetic fixture and never on the shipped data in the running game.
//
// Pure: `judgeStoreys` reads one run's record (the manifest scripts/verify-storeys.js writes) and
// returns checks, so it is tested with made-up runs (scripts/tests/storeys-judge.test.js, in
// `just rig-tests`) and re-run on a saved run without booting anything
// (`node scripts/verify-storeys.js --dry-verdict <manifest.json>`).
//
// WHAT IT JUDGES AGAINST: the tree's own data, read when the run is made (`readTreeData`) and kept
// in the manifest's `plan` block (`buildPlan`), so a saved run is judged against what it ran on:
// data/homes/homestead.ron (the stair, the elevator, the ladder, the floor slabs, the zones, the
// walls and guards, the front door), data/blueprints/structure_types.ron (the pieces' sizes),
// data/blueprints/wall_materials.ron (which walls are guards), data/rooms.ron (the name the HUD
// shows for each room type), and three constants read out of the code that the walk itself uses:
// the head allowance (renderer/camera.rs HEAD_ABOVE_EYE_M), the body's radius and its eye height
// (ship/wall_collision.rs PLAYER_RADIUS, EYE_HEIGHT_M).
//
// THE ROUTE (`ROUTE` below, in the homestead's own metres: x east, z south, from the box's
// corner) is the few choices the data cannot make: which wall upstairs to walk into, where to
// jump, which guard to lean on. `buildPlan` checks every one against the data before anything
// boots: each named piece, zone and wall exists where the route expects it, every straight leg of
// a walked route crosses no wall of its storey except through a door, the jump points are under a
// slab and under none, the wall chosen upstairs has no wall under it downstairs. A layout change
// that moves any of them refuses the run with the reason, instead of failing a leg for a reason
// nobody can read.
//
// WHAT THE GAME REPORTS (src/engine/storey_probe.rs): every recorder frame's `storey` block (the
// floor the body stands on, the slab underside over its head, on a ladder or at one, riding a
// car, the room the HUD's room card names, by id and name and floor, whether the HUD is drawn,
// every elevator car's height, the landings handed to the body so far and the last one's speed),
// and the done JSON's `storeys` block (each elevator's id, base and stops). Every height the
// game reports is world metres; the judge takes the home's box corner (`origin`) off.

"use strict";

const path = require("path");
const FJ = require("./falls-judge.js");

/// The limits, each with its reason.
const LIMITS = {
  /// Floors are data heights the walk sets exactly (a slab's top, a room's floor, a car's floor).
  FLOOR_TOL_M: 0.02,
  /// How near the requested start a leg's first frame must stand, metres.
  START_TOL_M: 0.6,
  /// Faster than this across the floor between two frames is not a walk (the walk is 5 m/s, and a
  /// sprint the rig never holds 9.5): something placed the body there.
  WALK_MAX_M_S: 6.5,
  /// Two recordings of one walked route must join: the body may not move between them, since the
  /// rig only turns the camera there (`face`).
  SEG_JOIN_M: 0.05,
  /// A body held by a wall or a gate stands its radius plus the blocker's half thickness off its
  /// line (each recorded frame is after the sweep's push-out), so it is never nearer than the
  /// radius less this. Tight on purpose: at the ladder hatch the ends of the guards either side of
  /// the gap (0.6 m until 2026-10-21, 0.8 m since) held a body about 0.24 m off the gate's line on
  /// their own, and a looser slack would credit the gate with what guards like those did.
  SIDE_SLACK_M: 0.02,
  /// A body walked into a blocker ends within this of its line: it reached it and was stopped there,
  /// not short of it.
  REACH_M: 0.75,
  /// And moved at least this toward it first: it was walking.
  PUSHED_M: 0.1,
  /// A line the body is meant to cross freely it ends at least this far past.
  PAST_M: 0.5,
  /// The head clamp holds the eye at the slab's underside less the head allowance; a frame may sit
  /// this far over it (float) and a jump must reach within CLAMP_REACH_M of it to count.
  CLAMP_TOL_M: 0.01,
  CLAMP_REACH_M: 0.08,
  /// A jump with nothing over it must rise at least this far past where the clamp holds one under
  /// the slab: so the clamp, not a weak jump, is what held the first. Not the full 0.34 m an ideal
  /// jump has over the clamp (1.27 m against 0.93): the walk integrates a jump frame by frame
  /// (velocity, then height), which peaks about half a frame's rise short of the ideal, 0.08 m at
  /// 30 frames a second and 0.25 m at 10 (the first green run's open jump rose 1.19 m). At this
  /// margin a clamped jump (held within CLAMP_TOL_M of the clamp) still cannot pass for an open one.
  OPEN_JUMP_OVER_M: 0.08,
  /// A car within this of a stop is at that landing (engine/footing.rs CAR_AT_STOP_M).
  CAR_AT_STOP_M: 0.02,
  /// Riding the car, Space chooses the floor and does not jump: the eye may sit no higher than
  /// this over the car's floor plus the eye height.
  RIDE_JUMP_M: 0.05,
  /// At the upper landing, the body must have stood at the shut landing door at least this long
  /// while the car came: so the door held it, not merely a car that arrived first.
  GATE_HELD_S: 0.5,
  /// A landing from less than this hurts nobody (the falls rules: a half-metre step and a jump hurt
  /// nobody, systems/wounds_tests.rs). The step off a ladder's top is 0.2 m.
  SAFE_DROP_M: 0.5,
  /// The climb holds the eye at the ladder's top plus the eye plus 0.2 m (renderer/camera.rs); it
  /// must reach within this of it.
  LADDER_TOP_TOL_M: 0.1,
  /// Every point where the HUD's room is checked stands at least this far from every wall of its
  /// storey (buildPlan refuses a route that does not). A room's box is the 0.5 m cells its flood
  /// reached, and the cells within half a wall plus 0.25 m of a wall are closed to it
  /// (ship/home_structure.rs `detect_rooms`), so the box stops about half a metre short of each
  /// wall and a body there is in no room at all: the HUD shows no name (seen in the running game,
  /// 2026-10-21: on the lift lobby's floor 0.47 m from its south wall).
  HUD_WALL_MARGIN_M: 0.6,
};

/// The route's choices, in the homestead's own metres (docs/design/homestead-upper-floor.md
/// sections 2 to 5). Everything else is read from the data.
const ROUTE = {
  /// The pieces walked: the house stair, the lift and the ladder into the hatch.
  stairs: "stairs-1",
  elevator: "elevator-1",
  ladder: "ladder-1",
  /// The rooms the HUD must name (zone ids; the name shown comes from their room_type in rooms.ron).
  upper_hall: "room-upper-hall",
  bedroom: "room-bedroom",
  lift_lobby: "room-lift-lobby",
  /// From the stair-top landing to the bedroom: south onto the gallery (its middle, between the
  /// guard G1 at z 39.3 and the upper hall's south wall U3 at 40.5), east along it to the bedroom
  /// door's middle (U4's door at x 41.0 to 41.9), north through it, and on into the middle of the
  /// room (away from its walls: see HUD_WALL_MARGIN_M).
  gallery_z: 39.9,
  bedroom_door_x: 41.45,
  bedroom_in: [
    [41.45, 36.4],
    [39.5, 35.0],
  ],
  /// THE WALL UPSTAIRS (U7, concrete, between the dressing room and the bathroom and wet room):
  /// the ground walls that stood on its line were deleted when the second bedroom took that space,
  /// so downstairs the same line is open floor. Walked into from the dressing room facing east;
  /// walked across downstairs, in the second bedroom, facing east.
  wall: { a: [45.0, 30.0], b: [45.0, 38.0] },
  wall_up_from: [43.5, 34.0],
  wall_down_from: [43.2, 36.8],
  wall_down_to: [47.2, 36.8],
  /// A jump under a solid slab (the ground hall, under the gallery's floor) and the same jump with
  /// nothing over it (the greenhouse, open to the 6 m roof).
  jump_under: [44.0, 40.0],
  jump_open: [30.0, 62.0],
  /// THE GUARD (G1, oak balustrade): the stairwell's long edge along the gallery. Walked into from
  /// the gallery facing north, over the open stairwell.
  guard: { a: [36.5, 39.3], b: [40.5, 39.3] },
  guard_from: [38.5, 40.0],
  /// The elevator: approached from the service way south of its shaft; stepped out of into the
  /// lift lobby south of the shaft; called from the lobby, from far enough away (2.3 m) that the
  /// car is not called until the body walks up to the landing door.
  lift_from: [31.0, 28.4],
  lift_out: [31.0, 28.4],
  lift_call_from: [31.0, 29.2],
  /// The ladder: climbed from inside the hatch (north of the ladder, which stands on the hatch's
  /// south edge), then walked off south over its top through the guards' 0.8 m gap onto the lobby
  /// floor, and west out of the ladder's reach (1.2 m from it: within it a body off the floor holds
  /// on, 0.2 m up), into the lobby's middle. Straight south out of reach would end within half a
  /// metre of the lobby's south wall, where the HUD names no room (the final green attempt of
  /// 2026-10-21 stepped off at z 29.53 and failed on that). Then, from `hatch_from` in line with
  /// the ladder, walked north into the hatch's gap, where the gate across its top must hold it.
  ladder_from: [33.0, 27.4],
  ladder_off: [
    [33.0, 28.8],
    [31.8, 28.8],
  ],
  hatch_from: [33.0, 29.2],
  /// The screenshots, home metres and radians (yaw 0 north, +PI/2 east, pitch up positive): the
  /// upper hall from the stair-top landing, east over the stairwell and its guards; the bedroom
  /// from inside its door; the ground hall looking up the stair.
  shots: [
    { id: "upper_hall", at: [35.4, 38.75], storey: "upper", yaw: Math.PI / 2, pitch: -0.22, note: "the upper hall from the stair-top landing, east over the stairwell and its guards G1 and G2" },
    { id: "bedroom", at: [41.45, 36.0], storey: "upper", yaw: -Math.PI / 2 - 0.35, pitch: -0.12, note: "the upstairs bedroom from inside its door" },
    { id: "ground_hall_up_stairs", at: [44.6, 38.7], storey: "ground", yaw: -Math.PI / 2, pitch: 0.32, note: "the ground hall looking west up the stair, the slab's edge over its foot and the stairwell open above" },
  ],
};

// ── Reading the tree's data (pure: each reader takes a file's text) ─────────────────────────────

const uncomment = (text) => String(text || "").replace(/\/\/[^\n]*/g, "");

/** The text of the `[...]` list that follows the first `key:` in `text` (balanced), or null. */
function listAfter(text, key) {
  const m = new RegExp(`\\b${key}\\s*:\\s*\\[`).exec(text);
  if (!m) return null;
  const open = text.indexOf("[", m.index);
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    if (text[i] === "[") depth++;
    else if (text[i] === "]" && --depth === 0) return text.slice(open + 1, i);
  }
  return null;
}

/** The top-level `( ... )` items of a list body. */
function items(body) {
  const out = [];
  let depth = 0;
  let start = -1;
  for (let i = 0; i < (body || "").length; i++) {
    const c = body[i];
    if (c === "(") {
      if (depth === 0) start = i;
      depth++;
    } else if (c === ")") {
      depth--;
      if (depth === 0 && start >= 0) out.push(body.slice(start + 1, i));
    }
  }
  return out;
}

const tuple = (s, key) => {
  const m = String(s).match(new RegExp(`\\b${key}\\s*:\\s*\\(([^)]*)\\)`));
  return m ? m[1].split(",").map((v) => Number(v.trim())) : null;
};
const numberOf = (s, key) => {
  const m = String(s).match(new RegExp(`\\b${key}\\s*:\\s*(-?[\\d.]+)`));
  return m ? Number(m[1]) : null;
};
const stringOf = (s, key) => {
  const m = String(s).match(new RegExp(`\\b${key}\\s*:\\s*(?:Some\\()?"([^"]*)"`));
  return m ? m[1] : null;
};
const wordOf = (s, key) => {
  const m = String(s).match(new RegExp(`\\b${key}\\s*:\\s*(?:Some\\()?(\\w+)`));
  return m ? m[1] : null;
};

/**
 * A home design (data/homes/<kind>.ron): its body's height, spawn (the front door), placed
 * structures, zones, floor slabs and interior walls with their openings. Every number is in the
 * home's own metres.
 */
function readHomeDesign(text) {
  const t = uncomment(text);
  const body = t.slice(Math.max(0, t.search(/\bbody\s*:\s*\(/)));
  const head = body.split(/\bwalls\s*:/)[0];
  const walls = items(listAfter(body, "walls")).map((w) => {
    const own = w.split(/\bopenings\s*:/)[0];
    return {
      a: tuple(own, "a"),
      b: tuple(own, "b"),
      base: numberOf(own, "base") || 0,
      height: numberOf(own, "height"),
      material: numberOf(own, "material"),
      openings: items(listAfter(w, "openings")).map((o) => ({
        kind: wordOf(o, "kind"),
        at: numberOf(o, "at"),
        width: numberOf(o, "width"),
        sill: numberOf(o, "sill") || 0,
        height: numberOf(o, "height"),
      })),
    };
  });
  const floors = items(listAfter(body, "floors")).map((f) => ({
    a: tuple(f, "a"),
    b: tuple(f, "b"),
    top: numberOf(f, "top"),
    thickness: numberOf(f, "thickness") === null ? 0.25 : numberOf(f, "thickness"),
    material: wordOf(f, "material") || "Solid",
    label: stringOf(f, "label"),
  }));
  const zones = items(listAfter(body, "zones")).map((z) => ({
    id: stringOf(z, "id"),
    origin: tuple(z, "origin"),
    size: tuple(z, "size"),
    label: stringOf(z, "label"),
    room_type: stringOf(z, "room_type"),
  }));
  const structures = FJ.readHomeStructures(body);
  const spawn = body.match(/\bspawn\s*:\s*Some\(\s*\(([^)]*)\)\s*\)/);
  return {
    height: numberOf(head, "height"),
    spawn: spawn ? spawn[1].split(",").map((v) => Number(v.trim())) : null,
    structures,
    zones,
    floors,
    walls,
  };
}

/** data/rooms.ron: room type key -> the name the HUD shows. */
function readRoomNames(text) {
  const t = uncomment(text);
  const out = {};
  for (const m of t.matchAll(/"([\w-]+)"\s*:\s*\(\s*name\s*:\s*"([^"]*)"/g)) out[m[1]] = m[2];
  return out;
}

/** data/blueprints/wall_materials.ron: the ids of the materials that are guards (`form: Some("guard")`). */
function readGuardMaterials(text) {
  const out = [];
  for (const line of uncomment(text).split(/\r?\n/)) {
    const id = line.match(/\(\s*id\s*:\s*(\d+)/);
    if (id && /\bform\s*:\s*Some\(\s*"guard"\s*\)/.test(line)) out.push(Number(id[1]));
  }
  return out;
}

/** A `const NAME: f32 = <number>;` out of a source file's text, or null. */
function readConst(text, name) {
  const m = String(text || "").match(new RegExp(`\\bconst\\s+${name}\\s*:\\s*f32\\s*=\\s*(-?[\\d.]+)\\s*;`));
  return m ? Number(m[1]) : null;
}

/** Everything the plan and the judge need from the tree. `readFile` is injectable for tests. */
function readTreeData(repo, readFile = (p) => require("fs").readFileSync(p, "utf8")) {
  const rd = (...p) => readFile(path.join(repo, ...p));
  const camera = rd("src", "renderer", "camera.rs");
  const walls = rd("src", "ship", "wall_collision.rs");
  return {
    home: readHomeDesign(rd("data", "homes", "homestead.ron")),
    types: FJ.readStructureTypes(rd("data", "blueprints", "structure_types.ron")),
    room_names: readRoomNames(rd("data", "rooms.ron")),
    guard_materials: readGuardMaterials(rd("data", "blueprints", "wall_materials.ron")),
    gravity_m_s2: FJ.readGravity(rd("data", "game.csv")),
    head_m: readConst(camera, "HEAD_ABOVE_EYE_M"),
    radius_m: readConst(walls, "PLAYER_RADIUS"),
    eye_m: readConst(walls, "EYE_HEIGHT_M"),
  };
}

// ── Geometry (home metres; a piece's local +z maps to (sin r, cos r), engine/footing.rs) ────────

/** The world (home) floor point of a piece's local (lx, lz), placed at `pos`, turned `yawRad`. */
function toWorld(pos, yawRad, lx, lz) {
  const s = Math.sin(yawRad);
  const c = Math.cos(yawRad);
  return [pos[0] + lx * c + lz * s, pos[2] - lx * s + lz * c];
}
/** The floor point (x, z) in a piece's own frame. */
function toLocal(pos, yawRad, x, z) {
  const s = Math.sin(yawRad);
  const c = Math.cos(yawRad);
  const dx = x - pos[0];
  const dz = z - pos[2];
  return [dx * c - dz * s, dx * s + dz * c];
}
/** How far the floor point is outside a piece's footprint (0 inside it). */
function outsideFootprint(fp, x, z) {
  const [lx, lz] = toLocal(fp.pos, fp.yaw, x, z);
  const ex = Math.max(0, Math.abs(lx) - fp.w / 2);
  const ez = Math.max(0, Math.abs(lz) - fp.d / 2);
  return Math.hypot(ex, ez);
}
/** Whether the floor point is within `margin` of a piece's footprint on each of its axes: the
 *  elevator's call reach (engine/footing.rs `Lift::reaches`, a box, not a circle). */
function inReach(fp, x, z, margin) {
  const [lx, lz] = toLocal(fp.pos, fp.yaw, x, z);
  return Math.abs(lx) <= fp.w / 2 + margin && Math.abs(lz) <= fp.d / 2 + margin;
}
/// The elevator's call reach past its shaft's edge, metres (engine/footing.rs CALL_REACH_M).
const CALL_REACH_M = 1.0;
/// How much wider than its ladder a hatch's gate is each side, metres (engine/footing.rs
/// HATCH_SIDE_CLEAR_M): the gap the guards leave over the 0.6 m ladder is 0.8 m.
const HATCH_SIDE_CLEAR_M = 0.1;
const slabContains = (s, x, z) => x >= Math.min(s.a[0], s.b[0]) && x <= Math.max(s.a[0], s.b[0]) && z >= Math.min(s.a[1], s.b[1]) && z <= Math.max(s.a[1], s.b[1]);
const zoneContains = (zn, x, y, z) =>
  x >= zn.origin[0] && x <= zn.origin[0] + zn.size[0] && z >= zn.origin[2] && z <= zn.origin[2] + zn.size[2] && y >= zn.origin[1] - 1e-6 && y < zn.origin[1] + zn.size[1];
/** Signed distance of (x, z) from the line through a and b (positive on its left, looking a->b). */
function sideOf(a, b, x, z) {
  const dx = b[0] - a[0];
  const dz = b[1] - a[1];
  const len = Math.hypot(dx, dz) || 1;
  return ((x - a[0]) * dz - (z - a[1]) * dx) / len;
}
/** Where along a->b (0..1) the point's projection falls. */
function alongOf(a, b, x, z) {
  const dx = b[0] - a[0];
  const dz = b[1] - a[1];
  return ((x - a[0]) * dx + (z - a[1]) * dz) / (dx * dx + dz * dz || 1);
}
/** Distance from (x, z) to the segment a..b. */
function segDist(a, b, x, z) {
  const t = Math.max(0, Math.min(1, alongOf(a, b, x, z)));
  return Math.hypot(x - (a[0] + t * (b[0] - a[0])), z - (a[1] + t * (b[1] - a[1])));
}
/** The yaw that faces from p to q (camera.rs: yaw a looks along (sin a, -cos a)). */
const yawTo = (p, q) => Math.atan2(q[0] - p[0], -(q[1] - p[1]));

/** Whether segment p->q crosses segment a->b, and where along a->b (metres from a). */
function crossing(p, q, a, b) {
  const r = [q[0] - p[0], q[1] - p[1]];
  const s = [b[0] - a[0], b[1] - a[1]];
  const den = r[0] * s[1] - r[1] * s[0];
  if (Math.abs(den) < 1e-9) return null;
  const t = ((a[0] - p[0]) * s[1] - (a[1] - p[1]) * s[0]) / den;
  const u = ((a[0] - p[0]) * r[1] - (a[1] - p[1]) * r[0]) / den;
  if (t < 0 || t > 1 || u < 0 || u > 1) return null;
  return u * Math.hypot(s[0], s[1]);
}

/** Does a wall stand across the knee-to-eye band of a body on `floor` (ship/wall_collision.rs
 *  `stops_body`: from 0.4 m over the feet to the eye)? */
const wallStops = (w, floor, eye) => w.base < floor + eye && w.base + w.height > floor + 0.4;

/** The walls of the home a walk on `floor` from p to q would walk into: crossed anywhere but
 *  through a door with a body's radius to spare either side. [] when the way is clear. */
function wallsCrossed(home, floor, eye, radius, p, q) {
  const hit = [];
  for (const w of home.walls) {
    if (!w.a || !w.b || !wallStops(w, floor, eye)) continue;
    const at = crossing(p, q, w.a, w.b);
    if (at === null) continue;
    const door = w.openings.some((o) => o.kind === "Door" && at >= o.at + radius && at <= o.at + o.width - radius);
    if (!door) hit.push(w);
  }
  return hit;
}

const fmtPt = (p) => `(${p.map((v) => Number(v).toFixed(2)).join(", ")})`;
const wallName = (w) => `${fmtPt(w.a)} to ${fmtPt(w.b)} (base ${w.base}, ${w.height} m)`;

/**
 * Everything a run walks, read from the tree's data and the route's choices, in home metres:
 * { plan, problems }. Each problem names what the layout no longer has where the route expects
 * it (the rig refuses to boot with any). The plan is what the rig walks and the judge judges.
 */
function buildPlan(tree) {
  const problems = [];
  const home = tree.home;
  const eye = tree.eye_m;
  const radius = tree.radius_m;
  if (!(eye > 0) || !(radius > 0) || !(tree.head_m > 0)) {
    problems.push("could not read EYE_HEIGHT_M and PLAYER_RADIUS (src/ship/wall_collision.rs) or HEAD_ABOVE_EYE_M (src/renderer/camera.rs)");
  }
  const piece = (id, kind) => {
    const s = home.structures.find((x) => x.id === id);
    const ty = s && tree.types[s.type_id];
    if (!s) problems.push(`data/homes/homestead.ron has no structure "${id}"`);
    else if (!ty || ty.kind !== kind || !Array.isArray(ty.size)) problems.push(`"${id}" is not a ${kind} the structure types know (type "${s.type_id}")`);
    return s && ty && Array.isArray(ty.size) ? { id, pos: s.pos, rot_deg: s.rot_deg, yaw: (s.rot_deg * Math.PI) / 180, w: ty.size[0], h: ty.size[1], d: ty.size[2], steps: ty.steps } : null;
  };
  const zone = (id) => {
    const z = home.zones.find((x) => x.id === id);
    if (!z) problems.push(`data/homes/homestead.ron has no zone "${id}"`);
    return z || null;
  };
  const slabAt = (x, z) => home.floors.filter((s) => slabContains(s, x, z)).sort((a, b) => b.top - a.top)[0] || null;
  const named = (z) => (z && z.room_type && tree.room_names[z.room_type]) || null;

  // The stair, its foot (1 m before its first step, facing up it) and the landing 1 m past its top.
  const st = piece(ROUTE.stairs, "Stairs");
  const upper = st ? st.pos[1] + st.h : NaN;
  let stairs = null;
  if (st) {
    const foot = toWorld(st.pos, st.yaw, 0, -st.d / 2 - 1.0);
    const top = toWorld(st.pos, st.yaw, 0, st.d / 2 + 1.0);
    stairs = { ...st, foot, top, up_yaw: Math.PI - st.yaw, top_m: upper };
    const landing = slabAt(top[0], top[1]);
    if (!landing || Math.abs(landing.top - upper) > 1e-6) {
      problems.push(`no floor slab at ${upper} m under the stair's top landing ${fmtPt(top)}: the stair leads to no floor`);
    }
  }
  const front = Array.isArray(home.spawn) && home.spawn.length === 2 ? home.spawn : null;
  if (!front) problems.push("data/homes/homestead.ron has no spawn (the front door the first leg walks in from)");

  // The upper hall, the bedroom and the lift lobby: zones on the upper floor.
  const hall = zone(ROUTE.upper_hall);
  const bed = zone(ROUTE.bedroom);
  const lobby = zone(ROUTE.lift_lobby);
  for (const z of [hall, bed, lobby]) {
    if (z && Math.abs(z.origin[1] - upper) > 1e-6) problems.push(`zone "${z.id}" is not on the upper floor (origin y ${z.origin[1]}, the stair rises to ${upper})`);
    if (z && !named(z)) problems.push(`zone "${z.id}"'s room type "${z.room_type}" has no name in data/rooms.ron (the HUD would show its id)`);
  }

  // The walked routes, each straight leg checked against the walls of its storey.
  const route = (what, floor, pts) => {
    for (let i = 1; i < pts.length; i++) {
      for (const w of wallsCrossed(home, floor, eye, radius, pts[i - 1], pts[i])) {
        problems.push(`${what}: the straight leg ${fmtPt(pts[i - 1])} to ${fmtPt(pts[i])} walks into the wall ${wallName(w)}`);
      }
    }
    return pts;
  };
  // The front door, west down the hall to the stair's foot, then up the flight to the landing
  // past its top (that last leg rises between the storeys, between the stair's own walls).
  const stairsRoute = stairs && front ? [...route("the front door to the stair", 0, [front, [stairs.foot[0], front[1]], stairs.foot]), stairs.top] : null;
  if (stairs) route("up the stair", 0, [stairs.foot, stairs.top]);
  const bedRoute = stairs ? route("the stair top to the bedroom", upper, [stairs.top, [stairs.top[0], ROUTE.gallery_z], [ROUTE.bedroom_door_x, ROUTE.gallery_z], ...ROUTE.bedroom_in]) : null;
  const bedEnd = ROUTE.bedroom_in[ROUTE.bedroom_in.length - 1];
  if (bed && !zoneContains(bed, bedEnd[0], upper, bedEnd[1])) problems.push(`the bedroom route ends at ${fmtPt(bedEnd)}, outside zone "${bed.id}"`);
  if (stairs && hall && !zoneContains(hall, stairs.top[0], upper, stairs.top[1])) problems.push(`the stair's top landing ${fmtPt(stairs.top)} is outside zone "${hall.id}"`);
  /** Where the HUD's room is checked, at least HUD_WALL_MARGIN_M from every wall of the storey. */
  const hudPoint = (what, p, floor) => {
    for (const w of home.walls) {
      if (!w.a || !w.b || !wallStops(w, floor, eye)) continue;
      const d = segDist(w.a, w.b, p[0], p[1]);
      if (d < LIMITS.HUD_WALL_MARGIN_M) problems.push(`${what} ${fmtPt(p)} is ${d.toFixed(2)} m from the wall ${wallName(w)}: within ${LIMITS.HUD_WALL_MARGIN_M} m of a wall the room's box (0.5 m cells) may not reach it and the HUD names no room`);
    }
  };
  if (stairs) hudPoint("the stair's top landing", stairs.top, upper);
  hudPoint("the bedroom route's end", bedEnd, upper);

  // The wall upstairs that has none under it.
  const wall = home.walls.find((w) => w.a && w.b && Math.abs(w.base - upper) < 1e-6 && sameLine(w, ROUTE.wall));
  if (!wall) problems.push(`no wall upstairs (base ${upper}) from ${fmtPt(ROUTE.wall.a)} to ${fmtPt(ROUTE.wall.b)}: pick another wall for the wall leg`);
  for (const w of home.walls) {
    if (w.a && w.b && wallStops(w, 0, eye) && wallsOverlap(w, ROUTE.wall)) problems.push(`the wall upstairs ${fmtPt(ROUTE.wall.a)} to ${fmtPt(ROUTE.wall.b)} has a ground wall on its line (${wallName(w)}): the downstairs half of the leg could not walk across it`);
  }
  if (wall && wall.openings.some((o) => o.kind === "Door")) problems.push("the wall upstairs chosen for the wall leg has a door in it");
  route("downstairs across the wall's line", 0, [ROUTE.wall_down_from, ROUTE.wall_down_to]);

  // The jumps: one under a slab, one under none.
  const under = slabAt(ROUTE.jump_under[0], ROUTE.jump_under[1]);
  if (!under) problems.push(`no floor slab over the jump point ${fmtPt(ROUTE.jump_under)}`);
  if (slabAt(ROUTE.jump_open[0], ROUTE.jump_open[1])) problems.push(`a floor slab stands over the open jump point ${fmtPt(ROUTE.jump_open)}`);

  // The guard over the stairwell.
  const guard = home.walls.find((w) => w.a && w.b && Math.abs(w.base - upper) < 1e-6 && sameLine(w, ROUTE.guard));
  if (!guard) problems.push(`no guard upstairs from ${fmtPt(ROUTE.guard.a)} to ${fmtPt(ROUTE.guard.b)}`);
  else if (!tree.guard_materials.includes(guard.material)) problems.push(`the wall ${wallName(guard)} is not a balustrade (material ${guard.material}; guards: ${tree.guard_materials.join(", ")})`);
  const beyond = (() => {
    // Over the guard, a step past it: the stairwell, open (no slab at the upper floor).
    const t = [ROUTE.guard_from[0], ROUTE.guard.a[1] - 0.5];
    const s = slabAt(t[0], t[1]);
    return s && Math.abs(s.top - upper) < 1e-6 ? s : null;
  })();
  if (beyond) problems.push(`the floor beyond the guard is a slab ("${beyond.label}"), not the open stairwell: the guard leg would prove nothing`);

  // The elevator, its stops (the storey floors its shaft spans) and its approach.
  const lift = piece(ROUTE.elevator, "Elevator");
  let elevator = null;
  if (lift) {
    const tops = [...new Set(home.floors.map((f) => f.top))].filter((t) => t - lift.pos[1] > 0.05 && t - lift.pos[1] <= lift.h + 0.05);
    elevator = { ...lift, stops: [0, ...tops.map((t) => t - lift.pos[1]).sort((a, b) => a - b)] };
    if (!elevator.stops.some((s) => Math.abs(lift.pos[1] + s - upper) < 1e-6)) problems.push(`"${lift.id}" has no stop at the upper floor (${upper}): its stops are ${elevator.stops.join(", ")}`);
    if (outsideFootprint(elevator, ROUTE.lift_from[0], ROUTE.lift_from[1]) < radius) problems.push("the elevator's approach point is inside its shaft");
    if (lobby && !zoneContains(lobby, ROUTE.lift_out[0], upper, ROUTE.lift_out[1])) problems.push(`the elevator's way out ${fmtPt(ROUTE.lift_out)} is not in zone "${lobby.id}"`);
    if (inReach(elevator, ROUTE.lift_call_from[0], ROUTE.lift_call_from[1], CALL_REACH_M + 0.05)) problems.push("the call leg starts within the elevator's call reach (1 m): the car would come before the body walks to the door");
    route("the service way onto the car", 0, [ROUTE.lift_from, [lift.pos[0], lift.pos[2]]]);
    route("off the car into the lift lobby", upper, [[lift.pos[0], lift.pos[2]], ROUTE.lift_out]);
    hudPoint("the elevator's way out", ROUTE.lift_out, upper);
  }

  // The ladder and the gate across its top: the ladder's width and HATCH_SIDE_CLEAR_M each side,
  // the whole gap the guards leave (engine/footing.rs `ladder_gates`).
  const ld = piece(ROUTE.ladder, "Ladder");
  let ladder = null;
  if (ld) {
    const gateA = toWorld(ld.pos, ld.yaw, -ld.w / 2 - HATCH_SIDE_CLEAR_M, 0);
    const gateB = toWorld(ld.pos, ld.yaw, ld.w / 2 + HATCH_SIDE_CLEAR_M, 0);
    ladder = { ...ld, top_m: ld.pos[1] + ld.h, gate: { a: gateA, b: gateB }, hang_eye_m: ld.pos[1] + ld.h + eye + 0.2 };
    if (Math.abs(ladder.top_m - upper) > 1e-6) problems.push(`"${ld.id}"'s top (${ladder.top_m}) is not the upper floor (${upper})`);
    if (slabAt(ROUTE.ladder_from[0], ROUTE.ladder_from[1])) problems.push(`a slab stands over the ladder's climbing point ${fmtPt(ROUTE.ladder_from)}: it is not in the hatch`);
    const off = ROUTE.ladder_off[ROUTE.ladder_off.length - 1];
    for (const p of ROUTE.ladder_off) {
      const s = slabAt(p[0], p[1]);
      if (!s || Math.abs(s.top - upper) > 1e-6) problems.push(`no floor slab at ${upper} under the step-off route's point ${fmtPt(p)}`);
    }
    const reach = Math.max(ld.w, ld.d) / 2 + 0.9; // engine/footing.rs `ladder_at`
    if (Math.hypot(ROUTE.ladder_from[0] - ld.pos[0], ROUTE.ladder_from[1] - ld.pos[2]) > reach) problems.push("the ladder's climbing point is out of its reach");
    if (Math.hypot(off[0] - ld.pos[0], off[1] - ld.pos[2]) <= reach + 0.1) problems.push(`the ladder's step-off point ${fmtPt(off)} is within its reach (${reach} m): the body would hang on there, 0.2 m off the floor`);
    route("off the ladder's top onto the lobby floor", upper, [ROUTE.ladder_from, ...ROUTE.ladder_off]);
    if (lobby && !zoneContains(lobby, off[0], upper, off[1])) problems.push(`the ladder's step-off point ${fmtPt(off)} is not in zone "${lobby.id}"`);
    hudPoint("the ladder's step-off point", off, upper);
    // Back into the hatch's gap: in line with the ladder, so the gate (its width, the gap's) is
    // what the walk meets, not the guards' ends either side.
    const mid = toWorld(ld.pos, ld.yaw, 0, 0);
    if (Math.abs(toLocal(ld.pos, ld.yaw, ROUTE.hatch_from[0], ROUTE.hatch_from[1])[0]) > 0.02) problems.push(`the hatch leg's start ${fmtPt(ROUTE.hatch_from)} is not in line with ${ld.id} (${fmtPt(mid)}): it would walk into the guards beside the gap`);
    route("the lobby to the hatch leg's start", upper, [off, ROUTE.hatch_from]);
    // The ladder legs must not call the car: the call leg needs it still at the bottom.
    for (const p of [ROUTE.ladder_from, ...ROUTE.ladder_off, ROUTE.hatch_from]) {
      if (elevator && inReach(elevator, p[0], p[1], CALL_REACH_M + 0.05)) problems.push(`the ladder leg's point ${fmtPt(p)} is within the elevator's call reach: it would call the car the call leg needs at the bottom`);
    }
  }

  const plan = {
    upper_m: upper,
    eye_m: eye,
    radius_m: radius,
    head_m: tree.head_m,
    gravity_m_s2: tree.gravity_m_s2,
    front_door: front,
    stairs,
    stairs_route: stairsRoute,
    upper_hall: hall && { id: hall.id, name: named(hall) },
    bedroom: bed && { id: bed.id, name: named(bed) },
    lift_lobby: lobby && { id: lobby.id, name: named(lobby) },
    bedroom_route: bedRoute,
    wall: { ...ROUTE.wall, up_from: ROUTE.wall_up_from, down_from: ROUTE.wall_down_from, down_to: ROUTE.wall_down_to },
    jump_under: { at: ROUTE.jump_under, underside_m: under ? under.top - under.thickness : NaN, slab: under && under.label },
    jump_open: { at: ROUTE.jump_open },
    guard: { ...ROUTE.guard, from: ROUTE.guard_from },
    elevator,
    lift_from: ROUTE.lift_from,
    lift_out: ROUTE.lift_out,
    lift_call_from: ROUTE.lift_call_from,
    ladder,
    ladder_from: ROUTE.ladder_from,
    // The route off the ladder's top (the rig walks it) and where it ends (the judge's).
    ladder_route: [ROUTE.ladder_from, ...ROUTE.ladder_off],
    ladder_off: ROUTE.ladder_off[ROUTE.ladder_off.length - 1],
    hatch_from: ROUTE.hatch_from,
    shots: ROUTE.shots,
  };
  return { plan, problems };
}

/** Whether wall w lies along the route's line l (both ends within 5 cm, either way round). */
function sameLine(w, l) {
  const near = (p, q) => Math.hypot(p[0] - q[0], p[1] - q[1]) < 0.05;
  return (near(w.a, l.a) && near(w.b, l.b)) || (near(w.a, l.b) && near(w.b, l.a));
}
/** Whether wall w overlaps the line l (collinear, sharing more than 10 cm of length). */
function wallsOverlap(w, l) {
  if (Math.abs(sideOf(l.a, l.b, w.a[0], w.a[1])) > 0.05 || Math.abs(sideOf(l.a, l.b, w.b[0], w.b[1])) > 0.05) return false;
  const len = Math.hypot(l.b[0] - l.a[0], l.b[1] - l.a[1]);
  const u0 = alongOf(l.a, l.b, w.a[0], w.a[1]);
  const u1 = alongOf(l.a, l.b, w.b[0], w.b[1]);
  const lo = Math.max(0, Math.min(u0, u1));
  const hi = Math.min(1, Math.max(u0, u1));
  return (hi - lo) * len > 0.1;
}

// ── The legs ─────────────────────────────────────────────────────────────────────────────────────

/** Every leg the rig runs, in order. A run that carries the plan must have every one. */
const ALL_LEGS = [
  "stairs_up",
  "bedroom",
  "wall_upstairs",
  "wall_downstairs",
  "jump_under_slab",
  "jump_open",
  "guard",
  "elevator_up",
  "elevator_out_top",
  "elevator_down",
  "elevator_out_bottom",
  "ladder_up",
  "hatch_gate",
  "elevator_call",
];
/** Every screenshot the rig takes. */
const ALL_SHOTS = ROUTE.shots.map((s) => s.id);

const num = (x) => (x === null || x === undefined || x === "" ? NaN : Number(x));
const f2 = (x) => (Number.isFinite(num(x)) ? num(x).toFixed(2) : "(none)");

/**
 * A leg's frames in home metres, every recording of it in order: { seg, t, x, y, z, floor,
 * ceiling, room: {id, name, floor} | null, hud, on_ladder, at_ladder, on_lift, cars, landings,
 * last_m_s }. Frames without a `storey` block (a build older than the probe) are dropped.
 */
function framesOf(leg, origin) {
  const o = Array.isArray(origin) ? origin.map(Number) : [0, 0, 0];
  const out = [];
  (Array.isArray(leg.segments) ? leg.segments : []).forEach((seg, si) => {
    for (const f of (seg.rec && seg.rec.frames) || []) {
      const s = f.storey;
      if (!s || !Array.isArray(f.cam)) continue;
      out.push({
        seg: si,
        t: num(f.t),
        x: num(f.cam[0]) - o[0],
        y: num(f.cam[1]) - o[1],
        z: num(f.cam[2]) - o[2],
        floor: num(s.floor) - o[1],
        ceiling: s.ceiling === null || s.ceiling === undefined ? null : num(s.ceiling) - o[1],
        room: s.room ? { id: s.room.id, name: s.room.name, floor: num(s.room.floor) - o[1] } : null,
        hud: s.hud_shown,
        aboard: s.aboard,
        on_ladder: !!s.on_ladder,
        at_ladder: !!s.at_ladder,
        on_lift: !!s.on_lift,
        cars: Array.isArray(s.cars) ? s.cars : [],
        landings: num(s.landings),
        last_m_s: s.last_m_s === null || s.last_m_s === undefined ? null : num(s.last_m_s),
      });
    }
  });
  return out;
}

/** The landings handed to the body over a leg's frames, each with its speed: the frames on which
 *  the count went up. `before`: the count before the leg (its first frame's otherwise). */
function landingsIn(frames) {
  const out = [];
  for (let i = 1; i < frames.length; i++) {
    if (frames[i].landings > frames[i - 1].landings) out.push({ t: frames[i].t, m_s: frames[i].last_m_s, at: [frames[i].x, frames[i].z] });
  }
  return out;
}

/**
 * Judge one run. `run` = { plan (buildPlan's), problems, origin [x, y, z] (the home's box corner,
 * world metres), legs: [{ id, kind, hold_s?, segments: [{ what, hold_s?, rec }] }], shots:
 * [{ id, file, ok, bytes }], data_dir { logged, how, real, expected } (the data folder the game
 * logged, resolved, and the one the run meant), panics, builtin_data [lines], other_build,
 * sandbox_data }. Returns { checks: [{ id, ok, detail }], ok, observed }.
 */
function judgeStoreys(run) {
  const checks = [];
  const observed = {};
  const check = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const P = run && run.plan;
  if (!P || !P.stairs || !Number.isFinite(num(P.upper_m))) {
    check("plan", false, "the manifest carries no plan (the homestead's stair, slabs and rooms): nothing to judge against");
    return { checks, ok: false, observed };
  }
  if (Array.isArray(run.problems) && run.problems.length) check("plan", false, `the layout does not have what the route walks: ${run.problems.join("; ")}`);
  const UP = num(P.upper_m);
  const R = num(P.radius_m);
  const EYE = num(P.eye_m);
  const G = num(P.gravity_m_s2);
  const safeMS = Math.sqrt(2 * G * LIMITS.SAFE_DROP_M);
  const origin = Array.isArray(run.origin) ? run.origin : null;
  if (!origin) check("origin", false, "no home origin (the first look's home box): nothing can be put in the home's metres");
  const legs = Array.isArray(run.legs) ? run.legs : [];
  for (const want of ALL_LEGS) if (!legs.some((l) => l && l.id === want)) check(`${want}.ran`, false, "this leg never ran (see the rig's output for where it stopped)");

  // Shared readings.
  const onFloor = (f, floor) => Math.abs(f.floor - floor) <= LIMITS.FLOOR_TOL_M;
  const standing = (f) => Math.abs(f.y - f.floor - EYE) <= 0.05;
  const last = (fs) => fs[fs.length - 1];
  const where = (f) => (f ? `(${f2(f.x)}, ${f2(f.z)}) eye ${f2(f.y)} on the floor at ${f2(f.floor)}` : "(no frame)");
  const roomSaid = (f) => (f && f.room ? `"${f.room.name}" (${f.room.id}, floor ${f2(f.room.floor)})` : "no room");

  /** Every frame-to-frame move within a recording no faster than a walk, and each recording
   *  starting where the last ended: the body walked, nothing placed it. */
  const walked = (id, fs) => {
    let worst = 0;
    let at = null;
    let gap = 0;
    for (let i = 1; i < fs.length; i++) {
      const a = fs[i - 1];
      const b = fs[i];
      const d = Math.hypot(b.x - a.x, b.z - a.z);
      if (b.seg !== a.seg) {
        if (d > gap) gap = d;
        continue;
      }
      const dt = b.t - a.t;
      if (dt > 1e-4 && d / dt > worst) {
        worst = d / dt;
        at = b;
      }
    }
    const ok = worst <= LIMITS.WALK_MAX_M_S && gap <= LIMITS.SEG_JOIN_M;
    check(
      `${id}.walked`,
      ok && fs.length > 1,
      fs.length < 2
        ? "fewer than two frames: no walk to read"
        : ok
          ? `walked: never faster than ${f2(worst)} m/s across the floor, and every recording starts where the last ended (at most ${f2(gap)} m apart)`
          : worst > LIMITS.WALK_MAX_M_S
            ? `the body moved ${f2(worst)} m/s across the floor near ${where(at)}: something placed it there, it did not walk`
            : `two recordings of the walk are ${f2(gap)} m apart: the body was moved between them`
    );
  };
  /** No landing handed to the body (`strict`), or none from a drop that could hurt. */
  const noFall = (id, fs, strict) => {
    const ls = landingsIn(fs);
    const hard = ls.filter((l) => !(num(l.m_s) < safeMS));
    const ok = strict ? ls.length === 0 : hard.length === 0;
    check(
      `${id}.no_fall`,
      ok,
      ok
        ? strict
          ? "no landing reached the body"
          : `${ls.length ? `${ls.length} landing(s), each under ${f2(safeMS)} m/s (a ${LIMITS.SAFE_DROP_M} m drop): ${ls.map((l) => f2(l.m_s)).join(", ")} m/s` : "no landing reached the body"}`
        : `${ls.length} landing(s) reached the body: ${ls.map((l) => `${f2(l.m_s)} m/s at (${f2(l.at[0])}, ${f2(l.at[1])})`).join("; ")}` +
            (strict ? " (none may)" : ` (a landing over ${f2(safeMS)} m/s is a fall of more than ${LIMITS.SAFE_DROP_M} m)`) +
            ": the body fell, through a missing floor, a hole or an open edge"
    );
  };
  /** Every frame on the floor at `floor`. */
  const stayedOn = (id, fs, floor, what) => {
    const off = fs.find((f) => !onFloor(f, floor));
    check(`${id}.stayed_on_floor`, !off && fs.length > 0, off ? `the floor under the body went to ${f2(off.floor)} at ${where(off)}; it must stay ${what} at ${f2(floor)}` : `on the floor at ${f2(floor)} (${what}) every frame`);
  };
  /** The last frame's HUD room is `want` ({id, name}) on the floor at `floor`. */
  const hudNames = (id, f, want, floor) => {
    const ok = f && f.room && want && f.room.id === want.id && f.room.name === want.name && Math.abs(f.room.floor - floor) <= LIMITS.FLOOR_TOL_M && f.hud === true;
    check(
      `${id}.hud_room`,
      ok,
      !want
        ? "the plan has no room to expect"
        : `the HUD names ${roomSaid(f)}${f && f.hud === false ? ", and the HUD is not drawn" : ""}; expected "${want.name}" (${want.id}) on the floor at ${f2(floor)}`
    );
  };
  /** The first frame within START_TOL_M of `p` on the floor at `floor`. */
  const startsAt = (id, f, p, floor) => {
    const ok = f && Math.hypot(f.x - p[0], f.z - p[1]) <= LIMITS.START_TOL_M && onFloor(f, floor);
    check(`${id}.start`, ok, `started at ${where(f)}; asked (${f2(p[0])}, ${f2(p[1])}) on the floor at ${f2(floor)}`);
  };
  /** Walked into the line a..b from the start side and held there: never closer than the radius
   *  less the slack, moved toward it, ended near it. */
  const heldBy = (id, fs, a, b, what) => {
    const s0 = fs.length ? sideOf(a, b, fs[0].x, fs[0].z) : NaN;
    const sign = Math.sign(s0) || 1;
    let nearest = Infinity;
    let worst = null;
    for (const f of fs) {
      const s = sideOf(a, b, f.x, f.z) * sign;
      const u = alongOf(a, b, f.x, f.z);
      const d = u >= 0 && u <= 1 ? s : Math.max(s, segDist(a, b, f.x, f.z));
      if (d < nearest) {
        nearest = d;
        worst = f;
      }
    }
    const endD = fs.length ? Math.max(sideOf(a, b, last(fs).x, last(fs).z) * sign, 0) : NaN;
    const startD = Math.abs(s0);
    const held = nearest >= R - LIMITS.SIDE_SLACK_M;
    const pushed = startD - endD >= LIMITS.PUSHED_M && endD <= LIMITS.REACH_M;
    check(
      `${id}.held`,
      held && pushed && fs.length > 1,
      fs.length < 2
        ? "fewer than two frames"
        : !held
          ? `the body went ${f2(Math.max(0, -nearest))} m past ${what} (its eye ${f2(nearest)} m from the line at ${where(worst)}; held, it stays ${f2(R)} m off): ${what} did not stop it`
          : !pushed
            ? `the body went from ${f2(startD)} m to ${f2(endD)} m off ${what}: it never walked up to it, so this says nothing`
            : `walked from ${f2(startD)} m off ${what} to ${f2(endD)} m and was held there (nearest ${f2(nearest)} m, the body's radius ${f2(R)})`
    );
    return { nearest, endD };
  };

  for (const leg of legs) {
    const id = leg.id;
    const fs = framesOf(leg, origin);
    const seen = (observed[id] = {
      frames: fs.length,
      start: fs[0] ? [fs[0].x, fs[0].y, fs[0].z] : null,
      end: fs.length ? [last(fs).x, last(fs).y, last(fs).z] : null,
      floor_end: fs.length ? last(fs).floor : null,
      room_end: fs.length && last(fs).room ? last(fs).room.id : null,
      landings: landingsIn(fs).map((l) => l.m_s),
    });
    if (!fs.length) {
      check(`${id}.recorded`, false, "no frame with a `storey` block: the game never answered the recorder, or the build is older than src/engine/storey_probe.rs");
      continue;
    }
    const f0 = fs[0];
    const fz = last(fs);

    if (id === "stairs_up") {
      const st = P.stairs;
      startsAt(id, f0, P.front_door, 0);
      walked(id, fs);
      // Up the treads: frames over the stair's footprint part way up.
      const fp = { pos: st.pos, yaw: (st.rot_deg * Math.PI) / 180, w: st.w, d: st.d };
      const onTreads = fs.filter((f) => outsideFootprint(fp, f.x, f.z) === 0 && f.floor > 0.3 && f.floor < UP - 0.3);
      check(`${id}.on_the_treads`, onTreads.length >= 3, `${onTreads.length} frame(s) stood on the stair's treads part way up (${onTreads.length ? `floors ${f2(Math.min(...onTreads.map((f) => f.floor)))} to ${f2(Math.max(...onTreads.map((f) => f.floor)))}` : "none"}): it walked up ${st.id}`);
      const top = P.stairs.top;
      const arrived = Math.hypot(fz.x - top[0], fz.z - top[1]) <= LIMITS.START_TOL_M;
      check(`${id}.upper_floor`, arrived && onFloor(fz, UP) && standing(fz), `ended at ${where(fz)}; the landing past the stair's top is (${f2(top[0])}, ${f2(top[1])}) on the floor at ${f2(UP)}${standing(fz) ? "" : ", and the body is not standing there (still falling or climbing)"}`);
      hudNames(id, fz, P.upper_hall, UP);
      noFall(id, fs, true);
      seen.top_floor = Math.max(...fs.map((f) => f.floor));
    } else if (id === "bedroom") {
      startsAt(id, f0, P.stairs.top, UP);
      walked(id, fs);
      stayedOn(id, fs, UP, "upstairs");
      const end = P.bedroom_route[P.bedroom_route.length - 1];
      check(`${id}.arrived`, Math.hypot(fz.x - end[0], fz.z - end[1]) <= LIMITS.START_TOL_M, `ended at ${where(fz)}; the route ends at (${f2(end[0])}, ${f2(end[1])}) inside the bedroom`);
      hudNames(id, fz, P.bedroom, UP);
      noFall(id, fs, true);
    } else if (id === "wall_upstairs") {
      startsAt(id, f0, P.wall.up_from, UP);
      stayedOn(id, fs, UP, "upstairs");
      heldBy(id, fs, P.wall.a, P.wall.b, `the wall upstairs ${fmtPt(P.wall.a)} to ${fmtPt(P.wall.b)}`);
      noFall(id, fs, true);
    } else if (id === "wall_downstairs") {
      startsAt(id, f0, P.wall.down_from, 0);
      walked(id, fs);
      stayedOn(id, fs, 0, "downstairs");
      const s0 = sideOf(P.wall.a, P.wall.b, f0.x, f0.z);
      const s1 = sideOf(P.wall.a, P.wall.b, fz.x, fz.z);
      const u = alongOf(P.wall.a, P.wall.b, fz.x, fz.z);
      const crossed = Math.sign(s0) !== Math.sign(s1) && Math.abs(s1) >= LIMITS.PAST_M;
      const under = fs.find((f, i) => i > 0 && Math.sign(sideOf(P.wall.a, P.wall.b, fs[i - 1].x, fs[i - 1].z)) !== Math.sign(sideOf(P.wall.a, P.wall.b, f.x, f.z)));
      const uAt = under ? alongOf(P.wall.a, P.wall.b, under.x, under.z) : u;
      check(
        `${id}.walked_under`,
        crossed && uAt > 0 && uAt < 1,
        crossed
          ? `downstairs the body walked across the line of the wall upstairs (from ${f2(Math.abs(s0))} m one side to ${f2(Math.abs(s1))} m the other), under its span: the wall stands on the upper floor only`
          : `downstairs the body got from ${f2(s0)} m to ${f2(s1)} m off the line of the wall upstairs (it must cross it by ${LIMITS.PAST_M} m): the wall upstairs stopped a body on the ground floor under it`
      );
      noFall(id, fs, true);
    } else if (id === "jump_under_slab" || id === "jump_open") {
      const at = id === "jump_under_slab" ? P.jump_under.at : P.jump_open.at;
      startsAt(id, f0, at, 0);
      const under = num(P.jump_under.underside_m);
      const clamp = under - num(P.head_m);
      const peak = Math.max(...fs.map((f) => f.y));
      seen.peak_eye = peak;
      seen.clamp_eye = clamp;
      if (id === "jump_under_slab") {
        const knew = f0.ceiling !== null && Math.abs(f0.ceiling - under) <= LIMITS.FLOOR_TOL_M;
        check(`${id}.ceiling_known`, knew, `the walk's ceiling over the body: ${f0.ceiling === null ? "none" : f2(f0.ceiling)}; the slab's underside is ${f2(under)} ("${P.jump_under.slab}")`);
        check(
          `${id}.head_clamped`,
          peak <= clamp + LIMITS.CLAMP_TOL_M,
          `the eye rose to ${f2(peak)} m; under the slab's underside at ${f2(under)} less the head's ${f2(P.head_m)} m it may reach ${f2(clamp)}` +
            (peak > clamp + LIMITS.CLAMP_TOL_M ? `: the head went ${f2(peak + num(P.head_m) - under)} m into the floor over it` : "")
        );
        check(`${id}.jumped`, peak >= clamp - LIMITS.CLAMP_REACH_M, `the jump lifted the eye from ${f2(f0.y)} to ${f2(peak)} (it must reach within ${LIMITS.CLAMP_REACH_M} m of ${f2(clamp)}, where the clamp holds it): ${peak >= clamp - LIMITS.CLAMP_REACH_M ? "it jumped into the clamp" : "it never jumped high enough for the clamp to matter"}`);
      } else {
        check(`${id}.no_ceiling`, f0.ceiling === null, `the walk's ceiling over the body: ${f0.ceiling === null ? "none, as the open point has no slab over it" : f2(f0.ceiling)}`);
        check(
          `${id}.rose_past_the_clamp`,
          peak >= clamp + LIMITS.OPEN_JUMP_OVER_M,
          `with nothing over it the same jump lifted the eye to ${f2(peak)}, ${f2(peak - clamp)} m past where the clamp holds one under the slab (${f2(clamp)}; at least ${LIMITS.OPEN_JUMP_OVER_M} m needed): ` +
            (peak >= clamp + LIMITS.OPEN_JUMP_OVER_M ? "so the slab, not a weak jump, held the one under it" : "a jump this weak proves nothing about the clamp")
        );
      }
      const back = onFloor(fz, 0) && standing(fz);
      check(`${id}.back_on_floor`, back, `after the jump: ${where(fz)}`);
      // A jump's own landing is allowed, and nothing harder.
      const ls = landingsIn(fs);
      const jumpMS = Math.sqrt(2 * G * Math.max(0, peak - EYE)) + 0.3;
      const hard = ls.filter((l) => !(num(l.m_s) <= jumpMS));
      check(`${id}.no_fall`, ls.length <= 1 && !hard.length, `${ls.length} landing(s) (${ls.map((l) => f2(l.m_s)).join(", ") || "none"} m/s); the jump's own lands at about ${f2(jumpMS - 0.3)} m/s`);
    } else if (id === "guard") {
      startsAt(id, f0, P.guard.from, UP);
      stayedOn(id, fs, UP, "upstairs, on the gallery");
      heldBy(id, fs, P.guard.a, P.guard.b, `the guard over the stairwell ${fmtPt(P.guard.a)} to ${fmtPt(P.guard.b)}`);
      noFall(id, fs, true);
    } else if (/^elevator_/.test(id)) {
      const E = P.elevator;
      const fp = { pos: E.pos, yaw: E.yaw, w: E.w, d: E.d };
      const inShaft = (f) => outsideFootprint(fp, f.x, f.z) === 0;
      const car = (f) => (f.cars[0] ? num(f.cars[0][0]) : NaN);
      const topStop = E.stops[E.stops.length - 1];
      const atStop = (f, s) => Math.abs(car(f) - s) <= LIMITS.CAR_AT_STOP_M;
      seen.car = [car(f0), car(fz)];
      if (id === "elevator_up") {
        const boarded = fs.find((f) => inShaft(f));
        check(`${id}.boarded`, boarded && onFloor(boarded, 0) && atStop(boarded, 0), boarded ? `walked onto the car at ${where(boarded)}, the car at ${f2(car(boarded))}` : `never stood over the car (shaft ${fmtPt([E.pos[0], E.pos[2]])}, ${E.w} x ${E.d} m)`);
        const rode = fs.some((f) => f.on_lift);
        check(`${id}.rode`, rode, rode ? "the game had the body riding the car (Space chose the floor)" : "the game never had the body riding the car");
        check(`${id}.car_up`, atStop(fz, topStop) && onFloor(fz, E.pos[1] + topStop) && inShaft(fz), `after Space the car is at ${f2(car(fz))} (its top stop ${f2(topStop)}) and the body at ${where(fz)}`);
        const jumped = fs.filter((f) => inShaft(f) && Number.isFinite(car(f))).map((f) => f.y - (E.pos[1] + car(f)) - EYE);
        const hop = jumped.length ? Math.max(...jumped) : NaN;
        check(`${id}.no_jump`, hop <= LIMITS.RIDE_JUMP_M, `on the car the eye stood at most ${f2(hop)} m over the car's floor plus the eye: ${hop <= LIMITS.RIDE_JUMP_M ? "Space chose the floor, it did not jump" : "Space jumped while riding"}`);
        noFall(id, fs, true);
      } else if (id === "elevator_out_top") {
        walked(id, fs);
        const out = !inShaft(fz) && onFloor(fz, UP) && standing(fz);
        check(`${id}.stepped_out`, out, `stepped off the car to ${where(fz)}; expected off the shaft on the floor at ${f2(UP)}`);
        hudNames(id, fz, P.lift_lobby, UP);
        check(`${id}.car_stays`, atStop(fz, topStop), `the car stays at its top stop: ${f2(car(fz))}`);
        noFall(id, fs, true);
      } else if (id === "elevator_down") {
        const back = fs.find((f) => inShaft(f) && onFloor(f, UP));
        check(`${id}.back_on`, !!back, back ? `walked back onto the car at the top at ${where(back)}` : "never stood on the car at the top again");
        check(`${id}.car_down`, atStop(fz, 0) && onFloor(fz, E.pos[1]) && inShaft(fz), `after Shift the car is at ${f2(car(fz))} and the body at ${where(fz)}`);
        noFall(id, fs, true);
      } else if (id === "elevator_out_bottom") {
        walked(id, fs);
        check(`${id}.stepped_out`, !inShaft(fz) && onFloor(fz, 0), `stepped off the car to ${where(fz)}; expected off the shaft on the ground floor`);
        noFall(id, fs, true);
      } else if (id === "elevator_call") {
        startsAt(id, f0, P.lift_call_from, UP);
        check(`${id}.car_away`, Number.isFinite(car(f0)) && !atStop(f0, topStop), `when the body set off for the landing door the car was at ${f2(car(f0))} (the upper landing is its stop ${f2(topStop)}): ${atStop(f0, topStop) ? "already there, so this proves nothing about the door" : "away"}`);
        // While the car was away the body stayed out of the shaft, by the radius, on the floor.
        let worst = Infinity;
        let worstF = null;
        let heldFrom = null;
        let arrived = null;
        for (const f of fs) {
          if (!atStop(f, topStop)) {
            const d = outsideFootprint(fp, f.x, f.z);
            if (d < worst) {
              worst = d;
              worstF = f;
            }
            if (heldFrom === null && d <= R + 0.25) heldFrom = f;
          } else if (arrived === null) arrived = f;
        }
        const kept = worst >= R - LIMITS.SIDE_SLACK_M;
        check(
          `${id}.kept_out`,
          kept,
          kept
            ? `while the car was away the body stayed ${f2(worst)} m off the shaft (its radius ${f2(R)}): the landing door held it`
            : `while the car was away (at ${f2(worstF && car(worstF))}) the body walked ${f2(Math.max(0, R - worst))} m into the shaft at ${where(worstF)}: the landing door did not hold it`
        );
        // One recording's frame clock (the rig records the call as one): times from two
        // recordings cannot be subtracted.
        const heldS = heldFrom !== null && arrived && heldFrom.seg === arrived.seg ? arrived.t - heldFrom.t : NaN;
        check(`${id}.held_at_door`, heldS >= LIMITS.GATE_HELD_S, `the body stood at the shut landing door ${f2(heldS)} s before the car came (at least ${LIMITS.GATE_HELD_S} s): ${heldS >= LIMITS.GATE_HELD_S ? "the door held it while the car came" : "the car came before the body was held, so the door was not tried"}`);
        const rode = fs.some((f) => f.on_lift && !atStop(f, topStop));
        check(`${id}.called`, !!arrived && !rode, arrived ? `the car came up to the landing (${f2(car(arrived))}) with nobody riding it: called from the upper landing` : `the car never came to the upper landing (last at ${f2(car(fz))})`);
        check(`${id}.walked_on`, inShaft(fz) && onFloor(fz, UP), `after the car came the body walked onto it: ${where(fz)}`);
        stayedOn(id, fs, UP, "at the upper landing");
        noFall(id, fs, true);
      }
    } else if (id === "ladder_up") {
      const L = P.ladder;
      startsAt(id, f0, P.ladder_from, 0);
      check(`${id}.at_ladder`, f0.at_ladder, f0.at_ladder ? `at ${L.id} at the start` : `the game did not put the body at ${L.id} at ${where(f0)}`);
      const peak = Math.max(...fs.map((f) => f.y));
      const climbed = peak >= L.hang_eye_m - LIMITS.LADDER_TOP_TOL_M && fs.some((f) => f.on_ladder);
      check(`${id}.climbed`, climbed, `the climb lifted the eye to ${f2(peak)} (the ladder's top holds it at ${f2(L.hang_eye_m)})${fs.some((f) => f.on_ladder) ? "" : ", and the game never had the body on the ladder"}`);
      const out = Math.hypot(fz.x - P.ladder_off[0], fz.z - P.ladder_off[1]) <= LIMITS.START_TOL_M && onFloor(fz, UP) && standing(fz);
      check(`${id}.stepped_off`, out, `stepped off the top to ${where(fz)}; expected (${f2(P.ladder_off[0])}, ${f2(P.ladder_off[1])}) on the floor at ${f2(UP)}`);
      hudNames(id, fz, P.lift_lobby, UP);
      noFall(id, fs, false);
    } else if (id === "hatch_gate") {
      const L = P.ladder;
      stayedOn(id, fs, UP, "upstairs, at the hatch");
      heldBy(id, fs, L.gate.a, L.gate.b, `the gate across ${L.id}'s top`);
      check(`${id}.off_the_ladder`, fs.every((f) => !f.on_ladder), fs.every((f) => !f.on_ladder) ? "on the floor, not on the ladder: the gate is shut to a walk" : "the body was on the ladder, where the gate lets it through");
      noFall(id, fs, true);
    }
  }

  // The screenshots: taken and saved with the run (what they show is for a person to look at).
  const shots = Array.isArray(run.shots) ? run.shots : [];
  for (const want of ALL_SHOTS) {
    const s = shots.find((x) => x && x.id === want);
    check(`shot.${want}`, s && s.ok && num(s.bytes) > 1000, s ? (s.ok ? `saved as ${s.file} (${s.bytes} bytes)` : `not taken: ${s.error || "no file"}`) : "never taken");
  }
  // THE DATA THE GAME READ. A rig's `data` junction is not what the game reads when the rig sits
  // inside a source tree (lib.rs `find_data_dir` prefers a data/ whose parent holds a Cargo.toml):
  // the first red run on a floorless sandbox homestead read the repo's own data and PASSED. Every
  // run says which folder the game logged, and it must be the one the run meant.
  const dd = run.data_dir;
  const ddOk = !!(dd && dd.real && sameDir(dd.real, dd.expected));
  check(
    "data_dir",
    ddOk,
    !dd || !dd.logged
      ? "the run recorded no data folder from run.log: whether the game read the data it meant is unknown"
      : `the game read ${dd.logged} (${dd.how}${sameDir(dd.real, dd.logged) ? "" : `, that is ${dd.real}`}); this run meant ${dd.expected}` +
          (ddOk ? "" : ": it walked other data than it meant (lib.rs find_data_dir prefers a source tree's data/ to the folder beside the exe)")
  );
  check("panics", num(run.panics) === 0, `${num(run.panics)} PANIC line(s) in run.log`);
  const builtin = Array.isArray(run.builtin_data) ? run.builtin_data : [];
  check("no_builtin_data", builtin.length === 0, builtin.length ? `served from the copy built into the exe: ${builtin.slice(0, 3).join(" | ")}` : "every data file read from the tree");
  return { checks, ok: checks.every((c) => c.ok), observed };
}

/** Whether two folder paths name the same folder (Windows: any case, either slash, no trailing one). */
function sameDir(a, b) {
  const norm = (p) => String(p || "").replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
  return !!a && !!b && norm(a) === norm(b);
}

/** The verdict lines a rig prints. */
function verdictLines(result) {
  const lines = [];
  for (const c of result.checks) lines.push(`${c.ok ? "PASS" : "FAIL"}  ${c.id.padEnd(34)} ${c.detail}`);
  const failed = result.checks.filter((c) => !c.ok).map((c) => c.id);
  lines.push("");
  lines.push(
    failed.length
      ? `FAIL  ${result.checks.length - failed.length}/${result.checks.length} passed; failed: ${failed.join(", ")}`
      : `PASS  ${result.checks.length}/${result.checks.length}: in the running game the homestead's upper floor holds you, its HUD names its rooms, its walls, guard, gates and head clamp stop you on their storey, and its elevator and ladder take you between the storeys`
  );
  return lines;
}

module.exports = {
  LIMITS,
  ROUTE,
  ALL_LEGS,
  ALL_SHOTS,
  readHomeDesign,
  readRoomNames,
  readGuardMaterials,
  readConst,
  readTreeData,
  buildPlan,
  toWorld,
  toLocal,
  outsideFootprint,
  inReach,
  sideOf,
  yawTo,
  wallsCrossed,
  framesOf,
  landingsIn,
  judgeStoreys,
  sameDir,
  verdictLines,
};
