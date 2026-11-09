# NVIDIA SimReady props in the homes

**Asked for:** 2026-11-08, the operator, on the AI tools scan: "Let's go with it. Stuff we don't have
to make saves us time." **Status:** BUILT (phase 2, 2026-11-09). The operator said yes to
both questions of section 12 on 2026-11-08 ("Yes" to the download; "Sure, let's make sure it's all
easy to access" to the home set in the app and the Medical Bay set as the first pack). The 712 pinned
files were fetched and checked ([`scripts/simready/starter-set.json`](../../scripts/simready/starter-set.json)),
and `just simready` converts them. Section 13 records what the build found and changed from the plan.

**The conditions are binding** and come from the dated legal finding,
[`../reference/findings/2026-11-08-nvidia-simready-assets.md`](../reference/findings/2026-11-08-nvidia-simready-assets.md):
CC BY 4.0 on every asset; converting is a technical modification, anything more (merging, decimating,
baking, resizing, repainting) is an adaptation that SOURCE.md must list; credit in `data/credits.ron`,
`CREDITS.md`, `LICENSES.md` and a `LICENSE.txt` plus `SOURCE.md` in every asset folder; the MDL shaders
are never fetched or shipped; the two trademark-named assets (`consumable_lubricant_silicone_wd40_a01`,
`toys_rubiksCube_a01`) are never used; every chosen texture is looked at for printed brands before it
ships. These are the project's first 3D models that are not CC0.

## 1. What the engine can draw today, and what SimReady hands us

| | SimReady asset | Machine model path today (`src/assets/mod.rs`, `docs/game/model-pipeline.md`) |
|---|---|---|
| File | OpenUSD: a root layer, `payloads/geometries.usd` (binary meshes), `instances.usda`, `materials.usda`, a neutral `Physics/physics.usda`, solver variants (PhysX, Newton, MuJoCo) that default to Disabled | one `.glb`/`.gltf` per catalogue row (`model:` in `data/machines/*.ron`) |
| Meshes | many prims, often CAD-dense (`geometries.usd` up to 39 MiB in the starter set) | the FIRST mesh's FIRST primitive only |
| Materials | many per asset (the fridge carries 13 texture sets), mostly NVIDIA's shared TILING library (`t_metal_chrome_a01_tile_*` appears in 17 of the 94 candidates first scanned), some unique UV sets (`t_kettle_chrome_a01_*`); albedo, normal and ORM PNGs up to 100 MiB each (the stainless tile albedo) | base colour texture only, at most 1024 px, CLAMPED sampler, fixed roughness 0.85, never metallic |
| Scale | real size, stage units and up axis in the layer metadata | scaled on load so the model stands at the row's `size` height |
| Physics | UsdPhysics mass, centre of mass, inertia, collision approximations, physics materials, sometimes joints | Rapier's world exists but is dormant; walls collide geometrically (`crates/humanity-ship/src/wall_collision.rs`); machines have no mass and do not block walking |

So conversion is not a format swap: every asset needs merging, simplification, a texture bake and
its physics carried into data. The plan is built in rungs, each one a rung of the real thing
(section 5).

## 2. The starter set: 47 assets

Chosen from the dataset's file listing (Hugging Face tree API, metadata only), room by room, for
real gaps. **Nine catalogue rows that are plain boxes today, or draw the wrong object, get a model**
(from nine assets); the rest are new rows with no stats (furniture and props, which `data/machines/home.ron` says are free to add).

| Room | Asset (NVIDIA folder) | Fills |
|---|---|---|
| Kitchen | `fixture_sink_basin_a01` + `fixture_faucet_singleLever_a01` | `kitchen_sink` (a box); the tap is joined on only if the basin has none (decided at the look) |
| | `fixture_counter_a01` | `serving_counter` in `ship.ron` (a box) and the counter run beside the sink |
| | `appliance_toaster_a01`, `kitchenware_kettle_a01`, `_pan_a01`, `_cuttingBoard_a01`, `_knife_a01`, `_jug_a01`, `_ladle_a01`, `dishware_mug_a01`, `dishware_bowl_a01` | counter and shelf props; the kitchen has none |
| Bathroom | `toilet_a01`, `washBasin_a01` | `toilet`, `bath_sink` (boxes) |
| | `cleaning_barSoap_a01`, `cleaning_towel_a01` | props by the basin |
| Workshop | `furniture_workbench_modular_a01` | `workbench` and `electronics_bench` (boxes) |
| | `equipment_rolling_tool_chest_cabinet_combo_d01` | `tool_rack`, drawn today with a coat-rack model |
| | `equipment_bench_vise_a01`, `lighting_taskLight_adjustableArmBench_a01`, `testingTool_multimeter_digital_b01`, `ppe_eyeProtection_safetyGlasses_a01`, `ladder_step_folding_a01` | the bench vise, the bench lamp (homestead.md's "spotlight aimed at the workbench"), the electronics bench's meter, safety glasses, a step ladder |
| | `tool_hammer_a01`, `handTool_wrench_adjustable_a01`, `handTool_screwdriver_phillips_ph2_a01`, `handTool_pliers_combination_a01`, `handTool_knife_retractableBlade_a01` | world models for `items.csv` rows `hammer_0`, `wrench_adjustable_0`, `screwdriver_phillips_0`, `pliers_0`, `knife_utility_0` |
| Life support | `humidifier_a01` | `humidifier` in the mushroom room (a cylinder) |
| | `ibcTank_a02`, `drum_plastic_blue_a01` | a 1,000 L rain tote for the water loop; a 55-gallon drum for stored water or grain |
| Safety | `fireExtinguisher_abc_a03` + `fireExtinguisher_stand_a01`, `detector_smoke_fire_a01` | an extinguisher for the kitchen, workshop and forge, and a smoke alarm per room; the home has neither |
| Storage | `shelve_steel_heavyDuty_a06` | `shelf` in the pantry and workshop, drawn today with a low bookcase |
| | `crate_gray_a08`, `crate_plywood_b01` (`items.csv` `crate_0`), `pail_screwTop_white_glossy_a01`, `storage_locker_double_bench_b01` | stacking crates, a lidded grain bucket, the boot room's bench locker |
| Medical Bay | `bed_hospital_standard_a01`, `IV_pole_a01`, `stool_mobile_padded_vinyl_a01`, `cabinet_for_drugs_a01`, `screen_hospital_a01`, `table_metal_stainless_a03`, `thermometer_fever_a01`, `safety_firstAidKit_box_plastic_a01` | the Medical Bay at General Services (Fore Landing, `mothership-master-plan.md`), and a first aid kit for every home |

**What the library cannot fill, said plainly:** it has no stove, oven, shower, bathtub, washing
machine or water heater, and nothing like the life-support machines (CO2 scrubber, air handler, heat
pump, water purifier, battery bank, septic tank); its `airSystem_*` and `coolingSystem_*` folders are
car parts. Those rows stay primitives until another source or our own modelling covers them. The
library leans hospital, warehouse and garage (food 538 folders, kitchenware 179, boxes 154).

**Left out on purpose:** the two trademark-named assets; `dishware_thermos_a01` (a brand word);
the defibrillator and the patient monitor (likely maker marks and screen content, and 332 and
586 MiB); the space heater (`climate_heater_portableElectric_a01`, 282 MiB of new files for a 0.2 m
machine) and the refrigerators (495 and 618 MiB; the Kenney fridge stands in) wait for a second batch.

## 3. The download (phase 2, after the operator's yes)

- **Pinned list:** `scripts/simready/starter-set.json`, every file's path, size and hash, taken
  2026-11-08 with each asset's `package_id`. A file whose live size or hash differs is not fetched and
  the run stops: NVIDIA changed the dataset (last modified 2026-11-04) and the list needs a fresh yes.
- **Per asset:** the licence and packaging metadata, the root layer, `base.usda`, `geometries.usd`,
  `instances.usda`, `materials.usda`, `Physics/physics.usda`, the three empty `disabled.usda` variant
  layers (absent from the mug and the bowl, which use NVIDIA's newer layout with a `.usda` root), one
  look image, and the `Textures/` PNGs. **Never fetched:** `.mdl` files, `source_assets/`
  (copies of the same PNGs), the solver-specific layers (`instances_physx.usda` and the rest,
  `enabled.usda`, `runnables/`), texture previews.
- **Totals:** 47 assets, 962 file entries, **712 files to fetch, 2,452,180,340 bytes (2.45 GB)**:
  textures 243 files (1,968 MiB), USD layers 327 (346 MiB), look images 47 (24 MiB), metadata 95
  (0.7 MiB). NVIDIA reuses its tiling textures across assets under the same bytes, so 250 entries are
  copies of content already fetched and are copied locally (fetching each entry would be 3.70 GB).
- **Two passes, so the list is a ceiling:** pass 1 fetches metadata, layers and look images
  (370 MiB); the converter's `plan` step opens each stage and lists the textures its materials
  actually reference; pass 2 fetches only those.
- **Where:** `E:\HumanityOS\simready-src\<folder>\` (beside `instruments-src`), never git, each asset
  in its own folder. Honest user agent, one request at a time, a pause between.
- **Untrusted files:** the converter refuses any layer or asset path that is absolute, a URL, or
  leaves the asset's folder, and lists every resolved dependency (`UsdUtils.ComputeAllDependencies`);
  an unresolved one is an error, not a warning.

## 4. The conversion route

**Checked on this PC 2026-11-08, nothing installed:**

- **Blender 5.2.3 LTS** is installed (Steam, `D:\Steam\steamapps\common\Blender\blender.exe`; the
  `.blend` file association still names an old `E:\` path, so the script finds Blender itself, or
  takes `HUMANITY_BLENDER`). Run headless (`--background --factory-startup`), it imports Pixar's USD
  Python bindings, **`pxr` at USD 0.26.3 with `UsdGeom`, `UsdShade` and `UsdPhysics`**, has the
  Decimate modifier and Cycles baking, on Python 3.13.13. Blender is GPL-2.0-or-later and is only a
  build tool here: nothing of it ships, and what it writes is ours.
- No `usdcat`/`usdview` on the PATH; the system Pythons (3.14, 3.11) have no `pxr`.
- **Not needed, named for the record:** `usd-core` from PyPI (Pixar's own wheels, version 26.11,
  13.1 MB for Windows and Python 3.13, licence "LicenseRef-TOST-1.0", the Tomorrow Open Source
  Technology License) would give the same `pxr` without Blender. A pure-Rust USD reader would be a new
  crate download of unproven binary-USD support; no node reader is in the repo.
- **Rapier's own parry3d** (already in the tree through `rapier3d`) has convex hulls and VHACD convex
  decomposition, so collider shapes need no new dependency.

**The pipeline**, one command, `just simready [--asset <folder>] [--check]`, driven by
`scripts/simready/build.js`. No incremental reuse: each run wipes its build folder
(`E:\HumanityOS\simready-build\`) and rebuilds what it is asked to from the source files (the
no-caches rule).

1. **fetch** (`scripts/simready/fetch.js`): section 3, verifying size and SHA-256 per file.
2. **extract** (`scripts/simready/extract_usd.py`, run inside Blender, `pxr` only): opens the root with
   every physics variant Disabled; walks the render meshes (skipping guide and collision-only prims),
   triangulates, applies world transforms, converts to metres and Y up from the stage metadata;
   reads each material from UsdPreviewSurface or from the OmniPBR shader prim's USD inputs
   (`diffuse_texture`, `normalmap_texture`, `ORM_texture`, the constant and tint inputs, texture
   scale and offset; the MDL file itself is never read); resolves UDIM tile sets (a `<UDIM>` path:
   `crate_plywood_b01` spreads its textures over tiles `.1001` to `.1004`) and decal layers (the
   ladder, the extinguisher and its stand carry `_Decal_` textures over their base material); reads
   the neutral physics (section 6). Writes `extract.json` and raw little-endian arrays.
3. **bake** (`scripts/simready/bake_lods.py`, Blender `bpy`): decimates to the triangle caps of
   section 9, unwraps one non-overlapping UV set (Smart UV Project, fixed angle and margin), and
   bakes the full-detail model onto it with Cycles on the CPU, fixed seed, samples and thread count:
   base colour, a tangent-space normal map (high to low, so detail lost to decimation survives as
   normals), roughness, metallic and ambient occlusion. This is the standard high-to-low game asset
   bake.
4. **pack** (`scripts/simready/pack.js`, node, pure): resamples textures to their size class
   (box filter in linear light for colour, renormalised for normals), packs ORM the glTF way
   (R occlusion, G roughness, B metallic), writes ONE `.glb` per asset holding mesh 0 = the baked
   model, mesh 1 = its quarter-triangle distance version (same material, textures stored once) and
   the collider hulls as unrendered meshes flagged in `extras`; writes `LICENSE.txt` (copied byte for
   byte and checked against the CC BY 4.0 legal code's SHA-256), `SOURCE.md`, `physics.ron`, the
   folder manifest, and `assets/models/LICENSE-nvidia-simready.md` from the manifest so it never
   drifts. Fixed field order, no timestamps, generator `"simready pack.js (HumanityOS)"`.
5. **check** (`--check`): rebuilds into a scratch folder and compares every byte with what is
   committed; any difference fails and names the file.

**Byte-stable, and how that is proven:** pack is deterministic by construction. Extract and bake run
Blender, so phase 2 proves determinism by building two assets twice and comparing hashes before
trusting it. The manifest records the Blender, USD and node versions; Steam updates Blender on its
own, so when the version differs from the manifest's, `--check` says so in words instead of
reporting a false mismatch. Python is used only where Blender requires it (extract, bake); the
rest is node, the repo's scripting language.

## 5. Rungs: from boxes to real props

1. **Baked model through today's path (phase 2).** Each row's `model:` names
   `assets/models/simready/<folder>/<folder>.glb`; the engine draws mesh 0 with its base colour as it
   does the Kenney furniture. The row's `size` is set from the model's measured bounds, so the
   fit-to-height scale becomes 1.0 (a test holds it within 2%: the row and the model agree).
2. **Real materials (phase 2 if it fits, else next).** Type 19 learns metallic-roughness, normal and
   occlusion: ONE new group-3 binding, a 2-layer `texture_2d_array` (layer 0 normal, layer 1 ORM;
   both linear, same size), plus the material's metallic and roughness factors. That takes the main
   pipeline's fragment-stage sampled textures from 12 to 13 of the 16 that `wgpu::Limits::default()`
   allows (counted in `src/renderer/pipeline.rs`). Every creation site carries the binding
   (`tests/bind_group_entry_lint.rs`) and `just verify-runtime` boots and enters the world. Models
   and textures are shared by path across instances (today every machine instance uploads its own
   copy), and that sharing obeys `humanity_core::reuse::enabled()`, the Dev page switch.
3. **Full-detail near view.** Within a few metres, the faithful multi-material model with NVIDIA's
   tiling materials and repeat samplers. It needs block texture compression (BC7, through KTX2 or an
   encode at load) first: the shared tile library at 1024 px uncompressed would cost several hundred
   MiB of video memory.
4. **Level of detail and instancing.** Distance switching between near, baked and quarter models;
   one instanced draw per model; then furniture that blocks walking (the collider's footprint as a
   solid box in `wall_collision`, the rule placed pieces follow) and Rapier bodies for objects that
   can be knocked over or carried.

## 6. Physics: from UsdPhysics to the engine and the rows

Generated per asset into `assets/models/simready/<folder>/physics.ron`, beside the model it belongs to
(the finding: credit physics with the asset). Units come from the stage (`metersPerUnit`,
`kilogramsPerUnit`). A row may override a value; otherwise the loader reads it from the model's folder.

| UsdPhysics (neutral layer) | Stored as | Used now | Used when Rapier wakes |
|---|---|---|---|
| `MassAPI` `physics:mass` (else `physics:density`) | `mass_kg` (else `density_kg_m3`) | nothing yet: NVIDIA's authored masses are unchecked, and some are far off (section 13), so no card shows them | `ColliderBuilder::mass` / `density` |
| `physics:centerOfMass`, `diagonalInertia`, `principalAxes` | `center_of_mass_m`, `principal_inertia_kg_m2`, `principal_axes` | none | `MassProperties::with_principal_inertia_frame` |
| `CollisionAPI` + `MeshCollisionAPI` `approximation`: `convexHull` | `collider: Hull`, the hull mesh in the GLB | the pick volume stays the row's `size` | `SharedShape::convex_hull` |
| `convexDecomposition` | `collider: Hulls`, decomposed offline with parry3d's VHACD (PhysX does it at run time; USD stores only the word) | | `SharedShape::compound` |
| `boundingCube`, `boundingSphere` | `collider: Box` / `Ball` | | `cuboid` / `ball` |
| `none`, `meshSimplification` | `collider: Mesh` | | trimesh, fixed bodies only |
| `PhysicsMaterialAPI` `staticFriction`, `dynamicFriction`, `restitution` | `friction` (dynamic; Rapier keeps one coefficient), `static_friction`, `restitution` | none | collider friction and restitution |
| `RigidBodyAPI`, joints (a door, a drawer) | `body`, `joints` | none | body type, revolute and prismatic joints |

Read by `src/assets/model_physics.rs` (`ModelPhysics::beside`, the file next to any model, so a
mod's model can bring its own). `MachineDef` gains no fields until a rung uses the values: a row
that needs a different mass will override it then, with its source.

## 7. Where the converted files live, and the size threshold

`assets/models/simready/<folder lowercased>/` (the asset rules want lowercase; `SOURCE.md` keeps
NVIDIA's exact name): `<folder>.glb`, `LICENSE.txt`, `SOURCE.md`, `physics.ron`. Releases already copy
`assets/models/` whole, so the licence files travel with every archive.

The comparisons: the instrument core ships in the app at 6.3 MiB and its packs are 6 to 70 MiB
release assets; the star catalogue ships a small core and offers its big catalogues as downloads
(`assets-stars-1`); the plant models already in `assets/models/plants/` are 183 MB. Estimated converted size (section 9's classes, PNG): about
1 MB per small prop, 2 MB per medium, 5 MB per large, so about **80 MB for the 39 home assets and
25 MB for the 8 Medical Bay ones**.

**Recommendation:** the home set ships in the app under a **96 MiB cap** for
`assets/models/simready/` (a test fails above it): these are the starting home's look, and a
download would leave every new player among boxes. Byte-stable rebuilds keep them from piling up in
git history. Anything beyond the cap, and every later batch, becomes a release-asset pack of at most
about 70 MiB (`assets-simready-1`), built and checked the instrument packs' way (SHA-256 in a
catalogue, Settings download, uploading is the operator's step), with the box primitive as the
fallback when a pack is not installed. By the estimate the Medical Bay set does not fit beside the
home set, so it is converted and checked with the rest but kept out of git (in
`E:\HumanityOS\simready-packs\`) and becomes the first pack when the Medical Bay is built; until then
nothing draws it, so the pack installer is built with that room, not before. If phase 2 measures the
two sets under the cap together, both ship in the app.

## 8. Credits and the brand check

**Credits** (the finding's "How the project would credit it"):

- each folder: `LICENSE.txt` unchanged, and `SOURCE.md` naming the NVIDIA folder, the dataset link,
  the `package_id`, "by NVIDIA, licensed under CC BY 4.0" with the licence link, every change made
  (converted from USD to glTF; merged and simplified to N and M triangles; materials baked into one
  texture set at P px; any repaint), and the no-warranty line;
- `assets/models/LICENSE-nvidia-simready.md`, generated, beside `LICENSE-cc0-model-packs.md`;
- `data/credits.ron` row `nvidia_simready`: licence "CC BY 4.0", notice "3D models based on NVIDIA
  SimReady Assets, by NVIDIA, CC BY 4.0, modified", `attribution_required: true`, `shown_in`
  "Credits" and, once built, "Machine walk-up card (model credit line)" (a one-line credit on the card
  of an object drawn with one of these models, words by i18n key);
- `CREDITS.md` "Asset libraries (models and textures)" and a `LICENSES.md` section saying these
  files stay CC BY 4.0, with the CC0 scope sentence kept true;
- never NVIDIA's logo, "SimReady" branding on screen, or wording that suggests endorsement.

**The brand check**, after pass 1 and before converting: `scripts/simready/review-sheet.js` tiles each
asset's look image and every referenced colour texture into one sheet, with full-resolution crops of
any area holding text. Each sheet is looked at (by the agent, and offered to the operator); the
verdict is recorded per asset in `starter-set.json` as `brand_check` (date, who, `clear`, `repaint`
or `drop`, notes). The build refuses an asset with no verdict or with `drop`. It looks for maker
names, logos, product labels, readable text naming a company, a person's likeness, and protected
emblems: a red cross or red crescent on white is protected in law in many countries, so one found on
the first aid kit, the medicine cabinet or the screen gets a dated finding before it ships, and the
default fix is repainting it as the ISO 7010 first aid sign (a white cross on green). Higher-risk
picks to look at closely: the three with decal textures (ladder, extinguisher, extinguisher stand),
which is where printed labels live, then the toaster, multimeter, smoke alarm, thermometer,
humidifier, tool chest, safety glasses, IBC tote, drum, bucket and the first aid kit's own texture. A repaint
is an adaptation and goes in `SOURCE.md`.

## 9. Budgets for a home full of them

A full homestead holds about 80 instances of the 39 home models. Size classes come from each model's
largest dimension:

| Class | Baked model | Distance version | Textures (albedo / normal+ORM array) | Video memory per model, mips included |
|---|---|---|---|---|
| Small, under 0.4 m | at most 3,000 triangles | 25% | 512 / 512 | 4 MiB |
| Medium, 0.4 to 1.2 m | at most 10,000 | 25% | 1024 / 512 | 8 MiB |
| Large, over 1.2 m | at most 25,000 | 25% | 1024 / 1024 | 16 MiB |

- **Video memory:** at most **320 MiB** for every SimReady model resident in one home at the Full
  level (about 21 small, 10 medium and 8 large: about 290 MiB), with textures and meshes shared by
  path; rung 1 alone (albedo only) is about 125 MiB. The Simple level halves each texture side (a
  quarter of the memory). Registered as a "Props" slice in `data/performance/budget_systems.ron` and
  through the existing `vram.textures` inventory.
- **Per frame:** about 160 draws (colour and shadow passes) and at most 500,000 triangles on screen.
  Budget: **0.5 ms GPU and 0.3 ms CPU** at 2560x1440 on the RTX 4070, measured as the A/B difference
  between props and primitives at `home-overview-noon` and three new vantages (kitchen, bathroom,
  workshop) that get `perf_floor_fps`.
- **World entry:** at most 0.5 s added, decoding on worker threads. **Disk:** the 96 MiB cap.
  **Network:** none; props are cosmetic and local, and only the row ids sync, as today.

## 10. Saying plainly what a person's hardware can run

Nothing here needs a particular GPU vendor or feature: rung 2 stays inside `wgpu::Limits::default()`.
Video memory is the one limit. **Settings > Graphics > Furniture detail** (built 2026-11-09,
`src/gui/furniture_detail.rs` with the engine's rebuild in `src/engine/furniture_detail.rs`, levels in `data/gui/furniture_detail.json`) offers Auto, Full,
Simple and Boxes, each with the video memory its models' textures and meshes take in the home the
player is in, MEASURED by the home's body build rather than estimated (a level change replaces each
model's picture in place, so Simple and Boxes give the memory back at once, not at the next start). The card's own memory comes from the same
reading the bounced-light setting uses (`renderer::bounced_light_detail::adapter_memory`, the
driver's install record on Windows). Auto, the default, draws Full on a card with at least 3 GB of
its own memory (or reporting none, as an integrated GPU sharing the computer's memory does) and
Simple below it, and says which it chose; a level that takes more than an eighth of the card's
memory is warned about in words: what it needs, what this card has, and that Simple or Boxes give
it back. Full, Simple and Boxes are also the dual-mode rule's realistic and simplified modes, and
Boxes is the plain-box fallback every machine model already had.

## 11. Phase 2 gates

- `scripts/tests/simready-pack.test.js`: packs a tiny synthetic extract twice, byte for byte; the GLB
  parses (chunk lengths, accessor bounds). Seen red by writing a timestamp.
- `tests/simready_lint.rs` (std-only, in `just lints`): per folder, the licence hash, SOURCE.md's
  words and `package_id`, no `.mdl` anywhere, no deny-listed folder (`wd40`, `rubikscube`), a
  `brand_check` verdict of `clear` or `repaint`; the `credits.ron` row with `attribution_required`
  and a surface; CREDITS.md and LICENSES.md mention it; the 96 MiB cap. Each seen red.
- Data: `just validate-data`; every SimReady `model:` path exists and fits its row within 2%.
- Runtime: `just verify-runtime` for rung 2; vantage screenshots judged; the frame-cost A/B.
- `node scripts/check-doc-links.js` at 0 broken.

## 12. Questions for the operator (answered 2026-11-08: yes to both)

1. **The download:** 2.45 GB to `E:\HumanityOS\simready-src`, never into git; Blender is already on
   this PC, so nothing is installed. *Recommendation: yes.*
2. **Converted models in the app:** about 80 MB for the home set under a 96 MiB cap, the first
   non-CC0 models, instead of a download pack. *Recommendation: yes; the Medical Bay set (about
   25 MB) waits outside git and becomes the first pack when the Medical Bay is built.*

## 13. What the build found (phase 2, 2026-11-09)

- **The download.** Pass 1 fetched 469 files (370.5 MiB) and copied 136 shared ones; the plan step
  found every pinned texture referenced, so pass 2 fetched all 243 (1,968.1 MiB) and copied 114.
  Every file matched its pinned size and hash. One reference fell outside the list: the humidifier's
  grey steel points into `source_assets/`, which is never fetched, so the bake paints that surface
  plain grey and its `SOURCE.md` says so (the plan step now names such references).
- **The look changed four fits.** `furniture_workbench_modular_a01` is a 2 m double-door cabinet,
  not a bench: it became the new "Workshop cabinet" row, and the stainless two-tier table
  (`table_metal_stainless_a03`, also the Medical Bay's instrument table) became both benches. The
  first aid kit belongs in every home, so it ships in the app. The basin is a drop-in sink that needs a
  counter, so the kitchen sink row draws the basin seated in a new 3 m kitchen counter (the same model
  draws the Commons serving counter), with the mixer tap behind it, each a row of its own. Most NVIDIA models face +X or +Z, and the engine draws a model's front along its
  local -Z, so `starter-set.json` records a `turn_deg` per model.
- **Base colour through emission.** Cycles' diffuse colour pass is the diffuse share of a surface,
  black for a metal, so the first bake drew the stainless table and the chrome black; the base colour
  is now baked through an emission pass of each material's own Base Color input, as metallic and
  occlusion already were.
- **The brand and emblem check** (verdicts in `starter-set.json`): 38 clear, 8 repainted, 1 dropped.
  Repainted in the source textures: the kettle's KETTLE WATER label, the hammer's TASKOV Crushing,
  Inc., the ladder's AVERKIN (R) oval and a CE mark, the extinguisher label's BENITEZ logo, its two
  address blocks and two certification marks (a UL-style cRUus and a T-in-triangle), on both the
  extinguisher and its stand, and the first aid kit's red-and-white cross, every red turned to the ISO
  7010 first aid green. Dropped: the smoke alarm, whose lid carries an embossed logo in its geometry.
- **A second look found three names moulded into the geometry** (2026-11-09), which a review of the
  textures cannot see: the ladder's ASSURED INDUSTRIES in orange rubber letters on both treads, the
  screwdriver's VORTEN and its ring mark on the handle (the first check had painted the grip inlays
  in the texture, which changed nothing visible), and the utility knife's DRAVOX (passed as "no
  lettering"). They showed in the homestead captures and on a sheet of every baked base colour, and
  are now painted out of the bake itself (a `"stage": "baked"` repaint in `pack.js`: the letters'
  pixels take the surface's own value in the base colour, normal and ORM maps, so no relief is left
  for a later rung's normal mapping either). **The lesson for the next batch: review the baked base
  colour of every model, not only its textures and NVIDIA's thumbnail.**
- **The Kenney furniture was drawn from its corner.** Its models are authored with the origin at a
  corner, and the machine fit only scaled them, so every piece stood up to 1.3 m from its own
  placement, label, pick volume and collision, and the Boxes level's box stood beside the model (a mug
  placed on the dining table sat in the air beside it). The fit now centres each model's footprint
  (`GltfCpuMesh::centre_footprint`), so the dining chairs stand round their table and the Boxes and
  Full levels draw the same furniture in the same place.
- **The walk-up cards.** A kitchen of small props drew a card each whose only line was the model
  credit, a wall of overlapping cards; a prop whose only line is its credit now shows its name until
  the crosshair is on it, and its card with the credit then. The licence's credit is the Credits
  entry, LICENSES.md and each folder's SOURCE.md (the finding); the card line is extra.
- **The masses are NVIDIA's, and several are wrong.** Solid-volume estimates give a 2,035 kg medicine
  cabinet, a 503 kg steel shelf, a 481 kg bench locker and a 0.18 kg drum. They are kept in
  `physics.ron` as published (credited with the model) but shown nowhere: the walk-up card carries the
  model credit line only, and a mass is checked against a real source before any rung uses it.

### 13.1 Measured against section 9's budgets (2026-11-09)

Same-session sweeps on the probe rig (2560x1387, RTX 4070, rig default graphics), this branch's rows
against main's rows from before them, with nothing else running (round 4 of the scratch sweeps;
earlier rounds with another agent compiling are marked contaminated by the sweep and not used here):

| Budget (section 9) | Plan | Measured |
|---|---|---|
| Disk, in the app | 96 MiB cap | 53.3 MiB (40 models); the Medical Bay pack 14.0 MiB |
| Video memory at Full | at most 320 MiB | 99.7 MiB: textures 72.0 MiB (`vram.textures` rose by exactly that) and meshes 27.7 MiB (each placed model owns its mesh; `vram.meshes` rose 26 MiB). Simple: 45.7 MiB. Boxes: none |
| Triangles | at most 500,000 on screen | 451,000 for all 45 placed SimReady instances in the home |
| GPU per frame | 0.5 ms | wide room views: overview -0.02, bathroom +0.11, workshop +0.20, kitchen +0.36 ms (gpu sum). Close views of dense clusters: +0.78 to +0.97 ms (the stove, the workshop cabinet and shelves, the counter), over budget |
| CPU per frame | 0.3 ms | not resolvable on this rig: the same vantage's cpu.frame_total moves 5 to 8 ms between runs; 45 more draws |
| World entry | at most 0.5 s added | the home's body build: 502 ms against 93 ms without the props, +0.41 s (it was 1,070 ms before the model read stopped decoding the normal and ORM pictures and each file was read once per build) |
| Network | none | none: the rows sync as before |

The close-view overage is the scene pass shading more surfaces where the boxes or empty wall stood;
the triangles are cheap (the whole home's 451,000 cost the overview nothing measurable). The likely
fixes, for a performance pass rather than this rung: drawing the home's bodies front to back or
behind a depth pre-pass so hidden surfaces are not shaded, and the distance version (LOD2, already
in every GLB) beyond a few metres. Interiors run near 30 fps on this rig with or without the props
(gpu sum 15 to 21 ms in the rooms against 13 ms for the overview), so the rooms' own cost is the
larger question.
