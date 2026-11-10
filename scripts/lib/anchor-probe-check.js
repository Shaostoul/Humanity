// Is the drawn body on what it touches? The judgement half of the probe rig's `anchor_probe` vantage
// check (character animation rung 2b, 2026-11-10; docs/design/character-animation.md section 17).
// The game writes the numbers (src/engine/anchor_probe.rs, the showcase verb {"anchor_probe":"1.5"}
// -> debug/anchor_probe.json); this reads them. Pure: no game, no disk, so
// scripts/tests/anchor-probe-check.test.js runs it in `just rig-tests`.
//
// WHY. A picture shows a person on a chair; this says how far their DRAWN hips are from THIS seat,
// their drawn hands from the rungs, from the mesh the GPU draws (the game skins the touching
// vertices on the CPU with the very palette it hands the GPU), never from the IK's own targets.
//
// A vantage opts in with:
//   "anchor_probe": { "secs": 1.5, "rules": ["hips_on_seat"], "who": ["crowd:0"] }
//
// THE RULES (each over every frame the probe watched while the person held the anchor):
//   hips_on_seat      the lowest drawn hip or thigh vertex over the seat's contact: no more than
//                     `seat_sink_m` into it and no more than `seat_float_m` over it.
//   hands_on_rungs    each climbing hand, while it grips a rung, within `rung_gap_m` of the rung's
//                     surface (and no more than `rung_sink_m` inside it).
//   hands_on_surface  each hand's lowest drawn vertex within `surface_sink_m` under to
//                     `surface_float_m` over the work surface.
//   hand_on_handle    the hand's nearest drawn vertex within `handle_m` of the door's handle.
//   head_hidden       the player's own body drawn in first person (`body_below_m` at least
//                     `body_below_min_m` under the eye) with nothing of it within `eye_clear_m` of
//                     the eye (the camera's near plane).
//
// MISCONFIGURED, so a vantage that proves nothing can never pass: no person judged, a judged person
// with fewer than `min_frames` frames of a measure the rule reads, or `who` naming someone the
// readout does not have.

const DEFAULTS = Object.freeze({
  seat_sink_m: 0.02,
  seat_float_m: 0.025,
  rung_gap_m: 0.02,
  rung_sink_m: 0.03,
  surface_sink_m: 0.015,
  surface_float_m: 0.025,
  handle_m: 0.04,
  eye_clear_m: 0.1,
  body_below_min_m: 1.0,
  min_frames: 8,
});

const RULES = Object.freeze(["hips_on_seat", "hands_on_rungs", "hands_on_surface", "hand_on_handle", "head_hidden"]);
// The measures each rule reads (`any`: at least one of them must have frames; each present is judged).
const READS = Object.freeze({
  hips_on_seat: { names: ["hips_m"], any: false },
  hands_on_rungs: { names: ["left_hand_m", "right_hand_m"], any: true },
  hands_on_surface: { names: ["left_hand_m", "right_hand_m"], any: false },
  hand_on_handle: { names: ["handle_m"], any: false },
  head_hidden: { names: ["eye_nearest_m", "body_below_m"], any: false },
});
const f3 = (x) => (typeof x === "number" ? x.toFixed(3) : String(x));

/**
 * Judge one readout. Returns { ok, misconfigured, failures: [string], notes: [string], rules:
 * {name: {ok, worst_m}} }. `readout` is debug/anchor_probe.json as parsed; `spec` the vantage's
 * `anchor_probe`; `opts` overrides DEFAULTS (tests).
 */
function judgeAnchors(readout, spec, opts = {}) {
  const o = { ...DEFAULTS, ...opts };
  const out = { ok: true, misconfigured: false, failures: [], notes: [], rules: {} };
  const bad = (msg, misconf = true) => {
    out.ok = false;
    if (misconf) out.misconfigured = true;
    out.failures.push(misconf ? `MISCONFIGURED: ${msg}` : msg);
    return out;
  };
  if (!spec || typeof spec !== "object") return bad("the vantage has no anchor_probe spec");
  if (!readout || typeof readout !== "object" || !Array.isArray(readout.people)) return bad("no readout");
  const rules = Array.isArray(spec.rules) ? spec.rules : [];
  if (!rules.length || rules.some((r) => !RULES.includes(r))) return bad(`rules must be some of ${RULES.join(", ")}, not ${JSON.stringify(spec.rules)}`);
  let people = readout.people;
  if (Array.isArray(spec.who) && spec.who.length) {
    const missing = spec.who.filter((w) => !people.some((p) => p.who === w));
    if (missing.length) return bad(`the readout has no ${missing.join(", ")} (it has ${people.map((p) => p.who).join(", ") || "nobody"})`);
    people = people.filter((p) => spec.who.includes(p.who));
  }
  if (!people.length) return bad("no person in an interaction was measured");
  const failures = [];
  for (const rule of rules) {
    const reads = READS[rule];
    let worst = null;
    let judged = 0;
    for (const p of people) {
      const m = p.measures || {};
      const present = reads.names.filter((n) => m[n] && m[n].frames >= o.min_frames);
      if (reads.any ? present.length === 0 : present.length < reads.names.length) {
        const have = reads.names.map((n) => `${n} ${m[n] ? m[n].frames : 0}`).join(", ");
        return bad(`${p.who} (${p.anchor}): ${rule} needs at least ${o.min_frames} frames of ${reads.names.join(reads.any ? " or " : " and ")} (has ${have})`);
      }
      judged++;
      for (const n of present) {
        const x = m[n];
        out.notes.push(`${p.who} (${p.body}, ${p.anchor}) ${n}: ${x.frames} frames, lowest ${f3(x.lowest_m)} m, highest ${f3(x.highest_m)} m, mean |${f3(x.mean_abs_m)}| m`);
        const fail = (what) => failures.push(`${rule}: ${p.who}'s ${n} ${what}`);
        if (rule === "hips_on_seat") {
          if (x.lowest_m < -o.seat_sink_m) fail(`reaches ${f3(-x.lowest_m)} m into the seat (at most ${o.seat_sink_m})`);
          if (x.highest_m > o.seat_float_m) fail(`stands ${f3(x.highest_m)} m over the seat (at most ${o.seat_float_m})`);
          worst = Math.max(worst ?? 0, Math.max(-x.lowest_m, x.highest_m));
        } else if (rule === "hands_on_rungs") {
          if (x.highest_m > o.rung_gap_m) fail(`is ${f3(x.highest_m)} m off its rung (at most ${o.rung_gap_m})`);
          if (x.lowest_m < -o.rung_sink_m) fail(`reaches ${f3(-x.lowest_m)} m into its rung (at most ${o.rung_sink_m})`);
          worst = Math.max(worst ?? 0, x.highest_m);
        } else if (rule === "hands_on_surface") {
          if (x.lowest_m < -o.surface_sink_m) fail(`reaches ${f3(-x.lowest_m)} m into the surface (at most ${o.surface_sink_m})`);
          if (x.highest_m > o.surface_float_m) fail(`hovers ${f3(x.highest_m)} m over the surface (at most ${o.surface_float_m})`);
          worst = Math.max(worst ?? 0, Math.max(-x.lowest_m, x.highest_m));
        } else if (rule === "hand_on_handle") {
          if (x.highest_m > o.handle_m) fail(`is ${f3(x.highest_m)} m from the handle (at most ${o.handle_m})`);
          worst = Math.max(worst ?? 0, x.highest_m);
        } else if (rule === "head_hidden") {
          if (n === "eye_nearest_m" && x.lowest_m < o.eye_clear_m) fail(`is ${f3(x.lowest_m)} m: the body is drawn inside the camera's near plane (at least ${o.eye_clear_m})`);
          if (n === "body_below_m" && x.highest_m < o.body_below_min_m) fail(`is ${f3(x.highest_m)} m: no body drawn under the eye (at least ${o.body_below_min_m})`);
          if (n === "eye_nearest_m") worst = x.lowest_m;
        }
      }
    }
    const ok = !failures.some((f) => f.startsWith(`${rule}:`));
    out.rules[rule] = { ok, people: judged, worst_m: worst };
  }
  for (const f of failures) bad(f, false);
  return out;
}

/** One line for the log. */
function summarize(v) {
  if (v.misconfigured) return `MISCONFIGURED (${v.failures.join("; ")})`;
  const parts = Object.entries(v.rules).map(([k, r]) => `${k} ${r.ok ? "ok" : "FAILED"} (worst ${f3(r.worst_m)} m, ${r.people} people)`);
  return `${v.ok ? "ok" : "FAILED"}: ${parts.join(", ")}`;
}

module.exports = { judgeAnchors, summarize, DEFAULTS, RULES };
