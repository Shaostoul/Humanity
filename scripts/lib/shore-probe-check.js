// Is the drawn ground at a coast what the real coast is? The judgement half of the probe rig's
// `shore_probe` vantage check (whole-Earth maps rung 1, 2026-11-09;
// docs/design/whole-earth-maps.md section 5.5). The game writes the numbers
// (src/engine/shore_probe.rs, the showcase verb {"shore_probe":"lat,lon;lat,lon"} ->
// debug/shore_probe.json); this reads them. Pure: no game, no disk, so
// scripts/tests/shore-probe-check.test.js runs it in `just rig-tests`.
//
// WHY. The operator, 2026-11-08: "in the water front initially didn't have any water and then we
// had weird sheer cliffs along water area instead of nice slopes like in real life." A picture
// shows a wall; this says how tall, where, and against what the data says the slope should be.
//
// A vantage opts in with, for a shore (land end first):
//   "shore_probe": { "land": [lat, lon], "water": [lat, lon], "rules": ["wet", "no_false_wall"],
//                    "coast": { "at": [lat, lon], "source": "..." } }      // coast: see below
// or, for a transect laid along the water:
//   "shore_probe": { "along_water": [[lat, lon], [lat, lon]], "rules": ["no_seam"],
//                    "crosses_lat": 47.59, "water_source": "..." }
//
// THE RULES (section 5.5, and `dry`, added in rung 1):
//   wet           every sample more than 15 m on the water side of the coast is at least 0.3 m
//                 below sea level (no dry harbour);
//   dry           every sample more than 15 m on the land side of the coast is at least 0.3 m
//                 above sea level (no drowned land). Added 2026-11-09 when coast-norderney-beach
//                 PASSED the plan's wet and shallow on a build that draws the whole island as a
//                 sea-level flat whose northern 830 m lie under water: coarse data smears a coast
//                 flat, so a shallow coast's rules pass it, and only this rule sees the island is
//                 gone. Not for a polder or a depression that is dry below sea level (the Dead Sea
//                 shore): there the question is whether water is drawn, a later rule;
//   no_false_wall within 60 m of the coast, no 10 m stretch rises (going inland) more than the
//                 unmodified land layer's own rise over the same stretch plus 2 m;
//   kept_cliff    the ground reaches at least 60 m within 150 m inland of the coast;
//   shallow       within 300 m either side of the coast the slope stays under 3 degrees and no
//                 step between samples exceeds 0.3 m;
//   no_seam       no step over 0.5 m between samples anywhere along the transect.
//
// WHERE THE COAST IS, AND WHY A VANTAGE CAN BE MISCONFIGURED. Every vantage first checks its ends
// against coastline data, so a wrong coordinate can never pass. The game reports, per sample, what
// its own coastline data says (`class`) and from where (`class_source`): a region's OSM water
// polygons ("region:<name>:<cell>m", about 9 m cells), or the connected-ocean mask
// ("ocean_mask:0.05deg", about 5.5 km cells). Only the first can place a coast to the metre.
// - Both ends on a region's polygons: the land end must read land and the water end water, and the
//   coast is the last land-to-water change before the water end.
// - Either end only on the coarse mask: the game cannot say where this coast is (that gap is what
//   rung 4's packs close), so the vantage must DECLARE the real coastline crossing, `coast.at`,
//   with its `source`; the declared point must lie on the transect and between its ends. Where
//   both exist they must agree. The game's coarse reading is still reported, as a note.
// - Along the water: each end on region polygons must read water; an end on the coarse mask needs
//   a `water_source` saying why it is water; `crosses_lat`, when given, must be crossed.

const DEFAULTS = Object.freeze({
  wet_offshore_m: 15,
  wet_depth_m: 0.3,
  dry_inland_m: 15,
  dry_height_m: 0.3,
  wall_window_m: 60,
  wall_stretch_m: 10,
  wall_slack_m: 2,
  cliff_height_m: 60,
  cliff_within_m: 150,
  shallow_window_m: 300,
  shallow_max_deg: 3,
  shallow_stretch_m: 10,
  shallow_max_step_m: 0.3,
  seam_max_step_m: 0.5,
  // A declared coast farther than this from every sample is not on the transect.
  coast_max_offset_m: 50,
  // A declared coast and the game's region coastline must agree this well along the transect.
  coast_agree_m: 40,
  // The readout's ends must be the spec's ends to within this.
  end_tol_m: 1,
});

const RULES = Object.freeze(["wet", "dry", "no_false_wall", "kept_cliff", "shallow", "no_seam"]);
const SHORE_RULES = Object.freeze(["wet", "dry", "no_false_wall", "kept_cliff", "shallow"]);

const R_EARTH = 6371000;
/** Great-circle distance in metres between two [lat, lon] points (haversine). */
function distM(a, b) {
  const toR = Math.PI / 180;
  const dLat = (b[0] - a[0]) * toR;
  const dLon = (b[1] - a[1]) * toR;
  const h = Math.sin(dLat / 2) ** 2 + Math.cos(a[0] * toR) * Math.cos(b[0] * toR) * Math.sin(dLon / 2) ** 2;
  return 2 * R_EARTH * Math.asin(Math.min(1, Math.sqrt(h)));
}

const fine = (src) => typeof src === "string" && src.startsWith("region:");
const isWater = (c) => c === "sea" || c === "lake";
const f1 = (x) => (typeof x === "number" ? x.toFixed(1) : String(x));
const f2 = (x) => (typeof x === "number" ? x.toFixed(2) : String(x));

/**
 * Judge one readout. Returns { ok, misconfigured, failures: [string], notes: [string], rules:
 * {name: {ok, ...numbers}}, coast: {...} }. `readout` is debug/shore_probe.json as parsed; `spec`
 * the vantage's `shore_probe`; `opts` overrides DEFAULTS (tests).
 */
function judgeShore(readout, spec, opts) {
  const o = Object.assign({}, DEFAULTS, opts || {});
  const failures = [];
  const notes = [];
  const rules = {};
  const bad = (msg) => ({ ok: false, misconfigured: true, failures: [`MISCONFIGURED: ${msg}`], notes, rules, coast: null });
  if (!spec || typeof spec !== "object") return bad("the vantage has no shore_probe spec");
  const wanted = Array.isArray(spec.rules) ? spec.rules : [];
  if (!wanted.length) return bad("the spec names no rules: a check of nothing would pass");
  const unknown = wanted.filter((r) => !RULES.includes(r));
  if (unknown.length) return bad(`unknown rule(s) ${unknown.join(", ")} (known: ${RULES.join(", ")})`);
  const along = Array.isArray(spec.along_water);
  const ends = along ? spec.along_water : [spec.land, spec.water];
  if (!ends.every((e) => Array.isArray(e) && e.length === 2 && e.every((v) => typeof v === "number"))) {
    return bad(along ? "along_water needs two [lat, lon] ends" : "a shore needs `land` and `water`, each [lat, lon]");
  }
  if (along && wanted.some((r) => SHORE_RULES.includes(r))) return bad(`${wanted.join(", ")}: shore rules need a land end and a water end, not along_water`);

  if (!readout || typeof readout !== "object") return { ok: false, misconfigured: false, failures: ["no readout"], notes, rules, coast: null };
  if (readout.error) return { ok: false, misconfigured: false, failures: [`the game could not measure: ${readout.error}`], notes, rules, coast: null };
  const S = Array.isArray(readout.samples) ? readout.samples : [];
  if (S.length < 3) return { ok: false, misconfigured: false, failures: [`only ${S.length} samples: the check would pass by measuring nothing`], notes, rules, coast: null };
  const off0 = distM(readout.from, ends[0]);
  const off1 = distM(readout.to, ends[1]);
  if (!(off0 <= o.end_tol_m && off1 <= o.end_tol_m)) {
    return bad(`the readout's transect (${readout.from} to ${readout.to}) is not the spec's (${ends[0]} to ${ends[1]})`);
  }
  const step = S.length > 1 ? S[1].s_m - S[0].s_m : 0;
  const first = S[0];
  const last = S[S.length - 1];
  const gameSays = (s) => `${s.class} (${s.class_source})`;

  // ── Where the coast is ──
  let sCoast = null;
  const coast = {};
  if (!along) {
    const endsFine = fine(first.class_source) && fine(last.class_source);
    if (endsFine) {
      if (isWater(first.class)) return bad(`the land end reads ${gameSays(first)} in the game's coastline`);
      if (!isWater(last.class)) return bad(`the water end reads ${gameSays(last)} in the game's coastline`);
      let j = S.length - 1;
      while (j > 0 && isWater(S[j].class)) j--;
      sCoast = (S[j].s_m + S[j + 1].s_m) / 2;
      coast.from = "game";
      coast.source = first.class_source;
    }
    const decl = spec.coast;
    if (decl && !(Array.isArray(decl.at) && decl.at.length === 2 && typeof decl.source === "string" && decl.source.trim())) {
      return bad("`coast` needs `at` [lat, lon] and a `source`");
    }
    if (decl) {
      let best = 0;
      let bestD = Infinity;
      S.forEach((s, i) => {
        const d = distM([s.lat, s.lon], decl.at);
        if (d < bestD) {
          bestD = d;
          best = i;
        }
      });
      if (bestD > o.coast_max_offset_m) return bad(`the declared coast ${decl.at} is ${f1(bestD)} m from the transect: it is not on it`);
      if (best === 0 || best === S.length - 1) return bad(`the declared coast ${decl.at} is at an end of the transect, not between the land end and the water end`);
      const sDecl = S[best].s_m;
      coast.declared_s_m = sDecl;
      coast.declared_source = decl.source;
      if (sCoast !== null && Math.abs(sDecl - sCoast) > o.coast_agree_m) {
        return bad(`the declared coast (${f1(sDecl)} m along) and the game's region coastline (${f1(sCoast)} m along) disagree by ${f1(Math.abs(sDecl - sCoast))} m`);
      }
      if (sCoast === null) {
        sCoast = sDecl;
        coast.from = "declared";
        coast.source = decl.source;
      }
    }
    if (sCoast === null) {
      const coarse = [first, last].filter((s) => !fine(s.class_source)).map((s) => s.class_source);
      return bad(
        `the game's only coastline data at the transect's ends is ${[...new Set(coarse)].join(", ")}, too coarse to place this coast; ` +
          "declare the real coastline crossing as shore_probe.coast { at, source }",
      );
    }
    for (const [name, s, want] of [["land end", first, "land"], ["water end", last, "water"]]) {
      if (!fine(s.class_source)) {
        const reads = isWater(s.class) ? "water" : s.class;
        notes.push(`the game's own coastline at the ${name} is ${s.class_source}, which reads ${reads}${reads === want ? "" : ` (the real ${name} is ${want})`}`);
      }
    }
    coast.s_m = sCoast;
  } else {
    for (const [name, s] of [["first end", first], ["last end", last]]) {
      if (fine(s.class_source)) {
        if (!isWater(s.class)) return bad(`the ${name} of an along-water transect reads ${gameSays(s)} in the game's coastline`);
      } else if (!(typeof spec.water_source === "string" && spec.water_source.trim())) {
        return bad(`the ${name} is only on ${s.class_source}, too coarse to say it is water; give \`water_source\` saying why it is`);
      } else {
        notes.push(`the ${name} is on ${s.class_source} (reads ${s.class}); water per: ${spec.water_source}`);
      }
    }
    if (typeof spec.crosses_lat === "number") {
      const [a, b] = [ends[0][0], ends[1][0]];
      if (!((a - spec.crosses_lat) * (b - spec.crosses_lat) < 0)) return bad(`the transect (${a} to ${b}) does not cross latitude ${spec.crosses_lat}`);
    }
  }

  // ── The rules ──
  const g = S.map((s) => s.ground_m);
  if (!g.every((v) => typeof v === "number" && Number.isFinite(v))) {
    return { ok: false, misconfigured: false, failures: ["a sample has no ground height"], notes, rules, coast };
  }
  const k = (len) => Math.max(1, Math.round(len / Math.max(step, 1e-6)));

  if (wanted.includes("wet")) {
    const water = S.map((s, i) => [s, i]).filter(([s]) => s.s_m - sCoast > o.wet_offshore_m);
    const dry = water.filter(([, i]) => g[i] > -o.wet_depth_m);
    const worst = dry.reduce((m, [s, i]) => (m === null || g[i] > g[m[1]] ? [s, i] : m), null);
    const r = { ok: water.length > 0 && dry.length === 0, measured: water.length, dry: dry.length };
    if (worst) {
      r.worst_m = g[worst[1]];
      r.worst_offshore_m = worst[0].s_m - sCoast;
    }
    rules.wet = r;
    if (!water.length) failures.push(`wet: no sample lies more than ${o.wet_offshore_m} m offshore: the check would pass by measuring nothing`);
    else if (dry.length) {
      failures.push(
        `wet: ${dry.length} of ${water.length} samples more than ${o.wet_offshore_m} m offshore stand above -${o.wet_depth_m} m; ` +
          `the highest is ${f2(r.worst_m)} m, ${f1(r.worst_offshore_m)} m offshore (dry where the coastline data says water)`,
      );
    }
  }

  if (wanted.includes("dry")) {
    const land = S.map((s, i) => [s, i]).filter(([s]) => sCoast - s.s_m > o.dry_inland_m);
    const drowned = land.filter(([, i]) => g[i] < o.dry_height_m);
    const worst = drowned.reduce((m, [s, i]) => (m === null || g[i] < g[m[1]] ? [s, i] : m), null);
    const r = { ok: land.length > 0 && drowned.length === 0, measured: land.length, drowned: drowned.length };
    if (worst) {
      r.worst_m = g[worst[1]];
      r.worst_inland_m = sCoast - worst[0].s_m;
      r.drowned_m = drowned.length * step;
      r.below_sea = drowned.filter(([, i]) => g[i] < 0).length;
    }
    rules.dry = r;
    if (!land.length) failures.push(`dry: no sample lies more than ${o.dry_inland_m} m inland: the check would pass by measuring nothing`);
    else if (drowned.length) {
      failures.push(
        `dry: ${drowned.length} of ${land.length} samples more than ${o.dry_inland_m} m inland stand below +${o.dry_height_m} m ` +
          `(${r.below_sea} of them below sea level); the lowest is ${f2(r.worst_m)} m, ${f1(r.worst_inland_m)} m inland (drowned where the coastline data says land)`,
      );
    }
  }

  if (wanted.includes("no_false_wall")) {
    const n = k(o.wall_stretch_m);
    let measured = 0;
    let worst = null;
    let unread = 0;
    for (let i = 0; i + n < S.length; i++) {
      const j = i + n; // i is landward of j (the land end comes first)
      if (Math.abs(S[i].s_m - sCoast) > o.wall_window_m || Math.abs(S[j].s_m - sCoast) > o.wall_window_m) continue;
      const ri = S[i].raw_m;
      const rj = S[j].raw_m;
      if (typeof ri !== "number" || typeof rj !== "number") {
        unread++;
        continue;
      }
      measured++;
      const drawnRise = g[i] - g[j];
      const rawRise = ri - rj;
      const excess = drawnRise - (rawRise + o.wall_slack_m);
      if (worst === null || excess > worst.excess) worst = { excess, drawnRise, rawRise, at_m: (S[i].s_m + S[j].s_m) / 2 - sCoast, raw_source: S[i].raw_source };
    }
    const r = { ok: measured > 0 && (worst === null || worst.excess <= 0), measured, unread };
    if (worst) Object.assign(r, { worst_excess_m: worst.excess, worst_drawn_rise_m: worst.drawnRise, worst_raw_rise_m: worst.rawRise, worst_at_m: worst.at_m, raw_source: worst.raw_source });
    rules.no_false_wall = r;
    if (!measured) failures.push(`no_false_wall: no ${o.wall_stretch_m} m stretch within ${o.wall_window_m} m of the coast had the raw land layer at both ends: nothing measured`);
    else if (worst.excess > 0) {
      failures.push(
        `no_false_wall: the drawn ground rises ${f2(worst.drawnRise)} m over ${o.wall_stretch_m} m going inland ${f1(worst.at_m)} m from the coast ` +
          `(negative is offshore), where the land layer (${worst.raw_source}) rises ${f2(worst.rawRise)} m: a wall ${f2(worst.excess)} m past the ${o.wall_slack_m} m allowance`,
      );
    }
  }

  if (wanted.includes("kept_cliff")) {
    const inland = S.map((s, i) => [s, i]).filter(([s]) => sCoast - s.s_m >= 0 && sCoast - s.s_m <= o.cliff_within_m);
    const top = inland.reduce((m, [, i]) => Math.max(m, g[i]), -Infinity);
    const r = { ok: inland.length > 0 && top >= o.cliff_height_m, measured: inland.length, top_m: inland.length ? top : null };
    rules.kept_cliff = r;
    if (!inland.length) failures.push(`kept_cliff: no sample within ${o.cliff_within_m} m inland of the coast: nothing measured`);
    else if (top < o.cliff_height_m) {
      failures.push(`kept_cliff: within ${o.cliff_within_m} m inland the ground reaches only ${f1(top)} m, not ${o.cliff_height_m} m: the cliff is gone (a ramp where the data has a face)`);
    }
  }

  if (wanted.includes("shallow")) {
    const n = k(o.shallow_stretch_m);
    const inWin = (i) => Math.abs(S[i].s_m - sCoast) <= o.shallow_window_m;
    let maxDeg = 0;
    let maxDegAt = null;
    let maxStep = 0;
    let maxStepAt = null;
    let measured = 0;
    for (let i = 0; i + 1 < S.length; i++) {
      if (!inWin(i) || !inWin(i + 1)) continue;
      measured++;
      const st = Math.abs(g[i + 1] - g[i]);
      if (st > maxStep) {
        maxStep = st;
        maxStepAt = S[i].s_m - sCoast;
      }
      if (i + n < S.length && inWin(i + n)) {
        const run = S[i + n].s_m - S[i].s_m;
        const deg = (Math.atan2(Math.abs(g[i + n] - g[i]), run) * 180) / Math.PI;
        if (deg > maxDeg) {
          maxDeg = deg;
          maxDegAt = (S[i].s_m + S[i + n].s_m) / 2 - sCoast;
        }
      }
    }
    const r = { ok: measured > 0 && maxDeg < o.shallow_max_deg && maxStep <= o.shallow_max_step_m, measured, max_slope_deg: maxDeg, max_slope_at_m: maxDegAt, max_step_m: maxStep, max_step_at_m: maxStepAt };
    rules.shallow = r;
    if (!measured) failures.push(`shallow: no samples within ${o.shallow_window_m} m of the coast: nothing measured`);
    else {
      if (maxDeg >= o.shallow_max_deg) failures.push(`shallow: the slope reaches ${f1(maxDeg)} degrees ${f1(maxDegAt)} m from the coast (under ${o.shallow_max_deg} wanted)`);
      if (maxStep > o.shallow_max_step_m) failures.push(`shallow: a ${f2(maxStep)} m step between samples ${f1(maxStepAt)} m from the coast (at most ${o.shallow_max_step_m} m)`);
    }
  }

  if (wanted.includes("no_seam")) {
    let maxStep = 0;
    let at = null;
    for (let i = 0; i + 1 < S.length; i++) {
      const st = Math.abs(g[i + 1] - g[i]);
      if (st > maxStep) {
        maxStep = st;
        at = i;
      }
    }
    // The whole rise too: on a coarse mesh a seam is spread over a triangle, so the worst single
    // step understates it (the old box edge read 1.24 m a step inside a 30 m rise, 2026-11-09).
    const lo = Math.min(...g);
    const hi = Math.max(...g);
    const r = { ok: maxStep <= o.seam_max_step_m, max_step_m: maxStep, min_m: lo, max_m: hi };
    if (at !== null) Object.assign(r, { at_lat: S[at].lat, at_lon: S[at].lon, from_m: g[at], to_m: g[at + 1] });
    rules.no_seam = r;
    if (!r.ok) {
      failures.push(
        `no_seam: a ${f2(maxStep)} m step between samples at ${S[at].lat.toFixed(5)}, ${S[at].lon.toFixed(5)} ` +
          `(${f2(g[at])} m to ${f2(g[at + 1])} m; at most ${o.seam_max_step_m} m), in a transect that runs from ${f2(lo)} m to ${f2(hi)} m`,
      );
    }
  }

  return { ok: failures.length === 0, misconfigured: false, failures, notes, rules, coast };
}

/** The one-line summary the sweep logs. */
function summarize(verdict) {
  const parts = Object.entries(verdict.rules || {}).map(([k, v]) => `${k} ${v.ok ? "ok" : "RED"}`);
  return `${verdict.misconfigured ? "MISCONFIGURED" : verdict.ok ? "ok" : "RED"}${parts.length ? ` (${parts.join(", ")})` : ""}`;
}

module.exports = { DEFAULTS, RULES, judgeShore, summarize, distM };
