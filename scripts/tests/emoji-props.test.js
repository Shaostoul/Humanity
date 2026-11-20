// The desktop's emoji properties agree with the website's engine (2026-11-20), run under Node:
//
//   node --test scripts/tests/emoji-props.test.js      (in `just rig-tests`)
//
// The desktop's emoji matcher (src/gui/widgets/emoji/mod.rs, docs/design/emoji.md rung 1) asks
// src/gui/widgets/emoji/props.rs which characters are emoji; the website's (web/shared/emoji.js)
// asks the JavaScript engine through \p{...}. props.rs is generated from the engine's own Unicode
// data (scripts/gen-emoji-props.js; why not the regex crate's tables: scripts/lib/emoji-props.js).
//
// What it proves:
//  1. With a Node of the same Unicode version as props.rs, the committed file is exactly what the
//     generator writes (fresh), so every property matches the engine's, code point for code point.
//  2. With an older Node (CI runs Node 20, Unicode 15.1), every character that Node gives a property
//     has it in props.rs too: emoji are only ever added, so a newer table holds the older one.
//  3. With a newer Node, it fails and says to regenerate: the website's engine then knows emoji the
//     desktop does not.
//
// Seen red 2026-11-20: test 1 with U+1F6D8 (Unicode 17.0's landslide) taken out of props.rs's
// Emoji_Presentation by hand: "src/gui/widgets/emoji/props.rs differs from Unicode 17.0: run node
// scripts/gen-emoji-props.js". Test 2 on the same broken file, its version told "18.0" so this Node
// counts as older: "Emoji_Presentation: U+1F6D8 is missing from props.rs".

"use strict";
const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const props = require("../lib/emoji-props");

const ROOT = path.join(__dirname, "..", "..");
// HOS_EMOJI_PROPS points it at another copy of props.rs (used to see it red).
const FILE = process.env.HOS_EMOJI_PROPS || path.join(ROOT, props.RUST_FILE);

test("props.rs holds this engine's emoji properties", () => {
  const text = fs.readFileSync(FILE, "utf8").replace(/\r\n/g, "\n");
  const table = props.parseRust(text);
  assert.ok(table.version, "props.rs says which Unicode version it was made from");
  for (const [prop] of props.PROPS) assert.ok(table.props[prop] && table.props[prop].length, `props.rs has ${prop}`);
  const mine = process.versions.unicode;
  const order = props.compareVersions(mine, table.version);
  assert.ok(order <= 0, `this Node has Unicode ${mine}, newer than props.rs's ${table.version}: run node scripts/gen-emoji-props.js`);
  if (order === 0) {
    assert.strictEqual(text, props.rustText(), `${props.RUST_FILE} differs from Unicode ${mine}: run node scripts/gen-emoji-props.js`);
    return;
  }
  // An older engine: props.rs must hold every character it knows (emoji are only ever added).
  for (const [prop] of props.PROPS) {
    const ranges = table.props[prop];
    const has = (cp) => ranges.some(([a, b]) => cp >= a && cp <= b);
    for (const [a, b] of props.rangesOf(prop)) {
      for (let cp = a; cp <= b; cp++) {
        assert.ok(has(cp), `${prop}: U+${cp.toString(16).toUpperCase().padStart(4, "0")} is missing from props.rs`);
      }
    }
  }
});
