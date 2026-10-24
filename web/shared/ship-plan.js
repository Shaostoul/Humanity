// The ship's PLAN for the web Maps page (Maps increment 3, "into the ship, deck by deck",
// docs/design/maps-zoom-to-anything.md): the web twin of the app's level view
// (src/gui/pages/maps/ship_view.rs). It reads /data/maps/ship_plan.json, which the APP writes
// from the shipped ship by its own rules (ship/levels.rs, detect_rooms, data/rooms.ron; the
// test `the_web_ship_plan_is_current` keeps it current), because the web cannot read the RON
// the ship is made of.
//
// A level is shown as the app shows it: every body's walls that stand in the storey the level
// is on (`wallInStorey`, the app's `wall_in_storey`), the body's outline, and the rooms whose
// floor is on the level (the app's `rooms_on_level`), filled in their room types' colours and
// named as the game names them. The web is not the game, so there is no "you" on it; your
// home (the plot the shipped ship puts it on) is outlined in the accent colour.
//
// Coordinates are SHIP metres: x across, z along the ship (its bow, -z, up the screen, as the
// app's plan has it), y up.
(function (root) {
  'use strict';

  const STOREY_EPS = 0.01; // home_structure.rs STOREY_EPS

  /** Which of `storeys` ([[floor, top], ...]) height `y` is on (home_structure::storey_index). */
  function storeyIndex(storeys, y) {
    let k = 0;
    storeys.forEach((s, i) => { if (s[0] <= y + STOREY_EPS) k = i; });
    return k;
  }

  /** True when a wall [ax, az, bx, bz, base, top] stands in storey k (home_structure::wall_in_storey). */
  function wallInStorey(storeys, wall, k) {
    const on = storeyIndex(storeys, wall[4]);
    return k === on || (k > on && k < storeys.length && wall[5] > storeys[k][0] + STOREY_EPS);
  }

  /** Which level (an index into `levels`) height y is on (ship/levels.rs level_at). */
  function levelAt(levels, y) {
    if (!levels.length) return null;
    let i = 0;
    levels.forEach((l, j) => { if (l.floor <= y + STOREY_EPS) i = j; });
    return i;
  }

  /** A body with its design resolved: {id, label, origin, size, storeys, walls}. */
  function resolve(plan, b) {
    const d = b.design ? plan.designs[b.design] : b;
    return { id: b.id, label: b.label, origin: b.origin, size: d.size, storeys: d.storeys, walls: d.walls };
  }

  /** Every body's wall segments on the level with this ordinal, ship metres: [[ax, az, bx, bz], ...]. */
  function wallsAt(plan, ordinal) {
    const level = plan.levels.find((l) => l.ordinal === ordinal);
    if (!level) return [];
    const out = [];
    for (const raw of plan.bodies) {
      const b = resolve(plan, raw);
      const [ox, oy, oz] = b.origin;
      const k = storeyIndex(b.storeys, level.floor - oy);
      for (const w of b.walls) if (wallInStorey(b.storeys, w, k)) out.push([ox + w[0], oz + w[1], ox + w[2], oz + w[3]]);
    }
    return out;
  }

  /** The rooms on the level with this ordinal (their floor on it). */
  function roomsOn(plan, ordinal) {
    return plan.rooms.filter((r) => r.level === ordinal);
  }

  /** Whether a room is named on the plan: the data gives it a name (the app's `room_is_named`);
   *  a corridor tube or a zone's leftover floor carries only its id. */
  function roomIsNamed(r) {
    return r.name !== r.id;
  }

  /** The ship's box in x and z: [min x, min z, max x, max z]. */
  function bounds(plan) {
    const b = [Infinity, Infinity, -Infinity, -Infinity];
    for (const raw of plan.bodies) {
      const r = resolve(plan, raw);
      b[0] = Math.min(b[0], r.origin[0]);
      b[1] = Math.min(b[1], r.origin[2]);
      b[2] = Math.max(b[2], r.origin[0] + r.size[0]);
      b[3] = Math.max(b[3], r.origin[2] + r.size[2]);
    }
    return b;
  }

  /** Draw the plan on `ctx`: `view` = {cx, cz, scale (px a metre), w, h}, `ordinal` the level
   *  shown (null: every body's outline only), `colours` = {edge, fill, alpha, label, halo, accent,
   *  space}; the body with id "home" (the assembled home's zone) is outlined as yours. Returns
   *  how many rooms it named. */
  function draw(ctx, plan, view, ordinal, colours) {
    const sx = (x) => view.w / 2 + (x - view.cx) * view.scale;
    const sz = (z) => view.h / 2 + (z - view.cz) * view.scale;
    ctx.fillStyle = colours.space;
    ctx.fillRect(0, 0, view.w, view.h);
    // Rooms first, under the walls.
    const rooms = ordinal === null ? [] : roomsOn(plan, ordinal);
    for (const r of rooms) {
      const c = r.fill ? `rgba(${Math.round(r.fill[0] * 255)}, ${Math.round(r.fill[1] * 255)}, ${Math.round(r.fill[2] * 255)}, ${colours.alpha})` : colours.fill;
      ctx.fillStyle = c;
      ctx.fillRect(sx(r.min[0]), sz(r.min[1]), (r.max[0] - r.min[0]) * view.scale, (r.max[1] - r.min[1]) * view.scale);
    }
    // Every body's outline; yours in the accent colour.
    ctx.lineWidth = 1.5;
    for (const raw of plan.bodies) {
      const b = resolve(plan, raw);
      ctx.strokeStyle = b.id === 'home' ? colours.accent : colours.edge;
      ctx.strokeRect(sx(b.origin[0]), sz(b.origin[2]), b.size[0] * view.scale, b.size[2] * view.scale);
    }
    if (ordinal !== null && view.scale > 0.6) {
      ctx.strokeStyle = colours.edge;
      ctx.beginPath();
      for (const s of wallsAt(plan, ordinal)) {
        ctx.moveTo(sx(s[0]), sz(s[1]));
        ctx.lineTo(sx(s[2]), sz(s[3]));
      }
      ctx.stroke();
    }
    // Names inside their rooms, the bigger rooms first; a name that does not fit is left out.
    let named = 0;
    if (view.scale > 1.5) {
      ctx.font = '12px sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      const taken = [];
      const sorted = rooms.slice().sort((a, b) => (b.max[0] - b.min[0]) * (b.max[1] - b.min[1]) - (a.max[0] - a.min[0]) * (a.max[1] - a.min[1]));
      for (const r of sorted) {
        if (!roomIsNamed(r)) continue;
        const w = ctx.measureText(r.name).width;
        const roomW = (r.max[0] - r.min[0]) * view.scale;
        if (w > roomW * 0.95) continue;
        const x = sx((r.min[0] + r.max[0]) / 2);
        const y = sz((r.min[1] + r.max[1]) / 2);
        const box = [x - w / 2, y - 8, x + w / 2, y + 8];
        if (taken.some((t) => t[0] < box[2] && box[0] < t[2] && t[1] < box[3] && box[1] < t[3])) continue;
        taken.push(box);
        ctx.fillStyle = colours.halo;
        for (const [dx, dy] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) ctx.fillText(r.name, x + dx, y + dy);
        ctx.fillStyle = colours.label;
        ctx.fillText(r.name, x, y);
        named++;
      }
    }
    return named;
  }

  /** Fetch the plan the app wrote (/data/maps/ship_plan.json). */
  function loadPlan() {
    return fetch('/data/maps/ship_plan.json').then((r) => (r.ok ? r.json() : Promise.reject(new Error(`ship_plan.json: ${r.status}`))));
  }

  const api = { storeyIndex, wallInStorey, levelAt, wallsAt, roomsOn, roomIsNamed, bounds, draw, loadPlan };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.HosShipPlan = api;
  }
})(typeof window !== 'undefined' ? window : this);
