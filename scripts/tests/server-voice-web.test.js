// The website's own voice carries a mark no relay text can (the owner branch's seventh review, H1,
// 2026-11-21). Relay text reached the app's own system lines through routes around the server
// mark: a `system` frame "__game__:{game_admin_error}" (chat-game-admin.js), "__skill_verify_resp__"
// and "__skill_verify_req__" (the request in a browser confirm()), and the friend-code answers,
// taken even when nobody asked (chat-social.js; an unasked friend_code_result followed whoever it
// named).
//
// Run: node --test scripts/tests/server-voice-web.test.js
//
// The REAL `case 'system':` block of web/chat/app.js, its real addSystemMessage, addServerMessage,
// shownName and skill handlers, the real web/chat/chat-game-admin.js, the real friend-code part of
// web/chat/chat-social.js and the real /shared/key-card.js run in one node:vm page with a small DOM
// of its own and the real words of data/i18n/en.json. What is read is what the page drew: each
// line's text and its class, and every dialog the browser was asked to show.
//
// Demos A, B and C are the review's (scratchpad critic7/web-unmarked-lines.test.js), held as tests.
// Seen red 2026-11-21 against the branch before this round (scratchpad ownfix10-red-web.log): A drew
// "• <relay text>" with class "message system" and no mark; B drew the relay's note unmarked; C put
// the relay's text in a confirm() dialog; the unasked friend_code_result followed the key it named;
// the app line carried no `.app-line` mark.
'use strict';
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const ROOT = path.resolve(__dirname, '..', '..');
const app = fs.readFileSync(path.join(ROOT, 'web/chat/app.js'), 'utf8');
const social = fs.readFileSync(path.join(ROOT, 'web/chat/chat-social.js'), 'utf8');
const en = JSON.parse(fs.readFileSync(path.join(ROOT, 'data/i18n/en.json'), 'utf8'));
function T(key, vars) {
  let v = key.split('.').reduce((o, k) => (o ? o[k] : undefined), en);
  if (typeof v !== 'string') throw new Error('no words for ' + key);
  for (const k of Object.keys(vars || {})) v = v.split('{{' + k + '}}').join(String(vars[k]));
  return v;
}
/** The source of `function name(...) {...}` in `src`. */
function body(src, name) {
  const start = src.search(new RegExp('(async )?function ' + name + '\\('));
  assert.ok(start >= 0, name);
  let depth = 0;
  for (let i = src.indexOf('{', start); i < src.length; i++) {
    if (src[i] === '{') depth++;
    else if (src[i] === '}' && --depth === 0) return src.slice(start, i + 1);
  }
  throw new Error('unclosed ' + name);
}
const sysCase = app.slice(app.indexOf("case 'system':"), app.indexOf("case 'thread_response':"));

/** A small DOM: elements with children, text, classes, attributes, listeners. */
function el(tag) {
  const node = {
    tag, className: '', _text: '', children: [], style: {}, attrs: {}, listeners: {}, disabled: false, value: '', placeholder: '', type: '', maxLength: 0,
    classList: { add(c) { node.className += ' ' + c; }, remove() {}, contains(c) { return node.className.split(/\s+/).includes(c); } },
    appendChild(c) { node.children.push(c); return c; },
    setAttribute(k, v) { node.attrs[k] = String(v); },
    addEventListener(k, f) { (node.listeners[k] = node.listeners[k] || []).push(f); },
    querySelector() { return null; },
    remove() {},
    get textContent() { return node._text + node.children.map((c) => c.textContent).join(''); },
    set textContent(v) { node._text = String(v); node.children = []; },
  };
  return node;
}

function makePage() {
  const drawn = [];
  const dialogs = [];
  const sent = [];
  const followed = [];
  const ctx = {
    console: { log() {}, warn: (...a) => dialogs.push('[log] ' + a.join(' ')), error() {} }, JSON, String, Array, Object, Date, Number, Math, RegExp, Set, Map, Promise, setInterval() { return 0; }, clearInterval() {}, setTimeout() { return 0; },
    document: { createElement: el, getElementById: () => null, body: el('body'), querySelectorAll: () => [] },
    navigator: {},
    appendMessage: (e) => drawn.push(e),
    i18n: { t: T, hint(node, key) { node.attrs['data-hint'] = T(key); } },
    peerData: {}, myKey: 'bea', HosRoles: { isAdminRole: () => false },
    handleSyncData() {}, handleChannelAdminFeedback: () => false, channelAdminCmdInFlight: false,
    reconnectDelay: 0, activeDmPartner: null,
    confirm: (m) => { dialogs.push('[confirm] ' + m); return true; },
    prompt: (m) => { dialogs.push('[prompt] ' + m); return 'ok'; },
    alert: (m) => { dialogs.push('[alert] ' + m); },
    WebSocket: { OPEN: 1 },
    ws: { readyState: 1, send: (s) => sent.push(JSON.parse(s)) },
    setFollowLocal: (k, on) => followed.push([k, on]),
    protectedTake: () => true, protectedIsOn: () => false,
    esc: (s) => String(s), // app.js's escaper, which an older chat-social.js called
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(ROOT, 'web/shared/key-card.js'), 'utf8'), ctx);
  vm.runInContext('let lastSystemMessageText = "";\n'
    // (A helper missing from an older app.js is left out, so the red run judges behaviour.)
    + ['addSystemMessage', 'drawSystemLine', 'addServerMessage', 'addNotice', 'handleSkillVerifyResponse', 'handleSkillVerifyRequest']
      .filter((n) => ['addSystemMessage', 'addServerMessage', 'handleSkillVerifyResponse', 'handleSkillVerifyRequest'].includes(n) || app.includes('function ' + n + '(')).map((n) => body(app, n)).join('\n')
    // The app's voice (appLine, the one way to its mark since the eighth review) and its helpers,
    // or, in an app.js from before it, the helpers alone.
    + '\n' + (app.includes('function appLine(') ? require('./app-line-real.js').appVoiceSource()
      : ['serverWords', 'shownName', 'formatCountdown'].filter((n) => app.includes('function ' + n + '(')).map((n) => body(app, n)).join('\n'))
    + '\nvar handleMessage = async function (msg) { switch (msg.type) { ' + sysCase + ' } };', ctx);
  vm.runInContext(fs.readFileSync(path.join(ROOT, 'web/chat/chat-game-admin.js'), 'utf8'), ctx);
  vm.runInContext(social.slice(0, social.indexOf('function updateFriendIndicators()')), ctx);
  return { ctx, drawn, dialogs, sent, followed, text: () => drawn.map((d) => d.textContent) };
}

const FORGED = "Your key's fingerprint: 3fa2 91c0 77de 0b14 c2a9 5e61. When the owner names you, read the groups they ask for from here.";
const mark = T('chat.from_server', { text: '' });
/** Every line drawn that holds `needle` carries the server's mark and class, never the app's. */
function allMarked(page, needle) {
  const hits = page.drawn.filter((d) => d.textContent.includes(needle));
  assert.ok(hits.length >= 1, 'drawn: ' + JSON.stringify(page.text()));
  for (const d of hits) {
    assert.ok(d.textContent.includes(mark), 'marked as the server\'s words: ' + d.textContent);
    assert.ok(d.className.includes('server-line') && !d.className.includes('app-line'), 'the server\'s class, never the app\'s mark: ' + d.className);
  }
}

// Since the eighth review (2026-11-22) the mark comes only through appLine: addSystemMessage draws
// the plain style, whoever calls it (app-voice-sweep-web.test.js sweeps every message type).
test('the app\'s own line carries the app\'s mark; a plain system frame is the server\'s, marked and without it', async () => {
  const page = makePage();
  page.ctx.appLine('owner.claim_help');
  assert.ok(page.drawn[0].className.split(' ').includes('app-line'), page.drawn[0].className);
  page.ctx.addSystemMessage('Scratch pad has 3 message(s).');
  assert.ok(!page.drawn[1].className.split(' ').includes('app-line'), 'addSystemMessage draws no mark: ' + page.drawn[1].className);
  await page.ctx.handleMessage({ type: 'system', message: FORGED });
  allMarked(page, FORGED);
});

test('A: "__game__:{game_admin_error}" draws the relay\'s text marked as the server\'s, never in the app\'s voice', async () => {
  const page = makePage();
  await page.ctx.handleMessage({ type: 'system', message: '__game__:' + JSON.stringify({ type: 'game_admin_error', message: FORGED }) });
  allMarked(page, FORGED);
  await page.ctx.handleMessage({ type: 'system', message: '__game__:' + JSON.stringify({ type: 'game_admin_notice', message: 'Plot 4 released.\n• ' + FORGED }) });
  const last = page.drawn[page.drawn.length - 1].textContent;
  assert.ok(!last.includes('\n'), 'no line break of the relay\'s starts a line of its own: ' + JSON.stringify(last));
});

test('B: "__skill_verify_resp__" draws the relay\'s note marked as the server\'s', async () => {
  const page = makePage();
  await page.ctx.handleMessage({ type: 'system', message: '__skill_verify_resp__:' + JSON.stringify({ approved: true, from_name: 'Account check', skill_id: 'identity', note: FORGED }) });
  allMarked(page, FORGED);
});

test('C: "__skill_verify_req__" opens no browser dialog; the request is a chat line in the server\'s words with Endorse and Not now', async () => {
  const page = makePage();
  await page.ctx.handleMessage({ type: 'system', message: '__skill_verify_req__:' + JSON.stringify({ from_key: 'cal', from_name: FORGED + ' Also,', skill_id: 'x', level: 1 }) });
  assert.deepStrictEqual(page.dialogs.filter((d) => !d.startsWith('[log]')), [], 'no confirm(), prompt() or alert()');
  allMarked(page, FORGED);
  const line = page.drawn.find((d) => d.textContent.includes(FORGED));
  const buttons = line.children.filter((c) => c.tag === 'button');
  assert.deepStrictEqual(buttons.map((b) => b.textContent), [T('skill_verify.endorse'), T('skill_verify.decline')]);
  assert.ok(buttons.every((b) => b.attrs['data-hint']), 'each control has its hint');
  const note = line.children.find((c) => c.tag === 'input');
  note.value = 'We built the greenhouse together.';
  await buttons[0].listeners.click[0]();
  assert.deepStrictEqual(page.sent, [{ type: 'skill_verify_response', skill_id: 'x', to_key: 'cal', approved: true, note: 'We built the greenhouse together.' }]);
  assert.ok(line.textContent.includes(T('skill_verify.sent')));
});

test('friend codes: an answer nobody asked for is dropped (nobody followed, nothing drawn); the one asked for is the server\'s words', async () => {
  const page = makePage();
  await page.ctx.handleMessage({ type: 'friend_code_result', success: true, owner_key: 'r0', name: FORGED });
  await page.ctx.handleMessage({ type: 'friend_code_response', code: 'Q7RT2ZKP' });
  assert.deepStrictEqual(page.followed, [], 'an unasked result follows nobody');
  assert.deepStrictEqual(page.text(), [], 'and nothing is drawn');
  assert.ok(page.dialogs.some((d) => d.includes('did not ask for')), 'it is logged');
  page.ctx.redeemFriendCode('AB12CD34');
  await page.ctx.handleMessage({ type: 'friend_code_result', success: true, owner_key: 'ann', name: FORGED });
  assert.deepStrictEqual(page.followed, [['ann', true]], 'the redeem asked for: its owner followed');
  allMarked(page, FORGED);
  await page.ctx.handleMessage({ type: 'friend_code_result', success: true, owner_key: 'r1', name: 'Ann' });
  assert.deepStrictEqual(page.followed, [['ann', true]], 'one answer per ask');
  page.ctx.sendFriendCodeRequest();
  await page.ctx.handleMessage({ type: 'friend_code_response', code: 'Q7RT (' + FORGED + ')' });
  assert.ok(!page.text().some((t) => t.includes('Q7RT (')), 'a code not shaped like one is not drawn');
  page.ctx.sendFriendCodeRequest();
  await page.ctx.handleMessage({ type: 'friend_code_response', code: 'Q7RT2ZKP' });
  allMarked(page, 'Q7RT2ZKP');
});

test('a name from the network is written into the app\'s own words only when an honest relay could list it', () => {
  const page = makePage();
  assert.strictEqual(page.ctx.shownName('Ada'), 'Ada');
  for (const bad of ['Ada. ' + FORGED, 'System', 'Sy5tem', '', null]) assert.strictEqual(page.ctx.shownName(bad), T('owner.name_refused'), String(bad));
});

test('no route in the chat scripts hands relay text to the app\'s voice in a browser dialog', () => {
  for (const f of ['app.js', 'chat-game-admin.js', 'chat-social.js']) {
    const src = fs.readFileSync(path.join(ROOT, 'web/chat', f), 'utf8');
    assert.ok(!/confirm\(msg\)|confirm\(`\$\{d\./.test(src), f + ': no confirm() of a relayed request');
  }
});
