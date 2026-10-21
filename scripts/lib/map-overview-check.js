// The Maps page's OVERVIEW check (Maps increment 2, "a solar system that looks
// real", docs/design/maps-zoom-to-anything.md): with the world loaded and the
// camera on the solar-system overview, did the page draw what makes it look
// real, and within its budget?
//
// It judges the game's own report of what it drew (debug/map_done.json,
// engine/map_rig.rs `report_json`) and the frame costs the rig captured
// (debug/frame_costs.json under HUMANITY_FRAME_COSTS=1):
//   * the world layer drew the real star sky (want TooFar or Diagram, a
//     sky-only render, at least one render),
//   * the belts' dust: at least `min_belt_points` points drawn,
//   * the orbits, the bodies and the names: at least `min_orbits`,
//     `min_bodies`, `min_labels`,
//   * the band is the solar system,
//   * the symbols cost under `max_symbology_ms`: the page's own measure in
//     the report AND, when the costs file is there, `cpu.maps_symbology`.
// The control vantage (maps-system-overview-nobelts) switches the belts off
// with {"map_layer":"belts=0"} and must FAIL here, or the check cannot fail.

/** Judge one report (and costs, when there are any) against the spec. */
function judgeOverview(report, spec = {}, costs = null) {
  const want = {
    min_belt_points: 10000,
    min_orbits: 8,
    min_bodies: 12,
    min_labels: 6,
    max_symbology_ms: 1.0,
    ...spec,
  };
  if (!report || report.ok !== true) return { ok: false, message: "no report from the game (debug/map_done.json)" };
  const s = report.symbology || {};
  const w = report.world || {};
  const fails = [];
  if (!report.page_open) fails.push("the Maps page is not open");
  if (s.band !== "SolarSystem") fails.push(`the camera is in the ${s.band || "?"} band, not the solar system`);
  if (!(w.renders > 0) || w.sky_only !== true) fails.push(`the world layer drew no star sky (renders ${w.renders}, sky_only ${w.sky_only}, want ${w.want})`);
  if (!(s.belt_points >= want.min_belt_points)) fails.push(`${s.belt_points ?? 0} belt points, want ${want.min_belt_points}`);
  if (!(s.orbits >= want.min_orbits)) fails.push(`${s.orbits ?? 0} orbits, want ${want.min_orbits}`);
  if (!(s.bodies >= want.min_bodies)) fails.push(`${s.bodies ?? 0} bodies, want ${want.min_bodies}`);
  if (!(s.labels_placed >= want.min_labels)) fails.push(`${s.labels_placed ?? 0} names, want ${want.min_labels}`);
  if (!(typeof s.ms === "number" && s.ms < want.max_symbology_ms)) fails.push(`the symbols took ${s.ms} ms by the page's own clock, over ${want.max_symbology_ms}`);
  const cost = costs && costs.cpu_ms ? costs.cpu_ms["cpu.maps_symbology"] : undefined;
  if (costs && !(typeof cost === "number" && cost < want.max_symbology_ms)) {
    fails.push(`cpu.maps_symbology ${cost === undefined ? "was not recorded" : `${cost.toFixed(3)} ms`}, over ${want.max_symbology_ms}`);
  }
  const what = `${s.belt_points} belt points, ${s.orbits} orbits, ${s.bodies} bodies, ${s.labels_placed} names (${s.labels_dropped} dropped), symbols ${typeof s.ms === "number" ? s.ms.toFixed(3) : "?"} ms` +
    (cost !== undefined ? `, cpu.maps_symbology ${cost.toFixed(3)} ms` : costs ? "" : ", no costs file");
  return fails.length ? { ok: false, message: `${fails.join("; ")} [${what}]` } : { ok: true, message: what };
}

/** A control passes only by failing the check. */
function vantageVerdict(check, expect = "pass") {
  if (expect === "fail") {
    return check.ok
      ? { ok: false, message: `the control PASSED the overview check, so the check cannot fail: ${check.message}` }
      : { ok: true, message: `the control failed the check, as it must: ${check.message}` };
  }
  return { ok: check.ok, message: check.message };
}

module.exports = { judgeOverview, vantageVerdict };
