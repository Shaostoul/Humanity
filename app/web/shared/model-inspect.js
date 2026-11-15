/* ── model-inspect.js ───────────────────────────────────────────────────────
 * The model inspector's reading (docs/design/model-inspector.md), for the
 * website's 3D viewer (web/shared/model-view.js, its Inspect mode) and the
 * command line (scripts/inspect-model.js). The desktop app's inspector is the
 * original (src/gui/model_view/inspect.rs); this one reads the same files the
 * same way, against the same conventions (data/models/inspect_conventions.json),
 * and gives the same findings with the same codes and the same words
 * (data/i18n/en.json, model_inspect.finding.<code>).
 *
 * inspect(name, bytes, opts) -> inspection, where opts holds:
 *   conventions  the parsed conventions file;
 *   physics      the physics.ron beside the file, as text (or undefined);
 *   catalog      the catalog rows that draw this model: [{file, id, label, shape, size: [w, h, d]}];
 *   readOutside  (uri) -> bytes or null: a .gltf's outside files (the command line reads them
 *                from beside the file; the website has none).
 * Every offset, length, count, stride and index the file gives is checked
 * before it is used: a broken file is a finding, never a read outside it.
 *
 * A classic script in the browser (window.HosModelInspect), a CommonJS module
 * under Node (scripts/tests/model-inspect.test.js runs it on the planted models
 * in scripts/tests/fixtures/inspect/, the files the desktop's tests read).
 */
(function (root) {
  'use strict';

  const MAX_EDGES = 20000;
  const MAX_TEXTURE_SIDE = 4096;
  const COMP = { 5120: [Int8Array, 1], 5121: [Uint8Array, 1], 5122: [Int16Array, 2], 5123: [Uint16Array, 2], 5125: [Uint32Array, 4], 5126: [Float32Array, 4] };
  const WIDTH = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT2: 4, MAT3: 9, MAT4: 16 };

  // ── Small vector and matrix helpers (column-major 4x4, as glTF writes them) ──
  const identity = () => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  function multiply(a, b) {
    const o = new Array(16).fill(0);
    for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) o[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k];
    return o;
  }
  function compose(t, q, s) {
    const [x, y, z, w] = q;
    const m = [
      (1 - 2 * (y * y + z * z)) * s[0], 2 * (x * y + z * w) * s[0], 2 * (x * z - y * w) * s[0], 0,
      2 * (x * y - z * w) * s[1], (1 - 2 * (x * x + z * z)) * s[1], 2 * (y * z + x * w) * s[1], 0,
      2 * (x * z + y * w) * s[2], 2 * (y * z - x * w) * s[2], (1 - 2 * (x * x + y * y)) * s[2], 0,
      t[0], t[1], t[2], 1,
    ];
    return m;
  }
  function nodeMatrix(n) {
    if (Array.isArray(n.matrix) && n.matrix.length === 16) return n.matrix.map(Number);
    return compose(n.translation || [0, 0, 0], n.rotation || [0, 0, 0, 1], n.scale || [1, 1, 1]);
  }
  const point = (m, p) => [m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14]];
  const col = (m, c) => [m[c * 4], m[c * 4 + 1], m[c * 4 + 2]];
  const len = (v) => Math.hypot(v[0], v[1], v[2]);
  const norm = (v) => { const l = len(v); return l > 0 ? [v[0] / l, v[1] / l, v[2] / l] : [0, 0, 0]; };
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const isIdentity = (m) => identity().every((v, i) => Math.abs(v - m[i]) <= 1e-5);
  const lo = () => [Infinity, Infinity, Infinity];
  const hi = () => [-Infinity, -Infinity, -Infinity];
  const grow = (a, b, p) => { for (let k = 0; k < 3; k++) { a[k] = Math.min(a[k], p[k]); b[k] = Math.max(b[k], p[k]); } };
  const finite = (v) => v.every(Number.isFinite);

  /** Metres to two decimals, the report's way. */
  const m2 = (x) => Number(x).toFixed(2);

  function asBytes(input) {
    if (input instanceof Uint8Array) return input;
    if (input instanceof ArrayBuffer) return new Uint8Array(input);
    if (ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    throw new Error('not bytes');
  }

  function base64Bytes(s) {
    if (typeof Buffer !== 'undefined') return new Uint8Array(Buffer.from(s, 'base64'));
    const bin = atob(s);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }
  function dataUri(uri) {
    const m = /^data:[^,]*;base64,(.*)$/s.exec(uri || '');
    return m ? base64Bytes(m[1]) : null;
  }

  // ── The conventions ──
  function plainName(name) {
    const n = String(name || '').trim().toLowerCase();
    const m = /^(.*)\.(\d{3})$/.exec(n);
    return m ? m[1] : n;
  }
  function emptyRule(conv, name) {
    const n = plainName(name);
    return (conv.empties || []).find((r) => (r.prefixes || []).some((p) => p && n.startsWith(p.toLowerCase()))) || null;
  }
  function isCollision(conv, name) {
    const n = plainName(name);
    const c = conv.collision || {};
    return (c.prefixes || []).some((p) => p && n.startsWith(p.toLowerCase())) || (c.suffixes || []).some((s) => s && n.endsWith(s.toLowerCase()));
  }
  function lodOf(conv, name) {
    const marker = ((conv.lod || {}).marker || '').toLowerCase();
    if (!marker) return null;
    const n = plainName(name);
    const at = n.lastIndexOf(marker);
    if (at < 0) return null;
    const rest = n.slice(at + marker.length);
    return /^\d+$/.test(rest) ? Number(rest) : null;
  }
  const TOL = { catalog_share: 0.1, catalog_floor_m: 0.05, collision_share: 0.1, collision_floor_m: 0.05, level_cos: 0.95 };
  const SCALE = { tallest_m: 50, smallest_m: 0.005 };

  // ── The file ──
  /** The JSON and binary chunk of a .glb, or a .gltf's JSON. */
  function container(bytes) {
    if (bytes.length >= 12 && bytes[0] === 0x67 && bytes[1] === 0x6c && bytes[2] === 0x54 && bytes[3] === 0x46) {
      const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      let at = 12;
      let json = null;
      let bin = null;
      while (at + 8 <= bytes.length) {
        const n = dv.getUint32(at, true);
        const type = dv.getUint32(at + 4, true);
        if (at + 8 + n > bytes.length) throw new Error('a chunk runs past the end of the file');
        const body = bytes.subarray(at + 8, at + 8 + n);
        if (type === 0x4e4f534a && !json) json = body;
        else if (type === 0x004e4942 && !bin) bin = body;
        at += 8 + n;
      }
      if (!json) throw new Error('it has no JSON part');
      return { doc: JSON.parse(new TextDecoder().decode(json)), bin };
    }
    return { doc: JSON.parse(new TextDecoder().decode(bytes)), bin: null };
  }

  function reader(doc, buffers) {
    function accessor(i) {
      const a = (doc.accessors || [])[i];
      if (!a || a.sparse) return null;
      const comp = COMP[a.componentType];
      const width = WIDTH[a.type];
      if (!comp || !width || !(a.count >= 0)) return null;
      const view = (doc.bufferViews || [])[a.bufferView];
      if (!view) return null;
      const buf = buffers[view.buffer];
      if (!buf) return null;
      const vOff = view.byteOffset || 0;
      if (vOff + view.byteLength > buf.length) return null;
      const elem = comp[1] * width;
      const stride = view.byteStride || elem;
      if (stride < elem) return null;
      const off = a.byteOffset || 0;
      if (a.count > 0 && off + (a.count - 1) * stride + elem > view.byteLength) return null;
      const dv = new DataView(buf.buffer, buf.byteOffset + vOff, view.byteLength);
      const get = { 5120: 'getInt8', 5121: 'getUint8', 5122: 'getInt16', 5123: 'getUint16', 5125: 'getUint32', 5126: 'getFloat32' }[a.componentType];
      return { count: a.count, width, at: (k, j) => dv[get](off + k * stride + j * comp[1], true), a };
    }
    /** A primitive's positions and triangles, or null. */
    function triangles(p) {
      if ((p.mode === undefined ? 4 : p.mode) !== 4) return null;
      const pos = accessor((p.attributes || {}).POSITION);
      if (!pos || pos.a.componentType !== 5126 || pos.width !== 3) return null;
      const positions = [];
      for (let k = 0; k < pos.count; k++) positions.push([pos.at(k, 0), pos.at(k, 1), pos.at(k, 2)]);
      let idx;
      if (p.indices !== undefined) {
        const ia = accessor(p.indices);
        if (!ia || ia.width !== 1 || ![5121, 5123, 5125].includes(ia.a.componentType)) return null;
        idx = [];
        for (let k = 0; k < ia.count; k++) idx.push(ia.at(k, 0));
      } else {
        idx = positions.map((_, k) => k);
      }
      if (idx.some((i) => i >= positions.length)) return null;
      idx.length -= idx.length % 3;
      return { positions, idx };
    }
    return { triangles };
  }

  /** A picture's width and height from its first bytes (PNG, JPEG, GIF, WebP), or null. */
  function imageSize(b) {
    if (!b || b.length < 24) return null;
    if (b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47) {
      const dv = new DataView(b.buffer, b.byteOffset, b.byteLength);
      return [dv.getUint32(16), dv.getUint32(20)];
    }
    if (b[0] === 0x47 && b[1] === 0x49 && b[2] === 0x46) return [b[6] | (b[7] << 8), b[8] | (b[9] << 8)];
    if (b[0] === 0x52 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x46 && b[8] === 0x57 && b[9] === 0x45) {
      const tag = String.fromCharCode(b[12], b[13], b[14], b[15]);
      if (tag === 'VP8X') return [1 + (b[24] | (b[25] << 8) | (b[26] << 16)), 1 + (b[27] | (b[28] << 8) | (b[29] << 16))];
      if (tag === 'VP8L') return [1 + (b[21] | ((b[22] & 0x3f) << 8)), 1 + ((b[22] >> 6) | (b[23] << 2) | ((b[24] & 0x0f) << 10))];
      if (tag === 'VP8 ') return [(b[26] | (b[27] << 8)) & 0x3fff, (b[28] | (b[29] << 8)) & 0x3fff];
      return null;
    }
    if (b[0] === 0xff && b[1] === 0xd8) {
      let i = 2;
      while (i + 9 < b.length) {
        if (b[i] !== 0xff) { i++; continue; }
        const mk = b[i + 1];
        const seg = (b[i + 2] << 8) | b[i + 3];
        if (mk >= 0xc0 && mk <= 0xcf && mk !== 0xc4 && mk !== 0xc8 && mk !== 0xcc) return [(b[i + 7] << 8) | b[i + 8], (b[i + 5] << 8) | b[i + 6]];
        i += 2 + seg;
      }
    }
    return null;
  }

  /** physics.ron's colliders, read for what the inspector needs: [{part, hull}], or an error. */
  function parsePhysics(text) {
    if (typeof text !== 'string') return null;
    if (!/colliders\s*:/.test(text)) return { error: 'it has no colliders list' };
    const colliders = [];
    const re = /\(\s*body\s*:\s*"([^"]*)"\s*,\s*part\s*:\s*"([^"]*)"[^()]*?hull_mesh\s*:\s*(?:Some\s*\(\s*(\d+)\s*\)|None)/g;
    let m;
    while ((m = re.exec(text))) colliders.push({ part: m[2], hull: m[3] === undefined ? null : Number(m[3]) });
    return { colliders };
  }

  // ── The fit, as the game does it (GltfCpuMesh::fit_for_machine) ──
  // A turn of yawDeg degrees about up, glam's sense (+Z toward +X), exact for whole quarter turns
  // (assets::quarter_exact_sin_cos): [sin, cos].
  function quarterSinCos(yawDeg) {
    if (!Number.isFinite(yawDeg)) return [0, 1];
    const q = yawDeg / 90;
    if (Math.abs(q - Math.round(q)) < 1e-4) return [[0, 1], [1, 0], [0, -1], [-1, 0]][((Math.round(q) % 4) + 4) % 4];
    const r = (yawDeg * Math.PI) / 180;
    return [Math.sin(r), Math.cos(r)];
  }
  // The bounds of positions turned by a row's model_yaw (GltfCpuMesh::turn_about_up), what the fit
  // measures for a model turned inside its box: [min, max].
  function turnedBounds(positions, yawDeg) {
    const [s, c] = quarterSinCos(yawDeg || 0);
    const mn = [Infinity, Infinity, Infinity];
    const mx = [-Infinity, -Infinity, -Infinity];
    for (const v of positions) {
      const p = [c * v[0] + s * v[2], v[1], -s * v[0] + c * v[2]];
      for (let k = 0; k < 3; k++) { mn[k] = Math.min(mn[k], p[k]); mx[k] = Math.max(mx[k], p[k]); }
    }
    return [mn, mx];
  }
  // rawMin..rawMax: the first part's positions turned by the row's model_yaw (turnedBounds). The
  // box (min, max) is in that turned frame; yaw says how to turn it back onto the file.
  function fit(rawMin, rawMax, row, tol) {
    tol = Object.assign({}, TOL, tol || {});
    const raw = [0, 1, 2].map((k) => Math.max(rawMax[k] - rawMin[k], 0));
    const target = row.size[1];
    let s = Number.isFinite(target) && target > 0 && raw[1] > 1e-6 ? target / raw[1] : 1;
    if (!Number.isFinite(s) || Math.abs(s - 1) < 1e-4) s = 1;
    const fitted = [raw[0] * s, raw[1] * s, raw[2] * s];
    const boxWd = row.shape === 'cylinder' || row.shape === 'sphere' ? [row.size[0] * 2, row.size[0] * 2] : [row.size[0], row.size[2]];
    const near = (a, b) => Math.abs(a - b) <= Math.max(b * tol.catalog_share, tol.catalog_floor_m);
    const verdict = near(fitted[0], boxWd[0]) && near(fitted[2], boxWd[1]) ? 'fits' : near(fitted[2], boxWd[0]) && near(fitted[0], boxWd[1]) ? 'turned' : 'off';
    const err = (x, y) => Math.abs(x - y) / Math.max(y, 1e-6);
    const across = verdict === 'off' && Math.max(err(fitted[2], boxWd[0]), err(fitted[0], boxWd[1])) < Math.max(err(fitted[0], boxWd[0]), err(fitted[2], boxWd[1]));
    const cx = (rawMin[0] + rawMax[0]) / 2;
    const cz = (rawMin[2] + rawMax[2]) / 2;
    const hw = boxWd[0] / 2 / s;
    const hd = boxWd[1] / 2 / s;
    return { row, scale: s, fitted, box: boxWd, verdict, across, min: [cx - hw, 0, cz - hd], max: [cx + hw, target / s, cz + hd], yaw: row.model_yaw || 0 };
  }

  // ── The inspection ──
  const LEVEL = { problem: 0, check: 1, note: 2 };

  function inspect(name, input, opts) {
    opts = opts || {};
    const conv = opts.conventions || {};
    const tol = Object.assign({}, TOL, conv.tolerance || {});
    const scale = Object.assign({}, SCALE, conv.scale || {});
    const catalog = opts.catalog || [];
    const out = { file: name, stl: false, nodes: [], min: [0, 0, 0], max: [0, 0, 0], parts: 0, materials: 0, textures: [], clips: [], skins: 0, colliders: [], edgesLeftOut: 0, catalog: [], findings: [] };
    const find = (level, code, vars) => out.findings.push({ level, code, vars: vars || {} });
    if (opts.conventionsError) find('check', 'conventions_missing', { why: opts.conventionsError });
    const bytes = asBytes(input);
    if (/\.stl$/i.test(name)) {
      out.stl = true;
      const b = stlBounds(bytes);
      if (b) { out.min = b.min; out.max = b.max; }
      out.parts = 1;
      const s = size(out);
      find('note', 'stl_units', { w: m2(s[0]), h: m2(s[1]), d: m2(s[2]) });
      scaleFindings(out, scale, find);
      return finish(out);
    }
    try {
      inspectGltf(bytes, out, { conv, tol, scale, catalog, physics: parsePhysics(opts.physics), readOutside: opts.readOutside || (() => null), find });
    } catch (e) {
      find('problem', 'unreadable', { why: String(e && e.message || e) });
    }
    return finish(out);
  }

  function finish(out) {
    out.findings = out.findings.map((f, i) => [f, i]).sort((a, b) => LEVEL[a[0].level] - LEVEL[b[0].level] || a[1] - b[1]).map(([f]) => f);
    return out;
  }
  const size = (out) => [0, 1, 2].map((k) => Math.max(out.max[k] - out.min[k], 0));

  function scaleFindings(out, scale, find) {
    const largest = Math.max(...size(out));
    if (largest > scale.tallest_m) find('check', 'scale_huge', { size: m2(largest), cm: m2(largest / 100), mm: m2(largest / 1000) });
    else if (largest > 0 && largest < scale.smallest_m) find('check', 'scale_tiny', { size: largest.toFixed(4), big: m2(largest * 1000) });
  }

  /** An STL's bounds, turned from z-up to y-up (x stays, y = z, z = -y), as the viewer reads it. */
  function stlBounds(b) {
    const a = lo();
    const z = hi();
    const add = (x, y, zz) => grow(a, z, [x, zz, -y]);
    const text = new TextDecoder().decode(b.subarray(0, Math.min(b.length, 512)));
    const dv = new DataView(b.buffer, b.byteOffset, b.byteLength);
    if (b.length >= 84 && 84 + dv.getUint32(80, true) * 50 === b.length) {
      const n = dv.getUint32(80, true);
      for (let t = 0; t < n; t++) for (let v = 0; v < 3; v++) { const o = 84 + t * 50 + 12 + v * 12; add(dv.getFloat32(o, true), dv.getFloat32(o + 4, true), dv.getFloat32(o + 8, true)); }
    } else if (/^\s*solid/.test(text)) {
      const re = /vertex\s+(\S+)\s+(\S+)\s+(\S+)/g;
      const all = new TextDecoder().decode(b);
      let m;
      while ((m = re.exec(all))) add(Number(m[1]), Number(m[2]), Number(m[3]));
    }
    return finite(a) && finite(z) ? { min: a, max: z } : null;
  }

  function inspectGltf(bytes, out, ctx) {
    const { conv, tol, scale, catalog, physics, readOutside, find } = ctx;
    const { doc, bin } = container(bytes);
    const buffers = (doc.buffers || []).map((b, i) => {
      let data = null;
      if (b.uri === undefined) data = i === 0 ? bin : null;
      else data = dataUri(b.uri) || readOutside(b.uri);
      if (!data) throw new Error(b.uri === undefined ? 'its binary part is missing' : `its file ${b.uri} is not beside it`);
      if (data.length < b.byteLength) throw new Error('a buffer is shorter than it says');
      return data;
    });
    const rd = reader(doc, buffers);
    const meshes = doc.meshes || [];
    const nodes = doc.nodes || [];

    // The node tree: the scene the viewer draws.
    const scene = (doc.scenes || [])[doc.scene !== undefined ? doc.scene : 0];
    let roots;
    if (scene) roots = scene.nodes || [];
    else {
      const kids = new Set(nodes.flatMap((n) => n.children || []));
      roots = nodes.map((_, i) => i).filter((i) => !kids.has(i));
    }
    const meshNodes = [];
    const collisionNodes = [];
    let visits = 0;
    const walk = (i, parent, depth, parentSlot) => {
      visits++;
      if (visits > 100000 || depth > 64) return;
      const n = nodes[i];
      if (!n) return;
      const world = multiply(parent, nodeMatrix(n));
      const name = n.name || '';
      const mesh = n.mesh !== undefined ? meshes[n.mesh] : undefined;
      let kind;
      let convention = null;
      if (mesh) {
        if (isCollision(conv, name) || (mesh.name && isCollision(conv, mesh.name))) {
          kind = 'collision';
          collisionNodes.push([n.mesh, world, name]);
        } else {
          let lod = lodOf(conv, name);
          if (lod === null && mesh.name) lod = lodOf(conv, mesh.name);
          if ((lod || 0) === 0) meshNodes.push([n.mesh, world]);
          kind = lod === null ? 'mesh' : 'lod';
          if (lod !== null) convention = String(lod);
        }
      } else {
        const r = emptyRule(conv, name);
        if (r) { kind = 'empty'; convention = r.id; }
        else if (!(n.children || []).length) kind = 'empty';
        else kind = 'group';
      }
      const slot = out.nodes.length;
      out.nodes.push({ index: i, name, depth, parent: parentSlot, kind, convention, mesh: mesh ? n.mesh : null, world });
      for (const c of n.children || []) walk(c, world, depth + 1, slot);
    };
    for (const r of roots) walk(r, identity(), 0, null);

    // The visible bounds.
    const a = lo();
    const z = hi();
    for (const [mi, world] of meshNodes) {
      for (const p of (meshes[mi].primitives || [])) {
        const t = rd.triangles(p);
        if (!t) continue;
        out.parts++;
        for (const v of t.positions) grow(a, z, point(world, v));
      }
    }
    if (finite(a) && finite(z)) { out.min = a; out.max = z; }
    out.materials = (doc.materials || []).length;
    out.skins = (doc.skins || []).length;
    out.clips = (doc.animations || []).map((an, i) => {
      let seconds = 0;
      for (const s of an.samplers || []) {
        const acc = (doc.accessors || [])[s.input];
        if (acc && Array.isArray(acc.max) && Number.isFinite(acc.max[0])) seconds = Math.max(seconds, acc.max[0]);
      }
      return { name: an.name || `#${i}`, seconds, channels: (an.channels || []).length };
    });

    // Size, pivot and scale.
    scaleFindings(out, scale, find);
    const s = size(out);
    const centre = [0, 1, 2].map((k) => (out.min[k] + out.max[k]) / 2);
    const centred = Math.abs(centre[0]) <= Math.max(s[0] * 0.1, 0.01) && Math.abs(centre[2]) <= Math.max(s[2] * 0.1, 0.01);
    const onBase = Math.abs(out.min[1]) <= Math.max(s[1] * 0.02, 0.005);
    if (centred && onBase) find('note', 'pivot_base_centred', {});
    else find('check', 'pivot_off', { x: m2(centre[0]), z: m2(centre[2]), y: m2(out.min[1]) });

    // Textures.
    const SLOTS = [['base', (m) => (m.pbrMetallicRoughness || {}).baseColorTexture], ['metal_rough', (m) => (m.pbrMetallicRoughness || {}).metallicRoughnessTexture], ['normal', (m) => m.normalTexture], ['occlusion', (m) => m.occlusionTexture], ['emissive', (m) => m.emissiveTexture]];
    (doc.materials || []).forEach((mat, mi) => {
      const mname = mat.name || `#${mi}`;
      const seen = new Set();
      for (const [slot, get] of SLOTS) {
        const info = get(mat);
        if (!info) continue;
        const tex = (doc.textures || [])[info.index];
        if (!tex || tex.source === undefined) continue;
        const img = (doc.images || [])[tex.source];
        if (!img || seen.has(`${slot}:${tex.source}`)) continue;
        seen.add(`${slot}:${tex.source}`);
        let file;
        let data = null;
        if (img.bufferView !== undefined) {
          file = img.name || `#${tex.source}`;
          const v = (doc.bufferViews || [])[img.bufferView];
          const buf = v && buffers[v.buffer];
          if (buf && (v.byteOffset || 0) + v.byteLength <= buf.length) data = buf.subarray(v.byteOffset || 0, (v.byteOffset || 0) + v.byteLength);
        } else {
          const uri = img.uri || '';
          file = uri.startsWith('data:') ? `#${tex.source}` : uri;
          data = dataUri(uri) || readOutside(uri);
        }
        const dims = imageSize(data);
        if (!dims) find('problem', 'texture_missing', { material: mname, slot, file });
        else if (Math.max(dims[0], dims[1]) > MAX_TEXTURE_SIDE) find('check', 'texture_big', { file, w: String(dims[0]), h: String(dims[1]) });
        out.textures.push({ material: mname, slot, file, size: dims });
      }
    });

    // What the game draws: the first mesh's first part, as written.
    const first = meshes[0];
    const firstPrim = first && (first.primitives || [])[0];
    const engine = firstPrim ? rd.triangles(firstPrim) : null;
    let engineMin = null;
    let engineMax = null;
    if (engine) {
      engineMin = lo();
      engineMax = hi();
      for (const v of engine.positions) grow(engineMin, engineMax, v);
    }
    const shows = out.nodes.filter((n) => n.mesh === 0 && (n.kind === 'mesh' || (n.kind === 'lod' && n.convention === '0')));
    const firstMoved = !!first && (shows.length === 0 || shows.some((n) => !isIdentity(n.world)));
    if (out.parts > 1 || firstMoved) find(catalog.length ? 'problem' : 'check', 'engine_differs', { part: (first && first.name) || '#0', parts: String(out.parts) });
    if (engineMin && finite(engineMin) && finite(engineMax)) {
      if (catalog.length && Math.abs(engineMin[1]) > Math.max(engineMax[1] - engineMin[1], 0) * 0.02 + 0.005) {
        find('problem', engineMin[1] > 0 ? 'base_above' : 'base_below', { y: m2(Math.abs(engineMin[1])) });
      }
      for (const row of catalog) {
        // Turned inside its box first, as the game turns it (the row's model_yaw).
        const [tMin, tMax] = turnedBounds(engine.positions, row.model_yaw);
        const f = fit(tMin, tMax, row, tol);
        const vars = { row: row.id, file: row.file, box_w: m2(f.box[0]), box_d: m2(f.box[1]), h: m2(f.fitted[1]), w: m2(f.fitted[0]), d: m2(f.fitted[2]) };
        const code = f.verdict === 'fits' ? 'catalog_fits' : f.verdict === 'turned' ? 'catalog_turned' : f.across ? 'catalog_across' : 'catalog_box';
        find(f.verdict === 'fits' ? 'note' : 'problem', code, vars);
        out.catalog.push(f);
      }
    }

    // Collision.
    let room = MAX_EDGES;
    const wire = (label, mi, world) => {
      const w = { label, edges: [], min: lo(), max: hi() };
      for (const p of (meshes[mi].primitives || [])) {
        const t = rd.triangles(p);
        if (!t) continue;
        const seenE = new Set();
        for (let k = 0; k + 2 < t.idx.length; k += 3) {
          for (const [x, y] of [[t.idx[k], t.idx[k + 1]], [t.idx[k + 1], t.idx[k + 2]], [t.idx[k + 2], t.idx[k]]]) {
            const key = Math.min(x, y) * 4294967296 + Math.max(x, y);
            if (seenE.has(key)) continue;
            seenE.add(key);
            const pa = point(world, t.positions[x]);
            const pb = point(world, t.positions[y]);
            grow(w.min, w.max, pa);
            grow(w.min, w.max, pb);
            if (room === 0) out.edgesLeftOut++;
            else { w.edges.push([pa, pb]); room--; }
          }
        }
      }
      return w;
    };
    if (physics && physics.error) find('problem', 'physics_bad', { why: physics.error });
    else if (physics) {
      for (const c of physics.colliders) {
        if (c.hull === null) continue;
        if (meshes[c.hull]) out.colliders.push(wire(c.part, c.hull, identity()));
        else find('problem', 'physics_bad', { why: `collider ${c.part} names mesh ${c.hull}, which this file does not have` });
      }
    }
    for (const [mi, world, label] of collisionNodes) out.colliders.push(wire(label, mi, world));
    out.colliders = out.colliders.filter((w) => finite(w.min) && finite(w.max));
    if (!out.colliders.length) find('note', 'collision_none', {});
    else {
      const cmin = lo();
      const cmax = hi();
      for (const w of out.colliders) { grow(cmin, cmax, w.min); grow(cmin, cmax, w.max); }
      const gap = (x, y, side) => Math.abs(x - y) > Math.max(side * tol.collision_share, tol.collision_floor_m);
      const off = [0, 1, 2].some((k) => gap(cmin[k], out.min[k], s[k]) || gap(cmax[k], out.max[k], s[k]));
      if (off) {
        const worst = Math.max(...[0, 1, 2].map((k) => Math.max(Math.abs(cmin[k] - out.min[k]), Math.abs(cmax[k] - out.max[k]))));
        find('problem', 'collision_off', { gap: m2(worst), count: String(out.colliders.length) });
      } else find('note', 'collision_ok', { count: String(out.colliders.length) });
    }

    // The empties.
    for (const n of out.nodes) {
      if (n.kind !== 'empty') continue;
      if (!n.name.trim()) { find('check', 'empty_unnamed', { index: String(n.index) }); continue; }
      const rule = n.convention && (conv.empties || []).find((r) => r.id === n.convention);
      if (!rule) { find('check', 'empty_unknown', { name: n.name }); continue; }
      const at = col(n.world, 3);
      const fwd = norm(col(n.world, 2));
      const up = norm(col(n.world, 1));
      if (rule.forward === 'outward') {
        const away = [at[0] - centre[0], at[1] - centre[1], at[2] - centre[2]];
        if (len(away) > 1e-4 && dot(fwd, norm(away)) < 0) find('problem', 'empty_wrong_way', { name: n.name, kind: rule.id });
      } else if (rule.forward === 'level') {
        if (up[1] < tol.level_cos) find('problem', 'empty_not_level', { name: n.name, kind: rule.id });
      }
      if (rule.size) {
        const sz = [len(col(n.world, 0)), len(col(n.world, 1)), len(col(n.world, 2))];
        if (Math.min(...sz) < rule.size.min_m || Math.max(...sz) > rule.size.max_m) {
          find('problem', 'empty_size', { name: n.name, kind: rule.id, size: `${m2(sz[0])} x ${m2(sz[1])} x ${m2(sz[2])}`, min: m2(rule.size.min_m), max: m2(rule.size.max_m) });
        }
      }
    }
  }

  /** A finding's words, from en.json's model_inspect.finding.<code> (`words` is that object). */
  function wordsOf(finding, words) {
    const template = (words && words[finding.code]) || `model_inspect.finding.${finding.code}`;
    return template.replace(/\{\{(\w+)\}\}/g, (m, k) => (finding.vars[k] !== undefined ? String(finding.vars[k]) : m));
  }

  const api = { inspect, fit, turnedBounds, quarterSinCos, parsePhysics, plainName, emptyRule, isCollision, lodOf, imageSize, wordsOf, m2, MAX_EDGES };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosModelInspect = api;
})(typeof window !== 'undefined' ? window : globalThis);
