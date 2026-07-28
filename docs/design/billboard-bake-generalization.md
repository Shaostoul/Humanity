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

## Trees-from-altitude gap (analysis 2026-07-27, operator field report)

Report: "as I fly up I can easily see a lot of trees aren't loaded...
maybe the code thinks I'm facing left/right of where I'm actually
facing?" Root cause is structural, not a facing bug:

- Trees bake INTO patch meshes and only exist at patch depth >= 15
  (TREE_MIN_DEPTH, 215 m patches). A patch stays resident while its
  projected edge exceeds terrain_split_px (default 4 px), so depth-15
  patches - and therefore ALL trees - exist only within a slant range of
  roughly 65 km at 1440p defaults (215 m * screen_h/(2 tan(fov/2)) /
  4 px). Above ~65 km altitude no tree exists anywhere on the planet.
- The deep-patch ring centers on the camera's NADIR (selection is
  screen-size-driven, view-direction-agnostic), while the operator looks
  toward the horizon - so the treeless zone dominates the view and reads
  as a mis-aimed loader. Nothing is mis-aimed; the mechanism just cannot
  serve far trees.

Why NOT a sparse mid-depth card stopgap (evaluated + rejected
2026-07-27): emitting a deterministic 1/16 hash-subset of cards into
depth-13 patches would fill the ring out to ~260 km, but each such patch
carries the card count of a full depth-15 patch (cells are planet-fixed,
so per-area density is constant), the frame is vertex-bound exactly
there (the v0.1015 sharing win bought that headroom for terrain, not for
4x more card quads), and the instancing rung below replaces the whole
mechanism - the stopgap would be torn out, classic throwaway.

What the FAR-TREE INSTANCING rung must deliver instead (this arc):
- Instanced billboard quads DECOUPLED from patch meshes: one instance
  buffer over the visible disc, driven by the SAME planet-fixed cell
  hash (deterministic agreement with the baked-in near cards), riding
  the patch-arena buffer pattern + one indirect draw.
- Density decimation by distance is mandatory, not optional: full
  TREES_PER_CELL (~16k/km^2) over a 65 km disc is ~200M instances.
  A hash-fraction ladder (e.g. full < 5 km, 1/16 to 20 km, 1/256 to
  100 km with 2-3x card scale to preserve visual mass, nothing past
  ~150 km where a tree is sub-pixel from any altitude) keeps instance
  counts in the low millions worst case, and the fraction test is one
  compare on the existing per-tree hash.
- The [TreeHandoff] 1 Hz diag (lib.rs) already reports near/drawn/
  covered/window/hide numbers to calibrate the ladder against.

Follow-up field evidence (2026-07-27 operator screenshots, ~1.7-2.6 km
straight down): a SQUARE patch of dense trees hard-edged at patch borders
(tree cards baked into the one resident deep patch; neighbors too shallow
to carry any) overlapping a CIRCULAR field of sparser tree dots centered
under the camera (the distance-gated near ring). Operator read it as "two
sets of tree gen code" - exactly the mechanism seam this arc's instanced
far trees erase: one planet-fixed hash population, one distance-driven
density ladder, no patch-shaped or radius-shaped boundaries.
