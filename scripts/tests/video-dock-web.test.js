// The chat's video dock on the website (2026-11-06, rung 3; web/shared/video-dock.js, wired in
// web/chat/app.js; docs/design/media-player.md, "Rung 3: the anchored dock"). The desktop app's twin
// is src/gui/pages/chat/video_dock.rs, with its own tests.
//
// The operator: "When we scroll them off page can we add like an anchored picture-in-picture mode
// that's stuck to the side of the chat it's at? It could be like a collapsible tap that has minimize
// (keeps playing unless paused) and close (closes anchored section.)"
//
// Run: node --test scripts/tests/video-dock-web.test.js
//
// A small stand-in document whose elements keep their parent and children the way a browser's do
// (a node moved is taken from where it was), and whose <video> counts every load() and every write
// to its src: the dock must MOVE the playing element (the same object), never make a new one, load
// it again or set its address, or the video would start again. Whether a browser plays a moved
// element on is the browser's part, checked in Chromium (the design doc says how); this test holds
// the dock to doing nothing that restarts one.
//
// HOS_WEB_DIR points it at another copy of web/ (used to see each check red).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const Dock = require(path.join(WEB, 'shared', 'video-dock.js'));
const EN = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8'));

/** en.json's words for `key`, with {{vars}} filled, as web/shared/i18n.js answers. */
function enWords(key, vars) {
  const v = key.split('.').reduce((o, k) => (o ? o[k] : undefined), EN);
  if (typeof v !== 'string') throw new Error('en.json has no ' + key);
  return v.replace(/\{\{(\w+)\}\}/g, (_m, k) => (vars && k in vars ? String(vars[k]) : ''));
}
const i18n = { t: enWords };

class El {
  constructor(doc, tag) {
    this.ownerDocument = doc;
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.parentNode = null;
    this.attrs = {};
    this.style = {};
    this.dataset = {};
    this.listeners = {};
    this.hidden = false;
    this.textContent = '';
    this.className = '';
    this.title = '';
  }
  detach() { if (this.parentNode) { const p = this.parentNode; p.children.splice(p.children.indexOf(this), 1); this.parentNode = null; } }
  appendChild(c) { c.detach(); c.parentNode = this; this.children.push(c); return c; }
  insertBefore(c, ref) { c.detach(); const i = this.children.indexOf(ref); c.parentNode = this; this.children.splice(i < 0 ? this.children.length : i, 0, c); return c; }
  replaceChild(n, old) { n.detach(); const i = this.children.indexOf(old); this.children[i] = n; n.parentNode = this; old.parentNode = null; return old; }
  removeChild(c) { c.detach(); return c; }
  setAttribute(k, v) { this.attrs[k] = String(v); }
  getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
  addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); }
  removeEventListener(t, f) { this.listeners[t] = (this.listeners[t] || []).filter((g) => g !== f); }
  fire(t) { for (const f of this.listeners[t] || []) f({ type: t, target: this }); }
  click() { this.fire('click'); }
  get isConnected() { for (let n = this; n; n = n.parentNode) if (n === this.ownerDocument.body) return true; return false; }
  querySelector(sel) { for (const c of this.children) { if (c.tagName.toLowerCase() === sel || ('.' + c.className) === sel) return c; const f = c.querySelector(sel); if (f) return f; } return null; }
  all() { return this.children.flatMap((c) => [c, ...c.all()]); }
  getBoundingClientRect() { return this.rect || { top: 100, left: 300, right: 1100, bottom: 700, width: 800, height: 600 }; }
  scrollIntoView(o) { this.scrolledWith = o; }
}

class Video extends El {
  constructor(doc, src) {
    super(doc, 'video');
    this._src = src;
    this.srcWrites = 0;
    this.loads = 0;
    this.paused = true;
    this.ended = false;
    this.currentTime = 0;
    this.offsetHeight = 315;
    this.offsetWidth = 560;
    this.pip = 0;
  }
  get src() { return this._src; }
  set src(v) { this.srcWrites += 1; this._src = v; this.currentTime = 0; }
  getAttribute(k) { return k === 'src' ? this._src : super.getAttribute(k); }
  load() { this.loads += 1; this.currentTime = 0; }
  play() { this.paused = false; this.fire('play'); }
  pause() { this.paused = true; this.fire('pause'); }
  requestPictureInPicture() { this.pip += 1; return Promise.resolve(); }
}

/** A page: a body holding the chat column, a message list, and one card per address. */
function page(opts = {}) {
  const doc = { pictureInPictureEnabled: !!opts.pip };
  doc.createElement = (tag) => new El(doc, tag);
  doc.body = new El(doc, 'body');
  const area = doc.body.appendChild(new El(doc, 'div'));
  const messages = area.appendChild(new El(doc, 'div'));
  const card = (url) => {
    const c = messages.appendChild(new El(doc, 'div'));
    c.className = 'file-card file-card-media';
    c.dataset.fileUrl = url;
    const v = c.appendChild(new Video(doc, url));
    // Then its name, size and Download, as fileCardHtml draws them after the player.
    c.appendChild(new El(doc, 'div')).className = 'file-info';
    return { card: c, video: v };
  };
  const told = { goTo: [], back: [] };
  const dock = Dock.create({
    document: doc,
    host: area,
    messages,
    i18n,
    narrow: () => !!opts.narrow,
    onGoTo: (origin) => { told.goTo.push(origin); if (opts.onGoTo) opts.onGoTo(origin); },
    findCard: (url) => messages.children.find((c) => c.dataset.fileUrl === url) || null,
    onBack: (video, line) => told.back.push({ video, line }),
  });
  return { doc, area, messages, card, dock, told };
}

const URL_A = '/uploads/1760000000000_sliding_bar.mp4';
const FROM = enWords('chat_video_dock.from_here', { channel: '#global' });

/** A video playing in its card, then taken into the dock. */
function docked(opts) {
  const p = page(opts);
  const { card, video } = p.card(URL_A);
  video.play();
  video.currentTime = 12.5;
  const line = p.dock.take(video, card, { channel: 'global' }, FROM);
  return Object.assign(p, { card, video, line });
}

// Seen red 2026-11-06 against a copy of web/ (HOS_WEB_DIR) whose `take` put a new player in the box
// in place of the playing one (its address set on it): "the same element is in the box"; with `take`
// setting the moved element's `src` again after the move: "its address is not set again (0 writes)".
// (Scripts in scratch: pip-red-web.js, one broken copy per check.)
test('a playing video taken into the dock is the same element, moved, never loaded or given its address again', () => {
  const { dock, card, video, line } = docked();
  const { picture, box } = dock.parts();
  assert.strictEqual(video.parentNode, picture, 'the same element is in the box');
  assert.strictEqual(dock.held(), video);
  assert.equal(video.paused, false, 'it plays on');
  assert.equal(video.loads, 0, 'never loaded again');
  assert.equal(video.srcWrites, 0, 'its address is not set again (0 writes)');
  assert.equal(video.currentTime, 12.5, 'at the place it reached');
  assert.ok(line && line.parentNode === card && line.className === 'file-video-away', 'its card keeps a line in its place');
  assert.equal(line.textContent, enWords('chat_video_dock.row_away'), 'saying where it is');
  assert.equal(line.style.height, '315px', 'as tall as the player was, so nothing in the list moves');
  assert.equal(box.hidden, false, 'the box shows');
  assert.equal(dock.take(new Video(page().doc, '/uploads/b.mp4'), card, {}, ''), null, 'one at a time: a second is not taken');
  const paused = new Video(page().doc, URL_A);
  assert.equal(page().dock.take(paused, card, {}, ''), null, 'a paused video is not taken (it goes as before)');
});

// Seen red 2026-11-06 against a copy of web/ with the Go to message button left out of `build`:
// "the box has Go to message, its words and hint by key".
test('the box says where the video came from and has Minimize, Close and Go to message, each with its words and hint by key', () => {
  const { dock } = docked();
  const { box, from } = dock.parts();
  assert.equal(from.textContent, FROM, 'where it came from');
  const buttons = box.all().filter((e) => e.tagName === 'BUTTON');
  for (const [key, hint, what] of [['chat_video_dock.minimize', 'chat_video_dock.minimize_hint', 'Minimize'], ['common.close', 'chat_video_dock.close_hint', 'Close'], ['chat_video_dock.go_to', 'chat_video_dock.go_to_hint', 'Go to message']]) {
    const b = buttons.find((x) => x.attrs['data-i18n'] === key);
    assert.ok(b, `the box has ${what}, its words and hint by key`);
    assert.equal(b.textContent, enWords(key), `${what}'s words`);
    assert.equal(b.attrs['data-i18n-tip'], hint, `${what}'s hint key`);
    assert.equal(b.attrs['data-tip-desc'], enWords(hint), `${what}'s hint words`);
  }
  assert.equal(box.attrs['aria-label'], enWords('chat_video_dock.label'), 'the box is a named region');
  assert.ok(!buttons.some((x) => x.attrs['data-i18n'] === 'chat_video_dock.pip'), 'no Float it where the browser has no picture-in-picture of its own');
});

// Seen red 2026-11-06 against a copy of web/ whose `minimize` paused the video: "minimized, it
// plays on".
test('Minimize folds it to the tab and it plays on; the tab says when it is paused and opens it again', () => {
  const { dock, video } = docked();
  const { box, tab } = dock.parts();
  box.all().find((e) => e.attrs['data-i18n'] === 'chat_video_dock.minimize').click();
  assert.equal(video.paused, false, 'minimized, it plays on');
  assert.equal(box.hidden, true, 'the box folds away');
  assert.equal(tab.hidden, false, 'the tab shows');
  assert.equal(tab.textContent, enWords('chat_video_dock.tab_playing'));
  assert.equal(tab.attrs['data-i18n-tip'], 'chat_video_dock.tab_hint', 'the tab has its hint');
  video.pause();
  assert.equal(tab.textContent, enWords('chat_video_dock.tab_paused'), 'paused, the tab says so');
  tab.click();
  assert.equal(box.hidden, false, 'the tab pressed, the box opens');
  assert.equal(tab.hidden, true);
});

// Seen red 2026-11-06 against a copy of web/ whose `close` only hid the box: "the video stops".
test('Close stops the video and puts it back in its card at its place; the box closes', () => {
  const { dock, card, video, line } = docked();
  const { box, tab } = dock.parts();
  box.all().find((e) => e.attrs['data-i18n'] === 'common.close').click();
  assert.equal(video.paused, true, 'the video stops');
  assert.strictEqual(video.parentNode, card, 'back in its card');
  assert.equal(line.parentNode, null, 'in its line\'s place');
  assert.equal(video.currentTime, 12.5, 'at the place it reached (its still)');
  assert.equal(video.loads + video.srcWrites, 0);
  assert.equal(box.hidden && tab.hidden, true, 'the box closes');
  assert.equal(dock.held(), null);
});

// Seen red 2026-11-06 against a copy of web/ whose `giveBack` appended the video to the card's end
// instead of its line's place: "where its line was".
test('its line back in view gives the video back to its card, playing on, the same element', () => {
  const { dock, card, video, line, told } = docked();
  const at = card.children.indexOf(line);
  assert.strictEqual(dock.giveBack(), video);
  assert.strictEqual(card.children[at], video, 'where its line was');
  assert.equal(video.paused, false, 'playing on');
  assert.equal(video.loads + video.srcWrites, 0, 'never loaded again');
  assert.deepEqual(told.back.map((b) => [b.video, b.line]), [[video, line]], 'app.js is told, to watch it again');
  assert.equal(dock.parts().box.hidden, true, 'the box closes');
});

// Seen red 2026-11-06 against a copy of web/ whose `goTo` did not wait for the card of a channel
// still opening (no `goToWaiting`): "the new card's player gives way to the docked one".
test('Go to message scrolls to its card and gives the video back; from another channel it waits for the card to be drawn', () => {
  const here = docked();
  here.dock.parts().box.all().find((e) => e.attrs['data-i18n'] === 'chat_video_dock.go_to').click();
  assert.deepEqual(here.told.goTo, [{ channel: 'global' }], 'app.js is asked to open its channel');
  assert.deepEqual(here.card.scrolledWith, { block: 'center' }, 'its card scrolled into view');
  assert.strictEqual(here.video.parentNode, here.card, 'the video is back in its card');
  assert.equal(here.video.paused, false);

  // Another channel open: its card is gone. Go to message opens the channel; its card, drawn again
  // with a new player of its own, takes the docked video back in place of that new player.
  const away = docked();
  away.messages.removeChild(away.card);
  away.dock.goTo();
  assert.equal(away.dock.held(), away.video, 'still in the box while the channel opens');
  const drawn = away.messages.appendChild(new El(away.doc, 'div'));
  drawn.className = 'file-card file-card-media';
  drawn.dataset.fileUrl = URL_A;
  const fresh = drawn.appendChild(new Video(away.doc, URL_A));
  away.dock.adopt(drawn);
  assert.strictEqual(away.video.parentNode, drawn, 'the new card\'s player gives way to the docked one');
  assert.equal(fresh.parentNode, null, 'its own new player is not used');
  assert.deepEqual(drawn.scrolledWith, { block: 'center' }, 'and the card is scrolled into view');
  assert.equal(away.video.paused, false, 'playing on');
  assert.equal(away.video.loads + away.video.srcWrites, 0, 'never loaded again');
});

// Seen red 2026-11-06 against a copy of web/ whose `take` ignored `narrow`: "on a phone-sized window
// it starts as the tab"; ignoring `tooSmall`: "a list too short for a picture: it starts as the
// tab"; with `letGo` keeping the picture's height: "back in its card, its card's own rules again".
test('on a phone-sized window it starts as the tab; Float it shows only where the browser has picture-in-picture', () => {
  const small = docked({ narrow: true });
  assert.equal(small.dock.parts().box.hidden, true, 'on a phone-sized window it starts as the tab');
  assert.equal(small.dock.parts().tab.hidden, false);
  assert.equal(small.video.paused, false, 'playing on');
  // A message list too short for a picture (a window 150 px tall): the tab too.
  const short = page();
  short.messages.rect = { top: 100, left: 300, right: 1100, bottom: 250, width: 800, height: 150 };
  const s = short.card(URL_A);
  s.video.play();
  short.dock.take(s.video, s.card, { channel: 'global' }, FROM);
  assert.equal(short.dock.parts().box.hidden, true, 'a list too short for a picture: it starts as the tab');
  // In a list with room, the picture is given the height the line and the buttons leave.
  const roomy = docked();
  assert.equal(roomy.video.style.maxHeight, '315px', 'at most 315 px of picture');
  roomy.dock.close();
  assert.equal(roomy.video.style.maxHeight, '', 'back in its card, its card\'s own rules again');
  const pip = docked({ pip: true });
  const float = pip.dock.parts().box.all().find((e) => e.attrs['data-i18n'] === 'chat_video_dock.pip');
  assert.ok(float, 'Float it where the browser has its own picture-in-picture');
  assert.equal(float.attrs['data-tip-desc'], enWords('chat_video_dock.pip_hint'));
  float.click();
  assert.equal(pip.video.pip, 1, 'the browser\'s own, asked for in one line');
});
