// The furniture and machines placed in a home, as the footprints the player's walk stops at
// (2026-11-14, BUG-306: the storeys rig walked its bedroom and wall legs through a shelf and a
// wardrobe, which the walk has stopped at since 2026-11-13).
//
// The game turns every placed machine into a turned box on the floor
// (crates/humanity-ship/src/crowd_nav/mesh.rs `Footprint::of_placed`): a box its size, a cylinder
// or a sphere its radius (size.0) each way, a pyramid its base; and a piece stops a body when it
// stands across the body's knee-to-eye band (`Footprint::stops_body`, the walls' rule). Since
// v0.1559.0 the player's walk stops at those boxes the way the crowd's does (src/engine/
// crowd_walk.rs `furniture_walls_near`), so a rig that plans a walk through a home must keep its
// route a body's radius clear of them, or the walk stops where the rig expected it to pass.
//
// This reads data/machines/<home>.ron (its `catalog` of shapes and sizes, its `instances` and its
// `arrays`, expanded as `MachineHome::all_instances` does) and gives each placement's footprint
// in the home's own metres (the instances' offsets are zone-local to the home's zone, "home",
// whose corner is the home's origin: `machines::zone_world_pos`, less its 0.3 m clamp, which no
// piece inside the house reaches). A screen resized in the build editor (`screen_size_m`) is read
// at its catalog size; the homes in data/ have none.
//
// Pure: each reader takes a file's text. Tested in scripts/tests/home-furniture.test.js.

"use strict";

/** The text with its `//` and block comments taken out, leaving strings whole (a URL in a label). */
function stripComments(text) {
  const s = String(text || "");
  let out = "";
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (c === '"') {
      let j = i + 1;
      while (j < s.length && s[j] !== '"') j += s[j] === "\\" ? 2 : 1;
      out += s.slice(i, j + 1);
      i = j + 1;
    } else if (c === "/" && s[i + 1] === "/") {
      while (i < s.length && s[i] !== "\n") i++;
    } else if (c === "/" && s[i + 1] === "*") {
      const end = s.indexOf("*/", i + 2);
      i = end < 0 ? s.length : end + 2;
    } else {
      out += c;
      i++;
    }
  }
  return out;
}

/** The index of the bracket that closes the one at `open`, skipping strings; -1 if none. */
function closer(s, open) {
  let depth = 0;
  for (let i = open; i < s.length; i++) {
    const c = s[i];
    if (c === '"') {
      i++;
      while (i < s.length && s[i] !== '"') i += s[i] === "\\" ? 2 : 1;
    } else if (c === "(" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === "]" || c === "}") {
      if (--depth === 0) return i;
    }
  }
  return -1;
}

/** Where `key:` stands at the top level of `body` (outside every bracket and string): the index
 *  just past its colon, or -1. */
function topKey(body, key) {
  const re = new RegExp(`^${key}\\s*:`);
  let depth = 0;
  for (let i = 0; i < body.length; i++) {
    const c = body[i];
    if (c === '"') {
      i++;
      while (i < body.length && body[i] !== '"') i += body[i] === "\\" ? 2 : 1;
    } else if (c === "(" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === "]" || c === "}") depth--;
    else if (depth === 0 && /\w/.test(c) && !/\w/.test(body[i - 1] || "")) {
      const m = re.exec(body.slice(i, i + key.length + 8));
      if (m) return i + m[0].length;
    }
  }
  return -1;
}

/** The bracketed value (its inside) that follows top-level `key:`, past an optional `Some(`, or null. */
function bracketed(body, key) {
  let at = topKey(body, key);
  if (at < 0) return null;
  const rest = body.slice(at);
  const some = /^\s*Some\s*\(/.exec(rest);
  if (some) at += some[0].length;
  const open = body.slice(at).search(/[([{]/);
  if (open < 0) return null;
  const start = at + open;
  const end = closer(body, start);
  return end < 0 ? null : body.slice(start + 1, end);
}
const numbers = (inside) => (inside === null ? null : inside.split(",").map((v) => Number(v.trim())).filter((v) => Number.isFinite(v)));
function scalar(body, key) {
  const at = topKey(body, key);
  if (at < 0) return null;
  const m = /^\s*(?:Some\s*\(\s*)?("(?:[^"\\]|\\.)*"|-?[\d.]+)/.exec(body.slice(at));
  if (!m) return null;
  return m[1][0] === '"' ? m[1].slice(1, -1) : Number(m[1]);
}

/** The top-level `( ... )` items of a list's inside, strings skipped. */
function items(inside) {
  const out = [];
  const s = inside || "";
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '"') {
      i++;
      while (i < s.length && s[i] !== '"') i += s[i] === "\\" ? 2 : 1;
    } else if (s[i] === "(") {
      const end = closer(s, i);
      if (end < 0) break;
      out.push(s.slice(i + 1, end));
      i = end;
    }
  }
  return out;
}

/**
 * A machine home (data/machines/<home>.ron): { catalog: { name: { shape, size, label } },
 * placements: [{ id, machine, offset: [x, y, z], rotation, zone }] } with its arrays expanded
 * (`MachineHome::all_instances`: row r, column c at origin + (c * spacing.0, 0, r * spacing.1),
 * id `<prefix>_<index>`, unturned).
 */
function readMachineHome(text) {
  const t = stripComments(text);
  const root = t.slice(t.indexOf("(") + 1, closer(t, t.indexOf("(")));
  const catalog = {};
  const cat = bracketed(root, "catalog") || "";
  const entry = /"([^"]+)"\s*:\s*\(/g;
  let m;
  while ((m = entry.exec(cat))) {
    const open = m.index + m[0].length - 1;
    const end = closer(cat, open);
    if (end < 0) break;
    const def = cat.slice(open + 1, end);
    catalog[m[1]] = { shape: scalar(def, "shape") || "box", size: numbers(bracketed(def, "size")), label: scalar(def, "label") || "" };
    entry.lastIndex = end + 1;
  }
  const placements = items(bracketed(root, "instances")).map((it) => ({
    id: scalar(it, "id"),
    machine: scalar(it, "machine"),
    offset: numbers(bracketed(it, "offset")),
    rotation: scalar(it, "rotation") || 0,
    zone: scalar(it, "zone") || "",
  }));
  for (const arr of items(bracketed(root, "arrays"))) {
    const origin = numbers(bracketed(arr, "origin"));
    const spacing = numbers(bracketed(arr, "spacing"));
    const rows = scalar(arr, "rows") || 0;
    const cols = scalar(arr, "cols") || 0;
    let idx = 0;
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        placements.push({
          id: `${scalar(arr, "id_prefix")}_${idx++}`,
          machine: scalar(arr, "machine"),
          offset: [origin[0] + c * spacing[0], origin[1], origin[2] + r * spacing[1]],
          rotation: 0,
          zone: scalar(arr, "zone") || "",
        });
      }
    }
  }
  return { catalog, placements };
}

/**
 * Each placement's footprint, `Footprint::of_placed`: { id, machine, label, centre: [x, z],
 * half: [hx, hz], yaw, y0, y1 } in the home's metres. A placement whose machine the catalog does
 * not have is left out, as `MachineHome::placements` leaves it.
 */
function footprints(machineHome) {
  const out = [];
  for (const p of machineHome.placements) {
    const def = machineHome.catalog[p.machine];
    if (!def || !Array.isArray(def.size) || def.size.length < 3 || !Array.isArray(p.offset)) continue;
    const [sx, sy, sz] = def.size;
    const [x, y, z] = p.offset;
    const [half, y0, y1] =
      def.shape === "sphere" ? [[sx, sx], y, y + 2 * sx] : def.shape === "cylinder" ? [[sx, sx], y, y + sy] : def.shape === "pyramid" ? [[sx / 2, sx / 2], y, y + sy] : [[sx / 2, sz / 2], y, y + sy];
    out.push({ id: p.id, machine: p.machine, label: def.label || p.machine, centre: [x, z], half, yaw: (p.rotation * Math.PI) / 180, y0, y1 });
  }
  return out;
}

/** The floor point in the footprint's own frame (`Footprint::local`). */
function local(fp, x, z) {
  const dx = x - fp.centre[0];
  const dz = z - fp.centre[1];
  const s = Math.sin(fp.yaw);
  const c = Math.cos(fp.yaw);
  return [c * dx - s * dz, s * dx + c * dz];
}
/** The footprint's frame point in the home (glam's turn about +Y, `Footprint::walls`). */
function world(fp, lx, lz) {
  const s = Math.sin(fp.yaw);
  const c = Math.cos(fp.yaw);
  return [fp.centre[0] + c * lx + s * lz, fp.centre[1] - s * lx + c * lz];
}

/** The footprint's four corners in the home, [x, z] each. */
function corners(fp) {
  const [hx, hz] = fp.half;
  return [
    world(fp, -hx, -hz),
    world(fp, hx, -hz),
    world(fp, hx, hz),
    world(fp, -hx, hz),
  ];
}

/** How far (x, z) is from the footprint's edge: 0 on it, below 0 inside (`Footprint::distance`). */
function distance(fp, x, z) {
  const [lx, lz] = local(fp, x, z);
  const qx = Math.abs(lx) - fp.half[0];
  const qz = Math.abs(lz) - fp.half[1];
  return Math.hypot(Math.max(qx, 0), Math.max(qz, 0)) + Math.min(Math.max(qx, qz), 0);
}

/** Whether the piece stands across the knee-to-eye band of a body with its feet at `feet`
 *  (`Footprint::stops_body`): a rug under the knee is walked over, a lamp over the head under. */
const stopsBody = (fp, feet, eye, knee) => fp.y0 < feet + eye && fp.y1 > feet + knee;

/** Distance from (x, z) to the segment a..b. */
function segPoint(a, b, x, z) {
  const dx = b[0] - a[0];
  const dz = b[1] - a[1];
  const t = Math.max(0, Math.min(1, ((x - a[0]) * dx + (z - a[1]) * dz) / (dx * dx + dz * dz || 1)));
  return Math.hypot(x - (a[0] + t * dx), z - (a[1] + t * dz));
}

/**
 * How close the straight walk p->q (or a body placed at p, with no q) comes to the footprint: 0
 * when it crosses it, else the least distance. Exact: the segment is clipped against the box in the box's frame (a crossing), and two
 * convex shapes that do not meet are closest at a corner of one of them (the segment's ends, or
 * the box's four corners against the segment).
 */
function clearance(fp, p, q = p) {
  const [px, pz] = local(fp, p[0], p[1]);
  const [qx, qz] = local(fp, q[0], q[1]);
  let t0 = 0;
  let t1 = 1;
  const clip = (d, lo, hi, o) => {
    if (Math.abs(d) < 1e-12) return o >= lo && o <= hi;
    let a = (lo - o) / d;
    let b = (hi - o) / d;
    if (a > b) [a, b] = [b, a];
    t0 = Math.max(t0, a);
    t1 = Math.min(t1, b);
    return t0 <= t1;
  };
  const [hx, hz] = fp.half;
  if (clip(qx - px, -hx, hx, px) && clip(qz - pz, -hz, hz, pz)) return 0;
  let best = Math.min(Math.max(distance(fp, p[0], p[1]), 0), Math.max(distance(fp, q[0], q[1]), 0));
  for (const [x, z] of corners(fp)) best = Math.min(best, segPoint(p, q, x, z));
  return best;
}

module.exports = { stripComments, readMachineHome, footprints, corners, distance, stopsBody, clearance };
