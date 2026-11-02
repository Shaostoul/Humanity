// Notifications rung 3 on the website (2026-11-01, docs/design/notifications.md section 11): the
// chat's glue to THE rule for the new kinds. The decision itself is the shared cases'
// (notify-rules-web.test.js); this holds what the chat does with it.
//
// Run: node --test scripts/tests/notify-rung3-web.test.js   (in `just rig-tests`)
//
// With the real notify-rules.js, events.js and chat-notify.js run in node:vm over a stand-in page,
// socket, localStorage and browser Notification:
//  1. The always-allow list's note to oneself has the bytes the desktop's has (one vector), and the
//     newest note wins whatever the order.
//  2. Someone on my list: their DM and their call come through quiet hours; adding them sends one
//     note to my own mailbox, and a note from anyone else changes nothing.
//  3. A ring: held by quiet hours (no notice, no sound) unless the caller is always allowed; its
//     sound repeats while it rings, and its notice is closed when the ring ends.
//  4. A task notice: given to me, as its own words; a muted server holds it; a click opens the task.
//  5. A kind's sound set to none plays nothing; another kind's chosen sound is the one played.
//
// Red first, 2026-11-01, each by breaking the code and watching the named assertion fail:
//  1: allowNoteText keeping the key's case ("the vector both apps hold"); 2: decide not reading
//     the list for a DM ("a DM from someone always allowed"); screenAllowNote taking anyone's note
//     ("a note from someone else changes nothing"); 3: ringEnded leaving the notice open ("the ring
//     ended: its notice is closed"); 4: onTaskNotice without the variant ("gave you a task");
//     5: decide's sound_id read from the dm kind for every kind ("the call's own sound").

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const read = (p) => fs.readFileSync(path.join(WEB, p), 'utf8').replace(/\r\n/g, '\n');
const plain = (v) => JSON.parse(JSON.stringify(v));
const notificationsFile = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'notifications', 'notifications.json'), 'utf8'));
const en = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8'));

function storage(initial) {
  const m = new Map(Object.entries(initial || {}));
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
  };
}

// English words with their placeholders filled, as the website's i18n.t gives them.
function t(key, vars) {
  let v = key.split('.').reduce((o, k) => (o ? o[k] : undefined), en);
  if (typeof v !== 'string') return key;
  for (const [k, x] of Object.entries(vars || {})) v = v.split('{{' + k + '}}').join(String(x));
  return v.replace(/\[\[([^\]|]+)(\|[^\]]*)?\]\]/g, '$1');
}

function chatPage(stored) {
  const sent = [];
  const shown = [];
  const played = [];
  const timers = [];
  const ctx = {
    console, JSON, Math, Date, Object, Array, String, Number, Promise, Map, Set,
    setTimeout: () => 0,
    setInterval: (fn) => { timers.push(fn); return timers.length; },
    clearInterval: (id) => { timers[id - 1] = null; },
    location: { origin: 'https://example.test', href: '' },
    localStorage: storage(stored),
    fetch: () => new Promise(() => {}),
    document: { readyState: 'complete', getElementById: () => null, addEventListener: () => {}, removeEventListener: () => {} },
    addEventListener: () => {},
    addNotice: () => {},
    focus: () => {},
    i18n: { t, hint: () => {} },
    ws: { readyState: 1, send: (s) => sent.push(JSON.parse(s)) },
    myKey: 'me', soundEnabled: true,
    playNotificationChime: () => played.push('chime'),
    // The sealed note to myself (crypto.js): its text is what matters here.
    pqBuildSelfNote: async (text, ts) => ({ put: { type: 'dm_put', to: 'me', note: text, ts } }),
    isBlockedKey: (k) => k === 'blocked',
  };
  ctx.Audio = function (src) { this.src = src; this.play = () => { played.push(src); return Promise.resolve(); }; };
  ctx.Notification = function (title, opts) { this.title = title; this.body = opts.body; this.closed = false; this.close = () => { this.closed = true; }; shown.push(this); };
  ctx.window = ctx;
  ctx.globalThis = ctx;
  vm.createContext(ctx);
  vm.runInContext(read('shared/events.js'), ctx, { filename: 'events.js' });
  vm.runInContext(read('shared/notify-rules.js'), ctx, { filename: 'notify-rules.js' });
  ctx.HosNotifyRules.use(notificationsFile);
  vm.runInContext(read('chat/chat-notify.js'), ctx, { filename: 'chat-notify.js' });
  ctx.HosChatNotify.prefsData({ type: 'notification_prefs_data', dm: true, mentions: true, tasks: true, dnd_start: null, dnd_end: null });
  return { ctx, N: ctx.HosChatNotify, R: ctx.HosNotifyRules, sent, shown, played, timers };
}

const AWAY = { focused: false, on_screen: false, desktop: true, sound: true };
const QUIET_ALL_DAY = { type: 'notification_prefs_data', dm: true, mentions: true, tasks: true, dnd_start: '00:00', dnd_end: '23:59', quiet_friends: true, quiet_mentions: false };
const flush = () => new Promise((r) => setImmediate(r));

test('1. the always-allow note has the bytes the desktop has, and the newest note wins', () => {
  const { R } = chatPage({});
  assert.strictEqual(R.allowNoteText('AB12', true), '[[hum:quiet-allow:v1]]ab12/on', 'the vector both apps hold');
  assert.deepStrictEqual(plain(R.allowNoteParse('[[hum:quiet-allow:v1]]ab12/off')), { key: 'ab12', on: false });
  for (const bad of ['[[hum:quiet-allow:v1]]AB12/on', '[[hum:quiet-allow:v1]]ab12/yes', '[[hum:quiet-allow:v1]]/on']) {
    assert.strictEqual(R.allowNoteParse(bad), null, bad);
    assert.ok(R.isAllowNote(bad), 'never shown: ' + bad);
  }
  const a = {}; const b = {};
  assert.ok(R.allowApply(a, 'ann', true, 100));
  assert.ok(R.allowApply(a, 'ann', false, 200));
  assert.ok(!R.allowApply(a, 'ann', true, 150), 'an older on after a newer off');
  R.allowApply(b, 'ann', false, 200);
  R.allowApply(b, 'ann', true, 100);
  assert.deepStrictEqual(plain(a), plain(b), 'either order, the same list');
});

test('2. someone always allowed comes through quiet hours, and only my own note changes the list', async () => {
  const { N, sent } = chatPage({});
  N.prefsData(QUIET_ALL_DAY);
  const dm = { kind: 'dm', from_me: false, server: 'https://example.test', room: null, conversation: 'dm:ab12', sender: 'Ann', room_name: '', text: 'hi', from_friend: false };
  assert.strictEqual(N.decide(Object.assign({}, dm), AWAY).why, 'quiet_hours', 'not on the list: it waits');
  N.setAllowed('AB12', true, 'Ann');
  await flush(); await flush();
  assert.ok(N.allowed('ab12'));
  assert.deepStrictEqual(sent.map((m) => m.note), ['[[hum:quiet-allow:v1]]ab12/on'], 'one note to my own mailbox');
  assert.strictEqual(N.decide(Object.assign({}, dm), AWAY).why, 'notify', 'a DM from someone always allowed');
  // A note from someone else is never shown and changes nothing; mine from another device does.
  assert.strictEqual(N.screenAllowNote({ from: 'eve', to: 'me', ts: Date.now() + 5, text: '[[hum:quiet-allow:v1]]eeee/on' }), true);
  assert.ok(!N.allowed('eeee'), 'a note from someone else changes nothing');
  assert.strictEqual(N.screenAllowNote({ from: 'me', to: 'me', ts: Date.now() + 10, text: '[[hum:quiet-allow:v1]]ab12/off' }), true);
  assert.ok(!N.allowed('ab12'), 'my other device took them off');
  assert.strictEqual(N.screenAllowNote({ from: 'me', to: 'me', ts: 1, text: 'hello' }), false, 'an ordinary DM is not a note');
});

test('3. a ring waits in quiet hours unless always allowed; its sound repeats; its notice goes when it ends', () => {
  const page = chatPage({});
  const { N, shown, played, timers } = page;
  N.prefsData(QUIET_ALL_DAY);
  // A friend, friends' messages excepted: a call still waits (the operator, 2026-11-01).
  assert.strictEqual(N.ring('ff01', 'Fred', false, true).why, 'quiet_hours');
  assert.strictEqual(shown.length + played.length, 0, 'no notice, no sound');
  page.ctx.localStorage.setItem('hos_quiet_allow', JSON.stringify({ me: { ab12: [true, 1] } }));
  const d = N.ring('ab12', 'Ann', false, true);
  assert.strictEqual(d.why, 'notify');
  assert.strictEqual(shown.length, 1);
  assert.strictEqual(shown[0].title, 'Ann is calling');
  assert.strictEqual(shown[0].body, 'Open HumanityOS to answer or decline it.', 'a call has no message to show');
  assert.strictEqual(played.length, 1, 'it rings');
  timers.filter(Boolean).forEach((fn) => fn());
  assert.strictEqual(played.length, 2, 'and rings again while it rings');
  N.ringEnded();
  assert.ok(shown[0].closed, 'the ring ended: its notice is closed');
  assert.strictEqual(timers.filter(Boolean).length, 0, 'and its sound stops');
});

test('4. a task notice: given to me in its own words, held by a muted server, opening the task', () => {
  const page = chatPage({});
  const { N, shown } = page;
  const msg = { type: 'task_notice', task_id: 7, title: 'Fix the pump', change: 'assigned', status: 'backlog', by_name: 'Mo', by_key: 'k_mod' };
  assert.strictEqual(N.onTaskNotice(Object.assign({}, msg, { by_key: 'blocked' }), false, true), null, 'from someone I blocked: nothing');
  const d = N.onTaskNotice(msg, false, true);
  assert.strictEqual(d.why, 'notify');
  assert.strictEqual(shown[0].title, 'Mo gave you a task');
  assert.ok(!shown[0].body.includes('pump'), 'the title only when the person shows the message');
  shown[0].onclick();
  assert.strictEqual(page.ctx.location.href, '/tasks?task=7');
  page.ctx.localStorage.setItem('hos_notify_mutes', JSON.stringify({ everything: null, servers: { 'https://example.test': 0 }, rooms: {}, people: {} }));
  assert.strictEqual(N.onTaskNotice(Object.assign({}, msg, { task_id: 8, change: 'changed' }), false, true).why, 'server_muted');
  assert.strictEqual(N.onTaskNotice(Object.assign({}, msg, { by_key: 'me', task_id: 9 }), false, true).why, 'own_message', 'my own change');
});

test("5. a kind's sound: none plays nothing; a chosen one is the one played", () => {
  const page = chatPage({ hos_notify_sounds: JSON.stringify({ dm: 'none', call: 'sfx.success' }) });
  const { N, played } = page;
  const dm = { kind: 'dm', from_me: false, server: 'https://example.test', room: null, conversation: 'dm:k1', sender: 'Ben', room_name: '', text: 'hi', from_friend: false };
  const d = N.decide(dm, AWAY);
  assert.strictEqual(d.why, 'notify');
  assert.strictEqual(d.sound, false, 'none plays nothing');
  N.ring('k2', 'Cy', false, false);
  assert.deepStrictEqual(played, ['/assets/audio/ui/success.ogg'], "the call's own sound");
});
