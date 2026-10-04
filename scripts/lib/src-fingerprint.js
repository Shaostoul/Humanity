// The source fingerprint: which tree a binary was compiled from (BUG-133).
//
// build.rs hashes every compiled-in source at build time and compiles the
// result into the exe (src/main.rs SOURCE_STAMP). This module computes the same
// thing from a tree on disk, reads the stamp back out of an exe's bytes, and
// decides whether the two match. scripts/check-fresh-exe.js is the CLI over it;
// every rig reaches that CLI through runFreshGate() at the bottom, so the
// --allow-other-build escape hatch and its record cannot be dropped by a rig.
//
// Why content and not dates: the gate used to call a binary current when it was
// newer than every source file. A build of a DIFFERENT tree (main tested in a
// worktree, or the reverse) passed whenever it happened to be newer, which is
// how a 1a build passed the 1b worktree's check on 2026-10-03.
//
// THE DEFINITION (build.rs `write_source_stamp` is the other half; the tests in
// scripts/tests/check-fresh-exe.test.js pin the two together):
//   - the inputs are build.rs's FINGERPRINT_INPUTS list, read out of the tree's
//     own build.rs (readInputs), so there is one list and it travels with the code
//   - every regular file at or under each input, as a '/'-separated path relative
//     to the tree root; symlinks and junctions are skipped, not followed
//   - sorted by the UTF-8 bytes of that path
//   - each file's content with every CRLF turned into LF, hashed with SHA-256
//   - manifest = "hos-src-manifest v1\n" + "<sha256 hex> <path>\n" per file
//   - fingerprint = SHA-256 hex of the manifest
//
// The exe carries the whole manifest, so a mismatch can name the files.

const fs = require("fs");
const os = require("os");
const path = require("path");
const crypto = require("crypto");
const { spawnSync } = require("child_process");

const MANIFEST_HEADER = "hos-src-manifest v1\n";
const STAMP_BEGIN = "HOS-SRC-STAMP v"; // followed by the version and "\n"
const STAMP_END = "HOS-SRC-STAMP END\n";
const STAMP_VERSION = "1";
const GATE = path.join(__dirname, "..", "check-fresh-exe.js");
const CRLF = Buffer.from("\r\n");

const sha256 = (buf) => crypto.createHash("sha256").update(buf).digest("hex");
const byteOrder = (a, b) => Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8"));

/** The FINGERPRINT_INPUTS list out of `<root>/build.rs`. Throws, with the file
 *  named, when it is not there or is not a flat list of string literals: an
 *  empty or half-read list would fingerprint nothing and match everything. */
function readInputs(root) {
  const file = path.join(root, "build.rs");
  let src;
  try {
    src = fs.readFileSync(file, "utf8");
  } catch (e) {
    throw new Error(`cannot read ${file} to learn which sources are compiled in (FINGERPRINT_INPUTS): ${e.message}`);
  }
  const m = src.match(/const\s+FINGERPRINT_INPUTS\s*:\s*&\s*\[\s*&\s*str\s*\]\s*=\s*&\s*\[([^\]]*)\]\s*;/);
  if (!m) {
    throw new Error(
      `${file} has no \`const FINGERPRINT_INPUTS: &[&str] = &[...];\` list. That list is the one ` +
        "definition of which sources are compiled in; the gate cannot fingerprint a tree without it."
    );
  }
  const items = [...m[1].matchAll(/"([^"\\]*)"/g)].map((x) => x[1]);
  const leftover = m[1]
    .replace(/"([^"\\]*)"/g, "")
    .replace(/\/\/[^\n]*/g, "")
    .replace(/[\s,]/g, "");
  if (!items.length || leftover) {
    throw new Error(
      `${file}: FINGERPRINT_INPUTS must be a flat list of plain string literals (found ${JSON.stringify(m[1].trim())}).`
    );
  }
  return items;
}

/** Every regular file at or under `rel`, '/'-separated, relative to root. */
function collect(root, rel, out) {
  let st;
  try {
    st = fs.lstatSync(path.join(root, rel));
  } catch {
    return;
  }
  if (st.isDirectory()) {
    let entries;
    try {
      entries = fs.readdirSync(path.join(root, rel), { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      const child = `${rel}/${e.name}`;
      if (e.isDirectory()) collect(root, child, out);
      else if (e.isFile()) out.push(child);
    }
  } else if (st.isFile()) {
    out.push(rel);
  }
}

/** CRLF -> LF; a lone CR stays (git does not treat it as a line ending). */
function crlfToLf(buf) {
  let i = buf.indexOf(CRLF);
  if (i < 0) return buf;
  const parts = [];
  let start = 0;
  while (i >= 0) {
    parts.push(buf.subarray(start, i));
    start = i + 1; // keep the LF, drop the CR
    i = buf.indexOf(CRLF, start);
  }
  parts.push(buf.subarray(start));
  return Buffer.concat(parts);
}

/** The fingerprint of the tree at `root`: { fingerprint, text, files: Map(path -> sha256), inputs, root }. */
function fingerprintTree(root, inputs = readInputs(root)) {
  const list = [];
  for (const input of inputs) collect(root, input, list);
  const paths = [...new Set(list)].sort(byteOrder);
  const files = new Map();
  let text = MANIFEST_HEADER;
  for (const rel of paths) {
    const digest = sha256(crlfToLf(fs.readFileSync(path.join(root, rel))));
    files.set(rel, digest);
    text += `${digest} ${rel}\n`;
  }
  return { fingerprint: sha256(Buffer.from(text, "utf8")), text, files, inputs, root };
}

/** Parse one stamp's text (from its "HOS-SRC-STAMP v" to just past its END
 *  line). Returns { ok: true, fingerprint, files: Map, count, profile, features }
 *  or { ok: false, problem }. The manifest must hash to the fingerprint it
 *  carries, so a truncated or garbled read can never pass as a stamp. */
function parseStamp(text) {
  const lines = text.split("\n");
  const version = (lines[0] || "").slice(STAMP_BEGIN.length);
  if (version !== STAMP_VERSION) {
    return { ok: false, problem: `stamp version "${version}" is not one this gate reads (it reads v${STAMP_VERSION})` };
  }
  const field = (i, name, re) => {
    const m = (lines[i] || "").match(new RegExp(`^${name} (${re})$`));
    return m ? m[1] : null;
  };
  const fingerprint = field(1, "fingerprint", "[0-9a-f]{64}");
  const count = field(2, "files", "\\d+");
  const profile = field(3, "profile", "\\S+");
  const features = field(4, "features", "\\S+");
  if (!fingerprint || count === null || !profile || !features) {
    return { ok: false, problem: "the stamp's header lines (fingerprint, files, profile, features) are not as build.rs writes them" };
  }
  const start = lines.slice(0, 5).join("\n").length + 1;
  const endAt = text.lastIndexOf(STAMP_END);
  const manifestText = text.slice(start, endAt);
  if (!manifestText.startsWith(MANIFEST_HEADER)) {
    return { ok: false, problem: "the stamp has no manifest header" };
  }
  const files = new Map();
  for (const line of manifestText.slice(MANIFEST_HEADER.length).split("\n")) {
    if (!line) continue;
    const m = line.match(/^([0-9a-f]{64}) (.+)$/);
    if (!m) return { ok: false, problem: `a manifest line is malformed: ${JSON.stringify(line.slice(0, 120))}` };
    files.set(m[2], m[1]);
  }
  if (files.size !== Number(count)) {
    return { ok: false, problem: `the stamp says ${count} files but lists ${files.size}` };
  }
  const actual = sha256(Buffer.from(manifestText, "utf8"));
  if (actual !== fingerprint) {
    return { ok: false, problem: `the manifest hashes to ${actual}, not to the fingerprint ${fingerprint} it carries` };
  }
  return { ok: true, fingerprint, files, count: files.size, profile, features };
}

/** Every stamp in the exe's bytes: { stamps: [parsed...], malformed: [problems],
 *  sha256 } where sha256 is the hash of the very bytes the stamps were read
 *  from (the gate records it, so a rig can prove the file it boots is the one
 *  that was judged: checkBootCopy below). */
function readExeStamp(exePath) {
  const buf = fs.readFileSync(exePath);
  const begin = Buffer.from(STAMP_BEGIN);
  const end = Buffer.from(STAMP_END);
  const stamps = [];
  const malformed = [];
  const fileSha256 = sha256(buf);
  let at = buf.indexOf(begin);
  while (at >= 0) {
    const stop = buf.indexOf(end, at);
    if (stop < 0) {
      malformed.push("a stamp starts but never ends (the exe is truncated or the marker is not build.rs's)");
      break;
    }
    const p = parseStamp(buf.subarray(at, stop + end.length).toString("utf8"));
    if (p.ok) {
      stamps.push(p);
      at = buf.indexOf(begin, stop + end.length);
    } else {
      // Resume just past THIS marker, not past the END it ran into: a stray
      // marker before the real stamp must not swallow the real one.
      malformed.push(p.problem);
      at = buf.indexOf(begin, at + begin.length);
    }
  }
  return { stamps, malformed, sha256: fileSha256 };
}

/** Which files differ between the build's manifest and the tree's. */
function diffManifests(exeFiles, treeFiles) {
  const changed = [];
  const onlyTree = [];
  const onlyExe = [];
  for (const [p, h] of treeFiles) {
    if (!exeFiles.has(p)) onlyTree.push(p);
    else if (exeFiles.get(p) !== h) changed.push(p);
  }
  for (const p of exeFiles.keys()) if (!treeFiles.has(p)) onlyExe.push(p);
  return { changed, onlyTree, onlyExe, total: changed.length + onlyTree.length + onlyExe.length };
}

/** Indented lines naming the differing files (the first `limit` of them). */
function describeDiff(d, limit = 12) {
  const rows = [
    ...d.changed.map((p) => `    changed            ${p}`),
    ...d.onlyTree.map((p) => `    only in this tree  ${p}`),
    ...d.onlyExe.map((p) => `    only in the build  ${p}`),
  ];
  const out = [
    `  files that differ   ${d.total}: ${d.changed.length} changed, ${d.onlyTree.length} only in this tree, ` +
      `${d.onlyExe.length} only in the build`,
    ...rows.slice(0, limit),
  ];
  if (rows.length > limit) out.push(`    ... and ${rows.length - limit} more (node scripts/check-fresh-exe.js --print-manifest exe|tree, then diff)`);
  return out;
}

const REBUILD = [
  "Fix (pick one):",
  "  cargo build --features native --release    # rebuild target/release from this tree",
  "  just build-game                            # rebuild + bump version + archive",
];
const OTHER_BUILD_HINT = [
  "To run a different build ON PURPOSE (a red check against an old or other build):",
  '  add --allow-other-build "<why>"   (the rigs pass it through and record it in their manifest)',
];

/**
 * The verdict. Pure: no I/O.
 *   exeInfo    readExeStamp() output
 *   tree       fingerprintTree() output
 *   allowOther null, or the --allow-other-build reason
 * Returns { ok, verdict: "current" | "other-build" | "refused", headline,
 *           lines (to print), other_build (the manifest record or null),
 *           exe_fingerprint, tree_fingerprint }.
 */
function decide({ exeLabel, exeInfo, tree, allowOther }) {
  const treeFp = tree.fingerprint;
  const fps = [...new Set(exeInfo.stamps.map((s) => s.fingerprint))];
  const treeLine = `  this tree           ${treeFp}  (${tree.files.size} files under ${tree.inputs.join(", ")} in ${tree.root})`;
  const result = (verdict, headline, lines, exeFp, extra = {}) => ({
    ok: verdict !== "refused",
    verdict,
    headline,
    lines,
    exe_fingerprint: exeFp,
    tree_fingerprint: treeFp,
    other_build: null,
    ...extra,
  });

  // Two stamps that disagree cannot come from one build. Refused even with the
  // flag: there is no single "build" to record.
  if (fps.length > 1) {
    return result("refused", `TWO STAMPS: ${exeLabel} carries ${fps.length} different source fingerprints.`, [
      ...fps.map((f) => `  stamp               ${f}`),
      treeLine,
      "",
      "One build writes one stamp. Two means the file was spliced or the marker text was",
      "compiled in somewhere else too, so the stamp cannot say which tree this is. Refusing.",
      ...REBUILD,
    ], null);
  }

  const stamp = exeInfo.stamps[0] || null;
  if (stamp && stamp.fingerprint === treeFp) {
    const lines = [];
    if (allowOther !== null) {
      lines.push("note: --allow-other-build was given, but this exe IS this tree's build; nothing to allow, nothing recorded.");
    }
    return result("current", "PASS: the binary under test is the current build (built from exactly this tree's compiled-in sources)", lines, stamp.fingerprint);
  }

  // From here the exe is not this tree's build (or cannot say whose it is).
  let headline;
  let detail;
  if (stamp) {
    const d = diffManifests(stamp.files, tree.files);
    headline = `WRONG TREE: ${exeLabel} was not built from this tree's sources.`;
    detail = [
      `  exe fingerprint     ${stamp.fingerprint}  (${stamp.count} files, ${stamp.profile}, features ${stamp.features})`,
      `  tree fingerprint    ${treeFp}  (${tree.files.size} files, ${tree.root})`,
      ...describeDiff(d),
      "",
      "A handful of files means they were edited after this build (in a shared checkout, perhaps",
      "by another session). Many means the binary came from another checkout or branch: the",
      "BUG-133 case, where a build of main passed a feature worktree's date-only check.",
    ];
  } else if (exeInfo.malformed.length) {
    headline = `BROKEN STAMP: ${exeLabel} carries a source stamp this gate cannot trust.`;
    detail = [...exeInfo.malformed.map((p) => `  problem             ${p}`), treeLine];
  } else {
    headline = `NO SOURCE STAMP: ${exeLabel} carries no source fingerprint, so nothing can say which tree it was built from.`;
    detail = [
      "  It was built before the stamp existed (BUG-133, 2026-10-03), or by a build.rs that writes none.",
      treeLine,
      "",
      "A file date cannot tell a build of this tree from a build of another one, which is how a",
      "build of main passed a feature worktree's check on 2026-10-03.",
    ];
  }

  if (allowOther !== null) {
    const exeFp = stamp ? stamp.fingerprint : null;
    const bar = "!".repeat(78);
    return result(
      "other-build",
      "OTHER BUILD: this binary is NOT this tree's code. Allowed on purpose with --allow-other-build.",
      [
        bar,
        "!! OTHER BUILD: this binary is NOT this tree's code. Allowed on purpose.",
        `!!   reason            ${allowOther}`,
        `!!   exe fingerprint   ${exeFp || "none (an unstamped build from before BUG-133, or a broken stamp)"}`,
        `!!   tree fingerprint  ${treeFp}`,
        `!!   why               ${headline}`,
        ...(stamp ? describeDiff(diffManifests(stamp.files, tree.files), 6).map((l) => `!! ${l}`) : []),
        "!! Every result from this run is about THAT build. Do not report it as a check of",
        "!! this tree's code. The rig records it in its manifest as other_build.",
        bar,
      ],
      exeFp,
      { other_build: { reason: allowOther, exe_fingerprint: exeFp, tree_fingerprint: treeFp } }
    );
  }

  return result("refused", headline, [
    ...detail,
    "",
    "Booting it would verify different code than the tree you are looking at. Refusing instead",
    "of reporting a meaningless pass.",
    ...REBUILD,
    "If a rebuild leaves the exe's fingerprint unchanged, cargo did not see the edit (a file",
    "copied in with its old date keeps cargo asleep): touch build.rs and build again.",
    ...OTHER_BUILD_HINT,
  ], stamp ? stamp.fingerprint : null);
}

/** The value after `--allow-other-build` in a rig's argv: undefined when the
 *  flag is absent, "" when it has no value (the gate then refuses, asking for
 *  a reason), else the reason. */
function allowOtherFrom(argv) {
  const i = argv.indexOf("--allow-other-build");
  if (i < 0) return undefined;
  const v = argv[i + 1];
  return v === undefined || v.startsWith("--") ? "" : v;
}

/**
 * Run the gate the way every rig must: forwards --allow-other-build from the
 * rig's own argv, and returns the gate's machine-readable result so the rig
 * can record `other_build` in its manifest.
 *   exe       the binary the rig is about to boot
 *   rigArgv   the rig's process.argv.slice(2)
 *   opts      { stdio: "inherit" (default) | "pipe", cwd, gateArgs: extra gate args }
 * Returns { status, other_build, result, stdout, stderr }. status 0 = may boot.
 */
function runFreshGate(exe, rigArgv = [], opts = {}) {
  const stdio = opts.stdio || "inherit";
  const out = path.join(os.tmpdir(), `hos-fresh-${process.pid}-${Date.now()}-${Math.random().toString(36).slice(2)}.json`);
  const args = [GATE, "--exe", exe, "--json-out", out, ...(opts.gateArgs || [])];
  const allow = allowOtherFrom(rigArgv);
  if (allow !== undefined) args.push("--allow-other-build", allow);
  const r = spawnSync(process.execPath, args, {
    cwd: opts.cwd,
    stdio,
    encoding: stdio === "pipe" ? "utf8" : undefined,
  });
  let result = null;
  try {
    result = JSON.parse(fs.readFileSync(out, "utf8"));
  } catch {}
  try {
    fs.rmSync(out, { force: true });
  } catch {}
  return {
    status: r.status,
    other_build: result ? result.other_build : null,
    result,
    stdout: r.stdout || "",
    stderr: r.stderr || "",
  };
}

/**
 * Is `copy`, the file a rig is about to boot, the very binary the gate judged?
 *
 * Every rig copies the exe into its own folder AFTER runFreshGate checked the
 * original, so something writing the original in between (a cargo build
 * finishing, another session's deliver) would put a binary nobody judged into
 * the rig (BUG-133, the second gap). The gate records the SHA-256 of the bytes
 * it judged (result.exe_sha256); the copy must hash the same. Bytes, not the
 * stamp: two builds of one tree carry the same stamp, and "the same source" is
 * not the claim a rig makes; "this binary" is.
 *   copy    the path the rig will spawn
 *   fresh   runFreshGate()'s return value
 * Returns { ok, sha256, lines } (lines say what differs when !ok).
 */
function checkBootCopy(copy, fresh) {
  const judged = fresh && fresh.result ? fresh.result : null;
  const want = judged ? judged.exe_sha256 : null;
  if (!want) {
    return {
      ok: false,
      sha256: null,
      lines: [
        `BOOT COPY UNCHECKED: the freshness gate recorded no hash of the exe it judged, so ${copy}`,
        "cannot be shown to be that exe. (A gate from before the boot-copy check, or one that refused.)",
      ],
    };
  }
  let buf;
  try {
    buf = fs.readFileSync(copy);
  } catch (e) {
    return { ok: false, sha256: null, lines: [`BOOT COPY MISSING: ${copy} cannot be read (${e.message}).`] };
  }
  const got = sha256(buf);
  if (got === want) return { ok: true, sha256: got, lines: [] };
  const info = readExeStamp(copy);
  const fp = info.stamps.length === 1 ? info.stamps[0].fingerprint : null;
  const whose = !fp
    ? "none readable"
    : fp === judged.exe_fingerprint
      ? `${fp} (the same sources as the judged exe: a rebuild of it, but not the bytes that were judged)`
      : `${fp} (DIFFERENT sources from the judged exe)`;
  return {
    ok: false,
    sha256: got,
    lines: [
      `BOOT COPY CHANGED: ${copy} is not the binary the freshness gate judged.`,
      `  judged             ${judged.exe}  sha256 ${want}`,
      `  about to boot      ${copy}  sha256 ${got}`,
      `  its source stamp   ${whose}`,
      "The exe changed between the check and the copy (most likely a build finished in between), or",
      "the rig is booting a copy it did not make. Booting it would verify a binary nobody checked.",
      "Run the rig again once nothing is writing the exe.",
    ],
  };
}

/** checkBootCopy, the way a rig uses it: prints one line and returns on a
 *  match, prints why and exits 1 (nothing booted) on anything else. */
function requireBootCopy(copy, fresh, rigName) {
  const r = checkBootCopy(copy, fresh);
  if (r.ok) {
    console.log(`[fresh] boot copy    ${copy} is byte-identical to the judged exe (sha256 ${r.sha256.slice(0, 16)})`);
    return r;
  }
  console.error("");
  for (const l of r.lines) console.error(l);
  console.error("");
  console.error(`${rigName}: REFUSED - nothing was booted.`);
  process.exit(1);
}

/** What a rig's manifest records about the binary it booted. */
function bootRecord(fresh, copy) {
  const r = fresh && fresh.result ? fresh.result : {};
  return {
    judged_exe: r.exe || null,
    booted_copy: copy,
    sha256: r.exe_sha256 || null,
    source_fingerprint: r.exe_fingerprint || null,
    verdict: r.verdict || null,
  };
}

/** One line for a rig's verdict block when its manifest records another build. */
function otherBuildNotice(ob) {
  if (!ob) return null;
  const fp = (f) => (f ? f.slice(0, 16) : "none (unstamped)");
  return (
    `OTHER BUILD  this verdict is about a binary that is NOT this tree (exe ${fp(ob.exe_fingerprint)}, ` +
    `tree ${fp(ob.tree_fingerprint)}): ${ob.reason}`
  );
}

module.exports = {
  MANIFEST_HEADER,
  STAMP_BEGIN,
  STAMP_END,
  readInputs,
  crlfToLf,
  fingerprintTree,
  parseStamp,
  readExeStamp,
  diffManifests,
  decide,
  allowOtherFrom,
  runFreshGate,
  checkBootCopy,
  requireBootCopy,
  bootRecord,
  otherBuildNotice,
};
