// The person's notification switches on the website (2026-10-30, the seams review of the
// notifications batch, docs/design/notifications.md): what notifies them (direct messages,
// mentions, tasks, quiet hours) is kept on their server, and the website must neither act on nor
// write a copy it never got from there.
//
// Run: node --test scripts/tests/notify-prefs-web.test.js   (in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/ (how each test was seen red).
//
// THE FAULTS IT GUARDS:
//  - The chat's DM cog sent a FULL update_notification_prefs built from this browser's
//    localStorage (mentions and tasks on by default, quiet hours only when both were stored),
//    and the relay replaced all five stored values with it: one click in a browser that never
//    opened Settings wiped the person's quiet hours and turned mentions back on everywhere.
//  - The website's rule said `loaded: true` from localStorage, so it notified mentions the
//    person had turned off on another device; the desktop refuses until its server answered
//    (src/notifications/decide.rs, Why::NotLoaded).
//  - Settings > Notifications on the website has no connection to the server; it wrote whole
//    sets into localStorage that the chat then sent on.
//
// What it proves, with the real notify-rules.js, events.js and chat-notify.js run in node:vm over
// a stand-in page, socket and localStorage, and chat-ui.js's own openDmSettings:
//  1. Before the server answered, nothing notifies (not_loaded) and the cog sends no change, only
//     a request for the stored switches, and says it is still asking.
//  2. Once accepted on a socket the chat asks for the stored switches; their answer is what the
//     rule reads (a mention the person turned off stays quiet) and what this browser keeps.
//  3. The cog then changes ONLY the DM switch: the frame it sends has `type` and `dm`, nothing
//     else, and it flips what the server holds, not what this browser remembered.
//  4. A switch changed on the Settings page (no socket there) is queued as just that switch, and
//     the chat sends that change, alone, when its socket is accepted.
//  5. Settings records only the switch that changed, never the whole page's set.
//
// Red first, 2026-10-30, each against the code before the fix (or a copy of web/ with the fix
// taken out, through HOS_WEB_DIR):
//  1: the old openDmSettings (a full frame from localStorage): "no change is written before the
//     server answered"; the old `loaded: true`: "nothing notifies before the server answered".
//  2: chat-notify.js without its socket-identified ask: "the chat asks its server once accepted".
//  3: a cog sending the whole held set: "the cog's frame names only the DM switch".
//  4: no hos_notify_pending flush: "the queued change goes, alone".
//  5: the old savePushPref writing all three: "only the switch that changed is queued".

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const read = (p) => fs.readFileSync(path.join(WEB, p), 'utf8').replace(/\r\n/g, '\n');
const notificationsFile = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'notifications', 'notifications.json'), 'utf8'));

function storage(initial) {
  const m = new Map(Object.entries(initial || {}));
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    _map: m,
  };
}

/** A top-level `name = function` or `function name(` in `src`, through its closing line. */
function slice(src, start, endMark) {
  const a = src.indexOf(start);
  assert.ok(a >= 0, 'no ' + start);
  const b = src.indexOf(endMark, a);
  assert.ok(b > a, 'no end for ' + start);
  return src.slice(a, b + endMark.length);
}

/** The chat page's notification pieces, loaded as index.html loads them, over stand-ins. */
function chatPage(stored) {
  const sent = [];
  const notices = [];
  const listeners = {};
  const ctx = {
    console,
    JSON, Math, Date, Object, Array, String, Number, Promise, Map, Set,
    setTimeout: () => 0,
    location: { origin: 'https://example.test' },
    localStorage: storage(stored),
    fetch: () => new Promise(() => {}), // the data file is handed in below instead
    document: {
      readyState: 'complete',
      getElementById: () => null,
      addEventListener: () => {},
      removeEventListener: () => {},
    },
    addEventListener: (type, fn) => { (listeners[type] = listeners[type] || []).push(fn); },
    addNotice: (text, color) => notices.push({ text, color }),
    i18n: { t: (k) => k, hint: () => {} },
    ws: { readyState: 1, send: (s) => sent.push(JSON.parse(s)) },
    WebSocket: { OPEN: 1 },
  };
  ctx.window = ctx;
  ctx.globalThis = ctx;
  vm.createContext(ctx);
  vm.runInContext(read('shared/events.js'), ctx, { filename: 'events.js' });
  vm.runInContext(read('shared/notify-rules.js'), ctx, { filename: 'notify-rules.js' });
  ctx.HosNotifyRules.use(notificationsFile);
  vm.runInContext(read('chat/chat-notify.js'), ctx, { filename: 'chat-notify.js' });
  const ui = read('chat/chat-ui.js');
  vm.runInContext(slice(ui, 'window.openDmSettings = function', '\n};\n'), ctx, { filename: 'chat-ui.js#openDmSettings' });
  return { ctx, sent, notices, listeners };
}

const MENTION = { kind: 'mention', from_me: false, server: 'https://example.test', room: 'general', conversation: 'general', sender: 'Ben', room_name: 'general', text: 'hey @Ann' };
const AWAY = { focused: false, on_screen: false, desktop: true, sound: true };
const STORED = { dm: true, mentions: false, tasks: true, dnd_start: null, dnd_end: null };

test('1. before the server answered, nothing notifies and the cog writes nothing', () => {
  // A browser that never opened Settings: nothing stored, so every default says "on".
  const page = chatPage({});
  assert.strictEqual(page.ctx.HosChatNotify.decide(MENTION, AWAY).why, 'not_loaded', 'nothing notifies before the server answered');
  page.ctx.openDmSettings();
  assert.deepStrictEqual(page.sent.filter((m) => m.type === 'update_notification_prefs'), [], 'no change is written before the server answered');
  assert.ok(page.sent.some((m) => m.type === 'get_notification_prefs'), 'it asks for the stored switches instead');
  assert.strictEqual(page.notices.length, 1);
  assert.strictEqual(page.notices[0].text, 'notifications.dm_wait', 'and says it is still asking');
});

test('2. once accepted, the chat asks its server, and the answer is what counts', () => {
  const page = chatPage({ hos_notify_mentions: 'true' }); // a stale copy says mentions are on
  page.ctx.hos.emit('socket-identified');
  assert.deepStrictEqual(page.sent, [{ type: 'get_notification_prefs' }], 'the chat asks its server once accepted');
  page.ctx.HosChatNotify.prefsData(Object.assign({ type: 'notification_prefs_data' }, STORED));
  assert.strictEqual(page.ctx.HosChatNotify.decide(MENTION, AWAY).why, 'kind_off', 'mentions are off, as the server holds them');
  assert.strictEqual(page.ctx.localStorage.getItem('hos_notify_mentions'), 'false', 'and this browser keeps the server\'s copy');
  // Quiet hours from the server are honoured too.
  page.ctx.HosChatNotify.prefsData({ type: 'notification_prefs_data', dm: true, mentions: true, tasks: true, dnd_start: '00:00', dnd_end: '23:59' });
  assert.strictEqual(page.ctx.HosChatNotify.decide(MENTION, AWAY).why, 'quiet_hours');
  assert.strictEqual(page.ctx.localStorage.getItem('hos_dnd_start'), '00:00');
});

test('3. the cog changes only the DM switch, from what the server holds', () => {
  // This browser remembers DMs as off; the server says on. The click turns them OFF.
  const page = chatPage({ hos_notify_dms: 'false' });
  page.ctx.HosChatNotify.prefsData(Object.assign({ type: 'notification_prefs_data' }, STORED));
  page.ctx.openDmSettings();
  const writes = page.sent.filter((m) => m.type === 'update_notification_prefs');
  assert.deepStrictEqual(writes, [{ type: 'update_notification_prefs', dm: false }], 'the cog\'s frame names only the DM switch');
  assert.strictEqual(page.notices[page.notices.length - 1].text, 'notifications.dm_off');
  page.ctx.openDmSettings();
  assert.deepStrictEqual(page.sent[page.sent.length - 1], { type: 'update_notification_prefs', dm: true }, 'and a second click turns them back on');
});

test('4. a change made on the Settings page goes, alone, when the chat is accepted', () => {
  const page = chatPage({ hos_notify_pending: JSON.stringify({ mentions: false }) });
  page.ctx.hos.emit('socket-identified');
  assert.deepStrictEqual(page.sent, [{ type: 'update_notification_prefs', mentions: false }], 'the queued change goes, alone');
  assert.strictEqual(page.ctx.localStorage.getItem('hos_notify_pending'), null, 'and is sent once');
  // One queued while the chat is open in another tab goes at once, through the storage event.
  page.ctx.HosChatNotify.prefsData(Object.assign({ type: 'notification_prefs_data' }, STORED));
  page.ctx.localStorage.setItem('hos_notify_pending', JSON.stringify({ dnd_start: null, dnd_end: null }));
  for (const fn of page.listeners.storage || []) fn({ key: 'hos_notify_pending' });
  assert.deepStrictEqual(page.sent[page.sent.length - 1], { type: 'update_notification_prefs', dnd_start: null, dnd_end: null });
  // Anything that is not a switch is never sent on.
  page.ctx.localStorage.setItem('hos_notify_pending', JSON.stringify({ tasks: false, evil: 1, dm: 'yes' }));
  for (const fn of page.listeners.storage || []) fn({ key: 'hos_notify_pending' });
  assert.deepStrictEqual(page.sent[page.sent.length - 1], { type: 'update_notification_prefs', tasks: false });
});

test('5. Settings queues only the switch that changed', () => {
  const src = read('pages/settings-app.js');
  const els = {
    'pref-push-dms': { id: 'pref-push-dms', checked: true },
    'pref-push-mentions': { id: 'pref-push-mentions', checked: false },
    'pref-push-tasks': { id: 'pref-push-tasks', checked: true },
  };
  const ctx = {
    JSON, Object, parseInt, isNaN,
    localStorage: storage({}),
    document: { getElementById: (id) => els[id] || null },
    window: {},
    sentToServer: 0,
  };
  vm.createContext(ctx);
  for (const name of ['queueNotifChange', 'savePushPref', 'syncNotifPrefsToServer']) {
    vm.runInContext(slice(src, 'function ' + name + '(', '\n}\n'), ctx, { filename: 'settings-app.js#' + name });
  }
  ctx.savePushPref(els['pref-push-mentions']);
  assert.deepStrictEqual(JSON.parse(ctx.localStorage.getItem('hos_notify_pending')), { mentions: false }, 'only the switch that changed is queued');
  assert.strictEqual(ctx.localStorage.getItem('hos_notify_mentions'), 'false', 'and this browser shows it');
  ctx.savePushPref(els['pref-push-dms']);
  assert.deepStrictEqual(JSON.parse(ctx.localStorage.getItem('hos_notify_pending')), { mentions: false, dm: true }, 'later changes join it');
  // The page's checkboxes say onchange="savePushPref(this)", so the one that changed is named.
  const html = read('pages/settings.html');
  for (const id of Object.keys(els)) {
    assert.match(html, new RegExp('id="' + id + '"[^>]*onchange="savePushPref\\(this\\)"'), id + ' names itself');
  }
});
