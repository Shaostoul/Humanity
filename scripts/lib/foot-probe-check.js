// Do the drawn feet stand on the ground? The judgement half of the probe rig's `foot_probe` vantage
// check (character animation rung 2a, 2026-11-09; docs/design/character-animation.md section 4).
// The game writes the numbers (src/engine/foot_probe.rs, the showcase verb {"foot_probe":"1.5"} ->
// debug/foot_probe.json); this reads them. Pure: no game, no disk, so
// scripts/tests/foot-probe-check.test.js runs it in `just rig-tests`.
//
// WHY. Rung 1 drew every clip as it came: the feet sank up to about 5 cm in the walk cycle and hovered
// over or sank through stair treads. A picture shows a foot in a tread; this says by how much, from
// the DRAWN mesh (the game skins each near body's foot vertices on the CPU with the very palette it
// hands the GPU and measures each against the ground under it), never from the IK's own targets.
//
// A vantage opts in with:
//   "foot_probe": { "secs": 1.5, "rules": ["feet_on_ground"], "min_spread_m": 0.1, "who": ["crowd:0"] }
//
// THE RULE:
//   feet_on_ground   every planted foot (a foot its clip says carries weight) of every person judged
//                    has its lowest drawn vertex no more than `sink_m` under the ground under it and
//                    no more than `float_m` over it, over every frame the probe watched.
//
// MISCONFIGURED, so a vantage that proves nothing can never pass: no person judged, a judged person
// with fewer than `min_planted` planted frames, a ground that does not vary under the feet by at
// least the vantage's `min_spread_m` (a stair or a slope vantage on flat floor), or `who` naming
// someone the readout does not have.

const DEFAULTS = Object.freeze({
  sink_m: 0.015,
  float_m: 0.025,
  min_planted: 10,
});

const RULES = Object.freeze(["feet_on_ground"]);
const f3 = (x) => (typeof x === "number" ? x.toFixed(3) : String(x));

/**
 * Judge one readout. Returns { ok, misconfigured, failures: [string], notes: [string], rules:
 * {name: {ok, ...numbers}} }. `readout` is debug/foot_probe.json as parsed; `spec` the vantage's
 * `foot_probe`; `opts` overrides DEFAULTS (tests).
 */
function judgeFeet(readout, spec, opts = {}) {
  const o = { ...DEFAULTS, ...opts };
  const out = { ok: true, misconfigured: false, failures: [], notes: [], rules: {} };
  const bad = (msg, misconf = true) => {
    out.ok = false;
    if (misconf) out.misconfigured = true;
    out.failures.push(misconf ? `MISCONFIGURED: ${msg}` : msg);
    return out;
  };
  if (!spec || typeof spec !== "object") return bad("the vantage has no foot_probe spec");
  if (!readout || typeof readout !== "object" || !Array.isArray(readout.people)) return bad("no readout");
  const rules = Array.isArray(spec.rules) ? spec.rules : [];
  if (!rules.length || rules.some((r) => !RULES.includes(r))) return bad(`rules must be some of ${RULES.join(", ")}, not ${JSON.stringify(spec.rules)}`);
  let people = readout.people;
  if (Array.isArray(spec.who) && spec.who.length) {
    const missing = spec.who.filter((w) => !people.some((p) => p.who === w));
    if (missing.length) return bad(`the readout has no ${missing.join(", ")} (it has ${people.map((p) => p.who).join(", ") || "nobody"})`);
    people = people.filter((p) => spec.who.includes(p.who));
  }
  if (!people.length) return bad("no near person was measured");
  const minSpread = typeof spec.min_spread_m === "number" ? spec.min_spread_m : 0;
  let worstSink = 0;
  let worstFloat = 0;
  const failures = [];
  for (const p of people) {
    const planted = (p.left?.planted_frames || 0) + (p.right?.planted_frames || 0);
    if (planted < o.min_planted) return bad(`${p.who}: only ${planted} planted foot-frames (at least ${o.min_planted})`);
    if (p.ground_spread_m < minSpread) return bad(`${p.who}: the ground under the feet varies only ${f3(p.ground_spread_m)} m (at least ${minSpread}): not the uneven ground this vantage is for`);
    for (const side of ["left", "right"]) {
      const f = p[side];
      if (!f || !f.planted_frames) continue;
      const sink = Math.max(0, -f.lowest_m);
      const float = Math.max(0, f.highest_m);
      worstSink = Math.max(worstSink, sink);
      worstFloat = Math.max(worstFloat, float);
      out.notes.push(`${p.who} (${p.body}, ${p.mode}) ${side}: planted ${f.planted_frames} frames, lowest ${f3(f.lowest_m)} m, highest ${f3(f.highest_m)} m, mean |${f3(f.mean_abs_m)}| m`);
      if (sink > o.sink_m) failures.push(`feet_on_ground: ${p.who}'s ${side} foot reaches ${f3(sink)} m into the ground while planted (at most ${o.sink_m})`);
      if (float > o.float_m) failures.push(`feet_on_ground: ${p.who}'s ${side} foot hovers ${f3(float)} m over the ground while planted (at most ${o.float_m})`);
    }
  }
  const ok = failures.length === 0;
  out.rules.feet_on_ground = { ok, people: people.length, worst_sink_m: worstSink, worst_float_m: worstFloat, frames: readout.frames };
  if (rules.includes("feet_on_ground") && !ok) for (const f of failures) bad(f, false);
  return out;
}

/** One line for the log. */
function summarize(v) {
  if (v.misconfigured) return `MISCONFIGURED (${v.failures.join("; ")})`;
  const r = v.rules.feet_on_ground;
  return `${v.ok ? "ok" : "FAILED"}: feet_on_ground ${r && r.ok ? "ok" : "FAILED"} (worst ${f3(r?.worst_sink_m)} m in, ${f3(r?.worst_float_m)} m over, ${r?.people} people)`;
}

module.exports = { judgeFeet, summarize, DEFAULTS, RULES };
