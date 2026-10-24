// The first five minutes (the ELI5 mandate, 2026-10-23): the guided tour both apps give a new
// person reads its steps from data/onboarding/tour.json (src/gui/first_minutes.rs on the desktop,
// web/shared/onboarding-tour.js on the website). This proves the file says only what both can
// show: every word a key English has, every place a page that exists.
//
// Run: node --test scripts/tests/first-minutes.test.js

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const W = require(path.join(ROOT, 'scripts', 'i18n-web.js'));
const TOUR = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'onboarding', 'tour.json'), 'utf8'));
const EN = W.flatten(JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8')));

/** The desktop app's page ids, read from dispatch::page_id's match arms. */
function nativePageIds() {
  const src = fs.readFileSync(path.join(ROOT, 'src', 'gui', 'dispatch.rs'), 'utf8');
  const at = src.indexOf('pub fn page_id(');
  const body = src.slice(at, src.indexOf('\n}\n', at));
  return new Set([...body.matchAll(/=>\s*"([a-z_]+)"/g)].map((m) => m[1]));
}

/** Does a website address name a page this repo serves? */
function webPageExists(addr) {
  const p = addr.split('#')[0].replace(/^\//, '');
  if (!p) return true;
  return [
    path.join(ROOT, 'web', 'pages', p + '.html'),
    path.join(ROOT, 'web', p, 'index.html'),
  ].some((f) => fs.existsSync(f));
}

function keysOf(step) {
  const out = [step.title, step.text];
  if (step.web && step.web.text) out.push(step.web.text);
  return out;
}

test('every step has an id, a title and words, and the ids are unique', () => {
  assert.ok(Array.isArray(TOUR.steps) && TOUR.steps.length >= 4);
  const ids = TOUR.steps.map((s) => s.id);
  assert.deepStrictEqual(ids.filter((id, i) => ids.indexOf(id) !== i), [], 'an id used twice');
  for (const want of ['phrase', 'message', 'world']) assert.ok(ids.includes(want), 'the ' + want + ' step is there');
});

test('every word of the tour is a key English has (no step shows its key)', () => {
  const bad = [];
  for (const step of TOUR.steps) for (const k of keysOf(step)) if (typeof EN[k] !== 'string') bad.push(step.id + ': ' + k);
  // The card's own words, read by both apps.
  for (const k of ['tour.card_name', 'tour.counter', 'tour.take_me_there', 'tour.back', 'tour.next', 'tour.finish', 'tour.hide', 'tour.skip', 'tour.phrase_written']) {
    if (typeof EN[k] !== 'string') bad.push('card: ' + k);
  }
  assert.deepStrictEqual(bad, []);
});

test('every place a step points at exists on its app', () => {
  const native = nativePageIds();
  assert.ok(native.has('chat') && native.has('settings'), 'read the desktop page ids');
  const bad = [];
  for (const step of TOUR.steps) {
    if (step.native && step.native.page && !native.has(step.native.page)) bad.push(step.id + ': desktop page ' + step.native.page);
    if (step.web && step.web.page && !webPageExists(step.web.page)) bad.push(step.id + ': web page ' + step.web.page);
  }
  assert.deepStrictEqual(bad, []);
});

test('a step that ticks itself names a fact the apps know', () => {
  const known = new Set(['phrase_written', 'message_sent', 'entered_world']);
  const bad = TOUR.steps.filter((s) => s.done_when && !known.has(s.done_when)).map((s) => s.id + ': ' + s.done_when);
  assert.deepStrictEqual(bad, []);
});
