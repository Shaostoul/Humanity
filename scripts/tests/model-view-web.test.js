// The 3D model viewer on the website (web/shared/model-view.js, 2026-10-16;
// docs/design/model-viewer.md). The desktop app's twin is src/gui/model_view/; both read the same
// test files, made by scripts/gen-model-fixtures.js: a textured cube on two nodes (its corners land
// at x 1..5, y and z -2..2 once the nodes apply) and a two-triangle wedge as binary and ASCII STL.
//
// Run: node --test scripts/tests/model-view-web.test.js
//
// What it proves:
//  1. The cube reads as 12 triangles with every node applied, one material (a base colour
//     texture, roughness 0.5) and one PNG picture.
//  2. Both STL forms read as the same two triangles, turned from z-up to y-up, with normals worked
//     out from the corners (unit length, never the file's zeros), in the plain grey material.
//  3. Broken files are errors in the viewer's words, never a read outside the file: a wrong magic
//     number, a cut-short file, a buffer view or accessor past its data, an index past the
//     vertices, Draco, a model over 5,000,000 triangles or 15,000,000 vertices, and nodes that
//     repeat each other past 1,000,000 visits.
//  4. The words are the spec's, exactly.
//  5. The camera: the fit's distance is r / sin(17.5 degrees) * 1.15 from the centre, turning holds
//     the pitch to 89 degrees, zoom holds the distance to 0.05 r .. 50 r, moving follows the
//     pointer, double-click's reset goes back to the fit.
//  6. The view: the fetching words with the file's size, then the model's own line under it and a
//     picture drawn; over 200 MB it says so and fetches nothing; without WebGL 2 it says so and
//     fetches nothing; a broken file and a refused fetch are said; Close gives everything back.
//  7. Each texture keeps the file's own filtering and wrapping (its glTF sampler), and a texture
//     without one, or a sampler field left out, gets glTF's usual defaults.
//
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
// Seen red 2026-10-16, each with one break made in a copy of web/shared/model-view.js under
// HOS_WEB_DIR, then green again on the real web/: see the note above each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const MV = require(path.join(WEB, "shared", "model-view.js"));
const { fakeDocument } = require("./media-web-harness.js");

const FIX = path.join(ROOT, "scripts", "tests", "fixtures");
/** A fixture's bytes as an ArrayBuffer of its own (what a fetch answers). */
function bytesOf(name) {
  const b = fs.readFileSync(path.join(FIX, name));
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
}
const near = (a, b, eps) => Math.abs(a - b) <= (eps || 1e-6);
function assertNear(actual, want, words, eps) {
  assert.equal(actual.length, want.length, words + ": " + Array.from(actual).join(", "));
  for (let i = 0; i < want.length; i++) assert.ok(near(actual[i], want[i], eps), words + " [" + i + "]: " + Array.from(actual).join(", ") + " / want " + want.join(", "));
}

/** The cube's JSON and binary parts, to break one thing and pack it again. */
function unpack(buf) {
  const b = Buffer.from(buf);
  const jsonLen = b.readUInt32LE(12);
  const json = JSON.parse(b.subarray(20, 20 + jsonLen).toString("utf8"));
  const binAt = 20 + jsonLen;
  const bin = Buffer.from(b.subarray(binAt + 8, binAt + 8 + b.readUInt32LE(binAt)));
  return { json, bin };
}
/** A .glb from a JSON description and a binary part (as gen-model-fixtures.js writes one). */
function pack(json, bin) {
  const pad = (buf, fill) => { const extra = (4 - (buf.length % 4)) % 4; return extra ? Buffer.concat([buf, Buffer.alloc(extra, fill)]) : buf; };
  const chunk = (type, data) => { const h = Buffer.alloc(8); h.writeUInt32LE(data.length, 0); h.write(type, 4, "ascii"); return Buffer.concat([h, data]); };
  const parts = [chunk("JSON", pad(Buffer.from(JSON.stringify(json), "utf8"), 0x20))];
  if (bin) parts.push(chunk("BIN\0", pad(bin, 0)));
  const body = Buffer.concat(parts);
  const header = Buffer.alloc(12);
  header.write("glTF", 0, "ascii");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(12 + body.length, 8);
  const all = Buffer.concat([header, body]);
  return all.buffer.slice(all.byteOffset, all.byteOffset + all.byteLength);
}
/** The cube with `change(json, bin)` made to it. */
function brokenCube(change) {
  const { json, bin } = unpack(bytesOf("model-cube.glb"));
  change(json, bin);
  return pack(json, bin);
}
/** `f()` throws the viewer's own error; its kind and words are answered. */
function refusal(f) {
  let err = null;
  try { f(); } catch (e) { err = e; }
  assert.ok(err, "it throws");
  assert.equal(err.name, "ModelError", "the viewer's own error, not " + (err && err.name) + ": " + (err && err.message));
  return err;
}

// ── 1. The cube ──────────────────────────────────────────────────────────

// Seen red 2026-10-16 with walk() ignoring each node's own matrix (`multiply(parent, identity())`):
// "the corners land at x 1..5 once both nodes apply / [0]: -1, -1, -1 / want 1, -2, -2".
test("1. the cube reads as 12 triangles on its nodes, with its material and picture", () => {
  const m = MV.parseGlb(bytesOf("model-cube.glb"));
  assert.equal(m.triangles, 12);
  assert.equal(m.indices.length, 36);
  assert.equal(m.positions.length, 24 * 3, "24 vertices");
  assertNear(m.bounds.min, [1, -2, -2], "the corners land at x 1..5 once both nodes apply");
  assertNear(m.bounds.max, [5, 2, 2], "and y, z -2..2");
  assertNear(m.bounds.centre, [3, 0, 0], "centred on (3, 0, 0)");
  assert.ok(near(m.bounds.radius, Math.sqrt(12)), "the farthest corner from the centre: " + m.bounds.radius);
  assert.equal(m.materials.length, 1, "one material");
  const mat = m.materials[0];
  assert.deepEqual([mat.baseTexture && mat.baseTexture.image, mat.roughness, mat.metallic, mat.alphaMode, mat.doubleSided], [0, 0.5, 0, "OPAQUE", false], "a base colour texture, roughness 0.5");
  assert.deepEqual(m.draws, [{ first: 0, count: 36, material: 0 }]);
  assert.equal(m.images.length, 1, "one picture");
  assert.equal(m.images[0].mimeType, "image/png");
  assert.deepEqual(Array.from(m.images[0].bytes.slice(0, 4)), [0x89, 0x50, 0x4e, 0x47], "its PNG bytes, still encoded");
  for (let i = 0; i < m.normals.length; i += 3) {
    assert.ok(near(Math.hypot(m.normals[i], m.normals[i + 1], m.normals[i + 2]), 1, 1e-5), "the scaled node's normals are unit length again");
  }
  assertNear(m.normals.slice(0, 3), [1, 0, 0], "the +x face keeps its normal");
  for (const i of m.indices) assert.ok(i < 24);
});

// ── 2. STL ───────────────────────────────────────────────────────────────

// Seen red 2026-10-16 with the z-up turn left out (`p.push([x, y, z])`): "model-wedge.stl: turned
// to y-up [7]: 0, 0, 0, 10, 0, 0, 0, 10, 0, ... / want 0, 0, 0, 10, 0, 0, 0, 0, -10, ...".
test("2. both STL forms are the same two triangles, turned to y-up, with worked-out normals", () => {
  for (const name of ["model-wedge.stl", "model-wedge-ascii.stl"]) {
    const m = MV.parseStl(bytesOf(name));
    assert.equal(m.triangles, 2, name);
    // STL is z-up: (x, y, z) becomes (x, z, -y).
    assertNear(m.positions, [0, 0, 0, 10, 0, 0, 0, 0, -10, 0, 0, 0, 0, 0, -10, 0, 5, 0], name + ": turned to y-up");
    // The file's own normals are (0, 0, 0): these come from the corners.
    assertNear(m.normals, [0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0], name + ": normals from the corners, unit and not zero");
    assert.deepEqual(Array.from(m.indices), [0, 1, 2, 3, 4, 5]);
    const mat = m.materials[0];
    assert.deepEqual([mat.baseColor, mat.metallic, mat.roughness, mat.doubleSided], [[0.72, 0.74, 0.78, 1], 0, 0.55, true], name + ": the plain material, both sides");
    assertNear(m.bounds.min, [0, 0, -10], name);
    assertNear(m.bounds.max, [10, 5, 0], name);
  }
  // The name's ending picks the reader.
  assert.equal(MV.parseModel("Wedge.STL", bytesOf("model-wedge.stl")).triangles, 2);
  assert.equal(MV.parseModel("cube.glb", bytesOf("model-cube.glb")).triangles, 12);
});

// ── 3. Broken files ──────────────────────────────────────────────────────

// Seen red 2026-10-16 twice: with accessor()'s last check taken out (`if (a.count > 0 && off + ...
// > view.bytes.length)`), "an accessor past its buffer view" got a RangeError from the DataView
// instead of the viewer's words ("the viewer's own error, not RangeError: Offset is outside the
// bounds of the DataView"); with the index check taken out (`if (i >= count)`), "an index past the
// vertices" was read without a word ("it throws").
test("3. broken files are the viewer's words, never a read outside the file", () => {
  const cube = bytesOf("model-cube.glb");
  const magic = new Uint8Array(cube.slice(0));
  magic[0] = 0x47;
  assert.equal(refusal(() => MV.parseGlb(magic.buffer)).message, "This 3D file could not be read: it is not a binary glTF (.glb) file.", "a wrong magic number");
  assert.equal(refusal(() => MV.parseGlb(cube.slice(0, 1000))).kind, "unreadable", "a .glb cut short");
  assert.equal(refusal(() => MV.parseGlb(cube.slice(0, 8))).kind, "unreadable", "a .glb cut to its first bytes");
  assert.equal(refusal(() => MV.parseStl(bytesOf("model-wedge.stl").slice(0, 174))).kind, "unreadable", "a binary STL cut short");
  assert.equal(refusal(() => MV.parseStl(new TextEncoder().encode("hello").buffer)).message, "This 3D file could not be read: it is neither a binary nor a text STL.");
  assert.equal(refusal(() => MV.parseStl(new TextEncoder().encode("solid w\nfacet normal 0 0 0\nouter loop\nvertex 0 0 0\nvertex 1 0 x\n").buffer)).kind, "unreadable", "a vertex that is not three numbers");

  const pastView = refusal(() => MV.parseGlb(brokenCube((j) => { j.accessors[0].count = 1000; })));
  assert.match(pastView.message, /an accessor runs past its buffer view/, "an accessor past its buffer view");
  const pastBuffer = refusal(() => MV.parseGlb(brokenCube((j) => { j.bufferViews[0].byteLength = 1000000; })));
  assert.match(pastBuffer.message, /a buffer view runs past its buffer/, "a buffer view past its buffer");
  const strided = refusal(() => MV.parseGlb(brokenCube((j) => { j.bufferViews[0].byteStride = 4; })));
  assert.match(strided.message, /overlap/, "a stride shorter than an element");
  const index = refusal(() => MV.parseGlb(brokenCube((j, bin) => { bin.writeUInt16LE(99, j.bufferViews[3].byteOffset); })));
  assert.equal(index.message, "This 3D file could not be read: an index points past the vertices.", "an index past the vertices");
  assert.match(refusal(() => MV.parseGlb(brokenCube((j) => { j.nodes[0].children = [7]; }))).message, /a node that is not there/);
  assert.match(refusal(() => MV.parseGlb(brokenCube((j) => { j.nodes[1].children = [0]; }))).message, /loop back on themselves/, "nodes that loop");
  assert.match(refusal(() => MV.parseGlb(brokenCube((j) => { j.buffers[0].uri = "cube.bin"; }))).message, /outside files/, "an outside file is not fetched");
  assert.match(refusal(() => MV.parseGlb(brokenCube((j) => { j.accessors[0].sparse = { count: 1 }; }))).message, /sparse/);
  assert.match(refusal(() => MV.parseGlb(brokenCube((j) => { j.meshes[0].primitives[0].mode = 1; }))).message, /holds no triangles/, "lines are not drawn");

  const draco = refusal(() => MV.parseGlb(brokenCube((j) => { j.extensionsUsed = j.extensionsRequired = ["KHR_draco_mesh_compression"]; })));
  assert.equal(draco.kind, "draco");
  assert.equal(draco.message, "This model is compressed with Draco, which the viewer cannot read yet. Download it to see it in Blender or another 3D program.");

  // 5,000,001 triangles, counted before anything is read (nothing that size is allocated).
  const huge = pack({
    asset: { version: "2.0" }, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0 }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 } }] }],
    accessors: [{ componentType: 5126, count: 15000003, type: "VEC3" }],
  }, null);
  const detailed = refusal(() => MV.parseGlb(huge));
  assert.equal(detailed.kind, "too_detailed");
  assert.equal(detailed.message, "This model has 5,000,001 triangles, more than the 5,000,000 the viewer draws. Download it to see it.");

  // A data: address written into the file is read; base64 that is not base64 is refused.
  const inline = brokenCube((j, bin) => { j.buffers[0].uri = "data:application/octet-stream;base64," + bin.toString("base64"); });
  assert.equal(MV.parseGlb(inline).triangles, 12, "a data: buffer");
  assert.match(refusal(() => MV.parseGlb(brokenCube((j) => { j.buffers[0].uri = "data:application/octet-stream;base64,@@@@"; }))).message, /outside files/);
});

// ── 4. The words ─────────────────────────────────────────────────────────

// Seen red 2026-10-16 with thousands() leaving the number as it was: "...double-click to start over.
// 12345 triangles." where "12,345 triangles." was wanted (and test 3's "5000001 triangles").
test("4. the words are the spec's, exactly", () => {
  const W = MV.WORDS;
  assert.equal(W.view, "View in 3D");
  assert.equal(MV.fill(W.viewTip, { size: "2 KB" }), "Look at it here and turn it with the mouse. It is fetched from the server first (2 KB).");
  assert.equal(MV.fill(W.fetching, { name: "ship.glb", size: "12.4 MB" }), "Fetching ship.glb (12.4 MB)...");
  assert.equal(W.reading, "Reading the model...");
  assert.equal(MV.fill(W.shown, { name: "ship.glb", triangles: MV.thousands(12345) }), "ship.glb: drag to turn it, scroll to zoom, right-drag or Shift-drag to move it, double-click to start over. 12,345 triangles.");
  assert.equal(MV.thousands(12345), "12,345");
  assert.equal(MV.thousands(5000000), "5,000,000");
  assert.equal(MV.thousands(12), "12");
  assert.equal(W.close, "Close the 3D view");
  assert.equal(MV.fill(W.tooLarge, { size: "300 MB" }), "This model is 300 MB, larger than the 200 MB the viewer opens. Download it to see it.");
  assert.equal(MV.fill(W.unreadable, { reason: "it is broken" }), "This 3D file could not be read: it is broken.");
  assert.equal(W.noGraphics, "Your browser cannot draw 3D here; Download the file to see it.");
  assert.equal(MV.fill(W.fetchFailed, { reason: "The server answered 403" }), "This model could not be fetched: The server answered 403.");
  assert.equal(MV.fill(W.tooManyVertices, { vertices: MV.thousands(15000003) }), "This model has 15,000,003 vertices, more than the 15,000,000 the viewer draws. Download it to see it.");
  assert.deepEqual([MV.MAX_MODEL_BYTES, MV.MAX_TRIANGLES, MV.MAX_TEXTURE_SIDE], [200 * 1024 * 1024, 5000000, 4096]);
  assert.ok(MV.isModelFile("ship.GLB") && MV.isModelFile("part.stl") && !MV.isModelFile("ship.blend") && !MV.isModelFile("ship.gltf"));
  for (const w of Object.values(W)) assert.ok(!w.includes("\u2014"), "no em dash: " + w);
});

// ── 5. The camera ────────────────────────────────────────────────────────

// Seen red 2026-10-16 with orbitCamera's pitch not held (`pitch: cam.pitch + dy * TURN_PER_POINT`):
// "the pitch stops at 89 degrees: 45856.62...".
test("5. the camera fits the model, turns, zooms and moves as the spec gives", () => {
  const m = MV.parseGlb(bytesOf("model-cube.glb"));
  const cam = MV.fitCamera(m.bounds);
  const r = Math.sqrt(12);
  const want = r / Math.sin(17.5 * Math.PI / 180) * 1.15;
  const eye = MV.cameraEye(cam);
  assert.ok(near(Math.hypot(eye[0] - 3, eye[1], eye[2]), want, 1e-9), "r / sin(17.5 deg) * 1.15 from the centre: " + eye);
  assert.ok(near(cam.yaw, 35 * Math.PI / 180) && near(cam.pitch, 20 * Math.PI / 180), "from yaw 35, pitch 20");
  // c + d * (cos(pitch) sin(yaw), sin(pitch), cos(pitch) cos(yaw)).
  const p = 20 * Math.PI / 180, y = 35 * Math.PI / 180;
  assertNear(eye, [3 + want * Math.cos(p) * Math.sin(y), want * Math.sin(p), want * Math.cos(p) * Math.cos(y)], "the camera's place", 1e-9);
  assertNear(MV.nearFar(cam), [Math.max(want - 2 * r, want * 0.001), want + 2 * r], "near and far", 1e-9);

  const up = MV.orbitCamera(cam, 0, 100000);
  assert.ok(near(up.pitch, 89 * Math.PI / 180), "the pitch stops at 89 degrees: " + (up.pitch * 180 / Math.PI));
  assert.ok(near(MV.orbitCamera(cam, 0, -100000).pitch, -89 * Math.PI / 180), "and at -89");
  const turned = MV.orbitCamera(cam, 10, 5);
  assert.ok(near(turned.yaw, cam.yaw - 0.08) && near(turned.pitch, cam.pitch + 0.04), "0.008 radians a point, yaw -= dx, pitch += dy");

  assert.ok(near(MV.zoomCamera(cam, 1e9).distance, 0.05 * r), "zoom in stops at 0.05 r");
  assert.ok(near(MV.zoomCamera(cam, -1e9).distance, 50 * r), "zoom out stops at 50 r");
  assert.ok(near(MV.zoomCamera(cam, 100).distance, want * Math.exp(-0.15)), "d *= exp(-points * 0.0015)");

  // A drag of 10 points right on a 400-point view: the target moves left along the camera's right,
  // so the model follows the pointer, by 10 * 2 d tan(17.5 deg) / 400.
  const moved = MV.panCamera(cam, 10, 0, 400);
  const step = 10 * 2 * want * Math.tan(17.5 * Math.PI / 180) / 400;
  const right = [Math.cos(y), 0, -Math.sin(y)];
  assertNear(moved.target, [3 - right[0] * step, 0, -right[2] * step], "moved along the camera's right", 1e-9);
  assert.ok(near(moved.distance, cam.distance), "moving does not zoom");

  const back = MV.resetCamera(MV.zoomCamera(MV.orbitCamera(moved, 40, 40), 300));
  assertNear(MV.cameraEye(back), eye, "double-click: back to the fit", 1e-9);
});

// ── 6. The view ──────────────────────────────────────────────────────────

/** A drawer that records what it was asked (Node has no WebGL). */
function standIn() {
  const r = { made: 0, prepared: null, draws: [], destroyed: false };
  r.factory = () => { r.made++; return r.api; };
  r.api = {
    prepare: async (model) => { r.prepared = model; },
    draw: (frame) => { r.draws.push(frame); return true; },
    free: () => {},
    destroy: () => { r.destroyed = true; },
  };
  return r;
}
/** A promise and its resolve, reject. */
function deferred() {
  const d = {};
  d.promise = new Promise((res, rej) => { d.resolve = res; d.reject = rej; });
  return d;
}
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
function host() {
  const doc = fakeDocument();
  const box = doc.createElement("div");
  doc.body.appendChild(box);
  return box;
}
const shownText = (box) => box.querySelector(".model-view-message").textContent;
const shownLine = (box) => box.querySelector(".model-view-line").textContent;

// Seen red 2026-10-16 with the size not checked before the fetch (the `size > MAX_MODEL_BYTES` line
// taken out): the 300 MB file was fetched and read, "'Reading the model...' == 'This model is 300 MB,
// larger than the 200 MB the viewer opens. Download it to see it.'". Seen red again the same day with
// the old words for a failed fetch put back: "'This file could not be loaded: The server answered
// 403.' == 'This model could not be fetched: The server answered 403.'" (test 4 too).
test("6. the view says each step, draws the model, and fetches nothing it should not", async () => {
  // Fetching, then the model's own line, and one picture drawn at the fit.
  const box = host();
  const r = standIn();
  const got = deferred();
  let asked = 0;
  const view = MV.open(box, { name: "model-cube.glb", size: 2224, renderer: r.factory, fetchBytes: () => { asked++; return got.promise; } });
  await wait(0);
  assert.equal(shownText(box), "Fetching model-cube.glb (2 KB)...");
  assert.equal(view.phase(), "fetching");
  got.resolve(bytesOf("model-cube.glb"));
  await view.done;
  assert.equal(view.phase(), "shown");
  assert.equal(shownLine(box), "model-cube.glb: drag to turn it, scroll to zoom, right-drag or Shift-drag to move it, double-click to start over. 12 triangles.");
  assert.equal(r.prepared.triangles, 12, "the drawer got the model");
  await wait(40);
  assert.equal(r.draws.length, 1, "drawn once, and not again while nothing changes");
  assert.ok(near(r.draws[0].cam.distance, Math.sqrt(12) / Math.sin(17.5 * Math.PI / 180) * 1.15), "at the fit");
  assert.equal(asked, 1);
  view.close();
  assert.equal(r.destroyed, true, "Close gives the GPU memory back");
  assert.equal(box.children.length, 0, "and takes the view away");

  // Over 200 MB: said, and nothing fetched (nor any drawing made).
  const big = host();
  const r2 = standIn();
  let fetched = 0;
  const v2 = MV.open(big, { name: "huge.glb", size: 300 * 1024 * 1024, renderer: r2.factory, fetchBytes: async () => { fetched++; return bytesOf("model-cube.glb"); } });
  await v2.done;
  assert.equal(shownText(big), "This model is 300 MB, larger than the 200 MB the viewer opens. Download it to see it.");
  assert.equal(fetched, 0, "nothing is fetched");
  assert.equal(r2.made, 0, "nor any drawing made");

  // No WebGL 2 (this stand-in document's canvas has none): said, and nothing fetched.
  const plain = host();
  let fetched3 = 0;
  const v3 = MV.open(plain, { name: "ship.glb", size: 2224, fetchBytes: async () => { fetched3++; return bytesOf("model-cube.glb"); } });
  await v3.done;
  assert.equal(shownText(plain), "Your browser cannot draw 3D here; Download the file to see it.");
  assert.equal(fetched3, 0);

  // A broken file, a Draco file, and a fetch the server refused.
  const said = async (fetchBytes, name) => {
    const b = host();
    const v = MV.open(b, { name: name || "ship.glb", size: 100, renderer: standIn().factory, fetchBytes });
    await v.done;
    return shownText(b);
  };
  assert.equal(await said(async () => new ArrayBuffer(40)), "This 3D file could not be read: it is not a binary glTF (.glb) file.");
  assert.equal(await said(async () => brokenCube((j) => { j.extensionsRequired = ["KHR_draco_mesh_compression"]; })), MV.WORDS.draco);
  assert.equal(await said(async () => { throw new Error("The server answered 403."); }), "This model could not be fetched: The server answered 403.", "one wording for a failed fetch, both apps");
  const stl = host();
  const v5 = MV.open(stl, { name: "wedge.stl", size: 262, renderer: standIn().factory, fetchBytes: async () => bytesOf("model-wedge-ascii.stl") });
  await v5.done;
  assert.equal(v5.phase(), "shown", "an STL is read as STL: " + shownText(stl));
  assert.ok(shownLine(stl).endsWith(" 2 triangles."), shownLine(stl));
  v5.close();

  // Closed while the file is on its way: the fetch is stopped and nothing is drawn after.
  const early = host();
  const r4 = standIn();
  const slow = deferred();
  let signal = null;
  const v4 = MV.open(early, { name: "ship.glb", size: 2224, renderer: r4.factory, fetchBytes: (s) => { signal = s; return slow.promise; } });
  await wait(0);
  v4.close();
  assert.equal(signal && signal.aborted, true, "the fetch is stopped");
  slow.resolve(bytesOf("model-cube.glb"));
  await v4.done;
  await wait(40);
  assert.equal(r4.prepared, null, "nothing read after Close");
  assert.equal(early.children.length, 0);
});

// ── 7. Each texture's own filtering and wrapping ────────────────────────

// Seen red 2026-10-16 with samplerOf() reading nothing from the file (`const s = {}`): "the cube's
// sampler, NEAREST both ways" got magFilter 9729, minFilter 9987 (the defaults) instead of 9728, 9728.
test("7. each texture keeps the file's own sampler; none, or a field left out, gets the defaults", () => {
  const NEAREST = 9728, LINEAR = 9729, LINEAR_MIPMAP_LINEAR = 9987, REPEAT = 10497, CLAMP = 33071, MIRROR = 33648;
  const defaults = { magFilter: LINEAR, minFilter: LINEAR_MIPMAP_LINEAR, wrapS: REPEAT, wrapT: REPEAT };
  const cube = MV.parseGlb(bytesOf("model-cube.glb"));
  assert.deepEqual(cube.materials[0].baseTexture, { image: 0, sampler: { magFilter: NEAREST, minFilter: NEAREST, wrapS: REPEAT, wrapT: REPEAT } }, "the cube's sampler, NEAREST both ways");
  const none = MV.parseGlb(brokenCube((j) => { delete j.textures[0].sampler; }));
  assert.deepEqual(none.materials[0].baseTexture.sampler, defaults, "no sampler: the defaults");
  const some = MV.parseGlb(brokenCube((j) => { j.samplers[0] = { wrapS: CLAMP, wrapT: MIRROR }; }));
  assert.deepEqual(some.materials[0].baseTexture.sampler, { magFilter: LINEAR, minFilter: LINEAR_MIPMAP_LINEAR, wrapS: CLAMP, wrapT: MIRROR }, "a field left out: its default");
  // One picture used twice with two samplers: one picture, each use with its own sampler.
  const twice = MV.parseGlb(brokenCube((j) => {
    j.samplers.push({ magFilter: LINEAR, minFilter: 9984 });
    j.textures.push({ source: 0, sampler: 1 });
    j.materials[0].emissiveTexture = { index: 1 };
  }));
  assert.equal(twice.images.length, 1, "the picture once");
  assert.deepEqual(twice.materials[0].emissiveTexture, { image: 0, sampler: { magFilter: LINEAR, minFilter: 9984, wrapS: REPEAT, wrapT: REPEAT } }, "its second use with its own sampler");
  assert.equal(twice.materials[0].baseTexture.sampler.magFilter, NEAREST, "the first use keeps its own");
  assert.match(refusal(() => MV.parseGlb(brokenCube((j) => { j.textures[0].sampler = 5; }))).message, /a sampler that is not there/);
  // STL has no textures.
  assert.equal(MV.parseStl(bytesOf("model-wedge.stl")).materials[0].baseTexture, null);
});

// ── 8. The vertex and node limits ───────────────────────────────────────

// Seen red 2026-10-16 twice: with the vertex check taken out, the 15,000,003 positions were read and
// refused for having no data ("This 3D file could not be read: an accessor has no data.");
// with the node-visit check taken out, all 8,388,607 visits ran and the file was refused for
// holding no triangles ("This 3D file could not be read: it holds no triangles.").
test("8. too many vertices, and nodes that repeat each other, say the shared words", () => {
  // One triangle drawn from 15,000,003 vertices: under the triangle limit, over the vertex one.
  const many = pack({
    asset: { version: "2.0" }, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0 }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1 }] }],
    accessors: [{ componentType: 5126, count: 15000003, type: "VEC3" }, { componentType: 5123, count: 3, type: "SCALAR" }],
  }, null);
  const vertices = refusal(() => MV.parseGlb(many));
  assert.equal(vertices.kind, "too_many_vertices");
  assert.equal(vertices.message, "This model has 15,000,003 vertices, more than the 15,000,000 the viewer draws. Download it to see it.");

  // 23 levels of nodes, each holding the next twice: 8,388,607 visits from one root, 22 deep.
  const nodes = [];
  for (let i = 0; i < 22; i++) nodes.push({ children: [i + 1, i + 1] });
  nodes.push({});
  const fanned = pack({ asset: { version: "2.0" }, scenes: [{ nodes: [0] }], nodes }, null);
  assert.equal(refusal(() => MV.parseGlb(fanned)).message, "This 3D file could not be read: its nodes repeat each other too many times.");

  // Nesting past 64 keeps its own words.
  const deep = [];
  for (let i = 0; i < 70; i++) deep.push({ children: [i + 1] });
  deep.push({});
  assert.equal(refusal(() => MV.parseGlb(pack({ asset: { version: "2.0" }, scenes: [{ nodes: [0] }], nodes: deep }, null))).message, "This 3D file could not be read: its nodes nest too deep (or loop back on themselves).");
});
