# Terrain draw batching

Status: increment 1 SHIPPED (v0.1001.0, 2026-07-27). Owner: renderer.
Operator approval: "Let's do the proper fix for the terrain gen" (2026-07-26,
after the 13 FPS empty-desert report at patch budget 12,288).

## The problem

Every chunked-LOD terrain patch used to be its own RenderObject with its own
GPU vertex + index buffers. At a 12,288 patch budget the celestial pass
submitted ~12.9k patches as ~50k render-pass commands per frame (per patch:
one dynamic-offset bind-group set, one vertex-buffer bind, one index-buffer
bind, one draw), plus two identical 3 MB object-uniform uploads (shadow pass
and celestial pass) that each rebuilt ~12.9k model matrices AND their
inverse-transposes on the CPU. wgpu command encoding costs about 1-2
microseconds per command on DX12, so submission alone accounted for most of
the 77 ms frame the operator measured (13 FPS in an EMPTY desert, no trees).

## The fix (increment 1)

One mega-buffer arena for all batched patches:

- `src/renderer/patch_arena.rs`: one shared vertex buffer (~1.28 GB) + one
  shared index buffer (~192 MB), range-allocated (first-fit free list with
  merge on free) because patch sizes VARY (the base grid is a fixed 1056
  vertices, but vegetation cards are appended into the same patch mesh).
  Patch builds upload with `queue.write_buffer` into their ranges, so patch
  streaming no longer creates or destroys ANY GPU buffer.
- Per-patch data (anchor translation + LOD crossfade) rides a storage array
  (`PatchInstance`, 16 bytes each) indexed by `@builtin(instance_index)`.
  The shared planet rotation is one batch uniform.
- The draw loop binds pipeline + bind groups + buffers ONCE, then issues one
  `draw_indexed(index_range, base_vertex, i..i+1)` per patch. The instance
  range i..i+1 makes instance_index equal i inside the shader with no extra
  device features.
- The shadow pass reuses the same arena, instance buffer, and full-list
  instance indices with the existing 6 km caster cull.
- The object-uniform staging shrinks from ~12.9k entries to the couple dozen
  actual celestial bodies, and the ~25k per-frame CPU matrix inversions for
  patches are gone (the batch rotation is computed once).

### Why direct draws and not multi_draw_indexed_indirect

The standard GPU-driven approach packs draw args into a buffer and issues one
`multi_draw_indexed_indirect`, using `first_instance` as the per-draw object
id. That requires instance_index to respect first_instance in INDIRECT draws,
and this project's primary DX12 adapter reports that downlevel flag MISSING
(`VERTEX_AND_INSTANCE_INDEX_RESPECTS_RESPECTIVE_FIRST_VALUE_IN_INDIRECT_DRAW`,
boot log 2026-07-27). Direct draws with an explicit instance range are core
WebGPU everywhere, need no feature requests (no v0.782-class boot risk), and
already collapse per-patch cost to one command. Indirect multi-draw remains a
future increment for adapters that support it; the buffers and shader are
already shaped for it (see Future below).

### Shader variant

The megashader now routes ALL object-uniform access through three accessors
(`obj_model()`, `obj_normal_matrix()`, `obj_lod_fade()`), defined inside a
marker-delimited block in `00-bindings-vertex.wgsl`:

- CLASSIC block: reads the group-1 dynamic-offset uniform, exactly as before.
- BATCH block (`shader_loader::batched_variant_of` swaps it in by marker
  substitution): reads the storage array + batch uniform. `obj_model()`
  rebuilds the matrix as rotation + instance translation, with the fade in
  the model[0].w metadata slot, preserving the classic contract.

Both stages set the module-private `g_inst` (vertex: instance_index;
fragment: a new flat varying), so shared shader code is identical between
variants. The variant derives from the SAME on-disk source at hot-reload
time, so shader edits keep applying to both pipeline families (now 6 PSOs).
Both assembled variants are naga-validated in tests
(`batched_variant_parses_and_validates`) and at every hot reload.

### Graceful degradation

`PatchArena::upload` returns None when the arena is full (capacity follows
the device's real max_buffer_size, so small adapters get smaller arenas).
The caller then builds a classic per-patch Mesh exactly as before, and the
cache entry records mesh-index-instead-of-slot. The draw split sends slotted
entries to the batch and classic entries through the old RenderObject path.
Overflow logs once per pressure episode (`[PatchArena] ... arena full`);
the 5 s `[PatchBatch]` diag shows the batched vs classic split.

### Real byte accounting (bonus fix)

Cache inserts now pass REAL geometry bytes (vertex count x 32 + index count
x 4) instead of the flat `PATCH_MESH_BYTES` estimate. Forest patches with
appended vegetation cards are much bigger than the estimate, so the old
accounting silently overshot the 1.5 GB VRAM cap; the LRU now tracks truth,
which also keeps the arena and the cache cap in agreement by construction.

## What stayed classic

- Water shell patches: alpha-blended (transparent pipeline, no depth write),
  a few hundred draws at most. Batching transparents needs ordering care;
  not worth it yet.
- All non-patch celestial objects (bodies, atmo shells, cloud shells).

## Future increments

1. `multi_draw_indexed_indirect` behind a feature check (Vulkan or any
   adapter with INDIRECT_FIRST_INSTANCE + the downlevel flag): the per-patch
   loop becomes one command. Args buffer is a trivial extension of the
   current instance upload.
2. GPU frustum culling of patches (compute pass writes the indirect args),
   removing the CPU selection's draw-list cost at very high budgets.
3. Tree instancing rides the same arena pattern (reserved billboard-bake
   arc; see PRIORITIES).

## Verification

- `cargo test --features native --lib` 1167 passed (arena allocator unit
  tests: roundtrip, merge, fragmentation, exhaustion, churn-never-leaks).
- Both shader variants naga-validate.
- Boot-verified on the release exe (the v0.782 bar: renderer changes are
  only proven by booting).
- Rig FPS A/B at patch budget 12,288: see the release notes of v0.1001.0.
