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
// data/status_effects.csv, and the stairs walked off from data/blueprints/structure_types.ron and
// data/homes/homestead.ron. The harm curve is evaluated HERE, by its own code, from the file: a
// second reading of falls.ron, not the game's.
//
// THE RUN (scripts/verify-falls.js says how each record is made). Aboard the homestead, in the
// world the autopilot enters (offline, the rig's own home), at the foot of its tallest staircase
// (stairs-1, 3 m: the homestead has no deck at the top of a ladder or a staircase, so the top
// step is the highest floor a player can walk onto, and walking on past it is a 3 m drop):
//   forgiving_walk  Illness and injury set to Forgiving; forward held: up the stairs and off the
//                   top. A 3 m drop lands at 7.67 m/s, read at 70% (5.37), under the first harm
//                   (6.26): a landing, and nothing else.
//   realistic_walk  the same in Realistic: 7.67 m/s, past the sprain (6.26) and short of the
//                   broken ankle (7.73): a Sprained Ankle and 9.7 health.
//   realistic_ladder  still Realistic, at the foot of the homestead's 3 m ladder (ladder-1):
//                   Space held climbs it to the top, where the eye stops 0.2 m over the top
//                   rung (4.9 m); let go and the body HOLDS ON there (camera.rs, "HANGING ON");
//                   then a step back off it is a 3.2 m fall: 7.92 m/s, a Broken Ankle and 11.5
//                   health. This is the "walk off the homestead ladder's top" PRIORITIES asked
//                   for; the homestead has no loft at its top, so the floor below is the drop.
//   placed_drop     still Realistic, the player PLACED 3 m up beside the stairs (the showcase
//                   `cam` verb, a teleport) and left to fall: the same drop, and no landing may
//                   reach the body, because something other than the walk moved it (FallWatch).
// And before any leg, `world_entry`: the first look after the autopilot entered the world has
// had no landing reach the body, no wound and full health.
// The forgiving leg goes first so each walk starts from a clean body (no wound, full health); a
// wound already on the body is not a new one, so each later leg is judged by what it added.
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
};

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
  return {
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

/**
 * The ladder a ladder leg climbs: the tallest Ladder in the home, the pose to stand at it (home
 * metres, eye height; STAND_M out from its face along its local +z, facing it) and how high the
 * eye hangs at its top. Mirrors src/renderer/camera.rs: climbing clamps the eye at the ladder's
 * top + eye height + 0.2 m, and the climb zone (src/lib.rs) is within half the ladder's widest
 * side + 0.9 m of its foot, so a step of more than that away from it lets go.
 */
function pickLadder(types, structures, eyeM = 1.7) {
  const STAND_M = 0.6;
  const ladders = structures
    .map((s) => ({ s, ty: types[s.type_id] }))
    .filter(({ ty }) => ty && ty.kind === "Ladder" && Array.isArray(ty.size))
    .sort((a, b) => b.ty.size[1] - a.ty.size[1]);
  if (!ladders.length) return null;
  const { s, ty } = ladders[0];
  const [w, h, d] = ty.size;
  const r = (s.rot_deg * Math.PI) / 180;
  return {
    id: s.id,
    type_id: s.type_id,
    size: [w, h, d],
    rot_deg: s.rot_deg,
    pos: s.pos.slice(),
    top_m: s.pos[1] + h,
    stand: [s.pos[0] + STAND_M * Math.sin(r), s.pos[1] + eyeM, s.pos[2] + STAND_M * Math.cos(r)],
    yaw: -r,
    hang_eye_m: s.pos[1] + h + eyeM + 0.2,
    reach_m: Math.max(w, d) * 0.5 + 0.9,
  };
}

/** Everything the judge needs from the tree. `readFile` is injectable for tests. */
function readTreeData(repo, readFile = (p) => require("fs").readFileSync(p, "utf8")) {
  const rd = (...p) => readFile(path.join(repo, ...p));
  const types = readStructureTypes(rd("data", "blueprints", "structure_types.ron"));
  const placed = readHomeStructures(rd("data", "homes", "homestead.ron"));
  return {
    falls: readFalls(rd("data", "medical", "falls.ron")),
    gravity_m_s2: readGravity(rd("data", "game.csv")),
    effects: readEffects(rd("data", "status_effects.csv")),
    stairs: pickStairs(types, placed),
    ladder: pickLadder(types, placed),
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

/**
 * The camera's drop in a recording: the highest the eye was, where it came to rest (the last
 * frame's height), the drop between, and the frame clock of the first frame back at rest after
 * the highest point (the landing). null with fewer than two frames.
 */
function dropOf(frames) {
  const fs = (Array.isArray(frames) ? frames : []).filter((f) => f && Array.isArray(f.cam) && Number.isFinite(num(f.cam[1])));
  if (fs.length < 2) return null;
  let top = 0;
  fs.forEach((f, i) => {
    if (num(f.cam[1]) > num(fs[top].cam[1])) top = i;
  });
  const rest = num(fs[fs.length - 1].cam[1]);
  const peak = num(fs[top].cam[1]);
  let landed = -1;
  for (let i = top + 1; i < fs.length; i++) {
    if (num(fs[i].cam[1]) <= rest + 0.02) {
      landed = i;
      break;
    }
  }
  // The longest stretch the eye stayed within 5 cm of its highest (on a ladder: the hang).
  let held = 0;
  let from = null;
  for (const f of fs) {
    if (num(f.cam[1]) >= peak - 0.05) {
      if (from === null) from = num(f.t);
      held = Math.max(held, num(f.t) - from);
    } else from = null;
  }
  return {
    peak_y: peak,
    peak_held_s: held,
    rest_y: rest,
    drop_m: peak - rest,
    landed_t: landed >= 0 ? num(fs[landed].t) : NaN,
    first_cam: fs[0].cam.slice(0, 3),
    frames: fs,
  };
}

// ── The judgement ────────────────────────────────────────────────────────────────────────────────

/**
 * Judge one run. `run` = { data (readTreeData's), entry (the first look after world entry),
 * legs: [{ id, mode, kind: "walk"|"ladder"|"placement", start [x, y, z], hang_s (a ladder's),
 * before, after }], panics, builtin_data [lines], other_build }. Returns
 * { checks: [{ id, ok, detail }], ok, observed: { <leg>: what was seen, for the report } }.
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
  const g = num(data.gravity_m_s2);
  const effects = data.effects || {};
  const legs = Array.isArray(run.legs) ? run.legs : [];
  for (const want of ["forgiving_walk", "realistic_walk", "realistic_ladder", "placed_drop"]) {
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

  for (const leg of legs) {
    const id = leg.id;
    const before = leg.before || null;
    const after = leg.after || null;
    if (!before || !after) {
      check(`${id}.recorded`, false, `no ${!before ? "before" : "after"} record: the game never answered the recorder`);
      continue;
    }
    const drop = dropOf(after.frames);
    const hb = healthOf(before);
    const ha = healthOf(after);
    const wb = woundIds(before) || [];
    const wa = woundIds(after);
    const newWounds = (wa || []).filter((w) => !wb.includes(w));
    const lb = landingsOf(before);
    const la = landingsOf(after);
    const last = after.falls ? num(after.falls.last_m_s) : NaN;
    const seen = (observed[id] = {
      mode: after.falls && after.falls.mode,
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

    // The state the leg asked for: the mode, on foot, aboard.
    const modeOk = after.falls && after.falls.mode === leg.mode && before.falls && before.falls.mode === leg.mode;
    check(`${id}.mode`, modeOk, `asked ${leg.mode}; the game said ${before.falls && before.falls.mode} before, ${after.falls && after.falls.mode} after`);
    const onFoot = [before, after].every((r) => r.falls && r.falls.flying === false && r.falls.aboard === true);
    check(
      `${id}.on_foot_aboard`,
      onFoot,
      onFoot
        ? "walking (no dev flight), aboard"
        : `flying ${before.falls && before.falls.flying}/${after.falls && after.falls.flying}, aboard ${before.falls && before.falls.aboard}/${after.falls && after.falls.aboard} (before/after): a fall in flight never hurts, so this leg proves nothing`
    );
    const gGame = after.falls ? num(after.falls.gravity_m_s2) : NaN;
    check(`${id}.gravity`, Math.abs(gGame - g) < 1e-3, `the walk's gravity ${f2(gGame)} m/s^2, data/game.csv ${f2(g)}`);
    if (!Number.isFinite(hb) || !Number.isFinite(ha) || !wa) {
      check(`${id}.body_reported`, false, "the probe reported no health or no wounds list: is the build older than src/engine/body_probe.rs?");
      continue;
    }
    if (after.vitals && after.vitals.registry === false) {
      check(`${id}.body_reported`, false, "the game had no status effect registry: no wound could be named, so 'no wound' means nothing");
      continue;
    }

    // The drop happened: without it, "no wound" is a pass that proves nothing.
    const startOk = drop && Array.isArray(leg.start) && Math.hypot(drop.first_cam[0] - leg.start[0], drop.first_cam[2] - leg.start[2]) <= LIMITS.START_TOL_M;
    const dropped = drop && drop.drop_m >= LIMITS.MIN_DROP_M;
    check(
      `${id}.dropped`,
      dropped && (leg.kind === "placement" || startOk),
      !drop
        ? "the recording has under two frames: no camera track to read"
        : `the eye fell ${f2(drop.drop_m)} m (from y ${f2(drop.peak_y)} to ${f2(drop.rest_y)})` +
            (leg.kind !== "placement" ? `, starting at (${drop.first_cam.map(f2).join(", ")}) for (${(leg.start || []).map(f2).join(", ")})` : "") +
            (dropped ? "" : `: under ${LIMITS.MIN_DROP_M} m, so the walk never went off the top and this leg says nothing about falls`)
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

    if (leg.kind === "placement") {
      // Placed, then fell: the watch must keep this landing from the body.
      check(
        `${id}.no_landing`,
        la === lb,
        la === lb
          ? `no landing reached the body (${lb} before and after) for a ${f2(drop.drop_m)} m drop that would land at ${f2(vCam)} m/s`
          : `a landing reached the body (${lb} -> ${la}, the last at ${f2(last)} m/s) after a PLACEMENT: FallWatch let a teleport's drop hurt (src/systems/falls.rs)`
      );
      check(`${id}.no_wound`, newWounds.length === 0, newWounds.length ? `new wound(s) ${newWounds.join(", ")} from a placement` : "no new wound");
      check(`${id}.health`, Math.abs(hb - ha) <= LIMITS.HEALTH_TOL, `health ${f2(hb)} -> ${f2(ha)}`);
      continue;
    }

    // A walk off the top: one landing, at the camera's speed, and what falls.ron says of it.
    const handed = la === lb + 1 && Number.isFinite(last);
    check(
      `${id}.landing_handed`,
      handed,
      handed
        ? `one landing reached the body (${lb} -> ${la}) at ${f2(last)} m/s`
        : `${Number.isFinite(la) && Number.isFinite(lb) ? `${la - lb} landings` : "no landing count"} reached the body for a ${f2(drop.drop_m)} m drop (expected exactly 1). ` +
            "The walk's landing never got to player_landed: the lib.rs hand-off (take_landing -> falls::player_landed), FallWatch treating the walk as a placement, or the walk reporting no impact"
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
      : `PASS  ${result.checks.length}/${result.checks.length}: falls hurt in the running game the way falls.ron says, in both modes, and a placement's drop hurt nobody`
  );
  return lines;
}

module.exports = {
  LIMITS,
  readFalls,
  readGravity,
  readEffects,
  readStructureTypes,
  readHomeStructures,
  pickStairs,
  pickLadder,
  readTreeData,
  readSpeed,
  harmAt,
  woundAt,
  expectFall,
  dropOf,
  judgeFalls,
  verdictLines,
};
