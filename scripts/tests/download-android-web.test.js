// The download page's phone-app card (Android rung A2, docs/design/platforms.md, 2026-11-20): the
// APK's download and its install walk-through (android/README.md) only when this server's newest
// release carries HumanityOS-android-arm64.apk, and otherwise the plain words that the phone app is
// not published yet. The page learns of a release's files only from /releases/manifest.json
// (BUG-281: never GitHub's API).
//
// Run: node --test scripts/tests/download-android-web.test.js
//
// What it proves, each against the real page and its real script:
//  1. With no script (or before it answers) the page says "not published yet": the APK's parts are
//     hidden in the markup and the "not published" parts are not.
//  2. A release list WITHOUT the APK leaves that state.
//  3. A release list WITH the APK shows the APK's parts and the install steps, hides the
//     "not published" parts, points the link at that release's own file, and on an Android phone
//     moves the main button to the phone app in the card's own words.
//  4. A test-signed APK name (never published, android/build-apk.sh) does not count.
// Seen red 2026-11-20: with showAndroid's `apk` forced to null the third test failed ("the APK's
// link shows") and so did the fourth; with it forced to the first asset the second, third and fifth
// failed ("not published: #android-arch-none shows").

'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const HTML = fs.readFileSync(path.join(ROOT, 'web', 'pages', 'download.html'), 'utf8');
const SCRIPT = fs.readFileSync(path.join(ROOT, 'web', 'pages', 'download-app.js'), 'utf8');

const NONE_IDS = ['android-arch-none', 'android-detail-none', 'android-use-web'];
const APK_IDS = ['android-arch-apk', 'android-detail-apk', 'android-apk-link', 'android-steps-link', 'android-install'];

/** The element with `id` in the page's markup, as its opening tag. */
function openingTag(id) {
  const m = HTML.match(new RegExp(`<[a-z0-9]+[^>]*\\bid="${id}"[^>]*>`, 'i'));
  assert.ok(m, `the page has #${id}`);
  return m[0];
}

/** Run the page's script against a stub page whose elements start as the markup has them, with
 *  `manifest` as the server's release list, on a browser saying `ua`. Resolves to the elements. */
async function run(manifest, ua) {
  const els = new Map();
  const el = (id) => {
    if (!els.has(id)) {
      const tag = HTML.match(new RegExp(`<[a-z0-9]+[^>]*\\bid="${id}"[^>]*>([^<]*)`, 'i'));
      els.set(id, {
        id,
        hidden: tag ? /\shidden(\s|>|=)/.test(tag[0]) : false,
        textContent: tag ? tag[1] : '',
        innerHTML: '',
        href: tag && /href="([^"]*)"/.test(tag[0]) ? tag[0].match(/href="([^"]*)"/)[1] : '',
        style: {},
        classList: { add() {}, remove() {} },
      });
    }
    return els.get(id);
  };
  const ctx = {
    navigator: { userAgent: ua },
    window: {},
    document: { getElementById: el, querySelectorAll: () => [] },
    fetch: async (url) => {
      assert.strictEqual(url, '/releases/manifest.json', 'only this server is asked (BUG-281)');
      return { json: async () => manifest };
    },
    encodeURIComponent,
    console,
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(SCRIPT, ctx);
  for (let i = 0; i < 10; i++) await new Promise((r) => setImmediate(r));
  return el;
}

const release = (names) => ({ releases: [{ tag: 'v9.9.9', assets: names.map((name) => ({ name, size: 1 })) }] });
const DESKTOP = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)';
const PHONE = 'Mozilla/5.0 (Linux; Android 14; OnePlus) Mobile';

test('with no script the card says the phone app is not published yet', () => {
  for (const id of NONE_IDS) assert.ok(!/\shidden(\s|>|=)/.test(openingTag(id)), `#${id} shows by default`);
  for (const id of APK_IDS) assert.ok(/\shidden(\s|>|=)/.test(openingTag(id)), `#${id} is hidden by default`);
});

test('a release without the APK keeps "not published yet"', async () => {
  const el = await run(release(['HumanityOS-windows-x64.zip', 'HumanityOS-windows-x64.exe']), DESKTOP);
  for (const id of NONE_IDS) assert.strictEqual(el(id).hidden, false, `not published: #${id} shows`);
  for (const id of APK_IDS) assert.strictEqual(el(id).hidden, true, `not published: #${id} is hidden`);
});

test('a release with the APK shows its download and the install steps', async () => {
  const el = await run(release(['HumanityOS-windows-x64.zip', 'HumanityOS-android-arm64.apk']), DESKTOP);
  for (const id of APK_IDS) assert.strictEqual(el(id).hidden, false, `the APK's ${id === 'android-apk-link' ? 'link' : id} shows`);
  for (const id of NONE_IDS) assert.strictEqual(el(id).hidden, true, `#${id} is hidden`);
  assert.strictEqual(
    el('android-apk-link').href,
    'https://github.com/Shaostoul/Humanity/releases/download/v9.9.9/HumanityOS-android-arm64.apk',
    "the link is that release's own file"
  );
  // A desktop's main button stays the desktop's.
  assert.notStrictEqual(el('detected-download').href, el('android-apk-link').href);
});

test("on an Android phone the main button is the phone app's, in the card's words", async () => {
  const el = await run(release(['HumanityOS-android-arm64.apk']), PHONE);
  assert.strictEqual(el('detected-download').href, el('android-apk-link').href);
  assert.strictEqual(el('detected-download').textContent, el('android-apk-link').textContent);
  assert.strictEqual(el('detected-sub').textContent, el('android-get-sub').textContent);
  assert.ok(el('android-apk-link').textContent.length > 0, 'the words came from the page');
});

test('a test-signed APK name does not count as published', async () => {
  const el = await run(release(['HumanityOS-android-arm64-test-signed.apk']), PHONE);
  for (const id of APK_IDS) assert.strictEqual(el(id).hidden, true, `#${id} is hidden`);
  for (const id of NONE_IDS) assert.strictEqual(el(id).hidden, false, `#${id} shows`);
});
