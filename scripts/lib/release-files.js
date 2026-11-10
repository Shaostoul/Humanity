// WHAT A RELEASE'S DOWNLOAD CARRIES BESIDE THE PROGRAM, in ONE place (2026-11-10).
//
// Three things ship the game's data and assets, and until today each listed them its own way:
//   - the platform bundles (HumanityOS-<platform>.zip and .tar.gz, the fresh-install download),
//     built in .github/workflows/build-desktop.yml's "Bundle binary with data" step with `cp -r`;
//   - the data archive (HumanityOS-data-<tag>.tar.gz), the same workflow's "Create data archive"
//     step, with `tar` and its own `--exclude`s;
//   - the per-file data manifest (data-manifest-<tag>.json, scripts/gen-data-manifest.js), which
//     walked only data/, assets/icons/ and assets/shaders/, so it named none of the models,
//     textures or sounds the archive carries, and named the audio placeholders the archive leaves
//     out: a client syncing file by file from it would have asked for files the release does not
//     have and never fetched files it does.
// Now all three take their list from `releaseFiles` here: the bundle step copies it (`--copy`), the
// archive step hands it to tar (`--list`), and the manifest lists it. scripts/tests/release-files.test.js
// checks the list against what the app reads.
//
// WHAT IS LEFT OUT, and why (each rule names its reason):
//   - data/audio/sfx, data/audio/music, data/audio/ambient: large audio placeholders (the rule both
//     steps carried before; the folders are empty today, the music's own recordings live in
//     data/music and ship).
//   - the garments' source PNGs (assets/models/characters/makehuman/garments/<id>/*.png but each
//     garment's <id>.thumb.png): MakeHuman's full-resolution textures, up to 4096 texels a side, 72
//     files and about 154 MiB. Since 2026-11-10 the app reads each garment's .dds, which the
//     converter (scripts/gen-makehuman-look.js) packs and BC7-encodes from them, and never the
//     PNGs (checked: nothing under src/ or crates/ opens one). They stay whole in git, where the
//     converter's `--check` reads them (CLAUDE.md's space rule keeps real data whole in the repo,
//     not in every person's download of files nothing reads). The thumbnails ship: about 3 MiB, and
//     the wardrobe choosers are where they belong (nothing reads them yet either; they are the
//     garments' own preview pictures, kept so a picture chooser needs no new download).
//
// Usage:
//   node scripts/lib/release-files.js --list            one path a line (POSIX), for `tar -T`
//   node scripts/lib/release-files.js --copy <dest>     copy every file to <dest>/<path>
//   node scripts/lib/release-files.js --summary         counts and sizes, shipped and left out

const fs = require("fs");
const path = require("path");

/** The folders the download carries, relative to the repo root. */
const ROOTS = ["data", "assets/icons", "assets/shaders", "assets/models", "assets/textures", "assets/audio"];

/** Single files the download carries: the licence files travel with every download (the ODbL offer
 * for the OpenStreetMap region files and the CC BY-SA offer for the star catalogues are in
 * LICENSES.md, 2026-11-08). */
const FILES = ["LICENSE", "LICENSES.md", "CREDITS.md"];

/** What is left out of ROOTS: each rule a test on a POSIX path relative to the repo root, and why. */
const EXCLUDED = [
  {
    what: "data/audio/sfx/, data/audio/music/, data/audio/ambient/",
    why: "large audio placeholders (the music's recordings live in data/music and ship)",
    test: (rel) => /^data\/audio\/(sfx|music|ambient)\//.test(rel),
  },
  {
    what: "the garments' source PNGs (assets/models/characters/makehuman/garments/**/*.png but *.thumb.png)",
    why: "the app reads each garment's .dds the converter made from them, never the PNGs; they stay whole in git",
    test: (rel) => rel.startsWith("assets/models/characters/makehuman/garments/") && rel.endsWith(".png") && !rel.endsWith(".thumb.png"),
  },
];

/** The rule that leaves `rel` out, or null when it ships. */
function excludedBy(rel) {
  return EXCLUDED.find((r) => r.test(rel)) || null;
}

function walk(dir, out) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.isFile()) out.push(full);
  }
  return out;
}

/** Every file under ROOTS and every one of FILES, relative and POSIX, sorted; `left` (when given)
 * collects the files a rule left out, with the rule. */
function scan(root, left) {
  const shipped = [];
  for (const r of ROOTS) {
    for (const full of walk(path.join(root, r), [])) {
      const rel = path.relative(root, full).replace(/\\/g, "/");
      const rule = excludedBy(rel);
      if (rule) {
        if (left) left.push({ path: rel, rule });
      } else {
        shipped.push(rel);
      }
    }
  }
  for (const f of FILES) {
    if (fs.existsSync(path.join(root, f))) shipped.push(f);
  }
  return shipped.sort();
}

/** The files a release's download carries beside the program, relative to `root`, POSIX, sorted. */
function releaseFiles(root) {
  return scan(root, null);
}

/** The files left out, each with the rule that left it out. */
function leftOut(root) {
  const left = [];
  scan(root, left);
  return left.sort((a, b) => a.path.localeCompare(b.path));
}

function bytesOf(root, rels) {
  return rels.reduce((s, rel) => s + fs.statSync(path.join(root, rel)).size, 0);
}

const mib = (b) => (b / 1048576).toFixed(1);

function main(argv) {
  const root = path.resolve(__dirname, "..", "..");
  const [cmd, arg] = argv;
  if (cmd === "--list") {
    process.stdout.write(releaseFiles(root).join("\n") + "\n");
    return 0;
  }
  if (cmd === "--copy") {
    if (!arg) {
      console.error("usage: release-files.js --copy <dest>");
      return 2;
    }
    const files = releaseFiles(root);
    for (const rel of files) {
      const to = path.join(arg, rel);
      fs.mkdirSync(path.dirname(to), { recursive: true });
      fs.copyFileSync(path.join(root, rel), to);
    }
    const left = leftOut(root);
    console.log(`Copied ${files.length} files (${mib(bytesOf(root, files))} MiB) to ${arg}; left out ${left.length} (${mib(bytesOf(root, left.map((l) => l.path)))} MiB): ${EXCLUDED.map((r) => r.what).join("; ")}`);
    return 0;
  }
  if (cmd === "--summary") {
    const files = releaseFiles(root);
    const left = leftOut(root);
    console.log(`ships ${files.length} files, ${mib(bytesOf(root, files))} MiB`);
    for (const r of EXCLUDED) {
      const mine = left.filter((l) => l.rule === r).map((l) => l.path);
      console.log(`leaves out ${mine.length} files, ${mib(bytesOf(root, mine))} MiB: ${r.what} (${r.why})`);
    }
    return 0;
  }
  console.error("usage: release-files.js --list | --copy <dest> | --summary");
  return 2;
}

module.exports = { ROOTS, FILES, EXCLUDED, excludedBy, releaseFiles, leftOut };

if (require.main === module) process.exit(main(process.argv.slice(2)));
