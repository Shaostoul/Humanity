// Notifications rung 2 on the website, part 1 (2026-10-31, docs/design/notifications.md): the
// in-app list behind the chat header's bell, the quiet-hours exceptions, the conversation mute,
// the room's level, the reply / contact-request / Commons kinds, and Settings' Muted list and
// exception switches. The decision itself is the shared cases' (notify-rules-web.test.js); this
// holds the chat's glue to it.
//
// Run: node --test scripts/tests/notify-list-web.test.js   (in `just rig-tests`)
//
// With the real notify-rules.js, events.js and chat-notify.js run in node:vm over a stand-in page,
// socket and localStorage, and functions sliced out of chat-ui.js, chat-privacy.js and
// settings-app.js as they are:
//  1. What the rule lets through is listed and counted; a burst is one entry; a line on screen is
//     not listed; the message is kept only when the person's setting shows it.
//  2. The exceptions arrive with the server's set and are what the rule reads; a change of one on
//     the Settings page goes, alone.
//  3. A muted conversation and a room's level, as the chat keeps them, reach the rule.
//  4. notifyNewMessage names the kind: a mention, else a reply to MY line, else an ordinary line.
//  5. A contact request notifies only when it came live, as its own kind.
//  6. A shared room's line notifies only while live and from someone not blocked.
//  7. Settings: the unread Do Not Disturb section and "Notification Sound" are gone, the two
//     exceptions are there, and the Muted list unmutes.
//
// Red first, 2026-10-31, each by breaking the code and watching the named assertion fail:
//  1: listAdd without the fold ("a burst is one entry"); the text kept for 'name' ("who, never
//     what"); 2: prefsData reading quiet_friends as `!== false` ("off unless the server says
//     on"); 3: loadMutes dropping `people` ("a muted conversation"); 4: the reply test reading
//     `where.reply_from` alone ("a reply to someone else's line"); 5: receiveContactRequest
//     ignoring `live` ("a page of old requests never notifies"); 6: the wrapper without its age
//     check ("a link catching up"); 7: the Muted list's Unmute leaving the mute ("Unmute clears it").

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const read = (p) => fs.readFileSync(path.join(WEB, p), 'utf8').replace(/\r\n/g, '\n');
// Objects made inside a vm context have its prototypes; compare them as plain JSON.
const plain = (v) => JSON.parse(JSON.stringify(v));
const notificationsFile = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'notifications', 'notifications.json'), 'utf8'));

function storage(initial) {
  const m = new Map(Object.entries(initial || {}));
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
  };
}

function slice(src, start, endMark) {
  const a = src.indexOf(start);
  assert.ok(a >= 0, 'no ' + start);
  const b = src.indexOf(endMark, a);
  assert.ok(b > a, 'no end for ' + start);
  return src.slice(a, b + endMark.length);
}

/** The chat page's notification pieces over stand-ins, with notifyNewMessage and its globals. */
function chatPage(stored) {
  const sent = [];
  const notices = [];
  const ctx = {
    console, JSON, Math, Date, Object, Array, String, Number, Promise, Map, Set,
    setTimeout: () => 0,
    location: { origin: 'https://example.test' },
    localStorage: storage(stored),
    fetch: () => new Promise(() => {}),
    document: { readyState: 'complete', getElementById: () => null, addEventListener: () => {}, removeEventListener: () => {} },
    addEventListener: () => {},
    addNotice: (text, color) => notices.push({ text, color }),
    i18n: { t: (k) => k, hint: () => {} },
    ws: { readyState: 1, send: (s) => sent.push(JSON.parse(s)) },
    // chat-ui.js's globals notifyNewMessage reads.
    windowFocused: false, unreadCount: 0, startTitleFlash: () => {}, myName: 'Ann', myKey: 'me',
    activeChannel: 'general', activeDmPartner: null, activeGroupId: null, soundEnabled: true,
    playNotificationChime: () => {},
  };
  ctx.window = ctx;
  ctx.globalThis = ctx;
  vm.createContext(ctx);
  vm.runInContext(read('shared/events.js'), ctx, { filename: 'events.js' });
  vm.runInContext(read('shared/notify-rules.js'), ctx, { filename: 'notify-rules.js' });
  ctx.HosNotifyRules.use(notificationsFile);
  vm.runInContext(read('chat/chat-notify.js'), ctx, { filename: 'chat-notify.js' });
  vm.runInContext(slice(read('chat/chat-ui.js'), 'function notifyNewMessage(', '\n}\n'), ctx, { filename: 'chat-ui.js#notifyNewMessage' });
  return { ctx, sent, notices };
}

const PREFS = { type: 'notification_prefs_data', dm: true, mentions: true, tasks: true, dnd_start: null, dnd_end: null };
const AWAY = { focused: false, on_screen: false, desktop: true, sound: true };
const dm = (key, text) => ({ kind: 'dm', from_me: false, server: 'https://example.test', room: null, conversation: 'dm:' + key, sender: 'Ben', room_name: '', text, from_friend: false });

test('1. what the rule lets through is listed: a burst is one entry, the screen and the text as chosen', () => {
  const page = chatPage({});
  const N = page.ctx.HosChatNotify;
  N.prefsData(PREFS);
  assert.strictEqual(N.decide(dm('k1', 'meet at noon'), AWAY).list, true);
  assert.strictEqual(N.list().length, 1);
  assert.strictEqual(N.unseen(), 1);
  assert.strictEqual(N.list()[0].text, null, 'who, never what, by default');
  // The burst guard holds a second line inside 5 seconds back (too_soon): not listed.
  assert.strictEqual(N.decide(dm('k1', 'again'), AWAY).why, 'too_soon');
  assert.strictEqual(N.list().length, 1);
  // A line on screen with the tab in front: no entry.
  assert.strictEqual(N.decide(dm('k2', 'x'), { focused: true, on_screen: true, desktop: true, sound: true }).why, 'on_screen');
  assert.strictEqual(N.list().length, 1);
  // The tab in front on another conversation: listed (the focused-window case).
  page.ctx.localStorage.setItem('humanity_settings', JSON.stringify({ 'notif-show': 'name_and_text' }));
  assert.strictEqual(N.decide(dm('k3', 'see you'), { focused: true, on_screen: false, desktop: true, sound: false }).list, true);
  assert.strictEqual(N.list()[0].text, 'see you', 'kept: the person shows the message');
  assert.strictEqual(N.unseen(), 2);
});

test('1b. a later notice for an unseen conversation folds into its entry', () => {
  const page = chatPage({});
  const N = page.ctx.HosChatNotify;
  N.prefsData(PREFS);
  const realNow = Date.now;
  let t = realNow();
  page.ctx.Date.now = () => t; // the burst guard reads seconds; step past it
  try {
    N.decide(dm('k1', 'one'), AWAY);
    t += 10000;
    N.decide(Object.assign(dm('k1', 'two'), { sender: 'Ben B' }), AWAY);
  } finally {
    page.ctx.Date.now = realNow;
  }
  assert.strictEqual(N.list().length, 1, 'a burst is one entry');
  assert.strictEqual(N.list()[0].sender, 'Ben B', 'with the latest sender');
  assert.strictEqual(N.unseen(), 1);
});

test('2. the exceptions come with the server set; a Settings change of one goes alone', () => {
  const page = chatPage({});
  const N = page.ctx.HosChatNotify;
  const quiet = Object.assign({}, PREFS, { dnd_start: '00:00', dnd_end: '23:59' });
  N.prefsData(quiet);
  const friend = Object.assign(dm('k1', 'hi'), { from_friend: true });
  assert.strictEqual(N.decide(friend, AWAY).why, 'quiet_hours', 'off unless the server says on');
  N.prefsData(Object.assign({}, quiet, { quiet_friends: 'yes' }));
  assert.strictEqual(N.decide(friend, AWAY).why, 'quiet_hours', 'off unless the server says on (a malformed value too)');
  N.prefsData(Object.assign({}, quiet, { quiet_friends: true }));
  assert.strictEqual(N.decide(friend, AWAY).why, 'notify', "a friend's DM comes through");
  assert.strictEqual(page.ctx.localStorage.getItem('hos_quiet_friends'), 'true', 'and this browser keeps the server copy');
  // The Settings page's switch is queued alone and sent alone.
  const p2 = chatPage({ hos_notify_pending: JSON.stringify({ quiet_mentions: true }) });
  p2.ctx.hos.emit('socket-identified');
  assert.deepStrictEqual(plain(p2.sent), [{ type: 'update_notification_prefs', quiet_mentions: true }]);
});

test('3. a muted conversation and a room level reach the rule', () => {
  const now = Math.floor(Date.now() / 1000);
  const page = chatPage({
    hos_notify_mutes: JSON.stringify({ everything: null, servers: {}, rooms: {}, people: { 'dm:k1': 0 } }),
    hos_notify_levels: JSON.stringify({ 'https://example.test|general': 'all' }),
  });
  const N = page.ctx.HosChatNotify;
  N.prefsData(PREFS);
  assert.strictEqual(N.decide(dm('k1', 'hi'), AWAY).why, 'person_muted', 'a muted conversation');
  assert.strictEqual(N.decide(dm('k2', 'hi'), AWAY).why, 'notify');
  const line = { kind: 'room_line', from_me: false, server: 'https://example.test', room: 'general', conversation: 'general', sender: 'Cy', room_name: 'general', text: 'hello' };
  assert.strictEqual(N.decide(line, AWAY).why, 'notify', 'a room set to every line');
  assert.strictEqual(N.decide(Object.assign({}, line, { room: 'random', conversation: 'random' }), AWAY).why, 'room_level', 'another room at mentions');
  assert.match(N.dmBellHtml('k1'), /row-mute muted/, 'the muted mark on the row');
  assert.ok(!/muted/.test(N.dmBellHtml('k2').split('"')[1]), 'none on another');
  assert.ok(now > 0);
});

test('4. notifyNewMessage names the kind: a mention, a reply to my line, an ordinary line', () => {
  const page = chatPage({ hos_notify_levels: JSON.stringify({ 'https://example.test|general': 'all', 'https://example.test|lobby': 'all' }) });
  const N = page.ctx.HosChatNotify;
  N.prefsData(PREFS);
  const kinds = () => N.list().map((e) => e.kind);
  page.ctx.notifyNewMessage('Ben', 'hey @ann', false, { room: 'general' });
  assert.deepStrictEqual(plain(kinds()), ['mention']);
  page.ctx.notifyNewMessage('Ben', 'agreed', false, { room: 'random', reply_from: 'me' });
  assert.strictEqual(kinds()[0], 'reply', 'a reply to my line');
  page.ctx.notifyNewMessage('Ben', 'agreed', false, { room: 'news', reply_from: 'k9' });
  assert.strictEqual(N.list().length, 2, "a reply to someone else's line is an ordinary line: news is at mentions");
  page.ctx.notifyNewMessage('Cy', 'just chatting', false, { room: 'lobby' });
  assert.strictEqual(kinds()[0], 'room_line', 'every line, in a room set to it');
});

test('5. a contact request notifies only when it came live, as its own kind', () => {
  const calls = [];
  const ctx = {
    myKey: 'me',
    reachStore: () => ({ isBlocked: () => false, addContactRequest: () => true }),
    reachCurrent: () => ({ message: 'everyone' }),
    reachDisplayName: () => 'Cy',
    reachSay: () => {},
    renderRequestsEverywhere: () => {},
    notifyNewMessage: (...a) => calls.push(a),
    Number, Date,
  };
  vm.createContext(ctx);
  vm.runInContext(slice(read('chat/chat-privacy.js'), 'function receiveContactRequest(', '\n}\n'), ctx);
  ctx.receiveContactRequest({ key: 'k5', ts: 1 });
  assert.strictEqual(calls.length, 0, 'a page of old requests never notifies');
  ctx.receiveContactRequest({ key: 'k5', ts: 1 }, true);
  assert.strictEqual(calls.length, 1);
  assert.strictEqual(calls[0][2], true);
  assert.deepStrictEqual(plain(calls[0][3]), { kind: 'contact_request', conversation: 'dm:k5' });
  assert.strictEqual(calls[0][1], '', 'who, never what: a request carries no words of theirs to show');
});

test('6. a shared room line notifies only while live and from someone not blocked', () => {
  const calls = [];
  const ctx = {
    _origHandleMessage: () => {},
    localStorage: storage({}),
    myKey: 'me',
    resolveSenderName: (n) => n,
    notifyNewMessage: (...a) => calls.push(a),
    isBlockedKey: (k) => k === 'bad',
    activeChannel: 'general',
    window: { HosChatNotify: { FEDERATED_LIVE_MS: 5 * 60 * 1000 } },
    Date, Number, String,
  };
  ctx.HosChatNotify = ctx.window.HosChatNotify;
  vm.createContext(ctx);
  vm.runInContext('var handleMessage; ' + slice(read('chat/chat-ui.js'), 'handleMessage = function(msg) {\n  _origHandleMessage(msg);', '\n};\n'), ctx);
  const fed = (o) => Object.assign({ type: 'federated_chat', server_name: 'B', from_name: 'Cy', from_key: 'k9', content: '@ann hi', channel: 'general', timestamp: Date.now() }, o);
  ctx.handleMessage(fed({}));
  assert.strictEqual(calls.length, 1);
  assert.strictEqual(calls[0][0], 'Cy (B)');
  assert.deepStrictEqual(plain(calls[0][3]), { room: 'general' });
  ctx.handleMessage(fed({ timestamp: Date.now() - 6 * 60 * 1000 }));
  assert.strictEqual(calls.length, 1, 'a link catching up');
  ctx.handleMessage(fed({ from_key: 'bad' }));
  assert.strictEqual(calls.length, 1, 'someone I blocked');
  ctx.handleMessage(fed({ from_key: 'me' }));
  assert.strictEqual(calls.length, 1, 'my own line from the other server');
  // A reply on a local line carries the replied-to author.
  ctx.handleMessage({ type: 'chat', from: 'k2', from_name: 'Ben', content: 'yes', channel: 'general', timestamp: 1, reply_to: { from: 'me' } });
  assert.strictEqual(calls[1][3].reply_from, 'me');
});

test('7. Settings: the unread section is gone, the exceptions are there, the Muted list unmutes', () => {
  const html = read('pages/settings.html');
  assert.ok(!html.includes('id="sec-dnd"'), 'the unread Do Not Disturb section is gone');
  assert.ok(!html.includes('pref-notif-sound'), 'and the unread Notification Sound switch');
  for (const id of ['pref-push-quiet-friends', 'pref-push-quiet-mentions']) {
    assert.match(html, new RegExp('id="' + id + '"[^>]*onchange="savePushPref\\(this\\)"'), id + ' names itself');
  }
  assert.ok(html.includes('id="notif-muted-list"'), 'the Muted list');
  const src = read('pages/settings-app.js');
  const els = { 'pref-push-quiet-friends': { id: 'pref-push-quiet-friends', checked: true } };
  // A tiny DOM for the Muted list: rows of elements whose buttons can be clicked.
  const made = [];
  function el(tag) {
    const e = { tag, children: [], textContent: '', className: '', listeners: {}, appendChild(c) { this.children.push(c); return c; }, setAttribute() {}, addEventListener(t, f) { this.listeners[t] = f; } };
    Object.defineProperty(e, 'innerHTML', { set() { e.children = []; }, get() { return ''; } });
    made.push(e);
    return e;
  }
  const list = el('div');
  const ctx = {
    JSON, Object, Math, Date, parseInt, isNaN, Array, String,
    localStorage: storage({ hos_notify_mutes: JSON.stringify({ everything: null, servers: {}, rooms: { 'https://example.test|general': 0 }, people: { 'dm:k1': 0 } }), hos_notify_names: JSON.stringify({ 'dm:k1': 'Ben' }) }),
    location: { origin: 'https://example.test', host: 'example.test' },
    document: { getElementById: (id) => (id === 'notif-muted-list' ? list : els[id] || null), createElement: el },
    window: { i18n: { t: (k, v) => k + (v ? JSON.stringify(v) : ''), hint: () => {} }, addEventListener: () => {} },
  };
  ctx.window.window = ctx.window;
  vm.createContext(ctx);
  for (const name of ['queueNotifChange', 'savePushPref', 'syncNotifPrefsToServer', 'notifT', 'notifLoad', 'notifMutes', 'notifActive', 'notifUntil', 'notifPersonName', 'notifRow', 'notifSaveMutes', 'renderMutedList']) {
    vm.runInContext(slice(src, 'function ' + name + '(', '\n}\n'), ctx, { filename: 'settings-app.js#' + name });
  }
  vm.runInContext('var notifFile = null;', ctx);
  ctx.savePushPref(els['pref-push-quiet-friends']);
  assert.deepStrictEqual(JSON.parse(ctx.localStorage.getItem('hos_notify_pending')), { quiet_friends: true }, 'the exception is queued alone');
  ctx.renderMutedList();
  const words = made.map((e) => e.textContent).join(' | ');
  assert.match(words, /notifications\.muted_person\{"name":"Ben"\}/, 'the conversation by the name the chat kept');
  assert.match(words, /notifications\.muted_room/, 'the room');
  const unmute = made.filter((e) => e.tag === 'button' && e.textContent === 'notifications.unmute');
  assert.strictEqual(unmute.length, 2);
  unmute[1].listeners.click();
  const m = JSON.parse(ctx.localStorage.getItem('hos_notify_mutes'));
  assert.deepStrictEqual(plain(m.people), {}, 'Unmute clears it');
  assert.deepStrictEqual(Object.keys(m.rooms), ['https://example.test|general'], 'and only it');
});
