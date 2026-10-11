#!/usr/bin/env node
// Writes the describe fixtures: tests/fixtures/describe/<file> for every file in
// scripts/lib/describe-corpus.js `fixed()`, and tests/fixtures/describe/expected.json with the
// web's own answer for each (`HosStrip.describe(bytes, nameOrType, {coordinates: true})`).
//
// Both apps' tests read them: the desktop's Rust describe (src/relay/core/describe_metadata.rs,
// its tests) must give these answers, and so must the web's (scripts/tests/describe-agree.test.js),
// so the attach step says the same words about the same file in both apps
// (docs/design/media-showcase.md, "Files on the sender's device and in messages, exact").
//
// Run it again only when the web's describe changes on purpose:  node scripts/make-describe-fixtures.js

'use strict';
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const S = require(path.join(ROOT, 'web', 'shared', 'strip-metadata.js'));
const { fixed } = require('./lib/describe-corpus.js');

const dir = path.join(ROOT, 'tests', 'fixtures', 'describe');
fs.mkdirSync(dir, { recursive: true });
const expected = {};
for (const [file, nameOrType, bytes] of fixed()) {
  fs.writeFileSync(path.join(dir, file), bytes);
  expected[file] = { nameOrType, answer: S.describe(bytes, nameOrType, { coordinates: true }) };
}
fs.writeFileSync(path.join(dir, 'expected.json'), JSON.stringify(expected, null, 1) + '\n');
console.log(`wrote ${Object.keys(expected).length} fixtures and their answers to ${path.relative(ROOT, dir)}`);
