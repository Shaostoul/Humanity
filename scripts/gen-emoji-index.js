#!/usr/bin/env node
// Writes assets/emoji/index.json, the list of emoji pictures the website reads (web/shared/emoji.js),
// from assets/emoji/sets.json and the folders it names. Run it after adding, replacing or removing a
// picture (a volunteer's drawing in its set's folder, or a new OpenMoji release through
// scripts/vendor-openmoji.js, which runs this itself).
//
//   node scripts/gen-emoji-index.js           write it
//   node scripts/gen-emoji-index.js --check   exit 1 when the committed file is stale
//
// The layout and the rules: scripts/lib/emoji-pack.js. scripts/tests/emoji-web.test.js checks the
// committed index is fresh on every run.

"use strict";

const fs = require("fs");
const path = require("path");
const pack = require("./lib/emoji-pack");

const root = path.join(__dirname, "..");
const out = path.join(root, pack.INDEX_FILE);
const text = pack.indexText(root);

if (process.argv.includes("--check")) {
  const have = fs.existsSync(out) ? fs.readFileSync(out, "utf8").replace(/\r\n/g, "\n") : "";
  if (have !== text) {
    console.error(`${pack.INDEX_FILE} is stale: run node scripts/gen-emoji-index.js`);
    process.exit(1);
  }
  console.log(`${pack.INDEX_FILE} is fresh`);
} else {
  fs.writeFileSync(out, text);
  const idx = pack.buildIndex(root);
  console.log(`wrote ${pack.INDEX_FILE}: ` + idx.sets.map((s) => `${s.id} ${s.names.length}`).join(", "));
}
