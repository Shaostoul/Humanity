#!/usr/bin/env node
// repack-plant-gltf.js — merge every mesh/primitive of a .gltf plant model into
// ONE mesh with ONE primitive, so the engine's limited glTF loader (which reads
// only the first mesh's first primitive — see src/assets/mod.rs parse_gltf_mesh)
// renders the whole model instead of silently dropping most of it.
//
// Usage:
//   node scripts/repack-plant-gltf.js <folder> [<folder> ...]   one or more plant folders
//   node scripts/repack-plant-gltf.js --all                     every folder in assets/models/plants/
//
// <folder> may be a slug ("fir_sapling") or a path ("assets/models/plants/fir_sapling").
//
// For each model this writes <slug>_merged.gltf + <slug>_merged.bin next to the
// originals: minimal valid glTF 2.0 (one buffer, one mesh, one primitive, one
// node, one scene, no materials), with POSITION/NORMAL/TEXCOORD_0 float32
// attributes and a u16 or u32 index buffer. Each source primitive's positions
// and normals are baked through its node's composed world transform, so the
// merged geometry sits exactly where the original multi-node model placed it.
// After writing, the script re-parses its own output and verifies vertex count,
// triangle count (must equal the sum over all source primitives), and bounds.
//
// Dependency-free: node built-ins only (fs, path). No npm installs.

'use strict';

const fs = require('fs');
const path = require('path');

const PLANTS_DIR = path.join(__dirname, '..', 'assets', 'models', 'plants');

// glTF componentType codes
const CT_U16 = 5123;
const CT_U32 = 5125;
const CT_F32 = 5126;
const COMPONENT_SIZE = { 5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4 };
const TYPE_COMPONENTS = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT2: 4, MAT3: 9, MAT4: 16 };

// ---------------------------------------------------------------------------
// 4x4 matrix helpers (column-major, matching the glTF spec's node.matrix)
// ---------------------------------------------------------------------------

const MAT4_IDENTITY = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];

/** out = a * b (both column-major 16-element arrays). */
function mat4Multiply(a, b) {
  const out = new Array(16);
  for (let col = 0; col < 4; col++) {
    for (let row = 0; row < 4; row++) {
      let sum = 0;
      for (let k = 0; k < 4; k++) sum += a[k * 4 + row] * b[col * 4 + k];
      out[col * 4 + row] = sum;
    }
  }
  return out;
}

/** Build a column-major matrix from glTF node translation/rotation/scale (T * R * S). */
function mat4FromTrs(t, r, s) {
  const [tx, ty, tz] = t;
  const [qx, qy, qz, qw] = r; // glTF quaternion order: x, y, z, w
  const [sx, sy, sz] = s;
  // Rotation matrix from unit quaternion
  const x2 = qx + qx, y2 = qy + qy, z2 = qz + qz;
  const xx = qx * x2, xy = qx * y2, xz = qx * z2;
  const yy = qy * y2, yz = qy * z2, zz = qz * z2;
  const wx = qw * x2, wy = qw * y2, wz = qw * z2;
  // Column-major: column i = rotated+scaled basis vector i
  return [
    (1 - (yy + zz)) * sx, (xy + wz) * sx, (xz - wy) * sx, 0,
    (xy - wz) * sy, (1 - (xx + zz)) * sy, (yz + wx) * sy, 0,
    (xz + wy) * sz, (yz - wx) * sz, (1 - (xx + yy)) * sz, 0,
    tx, ty, tz, 1,
  ];
}

/** Local transform of a glTF node: explicit matrix wins, else composed TRS. */
function nodeLocalMatrix(node) {
  if (node.matrix) return node.matrix.slice();
  return mat4FromTrs(
    node.translation || [0, 0, 0],
    node.rotation || [0, 0, 0, 1],
    node.scale || [1, 1, 1]
  );
}

/** Upper-left 3x3 of a column-major 4x4, as a column-major 9-element array. */
function mat4Upper3x3(m) {
  return [m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]];
}

function mat3Determinant(m) {
  return (
    m[0] * (m[4] * m[8] - m[7] * m[5]) -
    m[3] * (m[1] * m[8] - m[7] * m[2]) +
    m[6] * (m[1] * m[5] - m[4] * m[2])
  );
}

/** Inverse-transpose of a column-major 3x3 — the correct transform for normals. */
function mat3InverseTranspose(m) {
  const det = mat3Determinant(m);
  if (Math.abs(det) < 1e-12) {
    throw new Error('degenerate node transform (3x3 determinant ~ 0); cannot transform normals');
  }
  const invDet = 1 / det;
  // inverse = adjugate / det; then transpose. Doing both at once:
  // inverseTranspose[row][col] = cofactor[row][col] / det (no transpose of the adjugate).
  return [
    (m[4] * m[8] - m[7] * m[5]) * invDet,
    (m[6] * m[5] - m[3] * m[8]) * invDet,
    (m[3] * m[7] - m[6] * m[4]) * invDet,
    (m[7] * m[2] - m[1] * m[8]) * invDet,
    (m[0] * m[8] - m[6] * m[2]) * invDet,
    (m[6] * m[1] - m[0] * m[7]) * invDet,
    (m[1] * m[5] - m[4] * m[2]) * invDet,
    (m[3] * m[2] - m[0] * m[5]) * invDet,
    (m[0] * m[4] - m[3] * m[1]) * invDet,
  ];
}

// ---------------------------------------------------------------------------
// glTF parsing
// ---------------------------------------------------------------------------

/** Load every buffer referenced by the glTF (external .bin files or data: URIs). */
function loadBuffers(gltf, baseDir) {
  return (gltf.buffers || []).map((buf, i) => {
    if (!buf.uri) throw new Error(`buffer ${i} has no uri (GLB-style embedded buffers not supported)`);
    let bytes;
    if (buf.uri.startsWith('data:')) {
      const comma = buf.uri.indexOf(',');
      bytes = Buffer.from(buf.uri.slice(comma + 1), 'base64');
    } else {
      bytes = fs.readFileSync(path.join(baseDir, decodeURIComponent(buf.uri)));
    }
    if (bytes.length < buf.byteLength) {
      throw new Error(`buffer ${i} (${buf.uri}): file has ${bytes.length} bytes, glTF declares ${buf.byteLength}`);
    }
    return bytes;
  });
}

/**
 * Read accessor `idx` into a plain Float32Array / Uint32Array (indices are
 * widened to u32). Handles byteOffset on both accessor and bufferView, and
 * interleaved data via bufferView.byteStride. Uses DataView so alignment of
 * the underlying node Buffer never matters.
 */
function readAccessor(gltf, buffers, idx) {
  const acc = gltf.accessors[idx];
  if (acc.sparse) throw new Error(`accessor ${idx} is sparse — not supported`);
  const numComp = TYPE_COMPONENTS[acc.type];
  const compSize = COMPONENT_SIZE[acc.componentType];
  if (!numComp || !compSize) throw new Error(`accessor ${idx}: unknown type ${acc.type}/${acc.componentType}`);

  if (acc.bufferView === undefined) {
    // Spec: no bufferView means all zeros.
    const OutZero = acc.componentType === CT_F32 ? Float32Array : Uint32Array;
    return new OutZero(acc.count * numComp);
  }

  const bv = gltf.bufferViews[acc.bufferView];
  const buf = buffers[bv.buffer];
  const elemSize = numComp * compSize;
  const stride = bv.byteStride || elemSize;
  const base = (bv.byteOffset || 0) + (acc.byteOffset || 0);
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);

  let out;
  let read;
  switch (acc.componentType) {
    case CT_F32:
      out = new Float32Array(acc.count * numComp);
      read = (off) => view.getFloat32(off, true);
      break;
    case CT_U16:
      out = new Uint32Array(acc.count * numComp); // widen u16 -> u32 for uniform merging
      read = (off) => view.getUint16(off, true);
      break;
    case CT_U32:
      out = new Uint32Array(acc.count * numComp);
      read = (off) => view.getUint32(off, true);
      break;
    default:
      throw new Error(`accessor ${idx}: componentType ${acc.componentType} not supported (expected 5123/5125/5126)`);
  }
  for (let i = 0; i < acc.count; i++) {
    const elemOff = base + i * stride;
    for (let c = 0; c < numComp; c++) out[i * numComp + c] = read(elemOff + c * compSize);
  }
  return out;
}

/**
 * Walk the default scene's node tree, composing world transforms, and return a
 * flat list of { node, mesh, worldMatrix } for every node that carries a mesh.
 */
function collectMeshInstances(gltf) {
  const sceneIdx = gltf.scene !== undefined ? gltf.scene : 0;
  const scene = (gltf.scenes || [])[sceneIdx];
  if (!scene) throw new Error('glTF has no scene');
  const instances = [];
  const visit = (nodeIdx, parentMatrix) => {
    const node = gltf.nodes[nodeIdx];
    const world = mat4Multiply(parentMatrix, nodeLocalMatrix(node));
    if (node.mesh !== undefined) instances.push({ node, mesh: gltf.meshes[node.mesh], world });
    for (const child of node.children || []) visit(child, world);
  };
  for (const rootIdx of scene.nodes || []) visit(rootIdx, MAT4_IDENTITY);

  // Sanity: warn if the scene graph misses any meshes (they would be dropped).
  const referenced = new Set(instances.map((it) => it.mesh));
  const orphaned = (gltf.meshes || []).filter((m) => !referenced.has(m)).length;
  if (orphaned > 0) console.warn(`  WARNING: ${orphaned} mesh(es) are not referenced by the scene and were skipped`);
  return instances;
}

// ---------------------------------------------------------------------------
// The merge itself
// ---------------------------------------------------------------------------

/**
 * Merge every primitive of every scene-referenced mesh into single
 * position/normal/uv/index arrays, world transforms baked in.
 */
function mergeModel(gltf, buffers) {
  const instances = collectMeshInstances(gltf);
  if (instances.length === 0) throw new Error('scene references no meshes');

  // Pass 1: totals, so we can pre-allocate exact-size typed arrays.
  let totalVerts = 0;
  let totalIndices = 0;
  let sourcePrimitives = 0;
  for (const { mesh } of instances) {
    for (const prim of mesh.primitives) {
      if (prim.mode !== undefined && prim.mode !== 4) {
        throw new Error(`primitive mode ${prim.mode} not supported (triangles only)`);
      }
      const posAcc = gltf.accessors[prim.attributes.POSITION];
      if (!posAcc) throw new Error('primitive has no POSITION attribute');
      if (posAcc.componentType !== CT_F32 || posAcc.type !== 'VEC3') {
        throw new Error(`POSITION accessor must be float32 VEC3, got ${posAcc.componentType}/${posAcc.type}`);
      }
      totalVerts += posAcc.count;
      totalIndices += prim.indices !== undefined ? gltf.accessors[prim.indices].count : posAcc.count;
      sourcePrimitives++;
    }
  }
  if (totalIndices % 3 !== 0) throw new Error(`total index count ${totalIndices} is not a multiple of 3`);

  const positions = new Float32Array(totalVerts * 3);
  const normals = new Float32Array(totalVerts * 3);
  const uvs = new Float32Array(totalVerts * 2);
  const indices = new Uint32Array(totalIndices);

  // Pass 2: read, transform, append.
  let vertBase = 0;
  let indexBase = 0;
  for (const { node, mesh, world } of instances) {
    const m3 = mat4Upper3x3(world);
    const nrmMat = mat3InverseTranspose(m3);
    const flipWinding = mat3Determinant(m3) < 0; // mirrored transform reverses triangle winding

    for (const prim of mesh.primitives) {
      const attrs = prim.attributes;
      const pos = readAccessor(gltf, buffers, attrs.POSITION);
      const count = pos.length / 3;

      // NORMAL / TEXCOORD_0 are present in all Poly Haven plant models; if a
      // future asset lacks one, fill a sane default rather than crash.
      let nrm = null;
      if (attrs.NORMAL !== undefined) {
        const a = gltf.accessors[attrs.NORMAL];
        if (a.componentType !== CT_F32 || a.type !== 'VEC3') {
          throw new Error(`NORMAL accessor must be float32 VEC3, got ${a.componentType}/${a.type}`);
        }
        nrm = readAccessor(gltf, buffers, attrs.NORMAL);
      } else {
        console.warn(`  WARNING: node "${node.name || '?'}" primitive has no NORMAL; defaulting to +Y`);
      }
      let uv = null;
      if (attrs.TEXCOORD_0 !== undefined) {
        const a = gltf.accessors[attrs.TEXCOORD_0];
        if (a.componentType !== CT_F32 || a.type !== 'VEC2') {
          throw new Error(`TEXCOORD_0 accessor must be float32 VEC2, got ${a.componentType}/${a.type}`);
        }
        uv = readAccessor(gltf, buffers, attrs.TEXCOORD_0);
      } else {
        console.warn(`  WARNING: node "${node.name || '?'}" primitive has no TEXCOORD_0; defaulting to (0,0)`);
      }

      for (let i = 0; i < count; i++) {
        const px = pos[i * 3], py = pos[i * 3 + 1], pz = pos[i * 3 + 2];
        const o = (vertBase + i) * 3;
        // World-transform the position: world * [p, 1]
        positions[o] = world[0] * px + world[4] * py + world[8] * pz + world[12];
        positions[o + 1] = world[1] * px + world[5] * py + world[9] * pz + world[13];
        positions[o + 2] = world[2] * px + world[6] * py + world[10] * pz + world[14];

        // Transform + renormalize the normal via the inverse-transpose 3x3
        let nx = 0, ny = 1, nz = 0;
        if (nrm) { nx = nrm[i * 3]; ny = nrm[i * 3 + 1]; nz = nrm[i * 3 + 2]; }
        let tx = nrmMat[0] * nx + nrmMat[3] * ny + nrmMat[6] * nz;
        let ty = nrmMat[1] * nx + nrmMat[4] * ny + nrmMat[7] * nz;
        let tz = nrmMat[2] * nx + nrmMat[5] * ny + nrmMat[8] * nz;
        const len = Math.hypot(tx, ty, tz);
        if (len > 1e-12) { tx /= len; ty /= len; tz /= len; } else { tx = 0; ty = 1; tz = 0; }
        normals[o] = tx; normals[o + 1] = ty; normals[o + 2] = tz;

        const u = (vertBase + i) * 2;
        uvs[u] = uv ? uv[i * 2] : 0;
        uvs[u + 1] = uv ? uv[i * 2 + 1] : 0;
      }

      // Append indices, rebased onto the merged vertex array.
      let primIdx;
      if (prim.indices !== undefined) {
        const iAcc = gltf.accessors[prim.indices];
        if (iAcc.componentType !== CT_U16 && iAcc.componentType !== CT_U32) {
          throw new Error(`index accessor componentType ${iAcc.componentType} not supported (expected 5123 u16 or 5125 u32)`);
        }
        primIdx = readAccessor(gltf, buffers, prim.indices);
      } else {
        primIdx = new Uint32Array(count);
        for (let i = 0; i < count; i++) primIdx[i] = i;
      }
      if (primIdx.length % 3 !== 0) throw new Error('primitive index count is not a multiple of 3');
      for (let i = 0; i < primIdx.length; i += 3) {
        if (flipWinding) {
          indices[indexBase + i] = vertBase + primIdx[i];
          indices[indexBase + i + 1] = vertBase + primIdx[i + 2];
          indices[indexBase + i + 2] = vertBase + primIdx[i + 1];
        } else {
          indices[indexBase + i] = vertBase + primIdx[i];
          indices[indexBase + i + 1] = vertBase + primIdx[i + 1];
          indices[indexBase + i + 2] = vertBase + primIdx[i + 2];
        }
      }

      vertBase += count;
      indexBase += primIdx.length;
    }
  }

  return { positions, normals, uvs, indices, totalVerts, sourcePrimitives, sourceMeshes: instances.length };
}

// ---------------------------------------------------------------------------
// Output writing
// ---------------------------------------------------------------------------

function computeMinMax(positions) {
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < positions.length; i += 3) {
    for (let c = 0; c < 3; c++) {
      const v = positions[i + c];
      if (v < min[c]) min[c] = v;
      if (v > max[c]) max[c] = v;
    }
  }
  return { min, max };
}

/** Buffer view of a typed array's exact bytes (safe for any underlying offset). */
function typedArrayBytes(ta) {
  return Buffer.from(ta.buffer, ta.byteOffset, ta.byteLength);
}

function writeMerged(outDir, slug, merged) {
  const { positions, normals, uvs, indices, totalVerts } = merged;

  // Output u16 indices when they fit, u32 otherwise.
  const useU32 = totalVerts > 65535;
  const idxOut = useU32 ? indices : Uint16Array.from(indices);

  // Lay out the .bin: positions | normals | uvs | indices, 4-byte aligned.
  const parts = [
    { bytes: typedArrayBytes(positions), target: 34962 }, // ARRAY_BUFFER
    { bytes: typedArrayBytes(normals), target: 34962 },
    { bytes: typedArrayBytes(uvs), target: 34962 },
    { bytes: typedArrayBytes(idxOut), target: 34963 }, // ELEMENT_ARRAY_BUFFER
  ];
  const chunks = [];
  const bufferViews = [];
  let offset = 0;
  for (const part of parts) {
    const pad = (4 - (offset % 4)) % 4;
    if (pad) { chunks.push(Buffer.alloc(pad)); offset += pad; }
    bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: part.bytes.length, target: part.target });
    chunks.push(part.bytes);
    offset += part.bytes.length;
  }
  const bin = Buffer.concat(chunks, offset);

  const { min, max } = computeMinMax(positions);
  const round = (arr) => arr.map((v) => Math.fround(v)); // min/max must match float32 data exactly

  const gltfOut = {
    asset: { generator: 'HumanityOS scripts/repack-plant-gltf.js (all primitives merged, transforms baked)', version: '2.0' },
    scene: 0,
    scenes: [{ name: 'Scene', nodes: [0] }],
    nodes: [{ mesh: 0, name: `${slug}_merged` }],
    meshes: [
      {
        name: `${slug}_merged`,
        primitives: [{ attributes: { POSITION: 0, NORMAL: 1, TEXCOORD_0: 2 }, indices: 3, mode: 4 }],
      },
    ],
    accessors: [
      { bufferView: 0, componentType: CT_F32, count: totalVerts, type: 'VEC3', min: round(min), max: round(max) },
      { bufferView: 1, componentType: CT_F32, count: totalVerts, type: 'VEC3' },
      { bufferView: 2, componentType: CT_F32, count: totalVerts, type: 'VEC2' },
      { bufferView: 3, componentType: useU32 ? CT_U32 : CT_U16, count: idxOut.length, type: 'SCALAR' },
    ],
    bufferViews,
    buffers: [{ byteLength: bin.length, uri: `${slug}_merged.bin` }],
  };

  const gltfPath = path.join(outDir, `${slug}_merged.gltf`);
  const binPath = path.join(outDir, `${slug}_merged.bin`);
  fs.writeFileSync(binPath, bin);
  fs.writeFileSync(gltfPath, JSON.stringify(gltfOut, null, 2) + '\n');
  return { gltfPath, binPath };
}

// ---------------------------------------------------------------------------
// Per-model driver + self-verification
// ---------------------------------------------------------------------------

function findSourceGltf(dir) {
  const files = fs.readdirSync(dir).filter((f) => f.endsWith('.gltf') && !f.endsWith('_merged.gltf'));
  if (files.length === 0) throw new Error(`no .gltf file in ${dir}`);
  if (files.length > 1) throw new Error(`multiple source .gltf files in ${dir}: ${files.join(', ')}`);
  return path.join(dir, files[0]);
}

function processModel(dir) {
  const slug = path.basename(dir);
  const srcPath = findSourceGltf(dir);
  console.log(`\n${slug}: repacking ${path.basename(srcPath)}`);

  const gltf = JSON.parse(fs.readFileSync(srcPath, 'utf8'));
  const buffers = loadBuffers(gltf, dir);

  // Count source triangles across ALL primitives of ALL scene meshes — the
  // ground truth the merged output must match.
  let sourceTriangles = 0;
  for (const { mesh } of collectMeshInstances(gltf)) {
    for (const prim of mesh.primitives) {
      const count = prim.indices !== undefined
        ? gltf.accessors[prim.indices].count
        : gltf.accessors[prim.attributes.POSITION].count;
      sourceTriangles += count / 3;
    }
  }

  const merged = mergeModel(gltf, buffers);
  const { gltfPath, binPath } = writeMerged(dir, slug, merged);

  // VERIFY: re-parse the file we just wrote, using the same reader.
  const reGltf = JSON.parse(fs.readFileSync(gltfPath, 'utf8'));
  const reBuffers = loadBuffers(reGltf, dir);
  const rePrim = reGltf.meshes[0].primitives[0];
  const rePos = readAccessor(reGltf, reBuffers, rePrim.attributes.POSITION);
  const reIdx = readAccessor(reGltf, reBuffers, rePrim.indices);
  const verts = rePos.length / 3;
  const tris = reIdx.length / 3;
  const { min, max } = computeMinMax(rePos);
  const dims = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];

  if (tris !== sourceTriangles) {
    throw new Error(`VERIFY FAILED: merged has ${tris} triangles, source primitives sum to ${sourceTriangles}`);
  }
  for (const idx of reIdx) {
    if (idx >= verts) throw new Error(`VERIFY FAILED: index ${idx} out of range (${verts} vertices)`);
  }

  const fmt = (arr) => '[' + arr.map((v) => v.toFixed(3)).join(', ') + ']';
  console.log(`  source: ${merged.sourceMeshes} mesh node(s), ${merged.sourcePrimitives} primitive(s)`);
  console.log(`  merged: ${verts} vertices, ${tris} triangles (matches source sum: OK)`);
  console.log(`  bounds: min ${fmt(min)} max ${fmt(max)} size ${fmt(dims)} m`);
  console.log(`  wrote:  ${path.basename(gltfPath)} + ${path.basename(binPath)} (${(fs.statSync(binPath).size / 1024 / 1024).toFixed(2)} MB)`);

  return { slug, verts, tris, min, max, dims, mergedGltf: `${slug}/${slug}_merged.gltf` };
}

/** Record each model's merged file in manifest.json (idempotent). */
function updateManifest(results) {
  const manifestPath = path.join(PLANTS_DIR, 'manifest.json');
  if (!fs.existsSync(manifestPath)) return;
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  let touched = 0;
  for (const res of results) {
    const entry = (manifest.assets || []).find((a) => a.slug === res.slug);
    if (!entry) continue;
    entry.merged = res.mergedGltf;
    touched++;
  }
  if (touched > 0) {
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
    console.log(`\nmanifest.json: updated "merged" field on ${touched} asset(s)`);
  }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function main() {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    console.error('usage: node scripts/repack-plant-gltf.js <folder|slug> [...] | --all');
    process.exit(1);
  }

  let dirs;
  if (args.includes('--all')) {
    dirs = fs
      .readdirSync(PLANTS_DIR, { withFileTypes: true })
      .filter((d) => d.isDirectory())
      .map((d) => path.join(PLANTS_DIR, d.name));
  } else {
    dirs = args.map((arg) => {
      const asPath = path.resolve(arg);
      if (fs.existsSync(asPath) && fs.statSync(asPath).isDirectory()) return asPath;
      const asSlug = path.join(PLANTS_DIR, arg);
      if (fs.existsSync(asSlug) && fs.statSync(asSlug).isDirectory()) return asSlug;
      throw new Error(`not a plant folder: ${arg}`);
    });
  }

  const results = [];
  const failures = [];
  for (const dir of dirs) {
    try {
      results.push(processModel(dir));
    } catch (err) {
      failures.push({ slug: path.basename(dir), error: err.message });
      console.error(`\n${path.basename(dir)}: FAILED - ${err.message}`);
    }
  }

  updateManifest(results);

  // Summary table
  if (results.length > 0) {
    console.log('\n=== summary ===');
    const pad = (s, n) => String(s).padEnd(n);
    console.log(pad('model', 22) + pad('verts', 10) + pad('tris', 10) + 'size (x, y, z) m');
    for (const r of results) {
      console.log(
        pad(r.slug, 22) + pad(r.verts, 10) + pad(r.tris, 10) +
        r.dims.map((v) => v.toFixed(2)).join(' x ')
      );
    }
  }
  if (failures.length > 0) {
    console.error(`\n${failures.length} model(s) failed: ${failures.map((f) => f.slug).join(', ')}`);
    process.exit(1);
  }
}

main();
