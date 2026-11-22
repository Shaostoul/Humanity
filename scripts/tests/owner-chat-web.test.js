// The server owner in the website's chat (2026-11-20, docs/design/hide-and-keep.md, "How the owner
// role is set"): web/chat/chat-owner.js, drawn on its own (Claim ownership) and inside Held copies
// (chat-held.js).
//
// Run: node --test scripts/tests/owner-chat-web.test.js   (in `just rig-tests`)
//
// OWNER CHANGES HAPPEN ONLY IN THE DESKTOP APP (the owner branch's seventh review, M2, 2026-11-21):
// this page is served by the server being checked, so a server that was not honest could send a page
// showing the wrong fingerprint or a check that always matches. The website keeps Claim ownership and
// read-only views, says why in words, and draws none of a check frame's text (the seventh review's
// G1 and G2: a free-text `items[].id` reached the check prompt's own words). The desktop app's flows
// are tested in src/engine/owner_tests.rs and src/net/owner.rs.
//
// Both real scripts run in one node:vm page with a small DOM of its own (elements, text, listeners;
// no innerHTML at all), real words from data/i18n/en.json, the real /shared/key-card.js and
// /shared/own-fingerprint.js, and the vendored noble hash. The people are made from the seeds of
// scripts/tests/fixtures/held-v4.json ([7; 32] the owner Ola, [6; 32] the member Bea), [5; 32]
// (Cal) and [3; 32] (R, the relay's own key).
//
// Seen red 2026-11-21 against the branch before this round (scratchpad ownfix10-red-owner.log):
// "the owner sees why the controls are not here" failed (the page drew the member pickers and Hand
// over), "G1" and "G2" failed (the check prompt drew Check and accept, and after it R's fingerprint
// from the free-text id), and "my own fingerprint says how far to trust it" failed (no such words).
'use strict';
const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { pathToFileURL } = require('node:url');

const REPO = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, 'web');
const FX = JSON.parse(fs.readFileSync(path.join(REPO, 'scripts', 'tests', 'fixtures', 'held-v4.json'), 'utf8'));
const FX_OWNER = JSON.parse(fs.readFileSync(path.join(REPO, 'scripts', 'tests', 'fixtures', 'held-owner-v2.json'), 'utf8'));
const K = require(path.join(REPO, 'web', 'shared', 'key-card.js'));
const enc = new TextEncoder();
const CALLER_DOMAIN = 'hum/held-caller/v1';
const purposeOf = (route, grant, id, part) => CALLER_DOMAIN + '\nheld_' + route + ':' + grant + ':' + id + ':' + part + ':0';

const EN = (() => {
  const out = {};
  const walk = (o, p) => { for (const [k, v] of Object.entries(o)) { if (typeof v === 'string') out[p + k] = v; else if (v && typeof v === 'object') walk(v, p + k + '.'); } };
  walk(JSON.parse(fs.readFileSync(path.join(REPO, 'data', 'i18n', 'en.json'), 'utf8')), '');
  return out;
})();
const unmark = (s) => String(s).replace(/\[\[([^\]|]+)(?:\|[^\]]+)?\]\]/g, '$1');
const fill = (s, vars) => String(s).replace(/\{\{(\w+)\}\}/g, (m, k) => (vars && k in vars ? String(vars[k]) : m));
const T = (key, vars) => (key in EN ? fill(unmark(EN[key]), vars) : key);

// ── A small DOM ──
class El {
  constructor(tag) {
    this.tagName = String(tag).toUpperCase(); this.children = []; this.parentNode = null; this.style = {}; this.dataset = {};
    this.attrs = {}; this.listeners = {}; this.className = ''; this._text = ''; this.value = ''; this.disabled = false; this.id = '';
    const self = this;
    this.classList = { add(c) { if (!self.className.split(' ').includes(c)) self.className = (self.className + ' ' + c).trim(); }, remove(c) { self.className = self.className.split(' ').filter((x) => x !== c).join(' '); }, contains(c) { return self.className.split(' ').includes(c); } };
  }
  get textContent() { return this._text + this.children.map((c) => c.textContent).join(''); }
  set textContent(v) { this._text = String(v); this.children = []; }
  get firstChild() { return this.children[0] || null; }
  appendChild(c) { if (c.parentNode) c.parentNode.removeChild(c); c.parentNode = this; this.children.push(c); return c; }
  insertBefore(c, ref) { if (c.parentNode) c.parentNode.removeChild(c); const i = this.children.indexOf(ref); c.parentNode = this; if (i < 0) this.children.push(c); else this.children.splice(i, 0, c); return c; }
  removeChild(c) { const i = this.children.indexOf(c); if (i >= 0) this.children.splice(i, 1); c.parentNode = null; return c; }
  remove() { if (this.parentNode) this.parentNode.removeChild(this); }
  setAttribute(k, v) { this.attrs[k] = String(v); }
  getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
  addEventListener(type, fn) { (this.listeners[type] = this.listeners[type] || []).push(fn); }
  fire(type, extra) { for (const fn of this.listeners[type] || []) fn({ target: this, stopPropagation() {}, ...(extra || {}) }); }
  click() { if (!this.disabled) this.fire('click'); }
  matches(sel) { return sel.startsWith('.') ? this.className.split(/\s+/).includes(sel.slice(1)) : this.tagName === sel.toUpperCase(); }
  querySelectorAll(sel) { const out = []; const walk = (n) => { for (const c of n.children) { if (c.matches(sel)) out.push(c); walk(c); } }; walk(this); return out; }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
  get innerHTML() { throw new Error('innerHTML read'); }
  set innerHTML(_v) { throw new Error('innerHTML written'); }
}
function makeDocument() {
  const body = new El('body');
  const find = (n, id) => { for (const c of n.children) { if (c.id === id) return c; const f = find(c, id); if (f) return f; } return null; };
  return { body, createElement: (t) => new El(t), getElementById: (id) => find(body, id) };
}
const buttons = (root) => root.querySelectorAll('button');
const buttonTexts = (root) => buttons(root).map((b) => b.textContent);
const button = (root, text) => buttons(root).find((b) => b.textContent === text);
function type(root, cls, value) {
  const box = root.querySelector(cls);
  assert.ok(box, 'there is a box ' + cls);
  box.value = value;
  box.fire('input');
}

let NOBLE = null;
let H = null;
async function load() {
  if (!NOBLE) {
    NOBLE = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
    H = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'held-seal.js')).href);
  }
  return { m: NOBLE, H };
}
const hex = (u8) => Buffer.from(u8).toString('hex');
const derive = (m, ctx, bytes) => m.blake3.create({ context: enc.encode(ctx), dkLen: 32 }).update(bytes).digest();
function person(m, seedHex, name) {
  const dil = m.ml_dsa65.keygen(derive(m, 'hum/dilithium3/v1', Buffer.from(seedHex, 'hex')));
  return { name, key: hex(dil.publicKey) };
}
async function people() {
  const { m } = await load();
  return {
    ola: person(m, FX.seed_hex, 'Ola'),          // the owner, [7; 32]
    bea: person(m, FX.second_seed_hex, 'Bea'),   // a member, [6; 32]
    cal: person(m, '05'.repeat(32), 'Cal'),      // an admin
    r: person(m, '03'.repeat(32), 'Ola'),        // the relay's own key
  };
}
/** A key's fingerprint, by hand (BLAKE3 derive_key "hum/key-fingerprint/v1", 12 bytes in groups of
 *  four hex digits), to search what a page drew for it. */
const fpOf = (key) => hex(derive(NOBLE, 'hum/key-fingerprint/v1', Buffer.from(key, 'hex')).subarray(0, 12)).match(/.{4}/g).join(' ');
async function until(cond, what) {
  for (let i = 0; i < 400; i++) { if (cond()) return; await new Promise((r) => setImmediate(r)); }
  assert.fail('timed out waiting for ' + what);
}
const AT = 1763600000000;

async function loadPage({ role, me, peers, host }) {
  await load();
  const document = makeDocument();
  const chat = new El('div');
  chat.id = 'messages';
  document.body.appendChild(chat);
  // The Account and Identity menu's place for my own fingerprint (web/chat/index.html).
  const myFp = new El('div');
  myFp.id = 'my-fingerprint';
  document.body.appendChild(myFp);
  const sock = { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); } };
  const serverLines = [];
  const fetched = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {} }, document, setTimeout, clearTimeout, crypto: globalThis.crypto, TextEncoder, TextDecoder, URL, Blob,
    i18n: { t: T, hint(el, key) { el.setAttribute('title', T(key)); }, tn: (key, n) => T(key + (n === 1 ? '.one' : '.other'), { n }), fill(el, key, vars) { el.textContent = T(key, vars); return el; } },
    addSystemMessage: () => {},
    addServerMessage: (text) => serverLines.push(String(text)),
    appendMessage: (el) => chat.appendChild(el),
    reportsMyRole: () => role,
    reportsView: { tab: 'open', notes: {} },
    requestReports: () => true,
    handleMessage: () => {},
    fetch: (url) => { fetched.push(String(url)); return Promise.reject(new Error('no network in tests')); },
    peerData: peers || {},
    HosKeyCard: K,
    HosRoles: require(path.join(WEB, 'shared', 'roles.js')),
    location: { host: host || 'server-a.test' },
  };
  ctx.window = ctx;
  // A dynamic import() inside a vm script needs --experimental-vm-modules: each script's
  // `import('/shared/...')` is pointed at the same file through a function of the page's.
  ctx.__import = (spec) => import(pathToFileURL(path.join(REPO, 'web', spec.replace(/^\//, ''))).href);
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, 'shared', 'own-fingerprint.js'), 'utf8'), ctx, { filename: 'shared/own-fingerprint.js' });
  for (const file of ['chat-held.js', 'chat-owner.js']) {
    const src = fs.readFileSync(path.join(WEB, 'chat', file), 'utf8');
    vm.runInContext(src.replace(/import\('\/shared\//g, "__import('/shared/"), ctx, { filename: 'chat/' + file });
  }
  vm.runInContext('(s, k) => { ws = s; myKey = k; }', ctx)(sock, me.key);
  if (typeof ctx.hosShowMyFingerprint === 'function') ctx.hosShowMyFingerprint();
  return { ctx, document, sock, chat, serverLines, fetched, handle: (msg) => ctx.handleMessage(msg), sent: (t) => sock.sent.filter((f) => f.type === t) };
}

function peersOf(P, extra) {
  const out = {};
  for (const p of [P.ola, P.bea, P.cal]) out[p.key] = { public_key: p.key, display_name: p.name, role: p === P.ola ? 'owner' : (p === P.cal ? 'admin' : '') };
  return Object.assign(out, extra || {});
}

function heldList({ owner, successor, sealedToSuccessor, log }) {
  const items = [1, 2].map((id) => ({ id, report_id: 4 + id, file_count: id === 1 ? 1 : 0, parts: id === 1 ? 2 : 1, sealed_bytes: 4096, held_at: AT, held_by_name: 'Ada', delete_after: AT + 3e10, legal_hold_until: 0, deleted_at: AT + 3e10, exported_count: 0, sealed_to_owner: true, sealed_to_successor: !!sealedToSuccessor, package: owner ? 'a1'.repeat(16) : undefined }));
  return {
    type: 'held_items', can_keep: true, cannot_keep_why: '', i_am_owner: owner, items, actions: [], counts: { held: 2, exported: 0, legal_holds: 0, destroyed: 0, restored: 0 },
    successor: successor ? { key: successor.key, name: successor.name } : null,
    owner_log: log || [],
  };
}

test('script stays DOM-only, signs and fetches nothing, and every word it asks for is in en.json', () => {
  const src = fs.readFileSync(path.join(WEB, 'chat', 'chat-owner.js'), 'utf8');
  const code = src.split('\n').filter((l) => !l.trim().startsWith('//')).join('\n');
  assert.ok(!/innerHTML|outerHTML|insertAdjacentHTML|document\.write/.test(code), 'no markup from strings');
  assert.ok(!/data-on/.test(code), 'no data-on-* handlers');
  assert.ok(!/pqSignMessage|owner_change_begin|owner_change_accept|owner_change_decline|owner_change_finish|fetch\(/.test(code), 'no owner change is signed, begun, accepted or fetched here');
  const keys = [...code.matchAll(/'((?:chat_owner|held|owner|chat)\.[a-z_]+)'/g)].map((x) => x[1]);
  assert.ok(keys.length > 20, 'it asks for its words by key');
  for (const k of keys) assert.ok(k in EN, k + ' is in en.json');
});

test('wiring: index.html loads it after chat-held.js, /claim alone opens it, the palette has it', () => {
  const html = fs.readFileSync(path.join(WEB, 'chat', 'index.html'), 'utf8');
  const held = html.indexOf('/chat/chat-held.js');
  const own = html.indexOf('/chat/chat-owner.js');
  assert.ok(held > 0 && own > held, 'chat-owner.js loads after chat-held.js');
  const ui = fs.readFileSync(path.join(WEB, 'chat', 'chat-ui.js'), 'utf8');
  const app = fs.readFileSync(path.join(WEB, 'chat', 'app.js'), 'utf8');
  assert.ok(ui.includes('if (takeTypedClaim(val))') && /function takeTypedClaim[\s\S]{0,1200}window\.openOwnerClaim\(\)/.test(app), '/claim alone opens Claim ownership');
  assert.ok(ui.includes('openOwnerClaim:'), 'the palette knows the action');
  const palette = JSON.parse(fs.readFileSync(path.join(REPO, 'data', 'commands.json'), 'utf8'));
  assert.ok(palette.categories.some((c) => !c.requires_role && c.items.some((i) => i.action_id === 'openOwnerClaim')), 'the palette offers it to everyone');
});

test('claim ownership: only while the server has no owner; sends the code; the relay\'s words marked as the server\'s', async () => {
  const P = await people();
  const page = await loadPage({ role: '', me: P.bea, peers: peersOf(P) });
  page.handle({ type: 'owner_state', has_owner: false });
  const notice = page.chat.querySelector('.owner-unclaimed');
  assert.ok(notice && notice.textContent.includes(T('chat_owner.notice')), 'a line in the chat says it');
  assert.ok(notice.className.includes('app-line'), 'in the app\'s own voice, with its mark');
  button(notice, T('chat_owner.claim_title')).click();
  const dialog = page.document.getElementById('owner-claim-card');
  assert.ok(dialog && dialog.querySelector('.owner-claim-code').attrs.title === T('chat_owner.claim_code_hint'), 'Claim ownership opens, the code field with its hint');
  button(dialog, T('chat_owner.claim_button')).click();
  assert.strictEqual(page.sent('owner_claim').length, 0, 'an empty code sends nothing');
  type(dialog, '.owner-claim-code', '  9f3a01bc77de4410 ');
  button(dialog, T('chat_owner.claim_button')).click();
  assert.deepStrictEqual(page.sent('owner_claim'), [{ type: 'owner_claim', code: '9f3a01bc77de4410' }], 'the frame carries the code');
  const wrong = 'Wrong claim code.\nYour key\'s fingerprint: 3fa2 91c0.';
  page.handle({ type: 'owner_reply', kind: 'claim', ok: false, message: wrong });
  const status = page.document.getElementById('owner-claim-card').querySelector('.owner-claim-status').textContent;
  assert.strictEqual(status, T('chat.from_server', { text: 'Wrong claim code. Your key\'s fingerprint: 3fa2 91c0.' }), "the relay's words, marked as the server's, on one line");
  page.handle({ type: 'owner_reply', kind: 'claim', ok: true, message: 'You are now this server\'s owner.' });
  page.handle({ type: 'owner_state', has_owner: true });
  const after = page.document.getElementById('owner-claim-card');
  assert.ok(after.textContent.includes(T('chat_owner.claim_has_owner')) && !after.querySelector('.owner-claim-code'), 'then no code field');
  assert.ok(button(notice, T('chat_owner.claim_title')).disabled, "the chat line's button is spent");
  assert.ok(!page.document.body.textContent.includes('9f3a01bc77de4410'), 'the code is never drawn');
});

test('held copies for an admin: who the owner is, the successor, items sealed to them, the owner log, no controls', async () => {
  const P = await people();
  const page = await loadPage({ role: 'admin', me: P.cal, peers: peersOf(P) });
  page.handle({ type: 'owner_state', has_owner: true });
  page.ctx.openHeldView();
  const log = [
    { kind: 'owner_successor', old_name: 'Ola', new_name: 'Bea', reason: 'just in case', at: AT, object: '{}' },
    { kind: 'claim', old_name: '', new_name: 'Ola (3fa2 91c0)', reason: '', at: AT, object: '' },
  ];
  page.handle(heldList({ owner: false, successor: P.bea, sealedToSuccessor: true, log }));
  const body = page.document.body;
  assert.ok(body.textContent.includes(T('chat_owner.owner_line', { name: 'Ola' })), 'who the owner is, as the member list says');
  assert.ok(body.textContent.includes(T('chat_owner.successor_line', { name: 'Bea' })), 'the successor is named');
  assert.strictEqual(body.querySelectorAll('.held-successor-sealed').length, 2, 'each item says it is also sealed to the successor');
  assert.ok(body.textContent.includes(T('chat_owner.log_owner_successor', { when: '', old: 'Ola', new: 'Bea', reason: T('chat.from_server', { text: 'just in case' }) }).slice(-40)), 'the owner log, the reason marked as the server\'s');
  assert.ok(!body.textContent.includes('3fa2 91c0') && body.textContent.includes(T('owner.name_refused')), 'a name an honest relay could not list is a refused name');
  assert.ok(!body.textContent.includes(T('chat_owner.desktop_only_title')), 'the desktop-only words are for the owner');
  assert.ok(!body.querySelector('.owner-pick') && !buttons(body).some((b) => /hand over|successor/i.test(b.textContent)), 'no member picker and no owner control');
});

test('the owner sees why the controls are not here: owner changes happen in the desktop app, in words; nothing to pick, sign or send', async () => {
  const P = await people();
  const page = await loadPage({ role: 'owner', me: P.ola, peers: peersOf(P) });
  page.handle({ type: 'owner_state', has_owner: true });
  page.ctx.openHeldView();
  page.handle(heldList({ owner: true, successor: P.bea, sealedToSuccessor: true }));
  const body = page.document.body;
  assert.ok(body.textContent.includes(T('chat_owner.owner_line_me')), 'you are the owner');
  assert.ok(body.textContent.includes(T('chat_owner.desktop_only_title')) && body.textContent.includes(T('chat_owner.desktop_only')), 'the words say why: this page comes from this server');
  assert.ok(T('chat_owner.desktop_only').includes('cannot check this server') && T('chat_owner.desktop_only').includes('desktop app'), 'and what to use instead');
  assert.ok(!body.querySelector('.owner-pick') && !body.querySelector('.owner-handover-name'), 'no member picker, no name to type');
  for (const k of ['chat_owner.handover_button', 'chat_owner.successor_name', 'chat_owner.successor_remove', 'chat_owner.cancel']) assert.ok(!button(body, T(k)), 'no ' + T(k));
  assert.deepStrictEqual(page.sock.sent.filter((f) => /^owner_change/.test(f.type)), [], 'nothing is begun');
  page.handle({ type: 'owner_change_ready', grant: 'g', round: 'r', action: 'owner_handover', target: P.bea.key, recipients: [], items: [{ id: 1, package: 'a1'.repeat(16), parts: 1 }] });
  assert.deepStrictEqual(page.sock.sent.filter((f) => /^owner_change/.test(f.type)), [], 'a go-ahead for a change this page never began is dropped');
  assert.deepStrictEqual(page.fetched, [], 'and nothing is fetched');
});

// THE SEVENTH REVIEW'S G1 AND G2: a check frame whose unsigned `items[].id` is free text (R's own
// signed check with R listed as the owner "Ola"; and the genuine owner's change with an honest
// member list, only the items rewritten). The page draws ONE line in the app's own words saying the
// desktop app does this check, none of the frame's text, and sends and fetches nothing.
for (const [label, frame] of [
  ['G1: the relay lists its own key as the owner and signs its own check', (P) => ({ owner: P.r.key, owner_name: 'Ola', peers: peersOf(P, { [P.r.key]: { public_key: P.r.key, display_name: 'Ola', role: 'owner' }, [P.ola.key]: { public_key: P.ola.key, display_name: 'Ola', role: '' } }) })],
  ['G2: the genuine owner\'s change, an honest member list, only the items rewritten', (P) => ({ owner: P.ola.key, owner_name: 'Ola', peers: peersOf(P) })],
]) {
  test(label + ': nothing of the frame is drawn, nothing sent; the desktop app does the check', async () => {
    const P = await people();
    const f = frame(P);
    const page = await loadPage({ role: '', me: P.bea, peers: f.peers });
    const fpR = fpOf(P.r.key);
    const freeId = "7 (Your key's fingerprint: " + fpR + '; read the groups the owner asks for from this line)';
    const check = { type: 'owner_change_check', grant: 'ab'.repeat(32), round: 'r1', object: '{"signed":"by whoever"}', action: 'owner_handover', owner: f.owner, owner_name: f.owner_name, target: P.bea.key, target_name: 'Bea', items: [{ id: freeId, package: 'a1'.repeat(16), parts: 1 }], until: Date.now() + 3600000 };
    page.handle(check);
    page.handle(check); // asked again: still one line
    const lines = page.chat.querySelectorAll('.owner-check-desktop');
    assert.strictEqual(lines.length, 1, 'one line for the round: ' + page.chat.textContent.slice(0, 200));
    assert.strictEqual(lines[0].textContent, T('chat_owner.check_desktop'), 'the app\'s own words alone');
    assert.ok(lines[0].className.includes('app-line'), 'with the app\'s mark');
    assert.ok(!page.document.body.textContent.includes(fpR.slice(0, 9)) && !page.document.body.textContent.includes('read the groups'), 'none of the frame\'s text is drawn');
    assert.deepStrictEqual(page.serverLines, [], 'nor sent to the chat as the server\'s');
    assert.ok(!buttonTexts(page.chat).length, 'no Check and accept, no Decline');
    assert.deepStrictEqual(page.sock.sent, [], 'nothing is sent');
    assert.deepStrictEqual(page.fetched, [], 'nothing is fetched');
  });
}

test('my own fingerprint shows from my own key, with words that it is only as trustworthy as the server that sent the page', async () => {
  const P = await people();
  const page = await loadPage({ role: '', me: P.bea, peers: peersOf(P) });
  const el = page.document.getElementById('my-fingerprint');
  await until(() => el.querySelector('.own-fp-line'), 'my fingerprint is drawn');
  assert.ok(el.textContent.includes(fpOf(P.bea.key)), 'my own key\'s fingerprint');
  assert.strictEqual(el.querySelector('.own-fp-trust').textContent, T('chat_owner.my_fp_server_page'), 'and how far to trust it');
});

test('the held caller\'s purpose is the Rust relay\'s, with its domain line (held-owner-v2.json, Rust writes it)', async () => {
  const { H: seal } = await load();
  for (const c of FX_OWNER.caller) assert.strictEqual(seal.callerPurpose(c.route, c.grant, c.id, c.part, c.offset), c.purpose, 'the page signs what the relay checks: ' + c.route);
  assert.strictEqual(seal.callerPurpose('header', 'ab'.repeat(32), 1, 0, 0), purposeOf('header', 'ab'.repeat(32), 1, 0));
});

test('the parity exception is written down where the next person looks', () => {
  for (const doc of ['docs/design/hide-and-keep.md', 'docs/design/key-cards.md']) {
    const text = fs.readFileSync(path.join(REPO, doc), 'utf8');
    assert.ok(/desktop app only/i.test(text) && /parity exception/i.test(text), doc + ' says owner changes are the desktop app\'s, and why');
  }
});
