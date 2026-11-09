// A small, deterministic glTF 2.0 writer for the character asset converters
// (scripts/gen-makehuman-body.js, scripts/gen-quaternius-clips.js; character animation rung 1,
// 2026-11-09). It packs typed arrays into one .bin with 4-byte alignment, writes accessors (dense
// or sparse) and returns the .gltf text and the .bin bytes. Nothing random and no time stamps, so a
// rebuild from the same inputs is byte-identical (each converter's --check relies on it).
//
// Numbers in the JSON are written with nine significant digits (enough to round-trip a float32),
// the same rule as scripts/gen-test-figure.js.

'use strict';

const COMPONENT = { 5120: Int8Array, 5121: Uint8Array, 5122: Int16Array, 5123: Uint16Array, 5125: Uint32Array, 5126: Float32Array };
const WIDTH = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };

class GltfBuilder {
  constructor() {
    this.chunks = [];
    this.byteLength = 0;
    this.bufferViews = [];
    this.accessors = [];
  }

  /** Append raw bytes as a buffer view (4-byte aligned); returns its index. */
  view(typed, target) {
    const pad = (4 - (this.byteLength % 4)) % 4;
    if (pad) {
      this.chunks.push(Buffer.alloc(pad));
      this.byteLength += pad;
    }
    const bytes = Buffer.from(typed.buffer, typed.byteOffset, typed.byteLength);
    const v = { buffer: 0, byteOffset: this.byteLength, byteLength: bytes.length };
    if (target) v.target = target;
    this.bufferViews.push(v);
    this.chunks.push(Buffer.from(bytes));
    this.byteLength += bytes.length;
    return this.bufferViews.length - 1;
  }

  /**
   * A dense accessor. `data` is a typed array holding `count * width` components.
   * Options: target (34962 vertices, 34963 indices), minMax (POSITION needs it), name, normalized.
   */
  accessor(data, type, { target, minMax, name, normalized } = {}) {
    const componentType = componentOf(data);
    const width = WIDTH[type];
    const count = data.length / width;
    if (!Number.isInteger(count)) throw new Error(`accessor ${name || ''}: ${data.length} components is not a whole number of ${type}`);
    const a = { bufferView: this.view(data, target), componentType, count, type };
    if (normalized) a.normalized = true;
    if (minMax) Object.assign(a, minMaxOf(data, width));
    if (name) a.name = name;
    this.accessors.push(a);
    return this.accessors.length - 1;
  }

  /**
   * A sparse accessor over `count` elements, all zero but those listed: `indices` (Uint16Array or
   * Uint32Array, rising) and their `values` (Float32Array, width per index). glTF 2.0 section
   * 3.6.2.3: no bufferView, so the base is zeros.
   */
  sparse(count, type, indices, values, { name, minMax } = {}) {
    const width = WIDTH[type];
    if (values.length !== indices.length * width) throw new Error(`sparse accessor ${name || ''}: ${indices.length} indices but ${values.length} values`);
    const a = {
      componentType: 5126,
      count,
      type,
      sparse: {
        count: indices.length,
        indices: { bufferView: this.view(indices), componentType: componentOf(indices) },
        values: { bufferView: this.view(values) },
      },
    };
    if (minMax) {
      // min and max over every element, the zeros included.
      const mm = minMaxOf(values, width);
      if (indices.length < count) {
        mm.min = mm.min.map((x) => Math.min(x, 0));
        mm.max = mm.max.map((x) => Math.max(x, 0));
      }
      Object.assign(a, mm);
    }
    if (name) a.name = name;
    this.accessors.push(a);
    return this.accessors.length - 1;
  }

  /** The finished files: `json` text for `<name>.gltf` and the `bin` bytes for `<binName>`. */
  finish(gltf, binName) {
    gltf.accessors = this.accessors;
    gltf.bufferViews = this.bufferViews;
    gltf.buffers = [{ uri: binName, byteLength: this.byteLength }];
    const json = JSON.stringify(gltf, (key, v) => (typeof v === 'number' && !Number.isInteger(v) ? Number(v.toPrecision(9)) : v), 1) + '\n';
    return { json, bin: Buffer.concat(this.chunks) };
  }
}

function componentOf(data) {
  for (const [code, T] of Object.entries(COMPONENT)) if (data instanceof T) return Number(code);
  throw new Error('unsupported typed array');
}

function minMaxOf(data, width) {
  const min = new Array(width).fill(Infinity);
  const max = new Array(width).fill(-Infinity);
  for (let i = 0; i < data.length; i++) {
    const c = i % width;
    if (data[i] < min[c]) min[c] = data[i];
    if (data[i] > max[c]) max[c] = data[i];
  }
  return { min, max };
}

/**
 * Compare freshly made files with what is on disk. A .gltf is text and git may check it out with
 * CRLF, so it is compared without line endings; anything else byte for byte. Returns the list of
 * paths that differ (empty when all match).
 */
function differing(files, fs, path, root) {
  const bad = [];
  for (const [p, bytes] of files) {
    const have = fs.existsSync(p) ? fs.readFileSync(p) : null;
    const text = p.endsWith('.gltf') || p.endsWith('.md') || p.endsWith('.txt');
    const same = text ? have && have.toString().replace(/\r\n/g, '\n') === bytes.toString().replace(/\r\n/g, '\n') : have && have.equals(bytes);
    if (!same) bad.push(path.relative(root, p));
  }
  return bad;
}

module.exports = { GltfBuilder, differing, WIDTH };
