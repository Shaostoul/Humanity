# humanity-terrain

## What it is for

Planets as shapes and numbers. It holds the planet's surface and heights (`planet`,
`planet_heightmap`, `procedural_heightmap`), the level-of-detail patches and their meshes as plain
data (`planet_chunks`, `drawn_surface`, `terrain_tiles`), the oceans and water (`ocean_fft`,
`ocean_waves`, `water_patches`), the plants' and trees' geometry (`plant_mesh`, `tree_registry`,
`near_trees`, `far_trees`, `grass`), the whole-Earth map: its tile key (`map_tiles`), its pack
format (`map_pack`), its data files (`map_levels`), the packs drawn into the ground (`map_ground`:
the shore band, tile residency, the built-over mask; its child `map_beds`, the road beds and
building pads), the offline builder (`map_build`, behind the
`map-build` feature, built only for tests and `just map-packs`) and OSM features around an origin
with the 3D extrusion mesher (`osm_region`), asteroids and the celestial projection. It draws nothing itself: the renderer uploads what it builds and the simulation reads
its heights. It was edited twice in the 30 days before the split was planned, so as a crate under
both it compiles once and stays built.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-terrain/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-terrain/src/asteroid.rs`, `crates/humanity-terrain/src/heightmap.rs`, `crates/humanity-terrain/src/icosphere.rs`, `crates/humanity-terrain/src/planet.rs`, `crates/humanity-terrain/src/planet_albedo.rs`, `crates/humanity-terrain/src/planet_chunks.rs`, `crates/humanity-terrain/src/grass.rs`, `crates/humanity-terrain/src/drawn_surface.rs`, `crates/humanity-terrain/src/far_trees.rs`, `crates/humanity-terrain/src/tree_registry.rs`, `crates/humanity-terrain/src/plant_mesh.rs`, `crates/humanity-terrain/src/near_tree_gate.rs`, `crates/humanity-terrain/src/osm_region.rs`, `crates/humanity-terrain/src/water_carve.rs`, `crates/humanity-terrain/src/ocean_events.rs`, `crates/humanity-terrain/src/ocean_fft.rs`, `crates/humanity-terrain/src/ocean_mask.rs`, `crates/humanity-terrain/src/ocean_waves.rs`, `crates/humanity-terrain/src/terrain_tiles.rs`, `crates/humanity-terrain/src/planet_heightmap.rs`, `crates/humanity-terrain/src/procedural_heightmap.rs`, `crates/humanity-terrain/src/planet_surface.rs`, `crates/humanity-terrain/src/celestial_projection.rs`
  - test builds only: `crates/humanity-terrain/src/crossing_tests.rs`
- **Size:** 25,607 lines in 31 files, 298 of them in 2 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `crates/humanity-terrain/src/planet_chunks.rs` 4,630 of 4,740, `crates/humanity-terrain/src/grass.rs` 2,564 of 2,640, `crates/humanity-terrain/src/grass_mesh.rs` 948 of 1,000, `crates/humanity-terrain/src/drawn_surface.rs` 1,029 of 1,100, `crates/humanity-terrain/src/water_patches.rs` 323 of 450, `crates/humanity-terrain/src/grass_fields.rs` 238 of 350.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 16, `test-support` 7.
- **May use** (the plan): core. **Uses today** (references in non-test code): core 4.
- **May be used by** (the plan): systems, relay, renderer, gui, engine. **Used today by:** engine 131, renderer 33, gui 25, systems 7.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 255 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-terrain
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_terrain::`):

| Item | References | From |
|---|---:|---|
| `planet::PlanetDef` | 12 | engine 8, systems 2, gui 1, renderer 1 |
| `planet_heightmap::PlanetHeightmap` | 10 | engine 9, systems 1 |
| `tree_registry::registry` | 9 | engine 6, renderer 3 |
| `planet_chunks::DetailNoise` | 8 | engine 8 |
| `terrain_tiles::TerrainTiles` | 7 | engine 6, systems 1 |
| `icosphere::Icosphere` | 6 | engine 5, renderer 1 |
| `ocean_mask::OceanMask` | 6 | engine 5, systems 1 |
| `ocean_waves::SURFACE_LIFT_M` | 5 | engine 5 |
| `planet_chunks` | 5 | engine 4, systems 1 |
| `planet_albedo::PlanetAlbedo` | 4 | engine 4 |
| `planet_chunks::PatchSlot` | 4 | renderer 4 |
| `planet_surface::SurfaceMeshData` | 4 | engine 2, renderer 2 |
| `plant_mesh` | 4 | engine 2, renderer 2 |
| `grass::GRASS_COVER_MAX` | 3 | gui 3 |
| `grass::GRASS_FAR_M` | 3 | gui 3 |

And 74 more items, 106 references (`node scripts/crate-cards.js --interface humanity-terrain` lists them all).
<!-- end generated -->

## Rules

- **f32 at planet scale is a defect class.** Keep planet positions AND unit directions in f64
  (`DVec3`) all the way to a terrain or data sampler, downcast only at a coarse-data boundary, and
  check the whole chain, callers included. Source: [renderer.md](../ai/renderer.md), "f32 at planet
  scale" (with the worked example).
- **Build the 2030 technique, not a placeholder:** spline-lofted welded tree geometry, real
  heights, real oceans; when unsure which technique that is, ask the fidelity-expert before
  fencing the increment. Open-source methods only, no proprietary tool licences. Source:
  [CLAUDE.md](../../CLAUDE.md), "Realistic-first" and "Dual modes, open source, and a compute budget".
- **Every effect states its per-frame budget.** The patch builds already have one and are spread
  across frames; anything new here that runs per frame does the same. Source:
  [CLAUDE.md](../../CLAUDE.md) (the compute budget) and [sim-frame-cost.md](../design/sim-frame-cost.md),
  Stutter.
- **No caches kept across runs** (no baked meshes or heights on disk to speed a boot): make the cold
  path fast, and any reuse across frames asks `reuse::enabled()`. Source:
  [CLAUDE.md](../../CLAUDE.md), "No caches kept across runs during development".
- **Data, not code:** the tree species are `data/vegetation/trees.ron`; a new species is a row.
  Source: [infinite-of-x.md](../design/infinite-of-x.md).
- **Map data from OpenStreetMap carries its attribution** (`credits::OSM_NOTICE`, "Map data (c)
  OpenStreetMap contributors (ODbL)"): it is a licence obligation, not a courtesy. Source:
  [credits.rs](../../crates/humanity-core/src/credits.rs) and `data/credits.ron`.
- **Map data is keyed by the terrain's own patches.** A map tile's id is a `PatchId`; the tile a
  patch reads is its ancestor at a data level (`map_tiles::ancestor_at`, a mask), and the levels are
  data, passed in, never a constant. A height sample is a lattice vertex built by the tree's own
  normalized-midpoint subdivision (`map_tiles::tile_lattice`), never a barycentric blend, so coarse
  and fine samples are the same bits. A clip crossing is computed from the ORIGINAL feature edge,
  so neighbouring tiles agree bit for bit. Source:
  [whole-earth-maps.md](../design/whole-earth-maps.md), sections 3 and 4.2.
- **Nothing derived from the land layer is stored in a pack** (section 3.4: a pack is a Collective
  Database). Road beds and building pads are computed at run time from the land layer, per road
  PIECE pinned to the natural ground at both of its ends (where a road crosses a tile border both
  tiles hold the same end point), so every tile computes the same bed without a margin. They run
  before the shore band, which wins within its band. Source:
  [whole-earth-maps.md](../design/whole-earth-maps.md), section 6.4.
- **What it builds reaches pixels, so prove it there:** a change to patches, water or plants is
  checked by booting and looking, never by a static check alone. Source:
  [renderer.md](../ai/renderer.md), "Verify by booting and entering the world".

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance. The trees' card gates are the
  renderer's (`renderer/tree_mesh.rs`), so a change to `tree_registry` or `plant_mesh` runs
  `just test-lib renderer::tree_mesh::` too.
- **Boot and enter the world:** `just verify-runtime` (one of its three vantages is a textured
  forest ground).
- **See it:** `just probe-sweep` captures every canonical vantage with its frame rate;
  `just perf-sweep`, then `just perf-diff <old manifest>`, for cost; the `visual-sweep` skill judges
  the captures against their golden specs.
- **Trees on the ground:** [tree-ground-check.test.js](../../scripts/tests/tree-ground-check.test.js)
  (in `just rig-tests`) holds the probe sweep's ground check, written after BUG-156.
- **The map tile key:** `just test-lib --crate humanity-terrain map_tiles` (containment against
  `drawn_leaf_containing`, the lattice against `patch_corners`, crossings bit for bit, clipped
  areas to 1e-9, the codec within 6 mm; each test's comment names the mutation it was seen red
  against).
- **Coasts:** `just test-lib --crate humanity-terrain map_ground` runs the shore rules through the
  walk oracle on the shipped home packs at depths 13 and 20 (no boot). The four `coast-*` vantages
  carry a `shore_probe` (engine/shore_probe.rs, judged by
  [shore-probe-check.js](../../scripts/lib/shore-probe-check.js), in `just rig-tests`):
  `just probe-sweep --only coast-silverdale-waterfront,coast-silverdale-old-edge,coast-dover-cliffs,coast-norderney-beach`.
  Their red and green readings are in each vantage's `_red` and `_green` notes.
- **Road beds and building pads:** `just test-lib --crate humanity-terrain map_ground::beds` (sloped
  streets flat across, sloped footprints on their floor, a road across a tile border, coarse patches
  untouched, grades, the coast owning the waterfront street; each with a beds-off negative control).
  The vantages `silverdale-road-slope` and `silverdale-building-slope` carry a `bed_probe`
  (engine/bed_probe.rs, judged by [bed-probe-check.js](../../scripts/lib/bed-probe-check.js)); their red
  run is `HUMANITY_GROUND_BEDS=0`. A patch build's cost by depth:
  `cargo run --release -p humanity-terrain --features test-support --example bench_map_patches`.
- **The builder:** `just test-lib --crate humanity-terrain map_build` (its readers and the sea's
  border walk; the GEDTM30 summit test reads the real window on the operator's drive and says so
  when it is absent); `just map-packs` rebuilds the home packs, twice, compared byte for byte.
- Rigs boot one game at a time, in the background: [game-boots.md](../ai/game-boots.md).

## Open work

- **BUG-156's known limits** (the bug is fixed; [BUGS.md](../BUGS.md)): a far tree can sit off its
  own patch by that patch's level-of-detail error (re-grounding each tree on the leaf drawn under it
  would close it); the harvest does not re-run when a terrain tile arrives; inside a height-survey
  map pack with no tile tier installed, the ground draws with 54 m triangles while the districts'
  buildings and roads stand on the finest ground.
- **Whole-Earth maps after rung 2** ([whole-earth-maps.md](../design/whole-earth-maps.md), section
  13 and the rung 2 outcome): road beds and building pads in the one formula (deferred from rung 2),
  river surfaces that fall downstream, the planet build (rung 3), the downloader (rung 4).
- **`planet_chunks.rs` still mixes the height math with building the patch meshes.** The plan had it
  split in two for the line between terrain and the renderer ([crate-split.md](../design/crate-split.md),
  "Split terrain from the renderer?"); rung 0 moved the shared types down instead, so the split is
  now a question of size, not of the line (its budget is in At a glance).
- **A crate of its own since rung 2 (2026-11-02):** `crates/humanity-terrain/`, the old
  `terrain` module as the crate root. The renderer, the other crate that uses it, is cut in rung 3.
  Source: [crate-split.md](../design/crate-split.md), "Order of work".

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
