#!/usr/bin/env node
// THIRD-PARTY-NOTICES.txt: the copyright and licence notices of other people's code that HumanityOS
// is built from, generated (2026-11-20). Never edit that file by hand: run this.
//
// WHY. The MIT, BSD and Apache-2.0 licences ask that their copyright and licence notices travel with
// the program (MIT: "The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software"; Apache-2.0 section 4(d) asks for the NOTICE file
// too), and until today nothing collected them for the Rust packages the programs link (about 600 a
// program) or the website's vendored JavaScript. The Digital Public Goods review asks for it as well
// (docs/outreach/dpg-application.md, indicators 3 and 4). The file ships beside the program in every
// download (scripts/lib/release-files.js) and the app shows it in Settings > Credits
// (src/gui/pages/settings/third_party.rs).
//
// WHICH PACKAGES COUNT (the rule, written down because it is a judgement):
//   - The programs a release ships, each from the build that makes it: the desktop app for every
//     target in .github/workflows/build-desktop.yml's matrix (read from that file, so a new platform
//     is counted the day it is added), built `--features native`; the phone app (aarch64-linux-
//     android, android/build-apk.sh, the default features); and the headless server
//     (x86_64-unknown-linux-gnu, `--features relay --no-default-features`, the workflow's relay job).
//   - For each, `cargo metadata --offline --filter-platform <target>` with that build's features, and
//     a walk from humanity-engine along NORMAL dependency edges only: a build-dependency (it runs while
//     compiling, its code is not in the program) and a dev-dependency (tests) are never followed, and
//     a dependency for another platform is already gone from the filtered graph.
//   - A proc-macro package is COUNTED (the code it writes is compiled into the program) but the walk
//     does not go past it: its own dependencies (syn, quote and the like) run inside the compiler and
//     are not in the program.
//   - cargo metadata resolves the whole workspace at once, so a feature another workspace member
//     turns on counts here too: the list can be a little larger than a program, never smaller.
//   - Our own workspace members are left out (they are HumanityOS, under LICENSE); a vendored path
//     dependency (vendor/rav1d) is counted like any other package.
//   - The union of every program's packages, each entry saying which programs carry it.
//
// THE TEXTS. For each package, every licence or notice file in its folder (the local cargo registry's
// copy, or vendor/ for a vendored one), at any depth: a crate that compiles a C library carries that
// library's licence in a subfolder. NOTICE files are included like the rest, verbatim. Texts are
// stored once and referred to by number: one Apache-2.0 text alone is carried word for word by 161
// packages, and printing every text in every entry that names it would make the file 6.9 MiB
// instead of 1.4 (measured 2026-11-20) for no added notice. The only
// changes to a text are line endings, spaces at the ends of lines and blank lines at its start and
// end. Where a package carries no licence file at its top level, the standard text of each licence
// its expression names (scripts/lib/licence-texts/<SPDX id>.txt) is used instead, and its entry says
// so and names the authors its Cargo.toml gives.
//
// WHY .txt AND NOT .md. Licence texts must read exactly as written: Markdown would turn their
// asterisks, underscores, angle brackets and indented blocks into formatting, so a viewer would show
// them changed. Plain text opens in every editor on every system, and THIRD-PARTY-NOTICES.txt is the
// name people look for beside a program.
//
// FRESHNESS. The file's header carries a fingerprint of its inputs (Cargo.lock's packages, the
// workspace's member list, this script, the standard texts, the vendored code's licence files and
// web/shared/vendor/licences.json) and a checksum of its body. scripts/tests/third-party-notices-
// fresh.test.js recomputes both without cargo (so it runs everywhere, CI included) and fails when
// either differs: Cargo.lock changed and nobody regenerated, or someone edited the file by hand.
// `--check` regenerates in memory and compares the whole file (needs cargo and the registry; it is in
// `just preflight`).
//
// NO DOWNLOADS. `cargo metadata --offline` reads only what the build already fetched; a package
// missing from the local registry is an error naming it, never a download.
//
// Usage:
//   node scripts/gen-third-party-notices.js            write THIRD-PARTY-NOTICES.txt
//   node scripts/gen-third-party-notices.js --check    exit 1 if the committed file differs
//   node scripts/gen-third-party-notices.js --report   licences per package, and the counts

"use strict";
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { execFileSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..");
const OUT = "THIRD-PARTY-NOTICES.txt";
const TEXTS_DIR = "scripts/lib/licence-texts";
const WEB_MANIFEST = "web/shared/vendor/licences.json";
const BAR = "=".repeat(80);
const TEXT_MARK = "-".repeat(80);
/** A text's first line: the dashes, a space and its number. */
const TEXT_MARK_RE = /^-{80} (\d+)$/;

/** A licence or notice file, by its name: the word anywhere in it (LICENSE-MIT, COPYING,
 * emoji-icon-font-mit-license.txt, NOTICE), or a font licence's usual name (OFL.txt, UFL.txt). */
const LICENCE_NAME = /(licen[cs]e|copying|notice|copyright|unlicense|patents)|^(OFL|UFL)([-_.].*)?$/i;
/** Names that match but are code or binary, never a notice to reproduce. */
const NOT_A_TEXT = /\.(rs|c|h|cc|cpp|hpp|py|js|mjs|ts|toml|json|ya?ml|sh|ps1|bat|html?|xml|in|s|asm|go|java|kt|swift|m|mm|ttf|otf|woff2?|png|jpe?g|gif|gz|zip|tar|bin)$/i;
/** A font file: a .txt or .md beside one is that font's licence, whatever its name (a font
 * carries its licence that way: epaint_default_fonts' Hack-Regular.txt is Hack's MIT notice and
 * the Bitstream Vera licence). */
const FONT_FILE = /\.(ttf|otf|woff2?)$/i;
/** How to tell, from its words, which licence a found file carries: used only for a package with no
 * licence file at its top level, to decide which standard texts its own files leave out. A licence
 * with no sign here is never counted as carried, so a missing text fails loudly instead. */
const LICENCE_SIGNS = {
  MIT: /Permission is hereby granted, free of charge, to any person obtaining a copy/i,
  "Apache-2.0": /Apache License,?\s+Version 2\.0/i,
  "BSL-1.0": /Boost Software License/i,
  "OFL-1.1": /SIL OPEN FONT LICENSE/i,
  "Ubuntu-font-1.0": /UBUNTU FONT LICEN[CS]E/i,
  "CC0-1.0": /CC0 1\.0 Universal/i,
  Zlib: /This software is provided 'as-is', without any express or implied/i,
  Unlicense: /This is free and unencumbered software released into the public domain/i,
  ISC: /Permission to use, copy, modify, and\/or distribute this software for any/i,
  "MPL-2.0": /Mozilla Public License,?\s+(Version|v\.)\s*2\.0/i,
};
/** Folders never searched: build output and version-control data. */
const SKIP_DIRS = new Set(["target", ".git", "node_modules"]);

// ---------------------------------------------------------------------------------------------------
// Inputs

const posix = (p) => p.replace(/\\/g, "/");

/** Line endings to \n, spaces off line ends, blank lines off both ends; a BOM dropped. */
function normaliseText(t) {
  return t
    .replace(/^﻿/, "")
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((l) => l.replace(/[ \t\f\v]+$/, ""))
    .join("\n")
    .replace(/^\n+/, "")
    .replace(/\n+$/, "");
}

/** A file's text: UTF-8 when it is valid UTF-8, else Latin-1 (a few old notices are). */
function readText(file) {
  const buf = fs.readFileSync(file);
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(buf);
  } catch (e) {
    return buf.toString("latin1");
  }
}

/** The programs a release ships (the comment at the top says where each comes from). */
function programs(root) {
  const wf = fs.readFileSync(path.join(root, ".github/workflows/build-desktop.yml"), "utf8").replace(/\r/g, "");
  const lines = wf.split("\n");
  const desktop = [];
  for (let i = 0; i < lines.length; i++) {
    const t = lines[i].match(/^\s*-?\s*target:\s*([a-z0-9_.-]+)\s*$/);
    if (!t) continue;
    for (let j = i + 1; j < Math.min(lines.length, i + 4); j++) {
      const l = lines[j].match(/^\s*label:\s*([a-z0-9_.-]+)\s*$/);
      if (l) {
        desktop.push({ label: l[1], target: t[1], features: ["--features", "native"] });
        break;
      }
    }
  }
  if (desktop.length === 0) throw new Error("no target/label pairs found in .github/workflows/build-desktop.yml's matrix");
  return [
    ...desktop,
    { label: "android-arm64", target: "aarch64-linux-android", features: [] },
    { label: "server-linux-x64", target: "x86_64-unknown-linux-gnu", features: ["--features", "relay", "--no-default-features"] },
  ];
}

function cargoMetadata(root, args) {
  try {
    const out = execFileSync("cargo", ["metadata", "--offline", "--format-version", "1", ...args], {
      cwd: root,
      maxBuffer: 1 << 30,
      stdio: ["ignore", "pipe", "pipe"],
    });
    return JSON.parse(out);
  } catch (e) {
    const err = e.stderr ? e.stderr.toString() : String(e);
    throw new Error(`cargo metadata --offline ${args.join(" ")} failed (a package the build has not fetched?):\n${err.slice(-2000)}`);
  }
}

/** The packages linked into one program (the walk the comment at the top describes). */
function linkedPackages(meta) {
  const pkgs = new Map(meta.packages.map((p) => [p.id, p]));
  const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
  const root = meta.packages.find((p) => p.name === "humanity-engine" && !p.source);
  if (!root) throw new Error("cargo metadata names no humanity-engine package");
  const members = new Set(meta.workspace_members);
  const seen = new Set();
  const stack = [root.id];
  while (stack.length) {
    const id = stack.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    const p = pkgs.get(id);
    if (id !== root.id && p.targets.some((t) => t.kind.includes("proc-macro"))) continue;
    for (const d of nodes.get(id).deps) {
      if (d.dep_kinds.some((k) => k.kind === null)) stack.push(d.pkg);
    }
  }
  return [...seen].filter((id) => !members.has(id)).map((id) => pkgs.get(id));
}

/** Every licence or notice file under `dir`, relative and POSIX, sorted; top-level ones flagged. */
function licenceFiles(dir, licenseFileField) {
  const out = [];
  const walk = (d, rel) => {
    let entries;
    try {
      entries = fs.readdirSync(d, { withFileTypes: true });
    } catch (e) {
      return;
    }
    const fonts = entries.some((e) => e.isFile() && FONT_FILE.test(e.name));
    for (const e of entries) {
      const r = rel ? `${rel}/${e.name}` : e.name;
      if (e.isDirectory()) {
        if (!SKIP_DIRS.has(e.name)) walk(path.join(d, e.name), r);
      } else if (e.isFile() && !NOT_A_TEXT.test(e.name) && (LICENCE_NAME.test(e.name) || (fonts && /\.(txt|md)$/i.test(e.name)))) {
        out.push(r);
      }
    }
  };
  walk(dir, "");
  if (licenseFileField) {
    const r = posix(path.relative(dir, path.resolve(dir, licenseFileField)));
    if (!out.includes(r) && fs.existsSync(path.join(dir, r))) out.push(r);
  }
  return out.sort();
}

// ---------------------------------------------------------------------------------------------------
// SPDX expressions (enough of the grammar for Cargo's licence field: AND, OR, WITH, brackets, and
// the old "/" that meant OR)

function parseSpdx(expr) {
  const tokens = expr.replace(/\//g, " OR ").match(/\(|\)|[^\s()]+/g) || [];
  let i = 0;
  const peek = () => tokens[i];
  const next = () => tokens[i++];
  function primary() {
    const t = next();
    if (t === "(") {
      const e = orExpr();
      if (next() !== ")") throw new Error(`unbalanced brackets in licence "${expr}"`);
      return e;
    }
    if (!t) throw new Error(`licence "${expr}" ends early`);
    const id = { id: t.replace(/\+$/, "") };
    if (peek() === "WITH") {
      next();
      id.exception = next();
    }
    return id;
  }
  function andExpr() {
    const parts = [primary()];
    while (peek() === "AND") {
      next();
      parts.push(primary());
    }
    return parts.length === 1 ? parts[0] : { and: parts };
  }
  function orExpr() {
    const parts = [andExpr()];
    while (peek() === "OR") {
      next();
      parts.push(andExpr());
    }
    return parts.length === 1 ? parts[0] : { or: parts };
  }
  const e = orExpr();
  if (i !== tokens.length) throw new Error(`could not read licence "${expr}"`);
  return e;
}

/** Every licence id an expression names. */
function spdxIds(e, out = new Set()) {
  if (e.id) out.add(e.id);
  for (const p of e.and || e.or || []) spdxIds(p, out);
  return out;
}

/** Is the expression satisfied when only the ids `held` says yes to may be chosen? */
function satisfiable(e, held) {
  if (e.id) return held(e.id);
  if (e.and) return e.and.every((p) => satisfiable(p, held));
  return e.or.some((p) => satisfiable(p, held));
}

// ---------------------------------------------------------------------------------------------------
// The document

function standardText(root, id) {
  const f = path.join(root, TEXTS_DIR, `${id}.txt`);
  return fs.existsSync(f) ? normaliseText(readText(f)) : null;
}

function repoOf(p) {
  return p.repository || p.homepage || (p.source && p.source.startsWith("registry+") ? `https://crates.io/crates/${p.name}` : "none given");
}

/** Everything the document says, worked out (no text yet). */
function collect(root) {
  const progs = programs(root);
  const byId = new Map();
  for (const prog of progs) {
    const meta = cargoMetadata(root, [...prog.features, "--filter-platform", prog.target]);
    for (const p of linkedPackages(meta)) {
      if (!byId.has(p.id)) byId.set(p.id, { pkg: p, programs: new Set() });
      byId.get(p.id).programs.add(prog.label);
    }
  }
  const entries = [...byId.values()].sort((a, b) => cmp(a.pkg.name, b.pkg.name) || cmp(a.pkg.version, b.pkg.version));
  const texts = new Map(); // normalised text -> { n, caption, users: [] }
  const order = [];
  const textId = (body, caption, user) => {
    if (!texts.has(body)) {
      const t = { n: order.length + 1, body, caption, users: [] };
      texts.set(body, t);
      order.push(t);
    }
    const t = texts.get(body);
    t.users.push(user);
    return t.n;
  };
  const rust = [];
  for (const { pkg, programs: inProgs } of entries) {
    const dir = path.dirname(pkg.manifest_path);
    const files = licenceFiles(dir, pkg.license_file);
    const who = `${pkg.name} ${pkg.version}`;
    const refs = [];
    const found = [];
    for (const rel of files) {
      const body = normaliseText(readText(path.join(dir, rel)));
      if (!body) continue;
      found.push(body);
      refs.push({ n: textId(body, `${rel}, from ${who}`, who), what: rel });
    }
    let note = null;
    if (!files.some((f) => !f.includes("/"))) {
      // No licence file at the top level: the package's own licence text is missing. Add the
      // standard text of each licence it names that none of its other files carries (a font's
      // licence in a fonts/ folder counts as carried).
      if (!pkg.license) throw new Error(`${who} names no licence and carries no licence file`);
      const e = parseSpdx(pkg.license);
      const carried = (id) => !!LICENCE_SIGNS[id] && found.some((b) => LICENCE_SIGNS[id].test(b));
      const held = (id) => standardText(root, id) !== null;
      if (!satisfiable(e, (id) => carried(id) || held(id))) {
        const missing = [...spdxIds(e)].filter((id) => !carried(id) && !held(id));
        throw new Error(`${who} (${pkg.license}) carries no licence file at its top level, and ${TEXTS_DIR}/ has no standard text for ${missing.join(", ")}: add it`);
      }
      const added = [];
      for (const id of satisfiable(e, carried) ? [] : [...spdxIds(e)].sort()) {
        if (carried(id)) continue;
        const body = standardText(root, id);
        if (!body) continue;
        added.push(id);
        refs.push({ n: textId(body, `the standard ${id} text`, who), what: `the standard ${id} text` });
      }
      const authors = (pkg.authors || []).join(", ");
      note =
        "The published package carries no licence file at its top level" +
        (added.length ? `, so the standard ${added.join(" and ")} text${added.length > 1 ? "s follow" : " follows"}` : "") +
        (authors ? `; its Cargo.toml names its authors: ${authors}.` : "; its Cargo.toml names no authors.");
    }
    rust.push({ name: pkg.name, version: pkg.version, license: pkg.license || `see ${pkg.license_file}`, repository: repoOf(pkg), programs: inProgs, refs, note });
  }
  const web = [];
  for (const lib of webLibraries(root)) {
    const refs = [];
    const who = `${lib.name} ${lib.version}`;
    if (lib.notice_lines) {
      const [a, b] = lib.notice_lines;
      const lines = readText(path.join(root, lib.files[0])).replace(/\r/g, "").split("\n").slice(a - 1, b);
      refs.push({ n: textId(normaliseText(lines.join("\n")), `${lib.files[0]} lines ${a} to ${b}, its own notice`, who), what: `${lib.files[0]} lines ${a} to ${b}` });
    }
    for (const rel of lib.license_files) {
      refs.push({ n: textId(normaliseText(readText(path.join(root, rel))), `${rel}, for ${who}`, who), what: rel });
    }
    let note = null;
    if (lib.license_files.length === 0) {
      const e = parseSpdx(lib.license);
      for (const id of [...spdxIds(e)].sort()) {
        const body = standardText(root, id);
        if (!body) throw new Error(`${who} has no licence file and ${TEXTS_DIR}/ has no ${id} text`);
        refs.push({ n: textId(body, `the standard ${id} text`, who), what: `the standard ${id} text` });
      }
      note = "The file carries its copyright notice and names its licence without its full text, so the standard text follows.";
    }
    web.push({ name: lib.name, version: lib.version, license: lib.license, repository: lib.repository, served: lib.files.join(", "), used_for: lib.used_for, refs, note });
  }
  return { progs, rust, web, texts: order };
}

function cmp(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

function webLibraries(root) {
  const m = JSON.parse(fs.readFileSync(path.join(root, WEB_MANIFEST), "utf8"));
  // The noble versions are the builder's pins: the manifest may not name others.
  const builder = fs.readFileSync(path.join(root, "scripts/build-noble-bundle.mjs"), "utf8");
  const pin = (name) => (builder.match(new RegExp(`const ${name}\\s*=\\s*'([^']+)'`)) || [])[1];
  const pins = { "@noble/post-quantum": pin("PQ_VER"), "@noble/hashes": pin("HASHES_VER") };
  for (const lib of m.libraries) {
    if (pins[lib.name] && pins[lib.name] !== lib.version) {
      throw new Error(`${WEB_MANIFEST} says ${lib.name} ${lib.version}, but scripts/build-noble-bundle.mjs pins ${pins[lib.name]}`);
    }
  }
  return m.libraries;
}

/** "57 LICENSE, aws-lc/LICENSE; 58 aws-lc/third_party/fiat/LICENSE": a package's texts by number,
 * the files that carry the same text named together. */
function textsPhrase(refs) {
  const byN = new Map();
  for (const r of refs) byN.set(r.n, [...(byN.get(r.n) || []), r.what]);
  return [...byN].map(([n, whats]) => `${n} ${whats.join(", ")}`).join("; ");
}

function programsPhrase(set, progs) {
  if (progs.every((p) => set.has(p.label))) return `every program (${progs.length})`;
  return progs.filter((p) => set.has(p.label)).map((p) => p.label).join(", ");
}

/** The whole document (header, fingerprint and checksum included). */
function render(root) {
  const { progs, rust, web, texts } = collect(root);
  const body = [];
  const heading = (title) => body.push(BAR, title, BAR, "");
  heading(`PART 1. RUST PACKAGES BUILT INTO THE PROGRAMS (${rust.length})`);
  body.push(
    "Which program each package is in, by the release download it comes in:",
    ...progs.map((p) => `    ${p.label}: ${p.target}${p.label.startsWith("server") ? ", the headless server" : p.label.startsWith("android") ? ", the phone app" : ", the desktop app"}`),
    "",
  );
  for (const e of rust) {
    body.push(`${e.name} ${e.version}`);
    body.push(`    Licence: ${e.license}`);
    body.push(`    Repository: ${e.repository}`);
    body.push(`    Built into: ${programsPhrase(e.programs, progs)}`);
    body.push(`    Texts: ${textsPhrase(e.refs)}`);
    if (e.note) body.push(`    Note: ${e.note}`);
    body.push("");
  }
  heading(`PART 2. WEB LIBRARIES THE WEBSITE SERVES (${web.length})`);
  for (const e of web) {
    body.push(`${e.name} ${e.version}`);
    body.push(`    Licence: ${e.license}`);
    body.push(`    Repository: ${e.repository}`);
    body.push(`    Served as: ${e.served}`);
    body.push(`    Used for: ${e.used_for}`);
    body.push(`    Texts: ${textsPhrase(e.refs)}`);
    if (e.note) body.push(`    Note: ${e.note}`);
    body.push("");
  }
  heading(`PART 3. THE LICENCE AND NOTICE TEXTS (${texts.length})`);
  for (const t of texts) {
    for (const line of t.body.split("\n")) {
      if (TEXT_MARK_RE.test(line)) throw new Error(`a licence text (${t.caption}) holds a line this file uses as a marker`);
    }
    const more = t.users.length > 1 ? ` (and ${t.users.length - 1} more package${t.users.length > 2 ? "s" : ""}; Parts 1 and 2 name them)` : "";
    body.push(`${TEXT_MARK} ${t.n}`, `Text ${t.n}: ${t.caption}${more}`, "", t.body, "");
  }
  const bodyText = body.join("\n").replace(/\n*$/, "");
  const header = [
    "THIRD-PARTY SOFTWARE NOTICES FOR HUMANITYOS",
    "",
    "HumanityOS is built from open-source software other people wrote and shared. Their licences ask",
    "that their copyright and licence notices travel with the program, and here they are: every Rust",
    "package built into the programs a release ships (Part 1), the code the website serves from other",
    "projects (Part 2), and the full text of every licence and notice file those packages carry (Part 3).",
    "",
    "HumanityOS's own licences are in LICENSE and LICENSING.md beside this file. Other people's data,",
    "pictures and sounds (maps, star catalogues, models, fonts) are credited in LICENSES.md and",
    "CREDITS.md, and in the app under Settings > Credits, which also shows this file.",
    "",
    "Generated by scripts/gen-third-party-notices.js from Cargo.lock and the local copies of each",
    "package; do not edit it by hand. Each package's licence files are reproduced as the package",
    "carries them (line endings and trailing spaces aside). A text many packages share is printed once",
    "and named by its number.",
    "",
    `Fingerprint: ${fingerprint(root)}`,
    `Body checksum: ${sha256(bodyText)}`,
    "",
  ].join("\n");
  return `${header}${bodyText}\n`;
}

// ---------------------------------------------------------------------------------------------------
// Freshness (no cargo needed: the test calls these)

function sha256(s) {
  return crypto.createHash("sha256").update(s).digest("hex");
}

/** Cargo.lock's packages, one canonical line each: the parts that change what the programs link. */
function lockLines(lockText, memberNames) {
  const out = [];
  for (const block of lockText.replace(/\r/g, "").split(/\n\[\[package\]\]\n/).slice(1)) {
    const field = (k) => (block.match(new RegExp(`^${k} = "([^"]*)"`, "m")) || [])[1] || "";
    const deps = (block.match(/^dependencies = \[\n([\s\S]*?)\n\]/m) || [])[1] || "";
    const name = field("name");
    // A workspace member's own version is the project's, bumped with every release: not an input.
    const version = memberNames.has(name) && !field("source") ? "(member)" : field("version");
    out.push(`${name} ${version} ${field("source")} ${field("checksum")} [${deps.replace(/[\s"]/g, "")}]`);
  }
  return out.sort();
}

/** The workspace's member package names, from the root Cargo.toml and each member's own. */
function memberNames(root) {
  const top = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8").replace(/\r/g, "");
  const list = (top.match(/^\[workspace\][\s\S]*?^members = \[([^\]]*)\]/m) || [])[1];
  if (!list) throw new Error("Cargo.toml's [workspace] has no members list");
  const names = new Set();
  for (const m of list.match(/"([^"]+)"/g).map((s) => s.slice(1, -1))) {
    const toml = fs.readFileSync(path.join(root, m, "Cargo.toml"), "utf8").replace(/\r/g, "");
    const pkg = toml.match(/^\[package\]\n(?:[^[].*\n)*?name = "([^"]+)"/m);
    if (pkg) names.add(pkg[1]);
  }
  return names;
}

/** The repository files the document is made from besides Cargo.lock, relative, sorted. */
function inputFiles(root) {
  const out = ["scripts/gen-third-party-notices.js", WEB_MANIFEST, "scripts/build-noble-bundle.mjs"];
  for (const f of fs.readdirSync(path.join(root, TEXTS_DIR))) out.push(`${TEXTS_DIR}/${f}`);
  const m = JSON.parse(fs.readFileSync(path.join(root, WEB_MANIFEST), "utf8"));
  for (const lib of m.libraries) {
    out.push(...lib.license_files);
    if (lib.notice_lines) out.push(lib.files[0]);
  }
  // A vendored path dependency's licence files (vendor/<crate>/...).
  const vendor = path.join(root, "vendor");
  if (fs.existsSync(vendor)) {
    for (const d of fs.readdirSync(vendor, { withFileTypes: true })) {
      if (d.isDirectory()) for (const f of licenceFiles(path.join(vendor, d.name))) out.push(`vendor/${d.name}/${f}`);
    }
  }
  return [...new Set(out)].sort();
}

/** The fingerprint of everything the document is made from (Cargo.lock's packages, the member
 * list, and inputFiles), line endings normalised so a Windows and a Linux checkout agree. */
function fingerprint(root, lockText = fs.readFileSync(path.join(root, "Cargo.lock"), "utf8")) {
  const names = memberNames(root);
  const parts = [`members ${[...names].sort().join(",")}`, ...lockLines(lockText, names).map((l) => `lock ${l}`)];
  for (const rel of inputFiles(root)) {
    parts.push(`file ${rel} ${sha256(readText(path.join(root, rel)).replace(/\r\n?/g, "\n"))}`);
  }
  return sha256(parts.join("\n"));
}

/** The committed document's own fingerprint and checksum lines, and whether its body matches. */
function readHeader(text) {
  const t = text.replace(/\r\n?/g, "\n");
  const fp = (t.match(/^Fingerprint: ([0-9a-f]{64})$/m) || [])[1] || null;
  const ck = t.match(/^Body checksum: ([0-9a-f]{64})\n/m);
  const body = ck ? t.slice(ck.index + ck[0].length).replace(/\n*$/, "") : null;
  return { fingerprint: fp, checksum: ck ? ck[1] : null, bodyMatches: !!ck && sha256(body) === ck[1] };
}

/** Every entry of Part 1 (the Rust packages, `part: 1`) and Part 2 (the web libraries, `part: 2`)
 * of a document: its title line split at the last space into name and version. */
function entriesOf(text) {
  const t = text.replace(/\r\n?/g, "\n");
  const at = (n) => t.indexOf(`\nPART ${n}.`);
  const out = [];
  for (const [part, from, to] of [[1, at(1), at(2)], [2, at(2), at(3)]]) {
    if (from < 0 || to < 0) continue;
    for (const m of t.slice(from, to).matchAll(/^(\S.*) (\S+)\n {4}Licence: /gm)) out.push({ part, name: m[1], version: m[2] });
  }
  return out;
}

module.exports = { OUT, fingerprint, lockLines, memberNames, readHeader, entriesOf, parseSpdx, satisfiable, normaliseText, render };

// ---------------------------------------------------------------------------------------------------

function main(argv) {
  if (argv.includes("--report")) {
    const { progs, rust, web, texts } = collect(ROOT);
    const byLicence = {};
    for (const e of rust) (byLicence[e.license] = byLicence[e.license] || []).push(`${e.name} ${e.version}`);
    for (const [l, ps] of Object.entries(byLicence).sort((a, b) => b[1].length - a[1].length)) console.log(`${String(ps.length).padStart(4)}  ${l}`);
    for (const p of progs) console.log(`${p.label}: ${rust.filter((e) => e.programs.has(p.label)).length} packages`);
    console.log(`${rust.length} Rust packages, ${web.length} web libraries, ${texts.length} distinct texts, ${rust.filter((e) => e.note).length} with no top-level licence file`);
    const size = (e) => e.refs.reduce((s, r) => s + texts[r.n - 1].body.length, 0);
    console.log("largest by text: " + [...rust].sort((a, b) => size(b) - size(a)).slice(0, 12).map((e) => `${e.name} ${e.version} ${e.refs.length} files ${(size(e) / 1024).toFixed(0)} KiB`).join("; "));
    const shared = [...texts].sort((a, b) => b.users.length - a.users.length).slice(0, 6);
    console.log("most shared: " + shared.map((t) => `text ${t.n} x${t.users.length} ${(t.body.length / 1024).toFixed(1)} KiB`).join("; "));
    return 0;
  }
  const doc = render(ROOT);
  const file = path.join(ROOT, OUT);
  if (argv.includes("--check")) {
    const have = fs.existsSync(file) ? fs.readFileSync(file, "utf8").replace(/\r\n?/g, "\n") : "";
    if (have === doc) {
      console.log(`${OUT} is current`);
      return 0;
    }
    console.error(`${OUT} is stale: run node scripts/gen-third-party-notices.js`);
    return 1;
  }
  fs.writeFileSync(file, doc);
  const h = readHeader(doc);
  console.log(`wrote ${OUT}: ${entriesOf(doc).length} entries, ${(doc.length / 1048576).toFixed(2)} MiB (fingerprint ${h.fingerprint.slice(0, 12)})`);
  return 0;
}

if (require.main === module) {
  try {
    process.exit(main(process.argv.slice(2)));
  } catch (e) {
    console.error(`gen-third-party-notices: ${e.message}`);
    process.exit(2);
  }
}
