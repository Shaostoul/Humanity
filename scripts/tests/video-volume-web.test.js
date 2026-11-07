// The website's video start volume (2026-11-06): web/shared/video-volume.js, the twin of the
// desktop app's Settings > Audio > Videos (crates/humanity-core/src/audio/start_volume.rs). Pure
// node, no browser: a stand-in document and videos, the browser's storage a map. Runs in
// `just rig-tests`.
//
// What it holds: with a set 5% a video's first play starts it at 5% and a second video too,
// whatever the first was turned to; with "my last volume" a new video starts where a person last
// moved one; the volume the page itself sets is not taken for the person's; a video already given
// its start volume is not reset by a second play (a pause and play again keeps the person's level).
//
// Seen red 2026-11-06 with `startVolume` answering the last volume whatever the choice: "a set 5%:
// the first video starts at 5%" (actual 0.8, expected 0.05); and with the page's own volume change
// recorded as the person's: "the page's own change is not the last volume" (actual '0.05').

const test = require("node:test");
const assert = require("node:assert");
const VV = require("../../web/shared/video-volume.js");

function storage(init) {
  const m = new Map(Object.entries(init || {}));
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), map: m };
}

/** A stand-in document that keeps the capture listeners, and a video whose volume set queues a
 * volumechange the way a browser does (the page dispatches it after its own handler returns). */
function page() {
  const handlers = {};
  const doc = { addEventListener: (type, fn) => { handlers[type] = fn; } };
  const fire = (type, target) => handlers[type] && handlers[type]({ type, target });
  const video = () => {
    const v = { tagName: "VIDEO", _volume: 1, queued: [] };
    Object.defineProperty(v, "volume", { get() { return this._volume; }, set(x) { if (x !== this._volume) { this._volume = x; this.queued.push("volumechange"); } } });
    return v;
  };
  const flush = (v) => { while (v.queued.length) fire(v.queued.shift(), v); };
  return { doc, fire, video, flush };
}

test("a set 5%: every new video starts at 5%, whatever the last was turned to", () => {
  const s = storage({ humanity_settings: JSON.stringify({ "video-start": "set", "video-start-pct": "5" }), hos_video_last_volume: "0.8" });
  const p = page();
  VV.install(p.doc, s);
  const a = p.video();
  p.fire("play", a);
  p.flush(a);
  assert.strictEqual(a.volume, 0.05, "a set 5%: the first video starts at 5%");
  assert.strictEqual(s.getItem(VV.LAST_KEY), "0.8", "the page's own change is not the last volume");
  a.volume = 0.6; // the person turns it up in the browser's player
  p.flush(a);
  assert.strictEqual(s.getItem(VV.LAST_KEY), "0.6", "the person's move is the last volume");
  p.fire("play", a); // paused and played again: not reset
  p.flush(a);
  assert.strictEqual(a.volume, 0.6, "a second play keeps the person's level");
  const b = p.video();
  p.fire("play", b);
  p.flush(b);
  assert.strictEqual(b.volume, 0.05, "the next video starts at 5% too");
});

test("my last volume: a new video starts where a person last left one, across visits", () => {
  const s = storage({ humanity_settings: JSON.stringify({ "video-start": "last" }) });
  const p = page();
  VV.install(p.doc, s);
  const a = p.video();
  p.fire("play", a);
  p.flush(a);
  assert.strictEqual(a.volume, 1, "nothing kept yet: full");
  a.volume = 0.3;
  p.flush(a);
  // The next visit: a new page, the same browser storage.
  const q = page();
  VV.install(q.doc, s);
  const b = q.video();
  q.fire("play", b);
  q.flush(b);
  assert.strictEqual(b.volume, 0.3, "the last volume, kept across visits");
  assert.deepStrictEqual(VV.choiceFrom({}), { at: "last", set: 1 }, "the defaults: my last volume, set at full");
  assert.strictEqual(VV.startVolume({ "video-start": "set", "video-start-pct": "250" }, null), 1, "clamped to full");
});
