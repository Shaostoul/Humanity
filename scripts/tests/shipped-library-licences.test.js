// A NATIVE LIBRARY IN THE DOWNLOAD CARRIES ITS LICENCE BESIDE IT (2026-11-21).
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/shipped-library-licences.test.js
//
// The defect this guards: every Windows download since v0.865 carried Microsoft's dxcompiler.dll
// and dxil.dll beside the program, copied from the Windows SDK by .github/workflows/build-desktop.yml,
// with no licence or notice of any kind beside them, and comments calling them "MIT" (they are not:
// DXC's licence is University of Illinois/NCSA, and the SDK's copy falls under the SDK licence). Found
// by the Digital Public Goods review of 2026-11-20; the research is
// docs/reference/findings/2026-11-21-dxc-redistribution.md.
//
// THE RULE. Every library file (.dll, .so, .dylib) that goes into the desktop download's folder
// (dist/HumanityOS, which the workflow zips) must have, in the SAME folder of the download, a
// licence file named for it: `<stem>.LICENSE.txt` (or `<name>.LICENSE.txt`, or `.md`), for example
// dxcompiler.dll beside dxcompiler.LICENSE.txt. Named for it, because the download's root already
// holds LICENSE and LICENSES.md, the project's own: a rule satisfied by "any licence file in the
// folder" would pass for a DLL that carries nothing of its own (the third check below shows it).
//
// WHERE IT LOOKS. The two ways a file reaches dist/HumanityOS: a `cp` in build-desktop.yml, and the
// release list (scripts/lib/release-files.js, which the workflow's `--copy` ships whole). Any other
// mention of a library in the workflow (a `7z a` of one, a recursive `cp -r` into the download, whose
// contents this check cannot see) fails as unknown, so a new way in has to teach this check first.
//
// EXCEPTIONS, `PENDING`: a library shipped without a licence while a decision is pending, each naming
// its finding; the fourth check fails as soon as an exception is no longer needed (the library gone,
// or shipped with its licence), so an exception cannot outlive its fix. Empty since 2026-11-20: the
// DXC pair, its one entry, now comes from our own build of DXC's open source (build-dxc.yml) with
// DXC's LICENSE.TXT beside each file. Seen at the switch: with the entry still present, "every
// exception is still needed" failed with "dxcompiler.dll is no longer shipped without a licence:
// delete its PENDING entry"; emptied, all seven checks pass.
//
// SEEN RED (2026-11-21), each check with its part of the fix removed (a mutated copy run by hand):
//   - `PENDING` emptied: "the desktop download's libraries each carry a licence" failed with
//       dxcompiler.dll: no dxcompiler.LICENSE.txt beside it in the download
//       dxil.dll: no dxil.LICENSE.txt beside it in the download
//     (the real workflow, read as CI runs it).
//   - `PENDING` given a "lib/extra.dll" entry: "every exception is still needed" failed with
//       lib/extra.dll is no longer shipped without a licence: delete its PENDING entry
//   - `librariesWithoutLicence` made to accept any LICENSE* file in the folder: the "the project's
//     own LICENSE does not count" case failed (expected ['foo.dll'], got []), AND on the real
//     download "every exception is still needed" failed with "dxcompiler.dll is no longer shipped
//     without a licence": the root's LICENSE (the project's AGPL text) had "covered" the DLL.
//   - the `cp -r` rule removed: the "a way in it cannot see" case failed (the recursive copy was
//     read as shipping one file named "libs" instead of being reported).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const release = require("../lib/release-files.js");

const ROOT = path.resolve(__dirname, "..", "..");
const WORKFLOW = ".github/workflows/build-desktop.yml";
/** The folder the workflow fills and zips: the download, as a person unpacks it. */
const DOWNLOAD = "dist/HumanityOS";
const FINDING = "docs/reference/findings/2026-11-21-dxc-redistribution.md";

/** Shipped today without a licence, each waiting on the decision its finding names. Download paths.
 * Empty since 2026-11-20 (the DXC pair, its last entry, ships with its licence); an entry looks like
 * `["dxcompiler.dll", FINDING]`. */
const PENDING = new Map([]);

/** A library file by its name: Windows DLLs, Linux shared objects (versioned too), macOS dylibs. */
const isLibrary = (name) => /\.(dll|dylib)$/i.test(name) || /\.so(\.\d+)*$/i.test(name);
/** A library named anywhere in a word (a path, a variable-prefixed path). */
const MENTIONS_LIBRARY = /\.(dll|dylib|so)(\.\d+)*(?![\w])/i;

/** The licence file names that count as "named for" `name`, lower case. */
function licenceNamesFor(name) {
  const stem = name.replace(/\.[^.]+$/, "");
  const out = [];
  for (const base of [stem, name]) for (const ext of ["txt", "md"]) out.push(`${base}.LICENSE.${ext}`.toLowerCase());
  return out;
}

/** One shell line split into words the way bash would, near enough: quotes join, `#` outside a word
 * starts a comment, and ; & | stand alone as separators. A `${{ ... }}` expression becomes one word. */
function words(line) {
  const s = line.replace(/\$\{\{[^}]*\}\}/g, (m) => m.replace(/\s+/g, ""));
  const out = [];
  let cur = null;
  let quote = null;
  const push = () => {
    if (cur !== null) out.push(cur);
    cur = null;
  };
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (quote) {
      if (c === quote) quote = null;
      else cur += c;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      if (cur === null) cur = "";
      continue;
    }
    if (c === "\\" && i + 1 < s.length) {
      cur = (cur === null ? "" : cur) + s[++i];
      continue;
    }
    if (/\s/.test(c)) {
      push();
      continue;
    }
    if (c === "#" && cur === null) break;
    if (c === ";" || c === "&" || c === "|") {
      push();
      out.push(c);
      continue;
    }
    cur = (cur === null ? "" : cur) + c;
  }
  push();
  return out;
}

/** Words that end one command or begin the next. */
const SEPARATORS = new Set([";", "&", "|", "if", "then", "else", "elif", "fi", "do", "done", "while", "until", "!", "{", "}"]);
/** The test operators a library may follow without being shipped (`-f "$SDKBIN/dxil.dll"`). */
const FILE_TESTS = new Set(["-f", "-e", "-s", "-r", "-x"]);

/** Every command in a workflow's text, as word lists, with its line number. Comment lines are skipped,
 * and a YAML key or list dash in front of a single-line `run:` is dropped. */
function commands(text) {
  const out = [];
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    let line = lines[i];
    const lineNo = i + 1;
    while (/\\\s*$/.test(line) && i + 1 < lines.length) line = line.replace(/\\\s*$/, " ") + lines[++i];
    if (/^\s*#/.test(line)) continue;
    let cmd = [];
    const flush = () => {
      while (cmd.length && (cmd[0] === "-" || /:$/.test(cmd[0]))) cmd.shift();
      if (cmd.length) out.push({ words: cmd, line: lineNo });
      cmd = [];
    };
    for (const w of words(line)) {
      if (SEPARATORS.has(w)) flush();
      else cmd.push(w);
    }
    flush();
  }
  return out;
}

/**
 * What a workflow's text puts into the download, read from its `cp` / `mv` commands: `shipped` holds
 * each file's path inside the download, and `unknown` each library mention (or recursive copy into
 * the download) this reading cannot account for.
 */
function workflowDownload(text) {
  const shipped = [];
  const unknown = [];
  for (const { words: w, line } of commands(text)) {
    if (w[0] === "cp" || w[0] === "mv") {
      const flags = w.slice(1).filter((a) => a.startsWith("-"));
      const args = w.slice(1).filter((a) => !a.startsWith("-"));
      if (args.length < 2) continue;
      const dest = args[args.length - 1].replace(/^\.\//, "");
      if (dest !== DOWNLOAD && !dest.startsWith(DOWNLOAD + "/")) continue;
      if (flags.some((f) => /^-[a-zA-Z]*[rR]/.test(f) || f === "--recursive")) {
        unknown.push({ line, what: `a recursive copy into ${DOWNLOAD} (this check cannot see what it ships)` });
        continue;
      }
      const sources = args.slice(0, -1);
      const intoFolder = dest === DOWNLOAD || dest.endsWith("/") || sources.length > 1;
      const rel = dest.slice(DOWNLOAD.length).replace(/^\/+/, "").replace(/\/+$/, "");
      for (const src of sources) {
        const name = intoFolder ? path.posix.basename(src) : path.posix.basename(rel);
        const folder = intoFolder ? rel : path.posix.dirname(rel) === "." ? "" : path.posix.dirname(rel);
        shipped.push({ path: folder ? `${folder}/${name}` : name, line });
      }
      continue;
    }
    w.forEach((word, i) => {
      if (MENTIONS_LIBRARY.test(word) && !FILE_TESTS.has(w[i - 1])) {
        unknown.push({ line, what: `"${word}" in a \`${w[0]}\` command (not a cp/mv this check can read)` });
      }
    });
  }
  return { shipped, unknown };
}

/** The libraries among `paths` (download paths) with no licence named for them in the same folder. */
function librariesWithoutLicence(paths) {
  const have = new Set(paths.map((p) => p.toLowerCase()));
  const out = [];
  for (const p of paths) {
    const name = path.posix.basename(p);
    if (!isLibrary(name)) continue;
    const folder = path.posix.dirname(p) === "." ? "" : path.posix.dirname(p);
    const wanted = licenceNamesFor(name).map((n) => (folder ? `${folder}/${n}` : n));
    if (!wanted.some((n) => have.has(n.toLowerCase()))) out.push(p);
  }
  return out;
}

// ---- the rule itself, on samples (so each way it must fail is seen failing) ----

const SAMPLE_HEAD = "      - name: Bundle\n        run: |\n          mkdir -p dist/HumanityOS/assets\n";

test("a library copied into the download without a licence is caught", () => {
  const { shipped, unknown } = workflowDownload(SAMPLE_HEAD + '          cp "$LIBDIR/foo.dll" dist/HumanityOS/\n');
  assert.deepStrictEqual(unknown, []);
  assert.deepStrictEqual(librariesWithoutLicence(shipped.map((s) => s.path)), ["foo.dll"]);
});

test("a library with its own licence beside it passes, in a subfolder too", () => {
  const text =
    SAMPLE_HEAD +
    '          cp "$LIBDIR/foo.dll" "$LIBDIR/bar.dll" dist/HumanityOS/bin/\n' +
    '          cp "$LIBDIR/LICENSE.TXT" dist/HumanityOS/bin/foo.LICENSE.txt\n' +
    "          cp upstream/NOTICE dist/HumanityOS/bin/bar.dll.LICENSE.md\n";
  const paths = workflowDownload(text).shipped.map((s) => s.path);
  assert.deepStrictEqual(paths, ["bin/foo.dll", "bin/bar.dll", "bin/foo.LICENSE.txt", "bin/bar.dll.LICENSE.md"]);
  assert.deepStrictEqual(librariesWithoutLicence(paths), []);
});

test("the project's own LICENSE, or a licence in another folder, does not count", () => {
  // The download's root holds LICENSE and LICENSES.md (ours, from the release list): neither is the
  // DLL's licence. And a licence for it in another folder is not beside it.
  const paths = ["LICENSE", "LICENSES.md", "foo.dll", "sub/foo.LICENSE.txt"];
  assert.deepStrictEqual(librariesWithoutLicence(paths), ["foo.dll"]);
  assert.deepStrictEqual(librariesWithoutLicence(["libfoo.so.1", "libbar.dylib"]), ["libfoo.so.1", "libbar.dylib"]);
});

test("a way in it cannot see fails as unknown; a file test of a library does not", () => {
  const text =
    SAMPLE_HEAD +
    '          if [[ -f "$SDK/foo.dll" ]]; then echo found; fi\n' +
    "          cp -r vendor/libs dist/HumanityOS/\n" +
    "          cd dist && 7z a -tzip out.zip HumanityOS/ extra.dll\n" +
    "          cp foo.dll somewhere/else/\n";
  const { shipped, unknown } = workflowDownload(text);
  assert.deepStrictEqual(shipped, []);
  assert.strictEqual(unknown.length, 2, JSON.stringify(unknown));
  assert.match(unknown[0].what, /recursive copy/);
  assert.match(unknown[1].what, /extra\.dll/);
});

// ---- the real download ----

/** The download's files as CI makes them: the workflow's copies plus the release list. */
function realDownload() {
  const text = fs.readFileSync(path.join(ROOT, WORKFLOW), "utf8");
  const fromWorkflow = workflowDownload(text);
  const paths = fromWorkflow.shipped.map((s) => s.path).concat(release.releaseFiles(ROOT));
  return { paths, unknown: fromWorkflow.unknown, workflowPaths: fromWorkflow.shipped.map((s) => s.path) };
}
const REAL = realDownload();

test("the workflow is read: it ships the program and every library mention is accounted for", () => {
  // Not vacuous: the reading must find the exe copy (`${{matrix.asset_name}}`), or it reads nothing.
  assert.ok(REAL.workflowPaths.some((p) => p.includes("matrix.asset_name")), REAL.workflowPaths.join(", "));
  assert.deepStrictEqual(
    REAL.unknown.map((u) => `${WORKFLOW}:${u.line}: ${u.what}`),
    [],
    "teach this check the new way a library reaches the download, then give the library its licence"
  );
});

test("the desktop download's libraries each carry a licence", () => {
  const missing = librariesWithoutLicence(REAL.paths).filter((p) => !PENDING.has(p));
  assert.deepStrictEqual(
    missing.map((p) => `${p}: no ${path.posix.basename(p).replace(/\.[^.]+$/, "")}.LICENSE.txt beside it in the download`),
    [],
    "ship each library's licence file in the same folder (see this file's header for the naming)"
  );
});

test("every exception is still needed, and names a finding that exists", () => {
  const missing = new Set(librariesWithoutLicence(REAL.paths));
  for (const [p, finding] of PENDING) {
    assert.ok(missing.has(p), `${p} is no longer shipped without a licence: delete its PENDING entry`);
    assert.ok(fs.existsSync(path.join(ROOT, finding)), `${p}'s PENDING entry names ${finding}, which does not exist`);
  }
});
