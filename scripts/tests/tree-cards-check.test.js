// The rig's tree card gate (scripts/lib/tree-cards-check.js), fed made-up done files.
// Pure node, no game: runs in `just rig-tests`.
//
// Run:  node --test scripts/tests/tree-cards-check.test.js
//
// BUG-301 (2026-11-14): no tree card drew at any forest vantage for five days (and never in a
// release), and verify-runtime passed every one of those captures because nothing looked. The
// cases below are the ways such a gate goes wrong: passing an empty stage, passing when the
// counter is missing altogether (an old exe, or no planet selection), passing cards drawn as flat
// placeholders, and a control that "fails" for the wrong reason.
//
// Seen red 2026-11-14 with the count compared against `cards` instead of `in_window` (every card
// the draw list carries, kept or discarded): "AssertionError: cards carried but all discarded
// passed the gate". The rig's own red, the same day: fuji-forest-ground with the showcase
// `card_floor` "0" (the stage exactly as it was) failed "too few tree cards", 0 in the window,
// deepest drawn patch 13 (the BUG-301 entry in docs/BUGS.md has the run).

const test = require("node:test");
const assert = require("node:assert");
const { judgeTreeCards } = require("../lib/tree-cards-check.js");

const forest = { id: "fuji-forest-ground", min_tree_cards: 500 };
const control = { id: "fuji-forest-ground-no-floor", min_tree_cards: 500, tree_cards_expect: "fail" };

function reading(over = {}) {
  return {
    tree_cards: {
      drawn_patches: 900,
      card_patches: 300,
      cards: 40000,
      in_window: 12000,
      max_drawn_depth: 15,
      hide_m: 92,
      far_m: 1500,
      atlas_ready: true,
      floor: true,
      ...over,
    },
  };
}

test("a vantage without min_tree_cards is not judged", () => {
  const v = judgeTreeCards({ id: "ocean" }, reading());
  assert.strictEqual(v.judged, false);
  assert.strictEqual(v.ok, true);
});

test("a forest that draws its card stage passes", () => {
  const v = judgeTreeCards(forest, reading());
  assert.strictEqual(v.judged, true);
  assert.strictEqual(v.ok, true, v.message);
});

test("BUG-301's reading, base-only ground with no card patch, fails", () => {
  const v = judgeTreeCards(forest, reading({ card_patches: 0, cards: 0, in_window: 0, max_drawn_depth: 13, floor: false }));
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /too few tree cards/);
  assert.match(v.message, /deepest 13/);
});

test("cards carried but all discarded passed the gate", () => {
  // Every card patch lies past the far cutoff or inside the hide radius: the draw list carries
  // cards and the screen shows none.
  const v = judgeTreeCards(forest, reading({ in_window: 0 }));
  assert.strictEqual(v.ok, false, "cards carried but all discarded passed the gate");
});

test("a missing counter fails rather than passing as no news", () => {
  for (const shot of [{}, { tree_cards: null }, null]) {
    const v = judgeTreeCards(forest, shot);
    assert.strictEqual(v.ok, false);
    assert.match(v.message, /no tree_cards reading/);
  }
});

test("cards drawn as flat placeholders (the sprite atlas not baked) fail", () => {
  const v = judgeTreeCards(forest, reading({ atlas_ready: false }));
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /atlas is not baked/);
});

test("the control passes only by failing the count with a reading present", () => {
  assert.strictEqual(judgeTreeCards(control, reading({ in_window: 0, floor: false })).ok, true);
  const drew = judgeTreeCards(control, reading());
  assert.strictEqual(drew.ok, false, "a control that drew its stage controls nothing");
  assert.match(drew.message, /CONTROL drew/);
  assert.strictEqual(judgeTreeCards(control, {}).ok, false, "a control with no reading proves nothing");
});
