// The browser half of `just verify-browser-webrtc` (2026-11-07): scripts/browser-webrtc.js, the
// headless Chromium driver, and scripts/browser-webrtc/, the page it opens. The check itself is
// src/gui/crossing_tests/webrtc_browser.rs (an #[ignore]d lib test, run by the recipe); these are the parts
// of it that need no browser, held here so they cannot drift.
//
// Run: node --test scripts/tests/browser-webrtc.test.js   (in `just rig-tests`)
//
// What it proves:
//  1. The browser's command line keeps the machine rules (CLAUDE.md): headless, the DevTools
//     protocol on a pipe and never a port, a fresh profile of the run's own, the fake microphone
//     and camera with the prompt answered by the fake UI, the sound out muted.
//  2. The browser chosen: HOS_BROWSER when it exists (and nothing when it does not), else Chrome,
//     else Edge, else nothing.
//  3. The pipe's framing: each NUL-ended message whole, in order, however the bytes are split.
//  4. A run's profile is named so `just clean-test-temp` sweeps it if a run is killed.
//  5. The page loads the website's scripts in web/chat/index.html's own order, and no stand-in
//     (stand-ins.js) declares a function one of those scripts declares: the website's code is
//     what runs, never a copy of it.
//
// Red first, 2026-11-07, each with the mutation named made to the file and seen failing:
//  1: "--headless=new" taken out of BROWSER_FLAGS: "headless, so no window ever shows"; a
//     "--remote-debugging-port=9222" added: "the DevTools protocol on a pipe, never a port".
//  2: findBrowser trying Edge before Chrome: "Chrome first" (expected chrome.exe, got msedge.exe).
//  3: frameReader dropping the bytes after a NUL in the same chunk: the split case threw
//     "SyntaxError: Unexpected token 'e', "ethod":"Ru"... is not valid JSON".
//  5: harness.html loading chat-voice-calls.js before chat-voice-rooms.js: "in index.html's
//     order"; a `function startCall() {}` added to stand-ins.js: "no stand-in replaces the
//     website's own startCall"; stand-ins.js's `let allUsersSnapshot = [];` (the first real run's
//     mistake, which kept chat-voice-rooms.js from loading at all): "no stand-in replaces the
//     website's own allUsersSnapshot (/chat/chat-voice-rooms.js)".

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const B = require('../browser-webrtc.js');
const ROOT = path.join(__dirname, '..', '..');
const HARNESS = path.join(ROOT, 'scripts', 'browser-webrtc');

test('the browser runs headless, on a pipe, on its own fresh profile, with fake devices', () => {
  const args = B.browserArgs('C:\\tmp\\hum_browser_webrtc_1_x');
  assert.ok(args.includes('--headless=new'), 'headless, so no window ever shows');
  assert.ok(args.includes('--remote-debugging-pipe'), 'the DevTools protocol on a pipe');
  assert.ok(!args.some((a) => a.startsWith('--remote-debugging-port') || a.startsWith('--remote-debugging-address')), 'the DevTools protocol on a pipe, never a port');
  assert.ok(args.includes('--user-data-dir=C:\\tmp\\hum_browser_webrtc_1_x'), "the run's own profile");
  assert.strictEqual(args.filter((a) => a.startsWith('--user-data-dir=')).length, 1, 'one profile');
  for (const f of ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--mute-audio', '--disable-background-networking', '--no-first-run']) {
    assert.ok(args.includes(f), `${f}: never a real microphone or camera, no sound out, no background traffic`);
  }
  assert.strictEqual(args[args.length - 1], 'about:blank', 'it opens nothing of its own');
});

test('HOS_BROWSER first, then Chrome, then Edge, else none', () => {
  const chrome = 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
  const edge = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
  const has = (...paths) => (p) => paths.includes(p);
  assert.strictEqual(B.findBrowser({}, has(chrome, edge), 'win32'), chrome, 'Chrome first');
  assert.strictEqual(B.findBrowser({}, has(edge), 'win32'), edge, 'Edge when there is no Chrome');
  assert.strictEqual(B.findBrowser({}, has(), 'win32'), null, 'none');
  assert.strictEqual(B.findBrowser({ HOS_BROWSER: 'D:\\b.exe' }, has('D:\\b.exe', chrome), 'win32'), 'D:\\b.exe', 'HOS_BROWSER wins');
  assert.strictEqual(B.findBrowser({ HOS_BROWSER: 'D:\\gone.exe' }, has(chrome), 'win32'), null, 'a HOS_BROWSER that is not there is not quietly swapped for another');
});

test('the pipe hands over each NUL-ended message whole, however it is split', () => {
  const got = [];
  const feed = B.frameReader((m) => got.push(m));
  const a = Buffer.from(JSON.stringify({ id: 1, result: { x: 'é' } }) + '\0');
  const b = Buffer.from(JSON.stringify({ method: 'Runtime.consoleAPICalled', params: {} }) + '\0');
  const both = Buffer.concat([a, b]);
  feed(both.subarray(0, 5));
  feed(both.subarray(5, a.length + 3));
  feed(both.subarray(a.length + 3));
  assert.deepStrictEqual(got, [{ id: 1, result: { x: 'é' } }, { method: 'Runtime.consoleAPICalled', params: {} }], 'split across chunks');
  got.length = 0;
  feed(both);
  assert.strictEqual(got.length, 2, 'both messages of one chunk');
});

test("a run's profile is one clean-test-temp sweeps", () => {
  assert.ok(B.PROFILE_PREFIX.startsWith('hum_'), 'clean-test-temp.js deletes hum_* entries left in the temp folder');
});

test('the arguments: --url needed, nothing unknown taken', () => {
  assert.deepStrictEqual(B.parseArgs(['--url', 'http://127.0.0.1:1/', '--downloads', 'd']), { url: 'http://127.0.0.1:1/', downloads: 'd', browser: null });
  assert.throws(() => B.parseArgs([]), /--url is needed/);
  assert.throws(() => B.parseArgs(['--url', 'x', '--port', '9222']), /unknown argument --port/);
});

/** The scripts a page loads, in order, without their ?v= cache tags. */
function scriptsOf(html) {
  return [...html.matchAll(/<script\s+src="([^"]+)"/g)].map((m) => m[1].split('?')[0]);
}

test("the page loads the website's own scripts in index.html's order, and no stand-in replaces one", () => {
  const page = scriptsOf(fs.readFileSync(path.join(HARNESS, 'harness.html'), 'utf8'));
  const site = scriptsOf(fs.readFileSync(path.join(ROOT, 'web', 'chat', 'index.html'), 'utf8'));
  const theirs = page.filter((s) => !s.startsWith('/harness/'));
  assert.ok(theirs.length >= 8, `the website's scripts: ${theirs}`);
  for (const s of theirs) assert.ok(site.includes(s), `${s} is one the chat page loads`);
  const order = theirs.map((s) => site.indexOf(s));
  assert.deepStrictEqual(order, [...order].sort((x, y) => x - y), `in index.html's order: ${theirs.join(', ')}`);

  // Top-level names: functions, and let / const / var (a second `let` of one name is a SyntaxError
  // that stops the whole later script from loading: seen on the first real run, 2026-11-07, when
  // stand-ins.js declared chat-voice-rooms.js's own `allUsersSnapshot`).
  const declared = (src) => new Set([...src.matchAll(/^(?:(?:async\s+)?function\s+|(?:let|const|var)\s+)([A-Za-z_$][\w$]*)/gm)].map((m) => m[1]));
  const standIns = declared(fs.readFileSync(path.join(HARNESS, 'stand-ins.js'), 'utf8'));
  assert.ok(standIns.has('handleMessage') && standIns.has('pqBuildDmPuts') && standIns.has('ws'), `the stand-ins read: ${[...standIns]}`);
  for (const s of theirs) {
    const own = declared(fs.readFileSync(path.join(ROOT, 'web', s.replace(/^\//, '')), 'utf8'));
    for (const name of standIns) assert.ok(!own.has(name), `no stand-in replaces the website's own ${name} (${s})`);
  }
});
