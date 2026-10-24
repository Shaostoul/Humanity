// The admin action registry on the website (2026-10-22, docs/design/in-app-ops.md): the shared
// rules in web/shared/admin-actions.js and the admin page's wiring.
//
// Run: node --test scripts/tests/admin-actions-web.test.js   (in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/ (used to see each test red).
//
// What it proves:
//  1. The words a run signs are the relay's: the registry's own signing example
//     (data/admin/actions.json `signing.example`, which src/relay/admin/tests.rs holds the relay
//     to) comes out of adminRunPurpose exactly, so a browser's signature is one the relay checks.
//  2. A chat command is filled from the inputs as the relay and the desktop app fill it, and a
//     destructive action's confirm word is exact.
//  3. A form becomes the inputs the relay takes (numbers as numbers, an unticked box false, a
//     required box refused when empty), read against the real registry entries.
//  4. A masked secret and a peer's state read as words, the desktop's words.
//  5. The admin page loads the pieces in order, the console writes the server's answers as text
//     and never as markup, and its key check is the relay's key length (the old panel asked for
//     64 hex characters and refused every Dilithium3 server key).
//
// Red first, 2026-10-22, each mutation in a copy of web/ run with HOS_WEB_DIR and seen failing
// with the assertion named, then passing again on the real web/:
//  1: admin-actions.js adminRunPurpose joining with ':' for '\n': "the browser signs the relay's words".
//  3: adminInputsFromForm sending the number as text: "a number is sent as a number".
//  5: admin-console.js setting box.innerHTML for the alert list: "the console never writes markup".

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, '..', '..', 'web');
const ROOT = path.join(__dirname, '..', '..');
const A = require(path.join(WEB, 'shared', 'admin-actions.js'));
const registry = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'admin', 'actions.json'), 'utf8'));
const find = (id) => registry.actions.find((a) => a.id === id);

test('the browser signs the relay\'s words', () => {
  const ex = registry.signing.example;
  assert.strictEqual(A.adminRunPurpose(ex.action, ex.inputs), ex.purpose, "the browser signs the relay's words");
  assert.notStrictEqual(A.adminRunPurpose('alerts.list', '{ }'), ex.purpose, 'the exact inputs text is what is hashed');
});

test('a chat command is filled from the inputs, and a confirm word is exact', () => {
  const add = find('federation.add');
  assert.strictEqual(A.adminChatText(add, { url: 'https://a.example' }), '/server-add https://a.example');
  assert.strictEqual(A.adminChatText(add, { url: 'https://a.example', name: 'Alpha' }), '/server-add https://a.example Alpha');
  const wipe = find('maintenance.wipe_all');
  assert.ok(A.adminNeedsConfirm(wipe));
  assert.ok(!A.adminConfirmOk(wipe, 'wipe all'), 'the word is exact');
  assert.ok(A.adminConfirmOk(wipe, ' WIPE ALL '), 'trimmed');
  assert.ok(A.adminConfirmOk(find('federation.remove'), ''), 'nothing is asked of an action that can be undone');
});

test('a form becomes the inputs the relay takes', () => {
  const reorder = find('channels.reorder');
  assert.deepStrictEqual(A.adminInputsFromForm(reorder, { name: ' general ', position: '3' }), { ok: true, inputs: { name: 'general', position: 3 } }, 'a number is sent as a number');
  assert.strictEqual(A.adminInputsFromForm(reorder, { name: 'general', position: 'three' }).ok, false);
  assert.strictEqual(A.adminInputsFromForm(reorder, { position: '3' }).ok, false, 'a required box refused when empty');
  const chan = find('federation.channel');
  assert.deepStrictEqual(A.adminInputsFromForm(chan, { channel: 'general', federated: false }).inputs, { channel: 'general', federated: false }, 'an unticked box is false');
});

test('a masked secret and a peer\'s state read as words', () => {
  assert.strictEqual(A.adminMaskedWords({ set: true, host: 'discord.com', ends: 'ab12' }), 'discord.com, ends ab12');
  assert.strictEqual(A.adminMaskedWords({ set: false }), 'not set');
  assert.strictEqual(A.adminMaskedWords({ set: true, host: null, ends: '9z' }), 'ends 9z');
  assert.strictEqual(A.adminPeerState({ paused: true, linked: true }), 'paused by an admin');
  // The link's automatic hold-back reads as that, never as the admin's pause (the desktop's words).
  assert.strictEqual(
    A.adminPeerLink({ link: { flow: { state: 'paused', pauses: 2, missed: 3, set_aside: 0 }, cursor: { log: 'x', seq: 154 } } }),
    'Link: held back automatically for sending too fast; held back 2 times; 3 messages missed; read their log to message 154.'
  );
  assert.strictEqual(A.adminPeerLink({ link: null }), null, 'no link state, no line');
  assert.strictEqual(A.adminPeerState({ paused: false, linked: true, linked_secs: 7200 }), 'linked for 2 h');
  assert.strictEqual(
    A.adminPeerState({ paused: false, linked: false, status: 'unreachable', trust_tier: 0 }),
    'not linked, unreachable (trust below 2: not federating)'
  );
  // last_seen is the time of the last status change unless a link is up: "since", not "last heard".
  assert.strictEqual(
    A.adminPeerState({ paused: false, linked: false, status: 'unreachable', trust_tier: 2, last_seen: Date.now() - 125000 }),
    'not linked, unreachable since 2 min ago'
  );
});

test('the admin page loads the pieces in order and the console writes only text', () => {
  const html = fs.readFileSync(path.join(WEB, 'pages', 'admin.html'), 'utf8');
  const at = (s) => html.indexOf(s);
  for (const s of ['/shared/pq-relay-auth.js', '/shared/blake3.js', '/shared/admin-actions.js', '/pages/admin-console.js', '/pages/admin-app.js']) {
    assert.ok(at(s) > 0, 'the page loads ' + s);
  }
  assert.ok(at('/shared/blake3.js') < at('/shared/admin-actions.js') && at('/shared/admin-actions.js') < at('/pages/admin-console.js') && at('/pages/admin-console.js') < at('/pages/admin-app.js'), 'in order');
  for (const id of ['alerts-panel', 'federation-panel', 'actions-panel', 'adminlog-panel', 'fed-channels', 'fed-own-key']) {
    assert.ok(html.includes('id="' + id + '"'), 'the page has #' + id);
  }
  const consoleJs = fs.readFileSync(path.join(WEB, 'pages', 'admin-console.js'), 'utf8');
  assert.ok(!/\.innerHTML\s*=|insertAdjacentHTML|outerHTML\s*=/.test(consoleJs), 'the console never writes markup');
  const fed = fs.readFileSync(path.join(ROOT, 'src', 'relay', 'handlers', 'federation.rs'), 'utf8');
  const len = /SERVER_KEY_HEX_LEN: usize = (\d+);/.exec(fed)[1];
  assert.ok(consoleJs.includes('{' + len + '}'), 'the key check is the relay\'s key length (' + len + ')');
});
