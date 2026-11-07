/* shared/sound-levels.js: the person's own level for each sound the website plays (2026-11-06),
 * the twin of the desktop app's Settings > Audio > Sound levels
 * (crates/humanity-core/src/audio/sound_levels.rs, src/gui/pages/settings/sound_levels.rs).
 *
 * The operator, told the levelled button click came out about 9 dB louder: "Can we edit them in the
 * settings menu?" The desktop app lists every sound the game ships. The website plays no game
 * sounds at all: the only sounds it plays are the notification sounds named in
 * data/notifications/notifications.json's `sounds` (a chat message's chime, an alert, and the
 * three others a person can pick for a kind of notice), in the chat (chat-notify.js playSound)
 * and from Settings' Play buttons. So Settings > Audio & Video > Sound levels lists just those,
 * each with a level from -24 dB to +12 dB (0 dB is the sound as the website plays it, at 60%
 * volume), Play and Reset, and Reset all, and says the rest are set in the desktop app.
 *
 * Kept in this browser: localStorage `hos_sound_levels`, {sound id: dB}, only the sounds changed
 * (a reset takes the entry out), on the half-dB grid the desktop app keeps. Separate from the
 * desktop app's levels (those are in its config), as every website preference is.
 *
 * A browser's <audio> volume stops at 1.0, so a level that asks for more than that (above +4.4 dB
 * over the 60% base) goes through Web Audio: the element at full volume into a GainNode. The sound
 * files are on the page's own server (/assets/), so Web Audio may read them.
 *
 * Loaded by the chat (chat/index.html) and Settings (pages/settings.html). Pure enough to test in
 * node: scripts/tests/sound-levels-web.test.js.
 */
(function (root) {
  'use strict';

  var KEY = 'hos_sound_levels';
  var MIN_DB = -24;
  var MAX_DB = 12;
  var STEP_DB = 0.5;
  /** The volume the website plays a notification sound at with no level set (chat-notify.js). */
  var BASE = 0.6;

  /** A level as it is kept: on the half-dB grid, inside -24..12; null for a number that is not one. */
  function keptDb(db) {
    if (db === null || db === undefined || db === '') return null;
    db = Number(db);
    if (!isFinite(db)) return null;
    var s = Math.round(db / STEP_DB) * STEP_DB;
    s = Math.min(MAX_DB, Math.max(MIN_DB, s));
    return s === 0 ? 0 : s; // never -0
  }

  function load(storage) {
    try {
      var m = JSON.parse(storage.getItem(KEY));
      return m && typeof m === 'object' && !Array.isArray(m) ? m : {};
    } catch (e) {
      return {};
    }
  }

  function save(storage, m) {
    try {
      if (Object.keys(m).length) storage.setItem(KEY, JSON.stringify(m));
      else if (typeof storage.removeItem === 'function') storage.removeItem(KEY);
      else storage.setItem(KEY, '{}');
    } catch (e) { /* storage refused: the level lasts this visit only */ }
  }

  /** The level of the sound `id`, dB (0 for one never set). */
  function levelDb(storage, id) {
    var v = keptDb(load(storage)[id]);
    return v === null ? 0 : v;
  }

  /** Set the sound `id` to `db` (0 takes its entry out). True when the level changed. */
  function setLevel(storage, id, db) {
    var v = keptDb(db);
    if (v === null) return false;
    var before = levelDb(storage, id);
    var m = load(storage);
    if (v === 0) delete m[id];
    else m[id] = v;
    save(storage, m);
    return before !== v;
  }

  /** Put the sound `id` back to 0 dB. True when it had a level. */
  function reset(storage, id) {
    var m = load(storage);
    if (!(id in m)) return false;
    delete m[id];
    save(storage, m);
    return true;
  }

  /** Put every sound back to 0 dB. True when any had a level. */
  function resetAll(storage) {
    var any = Object.keys(load(storage)).length > 0;
    save(storage, {});
    return any;
  }

  /** A level in dB as the amplitude it multiplies a sound by: 10^(dB/20). */
  function gain(db) {
    return Math.pow(10, db / 20);
  }

  /** The amplitude the website plays the sound `id` at: 60% times its level. */
  function amplitude(storage, id) {
    return BASE * gain(levelDb(storage, id));
  }

  /** How to reach an amplitude: the element's own volume when it is 1 or under; above that the
   *  element at full volume and a Web Audio gain of the amplitude. */
  function route(amp) {
    return amp <= 1 ? { volume: amp, gain: null } : { volume: 1, gain: amp };
  }

  /** A level as a row shows it: "0.0 dB" at the default, else signed ("+3.5 dB", "-6.0 dB"). */
  function levelWords(db, T) {
    var n = db === 0 ? '0.0' : (db > 0 ? '+' : '') + db.toFixed(1);
    return T ? T('sound_levels.db', { db: n }) : n + ' dB';
  }

  var sharedCtx = null;

  /**
   * Play the sound file `path` (under /assets/) for the sound `id` at its level. `env` gives the
   * browser's Audio, AudioContext and storage (the page's own when not given). Returns
   * { audio, amp, gain } (gain: the GainNode's value, null when the element's volume did it), or
   * null when this browser cannot play sounds. `onFail` is called when the browser refuses to play
   * (the chat then plays its synthesized chime instead).
   */
  function play(path, id, env, onFail) {
    env = env || {};
    var AudioCtor = env.Audio || root.Audio;
    var storage = env.storage || root.localStorage;
    if (typeof AudioCtor !== 'function') {
      if (onFail) onFail();
      return null;
    }
    var amp = amplitude(storage, id);
    var r = route(amp);
    var a = new AudioCtor('/assets/' + path);
    var usedGain = null;
    if (r.gain !== null) {
      var Ctx = env.AudioContext !== undefined ? env.AudioContext : (root.AudioContext || root.webkitAudioContext || null);
      try {
        var ctx = env.ctx || sharedCtx || (Ctx ? new Ctx() : null);
        if (ctx) {
          if (!env.ctx) sharedCtx = ctx;
          var src = ctx.createMediaElementSource(a);
          var g = ctx.createGain();
          g.gain.value = r.gain;
          src.connect(g);
          g.connect(ctx.destination);
          if (ctx.state === 'suspended' && typeof ctx.resume === 'function') ctx.resume();
          usedGain = r.gain;
        }
      } catch (e) { usedGain = null; }
    }
    // Without Web Audio a level past full volume plays at full volume, the most the element can.
    a.volume = usedGain === null ? Math.min(1, r.volume) : 1;
    try {
      var p = a.play();
      if (p && typeof p.catch === 'function' && onFail) p.catch(onFail);
    } catch (e) {
      if (onFail) onFail();
    }
    return { audio: a, amp: amp, gain: usedGain };
  }

  /**
   * Draw the rows into `box`: one per sound of `sounds` (notifications.json's list: {id, path,
   * label}), named by its label key through `T` (i18n.t), with its level slider, the level in dB,
   * Play and Reset. `doc` and `storage` default to the page's. Returns a function that redraws.
   */
  function render(box, sounds, T, opts) {
    opts = opts || {};
    var doc = opts.doc || root.document;
    var storage = opts.storage || root.localStorage;
    var I = opts.i18n || root.i18n || null;
    function draw() {
      box.innerHTML = '';
      (sounds || []).forEach(function (s) {
        var row = doc.createElement('div');
        row.className = 'setting-row sound-level-row';
        var label = doc.createElement('div');
        label.className = 'setting-label';
        var strong = doc.createElement('strong');
        strong.id = 'sound-level-' + s.id.replace(/[^a-z0-9_-]/gi, '-');
        strong.textContent = T(s.label);
        label.appendChild(strong);
        row.appendChild(label);
        var level = levelDb(storage, s.id);
        var slider = doc.createElement('input');
        slider.type = 'range';
        slider.className = 'setting-range';
        slider.min = String(MIN_DB);
        slider.max = String(MAX_DB);
        slider.step = String(STEP_DB);
        slider.value = String(level);
        slider.setAttribute('aria-labelledby', strong.id);
        if (I) I.hint(slider, 'sound_levels.level_hint');
        var words = doc.createElement('span');
        words.className = 'sound-level-db';
        words.textContent = levelWords(level, T);
        slider.addEventListener('input', function () {
          setLevel(storage, s.id, slider.value);
          var now = levelDb(storage, s.id);
          words.textContent = levelWords(now, T);
          reset.disabled = now === 0;
          if (opts.onChange) opts.onChange();
        });
        row.appendChild(slider);
        row.appendChild(words);
        var playBtn = doc.createElement('button');
        playBtn.type = 'button';
        playBtn.className = 'btn btn-secondary';
        playBtn.textContent = T('sound_levels.play');
        if (I) I.hint(playBtn, 'sound_levels.play_hint');
        playBtn.addEventListener('click', function () { play(s.path, s.id, opts.env); });
        row.appendChild(playBtn);
        var reset = doc.createElement('button');
        reset.type = 'button';
        reset.className = 'btn btn-secondary';
        reset.textContent = T('sound_levels.reset');
        if (I) I.hint(reset, 'sound_levels.reset_hint');
        reset.disabled = level === 0;
        reset.addEventListener('click', function () {
          resetLevel(storage, s.id);
          draw();
          if (opts.onChange) opts.onChange();
        });
        row.appendChild(reset);
        box.appendChild(row);
      });
    }
    draw();
    return draw;
  }

  // `reset` is also a local name in render; the module's own under another name for it.
  var resetLevel = reset;

  var api = {
    KEY: KEY,
    MIN_DB: MIN_DB,
    MAX_DB: MAX_DB,
    STEP_DB: STEP_DB,
    BASE: BASE,
    keptDb: keptDb,
    levelDb: levelDb,
    setLevel: setLevel,
    reset: reset,
    resetAll: resetAll,
    gain: gain,
    amplitude: amplitude,
    route: route,
    levelWords: levelWords,
    play: play,
    render: render
  };
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  } else {
    root.HosSoundLevels = api;
  }
})(typeof window !== 'undefined' ? window : this);
