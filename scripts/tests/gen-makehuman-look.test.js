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

// A hand-edited BC7 texture is caught (the look's textures, 2026-11-10): the converter's outputs
// linked into a scratch folder (hard links, so nothing large is copied), one garment's DDS replaced
// by a copy with one byte of its level 0 changed, and --check run there without the sources: it must
// fail naming that file, and pass on the untouched copy (so the failure is the edit's, not the
// harness's). With the sources on the machine the full --check re-encodes and compares bytes, and the
// same edit fails it: run by hand 2026-11-10, "assets\models\characters\makehuman\garments\shoes01\
// shoes01.dds differs from what scripts/gen-makehuman-look.js makes".
// Seen red 2026-11-10 with the no-sources check made to skip the .dds files (as a check that never
// read them would): "a hand-edited BC7 file passes --check".
test("a hand-edited BC7 garment texture fails the check", () => {
  const fs = require("fs");
  const os = require("os");
  const note = path.join(ROOT, "assets", "models", "characters", "makehuman", "LOOK-SOURCE.md");
  const rels = [...fs.readFileSync(note, "utf8").replace(/\r\n/g, "\n").matchAll(/^- `([^`]+)` `[0-9a-f]{64}`$/gm)].map((m) => m[1]);
  const dds = rels.filter((r) => r.endsWith(".dds"));
  assert.ok(dds.length >= 40, `LOOK-SOURCE.md lists the BC7 textures (found ${dds.length})`);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "look-check-"));
  try {
    const place = (rel) => {
      const to = path.join(tmp, rel);
      fs.mkdirSync(path.dirname(to), { recursive: true });
      try { fs.linkSync(path.join(ROOT, rel), to); } catch { fs.copyFileSync(path.join(ROOT, rel), to); }
    };
    for (const rel of [...rels, "assets/models/characters/makehuman/LOOK-SOURCE.md"]) place(rel);
    const nowhere = path.join(ROOT, "no-such-folder");
    const env = { ...process.env, HUMANITY_MAKEHUMAN_DETAILS: nowhere, HUMANITY_MAKEHUMAN_PACK: nowhere, HUMANITY_LOOK_ROOT: tmp };
    const clean = spawnSync(process.execPath, [SCRIPT, "--check"], { encoding: "utf8", env });
    assert.strictEqual(clean.status, 0, `the untouched copy passes: ${clean.stdout}\n${clean.stderr}`);
    const target = dds.find((r) => r.endsWith("/shoes01.dds")) || dds[0];
    const at = path.join(tmp, target);
    const bytes = fs.readFileSync(at);
    fs.unlinkSync(at); // the link, not the repo's file
    bytes[148 + 1000] ^= 0x01; // one bit of one block of level 0
    fs.writeFileSync(at, bytes);
    const edited = spawnSync(process.execPath, [SCRIPT, "--check"], { encoding: "utf8", env });
    assert.notStrictEqual(edited.status, 0, "a hand-edited BC7 file passes --check");
    assert.match(edited.stderr, new RegExp(target.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&")), `the failure names the file: ${edited.stderr}`);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
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
