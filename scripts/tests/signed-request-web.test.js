// The website's signed REQUESTS, apart from its chat messages (2026-11-20).
//
// Run: node --test scripts/tests/signed-request-web.test.js   (in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/ (how each test was seen red).
//
// THE FAULT IT GUARDS: every identity-keyed request (an account export, a vault sync, a channel
// read, a group read, a bug vote, an admin action, a Files page action) was signed over
// `purpose\ntimestamp`, the very shape a chat message is signed over (`text\ntimestamp`, the same
// key, the same verify on the relay). So a person who posted the chat line `account_export` handed
// whoever saw that message's signature a working credential for POST /api/account/export, and
// most routes took the same signature again and again for five minutes. Now a request is signed
// over `hum/request/v2\n{audience}\n{purpose}\n{timestamp}` (web/chat/pq.js pqRequestPreimage, the
// twin of crates/humanity-protocol/src/signed_request.rs), every route takes a signature once, and
// no chat signer signs a text that begins with a signing domain line.
//
// ROUND 2 (2026-11-21): every signature names the server it is for (the AUDIENCE: the address the
// page talks to), so a server cannot replay a request it received at another server, nor pass
// another server's sign-in challenge through to this page (test 5).
//
// What it proves, with the real pq.js, crypto.js and shared/pq-relay-auth.js run in node:vm:
//  1. pq.js builds the fixture's preimage for every request and sign-in in
//     scripts/tests/fixtures/signed-request-v2.json (the file the Rust side is held to), the
//     fixture's audience for every URL, and refuses exactly the fixture's reserved chat texts.
//     Since round 3 (2026-11-21) the URLs include ones the hand reading and the browser's URL
//     parser read as two servers (a backslash, a trailing dot, IPv4 and IPv6 spellings), which
//     give no audience. Red evidence: with pqAudienceOf's `new URL` comparison taken out, test 1
//     failed at 'audience of "https://evil.example\\@victim.example/"'.
//  2. the standalone pages' signer (getPqSignedAuth) signs the fixture's bytes, a new millisecond
//     for each of two requests started together.
//  3. the chat's signers: pqSignMyRequest signs the request preimage; pqSignChatMessage signs
//     `text\ntimestamp` and refuses to sign a text that is a request's preimage minus its time.
//  4. no request is signed by the chat signer anywhere on the website: every pqSignChatMessage
//     call passes a chat text (`content` or `command`), and no page builds `purpose + '\n' + time`
//     itself.
//  5. a sign-in is signed only for the address the page's socket opened, never one the server
//     names, and only for a nonce of a relay's shape; a request names this page's own server.
//
// RED EVIDENCE (2026-11-20, against web/ as it was before this change, HOS_WEB_DIR at a copy):
//  1: "pq.js has no pqRequestPreimage" (the function did not exist).
//  2: "getPqSignedAuth signs the fixture's request bytes" (actual 'account_export\n1800000000000').
//  3: "pqSignMyRequest exists in crypto.js" (missing), and "a chat text holding a request's
//     preimage is never signed" (it was signed).
//  4: "every chat signer call passes a chat text": web/chat/chat-privacy.js signed 'account_export',
//     chat-profile.js 'system_profile', chat-groups-p2p.js 'p2p_group_read', app.js a channel read,
//     chat-files-card.js a Files purpose, chat-ui.js push_subscribe; and pq-relay-auth.js and
//     trade-app.js built `purpose + '\n' + ts` themselves.
// ROUND 2 RED EVIDENCE (2026-11-21, HOS_WEB_DIR at a copy of web/ from round 1): 1 failed at
// "pq.js has no pqAudienceOf"; 2 at "getPqSignedAuth signs the fixture's request bytes" (actual
// 'hum/request/v1\naccount_export\n1800000000000', no audience line); 3 at "a request is signed
// over the request preimage, for this page's own server" (the same actual); 5 at "pq.js has no
// pqIdentifyResponse".

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const read = (p) => fs.readFileSync(path.join(WEB, p), 'utf8').replace(/\r\n/g, '\n');
const FIXTURE = JSON.parse(fs.readFileSync(path.join(ROOT, 'scripts', 'tests', 'fixtures', 'signed-request-v2.json'), 'utf8'));

/** The text from `start` to the end of the first `end` after it. */
function slice(src, start, end) {
  const i = src.indexOf(start);
  if (i < 0) return null;
  const j = src.indexOf(end, i);
  return src.slice(i, j < 0 ? src.length : j + end.length);
}

/** The request half of pq.js (its domain, preimage, reserved-text check, clock and signer). */
function pqRequestPart() {
  return slice(read('chat/pq.js'), 'const PQ_REQUEST_DOMAIN', '// Exposed globally');
}

/** A page served from `host` with a Dilithium signer that records the bytes it is handed, at a fixed clock. */
function page(now, host) {
  const signed = [];
  const ctx = {
    location: { protocol: 'https:', host: host || 'united-humanity.us' },
    console: { log() {}, warn() {}, error() {} },
    TextEncoder, TextDecoder, atob, btoa, Uint8Array, URL,
    localStorage: { getItem: () => JSON.stringify({ privateKeyPkcs8: btoa(String.fromCharCode(...new Uint8Array(48).fill(7))) }) },
    myDilithiumSecret: new Uint8Array(4),
    pqSignMessage: (secret, bytes) => { signed.push(new TextDecoder().decode(bytes)); return Promise.resolve(new Uint8Array([0xab, 0xcd])); },
    pqDeriveIdentity: () => Promise.resolve({ dilithiumPublicHex: 'd1l1th1um', dilithiumSecret: new Uint8Array(4) }),
  };
  ctx.window = ctx;
  ctx.globalThis = ctx;
  vm.createContext(ctx);
  vm.runInContext('Date.now = function () { return ' + now + '; };', ctx);
  const part = pqRequestPart();
  if (part) vm.runInContext(part, ctx, { filename: 'pq.js#requests' });
  return { ctx, signed };
}

test('1. pq.js builds the fixture\'s request bytes and refuses the fixture\'s reserved chat texts', () => {
  const p = page(1);
  assert.strictEqual(typeof p.ctx.pqRequestPreimage, 'function', 'pq.js has no pqRequestPreimage');
  assert.strictEqual(typeof p.ctx.pqAudienceOf, 'function', 'pq.js has no pqAudienceOf');
  assert.strictEqual(vm.runInContext('PQ_REQUEST_DOMAIN', p.ctx), FIXTURE.domain);
  assert.strictEqual(vm.runInContext('PQ_IDENTIFY_DOMAIN', p.ctx), FIXTURE.identify_domain);
  for (const c of FIXTURE.requests) {
    assert.strictEqual(p.ctx.pqRequestPreimage(c.audience, c.purpose, c.timestamp), c.preimage, 'purpose ' + JSON.stringify(c.purpose));
  }
  for (const c of FIXTURE.identify) {
    assert.strictEqual(p.ctx.pqIdentifyPreimage(c.audience, c.nonce, c.key), c.preimage, 'sign-in for ' + c.audience);
  }
  for (const c of FIXTURE.audiences) {
    assert.strictEqual(p.ctx.pqAudienceOf(c.url), c.audience, 'audience of ' + JSON.stringify(c.url));
  }
  for (const t of FIXTURE.chat_refused) assert.strictEqual(p.ctx.pqIsReservedChatText(t), true, 'refused: ' + JSON.stringify(t));
  for (const t of FIXTURE.chat_allowed) assert.strictEqual(p.ctx.pqIsReservedChatText(t), false, 'allowed: ' + JSON.stringify(t));
  // The guarantee itself: the only chat text whose signature is a request's is a reserved one.
  for (const c of FIXTURE.requests) {
    const chatText = c.preimage.slice(0, c.preimage.length - ('\n' + c.timestamp).length);
    assert.strictEqual(p.ctx.pqIsReservedChatText(chatText), true, 'the chat text that would sign ' + JSON.stringify(c.purpose) + ' is refused');
  }
});

test('2. the standalone pages\' signer signs the fixture\'s bytes, a new millisecond each time', async () => {
  const c = FIXTURE.requests[0];
  const p = page(c.timestamp, c.audience);
  vm.runInContext(read('shared/pq-relay-auth.js'), p.ctx, { filename: 'pq-relay-auth.js' });
  const a = await p.ctx.getPqSignedAuth(c.purpose);
  const b = await p.ctx.getPqSignedAuth(c.purpose);
  assert.ok(a && b, 'both requests are signed');
  assert.strictEqual(p.signed[0], c.preimage, 'getPqSignedAuth signs the fixture\'s request bytes');
  assert.deepStrictEqual([a.key, a.timestamp, a.sig], ['d1l1th1um', c.timestamp, 'abcd']);
  assert.strictEqual(b.timestamp, c.timestamp + 1, 'two requests in one millisecond never share a time (the relay takes each once)');
});

test('3. the chat signs requests as requests and never signs a request\'s bytes as a chat line', async () => {
  const p = page(5, 'chat.example.org:8443');
  const crypto = read('chat/crypto.js');
  const mine = slice(crypto, 'async function pqSignMyRequest(', '\n}\n');
  assert.ok(mine, 'pqSignMyRequest exists in crypto.js');
  vm.runInContext(mine, p.ctx, { filename: 'crypto.js#pqSignMyRequest' });
  vm.runInContext(slice(crypto, 'async function pqSignChatMessage(', '\n}\n'), p.ctx, { filename: 'crypto.js#pqSignChatMessage' });

  const ts = 1800000000000;
  assert.strictEqual(await p.ctx.pqSignMyRequest('account_export', ts), 'abcd');
  assert.strictEqual(p.signed.pop(), 'hum/request/v2\nchat.example.org:8443\naccount_export\n' + ts, 'a request is signed over the request preimage, for this page\'s own server');

  assert.strictEqual(await p.ctx.pqSignChatMessage('hello', ts), 'abcd');
  assert.strictEqual(p.signed.pop(), 'hello\n' + ts, 'a chat message is signed over its text and time');

  const forged = p.ctx.pqRequestPreimage('chat.example.org:8443', 'account_export', ts).slice(0, -('\n' + ts).length);
  assert.strictEqual(await p.ctx.pqSignChatMessage(forged, ts), null, 'a chat text holding a request\'s preimage is never signed');
  assert.strictEqual(p.signed.length, 0, 'nothing was handed to the signer for it');
});

test('4. no request on the website is signed by the chat signer or built by hand', () => {
  const files = [];
  (function walk(dir) {
    for (const e of fs.readdirSync(path.join(WEB, dir), { withFileTypes: true })) {
      const rel = dir ? dir + '/' + e.name : e.name;
      if (e.isDirectory()) { if (e.name !== 'vendor' && e.name !== 'node_modules') walk(rel); }
      else if (/\.(js|html)$/.test(e.name)) files.push(rel);
    }
  })('');
  const wrong = [];
  for (const rel of files) {
    const src = read(rel);
    // Every chat-signer call passes a chat text: the message's content or a slash command.
    for (const m of src.matchAll(/pqSignChatMessage\(\s*([^,)]+)/g)) {
      if (!/^(content|command)$/.test(m[1].trim())) wrong.push(rel + ': pqSignChatMessage(' + m[1].trim());
    }
    // No page builds a purpose-then-time preimage itself (the old request shape). The one text-then-
    // time preimage left is a real chat post: the Files page's "Also post in chat" (files-add.js).
    for (const m of src.matchAll(/encode\(\s*([A-Za-z_]+)\s*\+\s*'\\n'\s*\+\s*(ts|timestamp)\s*\)/g)) {
      if (!(rel === 'pages/files-add.js' && m[1] === 'content')) wrong.push(rel + ': encode(' + m[1] + " + '\\n' + " + m[2] + ')');
    }
  }
  assert.deepStrictEqual(wrong, [], 'every chat signer call passes a chat text, and no request preimage is built by hand');
});

test('5. a sign-in names the address the page opened, and only a relay\'s nonce is signed', async () => {
  const p = page(1, 'united-humanity.us');
  assert.strictEqual(typeof p.ctx.pqIdentifyResponse, 'function', 'pq.js has no pqIdentifyResponse');
  const nonce = '0123456789abcdef'.repeat(4);
  // A server at a.example passes on b.example's challenge: the page signs for a.example, the socket it opened.
  const resp = await p.ctx.pqIdentifyResponse(new Uint8Array(4), 'ab12', 'wss://a.example/ws', nonce);
  assert.ok(resp, 'a relay\'s challenge is answered');
  assert.strictEqual(resp.type, 'identify_response');
  assert.strictEqual(resp.audience, 'a.example', 'the audience is the address opened, nothing the server said');
  assert.strictEqual(p.signed.pop(), 'hum/identify/v2\na.example\n' + nonce + '\nab12', 'the bytes signed name a.example');
  assert.strictEqual(resp.sig_b64, 'q80=', 'the signature, base64');
  for (const bad of ['', 'abc', nonce.toUpperCase(), nonce + 'a', nonce.slice(0, 50) + '\nb.example', 'g'.repeat(64)]) {
    assert.strictEqual(await p.ctx.pqIdentifyResponse(new Uint8Array(4), 'ab12', 'wss://a.example/ws', bad), null, 'not a relay\'s nonce: ' + JSON.stringify(bad));
  }
  assert.strictEqual(await p.ctx.pqIdentifyResponse(new Uint8Array(4), 'ab12', 'not an address', nonce), null, 'no address, no sign-in');
  assert.strictEqual(p.signed.length, 0, 'nothing else was handed to the signer');
  // Every page that answers a challenge does it through pq.js (or relay-session.js's twin), with the socket's own URL.
  for (const rel of ['chat/app.js', 'pages/tasks-app.js', 'pages/trade-app.js']) {
    const src = read(rel);
    assert.ok(/pqIdentifyResponse\(/.test(src), rel + ' answers the challenge through pqIdentifyResponse');
    assert.ok(!/hum\/identify\/v1/.test(src), rel + ' builds no old sign-in words');
  }
  const session = read('shared/relay-session.js');
  assert.ok(/IDENTIFY_DOMAIN = 'hum\/identify\/v2'/.test(session) && /o\.location && o\.location\.host/.test(session), 'relay-session.js signs for the address it opened');
});
