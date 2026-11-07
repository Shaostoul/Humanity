// A picture the server cannot clean: the person decides, and only their signed choice keeps its
// details (2026-11-06, docs/BUGS.md BUG-260; the operator: "can we have an option to send or
// refuse?"). The website's half: web/shared/upload-clean.js (`decide`, `keepChoice`) and the chat's
// upload (web/chat/chat-messages.js `uploadPrepared`, `askSendAnyway`). The relay's half is
// src/relay/upload_keep_tests.rs; the desktop's, src/gui/pages/chat/attach_keep_tests.rs.
//
// Run: node --test scripts/tests/upload-keep-web.test.js   (pure node, nothing booted or connected)
//
// What it proves:
//  1. The signed text is the relay's, read from src/relay/upload_rules.rs (domain and shape), and
//     keepChoice signs exactly it, over the file's BLAKE3.
//  2. A file for a public place that could not be cleaned is ASKED about (with the question's
//     words by key, "Send anyway" keeping its details), never refused outright: in the chat since
//     BUG-260, on the Files pages since 2026-11-07 (BUG-264); the protected setup still refuses it.
//     The two answers every page draws are one shared piece (upload-clean.js keepAnswers).
//  3. When the server answers 422 (it could not remove the details), the chat's question appears
//     with "Don't send" first and holding the focus; "Send anyway" uploads the file again with
//     keep_metadata=1 and the signed choice as the first part; "Don't send" uploads nothing more.
//
// HOS_WEB_DIR points at another copy of web/. Seen red 2026-11-06 against a copy holding the
// chat-messages.js and upload-clean.js from before this: test 1 failed at "upload-clean.js signs a
// keep choice" (no keepChoice), test 2 at "asked, and on yes it goes as it is" (a public file that
// could not be cleaned was refused outright), test 3 at "the chat uploads a prepared file" (no
// uploadPrepared: a 422 was only told as a failure).

'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const REPO = path.resolve(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, 'web');
const read = (rel) => fs.readFileSync(path.join(WEB, rel), 'utf8');
const RULES = fs.readFileSync(path.join(REPO, 'src', 'relay', 'upload_rules.rs'), 'utf8');

/** Just enough page: elements with children, attributes, focus and clicks. */
function fakeDocument() {
  const doc = { focused: null };
  const el = (tag) => {
    const e = {
      tagName: tag, children: [], dataset: {}, style: {}, attrs: {}, className: '', textContent: '', parent: null,
      setAttribute(k, v) { e.attrs[k] = String(v); },
      getAttribute(k) { return e.attrs[k]; },
      appendChild(c) { c.parent = e; e.children.push(c); return c; },
      insertBefore(c, ref) { e.children = e.children.filter((x) => x !== c); const i = e.children.indexOf(ref); e.children.splice(i < 0 ? e.children.length : i, 0, c); c.parent = e; return c; },
      get firstChild() { return e.children[0] || null; },
      remove() { if (e.parent) e.parent.children = e.parent.children.filter((x) => x !== e); e.parent = null; },
      focus() { doc.focused = e; },
      addEventListener() {},
      classList: { add() {}, remove() {}, toggle() {}, contains() { return false; } },
    };
    return e;
  };
  doc.createElement = el;
  doc.body = el('body');
  const byId = {};
  doc.getElementById = (id) => (byId[id] = byId[id] || el('div'));
  doc.querySelector = () => null;
  doc.addEventListener = () => {};
  return doc;
}

/** Every element under `root` (itself included). */
function all(root) {
  return [root].concat(...root.children.map(all));
}

/** A page with the shared files and the chat's messages file loaded, its network stood in. */
function chatPage(answers) {
  const document = fakeDocument();
  const said = [];
  const posts = [];
  class FormData {
    constructor() { this.parts = []; }
    append(name, value, filename) { this.parts.push({ name, value, filename }); }
  }
  const ctx = {
    console, Math, Date, JSON, Promise, Uint8Array, Uint32Array, TextEncoder, ArrayBuffer, Blob, File, Buffer,
    setTimeout, clearTimeout, btoa: (s) => Buffer.from(s, 'binary').toString('base64'),
    document, FormData, XMLHttpRequest: undefined,
    fetch: async () => ({ ok: false }), // the reactions list the file asks for as it loads
    addSystemMessage: (t) => said.push(String(t)),
    myKey: 'me_key', myDilithiumSecret: new Uint8Array(4),
    // The signature stood in by the signed bytes themselves, so the test can read what was signed.
    pqSignMessage: async (_secret, bytes) => new Uint8Array(bytes),
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  for (const rel of ['shared/blake3.js', 'shared/strip-metadata.js', 'shared/upload-clean.js', 'chat/chat-messages.js']) {
    vm.runInContext(read(rel), ctx, { filename: rel });
  }
  // The network: each upload answered in turn from `answers`, its form kept.
  ctx.uploadBaseUrl = () => '/api/upload?token=tok';
  ctx.uploadPost = async (url, form) => {
    posts.push({ url, parts: form.parts });
    const a = answers.shift();
    return { ok: a.status < 300, status: a.status, text: async () => a.text || '', json: async () => a.json || {} };
  };
  ctx.videoPosterFor = async () => null;
  return { ctx, document, said, posts };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

test('the signed text is the relay\'s, and keepChoice signs exactly it over the file\'s BLAKE3', async () => {
  const domain = RULES.match(/pub\(crate\) const KEEP_DOMAIN: &str = "([^"]+)";/);
  assert.ok(domain, 'upload_rules.rs declares KEEP_DOMAIN');
  assert.match(RULES, /format!\("\{KEEP_DOMAIN\}\\n\{key\}\\n\{\}\\n\{ts\}", blake3_hex\.to_ascii_lowercase\(\)\)/, 'the relay signs domain, key, hash, time, one per line');
  const C = require(path.join(WEB, 'shared', 'upload-clean.js'));
  const B3 = require(path.join(WEB, 'shared', 'blake3.js'));
  assert.strictEqual(typeof C.keepChoice, 'function', 'upload-clean.js signs a keep choice');
  assert.strictEqual(C.KEEP_DOMAIN, domain[1]);
  const bytes = new Uint8Array([1, 2, 3, 4, 5]);
  const made = await C.keepChoice(new Blob([bytes]), { me: 'me_key', sign: async (b) => new Uint8Array(b), blake3: B3, now: () => 1234 });
  const hex = B3.toHex(B3.hash(bytes));
  assert.strictEqual(made.blake3, hex);
  const choice = JSON.parse(made.json);
  assert.strictEqual(choice.ts, 1234);
  assert.strictEqual(Buffer.from(choice.sig, 'base64').toString(), `${domain[1]}\nme_key\n${hex}\n1234`, 'it signs exactly the relay\'s text');
  assert.strictEqual(await C.keepChoice(new Blob([bytes]), { me: 'me_key', sign: async () => null, blake3: B3 }), null, 'no signature, no choice');
});

// Since 2026-11-07 (BUG-264) the Files pages ask as the chat does (they refused until then); seen red
// against the upload-clean.js from before: "every public place asks" failed (actual 'refuse').
test('every public place asks about a file it could not clean (the chat and the Files pages); the protected setup refuses it', () => {
  const C = require(path.join(WEB, 'shared', 'upload-clean.js'));
  const clean = { unchecked: true, heif: false, kind: 'jpeg', original: 'ORIGINAL', cleaned: 'CLEANED', reason: 'could not be read' };
  const asked = C.decide(clean, { where: 'public', remove: true });
  assert.deepStrictEqual([asked.action, asked.file, asked.keepMetadata, asked.words], ['ask', 'ORIGINAL', true, 'upload_keep.question_picture'], 'every public place asks; on yes it goes as it is, its details kept by signed choice');
  assert.strictEqual(C.decide({ ...clean, kind: 'mp4' }, { where: 'public', remove: true }).words, 'upload_keep.question_video');
  assert.strictEqual(C.decide({ ...clean, kind: 'gif' }, { where: 'public', remove: true }).words, 'upload_keep.question_picture', 'a GIF is a picture (BUG-263)');
  assert.strictEqual(C.decide(clean, { where: 'public', remove: true, protectedOn: true }).action, 'refuse', 'the protected setup never sends one');
  // The two answers every page draws: "Don't send" then "Send anyway", each with its hint, by key.
  const doc = fakeDocument();
  const a = C.keepAnswers(doc, { no: 'n', yes: 'y' });
  assert.deepStrictEqual([a.no.textContent, a.no.attrs['data-tip-desc'], a.no.dataset.keepAsk, a.yes.textContent, a.yes.attrs['data-tip-desc'], a.yes.dataset.keepAsk],
    ['upload_keep.dont_send', 'upload_keep.dont_send_hint', 'dont-send', 'upload_keep.send_anyway', 'upload_keep.send_anyway_hint', 'send-anyway']);
  for (const k of ['question_picture', 'question_video', 'dont_send', 'dont_send_hint', 'send_anyway', 'send_anyway_hint', 'not_sent', 'cannot_sign', 'title']) {
    const en = JSON.parse(fs.readFileSync(path.join(REPO, 'data', 'i18n', 'en.json'), 'utf8'));
    assert.ok(en.upload_keep && typeof en.upload_keep[k] === 'string' && en.upload_keep[k].length > 0, `upload_keep.${k} has words`);
  }
});

test('the server\'s 422: the question appears, "Send anyway" sends it again signed, "Don\'t send" sends nothing', async () => {
  const photo = new File([new Uint8Array([0xff, 0xd8, 0xff, 0xe1, 0x70, 0, 1, 2, 3])], 'garden.jpg', { type: 'image/jpeg' });

  // Send anyway.
  let page = chatPage([{ status: 422, text: 'This picture could not be read all the way through...' }, { status: 200, json: { url: '/uploads/1_garden.jpg' } }]);
  assert.strictEqual(typeof page.ctx.uploadPrepared, 'function', 'the chat uploads a prepared file');
  let sending = page.ctx.uploadPrepared(photo, { file: photo, keepMetadata: false, unchecked: false });
  await settle();
  await settle();
  let overlay = page.document.body.children.find((c) => c.attrs.role === 'dialog' || all(c).some((x) => x.attrs.role === 'dialog'));
  assert.ok(overlay, 'the question appears');
  let els = all(overlay);
  assert.ok(els.some((x) => x.textContent === 'upload_keep.question_picture'), 'with the question\'s words');
  const buttons = els.filter((x) => x.tagName === 'button');
  assert.deepStrictEqual(buttons.map((b) => b.textContent), ['upload_keep.dont_send', 'upload_keep.send_anyway'], '"Don\'t send" first');
  assert.strictEqual(page.document.focused, buttons[0], 'and holding the focus');
  assert.ok(buttons.every((b) => b.attrs['data-tip-desc']), 'each says what it does');
  buttons[1].onclick();
  assert.strictEqual(await sending, '/uploads/1_garden.jpg', 'Send anyway stores it');
  assert.strictEqual(page.posts.length, 2);
  const again = page.posts[1];
  assert.match(again.url, /keep_metadata=1/, 'sent again keeping its details');
  assert.strictEqual(again.parts[0].name, 'keep', 'with the signed choice as the first part');
  const signed = Buffer.from(JSON.parse(again.parts[0].value).sig, 'base64').toString();
  const B3 = require(path.join(WEB, 'shared', 'blake3.js'));
  const hex = B3.toHex(B3.hash(new Uint8Array(await photo.arrayBuffer())));
  assert.ok(signed.startsWith(`hum/upload-keep/v1\nme_key\n${hex}\n`), 'signed by me, for this very file: ' + signed);
  assert.strictEqual(again.parts.find((p) => p.name === 'file').value, photo, 'the file exactly as it was picked');

  // Don't send.
  page = chatPage([{ status: 422, text: '...' }]);
  sending = page.ctx.uploadPrepared(photo, { file: photo, keepMetadata: false, unchecked: false });
  await settle();
  await settle();
  overlay = page.document.body.children[0];
  els = all(overlay);
  els.filter((x) => x.tagName === 'button')[0].onclick();
  assert.strictEqual(await sending, null, 'nothing is stored');
  assert.strictEqual(page.posts.length, 1, 'and nothing more is sent');
  assert.ok(page.said.includes('upload_keep.not_sent'), 'the conversation says it was not sent');
});
