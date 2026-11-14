// The rig's tree card gate (BUG-301, 2026-11-14): a forest vantage must draw its card stage.
//
// The tree cards (the patch-baked sprite cards that carry a forest past the 3D models) need
// depth-15 terrain patches, and a game without the 460 m tile tier drew its ground to depth 13,
// so no release ever drew a card, and once BUG-287 (2026-11-09) stopped an empty tile folder
// counting as the tier, no dev boot did either. Every capture of fuji-forest-ground from then on
// showed a bare slope behind the near models, verify-runtime passed it, and nobody noticed for
// five days: no check looked at whether the stage drew anything.
//
// The game writes its card counter into every screenshot's done file
// (engine/tree_card_stage.rs `done_json`): `tree_cards` = {drawn_patches, card_patches, cards,
// in_window, max_drawn_depth, hide_m, far_m, atlas_ready, floor}, or null when no planet
// selection ran in the last two seconds. A vantage with `min_tree_cards: N` fails unless at least
// N cards sit in the window the fragment stage keeps and the card sprites are baked. A vantage
// that also says `tree_cards_expect: "fail"` is a CONTROL (the showcase `card_floor` "0" twin):
// it passes only by failing the count with a reading present, which proves the gate can fail.
"use strict";

/**
 * Judge one capture. `vantage` is the vantages.json entry, `shot` the parsed done file.
 * Returns {judged, ok, message, reading}.
 */
function judgeTreeCards(vantage, shot) {
  const min = vantage && vantage.min_tree_cards;
  if (min === undefined || min === null) {
    return { judged: false, ok: true, message: "no min_tree_cards on this vantage", reading: null };
  }
  const control = vantage.tree_cards_expect === "fail";
  const tc = shot ? shot.tree_cards : undefined;
  if (!tc || typeof tc.in_window !== "number") {
    return {
      judged: true,
      ok: false,
      message:
        "the done file carries no tree_cards reading (an exe older than the BUG-301 counter, " +
        "or no planet selection ran in the two seconds before the capture)",
      reading: tc || null,
    };
  }
  const detail =
    `${tc.in_window} cards in the kept window of ${tc.cards} carried by ${tc.card_patches} ` +
    `of ${tc.drawn_patches} drawn patches (deepest ${tc.max_drawn_depth}; hide ${Math.round(tc.hide_m)} m, ` +
    `far ${Math.round(tc.far_m)} m; atlas ${tc.atlas_ready ? "baked" : "NOT baked"}; floor ${tc.floor ? "on" : "off"})`;
  const short = tc.in_window < min;
  const flat = !tc.atlas_ready;
  const drew = !short && !flat;
  if (control) {
    return drew
      ? { judged: true, ok: false, message: `CONTROL drew its card stage, so it controls nothing: ${detail}`, reading: tc }
      : { judged: true, ok: true, message: `control failed the count as it must: ${detail}`, reading: tc };
  }
  if (short) {
    return { judged: true, ok: false, message: `too few tree cards (min_tree_cards ${min}): ${detail}`, reading: tc };
  }
  if (flat) {
    return { judged: true, ok: false, message: `the cards draw as flat placeholders, the sprite atlas is not baked: ${detail}`, reading: tc };
  }
  return { judged: true, ok: true, message: `the card stage drew (min ${min}): ${detail}`, reading: tc };
}

module.exports = { judgeTreeCards };
