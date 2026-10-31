# humanity-terrain

## What it is for

Planets as shapes and numbers. It holds the planet's surface and heights (`planet`,
`planet_heightmap`, `procedural_heightmap`), the level-of-detail patches and their meshes as plain
data (`planet_chunks`, `drawn_surface`, `terrain_tiles`), the oceans and water (`ocean_fft`,
`ocean_waves`, `water_patches`), the plants' and trees' geometry (`plant_mesh`, `tree_registry`,
`near_trees`, `far_trees`, `grass`), real map regions (`osm_region`), asteroids and the celestial
projection. It draws nothing itself: the renderer uploads what it builds and the simulation reads
its heights. It was edited twice in the 30 days before the split was planned, so as a crate under
both it compiles once and stays built.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - no gate (every build): `terrain/`
- **Size:** 25,638 lines in 29 files, 330 of them in 1 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `terrain/planet_chunks.rs` 4,635 of 4,740, `terrain/grass.rs` 2,564 of 2,640, `terrain/grass_mesh.rs` 948 of 1,000, `terrain/drawn_surface.rs` 1,023 of 1,100, `terrain/water_patches.rs` 323 of 450, `terrain/grass_fields.rs` 238 of 350.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 16.
- **May use** (the plan): core. **Uses today** (references in non-test code): core 4.
- **May be used by** (the plan): systems, relay, renderer, gui, engine. **Used today by:** engine 131, renderer 33, gui 25, systems 7.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 5 in test code, to systems 3, gui 1, renderer 1: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted").
- **Its lib tests:** 256 tests, and this runs exactly them, checked against every test's name:

```
just test-lib terrain::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `terrain::planet::PlanetDef` | 14 | engine 10, systems 2, gui 1, renderer 1 |
| `terrain::planet_heightmap::PlanetHeightmap` | 10 | engine 9, systems 1 |
| `terrain::tree_registry::registry` | 9 | engine 6, renderer 3 |
| `terrain::planet_chunks::DetailNoise` | 8 | engine 8 |
| `terrain::terrain_tiles::TerrainTiles` | 7 | engine 6, systems 1 |
| `terrain::icosphere::Icosphere` | 6 | engine 5, renderer 1 |
| `terrain::ocean_mask::OceanMask` | 6 | engine 5, systems 1 |
| `terrain::ocean_waves::SURFACE_LIFT_M` | 5 | engine 5 |
| `terrain::planet_chunks` | 5 | engine 4, systems 1 |
| `terrain::planet_albedo::PlanetAlbedo` | 4 | engine 4 |
| `terrain::planet_chunks::PatchSlot` | 4 | renderer 4 |
| `terrain::planet_surface::SurfaceMeshData` | 4 | engine 2, renderer 2 |
| `terrain::plant_mesh` | 4 | engine 2, renderer 2 |
| `terrain::grass::GRASS_COVER_MAX` | 3 | gui 3 |
| `terrain::grass::GRASS_FAR_M` | 3 | gui 3 |

And 73 more items, 104 references (`node scripts/crate-cards.js --interface humanity-terrain` lists them all).
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
  [credits.rs](../../src/credits.rs) and `data/credits.ron`.
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
- Rigs boot one game at a time, in the background: [game-boots.md](../ai/game-boots.md).

## Open work

- **BUG-156's known limits** (the bug is fixed; [BUGS.md](../BUGS.md)): a far tree can sit off its
  own patch by that patch's level-of-detail error (re-grounding each tree on the leaf drawn under it
  would close it); the harvest does not re-run when a terrain tile arrives; inside a height-survey
  region with no tile tier installed, the ground draws with 54 m triangles while the region's
  buildings and roads stand on the finest ground.
- **`planet_chunks.rs` still mixes the height math with building the patch meshes.** The plan had it
  split in two for the line between terrain and the renderer ([crate-split.md](../design/crate-split.md),
  "Split terrain from the renderer?"); rung 0 moved the shared types down instead, so the split is
  now a question of size, not of the line (its budget is in At a glance).
- **Rung 2 cuts terrain** with ship, before systems; its test code's wrong-way references (At a
  glance) move first. Source: [crate-split.md](../design/crate-split.md), "Order of work".

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
