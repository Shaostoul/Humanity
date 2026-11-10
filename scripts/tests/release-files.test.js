// WHAT THE DOWNLOAD CARRIES BESIDE THE PROGRAM (scripts/lib/release-files.js, 2026-11-10), checked
// against what the app reads. Pure node, no game, the real repo: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/release-files.test.js
//
// The defect this guards: every download carried the 46 garments' full-resolution source PNGs (72
// files, about 154 MiB, up to 4096 texels a side) although the app reads only each garment's .dds,
// which the converter made from them (2026-11-10, the look's textures). And the per-file data
// manifest listed a different set of files from the data archive it describes. The list is built
// here exactly the way the workflow builds it (the module's `releaseFiles`, which its bundle step
// copies with `--copy` and its archive step hands to tar with `--list`), and each check below was
// seen to FAIL with its part of the fix removed: the break and the failure are written beside it.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");
const release = require("../lib/release-files.js");

const ROOT = path.resolve(__dirname, "..", "..");
const GARMENTS = "assets/models/characters/makehuman/garments/";

/** The list, once (a walk of about 1 GB of files' names, no reading). */
const LIST = release.releaseFiles(ROOT);
const SET = new Set(LIST);

/** Every "assets/..." path the character data names (data/characters/**.ron): each garment's .bin
 * and .dds (garments.ron's `file` and `texture`), the base meshes, the detail targets, the bodies,
 * the clips. The app resolves each beside the data folder (engine::animation::resolve_model), which
 * in a download is the install folder, so each must be in the list as written. */
function filesTheCharacterDataNames() {
  const out = new Set();
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) walk(full);
      else if (e.name.endsWith(".ron")) {
        for (const m of fs.readFileSync(full, "utf8").matchAll(/"(assets\/[^"]+)"/g)) out.add(m[1]);
      }
    }
  };
  walk(path.join(ROOT, "data", "characters"));
  return [...out].sort();
}

/** The garment files on disk in this checkout. */
function garmentFilesOnDisk() {
  const out = [];
  const dir = path.join(ROOT, GARMENTS);
  for (const g of fs.readdirSync(dir, { withFileTypes: true })) {
    if (!g.isDirectory()) continue;
    for (const f of fs.readdirSync(path.join(dir, g.name))) out.push(`${GARMENTS}${g.name}/${f}`);
  }
  return out.sort();
}

/** The judge, on any list: what it misses of `needed` and the garment source PNGs it carries. */
function judge(list, needed) {
  const set = new Set(list);
  return {
    missing: needed.filter((p) => !set.has(p)),
    sources: list.filter((p) => p.startsWith(GARMENTS) && p.endsWith(".png") && !p.endsWith(".thumb.png")),
  };
}

const NAMED = filesTheCharacterDataNames();
const ON_DISK = garmentFilesOnDisk();
const THUMBS = ON_DISK.filter((p) => p.endsWith(".thumb.png"));

test("the character data names garment files, thumbnails and source PNGs exist (the check has something to check)", () => {
  assert.ok(NAMED.filter((p) => p.startsWith(GARMENTS) && p.endsWith(".dds")).length >= 40, "garments.ron names the garments' .dds files");
  assert.ok(NAMED.filter((p) => p.startsWith(GARMENTS) && p.endsWith(".bin")).length >= 40, "and their .bin files");
  assert.ok(THUMBS.length >= 40, "the garments' thumbnails are in the checkout");
  assert.ok(ON_DISK.some((p) => p.endsWith(".png") && !p.endsWith(".thumb.png")), "and their source PNGs");
});

// RED CHECK: the module before the garment rule (its EXCLUDED holding only the audio placeholders,
// which is what the workflow's `cp -r` and `tar` shipped). Failed with:
//   AssertionError [ERR_ASSERTION]: 72 garment source PNGs ship, the first
//   assets/models/characters/makehuman/garments/afro01/afro_diffuse.png
test("no garment source PNG ships", () => {
  const { sources } = judge(LIST, []);
  assert.deepStrictEqual(sources, [], `${sources.length} garment source PNGs ship, the first ${sources[0]}`);
});

// RED CHECK: the garment rule leaving out every PNG (its `.thumb.png` exception deleted). Failed with:
//   AssertionError [ERR_ASSERTION]: 46 garment thumbnails are left out, the first
//   assets/models/characters/makehuman/garments/afro01/afro01.thumb.png
test("every garment thumbnail ships", () => {
  const { missing } = judge(LIST, THUMBS);
  assert.deepStrictEqual(missing, [], `${missing.length} garment thumbnails are left out, the first ${missing[0]}`);
});

// RED CHECK: a rule leaving out every file under garments/ but the PNGs (`.bin` and `.dds` dropped
// in its test). Failed with:
//   AssertionError [ERR_ASSERTION]: 92 files the character data names are not shipped, the first
//   assets/models/characters/makehuman/garments/afro01/afro01.bin
test("every file the character data names ships (each garment's .bin and .dds, the bodies, the clips)", () => {
  const { missing } = judge(LIST, NAMED);
  assert.deepStrictEqual(missing, [], `${missing.length} files the character data names are not shipped, the first ${missing[0]}`);
});

test("the judge fails a list with a source PNG in it, or a thumbnail or a .dds taken out (a gate that cannot fail proves nothing)", () => {
  const source = ON_DISK.find((p) => p.endsWith(".png") && !p.endsWith(".thumb.png"));
  assert.strictEqual(judge([...LIST, source], []).sources.length, 1, "a source PNG put back is caught");
  assert.deepStrictEqual(judge(LIST.filter((p) => p !== THUMBS[0]), THUMBS).missing, [THUMBS[0]], "a thumbnail taken out is caught");
  const dds = NAMED.find((p) => p.endsWith(".dds"));
  assert.deepStrictEqual(judge(LIST.filter((p) => p !== dds), NAMED).missing, [dds], "a .dds taken out is caught");
});

test("everything else under the folders still ships: only the rules' files are left out", () => {
  const left = release.leftOut(ROOT);
  for (const l of left) assert.ok(release.EXCLUDED.includes(l.rule), `${l.path} left out by a rule the module lists`);
  for (const p of left.map((l) => l.path)) assert.ok(!SET.has(p), `${p} is both shipped and left out`);
  // The left-out garment files are exactly the source PNGs on disk.
  const leftGarments = left.filter((l) => l.path.startsWith(GARMENTS)).map((l) => l.path).sort();
  assert.deepStrictEqual(leftGarments, ON_DISK.filter((p) => p.endsWith(".png") && !p.endsWith(".thumb.png")));
  // The licence files travel with every download.
  for (const f of ["LICENSE", "LICENSES.md", "CREDITS.md"]) assert.ok(SET.has(f), `${f} ships`);
});

// RED CHECK: gen-data-manifest.js as it was (its own walk of data/, assets/icons and assets/shaders),
// run on this checkout and its list set against the archive's (2026-11-10, before the garment
// rule): it listed 1,083 files and missed 884 the archive carries, the first CREDITS.md, then every
// model, texture and sound (it listed none the archive leaves out only because the audio
// placeholder folders are empty today). This check reads the manifest's list with `--paths-only`,
// which the old script did not have.
test("the per-file data manifest lists exactly what the archive carries", () => {
  const out = execFileSync(process.execPath, [path.join(ROOT, "scripts", "gen-data-manifest.js"), "--paths-only"], { encoding: "utf8", maxBuffer: 64 << 20 });
  const manifest = out.split(/\r?\n/).filter(Boolean);
  const extra = manifest.filter((p) => !SET.has(p));
  const missing = LIST.filter((p) => !manifest.includes(p));
  assert.ok(extra.length === 0 && missing.length === 0, `the manifest lists ${extra.length} files the archive does not carry and misses ${missing.length} it does: first extra ${extra[0]}, first missing ${missing[0]}`);
});

// RED CHECK: the workflow as it was (`cp -r assets/models/ ...` in the bundle step, `tar czf ...
// assets/models/ ...` in the archive step). Failed with:
//   AssertionError [ERR_ASSERTION]: the bundle step copies with its own `cp -r`, not the list
test("the workflow's bundle copy and data archive are both made from the list", () => {
  const yml = fs.readFileSync(path.join(ROOT, ".github", "workflows", "build-desktop.yml"), "utf8");
  const step = (name) => {
    const at = yml.indexOf(`- name: ${name}`);
    assert.ok(at >= 0, `the workflow has its "${name}" step`);
    const next = yml.indexOf("\n      - name:", at + 1);
    return yml.slice(at, next < 0 ? undefined : next);
  };
  const bundle = step("Bundle binary with data");
  assert.ok(!/^\s*cp -r (data|assets)/m.test(bundle), "the bundle step copies with its own `cp -r`, not the list");
  assert.match(bundle, /node scripts\/lib\/release-files\.js --copy dist\/HumanityOS/, "the bundle step copies the list");
  const archive = step("Create data archive");
  assert.match(archive, /node scripts\/lib\/release-files\.js --list > "\$RUNNER_TEMP\/release-files\.txt"/, "the archive step writes the list");
  assert.match(archive, /tar czf HumanityOS-data-[^\n]*\\\r?\n\s*--verbatim-files-from -T "\$RUNNER_TEMP\/release-files\.txt"/, "and hands it to tar");
  assert.ok(!/^\s*(data|assets\/\w+)\/ \\\r?$/m.test(archive), "the archive step names folders of its own beside the list");
});
