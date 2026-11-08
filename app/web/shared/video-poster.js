// ── video-poster.js ───────────────────────────────────────────────────────
// A posted video's poster picture, the website half (2026-11-06; docs/design/media-player.md,
// "Rung 2: the poster picture"). The first rung left a video in chat black before Play, because a
// still per row would mean reading every video on screen. Now the app that posts a video makes one
// small still of it, once, and sends it with the video; the relay keeps it beside the file
// (src/relay/poster.rs), and a card shows it before Play at the cost of one small picture.
//
//   - make(file): the browser's own <video> opens the file (an object URL, nothing leaves the
//     device), is moved about a second in (SEEK_S; the first frame of a shorter video), and that
//     frame is drawn on a canvas at most MAX_SIDE pixels on its longest side and made a JPEG at
//     quality 0.8, stepped down (QUALITIES) until it is under AIM_BYTES. Resolves the JPEG Blob,
//     or null when this browser cannot decode the video, it takes longer than WAIT_MS, or the
//     picture would be over MAX_BYTES (what the relay keeps): the video then goes without one and
//     its card keeps the black box. Never throws. chat-messages.js `uploadImage` sends it as the
//     form's `poster` part, BEFORE the file, for a video going to a public room only (never a
//     private conversation's file: a plain picture of a sealed file would show what it holds).
//   - posterUrlOf(address): where a video's poster picture is: the video's own address with
//     `.poster.jpg` after it, for a video at /uploads/<name>; null for any other address. The
//     desktop app works it out the same way (src/gui/pages/chat/video_poster.rs `poster_url`); both
//     are held to scripts/tests/fixtures/video-poster-addresses.json.
//
// A classic script in the browser (HosVideoPoster lands on window; web/chat/index.html loads it
// before app.js), a CommonJS module under Node (scripts/tests/video-poster-web.test.js).
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The relay's own limits (src/relay/poster.rs POSTER_MAX_SIDE, POSTER_MAX_BYTES) and what the
  // app aims under (src/gui/pages/chat/video_poster.rs POSTER_AIM_BYTES, QUALITIES).
  const MAX_SIDE = 640;
  const MAX_BYTES = 128 * 1024;
  const AIM_BYTES = 96 * 1024;
  const QUALITIES = [0.8, 0.65, 0.5, 0.35];
  // How far into a video its picture is taken: a first frame is often black or a fade.
  const SEEK_S = 1;
  // How long the browser may take to open the video and give that frame.
  const WAIT_MS = 8000;
  const SUFFIX = '.poster.jpg';
  const VIDEO_ENDINGS = ['mp4', 'webm'];

  /** Is a file named `name` a video the relay takes a poster picture with (an MP4 or a WebM)? */
  function isVideoName(name) {
    const ext = String(name == null ? '' : name).split('.').pop().toLowerCase();
    return String(name || '').includes('.') && VIDEO_ENDINGS.includes(ext);
  }

  /** The size a `w` x `h` picture is drawn at with its longest side at most `max`, its shape kept. */
  function fit(w, h, max) {
    const W = Math.max(1, Math.round(Number(w) || 0));
    const H = Math.max(1, Math.round(Number(h) || 0));
    const m = max || MAX_SIDE;
    if (Math.max(W, H) <= m) return { w: W, h: H };
    const scale = m / Math.max(W, H);
    return { w: Math.min(m, Math.max(1, Math.round(W * scale))), h: Math.min(m, Math.max(1, Math.round(H * scale))) };
  }

  /** Where in a video of `duration` seconds its picture is taken: SEEK_S in, or 0 for a shorter one. */
  function seekTarget(duration) {
    const d = Number(duration);
    return Number.isFinite(d) && d > SEEK_S ? SEEK_S : 0;
  }

  /** Where the poster picture of the video at `address` is, or null (see the header). */
  function posterUrlOf(address) {
    const a = String(address == null ? '' : address);
    const cut = a.lastIndexOf('/');
    if (cut < 0) return null;
    const folder = a.slice(0, cut);
    const name = a.slice(cut + 1);
    const bare = name !== '' && !name.startsWith('.') && !/[\\?#]/.test(name) && !name.includes('..');
    return folder.endsWith('/uploads') && bare ? a + SUFFIX : null;
  }

  /** Resolves when `el` fires `type` (true), or false after `ms` or on `error`. */
  function once(el, type, ms, timers) {
    return new Promise((resolve) => {
      let done = false;
      const finish = (v) => {
        if (done) return;
        done = true;
        el.removeEventListener(type, onType);
        el.removeEventListener('error', onError);
        timers.clearTimeout(timer);
        resolve(v);
      };
      const onType = () => finish(true);
      const onError = () => finish(false);
      el.addEventListener(type, onType);
      el.addEventListener('error', onError);
      const timer = timers.setTimeout(() => finish(false), ms);
    });
  }

  /** `canvas` as a JPEG Blob at `quality`, or null. */
  function toJpeg(canvas, quality) {
    return new Promise((resolve) => {
      try {
        canvas.toBlob((blob) => resolve(blob || null), 'image/jpeg', quality);
      } catch (e) {
        resolve(null);
      }
    });
  }

  /**
   * The poster picture of the video `file` (a File or Blob), as a JPEG Blob, or null (see the
   * header). `env` stands in for the browser in tests: {document, URL, setTimeout, clearTimeout}.
   */
  async function make(file, env) {
    const e = env || root;
    const doc = e.document;
    const urls = e.URL;
    const timers = { setTimeout: (f, ms) => e.setTimeout(f, ms), clearTimeout: (t) => e.clearTimeout(t) };
    if (!file || !doc || !urls || typeof urls.createObjectURL !== 'function') return null;
    let address = null;
    let video = null;
    try {
      video = doc.createElement('video');
      video.muted = true;
      video.playsInline = true;
      video.preload = 'auto';
      address = urls.createObjectURL(file);
      const opened = once(video, 'loadeddata', WAIT_MS, timers);
      video.src = address;
      if (typeof video.load === 'function') video.load();
      if (!(await opened)) return null;
      const at = seekTarget(video.duration);
      if (at > 0) {
        const landed = once(video, 'seeked', WAIT_MS, timers);
        video.currentTime = at;
        if (!(await landed)) return null;
      }
      if (!video.videoWidth || !video.videoHeight) return null; // sound only, or a picture it cannot decode
      const size = fit(video.videoWidth, video.videoHeight, MAX_SIDE);
      const canvas = doc.createElement('canvas');
      canvas.width = size.w;
      canvas.height = size.h;
      const g = canvas.getContext('2d');
      if (!g) return null;
      g.drawImage(video, 0, 0, size.w, size.h);
      let last = null;
      for (const q of QUALITIES) {
        last = await toJpeg(canvas, q);
        if (!last || last.type !== 'image/jpeg') return null;
        if (last.size <= AIM_BYTES) return last;
      }
      return last && last.size <= MAX_BYTES ? last : null;
    } catch (err) {
      return null;
    } finally {
      if (address) {
        try { urls.revokeObjectURL(address); } catch (err) { /* already gone */ }
      }
      if (video) {
        try {
          video.removeAttribute('src');
          if (typeof video.load === 'function') video.load();
        } catch (err) { /* let it go */ }
      }
    }
  }

  const api = { MAX_SIDE, MAX_BYTES, AIM_BYTES, QUALITIES, SEEK_S, WAIT_MS, SUFFIX, isVideoName, fit, seekTarget, posterUrlOf, make };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosVideoPoster = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
