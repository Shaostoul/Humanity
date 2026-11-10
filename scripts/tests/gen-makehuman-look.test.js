// The MakeHuman look (character animation, the look, 2026-11-09; docs/design/character-animation.md
// section 16): the committed detail targets, detail sliders, garments and their textures must be
// exactly what scripts/gen-makehuman-look.js makes from MakeHuman's CC0 data (the detail targets,
// the slider list, the system asset pack), so they can always be rebuilt from their sources and
// nobody edits them by hand. Where the sources are not on the machine (CI), the check compares the
// files with the SHA-256 hashes the converter recorded in LOOK-SOURCE.md and says so.
//
// Seen red 2026-11-09 with one garment's roughness edited by hand in data/characters/garments.ron:
// "data\characters\garments.ron differs from what scripts/gen-makehuman-look.js makes"; and, without
// the asset pack, the same edit: "data/characters/garments.ron: b39590f7..., LOOK-SOURCE.md records
// 46788b53...".

"use strict";
const test = require("node:test");
const assert = require("node:assert");
const path = require("path");
const { spawnSync } = require("child_process");

const ROOT = path.join(__dirname, "..", "..");
const SCRIPT = path.join(ROOT, "scripts", "gen-makehuman-look.js");

test("the committed MakeHuman look matches the converter", () => {
  const r = spawnSync(process.execPath, [SCRIPT, "--check"], { encoding: "utf8" });
  assert.strictEqual(r.status, 0, `${r.stdout}\n${r.stderr}`);
});

test("without the sources the committed look still matches the hashes LOOK-SOURCE.md records", () => {
  const nowhere = path.join(ROOT, "no-such-folder");
  const env = { ...process.env, HUMANITY_MAKEHUMAN_DETAILS: nowhere, HUMANITY_MAKEHUMAN_PACK: nowhere };
  const r = spawnSync(process.execPath, [SCRIPT, "--check"], { encoding: "utf8", env });
  assert.strictEqual(r.status, 0, `${r.stdout}\n${r.stderr}`);
  assert.match(r.stdout, /checked the \d+ committed files against LOOK-SOURCE\.md's hashes only/);
});

test("an .mhclo reads its bindings, keywords anywhere among them, and its hidden runs", () => {
  const { parseMhclo, sliderId } = require(SCRIPT);
  const clo = parseMhclo(["name x", "verts 0", "material x.mhmat", "1 2 3 0.2 0.3 0.5 0.01 0.02 0.03", "7", "delete_verts", "10 - 12 15"].join("\n"));
  assert.strictEqual(clo.verts.length, 2, "the keyword between the bindings is not a binding");
  assert.deepStrictEqual(clo.verts[0].v, [1, 2, 3]);
  assert.deepStrictEqual(clo.verts[1].w, [1, 0, 0], "a single index is the vertex itself");
  assert.deepStrictEqual(clo.hidden, [10, 11, 12, 15]);
  assert.strictEqual(sliderId("nose-trans", "up"), "nose_trans_up");
  assert.throws(() => sliderId("a-very-long-target-name-indeed-and-more", null));
});
