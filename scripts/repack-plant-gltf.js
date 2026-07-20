#!/usr/bin/env node
// repack-plant-gltf.js — merge every mesh/primitive of a .gltf plant model into
// ONE mesh with ONE primitive, so the engine's limited glTF loader (which reads
// only the first mesh's first primitive — see src/assets/mod.rs parse_gltf_mesh)
// renders the whole model instead of silently dropping most of it.
//
// Usage:
//   node scripts/repack-plant-gltf.js <folder> [<folder> ...]   merge mode, one or more plant folders
//   node scripts/repack-plant-gltf.js --all                     merge mode, every folder in assets/models/plants/
//   node scripts/repack-plant-gltf.js --split <folder|--all>    SPLIT mode (see below)
//
// <folder> may be a slug ("fir_sapling") or a path ("assets/models/plants/fir_sapling").
//
// MERGE mode: for each model this writes <slug>_merged.gltf + <slug>_merged.bin
// next to the originals: minimal valid glTF 2.0 (one buffer, one mesh, one
// primitive, one node, one scene, no materials), with POSITION/NORMAL/TEXCOORD_0
// float32 attributes and a u16 or u32 index buffer. Each source primitive's
// positions and normals are baked through its node's composed world transform,
// so the merged geometry sits exactly where the original multi-node model
// placed it. After writing, the script re-parses its own output and verifies
// vertex count, triangle count (must equal the sum over all source
// primitives), and bounds.
//
// SPLIT mode (--split): the Poly Haven source scenes lay several plant
// VARIANTS side by side (5 grass clumps in a row, 3 saplings at x=0/1/2, a
// 2x2 fern grid). --split clusters the mesh nodes by world-space X/Z into
// distinct variants and writes each as its own single-primitive
// <slug>_v1.gltf/.bin, <slug>_v2.gltf, ... with:
//   - the variant RE-CENTERED so its base (x/z centroid of its lowest
//     vertices) sits at local x=0,z=0; y is left untouched so y=0 stays the
//     ground plane;
//   - a minimal materials/textures/images/samplers section carrying the
//     variant's dominant (most triangles) source material's base-color
//     texture, referencing the ORIGINAL textures/ folder by relative URI
//     (variants live next to the source .gltf, so the source's own
//     "textures/..." URIs resolve unchanged — verified at write time).
// Clustering rule: measured bboxes show adjacent grass clumps OVERLAP
// slightly in X (gaps down to -0.027 m), so plain bbox-overlap would merge
// them. Instead, two nodes belong to the same variant when either node's
// XZ CENTER lies inside the other's (slightly expanded) XZ bbox, closed
// transitively (union-find). That keeps side-by-side rows separate while
// gluing stacked/nested parts (potted plant's pot + soil + stem + leaves)
// into one variant. Variant triangle totals are verified to sum exactly to
// the source-primitive total (same ground truth merge mode checks against).
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
  return mergeInstances(gltf, buffers, instances);
}

/**
 * Merge every primitive of the GIVEN mesh instances into single
 * position/normal/uv/index arrays, world transforms baked in. Used by merge
 * mode (all instances) and split mode (one cluster's instances at a time).
 */
function mergeInstances(gltf, buffers, instances) {
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
// Split mode: variant clustering, re-centering, material carry-over
// ---------------------------------------------------------------------------

/** Expansion margin (meters) for the center-inside-bbox cluster test. */
const CLUSTER_MARGIN = 0.01;

/** World-space bbox of one mesh instance (all its primitives' positions transformed). */
function instanceWorldBounds(gltf, buffers, inst) {
  const w = inst.world;
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (const prim of inst.mesh.primitives) {
    const pos = readAccessor(gltf, buffers, prim.attributes.POSITION);
    for (let i = 0; i < pos.length; i += 3) {
      const x = pos[i], y = pos[i + 1], z = pos[i + 2];
      const wx = w[0] * x + w[4] * y + w[8] * z + w[12];
      const wy = w[1] * x + w[5] * y + w[9] * z + w[13];
      const wz = w[2] * x + w[6] * y + w[10] * z + w[14];
      if (wx < min[0]) min[0] = wx; if (wx > max[0]) max[0] = wx;
      if (wy < min[1]) min[1] = wy; if (wy > max[1]) max[1] = wy;
      if (wz < min[2]) min[2] = wz; if (wz > max[2]) max[2] = wz;
    }
  }
  return { min, max, centerX: (min[0] + max[0]) / 2, centerZ: (min[2] + max[2]) / 2 };
}

/**
 * Cluster mesh instances into plant variants by X/Z position.
 *
 * Two instances share a variant when either one's XZ CENTER falls inside the
 * other's XZ bbox expanded by CLUSTER_MARGIN, closed transitively via
 * union-find. Plain bbox-overlap is NOT used: measured Poly Haven layouts
 * show adjacent grass clumps' bboxes overlap by up to ~0.03 m while their
 * centers stay well apart, whereas genuinely-stacked parts (pot + soil +
 * leaves of the potted plant) contain each other's centers.
 *
 * Returns clusters sorted by (min X, then min Z), each { instances, bounds }.
 */
function clusterInstances(gltf, buffers, instances) {
  const bounds = instances.map((inst) => instanceWorldBounds(gltf, buffers, inst));

  // Union-find over instance indices.
  const parent = instances.map((_, i) => i);
  const find = (i) => { while (parent[i] !== i) { parent[i] = parent[parent[i]]; i = parent[i]; } return i; };
  const union = (a, b) => { const ra = find(a), rb = find(b); if (ra !== rb) parent[rb] = ra; };

  const centerInside = (a, b) =>
    a.centerX >= b.min[0] - CLUSTER_MARGIN && a.centerX <= b.max[0] + CLUSTER_MARGIN &&
    a.centerZ >= b.min[2] - CLUSTER_MARGIN && a.centerZ <= b.max[2] + CLUSTER_MARGIN;

  for (let i = 0; i < instances.length; i++) {
    for (let j = i + 1; j < instances.length; j++) {
      if (centerInside(bounds[i], bounds[j]) || centerInside(bounds[j], bounds[i])) union(i, j);
    }
  }

  const byRoot = new Map();
  for (let i = 0; i < instances.length; i++) {
    const root = find(i);
    if (!byRoot.has(root)) byRoot.set(root, []);
    byRoot.get(root).push(i);
  }

  const clusters = [...byRoot.values()].map((idxs) => {
    const min = [Infinity, Infinity, Infinity];
    const max = [-Infinity, -Infinity, -Infinity];
    for (const i of idxs) {
      for (let c = 0; c < 3; c++) {
        if (bounds[i].min[c] < min[c]) min[c] = bounds[i].min[c];
        if (bounds[i].max[c] > max[c]) max[c] = bounds[i].max[c];
      }
    }
    return { instances: idxs.map((i) => instances[i]), bounds: { min, max } };
  });
  clusters.sort((a, b) => (a.bounds.min[0] - b.bounds.min[0]) || (a.bounds.min[2] - b.bounds.min[2]));
  return clusters;
}

/**
 * Shift a merged variant so its BASE sits at local x=0, z=0: take the x/z
 * centroid of the lowest vertices (within max(2 cm, 2% of height) of min Y)
 * and subtract it from every x/z. Y is untouched so y=0 stays the ground
 * plane the source models were authored on.
 */
function recenterOnBase(merged) {
  const pos = merged.positions;
  let minY = Infinity, maxY = -Infinity;
  for (let i = 1; i < pos.length; i += 3) {
    if (pos[i] < minY) minY = pos[i];
    if (pos[i] > maxY) maxY = pos[i];
  }
  const band = minY + Math.max(0.02, 0.02 * (maxY - minY));
  let sx = 0, sz = 0, n = 0;
  for (let i = 0; i < pos.length; i += 3) {
    if (pos[i + 1] <= band) { sx += pos[i]; sz += pos[i + 2]; n++; }
  }
  // n >= 1 always: the min-Y vertex itself is inside the band.
  const cx = sx / n, cz = sz / n;
  for (let i = 0; i < pos.length; i += 3) { pos[i] -= cx; pos[i + 2] -= cz; }
  return { cx, cz };
}

/** Material index (into gltf.materials) covering the most triangles of the cluster, or -1. */
function dominantMaterial(gltf, instances) {
  const triByMat = new Map();
  for (const { mesh } of instances) {
    for (const prim of mesh.primitives) {
      const count = (prim.indices !== undefined
        ? gltf.accessors[prim.indices].count
        : gltf.accessors[prim.attributes.POSITION].count) / 3;
      const key = prim.material !== undefined ? prim.material : -1;
      triByMat.set(key, (triByMat.get(key) || 0) + count);
    }
  }
  let best = -1, bestTris = -1;
  for (const [mat, tris] of triByMat) {
    if (tris > bestTris) { bestTris = tris; best = mat; }
  }
  return best;
}

/**
 * Build the minimal materials/textures/images/samplers sections for a variant
 * file: ONE material carrying the source material's base-color texture (plus
 * alpha/double-sided/factor settings), referencing the ORIGINAL textures/
 * folder by the source's own relative URI. Variants are written into the same
 * folder as the source .gltf, so that URI resolves unchanged — asserted here
 * with an existence check from the variant's location. Normal/ARM textures
 * and KHR_* extensions are deliberately dropped: the engine loader reads only
 * base color, and dropping them keeps gltf::import from decoding unused jpgs.
 * Returns null when the cluster has no textured material.
 */
function buildMaterialBlock(gltf, dir, matIdx) {
  if (matIdx < 0 || !gltf.materials || !gltf.materials[matIdx]) return null;
  const src = gltf.materials[matIdx];
  const pbrSrc = src.pbrMetallicRoughness || {};
  const bct = pbrSrc.baseColorTexture;
  if (!bct) return null;
  const tex = gltf.textures[bct.index];
  const img = gltf.images[tex.source];
  if (!img || !img.uri || img.uri.startsWith('data:')) return null;
  const resolved = path.join(dir, decodeURIComponent(img.uri));
  if (!fs.existsSync(resolved)) {
    throw new Error(`base-color texture URI does not resolve from variant location: ${img.uri}`);
  }

  const pbrOut = { baseColorTexture: { index: 0 } };
  if (pbrSrc.baseColorFactor) pbrOut.baseColorFactor = pbrSrc.baseColorFactor;
  if (pbrSrc.metallicFactor !== undefined) pbrOut.metallicFactor = pbrSrc.metallicFactor;
  if (pbrSrc.roughnessFactor !== undefined) pbrOut.roughnessFactor = pbrSrc.roughnessFactor;
  const material = { name: src.name || 'material', pbrMetallicRoughness: pbrOut };
  if (src.alphaMode) material.alphaMode = src.alphaMode;
  if (src.alphaCutoff !== undefined) material.alphaCutoff = src.alphaCutoff;
  if (src.doubleSided) material.doubleSided = true;

  const block = {
    materials: [material],
    textures: [tex.sampler !== undefined ? { sampler: 0, source: 0 } : { source: 0 }],
    images: [img.mimeType ? { mimeType: img.mimeType, uri: img.uri } : { uri: img.uri }],
    textureUri: img.uri,
    materialName: src.name || 'material',
  };
  if (tex.sampler !== undefined) block.samplers = [gltf.samplers[tex.sampler]];
  return block;
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

/**
 * Write merged arrays as a minimal single-mesh single-primitive glTF + bin
 * named <baseName>.gltf/.bin. `opts.material` (from buildMaterialBlock) adds
 * minimal materials/textures/images/samplers sections and points the
 * primitive at material 0; `opts.generator` overrides the asset generator tag.
 */
function writeMerged(outDir, baseName, merged, opts = {}) {
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

  const primitive = { attributes: { POSITION: 0, NORMAL: 1, TEXCOORD_0: 2 }, indices: 3, mode: 4 };
  if (opts.material) primitive.material = 0;

  const gltfOut = {
    asset: {
      generator: opts.generator || 'HumanityOS scripts/repack-plant-gltf.js (all primitives merged, transforms baked)',
      version: '2.0',
    },
    scene: 0,
    scenes: [{ name: 'Scene', nodes: [0] }],
    nodes: [{ mesh: 0, name: baseName }],
    meshes: [{ name: baseName, primitives: [primitive] }],
    accessors: [
      { bufferView: 0, componentType: CT_F32, count: totalVerts, type: 'VEC3', min: round(min), max: round(max) },
      { bufferView: 1, componentType: CT_F32, count: totalVerts, type: 'VEC3' },
      { bufferView: 2, componentType: CT_F32, count: totalVerts, type: 'VEC2' },
      { bufferView: 3, componentType: useU32 ? CT_U32 : CT_U16, count: idxOut.length, type: 'SCALAR' },
    ],
    bufferViews,
    buffers: [{ byteLength: bin.length, uri: `${baseName}.bin` }],
  };
  if (opts.material) {
    gltfOut.materials = opts.material.materials;
    gltfOut.textures = opts.material.textures;
    gltfOut.images = opts.material.images;
    if (opts.material.samplers) gltfOut.samplers = opts.material.samplers;
  }

  const gltfPath = path.join(outDir, `${baseName}.gltf`);
  const binPath = path.join(outDir, `${baseName}.bin`);
  fs.writeFileSync(binPath, bin);
  fs.writeFileSync(gltfPath, JSON.stringify(gltfOut, null, 2) + '\n');
  return { gltfPath, binPath };
}

// ---------------------------------------------------------------------------
// Per-model driver + self-verification
// ---------------------------------------------------------------------------

function findSourceGltf(dir) {
  const files = fs.readdirSync(dir).filter(
    (f) => f.endsWith('.gltf') && !f.endsWith('_merged.gltf') && !/_v\d+\.gltf$/.test(f)
  );
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
  const { gltfPath, binPath } = writeMerged(dir, `${slug}_merged`, merged);

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

/**
 * Split one model into per-variant files (<slug>_v1.gltf/.bin, ...): cluster
 * mesh nodes by X/Z, merge each cluster to a single primitive, re-center on
 * its base, attach the dominant material's base-color texture, write, and
 * verify (re-parse; per-variant index range; variant triangle sum must equal
 * the source-primitive total).
 */
function processModelSplit(dir) {
  const slug = path.basename(dir);
  const srcPath = findSourceGltf(dir);
  console.log(`\n${slug}: splitting ${path.basename(srcPath)} into variants`);

  const gltf = JSON.parse(fs.readFileSync(srcPath, 'utf8'));
  const buffers = loadBuffers(gltf, dir);
  const instances = collectMeshInstances(gltf);

  // Ground truth: triangle total over ALL primitives of ALL scene meshes.
  let sourceTriangles = 0;
  for (const { mesh } of instances) {
    for (const prim of mesh.primitives) {
      const count = prim.indices !== undefined
        ? gltf.accessors[prim.indices].count
        : gltf.accessors[prim.attributes.POSITION].count;
      sourceTriangles += count / 3;
    }
  }

  const clusters = clusterInstances(gltf, buffers, instances);

  // Remove stale variant files (idempotent re-runs; variant count may change).
  const stalePattern = new RegExp(`^${slug}_v\\d+\\.(gltf|bin)$`);
  for (const f of fs.readdirSync(dir)) {
    if (stalePattern.test(f)) fs.unlinkSync(path.join(dir, f));
  }

  const variants = [];
  let triSum = 0;
  clusters.forEach((cluster, ci) => {
    const baseName = `${slug}_v${ci + 1}`;
    const merged = mergeInstances(gltf, buffers, cluster.instances);
    const offset = recenterOnBase(merged);
    const matIdx = dominantMaterial(gltf, cluster.instances);
    const material = buildMaterialBlock(gltf, dir, matIdx);
    if (!material) console.warn(`  WARNING: ${baseName} has no textured material to carry over`);

    const { gltfPath, binPath } = writeMerged(dir, baseName, merged, {
      material,
      generator: 'HumanityOS scripts/repack-plant-gltf.js --split (one variant, base re-centered, transforms baked)',
    });

    // VERIFY: re-parse the file we just wrote with the same reader.
    const reGltf = JSON.parse(fs.readFileSync(gltfPath, 'utf8'));
    const reBuffers = loadBuffers(reGltf, dir);
    const rePrim = reGltf.meshes[0].primitives[0];
    const rePos = readAccessor(reGltf, reBuffers, rePrim.attributes.POSITION);
    const reIdx = readAccessor(reGltf, reBuffers, rePrim.indices);
    const verts = rePos.length / 3;
    const tris = reIdx.length / 3;
    for (const idx of reIdx) {
      if (idx >= verts) throw new Error(`VERIFY FAILED: ${baseName} index ${idx} out of range (${verts} vertices)`);
    }
    const { min, max } = computeMinMax(rePos);
    // Base-centering sanity: local origin must sit inside the XZ footprint.
    if (min[0] > 1e-4 || max[0] < -1e-4 || min[2] > 1e-4 || max[2] < -1e-4) {
      throw new Error(`VERIFY FAILED: ${baseName} XZ origin outside footprint (min ${min}, max ${max})`);
    }
    triSum += tris;

    const round3 = (arr) => arr.map((v) => Math.round(v * 1000) / 1000);
    variants.push({
      file: `${slug}/${baseName}.gltf`,
      verts,
      tris,
      bbox_min: round3(min),
      bbox_max: round3(max),
      base_centered: true,
      material: material ? material.materialName : null,
      texture: material ? `${slug}/${material.textureUri}` : null,
    });

    const fmt = (arr) => '[' + arr.map((v) => v.toFixed(3)).join(', ') + ']';
    console.log(
      `  ${baseName}: ${merged.sourceMeshes} node(s), ${verts} verts, ${tris} tris, ` +
      `base offset (${offset.cx.toFixed(3)}, ${offset.cz.toFixed(3)}), bounds min ${fmt(min)} max ${fmt(max)}, ` +
      `material ${material ? material.materialName : 'none'} ` +
      `(${(fs.statSync(binPath).size / 1024 / 1024).toFixed(2)} MB)`
    );
  });

  if (triSum !== sourceTriangles) {
    throw new Error(`VERIFY FAILED: variant triangles sum to ${triSum}, source primitives sum to ${sourceTriangles}`);
  }
  console.log(`  ${variants.length} variant(s); triangle sum ${triSum} matches source total: OK`);

  return { slug, variants, triSum };
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

/** Record each model's per-variant files in manifest.json (idempotent). */
function updateManifestVariants(results) {
  const manifestPath = path.join(PLANTS_DIR, 'manifest.json');
  if (!fs.existsSync(manifestPath)) return;
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  let touched = 0;
  for (const res of results) {
    const entry = (manifest.assets || []).find((a) => a.slug === res.slug);
    if (!entry) continue;
    entry.variants = res.variants;
    touched++;
  }
  if (touched > 0) {
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
    console.log(`\nmanifest.json: updated "variants" list on ${touched} asset(s)`);
  }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function main() {
  const rawArgs = process.argv.slice(2);
  const splitMode = rawArgs.includes('--split');
  const args = rawArgs.filter((a) => a !== '--split');
  if (args.length === 0) {
    console.error('usage: node scripts/repack-plant-gltf.js [--split] <folder|slug> [...] | --all');
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
      results.push(splitMode ? processModelSplit(dir) : processModel(dir));
    } catch (err) {
      failures.push({ slug: path.basename(dir), error: err.message });
      console.error(`\n${path.basename(dir)}: FAILED - ${err.message}`);
    }
  }

  if (splitMode) updateManifestVariants(results);
  else updateManifest(results);

  // Summary table
  if (results.length > 0 && splitMode) {
    console.log('\n=== summary (split) ===');
    const pad = (s, n) => String(s).padEnd(n);
    console.log(pad('variant', 28) + pad('verts', 10) + pad('tris', 10) + pad('size (x, y, z) m', 22) + 'material');
    for (const r of results) {
      for (const v of r.variants) {
        const dims = v.bbox_max.map((hi, c) => hi - v.bbox_min[c]);
        console.log(
          pad(path.basename(v.file, '.gltf'), 28) + pad(v.verts, 10) + pad(v.tris, 10) +
          pad(dims.map((d) => d.toFixed(2)).join(' x '), 22) + (v.material || 'none')
        );
      }
    }
  } else if (results.length > 0) {
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
