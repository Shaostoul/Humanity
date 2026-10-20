// ── files-sound.js ────────────────────────────────────────────────────────
// A sound or a song shown in the Files page's middle pane on the website (2026-10-19), the desktop's
// mirrored: src/gui/widgets/sound_view.rs (Play here, then the sound played with its waveform) and
// src/gui/widgets/waveform.rs (the waveform's peaks, the spectrum's bands, a press on it to go there).
// The operator, 2026-10-19: "middle is the fit to area picture, 3D model, sound/music visualizer".
//
// NOTHING IS FETCHED BEFORE PLAY HERE. Pressed, the sound is fetched ONCE from its own address (the
// page's `fetchSound`: its /uploads/ address, or the members' file route with the upload token in a
// header, as a 3D model is; never the counting route, since listening here is not a download), and
// then:
// - it plays in the browser's own <audio> element (its Play, Pause, time and volume), from an
//   address of the page's own (URL.createObjectURL) over the bytes already fetched;
// - it is READ ONCE by Web Audio's decodeAudioData into the PEAKS of the whole track: the lowest and
//   highest sample in each of 1024 equal stretches (`peaksOf`, the desktop's BUCKETS). Nothing is
//   read or decoded again while it is shown: each frame draws those peaks, the part already played in
//   the accent colour and the rest muted, with a line where it is. A press (or a drag) on it goes
//   there (`seekFraction`). A sound longer than 20 minutes plays without the waveform (decoding holds
//   the whole sound in memory) and says so;
// - while it plays, its SPECTRUM is drawn above the waveform, live, from an AnalyserNode on what is
//   playing: 24 bands spaced evenly in pitch from 40 Hz up, each from 60 dB below full scale
//   (nothing) to full scale (`bandLevels`, the desktop's BANDS, LOWEST_HZ, FLOOR_DB).
// Every colour is the theme's, read from its CSS variables (--accent, --text-muted, --text, --border,
// --bg-tertiary); the words are the desktop's, "in the app" made "here".
//
// A classic script in the browser (before files-item.js), a CommonJS module under Node, where
// scripts/tests/files-web-panes.test.js runs it (with the fixture
// tests/fixtures/media/two-tones-440-then-2000.wav, decoded by the test).
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  /** The stretches of the track the waveform keeps the peaks of (the desktop's BUCKETS). */
  const BUCKETS = 1024;
  /** The spectrum's bands, and the pitches they span (the desktop's BANDS, LOWEST_HZ, HIGHEST_HZ). */
  const BANDS = 24;
  const LOWEST_HZ = 40;
  const HIGHEST_HZ = 16000;
  /** How quiet a band can be before it shows nothing (dB below full scale). */
  const FLOOR_DB = 60;
  /**
   * What a full-scale sine reads in an AnalyserNode (dB): its transform is scaled by 1/N through a
   * Blackman window (coherent gain 0.42), so the sine's bin holds 0.42 / 2 of full scale, -13.6 dB.
   * Added back, a full-scale sine fills its band, as on the desktop.
   */
  const ANALYSER_FULL_SCALE_DB = -13.6;
  /** The analyser's transform length. */
  const FFT = 2048;
  /** The rate the waveform is read at: plenty for its peaks, and a quarter of the memory of 44.1 kHz stereo. */
  const DECODE_RATE = 22050;
  /** The longest sound whose waveform is drawn (decoding holds all of it in memory). */
  const WAVE_MAX_S = 20 * 60;
  /** The largest sound this page fetches to play. */
  const MAX_SOUND_BYTES = 200 * 1024 * 1024;

  const WORDS = Object.freeze({
    playHere: 'Play here',
    playHereTip: 'Fetch the sound and play it here, with its waveform.',
    fetching: 'Fetching the sound...',
    reading: 'Reading the sound for its waveform...',
    seekTip: 'Press anywhere on the waveform to go there.',
    tooLong: 'Its waveform is drawn for sounds up to 20 minutes long; this one plays without it.',
    waveLabel: 'The waveform. Press anywhere on it to go there.',
    playedTip: 'The part already played is in the accent colour; while it plays, its spectrum moves above it.',
  });
  /** What it says when the sound will not play here: the reason, then what to do. */
  function cannotPlayWords(why) {
    return 'This sound cannot play here: ' + why + '. Download it to hear it in another player.';
  }
  /** What it says about a sound larger than this page fetches. */
  function tooLargeWords(size) {
    return 'This sound is ' + size + ', larger than the 200 MB this page plays. Download it to hear it.';
  }

  /**
   * The peaks of a sound whose samples are `channels` (one array per channel, each -1 to 1): for each
   * of up to `buckets` equal stretches, [lowest, highest] over every channel. A sample that is not a
   * number counts as silence; one past full scale as full scale.
   */
  function peaksOf(channels, buckets) {
    const chans = Array.from(channels || []).filter((c) => c && typeof c.length === 'number');
    const n = chans.length ? Math.min(...chans.map((c) => c.length)) : 0;
    const count = Math.max(0, Math.min(Number(buckets) || BUCKETS, n));
    const out = [];
    for (let b = 0; b < count; b++) {
      const from = Math.floor(b * n / count);
      const to = Math.max(from + 1, Math.floor((b + 1) * n / count));
      let lo = Infinity;
      let hi = -Infinity;
      for (const c of chans) {
        for (let k = from; k < to; k++) {
          let s = c[k];
          s = Number.isFinite(s) ? Math.max(-1, Math.min(1, s)) : 0;
          if (s < lo) lo = s;
          if (s > hi) hi = s;
        }
      }
      out.push([lo, hi]);
    }
    return out;
  }

  /** The loudest peak, for scaling the drawing (a quiet recording still fills its height). */
  function loudest(peaks) {
    let m = 0;
    for (const p of peaks || []) m = Math.max(m, Math.abs(p[0]), Math.abs(p[1]));
    return m;
  }

  /** The band edges in Hz for a sound at `rate`: BANDS + 1, evenly spaced in pitch (the desktop's band_edges_hz). */
  function bandEdges(rate) {
    const top = Math.max(LOWEST_HZ * 2, Math.min(HIGHEST_HZ, (Number(rate) || 0) / 2));
    const ratio = Math.pow(top / LOWEST_HZ, 1 / BANDS);
    const out = [];
    for (let i = 0; i <= BANDS; i++) out.push(LOWEST_HZ * Math.pow(ratio, i));
    return out;
  }

  /**
   * The bands' levels, 0 to 1, from an analyser's dB per bin (`db`, getFloatFrequencyData's, the
   * transform `fft` long at `rate`): the loudest bin in each band, from FLOOR_DB below full scale to
   * full scale. The bins of each band as the desktop's Builder picks them.
   */
  function bandLevels(db, rate, fft) {
    const half = Math.floor((Number(fft) || FFT) / 2);
    const perBin = (Number(rate) || 44100) / (Number(fft) || FFT);
    const edges = bandEdges(rate);
    const out = [];
    let last = 0;
    for (let b = 0; b < BANDS; b++) {
      const lo = Math.min(Math.max(Math.floor(edges[b] / perBin), last + 1), half - 1);
      const hi = Math.min(Math.max(Math.floor(edges[b + 1] / perBin), lo), half - 1);
      last = hi;
      let best = -Infinity;
      for (let k = lo; k <= hi; k++) { const v = db ? Number(db[k]) : -Infinity; if (v > best) best = v; }
      const rel = best - ANALYSER_FULL_SCALE_DB;
      out.push(Number.isFinite(rel) ? Math.max(0, Math.min(1, (rel + FLOOR_DB) / FLOOR_DB)) : 0);
    }
    return out;
  }

  /** Where along a waveform drawn from `left`, `width` wide, a press at `x` is: 0 (its start) to 1 (its end). */
  function seekFraction(left, width, x) {
    if (!(width > 0)) return 0;
    return Math.max(0, Math.min(1, (Number(x) - Number(left)) / width));
  }

  /**
   * The browser's decoder: `bytes` (an ArrayBuffer) read once by Web Audio at DECODE_RATE into
   * {channels, sampleRate, duration}. Throws Error(why) where it cannot.
   */
  async function decodeInBrowser(bytes) {
    const Off = root.OfflineAudioContext || root.webkitOfflineAudioContext;
    if (!Off) throw new Error('this browser cannot read sounds');
    let ctx;
    try { ctx = new Off(1, 1, DECODE_RATE); } catch (e) { ctx = new Off(1, 1, 44100); }
    const buf = await new Promise((resolve, reject) => {
      let settled = false;
      const ok = (b) => { if (!settled) { settled = true; resolve(b); } };
      const no = (e) => { if (!settled) { settled = true; reject(e || new Error('it could not be read')); } };
      try {
        const p = ctx.decodeAudioData(bytes, ok, no);
        if (p && typeof p.then === 'function') p.then(ok, no);
      } catch (e) { no(e); }
    });
    const channels = [];
    for (let c = 0; c < buf.numberOfChannels; c++) channels.push(buf.getChannelData(c));
    return { channels, sampleRate: buf.sampleRate, duration: buf.duration };
  }

  /**
   * A sound in the middle pane. `o`:
   *   h, doc, win (requestAnimationFrame, devicePixelRatio, getComputedStyle; optional),
   *   URL (createObjectURL, revokeObjectURL), name (the file's name as shown), size (its bytes),
   *   sizeWords(bytes), fetchSound(signal) -> Promise<Blob> (its own address, never the counting
   *   route), decode(arrayBuffer) -> Promise<{channels, sampleRate, duration}> (the browser's
   *   decodeAudioData when not given), AudioContext (for the live spectrum; the browser's when not
   *   given, none under Node), Abort (AbortController), onChange() (called when it moved on).
   * Returns {el, start(), close(), state}. `state.phase`: 'idle' (Play here), 'fetching', 'reading'
   * (its waveform), 'ready', 'failed'; `state.peaks` once read, `state.problem` why not.
   */
  function createSoundView(o) {
    const h = o.h;
    const win = o.win || null;
    const URLs = o.URL || null;
    const decode = typeof o.decode === 'function' ? o.decode : decodeInBrowser;
    const AC = o.AudioContext !== undefined ? o.AudioContext : (root.AudioContext || root.webkitAudioContext || null);
    const state = { phase: 'idle', peaks: null, duration: 0, problem: '', note: '', fetches: 0, decodes: 0 };
    const el = h('div', { id: 'files-sound', className: 'files-sound' });
    let ctl = null;       // the fetch's AbortController
    let url = '';         // the object URL the <audio> plays
    let audio = null;
    let canvas = null;
    let noteEl = null;
    let actx = null;      // the AudioContext the analyser is in
    let analyser = null;
    let bins = null;
    let frame = 0;        // the animation frame asked for
    let closed = false;
    let seeking = false;

    const changed = () => { if (typeof o.onChange === 'function') o.onChange(state); };

    function drawIdle() {
      const play = h('button', {
        type: 'button', id: 'files-sound-play', className: 'files-btn files-btn-primary', text: WORDS.playHere,
        title: WORDS.playHereTip, on: { click: () => start() },
      });
      el.replaceChildren(
        h('p', { className: 'files-sound-line', id: 'files-sound-line', text: o.name + ', ' + o.sizeWords(o.size) }),
        play
      );
    }

    function say(words, warn) {
      state.note = words || '';
      if (!noteEl) return;
      noteEl.textContent = state.note;
      noteEl.className = 'files-sound-note' + (warn ? ' files-warn' : ' files-hint');
      noteEl.hidden = !state.note;
    }

    function fail(why) {
      state.phase = 'failed';
      state.problem = cannotPlayWords(why);
      if (!audio) {
        el.replaceChildren(h('p', { className: 'files-warn files-sound-note', id: 'files-sound-note', text: state.problem }));
      } else {
        say(state.problem, true);
      }
      changed();
    }

    /** Play here: fetch the sound, play it, read it once for its waveform. */
    async function start() {
      if (state.phase !== 'idle' || closed) return;
      state.phase = 'fetching';
      state.fetches++;
      // Made now, while the press is the reason (a browser lets sound start only after one).
      if (AC) { try { actx = new AC(); } catch (e) { actx = null; } }
      el.replaceChildren(h('p', { className: 'files-hint files-sound-note', id: 'files-sound-note', text: WORDS.fetching }));
      changed();
      const Abort = o.Abort || (typeof AbortController !== 'undefined' ? AbortController : null);
      ctl = Abort ? new Abort() : null;
      let blob;
      try {
        blob = await o.fetchSound(ctl ? ctl.signal : undefined);
      } catch (e) {
        if (closed) return;
        fail(e && e.message ? e.message : String(e));
        return;
      }
      ctl = null;
      if (closed) return;
      if (!blob || (typeof blob.size === 'number' && blob.size > MAX_SOUND_BYTES)) {
        state.phase = 'failed';
        state.problem = tooLargeWords(o.sizeWords(blob ? blob.size : o.size));
        el.replaceChildren(h('p', { className: 'files-warn files-sound-note', id: 'files-sound-note', text: state.problem }));
        changed();
        return;
      }
      showPlayer(blob);
      readWaveform(blob);
    }

    /** The waveform's canvas, the browser's <audio> under it, and the line under that. */
    function showPlayer(blob) {
      try { url = URLs ? URLs.createObjectURL(blob) : ''; } catch (e) { url = ''; }
      // Its hover hint says what the colours mean (the site's hint would otherwise guess a line).
      canvas = h('canvas', {
        id: 'files-sound-wave', className: 'files-sound-wave', title: WORDS.seekTip, dataset: { tipDesc: WORDS.playedTip },
        attrs: { role: 'img', 'aria-label': WORDS.waveLabel },
      });
      audio = h('audio', { id: 'files-sound-audio', className: 'files-sound-audio', controls: true, preload: 'auto', src: url || undefined });
      noteEl = h('p', { id: 'files-sound-note', className: 'files-sound-note files-hint', hidden: true });
      el.replaceChildren(canvas, audio, noteEl);
      audio.addEventListener('play', () => { listen(); loop(); });
      audio.addEventListener('pause', () => draw());
      audio.addEventListener('ended', () => draw());
      audio.addEventListener('timeupdate', () => { if (!frame) draw(); });
      audio.addEventListener('loadedmetadata', () => { state.duration = Number(audio.duration) || state.duration; draw(); });
      audio.addEventListener('error', () => { if (!closed && state.phase !== 'failed') fail('this browser cannot play its kind of sound'); });
      canvas.addEventListener('pointerdown', (e) => { seeking = true; seekTo(e); try { canvas.setPointerCapture(e.pointerId); } catch (err) { /* follows while over it */ } });
      canvas.addEventListener('pointermove', (e) => { if (seeking) seekTo(e); });
      canvas.addEventListener('pointerup', () => { seeking = false; });
      canvas.addEventListener('pointercancel', () => { seeking = false; });
      if (win && typeof win.addEventListener === 'function') win.addEventListener('resize', draw);
      // Played at once: Play here was the press (a browser that wants another waits for its Play).
      try { const p = audio.play && audio.play(); if (p && typeof p.catch === 'function') p.catch(() => {}); } catch (e) { /* its own Play */ }
      draw();
    }

    /** Read the whole sound once, for its peaks (not when it is longer than WAVE_MAX_S). */
    async function readWaveform(blob) {
      state.phase = 'reading';
      say(WORDS.reading);
      changed();
      let bytes;
      try { bytes = await blob.arrayBuffer(); } catch (e) { if (!closed) fail(e && e.message ? e.message : String(e)); return; }
      if (closed) return;
      let got;
      try {
        state.decodes++;
        got = await decode(bytes);
      } catch (e) {
        if (!closed) fail(e && e.message ? e.message : 'it could not be read');
        return;
      }
      if (closed) return;
      const duration = Number(got && got.duration) || 0;
      state.duration = duration || state.duration;
      if (duration > WAVE_MAX_S) {
        state.phase = 'ready';
        say(WORDS.tooLong);
        changed();
        return;
      }
      state.peaks = peaksOf(got.channels, BUCKETS);
      state.phase = 'ready';
      say('');
      draw();
      changed();
    }

    /** The spectrum's analyser, on what is playing (once; nothing more when the browser has none). */
    function listen() {
      if (!actx || analyser || !audio) {
        if (actx && typeof actx.resume === 'function') { try { Promise.resolve(actx.resume()).catch(() => {}); } catch (e) { /* stays as it is */ } }
        return;
      }
      try {
        const src = actx.createMediaElementSource(audio);
        analyser = actx.createAnalyser();
        analyser.fftSize = FFT;
        src.connect(analyser);
        analyser.connect(actx.destination);
        bins = new Float32Array(analyser.frequencyBinCount);
        if (typeof actx.resume === 'function') Promise.resolve(actx.resume()).catch(() => {});
      } catch (e) {
        analyser = null;
      }
    }

    function loop() {
      if (closed || frame || !win || typeof win.requestAnimationFrame !== 'function') { draw(); return; }
      const tick = () => {
        frame = 0;
        draw();
        if (!closed && audio && !audio.paused && !audio.ended) frame = win.requestAnimationFrame(tick);
      };
      frame = win.requestAnimationFrame(tick);
    }

    function seekTo(e) {
      if (!canvas || !audio) return;
      let r = null;
      try { r = canvas.getBoundingClientRect(); } catch (err) { r = null; }
      if (!r) return;
      const pad = padOf();
      const d = Number(audio.duration) || state.duration;
      if (!(d > 0)) return;
      try { audio.currentTime = seekFraction(r.left + pad, r.width - 2 * pad, e.clientX) * d; } catch (err) { /* not seekable yet */ }
      draw();
    }

    const padOf = () => 6;

    /** One picture of it: the spectrum (while there is one) over the waveform, the part played in the accent colour. */
    function draw() {
      if (!canvas || typeof canvas.getContext !== 'function') return;
      const g = canvas.getContext('2d');
      if (!g) return;
      const dpr = (win && win.devicePixelRatio) || 1;
      const cw = canvas.clientWidth || 600;
      const ch = canvas.clientHeight || 200;
      if (canvas.width !== Math.round(cw * dpr)) canvas.width = Math.round(cw * dpr);
      if (canvas.height !== Math.round(ch * dpr)) canvas.height = Math.round(ch * dpr);
      g.setTransform(dpr, 0, 0, dpr, 0, 0);
      const style = win && typeof win.getComputedStyle === 'function' ? win.getComputedStyle(canvas) : null;
      const colour = (name) => (style ? String(style.getPropertyValue(name) || '').trim() : '') || 'currentColor';
      const accent = colour('--accent');
      const muted = colour('--text-muted');
      g.clearRect(0, 0, cw, ch);
      g.fillStyle = colour('--bg-tertiary');
      g.fillRect(0, 0, cw, ch);
      const pad = padOf();
      const inner = { x: pad, y: pad, w: Math.max(1, cw - 2 * pad), h: Math.max(1, ch - 2 * pad) };
      // The spectrum while it plays, over the top two fifths.
      let specH = 0;
      if (analyser && bins) {
        specH = Math.floor(inner.h * 0.4);
        analyser.getFloatFrequencyData(bins);
        const levels = bandLevels(bins, actx.sampleRate, FFT);
        const gap = 3;
        const bw = Math.max(1, (inner.w - gap * (BANDS - 1)) / BANDS);
        g.fillStyle = accent;
        g.globalAlpha = 0.85;
        levels.forEach((lv, b) => {
          const bh = Math.max(1, lv * specH);
          g.fillRect(inner.x + b * (bw + gap), inner.y + specH - bh, bw, bh);
        });
        g.globalAlpha = 1;
      }
      const wave = { x: inner.x, y: inner.y + (specH ? specH + pad : 0), w: inner.w, h: inner.h - (specH ? specH + pad : 0) };
      const mid = wave.y + wave.h / 2;
      g.fillStyle = colour('--border');
      g.fillRect(wave.x, mid, wave.w, 1);
      const d = audio ? Number(audio.duration) || state.duration : state.duration;
      const played = d > 0 && audio ? Math.max(0, Math.min(1, (Number(audio.currentTime) || 0) / d)) : 0;
      const peaks = state.peaks;
      if (peaks && peaks.length) {
        const scale = 1 / Math.max(0.05, loudest(peaks));
        const cols = Math.max(1, Math.floor(wave.w));
        const n = peaks.length;
        for (let c = 0; c < cols; c++) {
          const from = Math.floor(c * n / cols);
          const to = Math.min(n, Math.max(from + 1, Math.floor((c + 1) * n / cols)));
          let lo = Infinity;
          let hi = -Infinity;
          for (let k = from; k < to; k++) { if (peaks[k][0] < lo) lo = peaks[k][0]; if (peaks[k][1] > hi) hi = peaks[k][1]; }
          const top = mid - Math.max(-1, Math.min(1, hi * scale)) * wave.h / 2;
          const bottom = Math.max(top + 1, mid - Math.max(-1, Math.min(1, lo * scale)) * wave.h / 2);
          g.fillStyle = (c + 0.5) / cols <= played ? accent : muted;
          g.fillRect(wave.x + c, top, 1, bottom - top);
        }
      }
      if (d > 0) {
        g.fillStyle = colour('--text');
        g.fillRect(wave.x + played * wave.w - 1, wave.y, 2, wave.h);
      }
    }

    /** Stop it and let go of everything: the fetch, the sound, its address, the analyser. */
    function close() {
      if (closed) return;
      closed = true;
      if (ctl) { try { ctl.abort(); } catch (e) { /* already ended */ } }
      if (frame && win && typeof win.cancelAnimationFrame === 'function') win.cancelAnimationFrame(frame);
      frame = 0;
      if (win && typeof win.removeEventListener === 'function') win.removeEventListener('resize', draw);
      if (audio) {
        try { if (typeof audio.pause === 'function') audio.pause(); } catch (e) { /* already stopped */ }
        try { audio.removeAttribute('src'); if (typeof audio.load === 'function') audio.load(); } catch (e) { /* gone with it */ }
      }
      if (url && URLs) { try { URLs.revokeObjectURL(url); } catch (e) { /* already gone */ } }
      url = '';
      if (actx && typeof actx.close === 'function') { try { Promise.resolve(actx.close()).catch(() => {}); } catch (e) { /* already closed */ } }
      actx = null;
      analyser = null;
    }

    drawIdle();
    return { el, start, close, state, draw, audio: () => audio };
  }

  const api = {
    BUCKETS, BANDS, LOWEST_HZ, HIGHEST_HZ, FLOOR_DB, FFT, DECODE_RATE, WAVE_MAX_S, MAX_SOUND_BYTES, WORDS,
    cannotPlayWords, tooLargeWords, peaksOf, loudest, bandEdges, bandLevels, seekFraction, decodeInBrowser, createSoundView,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.FilesSound = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
