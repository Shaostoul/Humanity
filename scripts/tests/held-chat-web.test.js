// Hide and keep in the website's chat (2026-11-20, docs/design/hide-and-keep.md rung 5):
// web/chat/chat-held.js, the browser twin of src/gui/pages/server_settings/held.rs.
//
// Run: node --test scripts/tests/held-chat-web.test.js   (in `just rig-tests`)
//
// The real script runs in a node:vm page with a small DOM of its own (elements, text, listeners;
// no innerHTML at all), real words from data/i18n/en.json, the real signing of /shared/pq-object.js
// with the vendored noble Dilithium3, and the real opening of /shared/held-seal.js. What is proved:
//   1. On a report about a post with no owner to seal to, the relay's sentence, what works instead
//      and what to do today are drawn and no Hide and keep button is; with an owner, the button
//      sends the decision with my own signed hide_and_keep, which verifies.
//   2. Held copies: an admin sees the item and the log and no action at all, and nothing anywhere
//      opens an item; the owner gets Legal hold, Export, Restore and Destroy now; Destroy now asks
//      first, saying it cannot be undone, sends nothing without a reason, then my signed
//      held_destroy.
//   3. Export opens the sealed parts here straight into the file the browser saves, and none of
//      the post's words or file bytes are ever drawn.
//   4. The script builds its DOM with createElement, textContent and addEventListener only: no
//      innerHTML, no data-on-* handlers.
// HOS_WEB_DIR points the test at another copy of web/ (used to see it red).
//
// Seen red 2026-11-20, each against a copy of chat-held.js with one change: the button offered
// whatever the server said (`if (true)`): "the relay's sentence is drawn"; the owner check in
// itemNode taken out: "an admin gets no action, and no button opens an item"; the export's status
// line carrying the post's words: "none of it is drawn"; the card emptied with `innerHTML = ''`:
// "no markup from strings" (and the fake DOM throws on any innerHTML).

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { pathToFileURL } = require('node:url');

const REPO = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, 'web');
const enc = new TextEncoder();

// The words, flattened from en.json, filled the way web/shared/i18n.js fills them.
const EN = (() => {
  const out = {};
  const walk = (o, prefix) => { for (const [k, v] of Object.entries(o)) { if (typeof v === 'string') out[prefix + k] = v; else walk(v, prefix + k + '.'); } };
  walk(JSON.parse(fs.readFileSync(path.join(REPO, 'data', 'i18n', 'en.json'), 'utf8')), '');
  return out;
})();
const fill = (s, vars) => String(s).replace(/\{\{(\w+)\}\}/g, (m, k) => (vars && k in vars ? String(vars[k]) : m));
const T = (key, vars) => (key in EN ? fill(EN[key], vars) : key);

// ── A small DOM ──
class El {
  constructor(tag) {
    this.tagName = String(tag).toUpperCase(); this.children = []; this.parentNode = null; this.style = {}; this.dataset = {};
    this.attrs = {}; this.listeners = {}; this.className = ''; this._text = ''; this.value = ''; this.disabled = false; this.id = '';
    const self = this;
    this.classList = { add(c) { if (!self.className.split(' ').includes(c)) self.className = (self.className + ' ' + c).trim(); }, remove(c) { self.className = self.className.split(' ').filter((x) => x !== c).join(' '); } };
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
  fire(type) { for (const fn of this.listeners[type] || []) fn({ target: this, stopPropagation() {} }); }
  click() { this.fire('click'); }
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

let NOBLE = null;
async function noble() {
  if (!NOBLE) NOBLE = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
  return NOBLE;
}
function keysOf(m, byte) {
  const seed = new Uint8Array(32).fill(byte);
  const dil = m.ml_dsa65.keygen(m.blake3.create({ context: enc.encode('hum/dilithium3/v1'), dkLen: 32 }).update(seed).digest());
  const kyb = m.ml_kem768.keygen(m.blake3.create({ context: enc.encode('hum/kyber768/v1'), dkLen: 64 }).update(seed).digest());
  return { dil, kyb, key: Buffer.from(dil.publicKey).toString('hex') };
}
async function until(cond, what) {
  for (let i = 0; i < 4000; i++) { if (cond()) return; await new Promise((r) => setImmediate(r)); }
  assert.fail('never happened: ' + what);
}

const DID = 'did:hum:held-web-test';
const NO_OWNER = 'This server has no owner set, so there is nobody a copy could be sealed to. Nothing was kept.';

async function loadPage(role, extra = {}) {
  const m = await noble();
  const me = keysOf(m, 9);
  const document = makeDocument();
  const sock = { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); } };
  const said = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {} }, document, setTimeout, clearTimeout, crypto: globalThis.crypto, TextEncoder, TextDecoder, URL, Blob,
    i18n: { t: T, hint(el, key) { el.setAttribute('title', T(key)); }, tn: (key, n) => T(key + (n === 1 ? '.one' : '.other'), { n }) },
    addSystemMessage: (t) => said.push(t),
    reportsMyRole: () => role,
    reportsView: { tab: 'open', notes: {} },
    requestReports: () => true,
    handleMessage: () => {},
    fetch: extra.fetch || (() => Promise.reject(new Error('no network in tests'))),
    ...extra.globals,
  };
  ctx.window = ctx;
  ctx.window.hosServerDid = DID;
  ctx.window.pqSignMessage = async (secret, bytes) => m.ml_dsa65.sign(bytes, secret);
  // A dynamic import() inside a vm script needs --experimental-vm-modules, which `node --test`
  // is not run with: the script's three `import('/shared/...')` calls are pointed at the same
  // files through a function of the page's (the only change made to the script).
  ctx.__import = (spec) => import(pathToFileURL(path.join(REPO, 'web', spec.replace(/^\//, ''))).href);
  vm.createContext(ctx);
  const src = fs.readFileSync(path.join(WEB, 'chat', 'chat-held.js'), 'utf8');
  assert.strictEqual((src.match(/import\('\/shared\//g) || []).length, 3, 'the script imports its three modules');
  vm.runInContext(src.replace(/import\('\/shared\//g, "__import('/shared/"), ctx, { filename: 'chat/chat-held.js' });
  vm.runInContext('(s, k, d, y) => { ws = s; myKey = k; myDilithiumSecret = d; myKyberSecret = y; }', ctx)(sock, me.key, me.dil.secretKey, me.kyb.secretKey);
  // The chat's own globals live in the page's lexical scope: declare them as the chat does.
  return { ctx, document, sock, said, me, m, handle: (msg) => ctx.handleMessage(msg) };
}

async function verified(m, submission) {
  const obj = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'pq-object.js')).href);
  const blake3 = (d) => m.blake3.create({ dkLen: 32 }).update(d).digest();
  const r = await obj.verifyObjectSubmission(submission, { blake3, pqVerify: async (pub, msg, sig) => m.ml_dsa65.verify(sig, msg, pub) });
  return r.ok;
}

/** The payload's text fields, read from its CBOR as plain strings. */
function payloadText(submission) {
  return Buffer.from(submission.payload_b64, 'base64').toString('latin1');
}

test('script stays DOM-only: no innerHTML, no data-on handlers', () => {
  const src = fs.readFileSync(path.join(WEB, 'chat', 'chat-held.js'), 'utf8').split('\n').filter((l) => !l.trim().startsWith('//')).join('\n');
  assert.ok(!/innerHTML|outerHTML|insertAdjacentHTML|document\.write/.test(src), 'no markup from strings');
  assert.ok(!/data-on/.test(src), 'no data-on-* handlers');
  assert.ok(!/safe-markup/.test(src), 'independent of safe-markup.js');
});

test('on a report: why not with no owner, and a signed decision with one', async () => {
  const page = await loadPage('admin');
  const card = page.document.createElement('div');
  const item = page.document.createElement('div');
  item.className = 'report-item';
  item.dataset.reportId = '5';
  card.appendChild(item);
  const model = { tab: 'open', items: [{ id: 5, context: 'post', reason: 'child_danger', reasonLabel: 'A child in danger' }] };
  page.handle({ type: 'reports', state: 'open', items: [], can_keep: false, cannot_keep_why: NO_OWNER });
  page.ctx.heldDecorateReports(card, model);
  assert.ok(card.textContent.includes(NO_OWNER), "the relay's sentence is drawn");
  assert.ok(card.textContent.includes(T('held.instead')) && card.textContent.includes(T('held.no_owner_today')), 'with what works instead and what to do today');
  assert.ok(!button(card, T('held.decide_label')), 'no Hide and keep button');
  assert.ok(button(card, T('held.open')), 'an admin gets Held copies');

  const card2 = page.document.createElement('div');
  const item2 = page.document.createElement('div');
  item2.className = 'report-item';
  item2.dataset.reportId = '5';
  card2.appendChild(item2);
  page.handle({ type: 'reports', state: 'open', items: [], can_keep: true, cannot_keep_why: '' });
  page.ctx.heldDecorateReports(card2, model);
  assert.ok(!card2.textContent.includes(NO_OWNER));
  button(card2, T('held.decide_label')).click();
  await until(() => page.sock.sent.some((f) => f.type === 'report_decide'), 'the decision is sent');
  const f = page.sock.sent.find((x) => x.type === 'report_decide');
  assert.strictEqual(f.id, 5);
  assert.strictEqual(f.decision, 'hide_and_keep');
  assert.ok(await verified(page.m, f.action), 'the signed action verifies');
  assert.strictEqual(f.action.space_id, DID, 'for this server');
  assert.strictEqual(Buffer.from(f.action.author_public_key_b64, 'base64').toString('hex'), page.me.key, 'signed by me');
  const p = payloadText(f.action);
  assert.ok(p.includes('hide_and_keep') && p.includes('report') && p.includes('Report 5: A child in danger'), p);
});

function heldList(owner) {
  const item = { id: 1, report_id: 5, file_count: 1, parts: 2, sealed_bytes: 4096, held_at: 1763600000000, held_by_name: 'Ada', delete_after: 1795136000000, legal_hold_until: 0, deleted_at: 1795136000000, exported_count: 0, sealed_to_owner: true };
  if (owner) item.package = 'pkg';
  return { type: 'held_items', can_keep: true, cannot_keep_why: '', i_am_owner: owner, items: [item], actions: [{ action: 'hide_and_keep', target: 5, signer_name: 'Ada', reason: 'the policy', until: 0, signed_at: 1763600000000 }], counts: { held: 1, exported: 0, legal_holds: 0, destroyed: 0, restored: 0 } };
}

test('held copies: numbers and dates, no way to view, actions only for the owner, destroy asks first', async () => {
  const page = await loadPage('admin');
  assert.ok(page.ctx.openHeldView());
  assert.deepStrictEqual(page.sock.sent.map((f) => f.type), ['held_list']);
  page.handle(heldList(false));
  const body = page.document.body;
  assert.ok(body.textContent.includes(T('held.item_line', { id: 1, report: 5 })) && body.textContent.includes('Ada'), 'the item and who held it');
  assert.ok(body.textContent.includes(T('held.owner_only')) && body.textContent.includes('the policy'), 'only the owner acts; the log');
  assert.deepStrictEqual(buttonTexts(body).sort(), [T('held.close'), T('held.refresh')].sort(), 'an admin gets no action, and no button opens an item');

  page.handle(heldList(true));
  for (const k of ['held.hold_set', 'held.export', 'held.restore', 'held.destroy']) assert.ok(button(body, T(k)), 'the owner gets ' + T(k));
  for (const t of buttonTexts(body)) assert.ok(!/^(View|Open|Show|Preview)$/.test(t), 'no button opens an item: ' + t);
  assert.ok(!body.textContent.includes('pkg'), 'the package id is never drawn');

  page.sock.sent.length = 0;
  button(body, T('held.destroy')).click();
  assert.strictEqual(page.sock.sent.length, 0, 'Destroy now sends nothing before it is confirmed');
  const question = T('held.destroy_confirm', { id: 1 });
  assert.ok(body.textContent.includes(question) && question.includes('cannot be undone'), 'it asks first, saying it cannot be undone');
  button(body, T('held.destroy_yes')).click();
  await new Promise((r) => setImmediate(r));
  assert.strictEqual(page.sock.sent.length, 0, 'no reason, nothing signed');
  assert.ok(body.textContent.includes(T('held.reason_needed')));
  const reason = body.querySelector('.held-reason');
  reason.value = 'a request from law enforcement';
  reason.fire('input');
  button(body, T('held.destroy')).click();
  button(body, T('held.destroy_yes')).click();
  await until(() => page.sock.sent.some((f) => f.type === 'held_act'), 'held_act is sent');
  const act = page.sock.sent.find((f) => f.type === 'held_act');
  assert.ok(await verified(page.m, act.action), 'the signed action verifies');
  const p = payloadText(act.action);
  assert.ok(p.includes('held_destroy') && p.includes('law enforcement'), p);
  page.handle(heldList(true)); // the relay's answer
});

/** A test-only twin of the Rust seal (as held-web.test.js uses), one piece. */
async function sealPart(m, H, publicKey, context, plain) {
  const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(publicKey);
  const keyBytes = m.blake3.create({ context: enc.encode(H.DOMAIN), dkLen: 32 }).update(sharedSecret).digest();
  const key = await crypto.subtle.importKey('raw', keyBytes, { name: 'AES-GCM' }, false, ['encrypt']);
  const aad = enc.encode(H.pieceAad(context, 0, true));
  const body = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv: new Uint8Array(12), additionalData: aad }, key, plain));
  return Buffer.concat([Buffer.from(enc.encode(H.MAGIC)), Buffer.from(cipherText), Buffer.from(body)]);
}

const WORDS = 'HELD-WORDS-91c2 the post words';
const PIC = 'HELD-PIC-5e0a picture bytes';

test('export opens the parts straight into the saved file, never onto the page', async () => {
  const m = await noble();
  const H = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'held-seal.js')).href);
  const me = keysOf(m, 9);
  const manifest = { v: 1, package: 'pkg', report_id: 5, author: 'aa', stamp: 42, channel: 'global', words: WORDS, held_by: 'bb', held_at: 1, restore_key: 'cd'.repeat(32),
    files: [{ part: 1, name: '1_x_pic.png', folder: 'public', uploader: 'aa', shelf: 0, original_name: 'pic.png', bytes: PIC.length, hash: '', poster_part: null, poster_bytes: 0, poster_hash: '' }] };
  const parts = [await sealPart(m, H, me.kyb.publicKey, H.partContext('pkg', 0), enc.encode(JSON.stringify(manifest))), await sealPart(m, H, me.kyb.publicKey, H.partContext('pkg', 1), enc.encode(PIC))];
  let written = null;
  const fetched = [];
  const page = await loadPage('owner', {
    fetch: async (url) => {
      fetched.push(url);
      const mm = /^\/api\/held\/1\/(\d+)\?grant=([0-9a-f]{64})$/.exec(url);
      if (!mm) return { ok: false, status: 404, arrayBuffer: async () => new ArrayBuffer(0) };
      const b = parts[Number(mm[1])];
      return { ok: true, status: 200, arrayBuffer: async () => b.buffer.slice(b.byteOffset, b.byteOffset + b.length) };
    },
    globals: { showSaveFilePicker: async (opts) => ({ name: opts.suggestedName, createWritable: async () => ({ write: async (b) => { written = Buffer.from(b); }, close: async () => {} }) }) },
  });
  page.ctx.openHeldView();
  page.handle(heldList(true));
  const body = page.document.body;
  const reason = body.querySelector('.held-reason');
  reason.value = 'a CyberTipline report';
  reason.fire('input');
  button(body, T('held.export')).click();
  await until(() => page.sock.sent.some((f) => f.type === 'held_act'), 'the signed export is sent first');
  const act = page.sock.sent.find((f) => f.type === 'held_act');
  assert.ok(payloadText(act.action).includes('held_export'));
  page.handle(heldList(true)); // granted
  await until(() => written !== null || body.textContent.includes(T('held.export_failed', { why: '' }).slice(0, 20)), 'the file is written');
  assert.ok(written, 'the zip was written: ' + body.textContent);
  assert.ok(fetched.every((u) => u.startsWith('/api/held/1/')), 'only the sealed parts were fetched');
  const zip = written.toString('latin1');
  assert.ok(zip.includes(WORDS) && zip.includes(PIC), 'the words and the file are in the saved file');
  assert.ok(!zip.includes('cd'.repeat(32)), 'never the restore key');
  await until(() => body.textContent.includes('held-item-1.zip'), 'the page says where it went');
  for (const secret of [WORDS, PIC, 'HELD-']) assert.ok(!body.textContent.includes(secret), 'none of it is drawn: ' + secret);
});
