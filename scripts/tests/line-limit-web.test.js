// The website counts a chat line against the limit the relay applies, its role's own, said on
// sign-in (`my_limits`) and with a length refusal (`chat_refused`), not a guessed 2,000
// (2026-10-24). Run: node --test scripts/tests/line-limit-web.test.js
//
// setMyMaxChars and getMaxMsgLength are read out of web/chat/chat-ui.js and run in a vm.
// CHAT_UI=<path> points at another copy. Seen red 2026-10-24 against the chat-ui.js before this:
// "the limit functions are where the test expects them". (The red highlight itself was looked at
// in the browser on a throwaway page running the same code: the first 20 characters plain, the
// rest red on dark red, aligned with the box.)
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const UI = process.env.CHAT_UI || path.join(__dirname, '..', '..', 'web', 'chat', 'chat-ui.js');
const APP = path.join(__dirname, '..', '..', 'web', 'chat', 'app.js');

function limits(role) {
  const src = fs.readFileSync(UI, 'utf8');
  const a = src.indexOf('let myMaxChars = 0;');
  const b = src.indexOf('const MAX_MSG_LENGTH');
  assert.ok(a >= 0 && b > a, 'the limit functions are where the test expects them');
  const ctx = vm.createContext({
    peerData: { me: { role } }, myKey: 'me', Number,
    document: { getElementById: () => null },
  });
  vm.runInContext(src.slice(a, b).replace('let myMaxChars', 'var myMaxChars') + ';globalThis.__l = { setMyMaxChars, getMaxMsgLength };', ctx);
  return ctx.__l;
}

test('before the relay says, the old guess stands; after, its limit is the one counted', () => {
  const l = limits('');
  assert.strictEqual(l.getMaxMsgLength(), 2000, 'the guess before the relay has said');
  l.setMyMaxChars(280);
  assert.strictEqual(l.getMaxMsgLength(), 280, 'an unverified account counts against 280');
  l.setMyMaxChars(0);
  assert.strictEqual(l.getMaxMsgLength(), 280, 'a nonsense limit is ignored');
});

test('the page takes the limit from my_limits and from a length refusal', () => {
  const app = fs.readFileSync(APP, 'utf8');
  assert.match(app, /case 'my_limits':[\s\S]{0,200}setMyMaxChars\(msg\.max_chars\)/);
  assert.match(app, /reason === 'too_long' && msg\.limit\) setMyMaxChars\(msg\.limit\)/);
});
