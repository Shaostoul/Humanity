// The website's sound levels (2026-11-06): web/shared/sound-levels.js, the twin of the desktop
// app's Settings > Audio > Sound levels (crates/humanity-core/src/audio/sound_levels.rs). The
// website plays only notification sounds, so those are the sounds it levels. Pure node, no
// browser: the browser's storage a map, Audio and AudioContext stand-ins, a stand-in document.
// Runs in `just rig-tests`.
//
// What it holds: a level moves the sound by exactly its dB (0.6 x 10^(dB/20)), kept on the half-dB
// grid inside -24..+12; a sound with no level plays at the website's 60%; Reset and Reset all
// take the levels out; a level past what an <audio> element's volume can reach goes through a Web
// Audio gain (and without Web Audio plays at full volume, the most it can); the rows Settings
// draws set and show a level, and Play plays at it.
//
// Seen red 2026-11-06, each on purpose: with `gain` dividing by 10 (a power ratio taken for an
// amplitude), "-6 dB plays at 0.6 x 10^(-6/20)" and, in the rows, "Play plays at the slider's
// level"; with `setLevel` keeping a 0, "a level set back to 0 is not kept" (actual
// '{"sfx.success":0}', expected null); with `route` always using the element's volume, "+12 dB
// goes through a Web Audio gain".

const test = require("node:test");
const assert = require("node:assert");
const SL = require("../../web/shared/sound-levels.js");

function storage(init) {
  const m = new Map(Object.entries(init || {}));
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), map: m };
}

/** A stand-in Audio that records what it was asked, and an AudioContext that records its graph. */
function audioEnv(withWebAudio) {
  const played = [];
  function Audio(src) { this.src = src; this.volume = 1; this.play = () => { played.push(this); return Promise.resolve(); }; }
  const graph = [];
  function Ctx() {
    this.state = "running";
    this.destination = { name: "speakers" };
    this.createMediaElementSource = (el) => ({ el, connect: (to) => graph.push(["source", to]) });
    this.createGain = () => { const g = { gain: { value: 1 }, connect: (to) => graph.push(["gain", to]) }; return g; };
  }
  return { env: { Audio, AudioContext: withWebAudio ? Ctx : null }, played, graph };
}

const close = (a, b) => Math.abs(a - b) < 1e-12;

test("a level moves a sound by exactly its dB; a sound with no level plays at the website's 60%", () => {
  const s = storage();
  assert.strictEqual(SL.amplitude(s, "sfx.chat_message"), 0.6, "no level: 60%, as the website always played it");
  assert.ok(SL.setLevel(s, "sfx.chat_message", -6), "a new level is a change");
  assert.ok(close(SL.amplitude(s, "sfx.chat_message"), 0.6 * Math.pow(10, -6 / 20)), "-6 dB plays at 0.6 x 10^(-6/20)");
  assert.strictEqual(SL.amplitude(s, "sfx.notification"), 0.6, "another sound, never set, plays at its default");
  SL.setLevel(s, "sfx.chat_message", 3.26);
  assert.strictEqual(SL.levelDb(s, "sfx.chat_message"), 3.5, "kept on the half-dB grid");
  SL.setLevel(s, "sfx.chat_message", 40);
  assert.strictEqual(SL.levelDb(s, "sfx.chat_message"), SL.MAX_DB, "held at +12");
  SL.setLevel(s, "sfx.chat_message", -99);
  assert.strictEqual(SL.levelDb(s, "sfx.chat_message"), SL.MIN_DB, "held at -24");
  assert.strictEqual(SL.setLevel(s, "sfx.chat_message", "not a number"), false, "a word is never kept");
});

test("Reset and Reset all put sounds back to 0 dB and keep nothing for them", () => {
  const s = storage();
  SL.setLevel(s, "sfx.success", 4);
  SL.setLevel(s, "sfx.success", 0);
  assert.strictEqual(s.getItem(SL.KEY), null, "a level set back to 0 is not kept");
  SL.setLevel(s, "sfx.success", -9);
  SL.setLevel(s, "sfx.menu_open", 2);
  assert.ok(SL.reset(s, "sfx.success"), "it had a level");
  assert.strictEqual(SL.levelDb(s, "sfx.success"), 0, "Reset: back to 0 dB");
  assert.strictEqual(SL.amplitude(s, "sfx.success"), 0.6, "exactly the default");
  assert.strictEqual(SL.levelDb(s, "sfx.menu_open"), 2, "the other keeps its level");
  assert.ok(SL.resetAll(s), "Reset all had a level to reset");
  assert.strictEqual(s.getItem(SL.KEY), null, "and keeps nothing");
  assert.strictEqual(SL.resetAll(s), false, "a second Reset all changes nothing");
});

test("a level within the element's volume uses it; past it, a Web Audio gain (or full volume without one)", () => {
  const s = storage({ hos_sound_levels: JSON.stringify({ "sfx.notification": -6, "sfx.chat_message": 12 }) });
  const quiet = audioEnv(true);
  const q = SL.play("audio/ui/notification.ogg", "sfx.notification", Object.assign({ storage: s }, quiet.env));
  assert.strictEqual(q.audio.src, "/assets/audio/ui/notification.ogg");
  assert.ok(close(q.audio.volume, 0.6 * Math.pow(10, -6 / 20)), "-6 dB is the element's own volume");
  assert.strictEqual(q.gain, null, "no Web Audio needed");
  assert.strictEqual(quiet.played.length, 1, "and it plays");
  const loud = audioEnv(true);
  const l = SL.play("audio/ui/chat_message.ogg", "sfx.chat_message", Object.assign({ storage: s, ctx: new loud.env.AudioContext() }, loud.env));
  assert.ok(l.gain !== null && close(l.gain, 0.6 * Math.pow(10, 12 / 20)), "+12 dB goes through a Web Audio gain");
  assert.strictEqual(l.audio.volume, 1, "the element at full volume under it");
  assert.deepStrictEqual(loud.graph.map((g) => g[0]), ["source", "gain"], "element -> gain -> speakers");
  const bare = audioEnv(false);
  const b = SL.play("audio/ui/chat_message.ogg", "sfx.chat_message", Object.assign({ storage: s }, bare.env));
  assert.strictEqual(b.audio.volume, 1, "without Web Audio: full volume, the most the element can");
  let failed = false;
  assert.strictEqual(SL.play("x.ogg", "sfx.x", { storage: s, Audio: null }, () => { failed = true; }), null);
  assert.ok(failed, "a browser that cannot play sounds says so (the chat plays its chime)");
});

/** A stand-in document: enough of createElement for the rows. */
function fakeDoc() {
  function el(tag) {
    const e = { tag, children: [], listeners: {}, attrs: {}, dataset: {}, disabled: false, textContent: "" };
    e.appendChild = (c) => { e.children.push(c); return c; };
    e.addEventListener = (type, fn) => { e.listeners[type] = fn; };
    e.setAttribute = (k, v) => { e.attrs[k] = String(v); };
    Object.defineProperty(e, "innerHTML", { set() { e.children = []; } });
    return e;
  }
  return { createElement: el, root: el("div") };
}

test("the rows set and show a level, Reset takes it out, and Play plays at it", () => {
  const s = storage();
  const doc = fakeDoc();
  const hinted = [];
  const words = { "sound_levels.play": "Play", "sound_levels.reset": "Reset", "notifications.sound_message": "Message", "notifications.sound_alert": "Alert" };
  const T = (k, v) => (k === "sound_levels.db" ? v.db + " dB" : words[k] || k);
  const sounds = [{ id: "sfx.chat_message", path: "audio/ui/chat_message.ogg", label: "notifications.sound_message" }, { id: "sfx.notification", path: "audio/ui/notification.ogg", label: "notifications.sound_alert" }];
  const a = audioEnv(true);
  SL.render(doc.root, sounds, T, { doc, storage: s, i18n: { hint: (e, key) => hinted.push(key) }, env: Object.assign({ storage: s }, a.env) });
  assert.strictEqual(doc.root.children.length, 2, "one row per sound the website plays");
  const row = doc.root.children[0];
  const [label, slider, shown, play, reset] = row.children;
  assert.strictEqual(label.children[0].textContent, "Message", "named by its words");
  assert.strictEqual(shown.textContent, "0.0 dB");
  assert.strictEqual(reset.disabled, true, "nothing to reset yet");
  slider.value = "-6";
  slider.listeners.input();
  assert.strictEqual(SL.levelDb(s, "sfx.chat_message"), -6, "the slider sets the level");
  assert.strictEqual(shown.textContent, "-6.0 dB", "and the row shows it");
  assert.strictEqual(reset.disabled, false);
  play.listeners.click();
  assert.ok(close(a.played[0].volume, 0.6 * Math.pow(10, -6 / 20)), "Play plays at the slider's level");
  reset.listeners.click();
  assert.strictEqual(SL.levelDb(s, "sfx.chat_message"), 0, "Reset puts it back to 0 dB");
  assert.strictEqual(doc.root.children[0].children[2].textContent, "0.0 dB", "redrawn at 0 dB");
  assert.deepStrictEqual([...new Set(hinted)].sort(), ["sound_levels.level_hint", "sound_levels.play_hint", "sound_levels.reset_hint"], "every control has its hint");
});
