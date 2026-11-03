// Per-person controls on the website (web/shared/person-controls.js), held to the cases the
// desktop app's twin answers too (src/net/person_controls.rs, its lib test reads the same file):
// scripts/tests/fixtures/person-controls-cases.json. Also the record kept in this browser, the
// notice level THE rule is handed, the lines a file shows, and the profile card's section and
// the files rule wired into the chat (by reading the chat's own source).
// Run: node --test scripts/tests/person-controls-web.test.js
//
// Seen red 2026-11-03: with the protected setup's fallback taken out of fileVerdict ("up to 10 MB
// under the protected setup: within the limit my setting decides" answered accept); with set()
// keeping the defaults as an entry ("back to the defaults leaves nothing kept"); with the
// section's call left out of showViewProfileCard ("the profile card draws Your settings for").

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const repo = path.resolve(__dirname, '..', '..');
const P = require(path.join(repo, 'web/shared/person-controls.js'));
const fx = JSON.parse(fs.readFileSync(path.join(repo, 'scripts/tests/fixtures/person-controls-cases.json'), 'utf8'));
const en = JSON.parse(fs.readFileSync(path.join(repo, 'data/i18n/en.json'), 'utf8'));

/** en.json's words for `key` with {{vars}} filled, as i18n.t does. */
function t(key, vars) {
  const words = key.split('.').reduce((o, k) => (o ? o[k] : undefined), en);
  assert.ok(typeof words === 'string', `${key} is in en.json`);
  return words.replace(/\{\{(\w+)\}\}/g, (_, v) => String(vars && vars[v] !== undefined ? vars[v] : ''));
}

/** A localStorage stand-in. */
function storage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), m };
}

test('every shared file case gets the same answer as on the desktop', () => {
  assert.ok(fx.files.length >= 12, `the shared cases shrank to ${fx.files.length}`);
  for (const c of fx.files) {
    const got = P.fileVerdict(c.controls, c.size, c.protected);
    assert.strictEqual(got.id, c.expect, c.name);
    if (c.mb !== undefined) assert.strictEqual(got.mb, c.mb, `${c.name}: the limit it is over`);
  }
});

test('every shared record reads as its defaults where it says nothing usable', () => {
  for (const c of fx.records) {
    const got = P.controlsFrom(c.raw);
    assert.strictEqual(got.notices, c.expect.notices, `${c.name}: notices`);
    assert.strictEqual(got.files, c.expect.files, `${c.name}: files`);
    assert.strictEqual(got.files_mb, c.expect.files_mb, `${c.name}: files_mb`);
    assert.strictEqual(P.isDefault(got), c.expect.default, `${c.name}: default`);
  }
});

test('the record keeps only what differs, by identity', () => {
  const s = storage();
  assert.ok(P.set(s, 'ME', 'Ab12', { notices: 'nothing' }));
  assert.strictEqual(P.get(s, 'me', 'ab12').notices, 'nothing', 'read back in any case');
  assert.strictEqual(P.get(s, 'other', 'ab12').notices, 'all', 'another identity has its own');
  assert.strictEqual(P.levelOf(s, 'me', 'ab12'), 'nothing');
  assert.strictEqual(P.levelOf(s, 'me', 'cd34'), null, 'nothing chosen: no level');
  assert.strictEqual(P.levelOf(s, 'me', ''), null, 'no sender: no level');
  P.set(s, 'me', 'ab12', { notices: 'all', files: 'mine' });
  assert.deepStrictEqual(JSON.parse(s.getItem(P.KEY)), {}, 'back to the defaults leaves nothing kept');
  assert.strictEqual(P.set(s, 'me', 'me', { files: 'never' }), false, 'never for myself');
  assert.strictEqual(P.set(s, '', 'ab12', { files: 'never' }), false, 'never without my key');
  P.set(s, 'me', 'ab12', { files: 'up_to', files_mb: 50 });
  assert.strictEqual(P.mb(P.get(s, 'me', 'ab12')), 50);
  P.set(s, 'me', 'ab12', { files: 'mine', files_mb: 50 });
  assert.deepStrictEqual(JSON.parse(s.getItem(P.KEY)), {}, 'my setting again with Everything is the defaults');
  const broken = { getItem: () => '{not json', setItem: () => { throw new Error('full'); } };
  assert.strictEqual(P.get(broken, 'me', 'ab12').files, 'mine', 'a damaged record reads as the defaults');
  assert.strictEqual(P.set(broken, 'me', 'ab12', { files: 'never' }), false, 'a browser that keeps nothing says so');
});

test('each asking verdict has its line, as the desktop says it', () => {
  assert.strictEqual(P.verdictLine({ id: 'ask_always' }, 'Ann', t), 'You chose to be asked about every file from Ann. Accept this one?');
  assert.strictEqual(P.verdictLine({ id: 'ask_over', mb: 10 }, 'Ann', t), 'This file is over the 10 MB you take from Ann without asking. Accept this one?');
  assert.strictEqual(P.verdictLine({ id: 'never' }, 'Ann', t), 'You chose not to take files from Ann.');
  assert.strictEqual(P.verdictLine({ id: 'mine' }, 'Ann', t), null);
  assert.strictEqual(P.verdictLine({ id: 'accept' }, 'Ann', t), null);
});

test('the chat loads the module before the scripts that ask it', () => {
  const html = fs.readFileSync(path.join(repo, 'web/chat/index.html'), 'utf8');
  const at = (src) => html.indexOf(src);
  assert.ok(at('/shared/person-controls.js') > 0, 'index.html loads person-controls.js');
  for (const later of ['/chat/chat-dms.js', '/shared/notify-rules.js', '/chat/chat-notify.js', '/chat/chat-profile.js', '/chat/chat-file-transfer.js']) {
    assert.ok(at('/shared/person-controls.js') < at(later), `before ${later}`);
  }
});

test('the profile card draws Your settings for the person, by key, with hints', () => {
  const src = fs.readFileSync(path.join(repo, 'web/chat/chat-profile.js'), 'utf8');
  assert.ok(/personControlsHtml\(/.test(src), 'the section is built');
  const call = src.indexOf('personControlsHtml(name, publicKey)');
  assert.ok(call > 0 && call < src.indexOf("document.getElementById('view-profile-content').innerHTML = html"), 'showViewProfileCard draws it into the card');
  for (const key of ['person.heading', 'person.notices', 'person.quiet_allow', 'person.files', 'person.kept_here']) {
    assert.ok(src.includes(`'${key}'`), `${key} is asked for`);
  }
  for (const hint of ['person.notices_hint', 'person.quiet_allow_hint', 'person.files_hint', 'person.files_mb_hint']) {
    assert.ok(src.includes(`'${hint}'`), `${hint} is a control's hint`);
  }
});
