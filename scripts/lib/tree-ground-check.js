// Do the near trees in a capture stand ON the ground drawn under them, and
// does the player? The judgement half of the probe rig's `ground_probe`
// vantage check (BUG-156, 2026-10-05). The game writes the numbers
// (src/engine/tree_ground.rs, the showcase verb {"tree_ground":"1"} ->
// debug/tree_ground.json); this reads them. Pure: no game, no disk, so
// scripts/tests/tree-ground-check.test.js runs it in `just rig-tests`.
//
// WHY A NUMBER AND NOT THE PICTURE. BUG-156 was seen in a capture: trees
// hanging metres above the Silverdale waterfront. A picture says something
// floats; it cannot say by how much, or whether the eye itself stands where
// the ground is drawn. The readout gives each on-screen tree's base against
// the patch actually drawn under it, at that patch's own level of detail, so
// the check measures what the player sees and nothing else.
//
// A vantage opts in with, for example,
//   "ground_probe": { "max_float_m": 0.3, "max_sink_m": 1.0, "min_trees": 5,
//                     "eye_height_m": 1.7, "eye_tol_m": 0.35 }
// Every field is optional; the defaults are below.

const DEFAULTS = Object.freeze({
  // A trunk base this far above the drawn ground shows daylight under it. The
  // unit gate (drawn_surface.rs `tree_bases_sit_on_the_drawn_surface`) holds
  // 0.15 m against a built patch at one depth; a live frame adds the one-level
  // LOD uncertainty (a tree on a patch that splits between the harvest and the
  // readout), so the rig's line is a little wider.
  max_float_m: 0.3,
  // Sunk deeper than this and the trunk reads as a post stuck in the dirt
  // (the harvest sinks a base by a quarter of its root flare on purpose, a
  // few centimetres).
  max_sink_m: 1.0,
  // A check that measured nothing must not pass: a vantage that opts in
  // expects trees in view.
  min_trees: 1,
  // The standing eye: surface_walk::EYE_HEIGHT_M plus the drawn clearance.
  eye_height_m: 1.75,
  eye_tol_m: 0.35,
});

/**
 * Judge one readout. Returns { ok, failures: [string], summary }.
 * `readout` is debug/tree_ground.json as parsed; `spec` the vantage's
 * `ground_probe` object (or `true` for the defaults).
 */
function judgeTreeGround(readout, spec) {
  const s = Object.assign({}, DEFAULTS, spec && typeof spec === "object" ? spec : {});
  const failures = [];
  if (!readout || typeof readout !== "object") {
    return { ok: false, failures: ["no readout"], summary: null };
  }
  if (readout.error) {
    return { ok: false, failures: [`the game could not measure: ${readout.error}`], summary: null };
  }
  const trees = Array.isArray(readout.trees) ? readout.trees : [];
  const measured = trees.filter((t) => t.on_screen && typeof t.gap_drawn_m === "number");
  const floating = measured.filter((t) => t.gap_drawn_m > s.max_float_m);
  const sunk = measured.filter((t) => t.gap_drawn_m < -s.max_sink_m);
  const gaps = measured.map((t) => t.gap_drawn_m).sort((a, b) => a - b);
  const eye = readout.eye || {};
  const summary = {
    trees_on_screen: measured.length,
    gap_min_m: gaps.length ? gaps[0] : null,
    gap_median_m: gaps.length ? gaps[Math.floor((gaps.length - 1) / 2)] : null,
    gap_max_m: gaps.length ? gaps[gaps.length - 1] : null,
    floating: floating.length,
    sunk: sunk.length,
    eye_above_drawn_m: typeof eye.above_drawn_m === "number" ? eye.above_drawn_m : null,
    harvest_depth: readout.harvest ? readout.harvest.depth : null,
  };
  if (measured.length < s.min_trees) {
    failures.push(
      `only ${measured.length} on-screen tree(s) had a drawn ground to measure against (need ${s.min_trees}): ` +
        `the check would pass by measuring nothing`,
    );
  }
  const show = (list) =>
    list
      .slice(0, 3)
      .map((t) => `#${t.i} ${t.gap_drawn_m >= 0 ? "+" : ""}${t.gap_drawn_m.toFixed(2)} m at ${t.dist_m.toFixed(0)} m (leaf depth ${t.leaf_depth})`)
      .join(", ");
  if (floating.length) {
    failures.push(
      `${floating.length} of ${measured.length} on-screen trees float more than ${s.max_float_m} m above the drawn ground: ${show(
        floating.sort((a, b) => b.gap_drawn_m - a.gap_drawn_m),
      )}`,
    );
  }
  if (sunk.length) {
    failures.push(
      `${sunk.length} of ${measured.length} on-screen trees are sunk more than ${s.max_sink_m} m into the drawn ground: ${show(
        sunk.sort((a, b) => a.gap_drawn_m - b.gap_drawn_m),
      )}`,
    );
  }
  if (summary.eye_above_drawn_m === null) {
    failures.push("no drawn ground under the eye to measure the standing height against");
  } else if (Math.abs(summary.eye_above_drawn_m - s.eye_height_m) > s.eye_tol_m) {
    failures.push(
      `the eye is ${summary.eye_above_drawn_m.toFixed(2)} m above the drawn ground, not ${s.eye_height_m} m: ` +
        `the player is not standing on the ground that is drawn`,
    );
  }
  return { ok: failures.length === 0, failures, summary };
}

module.exports = { DEFAULTS, judgeTreeGround };
