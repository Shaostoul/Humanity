// THE notification rule on the website (web/shared/notify-rules.js), held to the cases the
// desktop app's twin answers too (src/notifications/decide.rs, its lib test reads the same file):
// scripts/tests/fixtures/notify-cases.json, against data/notifications/notifications.json.
// Run: node --test scripts/tests/notify-rules-web.test.js
//
// Seen red 2026-10-30: with the server-mute check moved after quiet hours, "a muted server during
// quiet hours reports the mute" failed (quiet_hours); with isMention's boundary check removed,
// "@Benjamin hi" mentioned Ben; with words() given the text for 'name', the default notice carried
// the message.

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const repo = path.resolve(__dirname, '..', '..');
const rules = require(path.join(repo, 'web/shared/notify-rules.js'));
const file = JSON.parse(fs.readFileSync(path.join(repo, 'data/notifications/notifications.json'), 'utf8'));
const fx = JSON.parse(fs.readFileSync(path.join(repo, 'scripts/tests/fixtures/notify-cases.json'), 'utf8'));
const en = JSON.parse(fs.readFileSync(path.join(repo, 'data/i18n/en.json'), 'utf8'));

rules.use(file);

const merge = (base, over) => Object.assign({}, base, over || {});

test('every shared case gets the same answer as on the desktop', () => {
  assert.ok(fx.cases.length >= 30, `the shared cases shrank to ${fx.cases.length}`);
  for (const c of fx.cases) {
    let cand = merge(fx.base.candidate, c.mention ? fx.mention_room : null);
    cand = merge(cand, c.candidate);
    const got = rules.decide(cand, merge(fx.base.synced, c.synced), merge(fx.base.local, c.local), merge(fx.base.moment, c.moment));
    assert.deepStrictEqual(got, c.expect, c.name);
  }
});

test('mentions, clocks and quiet windows as the desktop reads them', () => {
  for (const m of fx.mentions) assert.strictEqual(rules.isMention(m.text, m.name), m.mentioned, `${JSON.stringify(m.text)} for ${JSON.stringify(m.name)}`);
  for (const c of fx.clocks) assert.strictEqual(rules.parseClock(c.text), c.minutes, JSON.stringify(c.text));
  for (const q of fx.quiet) assert.strictEqual(rules.inQuiet(q.start, q.end, q.minute), q.quiet, `${q.start}..${q.end} at ${q.minute}`);
  assert.strictEqual(rules.formatClock(1320), '22:00');
  assert.strictEqual(rules.formatClock(425), '07:05');
});

// The English words, as the website's i18n.t gives them (placeholders filled).
function t(key, vars) {
  let v = key.split('.').reduce((o, k) => (o ? o[k] : undefined), en);
  if (typeof v !== 'string') return key;
  for (const [k, x] of Object.entries(vars || {})) v = v.split('{{' + k + '}}').join(String(x));
  return v.replace(/\[\[([^\]|]+)(\|[^\]]*)?\]\]/g, '$1');
}

test('a notice shows the message only when the person chose it', () => {
  const dm = { kind: 'dm', sender: 'Ben', room_name: '', text: 'meet me at noon' };
  const [title, body] = rules.words(dm, 'name', t);
  assert.strictEqual(title, 'Message from Ben');
  assert.ok(!body.includes('noon'), body);
  const hidden = rules.words(dm, 'nothing', t);
  assert.ok(!hidden.join(' ').includes('Ben') && !hidden.join(' ').includes('noon'), hidden.join(' / '));
  assert.strictEqual(rules.words(dm, 'name_and_text', t)[1], 'meet me at noon');
  const mention = { kind: 'mention', sender: 'Ben', room_name: 'general', text: 'hey @me' };
  assert.strictEqual(rules.words(mention, 'name', t)[0], 'Ben mentioned you in #general');
  assert.strictEqual(rules.clean('  a\n\tb\u0007  c ', 80), 'a b c');
  const cut = rules.clean('é'.repeat(300), 10);
  assert.strictEqual(Array.from(cut).length, 10);
});

test('every word the data file names is in English', () => {
  const keys = [];
  for (const k of file.kinds) keys.push(k.label, k.hint, k.title, k.title_hidden, k.body_hidden);
  for (const m of file.mute_for) keys.push(m.label, m.hint);
  for (const l of file.levels) keys.push(l.label, l.hint);
  // A room set to nothing never notifies: every room kind needs at least mentions.
  for (const k of file.kinds.filter((x) => x.scope === 'room')) assert.notStrictEqual(k.level, 'nothing', `${k.id} would notify in a room set to nothing`);
  for (const s of file.sounds) keys.push(s.label);
  for (const key of keys) assert.notStrictEqual(t(key), key, `${key} is missing from en.json`);
});
