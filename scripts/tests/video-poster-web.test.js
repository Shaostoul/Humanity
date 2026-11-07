// A posted video's poster picture, the website half (2026-11-06; docs/design/media-player.md,
// "Rung 2: the poster picture"): web/shared/video-poster.js.
//
// Run: node --test scripts/tests/video-poster-web.test.js   (in `just rig-tests`)
//
// What runs: the module itself, with the browser's <video>, canvas and timers stood in. The stand-in
// video behaves as a browser's does for the file it is given: it opens only from the object URL
// made for that very file (the fixture video, tests/fixtures/media/colour-bar-av1-opus.webm, as a
// real File), says its length and its picture's size once its first frame is loaded, and lands a
// seek with `seeked`; one this browser "cannot decode" fires `error` instead. The stand-in canvas
// keeps what was drawn on it and makes a JPEG Blob whose size depends on the quality asked (so the
// step down can be seen). Nothing here touches a network. HOS_WEB_DIR points the test at another
// copy of web/ (used to see each test red).
//
// What it proves:
//  1. make() takes the frame about a second in (the first frame of a shorter video), draws it at
//     most 640 pixels on its longest side in the video's own shape, and steps the JPEG quality down
//     from 0.8 until it is under 96 KB; the object URL it made is let go.
//  2. make() never throws and never hangs: a video this browser cannot decode, one with no picture
//     (sound only), one that never answers (the wait runs out), and a picture over what the relay
//     keeps at every quality each give null.
//  3. posterUrlOf() finds the picture where the desktop app does (the shared cases,
//     scripts/tests/fixtures/video-poster-addresses.json), and isVideoName() knows the relay's
//     videos (MP4 and WebM).
//
// Red first: each test names the break it was seen failing on.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const V = require(path.join(WEB, "shared", "video-poster.js"));

const FIXTURE = fs.readFileSync(path.join(ROOT, "tests", "fixtures", "media", "colour-bar-av1-opus.webm"));
const SHARED = JSON.parse(fs.readFileSync(path.join(ROOT, "scripts", "tests", "fixtures", "video-poster-addresses.json"), "utf8"));

/** An element that keeps its listeners and fires them when told (a later turn, as a browser does). */
function eventful(extra) {
  const listeners = {};
  return Object.assign({
    attrs: {},
    addEventListener(t, f) { (listeners[t] = listeners[t] || []).push(f); },
    removeEventListener(t, f) { listeners[t] = (listeners[t] || []).filter((x) => x !== f); },
    fire(t) { setImmediate(() => { for (const f of (listeners[t] || []).slice()) f({ type: t }); }); },
    listening(t) { return (listeners[t] || []).length; },
    removeAttribute(k) { delete this.attrs[k]; },
  }, extra || {});
}

/**
 * A stand-in browser for make(): `video` describes the file as this browser would read it
 * ({duration, width, height} or {broken: true} or {silent: true}); `sizeAt(quality)` is the size of
 * the JPEG the canvas makes. Answers the env make() takes and what happened.
 */
function browser(file, video, sizeAt) {
  const seen = { urls: new Map(), revoked: [], seeks: [], drawn: null, canvas: null, qualities: [], timers: [] };
  const env = {
    URL: {
      createObjectURL(blob) { const u = "blob:test/" + (seen.urls.size + 1); seen.urls.set(u, blob); return u; },
      revokeObjectURL(u) { seen.revoked.push(u); },
    },
    setTimeout(fn, ms) { seen.timers.push({ fn, ms, live: true }); return seen.timers.length; },
    clearTimeout(id) { const t = seen.timers[id - 1]; if (t) t.live = false; },
    document: {
      createElement(tag) {
        if (tag === "video") {
          const v = eventful({ duration: NaN, videoWidth: 0, videoHeight: 0, readyState: 0 });
          let src = "";
          Object.defineProperty(v, "src", {
            get: () => src,
            set: (u) => {
              src = u;
              // It opens only the very file it was made for, from its object URL.
              if (seen.urls.get(u) !== file || video.broken) return v.fire("error");
              if (video.never) return;
              v.duration = video.duration;
              v.videoWidth = video.silent ? 0 : video.width;
              v.videoHeight = video.silent ? 0 : video.height;
              v.fire("loadeddata");
            },
          });
          let at = 0;
          Object.defineProperty(v, "currentTime", { get: () => at, set: (t) => { at = t; seen.seeks.push(t); v.fire("seeked"); } });
          v.load = () => {};
          seen.video = v;
          return v;
        }
        if (tag === "canvas") {
          const c = { width: 0, height: 0 };
          c.getContext = (kind) => (kind === "2d" ? { drawImage: (...args) => { seen.drawn = { args, at: seen.video.currentTime, w: c.width, h: c.height }; } } : null);
          c.toBlob = (cb, type, q) => {
            seen.qualities.push(q);
            const size = sizeAt(q);
            setImmediate(() => cb(size == null ? null : new Blob([new Uint8Array(size).fill(7)], { type })));
          };
          seen.canvas = c;
          return c;
        }
        throw new Error("no " + tag + " in this stand-in");
      },
    },
  };
  return { env, seen };
}

const KB = 1024;
const fixtureFile = () => new File([FIXTURE], "colour_bar.webm", { type: "video/webm" });

// Seen red 2026-11-06 against a copy of web/ (HOS_WEB_DIR) with make()'s seek taken out (the frame
// taken at 0): "about a second in" (actual: undefined); with its fit taken out: "at most 640
// pixels on its longest side" (actual: 1920); with the quality's step down taken out: "stepped
// down until under 96 KB" (actual: [0.8]); with the object URL kept: "the object URL is let go".
test("make: the frame about a second in, at most 640 pixels in the video's shape, stepped down under 96 KB", async () => {
  const file = fixtureFile();
  const sizes = { 0.8: 140 * KB, 0.65: 90 * KB, 0.5: 60 * KB, 0.35: 40 * KB };
  const { env, seen } = browser(file, { duration: 4.2, width: 1920, height: 1080 }, (q) => sizes[q]);
  const jpeg = await V.make(file, env);
  assert.ok(jpeg instanceof Blob && jpeg.type === "image/jpeg", "a JPEG");
  assert.equal(seen.seeks[0], 1, "about a second in");
  assert.ok(seen.drawn && seen.drawn.at === 1, "the frame it landed on was drawn");
  assert.equal(Math.max(seen.canvas.width, seen.canvas.height), 640, "at most 640 pixels on its longest side");
  assert.deepEqual([seen.canvas.width, seen.canvas.height], [640, 360], "in the video's own shape");
  assert.deepEqual(seen.drawn.args.slice(1), [0, 0, 640, 360], "drawn to fill it");
  assert.deepEqual(seen.qualities, [0.8, 0.65], "stepped down until under 96 KB");
  assert.equal(jpeg.size, 90 * KB);
  assert.deepEqual(seen.revoked, [...seen.urls.keys()], "the object URL is let go");

  // A video shorter than a second: its first frame, no seek. An upright one stays upright.
  const short = browser(file, { duration: 0.6, width: 1080, height: 1920 }, () => 30 * KB);
  assert.ok(await V.make(file, short.env), "a short video still gets one");
  assert.deepEqual(short.seen.seeks, [], "no seek: its first frame");
  assert.deepEqual([short.seen.canvas.width, short.seen.canvas.height], [360, 640], "an upright video stays upright");
  assert.deepEqual(short.seen.qualities, [0.8], "the first quality was small enough");
});

// Seen red 2026-11-06 against a copy of web/ (HOS_WEB_DIR) with make()'s wait taken out (the timer
// never set): "it waits at most WAIT_MS" (actual: undefined: nothing would ever end the wait).
test("make: never throws and never hangs: an undecodable, a silent, a silent-forever and an oversized video give null", async () => {
  const file = fixtureFile();
  const broken = browser(file, { broken: true }, () => 10 * KB);
  assert.equal(await V.make(file, broken.env), null, "a video this browser cannot decode gives none");
  assert.equal(broken.seen.revoked.length, 1, "and lets its object URL go");
  const silent = browser(file, { duration: 5, silent: true }, () => 10 * KB);
  assert.equal(await V.make(file, silent.env), null, "a video with no picture (sound only) gives none");
  const huge = browser(file, { duration: 5, width: 640, height: 360 }, () => 200 * KB);
  assert.equal(await V.make(file, huge.env), null, "one over 128 KB at every quality gives none");
  assert.deepEqual(huge.seen.qualities, V.QUALITIES, "after every quality was tried");
  const blank = browser(file, { duration: 5, width: 640, height: 360 }, () => null);
  assert.equal(await V.make(file, blank.env), null, "a canvas that makes no picture gives none");

  const never = browser(file, { never: true }, () => 10 * KB);
  const going = V.make(file, never.env);
  await new Promise((r) => setImmediate(r));
  const wait = never.seen.timers.find((t) => t.live);
  assert.ok(wait && wait.ms === V.WAIT_MS, "it waits at most WAIT_MS");
  let stop = null;
  const ran = await Promise.race([
    (async () => { wait.fn(); return await going; })(),
    new Promise((r) => { stop = setTimeout(() => r("still waiting"), 2000); }),
  ]);
  clearTimeout(stop);
  assert.equal(ran, null, "a video that never answers gives none once the wait runs out");
  assert.equal(await V.make(null, never.env), null, "no file, none");
});

// Seen red 2026-11-06 against a copy of web/ (HOS_WEB_DIR) with posterUrlOf accepting any folder:
// "/uploads/sub/1760000000004_talk.mp4" (actual: "/uploads/sub/1760000000004_talk.mp4.poster.jpg",
// expected: null).
test("posterUrlOf: where the desktop app finds it (the shared cases); isVideoName: MP4 and WebM", () => {
  assert.ok(SHARED.cases.length >= 8);
  for (const c of SHARED.cases) assert.equal(V.posterUrlOf(c.video), c.poster, c.video);
  assert.equal(V.SUFFIX, ".poster.jpg", "the relay's own ending (src/relay/storage/files.rs POSTER_SUFFIX)");
  for (const n of ["clip.mp4", "CLIP.MP4", "a.b.webm"]) assert.ok(V.isVideoName(n), n);
  for (const n of ["mp4", "clip.mov", "clip.mkv", "photo.jpg", "", null]) assert.ok(!V.isVideoName(n), String(n));
  assert.deepEqual(V.fit(1280, 720, 640), { w: 640, h: 360 });
  assert.deepEqual(V.fit(320, 180, 640), { w: 320, h: 180 }, "a small picture keeps its size");
  assert.equal(V.seekTarget(0.9), 0);
  assert.equal(V.seekTarget(NaN), 0, "a length the browser does not know: the first frame");
});
