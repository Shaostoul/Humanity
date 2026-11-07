// My own copy of a private message leaves apart from the recipient's copy, on the website
// (2026-11-06, docs/BUGS.md BUG-259): web/shared/self-mail.js, the twin of the desktop's
// src/net/self_mail.rs.
//
// Run: node --test scripts/tests/self-mail-web.test.js   (pure node, nothing booted or connected)
//
// What this proves:
//  1. The two limits of the wait are the desktop's, read from src/net/self_mail.rs.
//  2. On an attached socket, a `dm_put` to someone else goes at once and one to my own key waits a
//     random while between the limits; several wait in order; a socket that is not open when one
//     is due keeps it for the next socket; leaving the page sends what is waiting at once; a note
//     sent with hosSelfMailSendNow goes at once.
//  3. A real DM, built by chat/crypto.js `pqBuildDmPuts` and sent the way chat/chat-ui.js sends it
//     (recipient copy, then my copy, on `ws`), reaches the server as two puts apart in time.
//  4. Chat attaches every socket it opens (chat/app.js `connect`), and the page loads
//     shared/self-mail.js before app.js.
//
// Seen red 2026-11-06: against the web before this (no shared/self-mail.js) every test failed at
// loading it; with the module in place and the attach line taken out of app.js, test 4 failed
// ("every socket Chat opens is attached").

'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const REPO = path.resolve(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, 'web');
const read = (rel) => fs.readFileSync(path.join(WEB, rel), 'utf8');

/** A page context with shared/self-mail.js loaded (its names land on the context, as on window). */
function page(extra) {
  const ctx = { console, Math, Date, JSON, Uint32Array, Uint8Array, TextEncoder, setTimeout, clearTimeout, ...extra };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(read('shared/self-mail.js'), ctx, { filename: 'shared/self-mail.js' });
  assert.strictEqual(typeof ctx.createSelfMail, 'function', 'shared/self-mail.js defines createSelfMail');
  return ctx;
}

/** A clock and timers a test moves by hand. */
function fakeTime() {
  const t = { now: 0, timers: [] };
  t.setTimer = (fn, ms) => { const id = t.timers.length + 1; t.timers.push({ id, at: t.now + ms, fn }); return id; };
  t.clearTimer = (id) => { t.timers = t.timers.filter((x) => x.id !== id); };
  t.advance = (ms) => {
    const end = t.now + ms;
    for (;;) {
      t.timers.sort((a, b) => a.at - b.at);
      const next = t.timers[0];
      if (!next || next.at > end) break;
      t.timers.shift();
      t.now = next.at;
      next.fn();
    }
    t.now = end;
  };
  return t;
}

/** A socket that records what it puts on the wire, and when. */
function socket(t, open = true) {
  const s = { readyState: open ? 1 : 0, wire: [] };
  s.send = (data) => { s.wire.push({ at: t.now, frame: JSON.parse(data) }); };
  return s;
}

const ME = 'me_key';
const put = (to, content) => JSON.stringify({ type: 'dm_put', to, content });

test('the limits of the wait are the desktop app\'s', () => {
  const rs = fs.readFileSync(path.join(REPO, 'src', 'net', 'self_mail.rs'), 'utf8');
  const ms = (name) => {
    const m = rs.match(new RegExp(`pub const ${name}: u64 = ([0-9_]+);`));
    assert.ok(m, `src/net/self_mail.rs declares ${name}`);
    return Number(m[1].replace(/_/g, ''));
  };
  const ctx = page();
  assert.strictEqual(ctx.SELF_MAIL_DELAY_MIN_MS, ms('SELF_MAIL_DELAY_MIN_MS'));
  assert.strictEqual(ctx.SELF_MAIL_DELAY_MAX_MS, ms('SELF_MAIL_DELAY_MAX_MS'));
  assert.ok(ctx.SELF_MAIL_DELAY_MIN_MS >= 10000, 'long enough for other mail to land between');
});

test('mail to someone goes at once; mail to my own mailbox waits a random while, in order', () => {
  const t = fakeTime();
  const draws = [0, 0.999999, 0.5];
  const ctx = page();
  const q = ctx.createSelfMail({ now: () => t.now, setTimer: t.setTimer, clearTimer: t.clearTimer, random: () => draws.shift() });
  const s = q.attach(socket(t), () => ME);

  s.send(put('bob_key', 'to bob'));
  s.send(put(ME, 'my copy 1'));
  s.send(JSON.stringify({ type: 'chat', content: 'hello' }));
  assert.deepStrictEqual(s.wire.map((w) => w.frame.content || w.frame.type), ['to bob', 'hello'], 'only my own copy waits');

  t.advance(1000);
  s.send(put(ME, 'my copy 2')); // the longest wait
  s.send(put(ME, 'my copy 3')); // a middle wait, but never before copy 2
  t.advance(ctx.SELF_MAIL_DELAY_MIN_MS - 1001);
  assert.strictEqual(s.wire.length, 2, 'nothing of mine before the shortest wait');
  t.advance(1);
  assert.deepStrictEqual(s.wire.map((w) => w.frame.content || w.frame.type), ['to bob', 'hello', 'my copy 1']);
  t.advance(ctx.SELF_MAIL_DELAY_MAX_MS + 2000);
  const mine = s.wire.filter((w) => w.frame.to === ME);
  assert.deepStrictEqual(mine.map((w) => w.frame.content), ['my copy 1', 'my copy 2', 'my copy 3'], 'in the order they were sent');
  assert.ok(mine[1].at >= 1000 + ctx.SELF_MAIL_DELAY_MAX_MS - 1 && mine[1].at <= 1000 + ctx.SELF_MAIL_DELAY_MAX_MS, 'copy 2 waited the longest wait');
  assert.strictEqual(mine[2].at, mine[1].at, 'copy 3 waited for copy 2');
});

test('a copy due while no socket is open goes on the next one; leaving the page sends what waits', () => {
  const t = fakeTime();
  const ctx = page();
  const q = ctx.createSelfMail({ now: () => t.now, setTimer: t.setTimer, clearTimer: t.clearTimer, random: () => 0 });
  const first = q.attach(socket(t), () => ME);
  first.send(put(ME, 'kept'));
  first.readyState = 3; // closed
  t.advance(ctx.SELF_MAIL_DELAY_MAX_MS * 2);
  assert.strictEqual(first.wire.length, 0, 'nothing goes on a closed socket');
  const second = q.attach(socket(t), () => ME);
  t.advance(10);
  assert.deepStrictEqual(second.wire.map((w) => w.frame.content), ['kept'], 'it goes on the next open socket');

  second.send(put(ME, 'leaving'));
  assert.strictEqual(q.flush(), 1, 'leaving the page sends it at once');
  assert.deepStrictEqual(second.wire.map((w) => w.frame.content), ['kept', 'leaving']);

  ctx.hosSelfMailSendNow(second, put(ME, 'block note'));
  assert.strictEqual(second.wire[second.wire.length - 1].frame.content, 'block note', 'a note that pairs with nothing goes at once');
  assert.strictEqual(q.waiting().length, 0, 'nothing left waiting');
});

test('a real DM reaches the server as two puts apart in time', async () => {
  // chat-ui.js sends a DM's two copies on `ws`, recipient first: the order this test sends in.
  assert.match(read('chat/chat-ui.js'), /ws\.send\(JSON\.stringify\(built\.recipientPut\)\);\s*\n\s*ws\.send\(JSON\.stringify\(built\.selfPut\)\);/,
    'chat-ui.js sends the recipient copy, then my copy, on ws');
  const ctx = page({ btoa: (s) => Buffer.from(s, 'binary').toString('base64'), localStorage: { getItem: () => null, setItem() {}, removeItem() {} } });
  vm.runInContext(read('chat/crypto.js'), ctx, { filename: 'chat/crypto.js' });
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes.slice(0, 8));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: Buffer.from(pub).toString('base64'), nonce_b64: 'AAAA', ct_b64: Buffer.from(plaintext).toString('base64') });
  ctx.getPeerEcdhPublic = () => 'kyber-of-bob';
  vm.runInContext(`myDilithiumPublicHex = ${JSON.stringify(ME)}; myDilithiumSecret = new Uint8Array(4); myKyberPublicBase64 = 'kyber-of-me';`, ctx);
  const built = await vm.runInContext('pqBuildDmPuts', ctx)('hello Bob', 'bob_key', 1700000000000);
  assert.ok(built && built.recipientPut && built.selfPut, 'crypto.js builds both copies');

  const t = fakeTime();
  const q = ctx.createSelfMail({ now: () => t.now, setTimer: t.setTimer, clearTimer: t.clearTimer });
  const ws = q.attach(socket(t), () => ME);
  ws.send(JSON.stringify(built.recipientPut));
  ws.send(JSON.stringify(built.selfPut));
  assert.deepStrictEqual(ws.wire.map((w) => w.frame.to), ['bob_key'], 'the recipient copy goes at once, mine does not');
  t.advance(ctx.SELF_MAIL_DELAY_MAX_MS);
  assert.deepStrictEqual(ws.wire.map((w) => w.frame.to), ['bob_key', ME], 'my copy follows');
  const gap = ws.wire[1].at - ws.wire[0].at;
  assert.ok(gap >= ctx.SELF_MAIL_DELAY_MIN_MS && gap <= ctx.SELF_MAIL_DELAY_MAX_MS, `apart by the wait (${gap} ms)`);
  assert.strictEqual(ws.wire[1].frame.content, built.selfPut.content, 'and it is my copy, unchanged');
});

test('every socket Chat opens is attached, and the page loads self-mail.js before app.js', () => {
  const app = read('chat/app.js');
  const opened = app.split('\n').map((l, i) => [l, i]).filter(([l]) => /\bws = new WebSocket\(/.test(l));
  assert.ok(opened.length >= 1, 'app.js opens its socket');
  const lines = app.split('\n');
  for (const [, i] of opened) {
    const next = lines.slice(i + 1, i + 8).join('\n');
    assert.match(next, /hosSelfMail\.attach\(ws, \(\) => myKey\)/, 'every socket Chat opens is attached');
  }
  const html = read('chat/index.html');
  const mail = html.indexOf('/shared/self-mail.js');
  assert.ok(mail > 0 && mail < html.indexOf('/chat/app.js'), 'self-mail.js loads before app.js');
  // The notes that pair with nothing and keep their own "not sent yet" mark go at once.
  assert.match(read('chat/chat-privacy.js'), /hosSelfMailSendNow\(ws, JSON\.stringify\(built\.put\)\)/, 'block notes go at once');
  assert.match(read('chat/chat-notify.js'), /hosSelfMailSendNow\(sock\(\), JSON\.stringify\(built\.put\)\)/, 'allow-list notes go at once');
});
