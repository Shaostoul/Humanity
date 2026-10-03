// The OBJ converter's colours and faces (2026-10-03).
//
// Run:  node --test scripts/tests/obj-to-plant-gltf.test.js   (`just verify` runs it)
//
// The greenhouse towers grew BLACK sprouts because the converter wrote each
// MTL Kd straight into the palette PNG as round(Kd * 255). The engine decodes
// that PNG as sRGB, but the Quaternius crop pack's Kd numbers are linear
// light (a Blender export), so a lettuce leaf reached the shader at 1.5%
// luminance and a beetroot at 0.3%. These tests pin the fix: a linear Kd is
// sRGB-encoded on the way in, an sRGB Kd goes in as it is (a pale one
// included: the engine's white key no longer touches an RGBA palette, so
// nothing is darkened to dodge it), and --restamp converts an old palette
// exactly once.
//
// They also pin how a face is cut into triangles (`triangulate`, same
// day): the fan the converter always wrote, unless the face is concave, when
// one fan triangle would be wound backwards and culled.
//
// Red checks, run 2026-10-03: with kdToBytes's encode removed (the old
// byte-for-byte write), four fail: the linear-Kd test, the pale-Kd test,
// the --restamp test and the fresh-conversion test. With triangulate
// reduced to the plain fan, two fail: the concave-face test and the
// concave-OBJ conversion test. The end to end gates on the shipped models
// are the Rust tests in src/assets/mod.rs (crop_palette_tests).

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

test("a pale Kd goes in as it is, not darkened", () => {
  // 80% linear grey encodes to 231. Until 2026-10-03 the converter pulled
  // any near-white colour down to 205 to keep it out of the engine's white
  // key, and 205 was still inside the key's fade, so a pale block vanished
  // anyway. The key now leaves RGBA textures alone, so the colour is kept.
  assert.deepStrictEqual(C.kdToBytes([0.8, 0.8, 0.8], "linear"), [231, 231, 231]);
  assert.deepStrictEqual(C.kdToBytes([0.8, 0.8, 0.8], "srgb"), [204, 204, 204]);
  assert.deepStrictEqual(C.kdToBytes([1, 1, 1], "srgb"), [255, 255, 255]);
});

/** Which way triangle [i, j, k] of `pts` faces, as its normal's z: the
 *  test faces lie in the xy plane, wound counter-clockwise seen from +z. */
const faceZ = (pts, [i, j, k]) => {
  const [a, b, c] = [pts[i], pts[j], pts[k]];
  return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
};
const area = (pts, tris) => tris.reduce((s, t) => s + faceZ(pts, t) / 2, 0);

test("a convex face keeps the fan the converter always wrote", () => {
  const square = [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]];
  assert.deepStrictEqual(C.triangulate(square), [[0, 1, 2], [0, 2, 3]]);
  // A corner in the middle of an edge makes a flat fan triangle; that is
  // not backwards, so the fan stays (re-converting changes nothing).
  const withMid = [[0, 0, 0], [0.5, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]];
  assert.deepStrictEqual(C.triangulate(withMid), [[0, 1, 2], [0, 2, 3], [0, 3, 4]]);
  assert.deepStrictEqual(C.triangulate([[0, 0, 0], [1, 0, 0], [0, 1, 0]]), [[0, 1, 2]]);
});

test("a concave face is cut so every triangle faces the way the face does", () => {
  // An arrowhead pointing up, counter-clockwise from its right barb; its
  // notch (corner 3) is the reflex corner, so the fan's diagonal 0-2 runs
  // outside the face and (0, 2, 3) comes out the wrong way round, which
  // the GPU would cull.
  const dart = [[1, -1, 0], [0, 1, 0], [-1, -1, 0], [0, -0.4, 0]];
  assert.ok(faceZ(dart, [0, 2, 3]) < 0, "the fan from corner 0 really is wound backwards here");
  const tris = C.triangulate(dart);
  assert.strictEqual(tris.length, 2);
  for (const t of tris) assert.ok(faceZ(dart, t) > 0, `triangle ${t} faces backwards`);
  // And it covers exactly the face: an arrowhead of area 1.4.
  assert.ok(Math.abs(area(dart, tris) - 1.4) < 1e-9, `covers ${area(dart, tris)}`);
  // A concave six-corner face (an L, reflex at corner 2) is ear-clipped
  // the same way.
  const ell = [[2, 0, 0], [2, 1, 0], [1, 1, 0], [1, 2, 0], [0, 2, 0], [0, 0, 0]];
  const cut = C.triangulate(ell);
  assert.strictEqual(cut.length, 4);
  for (const t of cut) assert.ok(faceZ(ell, t) > 0, `L triangle ${t} faces backwards`);
  assert.ok(Math.abs(area(ell, cut) - 3) < 1e-9);
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
    // The lettuce leaf, sRGB-encoded: (30, 34, 19) as it was written before.
    assert.deepStrictEqual([...rgba.subarray(0, 4)], [96, 102, 77, 255]);
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

test("a concave OBJ face converts to triangles that all face its normal", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "obj2gltf-"));
  try {
    // The arrowhead above as one OBJ quad, normal +z, in the same order.
    fs.writeFileSync(path.join(dir, "Dart.mtl"), "newmtl Leaf\nKd 0.2 0.4 0.1\n");
    fs.writeFileSync(
      path.join(dir, "Dart.obj"),
      "mtllib Dart.mtl\nv 1 -1 0\nv 0 1 0\nv -1 -1 0\nv 0 -0.4 0\nvn 0 0 1\nusemtl Leaf\nf 1//1 2//1 3//1 4//1\n"
    );
    const out = path.join(dir, "out");
    const run = require("child_process").spawnSync(
      process.execPath,
      [path.join(__dirname, "..", "obj-to-plant-gltf.js"), "--kd", "linear", "--outdir", out, path.join(dir, "Dart.obj")],
      { encoding: "utf8" }
    );
    assert.strictEqual(run.status, 0, run.stderr);
    const g = JSON.parse(fs.readFileSync(path.join(out, "dart", "dart.gltf"), "utf8"));
    const bin = fs.readFileSync(path.join(out, "dart", "dart.bin"));
    const view = (k) => g.bufferViews[g.accessors[k].bufferView].byteOffset;
    const pos = (i) => [0, 1, 2].map((c) => bin.readFloatLE(view(0) + (i * 3 + c) * 4));
    const n = g.accessors[3].count;
    assert.strictEqual(n, 6, "one quad, two triangles");
    for (let t = 0; t < n / 3; t++) {
      const ids = [0, 1, 2].map((c) => bin.readUInt32LE(view(3) + (t * 3 + c) * 4));
      const pts = ids.map(pos);
      assert.ok(faceZ(pts, [0, 1, 2]) > 0, `triangle ${ids} is wound against the face's +z normal`);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
