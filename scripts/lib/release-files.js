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
//   node scripts/lib/release-files.js --phone-list      the phone app's files (below), one a line
//   node scripts/lib/release-files.js --phone-summary   their count and size, and what is left out
//
// THE PHONE APP'S FILES (Android rung A1, 2026-11-14; android/README.md). The APK carries the
// release's files for the app's PAGES, and the first start writes them into the app's own folder
// (src/android.rs). It leaves out what only the 3D world reads (PHONE_LEFT_OUT, each with its
// reason): a phone has no 3D world yet, and those files are most of the release's size. The data
// files compiled into the app are in the list too; the phone's first start skips them and the
// data folder's refresh writes them (crates/humanity-core/src/storage/refresh.rs), as on a desktop.

const fs = require("fs");
const path = require("path");

/** The folders the download carries, relative to the repo root. assets/emoji (2026-11-20): the emoji
 * pack (OpenMoji's SVGs, their LICENSE.md, sets.json and index.json, about 24 MB), which the desktop's
 * chat draws its emoji from (src/gui/widgets/emoji/pack.rs, docs/design/emoji.md rung 2); the phone
 * app carries it too, its chat draws them the same way. */
const ROOTS = ["data", "assets/icons", "assets/shaders", "assets/models", "assets/textures", "assets/audio", "assets/emoji"];

/** Single files the download carries: the licence files travel with every download (the ODbL offer
 * for the OpenStreetMap region files and the CC BY-SA offer for the star catalogues are in
 * LICENSES.md, 2026-11-08), and LICENSING.md says which licence covers which folder (2026-11-14).
 * THIRD-PARTY-NOTICES.txt (2026-11-20, scripts/gen-third-party-notices.js) is the copyright and
 * licence notices of every Rust package the programs link and of the website's vendored code, which
 * their MIT, BSD and Apache-2.0 licences ask to travel with the program; the app reads it from here
 * (beside data/) for Settings > Credits. */
const FILES = ["LICENSE", "LICENSING.md", "LICENSES.md", "CREDITS.md", "THIRD-PARTY-NOTICES.txt"];

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

/** What the phone app's files leave out of the release's, on top of EXCLUDED: what only the 3D world
 * reads. Each rule a test on a POSIX path relative to the repo root, and why. */
const PHONE_LEFT_OUT = [
  {
    what: "assets/models/, assets/textures/",
    why: "the 3D world's models and textures (about 450 MiB): a phone has no 3D world yet (rung A5)",
    test: (rel) => rel.startsWith("assets/models/") || rel.startsWith("assets/textures/"),
  },
  {
    what: "data/planets/, data/stars.csv, data/stars.bin, data/stars-map.bin, data/galaxy_glow.png",
    why: "the 3D world's planets and sky (about 190 MiB): read only when the world is drawn",
    test: (rel) =>
      rel.startsWith("data/planets/") || ["data/stars.csv", "data/stars.bin", "data/stars-map.bin", "data/galaxy_glow.png"].includes(rel),
  },
];

/** The phone app's files: the release's, less PHONE_LEFT_OUT (the comment at the top). */
function phoneFiles(root) {
  return releaseFiles(root).filter((rel) => !PHONE_LEFT_OUT.some((r) => r.test(rel)));
}

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
  if (cmd === "--phone-list") {
    process.stdout.write(phoneFiles(root).join("\n") + "\n");
    return 0;
  }
  if (cmd === "--phone-summary") {
    const files = phoneFiles(root);
    console.log(`the phone app carries ${files.length} files, ${mib(bytesOf(root, files))} MiB`);
    const all = releaseFiles(root);
    for (const r of PHONE_LEFT_OUT) {
      const mine = all.filter((rel) => r.test(rel));
      console.log(`leaves out ${mine.length} files, ${mib(bytesOf(root, mine))} MiB: ${r.what} (${r.why})`);
    }
    return 0;
  }
  console.error("usage: release-files.js --list | --copy <dest> | --summary | --phone-list | --phone-summary");
  return 2;
}

module.exports = { ROOTS, FILES, EXCLUDED, PHONE_LEFT_OUT, excludedBy, releaseFiles, leftOut, phoneFiles };

// exitCode, not process.exit() (2026-11-22): on Linux a write to a pipe finishes in the background,
// so exiting at once could cut `--list` or `--phone-list` off at 64 KiB when read through a pipe
// (gen-data-manifest.js had exactly that bug; CI writes these lists to files, which are written at once).
if (require.main === module) process.exitCode = main(process.argv.slice(2));
