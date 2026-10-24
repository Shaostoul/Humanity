// Is my chat line really posted? The website's own copy of a line I send reads "Sending..." until
// the server's copy comes back, and "Not sent" with the reason and an "Edit and send again" button
// when the server refuses it (relay ChatRefused, 2026-10-24).
// Run: node --test scripts/tests/line-status-web.test.js
//
// The three functions are read out of web/chat/app.js (from "Is my line really posted?" to
// `function appendMessage`) and run against a small stand-in for the page. APP_JS=<path> points
// the test at another copy. Seen red 2026-10-24 against the app.js before this (the functions did
// not exist: "the line-status functions are where the test expects them").
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const FILE = process.env.APP_JS || path.join(__dirname, '..', '..', 'web', 'chat', 'app.js');

// A tiny page: elements with classes, children, text, data attributes and listeners.
function makeEl(tag) {
  const el = {
    tagName: tag, children: [], dataset: {}, className: '', textContent: '', value: '', listeners: {},
    classList: {
      _s: new Set(),
      add(c) { this._s.add(c); }, remove(c) { this._s.delete(c); }, contains(c) { return this._s.has(c); },
    },
    appendChild(c) { c.parent = el; el.children.push(c); return c; },
    remove() { if (el.parent) el.parent.children = el.parent.children.filter((x) => x !== el); },
    querySelector(sel) { return sel === ':scope > .line-status' ? el.children.find((c) => c.className === 'line-status') || null : null; },
    addEventListener(t, fn) { el.listeners[t] = fn; },
    attrs: {}, setAttribute(k, v) { el.attrs[k] = v; },
    focus() {}, dispatchEvent() {},
  };
  return el;
}

function page() {
  const messages = [];
  const input = makeEl('textarea');
  const document = {
    createElement: makeEl,
    getElementById: (id) => (id === 'msg-input' ? input : null),
    querySelector(sel) {
      const m = sel.match(/data-from="([^"]*)"\]\[data-timestamp="([^"]*)"/);
      return m ? messages.find((e) => e.dataset.from === m[1] && String(e.dataset.timestamp) === m[2]) || null : null;
    },
  };
  const words = {
    'chat.line_sending': 'Sending...', 'chat.line_unconfirmed': 'Not confirmed by the server yet.',
    'chat.line_not_sent': 'Not sent.', 'chat.line_not_sent_too_long': 'Not sent: longer than your limit of {{limit}} characters.',
    'chat.line_not_sent_too_fast': 'Not sent: sending too fast.', 'chat.line_not_sent_new_account': 'Not sent: new accounts wait.',
    'chat.line_edit_again': 'Edit and send again',
  };
  const i18n = { t: (k, v) => (words[k] || k).replace(/\{\{(\w+)\}\}/g, (_, n) => (v && v[n] != null ? v[n] : '')) };
  const src = fs.readFileSync(FILE, 'utf8');
  const start = src.indexOf('// ── Is my line really posted?');
  const end = src.indexOf('function appendMessage(el) {');
  assert.ok(start >= 0 && end > start, 'the line-status functions are where the test expects them');
  const timers = [];
  const ctx = vm.createContext({ document, i18n, myKey: 'me', Map, String, Event: function () {}, setTimeout: (fn) => timers.push(fn) });
  vm.runInContext(src.slice(start, end) + '\n;globalThis.__f = { markLineSending, markLineSent, markLineNotSent };', ctx);
  const add = (ts) => { const e = makeEl('div'); e.dataset.from = 'me'; e.dataset.timestamp = ts; messages.push(e); return e; };
  return { f: ctx.__f, add, input, timers };
}

const status = (el) => el.children.find((c) => c.className === 'line-status');

test('a line reads Sending... until the server\'s copy comes back', () => {
  const p = page();
  const el = p.add(1000);
  p.f.markLineSending(1000, 'hello');
  assert.ok(el.classList.contains('line-sending'));
  assert.strictEqual(status(el).textContent, 'Sending...');
  p.f.markLineSent(1000);
  assert.ok(!el.classList.contains('line-sending'), 'confirmed');
  assert.strictEqual(status(el), undefined, 'the status line is gone');
});

test('a refused line says why and gives the words back', () => {
  const p = page();
  const el = p.add(2000);
  p.f.markLineSending(2000, 'a very long line');
  p.f.markLineNotSent(2000, 'too_long', 280);
  assert.ok(el.classList.contains('line-not-sent') && !el.classList.contains('line-sending'));
  assert.match(status(el).textContent, /limit of 280 characters/);
  const again = status(el).children.find((c) => c.className === 'line-send-again');
  assert.ok(again, 'an Edit and send again button');
  again.listeners.click();
  assert.strictEqual(p.input.value, 'a very long line', 'the words are back in the box');
});

test('a line the server never answers says so after a while', () => {
  const p = page();
  const el = p.add(3000);
  p.f.markLineSending(3000, 'x');
  p.timers.forEach((fn) => fn());
  assert.strictEqual(status(el).textContent, 'Not confirmed by the server yet.');
});

test('the page wires the server\'s answers to these functions', () => {
  const src = fs.readFileSync(FILE, 'utf8');
  assert.match(src, /case 'chat_refused':\s*\n\s*\/\/[^\n]*\n\s*markLineNotSent\(msg\.timestamp, msg\.reason, msg\.limit\)/, 'chat_refused marks the line not sent');
  assert.match(src, /if \(msg\.from === myKey\) markLineSent\(msg\.timestamp\)/, 'my own line coming back confirms it');
  assert.match(src, /markLineSending\(timestamp, content\)/, 'a sent line starts as sending');
});
