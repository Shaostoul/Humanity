// The sticky showcase pins the rigs release between vantages
// (scripts/lib/showcase-pins.js). Pure node, no game: runs in `just rig-tests`.
//
// Review finding 9 (2026-10-04): the pipe marking pin ({"pipe_marking":"full"})
// stays until "auto", and no rig released it, so in a full sweep the vantages
// after home-pipes-full ran in Full mode instead of the player default.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const { STICKY_PIN_RESETS, DIAG_RESETS, holdsStickyPin } = require("../lib/showcase-pins.js");

test("every sticky pin the sweep released before is still released", () => {
  for (const [k, v] of Object.entries({ wind: "auto", anim_clock: "auto", aurora: "1", room_gi: "1", present_dither: "1", sun_shadows: "auto", near_levels: "auto", fov: "auto" })) {
    assert.strictEqual(STICKY_PIN_RESETS[k], v, `${k} resets to ${v}`);
  }
  assert.strictEqual(DIAG_RESETS.map_diag, "0");
});

test("the pipe marking pin is released, and pinning it marks the pins as held", () => {
  assert.strictEqual(STICKY_PIN_RESETS.pipe_marking, "auto", "pipe_marking resets to auto (hands the mode back to Settings)");
  assert.strictEqual(holdsStickyPin({ pipe_marking: "full" }), true, "a vantage pinning Full holds a sticky pin");
  assert.strictEqual(holdsStickyPin({ pipe_marking: "auto" }), false, "auto is the release, not a pin");
  assert.strictEqual(holdsStickyPin({ time: "12" }), false, "a non-sticky key holds nothing");
  assert.strictEqual(holdsStickyPin(undefined), false);
});

test("both rigs take the shared list rather than a copy of their own", () => {
  for (const rig of ["probe-sweep.js", "probe-hot-ab.js"]) {
    const src = fs.readFileSync(path.join(__dirname, "..", rig), "utf8");
    assert.ok(src.includes("STICKY_PIN_RESETS"), `${rig} uses the shared STICKY_PIN_RESETS`);
    assert.ok(!/wind:\s*"auto",\s*anim_clock:\s*"auto"/.test(src), `${rig} keeps no copy of the list`);
  }
});
