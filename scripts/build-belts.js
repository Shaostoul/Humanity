#!/usr/bin/env node
// Build data/maps/belts.json: the asteroid belts the Maps page draws as dust,
// from the REAL orbits of real minor planets (Maps increment 2,
// docs/design/maps-zoom-to-anything.md 5.10: "The belts are point clouds
// sampled from the real orbit distribution", not a random ring).
//
// Source: the Minor Planet Center's MPCORB.DAT, the orbits of every numbered
// and multi-opposition minor planet, updated daily. Its terms are recorded in
// docs/reference/findings/2026-10-21-mpc-orbit-data.md. In short, the header
// of MPCORB.DAT says software programs may include the data provided (1) the
// header is included (a separate file is acceptable), (2) proper attribution to
// the Minor Planet Center is given, and (3) information on how to get updated
// copies is included. So this script ALWAYS writes the header, byte for byte,
// to data/maps/MPCORB-header.txt beside the derived file, the derived file
// names its source and the update address, and every row keeps its Reference
// and orbit Computer columns (how the header says the people who computed each
// orbit get their credit). The app never contacts the MPC: this script is run
// by a developer, once, and its output is committed.
//
// What is kept: the brightest (lowest absolute magnitude H, so the largest)
// objects of each class, up to the counts below. The class comes from the
// MPC's own orbit-type flag (columns 162-165, low 6 bits; the format manual's
// values: 1-4 near-Earth, 6 Hungaria, 8 Hilda, 9 Jupiter Trojan, 10 distant
// object), with the main belt as every other orbit between 1.78 and 3.7 AU.
// The manual itself warns "the orbit classification is based on cuts in
// osculating element space and is not 100% reliable", which is fine for dust.
//
// Usage:
//   node scripts/build-belts.js                      # download (93.9 MB gzip) and build
//   node scripts/build-belts.js --input MPCORB.DAT.gz  # build from a copy you already have
//   node scripts/build-belts.js --input MPCORB.DAT     # (or the uncompressed file)
//   node scripts/build-belts.js --cache <file.gz>    # keep the download for next time
//
// Output (all relative to the repo root):
//   data/maps/belts.json        the derived orbits (about 1.7 MB)
//   data/maps/MPCORB-header.txt the MPCORB header, verbatim (a licence condition)

"use strict";
const fs = require("fs");
const path = require("path");
const https = require("https");
const zlib = require("zlib");
const readline = require("readline");

const ROOT = path.resolve(__dirname, "..");
const OUT_JSON = path.join(ROOT, "data", "maps", "belts.json");
const OUT_HEADER = path.join(ROOT, "data", "maps", "MPCORB-header.txt");
const URL = "https://minorplanetcenter.net/iau/MPCORB/MPCORB.DAT.gz";
// The update address the header itself gives (finding 2 of the findings doc).
const UPDATE_URL = "https://www.minorplanetcenter.org/iau/MPCORB/MPCORB.DAT";

// The classes, in draw order, and how many of each to keep. Data, not code
// paths: the page reads the class list from the output file. `tone` picks the
// dust colour the page paints a class in: "rock" (the theme's map_belt) or "ice"
// (map_belt_outer), since the Kuiper belt is icy and the inner belts rocky.
const CLASSES = [
  { id: "main", label: "Main belt", tone: "rock", keep: 13000, about: "between Mars and Jupiter, 1.8 to 3.7 AU" },
  { id: "hilda", label: "Hildas", tone: "rock", keep: 1500, about: "the 3:2 resonance with Jupiter, a triangle with Jupiter at one corner" },
  { id: "trojan", label: "Jupiter trojans", tone: "rock", keep: 2500, about: "60 degrees ahead of and behind Jupiter (its L4 and L5)" },
  { id: "distant", label: "Kuiper belt and beyond", tone: "ice", keep: 3000, about: "beyond Neptune: the Kuiper belt, the scattered disc and the centaurs" },
];

function opt(name, dflt) {
  const i = process.argv.indexOf(name);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : dflt;
}

// MPC packed date: century letter (I=18, J=19, K=20), 2-digit year, month and
// day as 1-9 then A=10 .. V=31. "K2669" = 2026 June 9.0 TT.
function unpackEpoch(s) {
  const cent = { I: 18, J: 19, K: 20 }[s[0]];
  const val = (c) => (c >= "1" && c <= "9" ? c.charCodeAt(0) - 48 : c.charCodeAt(0) - 55);
  if (cent === undefined) return null;
  const year = cent * 100 + parseInt(s.slice(1, 3), 10);
  return julianDay(year, val(s[3]), val(s[4]));
}

// Julian Day at 0h of a Gregorian date (Meeus, Astronomical Algorithms ch. 7).
function julianDay(y, m, d) {
  if (m <= 2) {
    y -= 1;
    m += 12;
  }
  const a = Math.floor(y / 100);
  const b = 2 - a + Math.floor(a / 4);
  return Math.floor(365.25 * (y + 4716)) + Math.floor(30.6001 * (m + 1)) + d + b - 1524.5;
}

function classify(type, a) {
  if (type >= 1 && type <= 4) return null; // near-Earth: not a belt
  if (type === 8) return "hilda";
  if (type === 9) return "trojan";
  if (type === 10) return "distant";
  if (a >= 1.78 && a <= 3.7) return "main";
  return null;
}

// Keep the `keep` lowest-H rows seen so far without holding 1.3 million lines:
// collect, and once twice the target, sort and cut back.
class Brightest {
  constructor(keep) {
    this.keep = keep;
    this.rows = [];
  }
  push(row) {
    this.rows.push(row);
    if (this.rows.length >= this.keep * 2) this.trim();
  }
  trim() {
    this.rows.sort((x, y) => x.H - y.H || (x.desig < y.desig ? -1 : 1));
    this.rows.length = Math.min(this.rows.length, this.keep);
  }
}

function round(x, places) {
  const k = 10 ** places;
  return Math.round(x * k) / k;
}

function openInput() {
  const input = opt("--input", null);
  if (input) {
    const raw = fs.createReadStream(input);
    return Promise.resolve(input.endsWith(".gz") ? raw.pipe(zlib.createGunzip()) : raw);
  }
  const cache = opt("--cache", null);
  return new Promise((resolve, reject) => {
    console.log(`Downloading ${URL} (about 94 MB) ...`);
    https
      .get(URL, { headers: { "User-Agent": "HumanityOS build-belts.js (https://github.com/Shaostoul/Humanity)" } }, (res) => {
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} from ${URL}`));
          return;
        }
        if (cache) res.pipe(fs.createWriteStream(cache));
        resolve(res.pipe(zlib.createGunzip()));
      })
      .on("error", reject);
  });
}

async function main() {
  const stream = await openInput();
  const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });
  const header = [];
  let inHeader = true;
  const pools = Object.fromEntries(CLASSES.map((c) => [c.id, new Brightest(c.keep)]));
  let seen = 0;
  let skipped = 0;
  for await (const line of rl) {
    if (inHeader) {
      header.push(line);
      // The header ends with the dashed rule under the column legend.
      if (/^-{20,}/.test(line)) inHeader = false;
      continue;
    }
    if (line.length < 165) continue; // blank section separators
    seen++;
    const H = parseFloat(line.slice(8, 13));
    const epochJd = unpackEpoch(line.slice(20, 25));
    const M = parseFloat(line.slice(26, 35));
    const peri = parseFloat(line.slice(37, 46));
    const node = parseFloat(line.slice(48, 57));
    const inc = parseFloat(line.slice(59, 68));
    const e = parseFloat(line.slice(70, 79));
    const a = parseFloat(line.slice(92, 103));
    const flags = parseInt(line.slice(161, 165), 16);
    if (![H, M, peri, node, inc, e, a, flags].every(Number.isFinite) || epochJd === null || e >= 1 || a <= 0) {
      skipped++;
      continue;
    }
    const cls = classify(flags & 0x3f, a);
    if (!cls) continue;
    pools[cls].push({
      desig: line.slice(0, 7).trim(),
      H,
      epochJd,
      a,
      e,
      inc,
      node,
      peri,
      M,
      ref: line.slice(107, 116).trim(),
      computer: line.slice(150, 160).trim(),
    });
  }
  if (inHeader || header.length < 10) throw new Error("no MPCORB header found: is this MPCORB.DAT?");

  const rows = [];
  const classes = [];
  CLASSES.forEach((c, ci) => {
    const pool = pools[c.id];
    pool.trim();
    classes.push({ id: c.id, label: c.label, about: c.about, tone: c.tone, count: pool.rows.length });
    for (const r of pool.rows) {
      rows.push([r.desig, ci, round(r.H, 2), r.epochJd, round(r.a, 6), round(r.e, 6), round(r.inc, 4), round(r.node, 4), round(r.peri, 4), round(r.M, 4), r.ref, r.computer]);
    }
  });

  const today = new Date().toISOString().slice(0, 10);
  const out = {
    format: "humanityos-belts-1",
    about: "Real minor-planet orbits the Maps page draws as the asteroid belts (Maps increment 2). Built by scripts/build-belts.js; do not edit by hand.",
    source: "MPCORB.DAT, the Minor Planet Center's orbit database (International Astronomical Union)",
    credit: `Asteroid orbits: Minor Planet Center, International Astronomical Union (MPCORB, downloaded ${today}). Updated copies: ${UPDATE_URL}`,
    terms: "Included under the MPCORB header's terms for software (data/maps/MPCORB-header.txt, shipped beside this file); see docs/reference/findings/2026-10-21-mpc-orbit-data.md.",
    header_file: "MPCORB-header.txt",
    update_url: UPDATE_URL,
    downloaded: today,
    selection: "The brightest (lowest H) objects of each class, by the MPC's own orbit-type flag; the main belt is every other orbit from 1.78 to 3.7 AU.",
    elements: "Osculating elements, heliocentric, J2000.0 ecliptic; angles in degrees, a in AU, epoch a Julian Date (TT).",
    classes,
    columns: ["designation", "class", "H", "epoch_jd", "a_au", "e", "i_deg", "node_deg", "peri_deg", "M_deg", "ref", "computer"],
    rows,
  };
  fs.mkdirSync(path.dirname(OUT_JSON), { recursive: true });
  // One row per line: diffable, and still plain JSON.
  const body = JSON.stringify({ ...out, rows: [] }, null, 2).replace(
    '"rows": []',
    '"rows": [\n' + rows.map((r) => "    " + JSON.stringify(r)).join(",\n") + "\n  ]"
  );
  fs.writeFileSync(OUT_JSON, body + "\n");
  fs.writeFileSync(OUT_HEADER, header.join("\n") + "\n");
  console.log(`Read ${seen} orbits (${skipped} unreadable). Kept: ${classes.map((c) => `${c.id} ${c.count}`).join(", ")}.`);
  console.log(`Wrote ${path.relative(ROOT, OUT_JSON)} (${fs.statSync(OUT_JSON).size} bytes) and ${path.relative(ROOT, OUT_HEADER)} (${header.length} lines).`);
}

main().catch((e) => {
  console.error(`build-belts: ${e.message}`);
  process.exit(1);
});
