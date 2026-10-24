// The Maps page's SHIP check (Maps increment 3, "into the ship, deck by deck",
// docs/design/maps-zoom-to-anything.md): with the world loaded and a level of the
// ship shown, did the page show THAT level, its rooms named, the rooms under it
// left out, you on it, and (on the plan) its tiles baked within budget?
//
// It judges the game's own report of what it drew (debug/map_done.json, its
// `ship` block, engine/map_rig_ship.rs `report`):
//   * `level`: the level shown, by the name the picker gives it ("Storey 2"),
//   * `named`: names that must be among the rooms named on it (the HUD's names),
//   * `not_named`: names that must not be (the rooms of another level),
//   * `you_drawn`: your dot drawn (you are on the level shown),
//   * `plan`: the plan is on, with at least `min_tiles` tiles kept and the last
//     bake under `max_bake_ms` by the game's own clock.
// The control vantage (maps-ship-upstairs-ground) shows the ground level and must
// FAIL the upstairs spec, or the check cannot fail.
//
// THE EDIT (`judgeEdit`, the design's proof that "an edit through the editor's
// IPC changes exactly the tiles it touched"): given the tiles kept before an edit,
// the report after it, and the edit's footprint, every tile dropped must lie under
// the footprint and every tile kept before that lies under it must have been
// dropped. A tile's footprint comes from the report (`rect`, ship metres: min x,
// min z, max x, max z); the edit's from the vantage.

function touches(a, b) {
  return a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
}

const keyOf = (t) => `${t.level}/${t.z}/${t.x}/${t.y}`;

/** Judge one report against the spec. */
function judgeShip(report, spec = {}) {
  if (!report || report.ok !== true) return { ok: false, message: "no report from the game (debug/map_done.json)" };
  const s = report.ship || {};
  const fails = [];
  if (!report.page_open) fails.push("the Maps page is not open");
  if (spec.level !== undefined && s.level !== spec.level) fails.push(`the level shown is ${JSON.stringify(s.level)}, want ${JSON.stringify(spec.level)}`);
  const named = s.rooms_named || [];
  for (const n of spec.named || []) if (!named.includes(n)) fails.push(`${n} is not named`);
  for (const n of spec.not_named || []) if (named.includes(n)) fails.push(`${n} is named, and is not on this level`);
  if (spec.you_drawn !== undefined && s.you_drawn !== spec.you_drawn) fails.push(`your dot ${s.you_drawn ? "was" : "was not"} drawn`);
  const t = s.tiles || {};
  if (spec.plan) {
    if (!s.plan) fails.push("the plan is not on");
    const have = (t.have || []).length;
    if (have < (spec.min_tiles ?? 1)) fails.push(`${have} plan tiles kept, want ${spec.min_tiles ?? 1}`);
    if (spec.max_bake_ms !== undefined && !(t.last_bake_ms <= spec.max_bake_ms)) fails.push(`the last tile took ${t.last_bake_ms} ms to bake, over ${spec.max_bake_ms}`);
  }
  const what = `level ${s.level}, ${named.length} rooms named (${named.slice(0, 8).join(", ")}${named.length > 8 ? ", ..." : ""}), ${s.rooms_filled ?? 0} filled, you ${s.you_drawn ? "drawn" : "not drawn"}` +
    (s.plan ? `, plan: ${(t.have || []).length} tiles kept, ${t.baked} baked, last ${typeof t.last_bake_ms === "number" ? t.last_bake_ms.toFixed(2) : "?"} ms` : "");
  return fails.length ? { ok: false, message: `${fails.join("; ")} [${what}]` } : { ok: true, message: what };
}

/** Judge an edit: `before` the tiles kept before it, `after` the report's ship
 *  block after it, `footprint` [min x, min z, max x, max z] in ship metres. */
function judgeEdit(before, after, footprint) {
  const dropped = ((after && after.tiles && after.tiles.dropped) || []);
  const fails = [];
  if (!dropped.length) fails.push("the edit dropped no tile");
  for (const t of dropped) if (!touches(t.rect, footprint)) fails.push(`tile ${keyOf(t)} was dropped and is not under the edit`);
  const droppedKeys = new Set(dropped.map(keyOf));
  const under = (before || []).filter((t) => touches(t.rect, footprint));
  for (const t of under) if (!droppedKeys.has(keyOf(t))) fails.push(`tile ${keyOf(t)} is under the edit and was kept`);
  const what = `${dropped.length} dropped of ${(before || []).length} kept, ${under.length} under the edit`;
  return fails.length ? { ok: false, message: `${fails.join("; ")} [${what}]` } : { ok: true, message: what };
}

/** A control passes only by failing the check. */
function vantageVerdict(check, expect = "pass") {
  if (expect === "fail") {
    return check.ok
      ? { ok: false, message: `the control PASSED the ship check, so the check cannot fail: ${check.message}` }
      : { ok: true, message: `the control failed the check, as it must: ${check.message}` };
  }
  return { ok: check.ok, message: check.message };
}

module.exports = { judgeShip, judgeEdit, vantageVerdict, touches };
