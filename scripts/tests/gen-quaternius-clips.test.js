// The Quaternius clips (character animation rung 1, 2026-11-09): the committed glTF and
// Quaternius's License.txt must be exactly what scripts/gen-quaternius-clips.js makes from the CC0
// download, so the clips can always be rebuilt from the file their licence came with. Without the
// download (CI), the check compares the files with the hashes SOURCE.md records and says so.
//
// Seen red 2026-11-09 with Jump_Land taken off the converter's clip list and the files not rebuilt:
// "assets/models/characters/quaternius_ual/quaternius_ual.gltf differs from what
// scripts/gen-quaternius-clips.js makes"; and, without the download, with License.txt's last byte
// changed: "License.txt: ..., SOURCE.md records 6d01...".

"use strict";
const test = require("node:test");
const assert = require("node:assert");
const path = require("path");
const { spawnSync } = require("child_process");

const ROOT = path.join(__dirname, "..", "..");
const SCRIPT = path.join(ROOT, "scripts", "gen-quaternius-clips.js");

test("the committed Quaternius clips match the converter", () => {
  const r = spawnSync(process.execPath, [SCRIPT, "--check"], { encoding: "utf8" });
  assert.strictEqual(r.status, 0, `${r.stdout}\n${r.stderr}`);
});

test("without the download the committed clips still match the hashes SOURCE.md records", () => {
  const r = spawnSync(process.execPath, [SCRIPT, "--check", "--src", path.join(ROOT, "no-such-folder")], { encoding: "utf8" });
  assert.strictEqual(r.status, 0, `${r.stdout}\n${r.stderr}`);
  assert.match(r.stdout, /checked the 3 committed files against SOURCE\.md's hashes only/);
});
