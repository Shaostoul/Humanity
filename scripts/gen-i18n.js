#!/usr/bin/env node
/**
 * gen-i18n.js
 *
 * Writes web/shared/i18n-en.js from data/i18n/en.json, so the website has its
 * English words the moment a page loads (web/shared/i18n.js reads
 * window.HOS_I18N_EN) instead of after a fetch. data/i18n/en.json is the one
 * source: the desktop app reads it directly (src/i18n.rs), the website
 * through this generated copy. Never edit i18n-en.js by hand; edit en.json and
 * run this (scripts/tests/i18n-web.test.js fails while the two differ).
 *
 * Usage:
 *   node scripts/gen-i18n.js           # write web/shared/i18n-en.js
 *   node scripts/gen-i18n.js --check   # exit 1 when it is out of date
 */

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const EN_PATH = path.join(ROOT, 'data', 'i18n', 'en.json');
const OUT_PATH = path.join(ROOT, 'web', 'shared', 'i18n-en.js');

function build(enText) {
  const en = JSON.parse(enText);
  return (
    '// GENERATED from data/i18n/en.json by scripts/gen-i18n.js. Do not edit by hand:\n' +
    '// edit data/i18n/en.json and run `node scripts/gen-i18n.js`.\n' +
    '// The website\'s English words, here at once so a page can use i18n.t() while it\n' +
    '// builds (web/shared/i18n.js). Load it before i18n.js.\n' +
    'window.HOS_I18N_EN = ' + JSON.stringify(en, null, 2) + ';\n'
  );
}

function main() {
  const want = build(fs.readFileSync(EN_PATH, 'utf8'));
  if (process.argv.includes('--check')) {
    const have = fs.existsSync(OUT_PATH) ? fs.readFileSync(OUT_PATH, 'utf8').replace(/\r\n/g, '\n') : '';
    if (have !== want) {
      console.error('web/shared/i18n-en.js is out of date: run node scripts/gen-i18n.js');
      process.exit(1);
    }
    console.log('web/shared/i18n-en.js is current');
    return;
  }
  fs.writeFileSync(OUT_PATH, want);
  console.log('wrote ' + path.relative(ROOT, OUT_PATH));
}

if (require.main === module) main();
module.exports = { build, EN_PATH, OUT_PATH };
