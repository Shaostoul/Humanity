/* ── model-view.js ──────────────────────────────────────────────────────────
 * The 3D model viewer on the website (2026-10-16, docs/design/model-viewer.md).
 * The desktop app's viewer is the original (src/gui/model_view/: load.rs reads
 * the files, camera.rs turns the camera, model_view.wgsl shades); this one
 * reads the same files the same way, says the same words, turns the same
 * camera and shades with the same lights, in WebGL 2 with no outside library
 * and nothing downloaded.
 *
 * What is in it:
 *  - the readers: parseGlb(bytes) and parseStl(bytes) (and parseModel(name,
 *    bytes), which picks by the name's ending) turn a file into one model:
 *    world-space positions, normals and texture coordinates, indices, the runs
 *    drawn with each material, the materials, the pictures (still encoded),
 *    the bounds and the triangle count. Every offset, length, count, stride and
 *    index the file gives is checked before it is used, so a broken or hostile
 *    file is an Error with the viewer's words, never a read outside the file;
 *  - the camera: fitCamera, orbitCamera, panCamera, zoomCamera, resetCamera
 *    and the view and projection matrices, as the spec gives them;
 *  - the view: HosModelView.open(container, {name, size, fetchBytes}) fetches
 *    the file (fetchBytes(signal) answers its bytes), reads it, and draws it,
 *    only when something changed, with 4x multisampling, turned by the mouse or
 *    a finger. It returns {close()}, which gives every WebGL object back.
 *
 * A classic script in the browser (window.HosModelView), a CommonJS module
 * under Node, where scripts/tests/model-view-web.test.js runs the readers and
 * the camera on the shared test files (scripts/tests/fixtures/model-*).
 */
(function (root) {
  'use strict';

  // ── Limits (the spec's, the desktop's load.rs) ──
  const MAX_MODEL_BYTES = 200 * 1024 * 1024;
  const MAX_TRIANGLES = 5000000;
  const MAX_TEXTURE_SIDE = 4096;
  // How deep nodes may nest (a file whose nodes loop back on themselves stops here).
  const MAX_NODE_DEPTH = 64;
  // How many nodes one walk of the scene visits at most: a file whose nodes share children over
  // and over would otherwise ask for more mesh copies than there are atoms.
  const MAX_NODE_VISITS = 1000000;
  // The most vertices the viewer reads (a few big meshes placed many times can stay under the
  // triangle limit while asking for more memory than a browser tab has).
  const MAX_VERTICES = 3 * MAX_TRIANGLES;
  // A view not drawn (out of sight, or its tab hidden) for this long gives its GPU memory back.
  const FORGET_AFTER_MS = 10000;

  // ── The camera (camera.rs) ──
  const FOV_Y_DEG = 35;
  const FOV_Y = FOV_Y_DEG * Math.PI / 180;
  const START_YAW = 35 * Math.PI / 180;
  const START_PITCH = 20 * Math.PI / 180;
  const MAX_PITCH = 89 * Math.PI / 180;
  // Radians turned per point dragged, and zoom per point scrolled (d *= exp(-points * this)).
  const TURN_PER_POINT = 0.008;
  const ZOOM_PER_POINT = 0.0015;

  // The three studio lights: direction towards the light in view space, colour, intensity.
  const LIGHTS = [
    [[-0.45, 0.60, 0.66], [1.00, 0.97, 0.92], 2.6],
    [[0.70, 0.15, 0.50], [0.85, 0.90, 1.00], 0.8],
    [[0.00, 0.45, -0.89], [1.00, 1.00, 1.00], 1.1],
  ];

  // ── Words (both apps, exactly: docs/design/model-viewer.md, "Words") ──
  const WORDS = Object.freeze({
    view: 'View in 3D',
    viewTip: 'Look at it here and turn it with the mouse. It is fetched from the server first ({size}).',
    fetching: 'Fetching {name} ({size})...',
    reading: 'Reading the model...',
    shown: '{name}: drag to turn it, scroll to zoom, right-drag or Shift-drag to move it, double-click to start over. {triangles} triangles.',
    close: 'Close the 3D view',
    tooLarge: 'This model is {size}, larger than the 200 MB the viewer opens. Download it to see it.',
    tooDetailed: 'This model has {triangles} triangles, more than the 5,000,000 the viewer draws. Download it to see it.',
    tooManyVertices: 'This model has {vertices} vertices, more than the 15,000,000 the viewer draws. Download it to see it.',
    draco: 'This model is compressed with Draco, which the viewer cannot read yet. Download it to see it in Blender or another 3D program.',
    unreadable: 'This 3D file could not be read: {reason}.',
    // The spec's "needs the app's graphics" words, for a browser.
    noGraphics: 'Your browser cannot draw 3D here; Download the file to see it.',
    // A fetch that failed (the server refused, or could not be reached), both apps.
    fetchFailed: 'This model could not be fetched: {reason}.',
  });

  /** `template` with each {key} replaced by `values[key]`. */
  function fill(template, values) {
    return String(template).replace(/\{(\w+)\}/g, (m, k) => (values && values[k] !== undefined ? String(values[k]) : m));
  }

  /** `12345` as `12,345`. */
  function thousands(n) {
    return String(Math.max(0, Math.floor(Number(n) || 0))).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  }

  function oneDecimal(x) {
    return x.toFixed(1).replace(/\.0$/, '');
  }

  /** Bytes as people read them: "412 MB", "12.4 GB", "900 KB" (the Media page's sizeWords). */
  function sizeWords(bytes) {
    const MIB = 1024 * 1024;
    const b = Math.max(0, Number(bytes) || 0);
    if (b >= 1024 * MIB) return oneDecimal(b / (1024 * MIB)) + ' GB';
    if (b >= MIB) return (b / MIB >= 100 ? String(Math.round(b / MIB)) : oneDecimal(b / MIB)) + ' MB';
    if (b >= 1024) return Math.round(b / 1024) + ' KB';
    return b + (b === 1 ? ' byte' : ' bytes');
  }

  /** Whether a file of this name opens in the viewer (`.glb`, `.stl`, any case). */
  function isModelFile(name) {
    return /\.(glb|stl)$/i.test(String(name || '').trim());
  }

  // ── Errors: each carries the words the view shows (`message`) and its `kind` ──
  function modelError(kind, words, extra) {
    const e = new Error(words);
    e.name = 'ModelError';
    e.kind = kind;
    e.words = words;
    return Object.assign(e, extra || {});
  }
  const unreadable = (reason) => modelError('unreadable', fill(WORDS.unreadable, { reason }), { reason });
  const tooLarge = (size) => modelError('too_large', fill(WORDS.tooLarge, { size: sizeWords(size) }), { size });
  const tooDetailed = (triangles) => modelError('too_detailed', fill(WORDS.tooDetailed, { triangles: thousands(triangles) }), { triangles });
  const tooManyVertices = (vertices) => modelError('too_many_vertices', fill(WORDS.tooManyVertices, { vertices: thousands(vertices) }), { vertices });
  const dracoError = () => modelError('draco', WORDS.draco);

  /** The bytes of an ArrayBuffer, a typed array or a Node Buffer, without copying. */
  function asBytes(input) {
    if (input instanceof ArrayBuffer) return new Uint8Array(input);
    if (input && ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    throw unreadable('nothing was read');
  }

  const isCount = (x) => Number.isInteger(x) && x >= 0 && x <= Number.MAX_SAFE_INTEGER;
  const isIndex = (x, len) => Number.isInteger(x) && x >= 0 && x < len;
  const list = (x) => (Array.isArray(x) ? x : []);
  /** `x` when it is an array of `n` finite numbers, else `fallback`. */
  function numbers(x, n, fallback) {
    return Array.isArray(x) && x.length === n && x.every((v) => typeof v === 'number' && Number.isFinite(v)) ? x.slice() : fallback.slice();
  }
  const finite = (x, fallback) => (typeof x === 'number' && Number.isFinite(x) ? x : fallback);

  // ── base64, for `data:` addresses written into the file itself ──
  const B64 = (() => {
    const t = new Int16Array(256).fill(-1);
    const abc = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    for (let i = 0; i < abc.length; i++) t[abc.charCodeAt(i)] = i;
    return t;
  })();
  /** The bytes of base64 text, or null when it is not base64. */
  function base64Bytes(s) {
    let end = s.length;
    while (end > 0 && s.charCodeAt(end - 1) === 61) end--; // '='
    if (s.length - end > 2 || end % 4 === 1) return null;
    const out = new Uint8Array(Math.floor(end * 3 / 4));
    let o = 0;
    let acc = 0;
    let bits = 0;
    for (let i = 0; i < end; i++) {
      const code = s.charCodeAt(i);
      const v = code < 256 ? B64[code] : -1;
      if (v < 0) return null;
      acc = (acc << 6) | v;
      bits += 6;
      if (bits >= 8) {
        bits -= 8;
        out[o++] = (acc >> bits) & 255;
        acc &= (1 << bits) - 1;
      }
    }
    return out;
  }
  /** A `data:<type>;base64,...` address: {mime, bytes}, or null for anything else (an outside file). */
  function dataUri(uri) {
    if (typeof uri !== 'string' || !uri.startsWith('data:')) return null;
    const comma = uri.indexOf(',');
    if (comma < 0) return null;
    const head = uri.slice(5, comma);
    if (!head.endsWith(';base64')) return null;
    const bytes = base64Bytes(uri.slice(comma + 1));
    return bytes ? { mime: head.split(';')[0], bytes } : null;
  }

  /** What a picture's first bytes say it is ('' when neither PNG nor JPEG). */
  function sniff(bytes) {
    if (bytes.length >= 8 && bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47) return 'image/png';
    if (bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) return 'image/jpeg';
    return '';
  }

  // ── 4x4 matrices, column-major (glTF's and WebGL's order) ──
  function identity() {
    const m = new Float64Array(16);
    m[0] = m[5] = m[10] = m[15] = 1;
    return m;
  }
  function multiply(a, b) {
    const o = new Float64Array(16);
    for (let c = 0; c < 4; c++) {
      for (let r = 0; r < 4; r++) {
        let s = 0;
        for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k];
        o[c * 4 + r] = s;
      }
    }
    return o;
  }
  /** Translation, rotation (a quaternion x, y, z, w) and scale as one matrix. */
  function compose(t, q, s) {
    let [x, y, z, w] = q;
    const len = Math.hypot(x, y, z, w);
    if (len > 0) { x /= len; y /= len; z /= len; w /= len; } else { x = y = z = 0; w = 1; }
    const xx = x * x, yy = y * y, zz = z * z, xy = x * y, xz = x * z, yz = y * z, wx = w * x, wy = w * y, wz = w * z;
    const m = new Float64Array(16);
    m[0] = (1 - 2 * (yy + zz)) * s[0]; m[1] = 2 * (xy + wz) * s[0]; m[2] = 2 * (xz - wy) * s[0];
    m[4] = 2 * (xy - wz) * s[1]; m[5] = (1 - 2 * (xx + zz)) * s[1]; m[6] = 2 * (yz + wx) * s[1];
    m[8] = 2 * (xz + wy) * s[2]; m[9] = 2 * (yz - wx) * s[2]; m[10] = (1 - 2 * (xx + yy)) * s[2];
    m[12] = t[0]; m[13] = t[1]; m[14] = t[2]; m[15] = 1;
    return m;
  }
  /** A node's own matrix: its `matrix`, else its translation, rotation and scale. */
  function nodeMatrix(node) {
    if (node.matrix !== undefined) {
      if (!Array.isArray(node.matrix) || node.matrix.length !== 16 || !node.matrix.every((v) => typeof v === 'number' && Number.isFinite(v))) {
        throw unreadable('a node has a broken matrix');
      }
      return Float64Array.from(node.matrix);
    }
    for (const [k, n] of [['translation', 3], ['rotation', 4], ['scale', 3]]) {
      if (node[k] !== undefined && numbers(node[k], n, []).length !== n) throw unreadable('a node has a broken ' + k);
    }
    return compose(numbers(node.translation, 3, [0, 0, 0]), numbers(node.rotation, 4, [0, 0, 0, 1]), numbers(node.scale, 3, [1, 1, 1]));
  }
  /**
   * The matrix that turns a node's normals (the inverse transpose of its upper 3x3, written as the
   * cofactors over the determinant) and that determinant, whose sign says whether it mirrors.
   */
  function normalMatrix(m) {
    const a00 = m[0], a10 = m[1], a20 = m[2], a01 = m[4], a11 = m[5], a21 = m[6], a02 = m[8], a12 = m[9], a22 = m[10];
    const c00 = a11 * a22 - a12 * a21, c01 = -(a10 * a22 - a12 * a20), c02 = a10 * a21 - a11 * a20;
    const c10 = -(a01 * a22 - a02 * a21), c11 = a00 * a22 - a02 * a20, c12 = -(a00 * a21 - a01 * a20);
    const c20 = a01 * a12 - a02 * a11, c21 = -(a00 * a12 - a02 * a10), c22 = a00 * a11 - a01 * a10;
    const det = a00 * c00 + a01 * c01 + a02 * c02;
    // Row-major: n' = N n.
    const N = [c00, c01, c02, c10, c11, c12, c20, c21, c22].map((v) => v / det);
    return { N, det };
  }

  // ── glTF (.glb) ─────────────────────────────────────────────────────────

  const GLB_MAGIC = 0x46546c67; // "glTF"
  const CHUNK_JSON = 0x4e4f534a;
  const CHUNK_BIN = 0x004e4942;
  const COMPONENT_BYTES = { 5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4 };
  const TYPE_COMPONENTS = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT2: 4, MAT3: 9, MAT4: 16 };
  const FLOAT = 5126;
  const TRIANGLES = 4;
  // A texture's own filtering and wrapping (glTF samplers; the numbers are WebGL's own constants).
  const NEAREST = 9728;
  const LINEAR = 9729;
  const LINEAR_MIPMAP_LINEAR = 9987;
  const REPEAT = 10497;
  const MAG_FILTERS = [NEAREST, LINEAR];
  const MIN_FILTERS = [NEAREST, LINEAR, 9984, 9985, 9986, LINEAR_MIPMAP_LINEAR];
  const WRAPS = [33071, 33648, REPEAT]; // clamp to edge, mirrored repeat, repeat
  /** What a texture without a sampler (or a sampler field left out) uses. */
  const DEFAULT_SAMPLER = Object.freeze({ magFilter: LINEAR, minFilter: LINEAR_MIPMAP_LINEAR, wrapS: REPEAT, wrapT: REPEAT });

  /** A binary glTF 2.0 file: its scene's meshes on their nodes, their materials and pictures. */
  function parseGlb(input) {
    const bytes = asBytes(input);
    if (bytes.length > MAX_MODEL_BYTES) throw tooLarge(bytes.length);
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (bytes.length < 12 || dv.getUint32(0, true) !== GLB_MAGIC) throw unreadable('it is not a binary glTF (.glb) file');
    const version = dv.getUint32(4, true);
    if (version !== 2) throw unreadable('it is glTF version ' + version + ', and the viewer reads version 2');
    const total = dv.getUint32(8, true);
    if (total > bytes.length) throw unreadable('it is shorter than its header says, so part of it is missing');
    let at = 12;
    let json = null;
    let bin = null;
    while (at + 8 <= total) {
      const len = dv.getUint32(at, true);
      const type = dv.getUint32(at + 4, true);
      const start = at + 8;
      if (start + len > total) throw unreadable('a part of it runs past its end, so part of it is missing');
      if (at === 12) {
        if (type !== CHUNK_JSON) throw unreadable('its first part is not its JSON');
        json = bytes.subarray(start, start + len);
      } else if (type === CHUNK_BIN && !bin) {
        bin = bytes.subarray(start, start + len);
      }
      at = start + len;
    }
    if (!json) throw unreadable('it has no JSON part');
    let doc = null;
    try {
      doc = JSON.parse(new TextDecoder('utf-8').decode(json));
    } catch (e) {
      throw unreadable('its JSON part is not valid JSON');
    }
    if (!doc || typeof doc !== 'object' || Array.isArray(doc)) throw unreadable('its JSON part is not a glTF description');
    return readGltf(doc, bin);
  }

  /** A glTF description `doc` whose file's binary part is `bin` (null when it has none). */
  function readGltf(doc, bin) {
    const required = list(doc.extensionsRequired);
    if (required.includes('KHR_draco_mesh_compression')) throw dracoError();
    if (required.includes('EXT_meshopt_compression')) throw unreadable('it is compressed with meshopt, which the viewer cannot read yet');
    if (doc.asset && typeof doc.asset.version === 'string' && !/^2(\.|$)/.test(doc.asset.version)) {
      throw unreadable('it is glTF ' + doc.asset.version.slice(0, 12) + ', and the viewer reads glTF 2.0');
    }
    const nodes = list(doc.nodes);
    const meshes = list(doc.meshes);
    const accessors = list(doc.accessors);
    const views = list(doc.bufferViews);
    const buffers = list(doc.buffers);
    const scenes = list(doc.scenes);
    const gltfMaterials = list(doc.materials);
    const textures = list(doc.textures);
    const gltfImages = list(doc.images);
    const gltfSamplers = list(doc.samplers);

    // The buffers: the file's own binary part, or data written into the file itself.
    const bufferCache = new Map();
    function buffer(i) {
      if (!isIndex(i, buffers.length)) throw unreadable('a buffer view points to a buffer that is not there');
      if (bufferCache.has(i)) return bufferCache.get(i);
      const b = buffers[i] || {};
      let data = null;
      if (b.uri === undefined) {
        if (!bin) throw unreadable('its binary part is missing');
        data = bin;
      } else {
        const d = dataUri(b.uri);
        if (!d) throw unreadable('it points to outside files, which a .glb should not');
        data = d.bytes;
      }
      if (!isCount(b.byteLength)) throw unreadable('a buffer does not say how long it is');
      if (data.length < b.byteLength) throw unreadable('a buffer is shorter than it says');
      bufferCache.set(i, data);
      return data;
    }
    /** A buffer view's bytes (inside its buffer, checked) and its stride (0: packed). */
    function bufferView(i) {
      if (!isIndex(i, views.length)) throw unreadable('an accessor points to a buffer view that is not there');
      const v = views[i] || {};
      const data = buffer(v.buffer);
      const off = v.byteOffset === undefined ? 0 : v.byteOffset;
      if (!isCount(off) || !isCount(v.byteLength)) throw unreadable('a buffer view has a broken offset or length');
      if (off + v.byteLength > data.length) throw unreadable('a buffer view runs past its buffer');
      if (v.byteStride !== undefined && !(isCount(v.byteStride) && v.byteStride > 0)) throw unreadable('a buffer view has a broken stride');
      return { bytes: data.subarray(off, off + v.byteLength), stride: v.byteStride || 0 };
    }
    /**
     * Accessor `i`, checked: its elements lie inside its buffer view and the view inside its
     * buffer. `want` ({componentType, type, words}) refuses anything else with those words.
     */
    function accessor(i, want) {
      if (!isIndex(i, accessors.length)) throw unreadable('a mesh points to an accessor that is not there');
      const a = accessors[i] || {};
      if (a.sparse !== undefined) throw unreadable('it uses sparse accessors, which the viewer cannot read yet');
      if (want && (a.componentType !== want.componentType || a.type !== want.type)) throw unreadable(want.words);
      const comps = TYPE_COMPONENTS[a.type];
      const size = COMPONENT_BYTES[a.componentType];
      if (!comps || !size) throw unreadable('an accessor has a type the viewer does not know');
      if (!isCount(a.count)) throw unreadable('an accessor has a broken count');
      if (a.bufferView === undefined) throw unreadable('an accessor has no data');
      const view = bufferView(a.bufferView);
      const off = a.byteOffset === undefined ? 0 : a.byteOffset;
      if (!isCount(off)) throw unreadable('an accessor has a broken offset');
      const elem = size * comps;
      const stride = view.stride || elem;
      if (stride < elem) throw unreadable("a buffer view's elements overlap");
      if (a.count > 0 && off + (a.count - 1) * stride + elem > view.bytes.length) throw unreadable('an accessor runs past its buffer view');
      const dv = new DataView(view.bytes.buffer, view.bytes.byteOffset, view.bytes.byteLength);
      return { count: a.count, comps, size, stride, off, dv, type: a.type, componentType: a.componentType };
    }
    /** Element `k`, component `j` of an accessor, as a number (whole numbers as they are). */
    function value(acc, k, j) {
      const o = acc.off + k * acc.stride + j * acc.size;
      switch (acc.componentType) {
        case 5126: return acc.dv.getFloat32(o, true);
        case 5121: return acc.dv.getUint8(o);
        case 5123: return acc.dv.getUint16(o, true);
        case 5125: return acc.dv.getUint32(o, true);
        case 5120: return acc.dv.getInt8(o);
        default: return acc.dv.getInt16(o, true);
      }
    }
    /** `f` without throwing: its answer, or null. */
    const tryOr = (f) => { try { return f(); } catch (e) { return null; } };

    // The scene: the one named, else the first, else every root node.
    let roots;
    if (doc.scene !== undefined) {
      if (!isIndex(doc.scene, scenes.length)) throw unreadable('its scene number points past its scenes');
      roots = list((scenes[doc.scene] || {}).nodes);
    } else if (scenes.length) {
      roots = list((scenes[0] || {}).nodes);
    } else {
      const children = new Set();
      for (const n of nodes) for (const c of list(n && n.children)) children.add(c);
      roots = nodes.map((n, i) => i).filter((i) => !children.has(i));
    }

    // Every mesh instance with its world matrix (each node's own matrix through its parents).
    const instances = [];
    let visits = 0;
    function walk(i, parent, depth) {
      if (depth > MAX_NODE_DEPTH) throw unreadable('its nodes nest too deep (or loop back on themselves)');
      if (++visits > MAX_NODE_VISITS) throw unreadable('its nodes repeat each other too many times');
      if (!isIndex(i, nodes.length)) throw unreadable('a scene points to a node that is not there');
      const node = nodes[i] || {};
      const world = multiply(parent, nodeMatrix(node));
      if (node.mesh !== undefined) {
        if (!isIndex(node.mesh, meshes.length)) throw unreadable('a node points to a mesh that is not there');
        instances.push([meshes[node.mesh] || {}, world]);
      }
      for (const c of list(node.children)) walk(c, world, depth + 1);
    }
    for (const r of roots) walk(r, identity(), 0);

    // The triangles and vertices they come to, before anything is read.
    let triangles = 0;
    let vertices = 0;
    for (const [mesh] of instances) {
      for (const p of list(mesh.primitives)) {
        if (!p || (p.mode !== undefined && p.mode !== TRIANGLES)) continue; // points, lines and strips are not drawn
        const attrs = p.attributes || {};
        const posAt = attrs.POSITION;
        const pos = isIndex(posAt, accessors.length) ? accessors[posAt] || {} : null;
        // A Draco mesh whose positions are only in its compressed copy, extension merely "used".
        if (p.extensions && p.extensions.KHR_draco_mesh_compression && pos && pos.bufferView === undefined) throw dracoError();
        const posCount = pos && isCount(pos.count) ? pos.count : 0;
        let n = Math.floor(posCount / 3);
        if (p.indices !== undefined) {
          if (!isIndex(p.indices, accessors.length)) throw unreadable('a mesh points to an accessor that is not there');
          const ic = (accessors[p.indices] || {}).count;
          n = isCount(ic) ? Math.floor(ic / 3) : 0;
        }
        triangles += n;
        vertices += posCount;
      }
    }
    if (triangles > MAX_TRIANGLES) throw tooDetailed(triangles);
    if (vertices > MAX_VERTICES) throw tooManyVertices(vertices);

    const positions = new Float32Array(vertices * 3);
    const normals = new Float32Array(vertices * 3);
    const uvs = new Float32Array(vertices * 2);
    const indices = new Uint32Array(triangles * 3);
    const draws = [];
    const materials = [];
    const images = [];
    const materialSlots = new Map(); // glTF material index (or 'default') -> index in `materials`
    const imageSlots = new Map(); // glTF image index -> index in `images` (null: drawn without it)
    let vAt = 0; // vertices written
    let iAt = 0; // indices written

    /**
     * The texture `info` names: {image (its slot in `images`), sampler (the file's own filtering and
     * wrapping for it)}, or null to draw without it.
     */
    function textureSlot(info) {
      if (!info || typeof info !== 'object') return null;
      if (!isIndex(info.index, textures.length)) throw unreadable('a material points to a texture that is not there');
      const tex = textures[info.index] || {};
      const sampler = samplerOf(tex.sampler);
      const image = imageSlot(tex.source);
      return image === null ? null : { image, sampler };
    }
    /** Sampler `i` of the file, each field checked; glTF's usual defaults for a missing one (or none). */
    function samplerOf(i) {
      if (i !== undefined && !isIndex(i, gltfSamplers.length)) throw unreadable('a texture points to a sampler that is not there');
      const s = i === undefined ? {} : gltfSamplers[i] || {};
      const pick = (v, allowed, fallback) => (allowed.includes(v) ? v : fallback);
      return {
        magFilter: pick(s.magFilter, MAG_FILTERS, LINEAR),
        minFilter: pick(s.minFilter, MIN_FILTERS, LINEAR_MIPMAP_LINEAR),
        wrapS: pick(s.wrapS, WRAPS, REPEAT),
        wrapT: pick(s.wrapT, WRAPS, REPEAT),
      };
    }
    /** Picture `source` of the file: its slot in `images`, or null to draw without it. */
    function imageSlot(source) {
      if (source === undefined) return null;
      if (!isIndex(source, gltfImages.length)) throw unreadable('a texture points to a picture that is not there');
      if (imageSlots.has(source)) return imageSlots.get(source);
      const img = gltfImages[source] || {};
      let bytes = null;
      let mime = typeof img.mimeType === 'string' ? img.mimeType : '';
      if (img.bufferView !== undefined) {
        const v = tryOr(() => bufferView(img.bufferView));
        if (!v) throw unreadable('a picture runs past its buffer');
        bytes = v.bytes.slice();
      } else {
        const d = dataUri(img.uri);
        if (d) { bytes = d.bytes; mime = mime || d.mime; }
        // A picture outside the file: drawn without it (the desktop does the same).
      }
      let slot = null;
      if (bytes) {
        images.push({ mimeType: sniff(bytes) || mime, bytes });
        slot = images.length - 1;
      }
      imageSlots.set(source, slot);
      return slot;
    }
    /** The viewer's material for glTF material `index` (undefined: glTF's default material). */
    function materialSlot(index) {
      const key = index === undefined ? 'default' : index;
      if (materialSlots.has(key)) return materialSlots.get(key);
      if (index !== undefined && !isIndex(index, gltfMaterials.length)) throw unreadable('a mesh points to a material that is not there');
      const m = index === undefined ? {} : gltfMaterials[index] || {};
      const pbr = m.pbrMetallicRoughness || {};
      const normal = m.normalTexture && typeof m.normalTexture === 'object' ? m.normalTexture : null;
      const occlusion = m.occlusionTexture && typeof m.occlusionTexture === 'object' ? m.occlusionTexture : null;
      const alphaMode = m.alphaMode === 'MASK' || m.alphaMode === 'BLEND' ? m.alphaMode : 'OPAQUE';
      materials.push({
        baseColor: numbers(pbr.baseColorFactor, 4, [1, 1, 1, 1]),
        baseTexture: textureSlot(pbr.baseColorTexture),
        metallic: finite(pbr.metallicFactor, 1),
        roughness: finite(pbr.roughnessFactor, 1),
        metallicRoughnessTexture: textureSlot(pbr.metallicRoughnessTexture),
        normalTexture: textureSlot(normal),
        normalScale: finite(normal && normal.scale, 1),
        occlusionTexture: textureSlot(occlusion),
        occlusionStrength: finite(occlusion && occlusion.strength, 1),
        emissive: numbers(m.emissiveFactor, 3, [0, 0, 0]),
        emissiveTexture: textureSlot(m.emissiveTexture),
        alphaMode,
        alphaCutoff: finite(m.alphaCutoff, 0.5),
        doubleSided: m.doubleSided === true,
      });
      materialSlots.set(key, materials.length - 1);
      return materials.length - 1;
    }

    for (const [mesh, world] of instances) {
      const { N, det } = normalMatrix(world);
      const flips = det < 0;
      for (const p of list(mesh.primitives)) {
        if (!p || (p.mode !== undefined && p.mode !== TRIANGLES)) continue;
        const attrs = p.attributes || {};
        if (attrs.POSITION === undefined) throw unreadable('a mesh has no positions');
        const pos = accessor(attrs.POSITION, { componentType: FLOAT, type: 'VEC3', words: 'its positions are stored compressed, which the viewer cannot read yet' });
        const count = pos.count;
        // Normals and texture coordinates the viewer cannot use are worked out or left at zero.
        const nor = attrs.NORMAL === undefined ? null : tryOr(() => accessor(attrs.NORMAL, { componentType: FLOAT, type: 'VEC3', words: '' }));
        const normalsOk = !!nor && nor.count === count;
        const uv = attrs.TEXCOORD_0 === undefined ? null : tryOr(() => accessor(attrs.TEXCOORD_0));
        const uvsOk = !!uv && uv.type === 'VEC2' && (uv.componentType === 5121 || uv.componentType === 5123 || uv.componentType === FLOAT) && uv.count === count;
        let idx = null;
        if (p.indices !== undefined) {
          idx = accessor(p.indices);
          if (idx.type !== 'SCALAR' || !(idx.componentType === 5121 || idx.componentType === 5123 || idx.componentType === 5125)) {
            throw unreadable('its indices are not whole numbers');
          }
        }
        const base = vAt;
        // Positions, through the node's matrix.
        for (let k = 0; k < count; k++) {
          const x = value(pos, k, 0), y = value(pos, k, 1), z = value(pos, k, 2);
          const wx = world[0] * x + world[4] * y + world[8] * z + world[12];
          const wy = world[1] * x + world[5] * y + world[9] * z + world[13];
          const wz = world[2] * x + world[6] * y + world[10] * z + world[14];
          if (!Number.isFinite(wx) || !Number.isFinite(wy) || !Number.isFinite(wz)) throw unreadable('a position is not a number');
          const o = (base + k) * 3;
          positions[o] = wx; positions[o + 1] = wy; positions[o + 2] = wz;
        }
        // Indices: every one checked against this mesh's vertices, whole triangles only.
        const first = iAt;
        const n = idx ? idx.count - (idx.count % 3) : count - (count % 3);
        for (let k = 0; k < n; k++) {
          const i = idx ? value(idx, k, 0) : k;
          if (i >= count) throw unreadable('an index points past the vertices');
          indices[iAt++] = base + i;
        }
        if (flips) {
          // A mirrored node turns its triangles inside out; turn them back.
          for (let t = first; t < iAt; t += 3) { const s = indices[t + 1]; indices[t + 1] = indices[t + 2]; indices[t + 2] = s; }
        }
        if (normalsOk) {
          for (let k = 0; k < count; k++) {
            const x = value(nor, k, 0), y = value(nor, k, 1), z = value(nor, k, 2);
            let nx = N[0] * x + N[1] * y + N[2] * z;
            let ny = N[3] * x + N[4] * y + N[5] * z;
            let nz = N[6] * x + N[7] * y + N[8] * z;
            const len = Math.hypot(nx, ny, nz);
            if (len > 0 && Number.isFinite(len)) { nx /= len; ny /= len; nz /= len; } else { nx = 0; ny = 1; nz = 0; }
            const o = (base + k) * 3;
            normals[o] = nx; normals[o + 1] = ny; normals[o + 2] = nz;
          }
        } else {
          smoothNormals(positions, normals, indices, first, iAt, base, count);
        }
        if (uvsOk) {
          // Bytes and shorts are read as fractions of their largest value (glTF's normalized).
          const scale = uv.componentType === 5121 ? 1 / 255 : uv.componentType === 5123 ? 1 / 65535 : 1;
          for (let k = 0; k < count; k++) {
            uvs[(base + k) * 2] = value(uv, k, 0) * scale;
            uvs[(base + k) * 2 + 1] = value(uv, k, 1) * scale;
          }
        }
        vAt += count;
        draws.push({ first, count: iAt - first, material: materialSlot(p.material) });
      }
    }
    if (iAt === 0) throw unreadable('it holds no triangles');
    return finish({ positions, normals, uvs, indices, draws, materials, images });
  }

  /**
   * Smooth normals for the vertices `base..base+count` from the triangles `indices[from..to]`
   * (their faces' normals, summed per vertex, so a big face counts for more).
   */
  function smoothNormals(positions, normals, indices, from, to, base, count) {
    const sum = new Float64Array(count * 3);
    for (let t = from; t + 2 < to; t += 3) {
      const a = indices[t] * 3, b = indices[t + 1] * 3, c = indices[t + 2] * 3;
      const ux = positions[b] - positions[a], uy = positions[b + 1] - positions[a + 1], uz = positions[b + 2] - positions[a + 2];
      const vx = positions[c] - positions[a], vy = positions[c + 1] - positions[a + 1], vz = positions[c + 2] - positions[a + 2];
      const nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
      for (const v of [indices[t], indices[t + 1], indices[t + 2]]) {
        const o = (v - base) * 3;
        sum[o] += nx; sum[o + 1] += ny; sum[o + 2] += nz;
      }
    }
    for (let k = 0; k < count; k++) {
      const x = sum[k * 3], y = sum[k * 3 + 1], z = sum[k * 3 + 2];
      const len = Math.hypot(x, y, z);
      const o = (base + k) * 3;
      if (len > 0 && Number.isFinite(len)) { normals[o] = x / len; normals[o + 1] = y / len; normals[o + 2] = z / len; } else { normals[o] = 0; normals[o + 1] = 1; normals[o + 2] = 0; }
    }
  }

  /** The bounds and the triangle count, and the limit checked once more. */
  function finish(m) {
    const triangles = m.indices.length / 3;
    if (triangles > MAX_TRIANGLES) throw tooDetailed(triangles);
    const min = [Infinity, Infinity, Infinity];
    const max = [-Infinity, -Infinity, -Infinity];
    const p = m.positions;
    for (let i = 0; i < p.length; i += 3) {
      for (let j = 0; j < 3; j++) {
        if (p[i + j] < min[j]) min[j] = p[i + j];
        if (p[i + j] > max[j]) max[j] = p[i + j];
      }
    }
    // The bounding sphere: the box's centre, and the farthest vertex from it.
    const centre = [0, 1, 2].map((j) => (min[j] + max[j]) / 2);
    let r2 = 0;
    for (let i = 0; i < p.length; i += 3) {
      const dx = p[i] - centre[0], dy = p[i + 1] - centre[1], dz = p[i + 2] - centre[2];
      const d = dx * dx + dy * dy + dz * dz;
      if (d > r2) r2 = d;
    }
    return Object.assign(m, { bounds: { min, max, centre, radius: Math.max(Math.sqrt(r2), 1e-6) }, triangles });
  }

  // ── STL ─────────────────────────────────────────────────────────────────

  /** STL's one plain material (the spec's grey, both sides drawn). */
  function plainMaterial() {
    return {
      baseColor: [0.72, 0.74, 0.78, 1], baseTexture: null, metallic: 0, roughness: 0.55,
      metallicRoughnessTexture: null, normalTexture: null, normalScale: 1, occlusionTexture: null, occlusionStrength: 1,
      emissive: [0, 0, 0], emissiveTexture: null, alphaMode: 'OPAQUE', alphaCutoff: 0.5, doubleSided: true,
    };
  }

  /**
   * An STL file, binary or ASCII, turned from STL's z-up to y-up (x stays, y = z, z = -y), its
   * normals worked out from each triangle's corners (a file's own are often zero), one plain material.
   */
  function parseStl(input) {
    const bytes = asBytes(input);
    if (bytes.length > MAX_MODEL_BYTES) throw tooLarge(bytes.length);
    const corners = stlBinary(bytes) || stlAscii(bytes);
    const n = corners.length / 9;
    if (n === 0) throw unreadable('it holds no triangles');
    const positions = new Float32Array(n * 9);
    const normals = new Float32Array(n * 9);
    const indices = new Uint32Array(n * 3);
    for (let t = 0; t < n; t++) {
      const p = [];
      for (let k = 0; k < 3; k++) {
        const o = t * 9 + k * 3;
        p.push([corners[o], corners[o + 2], -corners[o + 1]]);
      }
      const ux = p[1][0] - p[0][0], uy = p[1][1] - p[0][1], uz = p[1][2] - p[0][2];
      const vx = p[2][0] - p[0][0], vy = p[2][1] - p[0][1], vz = p[2][2] - p[0][2];
      let nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
      const len = Math.hypot(nx, ny, nz);
      if (len > 0 && Number.isFinite(len)) { nx /= len; ny /= len; nz /= len; } else { nx = 0; ny = 1; nz = 0; }
      for (let k = 0; k < 3; k++) {
        const o = t * 9 + k * 3;
        positions[o] = p[k][0]; positions[o + 1] = p[k][1]; positions[o + 2] = p[k][2];
        normals[o] = nx; normals[o + 1] = ny; normals[o + 2] = nz;
        indices[t * 3 + k] = t * 3 + k;
      }
    }
    return finish({
      positions, normals, uvs: new Float32Array(n * 6), indices,
      draws: [{ first: 0, count: n * 3, material: 0 }], materials: [plainMaterial()], images: [],
    });
  }

  /** Whether `bytes` starts with "solid" once spaces, tabs and line ends are skipped. */
  function startsSolid(bytes) {
    let i = 0;
    while (i < bytes.length && (bytes[i] === 32 || (bytes[i] >= 9 && bytes[i] <= 13))) i++;
    return bytes.length - i >= 5 && String.fromCharCode(bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3], bytes[i + 4]) === 'solid';
  }

  /** A binary STL's corners (9 numbers a triangle), or null when `bytes` is not one (then it is read as ASCII). */
  function stlBinary(bytes) {
    if (bytes.length < 84) return null;
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const count = dv.getUint32(80, true);
    const need = 84 + count * 50;
    // An ASCII file starts "solid"; a binary one may too (its header is free text), so a size that
    // matches its count exactly decides.
    if (!(need === bytes.length || (need < bytes.length && !startsSolid(bytes)))) return null;
    if (count > MAX_TRIANGLES) throw tooDetailed(count);
    const out = new Float32Array(count * 9);
    for (let t = 0; t < count; t++) {
      const o = 84 + t * 50 + 12; // past the stored normal
      for (let j = 0; j < 9; j++) {
        const v = dv.getFloat32(o + j * 4, true);
        if (!Number.isFinite(v)) throw unreadable('a corner is not a number');
        out[t * 9 + j] = v;
      }
    }
    return out;
  }

  const NUMBER_WORD = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;
  /** An ASCII STL's corners: every `vertex x y z`, three to a triangle. */
  function stlAscii(bytes) {
    let text = '';
    try {
      text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    } catch (e) {
      throw unreadable('it is neither a binary nor a text STL');
    }
    if (!text.trimStart().startsWith('solid')) throw unreadable('it is neither a binary nor a text STL');
    const out = [];
    const word = /\S+/g;
    let m;
    while ((m = word.exec(text))) {
      if (m[0] !== 'vertex') continue;
      for (let j = 0; j < 3; j++) {
        const w = word.exec(text);
        const v = w && NUMBER_WORD.test(w[0]) ? Number(w[0]) : NaN;
        if (!Number.isFinite(v)) throw unreadable('a vertex is not three numbers');
        out.push(v);
      }
      // As the desktop counts: whole triangles so far, stopped at the first one over the limit.
      if (Math.floor(out.length / 9) > MAX_TRIANGLES) throw tooDetailed(Math.floor(out.length / 9));
    }
    if (out.length % 9 !== 0) throw unreadable('its vertices do not come in threes');
    return out;
  }

  /** Read `bytes`, named `name` (`.stl` is read as STL, anything else as glTF). */
  function parseModel(name, input) {
    const bytes = asBytes(input);
    if (bytes.length > MAX_MODEL_BYTES) throw tooLarge(bytes.length);
    return /\.stl$/i.test(String(name || '').trim()) ? parseStl(bytes) : parseGlb(bytes);
  }

  /**
   * A fetch answer's bytes, never more than `max` (200 MB): refused before reading when its
   * length says so, and again while reading.
   */
  async function readBody(res, max) {
    const limit = max || MAX_MODEL_BYTES;
    let declared = NaN;
    try { if (res.headers && typeof res.headers.get === 'function') declared = Number(res.headers.get('content-length')); } catch (e) { declared = NaN; }
    if (Number.isFinite(declared) && declared > limit) throw tooLarge(declared);
    if (res.body && typeof res.body.getReader === 'function') {
      const reader = res.body.getReader();
      const parts = [];
      let total = 0;
      for (;;) {
        const { done, value: chunk } = await reader.read();
        if (done) break;
        total += chunk.byteLength;
        if (total > limit) {
          try { reader.cancel(); } catch (e) { /* stopping it is all that is wanted */ }
          throw tooLarge(total);
        }
        parts.push(chunk);
      }
      const out = new Uint8Array(total);
      let at = 0;
      for (const p of parts) { out.set(p, at); at += p.byteLength; }
      return out.buffer;
    }
    const buf = typeof res.arrayBuffer === 'function' ? await res.arrayBuffer() : await (await res.blob()).arrayBuffer();
    if (buf.byteLength > limit) throw tooLarge(buf.byteLength);
    return buf;
  }

  // ── The camera (camera.rs, exactly) ─────────────────────────────────────

  /** The camera fitted to `bounds`: the whole model in view, from a little above and to the side. */
  function fitCamera(bounds) {
    const radius = Math.max(Number(bounds.radius) || 0, 1e-6);
    const distance = radius / Math.sin(FOV_Y / 2) * 1.15;
    const target = bounds.centre.slice(0, 3);
    return { yaw: START_YAW, pitch: START_PITCH, distance, target, radius, fitTarget: target.slice(), fitDistance: distance };
  }
  /** Back to the fit. */
  function resetCamera(cam) {
    return Object.assign({}, cam, { yaw: START_YAW, pitch: START_PITCH, distance: cam.fitDistance, target: cam.fitTarget.slice() });
  }
  /** Turned by a drag of `dx`, `dy` points. */
  function orbitCamera(cam, dx, dy) {
    return Object.assign({}, cam, {
      yaw: cam.yaw - dx * TURN_PER_POINT,
      pitch: Math.min(MAX_PITCH, Math.max(-MAX_PITCH, cam.pitch + dy * TURN_PER_POINT)),
    });
  }
  /** The camera's position. */
  function cameraEye(cam) {
    const cp = Math.cos(cam.pitch);
    return [
      cam.target[0] + cam.distance * cp * Math.sin(cam.yaw),
      cam.target[1] + cam.distance * Math.sin(cam.pitch),
      cam.target[2] + cam.distance * cp * Math.cos(cam.yaw),
    ];
  }
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  function normalizeOr(v, fallback) {
    const len = Math.hypot(v[0], v[1], v[2]);
    return len > 0 && Number.isFinite(len) ? [v[0] / len, v[1] / len, v[2] / len] : fallback.slice();
  }
  /** The camera's forward, right and up directions in the world. */
  function cameraBasis(cam) {
    const forward = normalizeOr(sub(cam.target, cameraEye(cam)), [0, 0, -1]);
    const right = normalizeOr(cross(forward, [0, 1, 0]), [1, 0, 0]);
    return { forward, right, up: cross(right, forward) };
  }
  /** Moved by a drag of `dx`, `dy` points on a view `viewHeight` points tall: the model follows the pointer. */
  function panCamera(cam, dx, dy, viewHeight) {
    const perPoint = 2 * cam.distance * Math.tan(FOV_Y / 2) / Math.max(viewHeight, 1);
    const { right, up } = cameraBasis(cam);
    const t = cam.target;
    return Object.assign({}, cam, {
      target: [0, 1, 2].map((j) => t[j] + (-dx * right[j] + dy * up[j]) * perPoint),
    });
  }
  /** Zoomed by `points` scrolled (positive: closer), held to [0.05 r, 50 r]. */
  function zoomCamera(cam, points) {
    const d = cam.distance * Math.exp(-points * ZOOM_PER_POINT);
    return Object.assign({}, cam, { distance: Math.min(50 * cam.radius, Math.max(0.05 * cam.radius, d)) });
  }
  /** The clipping planes: close enough that zooming in never cuts the model, far enough to hold all of it. */
  function nearFar(cam) {
    const r = cam.radius;
    return [Math.max(cam.distance - 2 * r, cam.distance * 0.001), cam.distance + 2 * r];
  }
  /** The view matrix (right-handed, looking at the target, up +y), column-major. */
  function viewMatrix(cam) {
    const eye = cameraEye(cam);
    const { forward: f, right: s, up: u } = cameraBasis(cam);
    const m = new Float64Array(16);
    m[0] = s[0]; m[4] = s[1]; m[8] = s[2];
    m[1] = u[0]; m[5] = u[1]; m[9] = u[2];
    m[2] = -f[0]; m[6] = -f[1]; m[10] = -f[2];
    m[12] = -dot(s, eye); m[13] = -dot(u, eye); m[14] = dot(f, eye); m[15] = 1;
    return m;
  }
  /** The projection for a view `aspect` wide to tall (WebGL's -1..1 depth), column-major. */
  function projectionMatrix(cam, aspect) {
    const [near, far] = nearFar(cam);
    const f = 1 / Math.tan(FOV_Y / 2);
    const m = new Float64Array(16);
    m[0] = f / Math.max(aspect, 1e-3);
    m[5] = f;
    m[10] = (far + near) / (near - far);
    m[11] = -1;
    m[14] = 2 * far * near / (near - far);
    return m;
  }

  // ── Drawing (WebGL 2; the shading is model_view.wgsl's) ─────────────────

  const VERTEX_SHADER = `#version 300 es
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNormal;
layout(location = 2) in vec2 aUv;
uniform mat4 uViewProj;
out vec3 vWorld;
out vec3 vNormal;
out vec2 vUv;
void main() {
  vWorld = aPos;
  vNormal = aNormal;
  vUv = aUv;
  gl_Position = uViewProj * vec4(aPos, 1.0);
}`;

  // glTF's metallic-roughness model (GGX, Smith-Schlick, Schlick Fresnel), the three studio lights,
  // the sky-and-ground hemisphere, exposure 1 and the ACES curve, written as sRGB (the canvas is
  // not an sRGB target, so the shader encodes; the desktop's target encodes for it).
  const FRAGMENT_SHADER = `#version 300 es
precision highp float;
in vec3 vWorld;
in vec3 vNormal;
in vec2 vUv;
uniform vec3 uEye;
uniform vec4 uLightDir[3];   // towards each light, in the world (xyz), intensity (w)
uniform vec3 uLightColor[3];
uniform vec4 uBase;
uniform vec3 uEmissive;
uniform vec4 uParams;        // metallic, roughness, alpha cutoff, alpha mode (0 opaque, 1 mask, 2 blend)
uniform vec4 uExtra;         // normal scale, occlusion strength, has a normal texture
uniform sampler2D tBase;
uniform sampler2D tMR;
uniform sampler2D tNormal;
uniform sampler2D tOcc;
uniform sampler2D tEmissive;
out vec4 outColor;
const float PI = 3.14159265;
float ggx(float nh, float a) {
  float a2 = a * a;
  float d = nh * nh * (a2 - 1.0) + 1.0;
  return a2 / (PI * d * d);
}
float smith(float nv, float nl, float rough) {
  float k = (rough + 1.0) * (rough + 1.0) / 8.0;
  return (nv / (nv * (1.0 - k) + k)) * (nl / (nl * (1.0 - k) + k));
}
vec3 aces(vec3 x) {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}
vec3 toSrgb(vec3 c) {
  return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}
void main() {
  // Every texture read and derivative first, while all the pixel's neighbours still run.
  vec4 baseSample = texture(tBase, vUv);
  vec4 mr = texture(tMR, vUv);
  vec3 tn = texture(tNormal, vUv).xyz * 2.0 - 1.0;
  float occSample = texture(tOcc, vUv).r;
  vec3 emissiveSample = texture(tEmissive, vUv).rgb;
  vec3 dp1 = dFdx(vWorld);
  vec3 dp2 = dFdy(vWorld);
  vec2 duv1 = dFdx(vUv);
  vec2 duv2 = dFdy(vUv);

  vec4 base = uBase * baseSample;
  vec3 n = normalize(vNormal);
  if (!gl_FrontFacing) n = -n; // the back of a double-sided surface
  if (uExtra.z > 0.5) {
    // Tangent frame from screen-space derivatives (no tangent attribute needed).
    vec3 dp2perp = cross(dp2, n);
    vec3 dp1perp = cross(n, dp1);
    vec3 t = dp2perp * duv1.x + dp1perp * duv2.x;
    vec3 b = dp2perp * duv1.y + dp1perp * duv2.y;
    float inv = inversesqrt(max(max(dot(t, t), dot(b, b)), 1e-20));
    vec3 scaled = vec3(tn.xy * uExtra.x, tn.z);
    n = normalize(mat3(t * inv, b * inv, n) * scaled);
  }
  vec3 v = normalize(uEye - vWorld);
  float metallic = clamp(uParams.x * mr.b, 0.0, 1.0);
  float rough = clamp(uParams.y * mr.g, 0.04, 1.0);
  float a = rough * rough;
  vec3 f0 = mix(vec3(0.04), base.rgb, metallic);
  vec3 diffuse = base.rgb * (1.0 - metallic);
  float nv = max(dot(n, v), 1e-4);

  vec3 color = vec3(0.0);
  for (int i = 0; i < 3; i++) {
    vec3 l = normalize(uLightDir[i].xyz);
    vec3 h = normalize(l + v);
    float nl = max(dot(n, l), 0.0);
    if (nl <= 0.0) continue;
    vec3 f = f0 + (1.0 - f0) * pow(1.0 - max(dot(h, v), 0.0), 5.0);
    vec3 spec = ggx(max(dot(n, h), 0.0), a) * smith(nv, nl, rough) * f / max(4.0 * nv * nl, 1e-4);
    vec3 kd = (1.0 - f) * diffuse / PI;
    color += (kd + spec) * uLightColor[i] * uLightDir[i].w * nl;
  }
  float occlusion = mix(1.0, occSample, uExtra.y);
  vec3 hemi = mix(vec3(0.25, 0.22, 0.20), vec3(0.55, 0.60, 0.70), n.y * 0.5 + 0.5) * 0.6;
  color += hemi * (diffuse + f0 * 0.25) * occlusion;
  color += uEmissive * emissiveSample;
  color = aces(color);

  float alpha = base.a;
  if (uParams.w < 0.5) {
    alpha = 1.0;
  } else if (uParams.w < 1.5) {
    if (alpha < uParams.z) discard;
    alpha = 1.0;
  }
  outColor = vec4(toSrgb(color), alpha);
}`;

  /**
   * Decode a model's pictures (createImageBitmap), each scaled to at most `side` on a side:
   * [bitmap or canvas or null], or throws the viewer's words when one cannot be decoded.
   */
  async function decodeImages(images, side, doc) {
    if (!images.length) return [];
    if (typeof root.createImageBitmap !== 'function') throw unreadable('this browser cannot read the pictures in it');
    const raw = { premultiplyAlpha: 'none', colorSpaceConversion: 'none' };
    return Promise.all(images.map(async (img) => {
      if (!img) return null;
      let bmp = null;
      try {
        bmp = await root.createImageBitmap(new Blob([img.bytes], { type: img.mimeType || '' }), raw);
      } catch (e) {
        throw unreadable('a picture in it could not be decoded');
      }
      if (bmp.width <= side && bmp.height <= side) return bmp;
      const scale = side / Math.max(bmp.width, bmp.height);
      const w = Math.max(1, Math.round(bmp.width * scale));
      const h = Math.max(1, Math.round(bmp.height * scale));
      let small = null;
      try {
        small = await root.createImageBitmap(bmp, Object.assign({ resizeWidth: w, resizeHeight: h, resizeQuality: 'high' }, raw));
      } catch (e) {
        small = null;
      }
      if (!small || small.width > side || small.height > side) {
        // A browser that does not resize while decoding: drawn smaller on a canvas instead.
        if (small && small.close) small.close();
        const c = doc.createElement('canvas');
        c.width = w;
        c.height = h;
        c.getContext('2d').drawImage(bmp, 0, 0, w, h);
        small = c;
      }
      if (bmp.close) bmp.close();
      return small;
    }));
  }

  /** Parse a CSS colour ("#rgb", "#rrggbb", "rgb(r, g, b)") into [r, g, b] from 0 to 1, or null. */
  function cssColour(text) {
    const s = String(text || '').trim();
    let m = /^#([0-9a-f]{3,8})$/i.exec(s);
    if (m) {
      let hex = m[1];
      if (hex.length === 3 || hex.length === 4) hex = hex.split('').map((c) => c + c).join('');
      if (hex.length !== 6 && hex.length !== 8) return null;
      return [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
    }
    m = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)/i.exec(s);
    return m ? [m[1], m[2], m[3]].map((v) => Math.min(1, Number(v) / 255)) : null;
  }

  /**
   * The WebGL 2 drawer for `canvas`, or null when this browser has no WebGL 2. `prepare(model)`
   * decodes the pictures; `draw({cam, width, height, background})` draws one picture; `free()`
   * gives the GPU memory back (the next draw takes it again); `destroy()` gives everything back.
   */
  function createGlRenderer(canvas) {
    let gl = null;
    try {
      gl = canvas && typeof canvas.getContext === 'function'
        // alpha: true so the canvas's own picture is RGBA8 like the multisampled one, which a blit
        // needs (an alpha: false canvas is RGB and the browser refuses the copy); the shader and
        // the blending keep its alpha at 1, so nothing of the page shows through.
        ? canvas.getContext('webgl2', { antialias: false, alpha: true, depth: true, stencil: false, preserveDrawingBuffer: false })
        : null;
    } catch (e) {
      gl = null;
    }
    if (!gl) return null;
    let model = null;
    let bitmaps = [];
    let gpu = null;
    let lost = false;
    const r = { onRestored: null };
    const onLost = (e) => { e.preventDefault(); lost = true; gpu = null; };
    const onRestored = () => { lost = false; gpu = null; if (r.onRestored) r.onRestored(); };
    canvas.addEventListener('webglcontextlost', onLost);
    canvas.addEventListener('webglcontextrestored', onRestored);

    function compile(type, source) {
      const s = gl.createShader(type);
      gl.shaderSource(s, source);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
        const why = gl.getShaderInfoLog(s);
        gl.deleteShader(s);
        throw new Error('the 3D view could not start (' + why + ')');
      }
      return s;
    }
    function texture(width, height, pixels, internal) {
      const t = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
      gl.pixelStorei(gl.UNPACK_COLORSPACE_CONVERSION_WEBGL, gl.NONE);
      if (pixels && pixels.BYTES_PER_ELEMENT) gl.texImage2D(gl.TEXTURE_2D, 0, internal, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      else gl.texImage2D(gl.TEXTURE_2D, 0, internal, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      gl.generateMipmap(gl.TEXTURE_2D);
      return t;
    }
    /** Upload the model and make everything a picture needs. */
    function build() {
      const vs = compile(gl.VERTEX_SHADER, VERTEX_SHADER);
      const fs = compile(gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
      const prog = gl.createProgram();
      gl.attachShader(prog, vs);
      gl.attachShader(prog, fs);
      gl.linkProgram(prog);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error('the 3D view could not start (' + gl.getProgramInfoLog(prog) + ')');
      const u = (n) => gl.getUniformLocation(prog, n);
      const loc = {
        viewProj: u('uViewProj'), eye: u('uEye'), lightDir: u('uLightDir'), lightColor: u('uLightColor'),
        base: u('uBase'), emissive: u('uEmissive'), params: u('uParams'), extra: u('uExtra'),
      };
      gl.useProgram(prog);
      ['tBase', 'tMR', 'tNormal', 'tOcc', 'tEmissive'].forEach((n, i) => gl.uniform1i(u(n), i));

      // Positions are uploaded around the model's centre, so a model far from the origin keeps its
      // detail in the GPU's 32-bit numbers.
      const c = model.bounds.centre;
      const rel = new Float32Array(model.positions.length);
      for (let i = 0; i < rel.length; i += 3) {
        rel[i] = model.positions[i] - c[0];
        rel[i + 1] = model.positions[i + 1] - c[1];
        rel[i + 2] = model.positions[i + 2] - c[2];
      }
      const vao = gl.createVertexArray();
      gl.bindVertexArray(vao);
      const buffers = [[rel, 3], [model.normals, 3], [model.uvs, 2]].map(([data, size], i) => {
        const b = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, b);
        gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
        gl.enableVertexAttribArray(i);
        gl.vertexAttribPointer(i, size, gl.FLOAT, false, 0, 0);
        return b;
      });
      const ib = gl.createBuffer();
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, model.indices, gl.STATIC_DRAW);
      gl.bindVertexArray(null);

      const white = new Uint8Array([255, 255, 255, 255]);
      gpu = {
        prog, loc, vao, buffers, ib,
        whiteSrgb: texture(1, 1, white, gl.SRGB8_ALPHA8),
        white: texture(1, 1, white, gl.RGBA8),
        textures: new Map(), // "<image>:<srgb>" -> texture
        samplers: new Map(), // "<mag>:<min>:<wrapS>:<wrapT>" -> sampler
        fbo: null, color: null, depth: null, size: [0, 0],
        samples: Math.min(4, Number(gl.getParameter(gl.MAX_SAMPLES)) || 0),
      };
    }
    /** Picture `i` as a texture: sRGB for colours, linear for data; white when there is none. */
    /**
     * A texture's own filtering and wrapping as a WebGL sampler, made once for each different one
     * (glTF's numbers are WebGL's constants). Every texture has mipmaps, so every mode works.
     */
    function samplerOf(sm) {
      const s = sm || DEFAULT_SAMPLER;
      const key = [s.magFilter, s.minFilter, s.wrapS, s.wrapT].join(':');
      if (!gpu.samplers.has(key)) {
        const smp = gl.createSampler();
        gl.samplerParameteri(smp, gl.TEXTURE_MAG_FILTER, s.magFilter);
        gl.samplerParameteri(smp, gl.TEXTURE_MIN_FILTER, s.minFilter);
        gl.samplerParameteri(smp, gl.TEXTURE_WRAP_S, s.wrapS);
        gl.samplerParameteri(smp, gl.TEXTURE_WRAP_T, s.wrapT);
        gpu.samplers.set(key, smp);
      }
      return gpu.samplers.get(key);
    }
    function textureOf(i, srgb) {
      if (i == null || !bitmaps[i]) return srgb ? gpu.whiteSrgb : gpu.white;
      const key = i + ':' + (srgb ? 1 : 0);
      if (!gpu.textures.has(key)) gpu.textures.set(key, texture(0, 0, bitmaps[i], srgb ? gl.SRGB8_ALPHA8 : gl.RGBA8));
      return gpu.textures.get(key);
    }
    /** The 4x multisampled picture the model is drawn into, `w` by `h`; null to draw on the canvas. */
    function targets(w, h) {
      if (gpu.samples < 2) return null;
      if (gpu.fbo && gpu.size[0] === w && gpu.size[1] === h) return gpu.fbo;
      dropTargets();
      const rb = (format) => {
        const b = gl.createRenderbuffer();
        gl.bindRenderbuffer(gl.RENDERBUFFER, b);
        gl.renderbufferStorageMultisample(gl.RENDERBUFFER, gpu.samples, format, w, h);
        return b;
      };
      gpu.color = rb(gl.RGBA8);
      gpu.depth = rb(gl.DEPTH_COMPONENT24);
      gpu.fbo = gl.createFramebuffer();
      gl.bindFramebuffer(gl.FRAMEBUFFER, gpu.fbo);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.RENDERBUFFER, gpu.color);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, gpu.depth);
      if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) {
        dropTargets();
        gpu.samples = 0; // drawn straight onto the canvas instead
        return null;
      }
      gpu.size = [w, h];
      return gpu.fbo;
    }
    function dropTargets() {
      if (!gpu) return;
      if (gpu.fbo) gl.deleteFramebuffer(gpu.fbo);
      if (gpu.color) gl.deleteRenderbuffer(gpu.color);
      if (gpu.depth) gl.deleteRenderbuffer(gpu.depth);
      gpu.fbo = gpu.color = gpu.depth = null;
      gpu.size = [0, 0];
    }

    async function prepare(m) {
      model = m;
      const side = Math.min(MAX_TEXTURE_SIDE, Number(gl.getParameter(gl.MAX_TEXTURE_SIZE)) || MAX_TEXTURE_SIDE);
      bitmaps = await decodeImages(m.images, side, canvas.ownerDocument);
    }

    /** Draw one picture of the model as `cam` sees it, `width` by `height` pixels. */
    function draw(frame) {
      if (lost || !model || (typeof gl.isContextLost === 'function' && gl.isContextLost())) return false;
      if (!gpu) build();
      const max = Number(gl.getParameter(gl.MAX_RENDERBUFFER_SIZE)) || 4096;
      const w = Math.max(1, Math.min(max, Math.round(frame.width)));
      const h = Math.max(1, Math.min(max, Math.round(frame.height)));
      if (canvas.width !== w) canvas.width = w;
      if (canvas.height !== h) canvas.height = h;
      const fbo = targets(w, h);
      gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
      gl.viewport(0, 0, w, h);
      const bg = frame.background || [0, 0, 0];
      gl.clearColor(bg[0], bg[1], bg[2], 1);
      gl.clearDepth(1);
      gl.enable(gl.DEPTH_TEST);
      gl.depthFunc(gl.LEQUAL);
      gl.depthMask(true);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

      // The camera around the model's centre (where its positions were uploaded).
      const c = model.bounds.centre;
      const cam = Object.assign({}, frame.cam, { target: sub(frame.cam.target, c) });
      const view = viewMatrix(cam);
      const viewProj = multiply(projectionMatrix(cam, w / h), view);
      const { forward, right, up } = cameraBasis(cam);
      const dirs = new Float32Array(12);
      const colours = new Float32Array(9);
      LIGHTS.forEach(([d, colour, intensity], i) => {
        // From view space to the world: right, up and towards the viewer (-forward).
        const world = normalizeOr([0, 1, 2].map((j) => right[j] * d[0] + up[j] * d[1] - forward[j] * d[2]), [0, 1, 0]);
        dirs.set([world[0], world[1], world[2], intensity], i * 4);
        colours.set(colour, i * 3);
      });
      gl.useProgram(gpu.prog);
      gl.uniformMatrix4fv(gpu.loc.viewProj, false, new Float32Array(viewProj));
      gl.uniform3fv(gpu.loc.eye, new Float32Array(cameraEye(cam)));
      gl.uniform4fv(gpu.loc.lightDir, dirs);
      gl.uniform3fv(gpu.loc.lightColor, colours);
      gl.bindVertexArray(gpu.vao);
      gl.frontFace(gl.CCW);
      gl.cullFace(gl.BACK);
      // Opaque and masked parts first, then blended ones over them, without writing depth.
      for (const blended of [false, true]) {
        if (blended) {
          gl.enable(gl.BLEND);
          gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
          gl.depthMask(false);
        } else {
          gl.disable(gl.BLEND);
          gl.depthMask(true);
        }
        for (const d of model.draws) {
          const m = model.materials[d.material];
          if (!m || (m.alphaMode === 'BLEND') !== blended || d.count === 0) continue;
          if (m.doubleSided) gl.disable(gl.CULL_FACE); else gl.enable(gl.CULL_FACE);
          gl.uniform4fv(gpu.loc.base, m.baseColor);
          gl.uniform3fv(gpu.loc.emissive, m.emissive);
          gl.uniform4f(gpu.loc.params, m.metallic, m.roughness, m.alphaCutoff, m.alphaMode === 'MASK' ? 1 : m.alphaMode === 'BLEND' ? 2 : 0);
          gl.uniform4f(gpu.loc.extra, m.normalScale, m.occlusionStrength, m.normalTexture && bitmaps[m.normalTexture.image] ? 1 : 0, 0);
          // Each slot: its picture (white when it has none) through its own sampler.
          const units = [[m.baseTexture, true], [m.metallicRoughnessTexture, false], [m.normalTexture, false], [m.occlusionTexture, false], [m.emissiveTexture, true]];
          units.forEach(([tex, srgb], unit) => {
            gl.activeTexture(gl.TEXTURE0 + unit);
            gl.bindTexture(gl.TEXTURE_2D, textureOf(tex ? tex.image : null, srgb));
            gl.bindSampler(unit, samplerOf(tex ? tex.sampler : null));
          });
          gl.drawElements(gl.TRIANGLES, d.count, gl.UNSIGNED_INT, d.first * 4);
        }
      }
      gl.depthMask(true);
      gl.disable(gl.BLEND);
      gl.bindVertexArray(null);
      if (fbo) {
        // The multisampled picture, resolved onto the canvas.
        // The first copy is checked (asking the GPU for errors waits for it, so not every frame).
        const check = !gpu.blitChecked;
        if (check) while (gl.getError() !== gl.NO_ERROR) { /* earlier errors are not this copy's */ }
        gl.bindFramebuffer(gl.READ_FRAMEBUFFER, fbo);
        gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
        gl.blitFramebuffer(0, 0, w, h, 0, 0, w, h, gl.COLOR_BUFFER_BIT, gl.NEAREST);
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        gpu.blitChecked = true;
        if (check && gl.getError() !== gl.NO_ERROR) {
          // A browser whose canvas cannot take the copy: drawn straight onto the canvas from now
          // on (without the multisampling), and this picture drawn again that way.
          dropTargets();
          gpu.samples = 0;
          return draw(frame);
        }
      }
      return true;
    }

    /** Give the GPU memory back (the pictures stay decoded, so the next draw uploads again). */
    function free() {
      if (gpu && !lost && !(typeof gl.isContextLost === 'function' && gl.isContextLost())) {
        dropTargets();
        for (const t of gpu.textures.values()) gl.deleteTexture(t);
        gl.deleteTexture(gpu.white);
        gl.deleteTexture(gpu.whiteSrgb);
        for (const smp of gpu.samplers.values()) gl.deleteSampler(smp);
        for (const b of gpu.buffers) gl.deleteBuffer(b);
        gl.deleteBuffer(gpu.ib);
        gl.deleteVertexArray(gpu.vao);
        gl.deleteProgram(gpu.prog);
      }
      gpu = null;
    }
    function destroy() {
      free();
      for (const b of bitmaps) if (b && typeof b.close === 'function') b.close();
      bitmaps = [];
      model = null;
      canvas.removeEventListener('webglcontextlost', onLost);
      canvas.removeEventListener('webglcontextrestored', onRestored);
      // Ask the browser to let go of the context now, not whenever it collects the canvas.
      try { const ext = gl.getExtension('WEBGL_lose_context'); if (ext) ext.loseContext(); } catch (e) { /* it goes with the canvas */ }
    }
    return Object.assign(r, { prepare, draw, free, destroy });
  }

  // ── The view ────────────────────────────────────────────────────────────

  /**
   * One animation frame later, so words are painted before work; or a tenth of a second, whichever
   * is first (a hidden tab has no frames, and the reading should not wait for it to be seen).
   */
  function nextFrame(win) {
    return new Promise((resolve) => {
      if (win && typeof win.requestAnimationFrame === 'function') win.requestAnimationFrame(() => setTimeout(resolve, 0));
      setTimeout(resolve, win && typeof win.requestAnimationFrame === 'function' ? 100 : 16);
    });
  }

  /**
   * Open the viewer in `container`. `o`: name (the file's own name, as shown), fileName (the name
   * whose ending says how to read it; `name` when left out), size (bytes, as the page knows it),
   * fetchBytes(signal) -> Promise<ArrayBuffer>, sizeWords (the page's own, else this file's),
   * background ([r, g, b] from 0 to 1; else
   * the theme's card colour), renderer(canvas) (a stand-in drawer, for tests; the page leaves it
   * out). Returns {close(), phase(), camera()}.
   */
  function open(container, opts) {
    const o = opts || {};
    const doc = container.ownerDocument;
    const win = doc.defaultView || (typeof window !== 'undefined' ? window : null);
    const name = String(o.name || 'model');
    const fileName = String(o.fileName || name);
    const size = Number(o.size) || 0;
    const words = typeof o.sizeWords === 'function' ? o.sizeWords : sizeWords;
    const raf = win && typeof win.requestAnimationFrame === 'function' ? (f) => win.requestAnimationFrame(f) : (f) => setTimeout(f, 16);
    const cancelRaf = win && typeof win.cancelAnimationFrame === 'function' ? (id) => win.cancelAnimationFrame(id) : (id) => clearTimeout(id);
    const Abort = typeof AbortController !== 'undefined' ? AbortController : null;
    const ctl = Abort ? new Abort() : null;

    const el = (tag, className, css) => {
      const e = doc.createElement(tag);
      e.className = className;
      if (css) e.style.cssText = css;
      return e;
    };
    const stage = el('div', 'model-view-stage', 'position:relative;width:100%;height:240px;background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);overflow:hidden;user-select:none;-webkit-user-select:none;');
    const canvas = el('canvas', 'model-view-canvas', 'display:none;width:100%;height:100%;touch-action:none;cursor:grab;');
    canvas.setAttribute('aria-label', name);
    const message = el('p', 'model-view-message', 'position:absolute;inset:0;display:flex;align-items:center;justify-content:center;text-align:center;margin:0;padding:var(--space-lg);font-size:var(--text-sm);color:var(--text-muted);');
    const line = el('p', 'model-view-line', 'margin:var(--space-sm) 0 0;font-size:var(--text-xs);color:var(--text-muted);');
    stage.append(canvas, message);
    container.replaceChildren(stage, line);

    let phase = 'starting';
    let closed = false;
    let cam = null;
    let model = null;
    let renderer = null;
    let frameId = null;
    let dirty = false;
    let forgetTimer = null;
    let forgotten = false;
    let visible = true;
    let resizeWatch = null;
    let sightWatch = null;
    const pointers = new Map();
    let lastTap = null;
    let drawnSize = ''; // the view's width and pixel ratio when it was last drawn

    function say(text, warn) {
      message.textContent = text;
      message.style.color = warn ? 'var(--warning)' : 'var(--text-muted)';
      message.style.display = 'flex';
      canvas.style.display = 'none';
    }
    function fail(text) {
      phase = 'failed';
      say(text, true);
      if (renderer) { try { renderer.destroy(); } catch (e) { /* nothing left to give back */ } renderer = null; }
    }

    // ── Size and drawing: only when something changed ──
    function viewWidth() {
      const r = typeof stage.getBoundingClientRect === 'function' ? stage.getBoundingClientRect() : null;
      return r && r.width > 0 ? r.width : 640;
    }
    /** The view's height in points: 0.62 of its width, 160 to 480 (the desktop's). */
    function viewHeight() {
      return Math.min(480, Math.max(160, viewWidth() * 0.62));
    }
    /** The canvas's own size on the page in points (inside the stage's border), once it is shown. */
    function canvasSize() {
      const r = typeof canvas.getBoundingClientRect === 'function' ? canvas.getBoundingClientRect() : null;
      return r && r.width > 0 && r.height > 0 ? [r.width, r.height] : [viewWidth(), viewHeight()];
    }
    function sizeKey() {
      return viewWidth() + 'x' + ((win && win.devicePixelRatio) || 1);
    }
    function fitHeight() {
      const hgt = Math.round(viewHeight()) + 'px';
      if (stage.style.height !== hgt) stage.style.height = hgt;
    }
    function invalidate() {
      if (closed || !cam) return;
      dirty = true;
      if (frameId === null) frameId = raf(drawFrame);
    }
    function drawFrame() {
      frameId = null;
      if (closed || !dirty || !cam || !renderer || !visible) return;
      dirty = false;
      fitHeight();
      const dpr = (win && win.devicePixelRatio) || 1;
      drawnSize = sizeKey();
      // The picture is exactly as many pixels as the canvas shows, so nothing is stretched.
      const [w, h] = canvasSize();
      try {
        renderer.draw({ cam, width: w * dpr, height: h * dpr, background: background() });
        forgotten = false;
      } catch (e) {
        // The browser's graphics refused (a shader it would not build, memory it would not give):
        // not the file's fault, so not the file's words. The reason goes to the console for a developer.
        if (typeof console !== 'undefined') console.warn('model-view: drawing failed', e);
        fail(WORDS.noGraphics);
      }
    }
    /** The theme's card colour (the page's own background behind the model). */
    function background() {
      if (Array.isArray(o.background)) return o.background;
      try {
        if (win && typeof win.getComputedStyle === 'function') {
          const s = win.getComputedStyle(container);
          return cssColour(s.getPropertyValue('--bg-card')) || cssColour(s.getPropertyValue('--bg')) || [0.016, 0.016, 0.016];
        }
      } catch (e) { /* the dark card colour below */ }
      return [0.016, 0.016, 0.016];
    }
    /** Out of sight (or its tab hidden) for ten seconds, the view gives its GPU memory back. */
    function setVisible(now) {
      if (closed) return;
      visible = now;
      if (now) {
        if (forgetTimer) { clearTimeout(forgetTimer); forgetTimer = null; }
        if (forgotten) { forgotten = false; dirty = true; }
        if (dirty) invalidate();
      } else if (!forgetTimer) {
        forgetTimer = setTimeout(() => {
          forgetTimer = null;
          if (closed || visible || !renderer) return;
          renderer.free();
          forgotten = true;
          dirty = true; // drawn again (uploaded again) when it is back in sight
        }, FORGET_AFTER_MS);
      }
    }
    let inSight = true;
    const onVisibility = () => setVisible(inSight && doc.visibilityState !== 'hidden');

    // ── Hands on it: drag turns, right-drag or Shift-drag moves, scroll zooms, double-click starts over ──
    function at(e) { return { x: e.clientX, y: e.clientY }; }
    function onDown(e) {
      if (!cam || closed) return;
      if (e.pointerType === 'mouse' && e.button !== 0 && e.button !== 2) return;
      try { canvas.setPointerCapture(e.pointerId); } catch (err) { /* moves outside the view are then missed */ }
      const p = at(e);
      pointers.set(e.pointerId, { x: p.x, y: p.y, downX: p.x, downY: p.y, downAt: Date.now(), button: e.button, touch: e.pointerType === 'touch' || e.pointerType === 'pen' });
      canvas.style.cursor = 'grabbing';
    }
    function onMove(e) {
      const p = pointers.get(e.pointerId);
      if (!p || !cam) return;
      const now = at(e);
      if (!p.touch) {
        const dx = now.x - p.x;
        const dy = now.y - p.y;
        p.x = now.x; p.y = now.y;
        if (p.button === 2 || e.shiftKey) cam = panCamera(cam, dx, dy, canvasSize()[1]);
        else cam = orbitCamera(cam, dx, dy);
        invalidate();
        return;
      }
      const touches = Array.from(pointers.values()).filter((q) => q.touch);
      if (touches.length === 1) {
        cam = orbitCamera(cam, now.x - p.x, now.y - p.y);
        p.x = now.x; p.y = now.y;
      } else {
        // Two fingers: their middle moves the model, their spread zooms.
        const [a, b] = touches;
        const before = { mx: (a.x + b.x) / 2, my: (a.y + b.y) / 2, d: Math.hypot(a.x - b.x, a.y - b.y) };
        p.x = now.x; p.y = now.y;
        const after = { mx: (a.x + b.x) / 2, my: (a.y + b.y) / 2, d: Math.hypot(a.x - b.x, a.y - b.y) };
        cam = panCamera(cam, after.mx - before.mx, after.my - before.my, canvasSize()[1]);
        if (before.d > 0 && after.d > 0) cam = zoomCamera(cam, Math.log(after.d / before.d) / ZOOM_PER_POINT);
      }
      invalidate();
    }
    function onUp(e) {
      const p = pointers.get(e.pointerId);
      if (!p) return;
      pointers.delete(e.pointerId);
      if (!pointers.size) canvas.style.cursor = 'grab';
      // A touch screen's double tap starts over, as a mouse's double-click does.
      if (p.touch && e.type === 'pointerup' && Math.hypot(e.clientX - p.downX, e.clientY - p.downY) < 10 && Date.now() - p.downAt < 300) {
        const t = Date.now();
        if (lastTap && t - lastTap.at < 350 && Math.hypot(e.clientX - lastTap.x, e.clientY - lastTap.y) < 30) {
          lastTap = null;
          reset();
        } else {
          lastTap = { at: t, x: e.clientX, y: e.clientY };
        }
      }
    }
    function onWheel(e) {
      if (!cam) return;
      if (e.cancelable) e.preventDefault(); // the scroll zooms the model rather than scrolling the page
      const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? canvasSize()[1] : 1;
      cam = zoomCamera(cam, -(Number(e.deltaY) || 0) * unit);
      invalidate();
    }
    function reset() {
      if (!cam) return;
      cam = resetCamera(cam);
      invalidate();
    }
    const noMenu = (e) => { if (e.cancelable) e.preventDefault(); };
    canvas.addEventListener('pointerdown', onDown);
    canvas.addEventListener('pointermove', onMove);
    canvas.addEventListener('pointerup', onUp);
    canvas.addEventListener('pointercancel', onUp);
    canvas.addEventListener('wheel', onWheel, { passive: false });
    canvas.addEventListener('dblclick', reset);
    canvas.addEventListener('contextmenu', noMenu);

    function show() {
      phase = 'shown';
      message.style.display = 'none';
      canvas.style.display = 'block';
      line.textContent = fill(WORDS.shown, { name, triangles: thousands(model.triangles) });
      fitHeight();
      if (typeof root.ResizeObserver === 'function') {
        // Drawn again only when its width (or the screen's pixel ratio) changed, not for the height
        // the view sets itself after its first picture.
        resizeWatch = new root.ResizeObserver(() => { if (sizeKey() !== drawnSize) invalidate(); });
        resizeWatch.observe(stage);
      } else if (win && typeof win.addEventListener === 'function') {
        win.addEventListener('resize', invalidate);
      }
      if (typeof root.IntersectionObserver === 'function') {
        sightWatch = new root.IntersectionObserver((entries) => {
          for (const en of entries) inSight = !!en.isIntersecting;
          onVisibility();
        });
        sightWatch.observe(stage);
      }
      if (typeof doc.addEventListener === 'function') doc.addEventListener('visibilitychange', onVisibility);
      invalidate();
    }

    async function start() {
      // Over the size limit: said, and nothing fetched.
      if (size > MAX_MODEL_BYTES) { fail(fill(WORDS.tooLarge, { size: words(size) })); return; }
      renderer = (typeof o.renderer === 'function' ? o.renderer : createGlRenderer)(canvas);
      if (!renderer) { fail(WORDS.noGraphics); return; }
      renderer.onRestored = () => { dirty = true; invalidate(); };
      phase = 'fetching';
      say(fill(WORDS.fetching, { name, size: words(size) }));
      let bytes = null;
      try {
        bytes = await o.fetchBytes(ctl ? ctl.signal : undefined);
      } catch (e) {
        if (closed) return;
        if (e && e.kind === 'too_large') fail(fill(WORDS.tooLarge, { size: words(Math.max(Number(e.size) || 0, size)) }));
        else if (e && e.name === 'ModelError') fail(e.message);
        else fail(fill(WORDS.fetchFailed, { reason: String((e && e.message) || e || 'no answer').replace(/\.$/, '') }));
        return;
      }
      if (closed) return;
      phase = 'reading';
      say(WORDS.reading);
      await nextFrame(win);
      if (closed) return;
      try {
        model = parseModel(fileName, bytes);
        bytes = null;
        await renderer.prepare(model);
      } catch (e) {
        if (closed) return;
        fail(e && e.name === 'ModelError' ? e.message : fill(WORDS.unreadable, { reason: String((e && e.message) || e).replace(/\.$/, '') }));
        return;
      }
      if (closed) return;
      cam = fitCamera(model.bounds);
      show();
    }

    function close() {
      if (closed) return;
      closed = true;
      phase = 'closed';
      if (ctl) { try { ctl.abort(); } catch (e) { /* the fetch had ended */ } }
      if (frameId !== null) { cancelRaf(frameId); frameId = null; }
      if (forgetTimer) { clearTimeout(forgetTimer); forgetTimer = null; }
      if (resizeWatch) resizeWatch.disconnect();
      if (sightWatch) sightWatch.disconnect();
      if (win && typeof win.removeEventListener === 'function') win.removeEventListener('resize', invalidate);
      if (typeof doc.removeEventListener === 'function') doc.removeEventListener('visibilitychange', onVisibility);
      if (renderer) { try { renderer.destroy(); } catch (e) { /* it was already gone */ } renderer = null; }
      model = null;
      cam = null;
      container.replaceChildren();
    }

    const done = start();
    return { close, done, phase: () => phase, camera: () => cam, reset };
  }

  const api = {
    WORDS, MAX_MODEL_BYTES, MAX_TRIANGLES, MAX_TEXTURE_SIDE, FOV_Y_DEG, TURN_PER_POINT, ZOOM_PER_POINT, LIGHTS,
    fill, thousands, sizeWords, isModelFile,
    parseGlb, parseStl, parseModel, readBody,
    fitCamera, resetCamera, orbitCamera, panCamera, zoomCamera, cameraEye, nearFar, viewMatrix, projectionMatrix,
    createGlRenderer, open,
  };
  root.HosModelView = api;
  if (typeof module === 'object' && module && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
