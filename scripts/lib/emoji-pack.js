// THE EMOJI PACK: the emoji pictures HumanityOS draws in place of emoji characters, kept in our own
// tree (assets/emoji/) so no chat visitor's browser asks another site for one (2026-11-20).
//
// The operator chose OpenMoji (2026-11-20: "of that list the OpenMoji sounds better"). The research
// and what its licence (CC BY-SA 4.0) asks of us: docs/reference/findings/2026-11-20-openmoji.md.
// How each app draws them: docs/design/emoji.md.
//
// The layout:
//   assets/emoji/sets.json      which sets there are, in the order they are looked in: the first set
//                               holding an emoji draws it. Today one, OpenMoji; the volunteer-drawn
//                               HumanityOS set (docs/outreach/volunteers-wanted.md, "Emoji
//                               designers") goes AHEAD of it, so each design adopted replaces the
//                               open set's picture of that one emoji and nothing else.
//   assets/emoji/<dir>/*.svg    one picture per emoji, named by its code points in upper-case hex
//                               joined with "-" (OpenMoji's own naming: 1F600.svg,
//                               1F468-200D-1F469-200D-1F467.svg, 0023-FE0F-20E3.svg).
//   assets/emoji/index.json     GENERATED from the two above (scripts/gen-emoji-index.js): every
//                               set's picture names, so the website knows what it has without a
//                               request per guess. scripts/tests/emoji-web.test.js fails when it is
//                               stale.
//
// Everything here is plain node (fs, path), no packages.

"use strict";

const fs = require("fs");
const path = require("path");

const EMOJI_DIR = "assets/emoji";
const SETS_FILE = EMOJI_DIR + "/sets.json";
const INDEX_FILE = EMOJI_DIR + "/index.json";

/** A picture's name without ".svg": code points, upper-case hex, 4 to 6 digits each, joined by "-". */
const NAME_RE = /^[0-9A-F]{4,6}(?:-[0-9A-F]{4,6})*$/;

/** The SVG elements a picture may use. OpenMoji 17.0.0 uses the first eleven; the gradient and text
 *  ones are allowed for the volunteers' drawings. Anything else (script, foreignObject, image, use,
 *  a, style, animate...) is refused: an emoji is drawn with shapes, and a picture opened on its own
 *  (its address typed into a browser) must not be able to run or fetch anything. */
const ALLOWED_ELEMENTS = new Set([
  "svg", "g", "rect", "line", "polyline", "path", "circle", "polygon", "ellipse", "defs", "clipPath",
  "linearGradient", "radialGradient", "stop", "title", "desc",
]);

/** What is wrong with one picture's text, as sentences; empty when it is fine. */
function checkSvg(text) {
  const problems = [];
  if (!/^\s*<svg[\s>]/.test(text)) problems.push("does not start with <svg");
  if (/<!/.test(text)) problems.push("holds a <! declaration (DOCTYPE, ENTITY or CDATA)");
  if (/<\?/.test(text.replace(/^\s*<\?xml[^>]*\?>/, ""))) problems.push("holds a processing instruction");
  for (const m of text.matchAll(/<\s*([A-Za-z][\w:.-]*)/g)) {
    if (!ALLOWED_ELEMENTS.has(m[1])) problems.push(`uses the element <${m[1]}>`);
  }
  for (const m of text.matchAll(/\s(on[a-z]+)\s*=/gi)) problems.push(`has an event attribute ${m[1]}`);
  if (/\s(?:xlink:)?href\s*=/i.test(text)) problems.push("has an href (a link or a reference out)");
  if (/url\(\s*['"]?(?!#)/i.test(text)) problems.push("has a url() that points outside the picture");
  if (/javascript:/i.test(text)) problems.push("mentions javascript:");
  return [...new Set(problems)];
}

/** The key an emoji is looked up by: its picture name with every U+FE0F (the "draw as emoji"
 *  selector) taken out. OpenMoji names a single emoji without it (2764.svg for a red heart, which
 *  people type as 2764 FE0F) but keeps it inside sequences (2764-FE0F-200D-1F525.svg) and keycaps
 *  (0023-FE0F-20E3.svg), and people type both forms; taking it out of both sides matches them. */
function keyOfName(name) {
  return name.split("-").filter((cp) => cp !== "FE0F").join("-");
}

/** The sets, from assets/emoji/sets.json. */
function readSets(root) {
  const sets = JSON.parse(fs.readFileSync(path.join(root, SETS_FILE), "utf8")).sets;
  if (!Array.isArray(sets) || !sets.length) throw new Error(SETS_FILE + ": no sets");
  for (const s of sets) {
    for (const k of ["id", "dir", "name", "licence", "credit"]) {
      if (typeof s[k] !== "string" || !s[k]) throw new Error(`${SETS_FILE}: a set has no ${k}`);
    }
    if (!/^[a-z0-9-]+$/.test(s.dir)) throw new Error(`${SETS_FILE}: the folder "${s.dir}" is not a plain name`);
  }
  return sets;
}

/** One set's picture names (no ".svg"), sorted by code unit so the index never reorders itself. A
 *  file that is not a well-named .svg is an error, not something quietly left out. */
function setNames(root, set) {
  const dir = path.join(root, EMOJI_DIR, set.dir);
  if (!fs.existsSync(dir)) return [];
  const names = [];
  for (const f of fs.readdirSync(dir)) {
    if (!f.endsWith(".svg")) continue; // LICENSE.md, SOURCE.md
    const name = f.slice(0, -4);
    if (!NAME_RE.test(name)) throw new Error(`${EMOJI_DIR}/${set.dir}/${f}: not named by code points (like 1F600.svg)`);
    names.push(name);
  }
  return names.sort();
}

/** The index the website reads: each set, in order, with its picture names. */
function buildIndex(root) {
  return {
    sets: readSets(root).map((s) => ({ id: s.id, dir: s.dir, credit: s.credit, names: setNames(root, s) })),
  };
}

/** The index as the text of index.json. Every chat visitor's browser downloads it once per visit,
 *  so each set's names are ONE space-separated string (about 75 KB for OpenMoji's 4,495) rather than
 *  a JSON array one name per line (123 KB, measured 2026-11-20): the folder listing, not this file,
 *  is what a change reads as in git. */
function indexText(root) {
  const idx = buildIndex(root);
  const lines = [
    "{",
    '  "_comment": "GENERATED by scripts/gen-emoji-index.js from assets/emoji/sets.json and the folders it names. Do not edit: add or remove a picture, then run node scripts/gen-emoji-index.js. Read by web/shared/emoji.js. names: each picture file name without .svg, separated by spaces.",',
    '  "sets": [',
  ];
  idx.sets.forEach((s, i) => {
    const fields = [`"id": ${JSON.stringify(s.id)}`, `"dir": ${JSON.stringify(s.dir)}`, `"credit": ${JSON.stringify(s.credit)}`];
    lines.push(`    { ${fields.join(", ")},`);
    lines.push(`      "names": ${JSON.stringify(s.names.join(" "))} }${i < idx.sets.length - 1 ? "," : ""}`);
  });
  lines.push("  ]", "}", "");
  return lines.join("\n");
}

module.exports = { EMOJI_DIR, SETS_FILE, INDEX_FILE, NAME_RE, ALLOWED_ELEMENTS, checkSvg, keyOfName, readSets, setNames, buildIndex, indexText };
