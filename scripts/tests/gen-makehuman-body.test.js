// The MakeHuman bodies (character animation rung 1, 2026-11-09): the committed glTF files must be
// exactly what scripts/gen-makehuman-body.js makes from MakeHuman's CC0 data, so the body every
// person is drawn with can always be rebuilt from its source and nobody edits it by hand. Where the
// sources are not on the machine (CI), the check compares the files with the SHA-256 hashes the
// converter recorded in SOURCE.md and says so.
//
// Seen red 2026-11-09 with the hair shell's offset changed from 6 to 7 mm and the files not
// rebuilt: "assets/models/characters/makehuman/makehuman_female.bin differs from what
// scripts/gen-makehuman-body.js makes"; and, without the sources, with one byte of
// makehuman_male.bin changed: "makehuman_male.bin: ..., SOURCE.md records 9950...".

"use strict";
const test = require("node:test");
const assert = require("node:assert");
const path = require("path");
const { spawnSync } = require("child_process");

const ROOT = path.join(__dirname, "..", "..");
const SCRIPT = path.join(ROOT, "scripts", "gen-makehuman-body.js");

test("the committed MakeHuman bodies match the converter", () => {
  const r = spawnSync(process.execPath, [SCRIPT, "--check"], { encoding: "utf8" });
  assert.strictEqual(r.status, 0, `${r.stdout}\n${r.stderr}`);
});

test("without the sources the committed bodies still match the hashes SOURCE.md records", () => {
  const r = spawnSync(process.execPath, [SCRIPT, "--check", "--src", path.join(ROOT, "no-such-folder")], { encoding: "utf8" });
  assert.strictEqual(r.status, 0, `${r.stdout}\n${r.stderr}`);
  // The two bodies' glTF and .bin, their base files and the shared base file (the look, 2026-11-09).
  assert.match(r.stdout, /checked the 7 committed files against SOURCE\.md's hashes only/);
});

test("weights are four unorm16 shares summing to exactly 65535", () => {
  const { unorm16 } = require(SCRIPT);
  for (const top of [[[3, 0.1], [7, 0.2], [9, 0.7]], [[1, 1 / 3], [2, 1 / 3], [4, 1 / 3]], [[5, 0.25], [6, 0.25], [7, 0.25], [8, 0.25]], [[2, 0.9]]]) {
    const s = unorm16(top);
    assert.strictEqual(s.weights.reduce((a, b) => a + b, 0), 65535, JSON.stringify(top));
    assert.strictEqual(s.joints.length, 4);
  }
});
