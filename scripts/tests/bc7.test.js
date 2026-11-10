// Our BC7 encoder (scripts/lib/bc7.js, the look's garment textures, 2026-11-10; docs/reference/
// research/2026-11-10-bc7-encoders/): every block it writes decodes to exactly the texels it
// measured its error on, in every mode it writes, opaque and card blocks alike; a DDS file reads back
// as written. The decoder here is tied to the GPU's own by the Rust twin's test
// (renderer::bc7::the_gpu_decodes_bc7_as_we_do), and the shipped files to their sources by
// renderer::bc7::the_shipped_garment_textures_decode_to_their_sources.
//
// Seen red 2026-11-10 with mode 3's two p-bits written in the other order (subset 1's before subset
// 0's): "the encoder's error and its decoded block's differ in 436 of 3000 blocks (modes 3)".

"use strict";
const test = require("node:test");
const assert = require("node:assert");
const path = require("path");
const bc7 = require(path.join(__dirname, "..", "lib", "bc7.js"));

function blocks(count) {
  let seed = 12345;
  const rnd = () => { seed = (seed * 1103515245 + 12345) >>> 0; return (seed >>> 8) / (1 << 24); };
  const out = [];
  for (let k = 0; k < count; k++) {
    const px = new Uint8Array(64);
    const kind = k % 4;
    const cards = k % 3 === 0;
    const base = [rnd() * 255, rnd() * 255, rnd() * 255];
    const dir = [rnd() * 80 - 40, rnd() * 80 - 40, rnd() * 80 - 40];
    const split = Math.floor(rnd() * 64);
    for (let i = 0; i < 16; i++) {
      const t = (i % 4) / 3 + Math.floor(i / 4) / 3;
      let c;
      if (kind === 0) c = [rnd() * 255, rnd() * 255, rnd() * 255];
      else if (kind === 1) c = base.map((b, j) => b + dir[j] * t);
      else if (kind === 2) c = (bc7.PARTITIONS2[split] >> i) & 1 ? base.map((b, j) => b + dir[j]) : base.map((b) => 255 - b);
      else c = base.map((b, j) => b + dir[j] * t * 0.1 + rnd() * 6);
      for (let j = 0; j < 3; j++) px[i * 4 + j] = Math.max(0, Math.min(255, Math.round(c[j])));
      px[i * 4 + 3] = cards ? (rnd() < 0.3 ? 0 : Math.round(rnd() * 255)) : 255;
    }
    out.push({ px, cards });
  }
  return out;
}

test("every block decodes to the texels the encoder measured, in every mode it writes", () => {
  const modes = new Map();
  const wrong = new Map();
  const all = blocks(3000);
  for (const { px, cards } of all) {
    const out = new Uint8Array(16);
    const err = bc7.encodeBlock(px, out, 0, { cards });
    const m = bc7.blockMode(out, 0);
    modes.set(m, (modes.get(m) || 0) + 1);
    const dec = new Uint8Array(64);
    bc7.decodeBlock(out, 0, dec, 0);
    let e = 0;
    for (let i = 0; i < 16; i++) {
      const a = px[i * 4 + 3];
      const cw = cards ? Math.max(a / 255, 1 / 64) : 1;
      for (let j = 0; j < 3; j++) { const d = dec[i * 4 + j] - px[i * 4 + j]; e += d * d * cw; }
      const d = dec[i * 4 + 3] - a;
      e += d * d;
    }
    if (Math.abs(e - err) > 1e-6 * Math.max(1, e)) wrong.set(m, (wrong.get(m) || 0) + 1);
    if (!cards) for (let i = 3; i < 64; i += 4) assert.strictEqual(dec[i], 255, "an opaque block decodes opaque");
  }
  const n = [...wrong.values()].reduce((s, x) => s + x, 0);
  assert.strictEqual(n, 0, `the encoder's error and its decoded block's differ in ${n} of ${all.length} blocks (modes ${[...wrong.keys()].join(", ")})`);
  for (const m of [1, 3, 5, 6, 7]) assert.ok(modes.get(m) > 0, `mode ${m} is written (modes seen: ${[...modes.keys()].join(", ")})`);
  for (const m of [0, 2, 4]) assert.ok(!modes.has(m), `mode ${m} is never written`);
});

// A gradient across both axes of a block is not a line in colour, which one subset can only
// approximate, so 40 dB (a step of 8 bits or so) is the bar here; the shipped cloth reads 41 to 57.
test("a smooth opaque gradient encodes close to its source", () => {
  const w = 64, h = 64;
  const rgba = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const o = (y * w + x) * 4;
    rgba[o] = x * 4; rgba[o + 1] = y * 4; rgba[o + 2] = 128; rgba[o + 3] = 255;
  }
  const back = bc7.decodeImage(bc7.encodeImage(rgba, w, h), w, h);
  const db = bc7.psnr(rgba, back, [0, 1, 2]);
  assert.ok(db > 40, `a gradient decodes ${db.toFixed(1)} dB from its source`);
});

test("a DDS file reads back as written, and a cut one is refused", () => {
  const levels = [Buffer.alloc(8 / 4 * 4 / 4 * 16, 7), Buffer.alloc(16, 9)];
  const file = bc7.writeDds(8, 4, levels);
  const back = bc7.readDds(file);
  assert.deepStrictEqual([back.w, back.h, back.levels.length], [8, 4, 2]);
  assert.ok(back.levels[0].equals(levels[0]) && back.levels[1].equals(levels[1]));
  assert.throws(() => bc7.readDds(file.subarray(0, file.length - 1)), /cut short/);
});
