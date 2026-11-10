// Does a road lie on its bed, and does a building stand on its pad? The judgement half of the
// probe rig's `bed_probe` vantage check (whole-Earth maps, the rung between 2 and 3, 2026-11-09;
// docs/design/whole-earth-maps.md sections 6.1 and 6.2). The game writes the numbers
// (src/engine/bed_probe.rs, the showcase verb {"bed_probe":"road;lat,lon"} or
// {"bed_probe":"building;lat,lon"} -> debug/bed_probe.json); this reads them. Pure: no game, no
// disk, so scripts/tests/bed-probe-check.test.js runs it in `just rig-tests`.
//
// WHY. Until this rung a road's ribbon floated 0.18 m over whatever the ground did under it (on a
// side slope: buried at the uphill edge, floating at the downhill one) and a building stood on the
// lowest ground under its walls with a 2.5 m skirt buried below. A picture shows the edge sinking
// into the hill; this says by how much, from the game's own numbers.
//
// A vantage opts in with:
//   "bed_probe": { "road": [lat, lon], "rules": ["road_on_bed"] }
//   "bed_probe": { "building": [lat, lon], "rules": ["building_on_pad"] }
//
// THE RULES:
//   road_on_bed      across the carriageway the drawn ground (the patch drawn there this frame)
//                    spreads by at most `flat_m`, and the ribbon lies on it: never under it (no
//                    buried edge) and never more than `float_m` over it (no floating edge);
//   building_on_pad  at every corner the drawn ground is the mesher's floor within `pad_m` (the
//                    floor stands on its pad, not sunk into the hill or floating), the wall bottom
//                    is never above the drawn ground at a corner (no daylight under a wall), and it
//                    reaches no deeper than the lowest of the drawn and natural ground at the corners
//                    by more than `skirt_m` (no buried skirt).
//
// MISCONFIGURED, so a flat street can never pass for a sloped one: the natural ground (the land
// layer, no bed) must fall at least `min_rise_m` across the carriageway, or spread at least
// `min_spread_m` under the footprint's corners; the found road or building must be within
// `max_from_asked_m` of the asked point; and every sample must have a drawn ground.

const DEFAULTS = Object.freeze({
  flat_m: 0.05,
  float_m: 0.08,
  bury_m: 0.005,
  pad_m: 0.05,
  skirt_m: 0.05,
  min_rise_m: 0.3,
  min_spread_m: 0.5,
  max_from_asked_m: 6,
});

const RULES = Object.freeze(["road_on_bed", "building_on_pad"]);
const f3 = (x) => (typeof x === "number" ? x.toFixed(3) : String(x));

/**
 * Judge one readout. Returns { ok, misconfigured, failures: [string], notes: [string], rules:
 * {name: {ok, ...numbers}} }. `readout` is debug/bed_probe.json as parsed; `spec` the vantage's
 * `bed_probe`; `opts` overrides DEFAULTS (tests).
 */
function judgeBeds(readout, spec, opts = {}) {
  const o = { ...DEFAULTS, ...opts };
  const out = { ok: true, misconfigured: false, failures: [], notes: [], rules: {} };
  const bad = (msg, misconf = true) => {
    out.ok = false;
    if (misconf) out.misconfigured = true;
    out.failures.push(misconf ? `MISCONFIGURED: ${msg}` : msg);
    return out;
  };
  if (!spec || typeof spec !== "object") return bad("the vantage has no bed_probe spec");
  if (!readout || typeof readout !== "object") return bad("no readout");
  if (readout.error) return bad(`the game could not answer: ${readout.error}`);
  const rules = Array.isArray(spec.rules) ? spec.rules : [];
  if (!rules.length || rules.some((r) => !RULES.includes(r))) return bad(`rules must be some of ${RULES.join(", ")}, not ${JSON.stringify(spec.rules)}`);
  const kind = spec.road ? "road" : spec.building ? "building" : null;
  if (!kind) return bad("the spec names neither a road nor a building");
  if (readout.kind !== kind) return bad(`asked for a ${kind}, the readout is a ${readout.kind}`);
  out.notes.push(`beds ${readout.beds ? "on" : "OFF (the red run)"}, walk depth ${readout.walk_depth}, pack depth ${readout.pack_depth}`);

  if (kind === "road") {
    if (!(readout.from_asked_m <= o.max_from_asked_m)) return bad(`the nearest road is ${f3(readout.from_asked_m)} m from the asked point (at most ${o.max_from_asked_m})`);
    const lane = (readout.samples || []).filter((s) => typeof s.ribbon_m === "number");
    if (lane.length < 5) return bad(`only ${lane.length} samples across the carriageway`);
    if (lane.some((s) => typeof s.drawn_m !== "number" || typeof s.raw_m !== "number")) return bad("a carriageway sample has no drawn or natural ground (no patch drawn there, or no land data)");
    const raws = lane.map((s) => s.raw_m);
    const rise = Math.max(...raws) - Math.min(...raws);
    if (rise < o.min_rise_m) return bad(`the natural ground falls only ${f3(rise)} m across way ${readout.osm_id} (at least ${o.min_rise_m}): not a road on a slope`);
    const drawn = lane.map((s) => s.drawn_m);
    const spread = Math.max(...drawn) - Math.min(...drawn);
    const gaps = lane.map((s) => s.ribbon_m - s.drawn_m);
    const under = -Math.min(...gaps);
    const over = Math.max(...gaps);
    const ok = spread <= o.flat_m && under <= o.bury_m && over <= o.float_m;
    out.rules.road_on_bed = { ok, natural_rise_m: rise, drawn_spread_m: spread, ribbon_buried_m: Math.max(0, under), ribbon_floating_m: over, leaf_depths: [...new Set(lane.map((s) => s.leaf_depth))] };
    out.notes.push(`way ${readout.osm_id} class ${readout.class}: natural rise ${f3(rise)} m across ${f3(2 * readout.half_w_m)} m; drawn spread ${f3(spread)} m; ribbon ${f3(Math.max(0, under))} m under, ${f3(over)} m over the drawn ground`);
    if (rules.includes("road_on_bed") && !ok) {
      if (spread > o.flat_m) bad(`road_on_bed: the ground across way ${readout.osm_id} spreads ${f3(spread)} m (at most ${o.flat_m}): no flat bed`, false);
      if (under > o.bury_m) bad(`road_on_bed: the ribbon's edge lies ${f3(under)} m under the drawn ground (a buried edge)`, false);
      if (over > o.float_m) bad(`road_on_bed: the ribbon's edge floats ${f3(over)} m over the drawn ground (at most ${o.float_m})`, false);
    }
  } else {
    const corners = readout.corners || [];
    if (corners.length < 3) return bad(`only ${corners.length} corners`);
    if (corners.some((c) => typeof c.drawn_m !== "number" || typeof c.raw_m !== "number")) return bad("a corner has no drawn or natural ground");
    const raws = corners.map((c) => c.raw_m);
    const nspread = Math.max(...raws) - Math.min(...raws);
    if (nspread < o.min_spread_m) return bad(`the natural ground spreads only ${f3(nspread)} m under building ${readout.osm_id} (at least ${o.min_spread_m}): not a building on a slope`);
    const floor = readout.mesh_floor_m;
    const bottom = readout.mesh_bottom_m;
    const offFloor = Math.max(...corners.map((c) => Math.abs(c.drawn_m - floor)));
    const lowest = Math.min(...corners.map((c) => c.drawn_m));
    const deepest = Math.min(lowest, ...raws);
    const daylight = bottom - lowest;
    const skirt = deepest - bottom;
    const ok = offFloor <= o.pad_m && daylight <= 0.005 && skirt <= o.skirt_m;
    out.rules.building_on_pad = { ok, natural_spread_m: nspread, floor_m: floor, corner_off_floor_m: offFloor, wall_daylight_m: Math.max(0, daylight), buried_skirt_m: Math.max(0, skirt) };
    out.notes.push(`building ${readout.osm_id}: natural spread ${f3(nspread)} m; floor ${f3(floor)} m; corners off it by ${f3(offFloor)} m; wall bottom ${f3(bottom)} m (${f3(Math.max(0, skirt))} m below the lowest ground)`);
    if (rules.includes("building_on_pad") && !ok) {
      if (offFloor > o.pad_m) bad(`building_on_pad: a corner's ground is ${f3(offFloor)} m off the floor (at most ${o.pad_m}): the building is sunk into the slope or floats`, false);
      if (daylight > 0.005) bad(`building_on_pad: the wall bottom stands ${f3(daylight)} m over the ground at a corner (daylight under a wall)`, false);
      if (skirt > o.skirt_m) bad(`building_on_pad: the wall reaches ${f3(skirt)} m below the lowest ground (a buried skirt)`, false);
    }
  }
  return out;
}

/** One line for the log. */
function summarize(v) {
  if (v.misconfigured) return `MISCONFIGURED (${v.failures.join("; ")})`;
  const parts = Object.entries(v.rules).map(([k, r]) => `${k} ${r.ok ? "ok" : "FAILED"}`);
  return `${v.ok ? "ok" : "FAILED"}: ${parts.join(", ")}`;
}

module.exports = { judgeBeds, summarize, DEFAULTS, RULES };
