// terminator-grain.js must not let a blur win the grain number.
//
// Run:  node --test scripts/tests/terminator-grain.test.js
//
// On 2026-09-22 the grain metric was the mean deviation of each pixel from its
// four neighbours, and forcing the cloud resolve's spatial filter to full (a
// 3x3 box blur, `cur_s = mu`) scored best of every candidate at the dusk line,
// 6.07 percent against 8.68 for the animated jitter that actually removes the
// noise. The metric was rewarding blur. PRIORITIES 2a-ii.
//
// The fix measures GRAIN as noise over real detail at a ~15 px scale, and its
// own --controls mode blurs every capture and fails if a blur scores better.
// This file pins that contract on synthetic captures so it runs in `just
// rig-tests` (and so `just verify`) with no GPU, and it checks all three
// directions, because a gate that cannot fail is worse than none:
//   1. a box blur of the baseline is judged "same", while the OLD speckle
//      column still falls for it (so the blur really is the one that fooled
//      the old metric);
//   2. averaging independent noise (what the temporal accumulator does with an
//      animated jitter) is judged BETTER;
//   3. the controls go RED, exit 1, when the scale is moved down to the pixel
//      scale, where a blur genuinely can win.
// The real-capture evidence (the 2026-09-22 terminator captures) is in
// PRIORITIES 2a-ii; this is the always-on half.
//
// The script is run as a real child process against PNGs in a temp dir, no
// mocking, because its exit code and its JSON are what a caller reads.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const sharp = require("sharp");

const SCRIPT = path.resolve(__dirname, "..", "terminator-grain.js");
const W = 640, H = 480;

// Deterministic randomness, so the test is the same test every run.
function rng(seed) {
  let a = seed >>> 0;
  const u = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return () => Math.sqrt(-2 * Math.log(u() + 1e-12)) * Math.cos(2 * Math.PI * u()); // standard normal
}

function blur(src, s) {
  const r = Math.ceil(3 * s), k = [];
  let sum = 0;
  for (let i = -r; i <= r; i++) { k.push(Math.exp(-(i * i) / (2 * s * s))); sum += k[k.length - 1]; }
  const tmp = new Float64Array(W * H), out = new Float64Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    let a = 0; for (let i = -r; i <= r; i++) a += src[y * W + Math.min(W - 1, Math.max(0, x + i))] * k[i + r];
    tmp[y * W + x] = a / sum;
  }
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    let a = 0; for (let i = -r; i <= r; i++) a += tmp[Math.min(H - 1, Math.max(0, y + i)) * W + x] * k[i + r];
    out[y * W + x] = a / sum;
  }
  return out;
}

// A dusk-line-like scene, built the way the real terminator captures measured:
// broad cloud masses at luma ~60 over a dark sea, FLAT inside, with their
// ~15 px detail on the outline (a lit rim inside the edge and a darker halo
// outside it, which is what a grazing sun draws). Outlines come from
// thresholding a coarse random field with a gentle finer wiggle.
// Two scene choices that did NOT work, kept so nobody rebuilds them: plain
// disks (a Gaussian passes a straight ramp through unchanged, so a band-pass
// sees almost nothing on a smooth round edge and there is no detail to judge
// against), and a strong fine wiggle (it leaves small islands the sigma-4 mask
// washes out, so they land in the INTERIOR and read as noise even with no
// noise in the frame; a noise-free scene must read GRAIN near 0).
function unit(f) {
  let m = 0, v = 0;
  for (const x of f) m += x;
  m /= f.length;
  for (const x of f) v += (x - m) ** 2;
  const sd = Math.sqrt(v / f.length);
  return f.map((x) => (x - m) / sd);
}
function white(seed) {
  const n = rng(seed), o = new Float64Array(W * H);
  for (let i = 0; i < o.length; i++) o[i] = n();
  return o;
}
function scene() {
  const coarse = unit(blur(white(5), 30)), fine = unit(blur(white(6), 10));
  const field = coarse.map((v, i) => v + 0.15 * fine[i]);
  const sorted = Float64Array.from(field).sort();
  const cut = sorted[Math.floor(0.55 * sorted.length)]; // ~45% cloud cover
  const shape = field.map((v) => (v > cut ? 1 : 0));
  const soft = blur(shape, 1.5), wide = blur(shape, 5);
  return soft.map((s, i) => 4 + 56 * s + 30 * (s - wide[i]));
}

// One frame of the grain: per-pixel white noise, 40% of the local level, which
// is the dusk-line regime. `frames` > 1 averages that many INDEPENDENT draws,
// which is what the temporal accumulator does once the jitter moves.
function noisy(clean, seed, frames = 1) {
  const n = rng(seed), out = new Float64Array(W * H);
  for (let f = 0; f < frames; f++) for (let i = 0; i < out.length; i++) out[i] += clean[i] * (1 + 0.4 * n()) / frames;
  return out;
}

function box3(src) {
  const out = new Float64Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    let a = 0;
    for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
      a += src[Math.min(H - 1, Math.max(0, y + dy)) * W + Math.min(W - 1, Math.max(0, x + dx))];
    }
    out[y * W + x] = a / 9;
  }
  return out;
}

async function png(L, file) {
  const buf = Buffer.alloc(W * H * 3);
  for (let i = 0; i < W * H; i++) {
    const v = Math.max(0, Math.min(255, Math.round(L[i])));
    buf[i * 3] = buf[i * 3 + 1] = buf[i * 3 + 2] = v;
  }
  await sharp(buf, { raw: { width: W, height: H, channels: 3 } }).png().toFile(file);
}

let dir, files;
test.before(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), "grain-test-"));
  const clean = scene();
  const base = noisy(clean, 11);
  files = {
    base: path.join(dir, "base.png"),
    temporal: path.join(dir, "temporal.png"),
    blurred: path.join(dir, "blurred.png"),
    clean: path.join(dir, "clean.png"),
  };
  await png(base, files.base);
  await png(noisy(clean, 23, 9), files.temporal);
  await png(box3(base), files.blurred);
  await png(clean, files.clean);
});
test.after(() => fs.rmSync(dir, { recursive: true, force: true }));

// Identical invocations are run once: several tests read the same run.
const runs = new Map();
function run(extra, inputs = [files.base, files.temporal, files.blurred]) {
  const key = JSON.stringify([extra, inputs]);
  if (!runs.has(key)) runs.set(key, runOnce(extra, inputs));
  return runs.get(key);
}
function runOnce(extra, inputs) {
  const json = path.join(dir, `out-${Math.random().toString(36).slice(2)}.json`);
  const r = spawnSync(process.execPath, [SCRIPT, ...inputs, "--bands", "1", "--band", "0", "--json", json, ...extra], { encoding: "utf8" });
  const out = fs.existsSync(json) ? JSON.parse(fs.readFileSync(json, "utf8")) : null;
  return { status: r.status, stdout: r.stdout, stderr: r.stderr, out };
}

test("a box blur is 'same' and averaging independent noise is BETTER", () => {
  const r = run([]);
  assert.strictEqual(r.status, 0, r.stdout + r.stderr);
  const [base, temporal, blurred] = r.out.rows;
  // The flaw being guarded against: the OLD speckle rewards the blur hugely.
  assert.ok(blurred.speckle_old < 0.5 * base.speckle_old, `the old speckle should fall for a blur: ${blurred.speckle_old} vs ${base.speckle_old}`);
  // The fix: GRAIN does not.
  assert.strictEqual(blurred.verdict, "same", `a box blur must not score as a GRAIN change: ${JSON.stringify(blurred)}`);
  // And real noise removal still registers, with the real detail kept.
  assert.strictEqual(temporal.verdict, "BETTER", `averaging nine independent frames must score better: ${JSON.stringify(temporal)}`);
  assert.ok(temporal.detail_kept_9 > 0.9 && temporal.detail_kept_15 > 0.9, "averaging frames keeps the real detail");
});

test("the grainy baseline reads as grainy: a noise-free frame is near zero and the baseline is worse", () => {
  // The same scene with no noise at all is the floor. If it read high, the
  // measure would be scoring cloud structure as grain, and every arm would sit
  // on that structure instead of on its noise.
  const r = run([], [files.clean, files.base]);
  assert.strictEqual(r.status, 0, r.stdout + r.stderr);
  const [clean, base] = r.out.rows;
  assert.ok(clean.grain < 2, `a noise-free scene must read GRAIN near 0, got ${clean.grain}`);
  assert.strictEqual(base.verdict, "worse", `the noisy baseline must be judged worse than the clean frame: ${JSON.stringify(base)}`);
  assert.ok(base.grain > 5 * clean.grain + 5, `the baseline must be clearly grainy: ${base.grain}`);
});

test("--controls passes on the baseline: no blur of it is judged BETTER", () => {
  const r = run(["--controls"], [files.base]);
  assert.strictEqual(r.status, 0, `controls must pass at the default scale:\n${r.stdout}\n${r.stderr}`);
  assert.ok(r.out.controls.length === 6 && r.out.controls.every((c) => c.result === "PASS"), JSON.stringify(r.out.controls, null, 1));
});

// The measure has two locks, and each one is proven load-bearing by switching
// the OTHER one off and watching the controls go red. --no-sharpness-lock is a
// diagnostic flag that exists for exactly this.
// MILD blurs are too small to reach the 15 px band. Of them, box 3x3 (the
// forced filter's own kernel) and gauss s1 clearly win at the pixel scale;
// gauss s0.5 is too gentle to win there and is only asked to pass at 15 px.
const MILD = ["box 3x3", "gauss s0.5", "gauss s1"];
const PIXEL_WINNERS = ["box 3x3", "gauss s1"];
const pick = (r, names) => r.out.controls.filter((c) => names.includes(c.blur));

test("the SCALE lock is load-bearing: mild blurs win at the pixel scale and not at 15 px", () => {
  const at15 = run(["--controls", "--no-sharpness-lock"], [files.base]);
  const atPx = run(["--controls", "--no-sharpness-lock", "--scale", "0.71,1.41"], [files.base]);
  // At the headline scale a blur too small to reach it cannot move GRAIN, so
  // with the sharpness lock off the mild blurs still pass on the scale alone.
  assert.ok(pick(at15, MILD).every((c) => c.result === "PASS"), JSON.stringify(pick(at15, MILD), null, 1));
  // Move the scale to the pixel scale and the same blurs win: red, exit 1.
  assert.strictEqual(atPx.status, 1, `at the pixel scale the blur controls must FAIL:\n${atPx.stdout}`);
  assert.match(atPx.stdout, /CONTROLS FAILED/);
  assert.ok(pick(atPx, PIXEL_WINNERS).every((c) => c.verdict === "BETTER" && /judged BETTER/.test(c.result)), JSON.stringify(pick(atPx, PIXEL_WINNERS), null, 1));
});

test("the SHARPNESS lock is load-bearing: a wide blur reaches into the 15 px band and only the lock stops it", () => {
  // A sigma-3 Gaussian cuts through the headline band itself (its response
  // falls from 0.75 to 0.17 across it), so a finite band can tilt and GRAIN
  // drops. What it cannot do is keep the 9 px detail.
  const on = run(["--controls"], [files.base]);
  const off = run(["--controls", "--no-sharpness-lock"], [files.base]);
  const [wideOn] = pick(on, ["gauss s3"]), [wideOff] = pick(off, ["gauss s3"]);
  assert.strictEqual(wideOn.result, "PASS", JSON.stringify(wideOn));
  assert.strictEqual(wideOn.verdict, "blurred", "with the lock on, the wide blur is named for what it is");
  assert.ok(wideOn.detail_kept_9 < 0.9, "the wide blur must visibly cost 9 px detail");
  assert.strictEqual(off.status, 1, `with the sharpness lock off the wide blur must win:\n${off.stdout}`);
  assert.strictEqual(wideOff.verdict, "BETTER", JSON.stringify(wideOff));
});

