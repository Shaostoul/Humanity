// scripts/lib/startup-times.js: the start's two times (the pages' first frame, the world ready)
// read apart from a run.log, as boot-timing.js reports them since the desktop's pages-first start
// (2026-11-20). Seen red: with the ready line's pattern changed to look for "world is ready in", the
// first test failed (world_ready_ms null against 11872), so it reads the engine's real line.

"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const path = require("node:path");
const { parseStartup } = require(path.join(__dirname, "..", "lib", "startup-times.js"));

const log = [
  "[Boot] first frame drawn 1043 ms after the window opened (pages-only start: yes)",
  "[WorldPrep] Play pressed before the world was ready: finishing it now, the page shows how far along it is",
  "[WorldPrep] the 3D world is ready: 11872 ms after the window opened, 10790 ms after its preparation began",
  "[WorldPrep] opening the world 10650 ms after Play was pressed",
  "3D world loaded in 2140ms",
].join("\n");

test("the pages' first frame and the world's readiness are read apart", () => {
  assert.deepStrictEqual(parseStartup(log), {
    first_page_frame_ms: 1043,
    pages_only: true,
    world_ready_ms: 11872,
    world_prep_ms: 10790,
    play_waited_ms: 10650,
    world_load_ms: 2140,
  });
});

test("an old-order start has its first frame only, and nothing is guessed", () => {
  const r = parseStartup("[Boot] first frame drawn 18588 ms after the window opened (pages-only start: no)");
  assert.strictEqual(r.first_page_frame_ms, 18588);
  assert.strictEqual(r.pages_only, false);
  assert.strictEqual(r.world_ready_ms, null);
  assert.strictEqual(r.play_waited_ms, null);
  assert.deepStrictEqual(parseStartup(""), { first_page_frame_ms: null, pages_only: null, world_ready_ms: null, world_prep_ms: null, play_waited_ms: null, world_load_ms: null });
});
