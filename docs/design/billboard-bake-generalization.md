# Billboard bake generalization: any model gets a card

Status: DESIGN (2026-07-26, grounded in a code read of the shipped baker).
The conifer pipeline proved every stage; this note maps the increments
that extend it from "six hardcoded trees" to "any decorations.ron model,
and eventually any GLTF a modder drops in." Implementation wants a fresh
focused session (renderer + megashader + draw-path surgery - the same
discipline as the megashader source split).

## What already ships (v0.959-v0.970)

| Stage | Where | Notes |
|---|---|---|
| Side-on unlit baker | `renderer/billboard_bake.rs` `bake_billboard_texture` | Ortho along -Z, AABB-framed, 0.3 cutout, swapchain format |
| PNG dump for eyeballing | `bake_billboard_to_png` + showcase `"bake":"trees"` | Scripted verification path |
| Persistent atlas | `bake_tree_atlas` -> `tree_atlas_texture` (3x2 x 512 px) | In-place rewrite, group-3 binding 14 |
| Card consumer | type-12 sprite-card branch in the megashader (uv.x < -0.5 encoding) | Lit like terrain, alpha-test 0.5 |
| Distance swap | `tree_card_hide_m` / `tree_card_far_m` renderer knobs | Settings sliders live |

## The gap

Decorations (`data/entities/decorations.ron` -> `state.decoration_objects`)
always draw their full GLTF mesh at every distance - no card rung. The
tree atlas is FIXED 3x2 and its slot mapping is hardcoded to the six
conifers.

## Increment plan

1. **Dynamic atlas registry.** Replace the fixed 3x2 constants with an
   atlas allocator keyed by model id: `HashMap<String, (tile, footprint)>`
   over an NxN atlas (16 slots at 512 px = 2048^2, one texture). Bake at
   world load for each UNIQUE model in decorations.ron (the loader already
   caches mesh+texture per model - reuse those CPU buffers as BakeParts
   before upload). Persist nothing: bakes are ~ms each and hot-reload
   friendly.
2. **Card draw rung for decorations.** In the decoration draw loop
   (lib.rs), beyond `deco_card_m` (new slider, default ~120 m) emit a
   card quad (the tree-card vertex encoding, atlas tile from the registry)
   instead of the mesh; inside, the mesh as today. Reuse the type-12
   sprite-card shader branch unchanged if the uv encoding can carry the
   bigger tile index range (today |uv.x| = 1 + tile + u01*0.5 - fine up
   to dozens of tiles).
3. **Modder path.** Any model referenced by decorations.ron gets a card
   automatically - that IS the modder path (infinite-of-x: the data file
   names the model, the engine does the rest). Document in
   docs/user/creating/.
4. **Deferred polish** (explicitly out of first scope): multi-angle
   imposters (4/8-view tiles + view-dependent pick), supersampled bakes,
   normal-map capture for lit cards.

## Verification bar (per the v0.782 rule)

Full battery + boot + probe sweep at a vantage with decorations visible
near AND far + before/after FPS at the fuji-forest vantage. The bake
itself is provable headlessly via the existing showcase PNG dump.

## LOD registry tie-in

`data/lod/categories.ron` gains a "decorations" row once the card rung
ships, with the slider wired in Settings > Graphics like the tree
sliders (the v0.971 registry pattern).
