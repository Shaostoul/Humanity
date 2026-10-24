// The website writes a message's time the way the desktop app does (2026-10-24): the same five
// formats, the same two zones (this computer's time by default, or UTC), the same words, and the
// same config strings, so one person's choice reads the same in both. The desktop side is
// src/gui/pages/chat/timestamps.rs; this runs the website's formatter (web/chat/view/timestampPill.js)
// in a vm under a fixed time zone and checks its Settings rows and defaults against the Rust source.
// Run: node --test scripts/tests/timestamps-web.test.js
//
// Seen red 2026-10-24 against the web/ before this: timestampPill.js had no formatter ("formatStamp
// is defined"), and the Settings page had neither select.
'use strict';

// A fixed zone with daylight saving, set before any Date is made, so "this computer's time" is
// the same on every machine that runs this (CI is in UTC, the operator's PC in Pacific time).
process.env.TZ = 'America/Los_Angeles';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const read = (rel) => fs.readFileSync(path.join(WEB, rel), 'utf8');

function formatter(stored) {
  const win = { localStorage: { getItem: (k) => (k === 'humanity_settings' && stored ? JSON.stringify(stored) : null) } };
  win.window = win;
  const ctx = vm.createContext(win);
  vm.runInContext(read('shared/defaults.js'), ctx);
  vm.runInContext(read('chat/view/timestampPill.js'), ctx);
  assert.strictEqual(typeof win.formatStamp, 'function', 'formatStamp is defined');
  return win;
}

// 2026-10-24 19:48:24.000 UTC (the Rust test's own moment): 12:48:24 in California, summer time.
const SUMMER = Date.UTC(2026, 9, 24, 19, 48, 24);
// 2026-01-15 09:05:07.042 UTC: 01:05:07 in California, winter time.
const WINTER = Date.UTC(2026, 0, 15, 9, 5, 7, 42);

test('in UTC the website writes exactly what the desktop app writes', () => {
  const w = formatter();
  // The same strings src/gui/pages/chat/timestamps.rs's test asserts for SUMMER in UTC.
  assert.strictEqual(w.formatStamp(SUMMER, 'date_hour_min_sec', 'utc'), '2026-10-24 19:48:24 UTC');
  assert.strictEqual(w.formatStamp(SUMMER, 'hour_min', 'utc'), '19:48 UTC');
  assert.strictEqual(w.formatFullStamp(SUMMER, 'utc'), '2026-10-24 19:48:24 UTC');
  assert.strictEqual(w.formatStamp(WINTER, 'hour_min_sec', 'utc'), '09:05:07 UTC');
  assert.strictEqual(w.formatStamp(WINTER, 'date_hour_min', 'utc'), '2026-01-15 09:05 UTC');
  assert.strictEqual(w.formatStamp(WINTER, 'full', 'utc'), '2026-01-15 09:05:07.042 UTC');
});

test("this computer's time is written bare, and the full form names its offset, daylight saving included", () => {
  const w = formatter();
  assert.strictEqual(w.formatStamp(SUMMER, 'hour_min', 'local'), '12:48');
  assert.strictEqual(w.formatStamp(SUMMER, 'date_hour_min_sec', 'local'), '2026-10-24 12:48:24');
  assert.strictEqual(w.formatFullStamp(SUMMER, 'local'), '2026-10-24 12:48:24 (UTC-07:00)');
  assert.strictEqual(w.formatStamp(WINTER, 'hour_min', 'local'), '01:05');
  assert.strictEqual(w.formatFullStamp(WINTER, 'local'), '2026-01-15 01:05:07 (UTC-08:00)');
});

test('the choice is read from the settings, and this computer\'s time is the default', () => {
  assert.deepStrictEqual({ ...formatter().timestampSettings() }, { format: 'hour_min', zone: 'local' });
  assert.deepStrictEqual({ ...formatter({ timestamp_format: 'full', timestamp_zone: 'utc' }).timestampSettings() }, { format: 'full', zone: 'utc' });
});

test("the Settings rows offer the desktop app's choices under its config strings and words", () => {
  const rust = fs.readFileSync(path.join(ROOT, 'src', 'gui', 'pages', 'chat', 'timestamps.rs'), 'utf8');
  const strings = (enumName) => {
    const block = rust.slice(rust.indexOf('impl ' + enumName + ' {'));
    const asStr = block.slice(0, block.indexOf('pub fn from_config_str'));
    return [...asStr.matchAll(/=> "([a-z_]+)",/g)].map((m) => m[1]);
  };
  const formats = strings('TimestampFormat');
  const zones = strings('TimestampZone');
  assert.deepStrictEqual(formats, ['hour_min', 'hour_min_sec', 'date_hour_min', 'date_hour_min_sec', 'full']);
  assert.deepStrictEqual(zones, ['local', 'utc']);
  const html = read('pages/settings.html');
  const options = (id) => {
    const at = html.indexOf('id="' + id + '"');
    assert.ok(at > 0, id + ' is on the Settings page');
    const sel = html.slice(at, html.indexOf('</select>', at));
    return [...sel.matchAll(/<option value="([a-z_]+)" data-i18n="([a-z_.]+)"/g)].map((m) => [m[1], m[2]]);
  };
  assert.deepStrictEqual(options('pref-timestamp_format'), formats.map((f) => [f, 'settings.timestamp_format_' + f]));
  assert.deepStrictEqual(options('pref-timestamp_zone'), zones.map((z) => [z, 'settings.timestamp_zone_' + z]));
  // The desktop's labels come from the same keys.
  for (const f of formats) assert.ok(rust.includes('t("settings.timestamp_format_' + f + '")'), 'the desktop names ' + f + ' from the word source');
  // And the website's default is the desktop's default outside tests.
  const defaults = read('shared/defaults.js');
  assert.match(defaults, /timestamp_format: 'hour_min',/);
  assert.match(defaults, /timestamp_zone: 'local',/);
  assert.match(rust, /pub const DEFAULT: TimestampZone = if cfg!\(test\) \{ TimestampZone::Utc \} else \{ TimestampZone::Local \};/);
});

test('the pill and its expanded form use this formatter', () => {
  const app = read('chat/app.js');
  assert.match(app, /function formatTimePill\(ts\) \{\s*\n\s*const s = timestampSettings\(\);\s*\n\s*return formatStamp\(ts, s\.format, s\.zone\)\.replace\(\/ UTC\$\/, ''\);/);
  assert.match(read('chat/view/pillPopup.js'), /return window\.formatFullStamp\(ms, window\.timestampSettings\(\)\.zone\);/);
});
