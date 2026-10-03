// The OBJ converter's colour handling (2026-10-03).
//
// Run:  node --test scripts/tests/obj-to-plant-gltf.test.js   (`just verify` runs it)
//
// The greenhouse towers grew BLACK sprouts because the converter wrote each
// MTL Kd straight into the palette PNG as round(Kd * 255). The engine decodes
// that PNG as sRGB, but the Quaternius crop pack's Kd numbers are linear
// light (a Blender export), so a lettuce leaf reached the shader at 1.5%
// luminance and a beetroot at 0.3%. These tests pin the fix: a linear Kd is
// sRGB-encoded on the way in, an sRGB Kd goes in as it is, the white-key
// dodge sees the bytes the PNG really holds, and --restamp converts an old
// palette exactly once.
//
// Red check, run when this was written: with kdToBytes's encode removed (the
// old byte-for-byte write), three fail: the linear-Kd test, the white-key
// test and the --restamp test. (The fresh-conversion test checks the wiring
// against kdToBytes itself, so it passes either way; the end to end gate on
// the shipped palettes is the Rust test in src/assets/mod.rs.)

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const C = require("../obj-to-plant-gltf.js");

// The bytes a texel would decode back to on the GPU (Rgba8UnormSrgb), as
// linear light. Used to state the tests in the units the shader sees.
const srgbDecode = (b) => {
  const c = b / 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
};

test("the sRGB transfer function has its fixed points and its mid-grey", () => {
  const byte = (x) => Math.round(C.srgbEncode(x) * 255);
  assert.strictEqual(byte(0), 0);
  assert.strictEqual(byte(1), 255);
  // Half the light is byte 188, not 128: the curve that darkened the crops.
  assert.strictEqual(byte(0.5), 188);
  // Out-of-range Kd values clamp rather than wrap.
  assert.strictEqual(byte(-0.5), 0);
  assert.strictEqual(byte(2), 255);
});

test("a linear Kd is sRGB-encoded, so the GPU decodes it back to the same light", () => {
  // The crop pack's lettuce leaf and its common leaf green.
  for (const kd of [[0.118, 0.133, 0.075], [0.184, 0.357, 0.102], [0.102, 0.016, 0.027]]) {
    const bytes = C.kdToBytes(kd, "linear");
    bytes.forEach((b, i) => {
      assert.ok(Math.abs(srgbDecode(b) - kd[i]) < 0.004, `Kd ${kd} -> ${bytes}: channel ${i} decodes to ${srgbDecode(b)}`);
    });
  }
  // The leaf green the old converter wrote as (47, 91, 26).
  assert.deepStrictEqual(C.kdToBytes([0.184, 0.357, 0.102], "linear"), [119, 161, 90]);
});

test("an sRGB Kd goes into the palette as it is", () => {
  // Kenney's light wood.
  assert.deepStrictEqual(C.kdToBytes([229 / 255, 153 / 255, 100 / 255], "srgb"), [229, 153, 100]);
});

test("the white-key dodge sees the encoded bytes", () => {
  // 80% linear grey encodes to 231, inside the engine's white key, so it is
  // dodged; as raw bytes (204) it would not have been.
  assert.deepStrictEqual(C.kdToBytes([0.8, 0.8, 0.8], "linear"), [205, 205, 205]);
  assert.deepStrictEqual(C.kdToBytes([0.8, 0.8, 0.8], "srgb"), [204, 204, 204]);
});

test("there is no default colour space", () => {
  assert.throws(() => C.kdToBytes([0.5, 0.5, 0.5], undefined), /kd space/);
  assert.throws(() => C.kdToBytes([0.5, 0.5, 0.5], "gamma"), /kd space/);
});

/** A model as the converter wrote it before --kd: an 8 x 4 palette whose
 *  left block is one linear colour written byte for byte and whose right
 *  block is the unused white fill. */
function oldModel(dir) {
  const rgba = Buffer.alloc(8 * 4 * 4, 255);
  for (let y = 0; y < 4; y++) {
    for (let x = 0; x < 4; x++) {
      const o = (y * 8 + x) * 4;
      rgba[o] = 30; rgba[o + 1] = 34; rgba[o + 2] = 19;
    }
  }
  fs.writeFileSync(path.join(dir, "leaf_palette.png"), C.writePng(8, 4, rgba));
  const gltf = {
    asset: { version: "2.0", generator: "obj-to-plant-gltf.js (HumanityOS, Quaternius CC0 source)" },
    images: [{ uri: "leaf_palette.png" }],
  };
  const p = path.join(dir, "leaf.gltf");
  fs.writeFileSync(p, JSON.stringify(gltf));
  return p;
}

test("--restamp encodes an old palette once, leaves the fill, and refuses a second time", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "obj2gltf-"));
  try {
    const p = oldModel(dir);
    C.restamp(p, "linear");
    const { rgba } = C.readPng(fs.readFileSync(path.join(dir, "leaf_palette.png")));
    // The leaf block: byte 30 was Kd 30/255, now encoded.
    assert.deepStrictEqual([...rgba.subarray(0, 4)], [...C.kdToBytes([30 / 255, 34 / 255, 19 / 255], "linear"), 255]);
    assert.ok(srgbDecode(rgba[1]) > 0.12, `the leaf decodes to ${srgbDecode(rgba[1])}, still near black`);
    // The fill block is untouched.
    assert.deepStrictEqual([...rgba.subarray(4 * 4, 4 * 4 + 4)], [255, 255, 255, 255]);
    const g = JSON.parse(fs.readFileSync(p, "utf8"));
    assert.strictEqual(g.asset.extras.kd_space, "linear");
    assert.strictEqual(g.asset.generator, C.GENERATOR);
    assert.throws(() => C.restamp(p, "linear"), /already stamped/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("a fresh conversion encodes its Kd and stamps its colour space", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "obj2gltf-"));
  try {
    fs.writeFileSync(path.join(dir, "Sprout.mtl"), "newmtl Leaf\nKd 0.118 0.133 0.075\n");
    fs.writeFileSync(path.join(dir, "Sprout.obj"), "mtllib Sprout.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Leaf\nf 1 2 3\n");
    const out = path.join(dir, "out");
    const run = require("child_process").spawnSync(
      process.execPath,
      [path.join(__dirname, "..", "obj-to-plant-gltf.js"), "--kd", "linear", "--outdir", out, path.join(dir, "Sprout.obj")],
      { encoding: "utf8" }
    );
    assert.strictEqual(run.status, 0, run.stderr);
    const g = JSON.parse(fs.readFileSync(path.join(out, "sprout", "sprout.gltf"), "utf8"));
    assert.strictEqual(g.asset.extras.kd_space, "linear");
    const { rgba } = C.readPng(fs.readFileSync(path.join(out, "sprout", "sprout_palette.png")));
    assert.deepStrictEqual([...rgba.subarray(0, 3)], C.kdToBytes([0.118, 0.133, 0.075], "linear"));
    // Without --kd the converter refuses rather than guessing.
    const bare = require("child_process").spawnSync(
      process.execPath,
      [path.join(__dirname, "..", "obj-to-plant-gltf.js"), "--outdir", out, path.join(dir, "Sprout.obj")],
      { encoding: "utf8" }
    );
    assert.strictEqual(bare.status, 1);
    assert.match(bare.stderr, /--kd is required/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
