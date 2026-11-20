#!/usr/bin/env node
// Writes src/gui/widgets/emoji/props.rs, Unicode's emoji properties as Rust tables for the desktop's
// emoji matcher, from this Node's own Unicode data (the same \p{...} the website's matcher asks).
// Run it when Node moves to a newer Unicode (scripts/tests/emoji-props.test.js fails then).
//
//   node scripts/gen-emoji-props.js           write it
//   node scripts/gen-emoji-props.js --check   exit 1 when the committed file differs from this Node's
//
// Why a table: scripts/lib/emoji-props.js.

"use strict";

const fs = require("fs");
const path = require("path");
const props = require("./lib/emoji-props");

const out = path.join(__dirname, "..", props.RUST_FILE);
const text = props.rustText();

if (process.argv.includes("--check")) {
  const have = fs.existsSync(out) ? fs.readFileSync(out, "utf8").replace(/\r\n/g, "\n") : "";
  if (have !== text) {
    console.error(`${props.RUST_FILE} differs from Unicode ${process.versions.unicode}: run node scripts/gen-emoji-props.js`);
    process.exit(1);
  }
  console.log(`${props.RUST_FILE} is fresh (Unicode ${process.versions.unicode})`);
} else {
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, text);
  console.log(`wrote ${props.RUST_FILE} (Unicode ${process.versions.unicode})`);
}
