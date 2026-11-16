# The model inspector

**Asked for:** 2026-11-14, by the operator. Verbatim: "We'll want to improve our file tools so we can
actually review collision meshes, empties, etc. of 3D models we're wanting to use. One of the issues I
and other people kept running into with modding Space Engineers is that the only way we could check a
file was to launch the game with the mod enabled. We couldn't double check the models before we
converted our blend files to the mwm format Space Engineers uses. Like turrets were a real problem
because people never knew what the empties were, what the settings they needed were, what orientation
they were. Everything was a lot of trial and error. We want to make it easier/faster to figure out
what's wrong."

The same day the homestead fixes found catalog footprints that do not match their models (the bed's
2.0 x 1.0 m box against a model 1.53 x 1.8 m turned across it, a bookshelf 0.35 m deep against 0.57 m,
a wardrobe 1.2 m wide against 0.94 m, six SimReady medical models referenced but missing), so the
walk, the collision and the placement followed boxes nobody could see.

So: **see everything the game will do with a model before the game does it.** The inspector is the 3D
viewer's Inspect mode (docs/design/model-viewer.md), native first, the website mirroring it, plus a
command-line twin for agents and scripts. It never changes a file in rung 1; it shows and says.

## 1. What a modder or a developer needs to check

Each is a question people answered by trial and error in the game. The inspector answers it on the
file, in words and drawn over the model.

| Question | What the inspector shows | Rung |
| --- | --- | --- |
| What are the nodes, and which are empties (attachment points)? | The node tree, every node named, empties marked, each selectable | 1 |
| Which way does each empty point? | Its local axes drawn at it (x, y, z in the axis colours), the selected one larger and labelled | 1 |
| Which way is up and which way is front? | The origin's axes, labelled "up" (+Y) and "front" (+Z), glTF's convention and the game's (a placed machine faces its own +Z) | 1 |
| What units is it in, and is the scale right? | Its size in metres; a model hundreds of metres tall or a few millimetres small is called out (centimetres or millimetres exported as metres) | 1 |
| Where is its pivot? | Where the origin sits against the model: on its base and centred, on a corner, floating | 1 |
| Does it fit the box the catalog gives it? | The bounds and the catalog box drawn together, the mismatch in words, and whether a quarter turn would fit | 1 |
| Where are its collision shapes? | physics.ron's colliders and any collision mesh named by convention, drawn as wireframes, and whether they cover the visible model | 1 |
| What will the game actually draw? | The home draws a catalog model's first mesh's first part, without its node's transform: the inspector says when that differs from the whole model | 1 |
| Materials and textures: are they all there, and how big? | Each material, each texture's size, missing pictures named | 1 |
| Levels of detail, animation clips, skins | Listed: LOD nodes by convention, each clip's name, length and channels, the skin count | 1 (listed), 5 (played) |
| Seats and mount points | Empties named by convention; a catalog row's interaction anchors (data/characters/interactions.ron) | 1 (empties), 3 (anchors and a figure) |
| How does a turret turn? | Its mount's yaw and pitch limits, the joints from physics.ron, swept in place, its muzzle's line of fire | 3 |
| Is my .blend right before I export? | The same inspection, the .blend exported through the person's own Blender, if they have it | 4 |

## 2. The conventions are data

`data/models/inspect_conventions.json` (Infinite-of-X: a new kind of empty is a new row, never
code). Read by the desktop (`src/gui/model_view/inspect.rs`), the website and the command line
(`web/shared/model-inspect.js`). Its parts:

- **empties**: one row per kind of attachment point: `id`, the name `prefixes` that mark it (matched
  case-insensitively on the start of the node's name, after Blender's `.001` suffix is dropped), the
  words key saying what it is for, and what it needs:
  - `forward`: `outward` (a muzzle, an exhaust: its +Z points away from the model's middle),
    `level` (a seat, a door: its +Z lies flat, its +Y is up), or `any`;
  - `size`: when the empty's scale carries a size (Space Engineers' detector and door dummies did
    this), the smallest and largest it may be, in metres.
  The starting rows: mount, muzzle, seat, door, light, socket (something attaches here), hand and
  grip, camera and eye, exhaust and thruster, port (a pipe, cable or conveyor joins here), spawn.
- **collision**: the name prefixes and suffixes that mark a mesh as a collision shape, not a visible
  part (Unreal's `UCX_`, `UBX_`, `USP_`, `UCP_`; Godot's `-col` and `-colonly`; plain `collision`
  and `col_`). Such a mesh is drawn as a wireframe and left out of the visible bounds.
- **lod**: the name suffix that marks a level of detail (`_LOD0`, `_LOD1`, ...).
- **scale**: the tallest and the smallest a model may plausibly be in metres, past which the report
  asks whether it was made in centimetres or millimetres.
- **tolerance**: how far a catalog box and its fitted model may differ (a share of the box and a
  floor in metres), how far a collision shape may stand off the visible model, and how closely a
  `level` empty must lie flat.

## 3. The report, in plain words

Every finding has a level, a code, and words by key (`model_inspect.finding.<code>` in
`data/i18n/en.json`, the one word source), filled with its numbers:

- **problem**: the game will get it wrong (a missing texture, a catalog box the model does not fit, a
  collision shape off the model, a model the game will draw differently from what you see, an empty
  pointing the wrong way, a size an empty must have and does not);
- **check**: worth a look (a scale that suggests centimetres, a pivot on a corner, an empty named by
  no convention, an empty with no name);
- **note**: plain facts (no physics file, the units, the number of clips).

The report is the same on the desktop, the website and the command line: the same codes from the same
conventions, the same words. The command line prints it as JSON:

```
node scripts/inspect-model.js assets/models/furniture/beddouble/beddouble.gltf
node scripts/inspect-model.js --catalog        # every catalog row with a model: its box against its fitted model
```

With `--catalog` it also lists the known exceptions with their reasons
(`data/models/catalog_fit_exceptions.json`) and each row whose model is in a download pack not installed
on this computer, with the pack, its size and where a person gets it (Settings > Graphics > Furniture
models); it exits 1 when a row's box does not fit its model and is not a known exception.

## 4. The fit, exactly as the game does it

A catalog row (`data/machines/*.ron`, a `MachineDef` with `model: Some(...)`) draws its model through
`AssetManager::parse_gltf_mesh_with_texture` and `GltfCpuMesh::fit_for_machine`: the **first mesh's
first primitive**, its positions as written (no node transform), **turned about up by the row's
`model_yaw`** (degrees, the instance `rotation`'s sense; 0 when the row has none), scaled uniformly about
the origin to the row's height (`size.1`), then its footprint centred on the origin. The interaction
anchors (`data/characters/interactions.ron`) are in the model's frame and turn with it. The inspector computes the same
fit and compares the fitted footprint (x by z) with the box (`size.0` by `size.2`):

- it fits when each side is within the tolerance (10% of the box side, at least 5 cm);
- **turned across its box** when the sides fit only after a quarter turn (`catalog_turned`);
- **across and the wrong size** when it fits neither way but is nearer to fitting turned
  (`catalog_across`: the bed, 1.53 x 1.80 m fitted against its 2.00 x 1.00 m box);
- otherwise the words name both: the box's width and depth against the fitted model's
  (`catalog_box`: the bookshelf, 0.91 x 0.57 m against 1.00 x 0.35 m).

The first run (2026-11-14) found 12 of 61 catalog rows off their models and 6 rows naming SimReady
medical models that are not on the computer. **The furniture fixes (2026-11-15) made every box agree
with its fitted model**: the mirror was fixed by BUG-309; the bed turned with `model_yaw` 270 and took a
queen bed's length (2.04 x 1.73 m at 0.68 m tall); the nightstand, wardrobe, couch, chair, bookshelf and
freezer took their models' footprints at their real heights; the solo homestead's side table, shelf,
pantry cabinet and tool rack took the family home's fixed rows (v0.1324). Each row's data comment says
where its size comes from. Every re-sized piece was re-placed in all nine machine files, standing
against the skirting (`crates/humanity-ship/src/furniture_fit_tests.rs`). The six medical models are the
Medical Bay pack (`data/models/packs.ron`, 14.7 MB, a release asset under the tag `assets-simready-1`),
downloaded from Settings > Graphics > Furniture models; until it is installed each draws its plain box
and its card says why and where to get it (`model_packs.card_not_installed`), and the check names the
pack instead of judging the row.

**The check fails since then** (`gui::model_view::inspect_tests::every_catalog_box_fits_its_fitted_model`,
and its twin in `scripts/tests/model-inspect.test.js`): a row whose box does not fit is a test failure
unless `data/models/catalog_fit_exceptions.json` lists it with its reason (empty today), and a listed row
that fits or is gone fails too, so the list only shrinks.

A test pins the inspector's fit to the engine's own (`fit_for_machine`, the turn included) on every
catalog model, so the two can never drift.

## 5. Rungs

**Rung 1 (2026-11-14, built).** In the Files page:
- the HumanityOS folder place opens a `.glb`, a `.gltf` (with its outside files, read from beside it
  and never from above its folder) or an `.stl` in the 3D view instead of as text (a `.gltf` keeps
  "Read its text" to edit it as before);
- the 3D view gains **Inspect** (beside Close the 3D view): the node tree with empties listed and
  selectable, axes on every empty and at the origin, up and front labelled, the bounds and the catalog
  box drawn together with the mismatch in words, collision shapes as wireframes, and the report;
- the overlay is drawn over the model (x-ray, never hidden inside it), from the theme's axis and
  inspector colours;
- `node scripts/inspect-model.js <file>` prints the same report as JSON, `--catalog` the mismatch
  list; a test ran the catalog check in report mode (it listed, it did not fail) for the furniture
  fixes that followed, and fails since they landed (2026-11-15, section 4).
- The website mirrors the Inspect mode in its viewer (`web/shared/model-view.js` with
  `web/shared/model-inspect.js`): the report, the node tree and the overlay. It has no HumanityOS
  folder and no catalog (a model on a server is not a catalog entry), so it shows no catalog box.

**Rung 2: editing the catalog box from the inspector.** "Fit the box to the model" and "Turn the
model a quarter" on a catalog row, written back to `data/machines/<file>.ron` through the HumanityOS
folder's save (its `.bak` kept), the row's height kept unless asked. **Its data half is built
(2026-11-15)**: `MachineDef::model_yaw`, the turn the model gets inside its box, read by the game's fit
and draw, the anchors, the inspector (desktop and website) and the command line, so a model turned
across its box is fixed by data, not by re-exporting it; the two buttons remain.

**Rung 3: turret and seat preview.** A seat empty or a catalog anchor (interactions.ron) shows the
body's figure sitting or lying there; a mount shows its yaw and pitch limits as arcs and turns through
them (physics.ron's revolute joints and their limits); a muzzle shows its line of fire; a door its
swing. This is the Space Engineers turret problem answered on the file.

**Rung 4: Blender files.** A `.blend` opens through the person's own Blender, if they have one
(found on the usual paths or set in Settings, never required, never bundled): Blender runs in the
background with a short export script to a temporary `.glb`, which is inspected and deleted. No
Blender: the `.blend` says so plainly and how to export a `.glb` by hand. The same route opens
`.obj` and `.fbx`.

**Rung 4b: the HumanityOS add-on for Blender (operator, 2026-11-16, not built).** Verbatim: "Speaking
of blender we'll want to create an addon specifically for using blender to create stuff for HumanityOS.
Like a script for exporting everything properly with all the proper names and properties and whatever
else. The less chance to misspell something the better. Kind of like the empties being misnamed for
turrets in Space Engineers." The add-on is the other half of this inspector: the inspector finds what
is wrong after export, the add-on makes it hard to get wrong before. Its shape, to plan in full:

- **No typing names.** A HumanityOS panel adds empties by choosing their kind from a list read from
  `data/models/inspect_conventions.json` (mount, muzzle, seat, door, light and the rest), named and
  oriented by the convention, with each kind's needs (a forward axis, a size) set or asked for. A new
  kind of empty is a new row in that file, never an add-on update (Infinite-of-X).
- **The catalog box and collision shapes as objects** the add-on draws (a wire box of the catalog row,
  primitives for physics.ron), so the fit is seen in Blender as the game does it (section 4, including
  `model_yaw`).
- **Export done right in one click:** glTF binary, metres, +Y up and front +Z (Blender's exporter
  converts from its Z-up), transforms applied, the file named and placed where the game reads it, the
  physics.ron and catalog row written beside it.
- **The same report before export:** the inspector's checks (section 3) run in Blender from the same
  conventions file, so an empty pointing the wrong way, a missing texture or a wrong scale is caught
  before the file leaves Blender; where HumanityOS is installed, its own `inspect-model` command can
  run instead, so there is one set of rules.
- **To settle first:** the licence. Blender add-ons that use its Python API are expected to be under a
  licence compatible with Blender's GPL, and the project's code is AGPL-3.0-or-later since 2026-11-14;
  a dated finding (CLAUDE.md) must say whether the add-on can ship under the project's licence or
  needs its own, and what Blender's extensions platform requires.

**Rung 5: levels of detail, animation and textures.** Switch between LODs; play each clip (skins and
morph targets); show each texture map on its own (base colour, normal, roughness, metallic,
occlusion) and a UV checker.

**Rung 6: a whole mod at once.** Inspect a folder: every model's report in one list, the worst first;
a mod's models inspected before it is published on Files, the report shown to the person publishing.

## 6. Costs

The inspection runs once on the loading thread with the model (no frame cost while nobody inspects).
The overlay is a few hundred lines for the gizmos and boxes plus the collision wireframes, capped at
20,000 edges (the rest are said, not drawn), projected on the CPU each frame the view repaints. Nothing
is kept between runs; nothing is networked.

## 7. Where the two apps differ, on purpose

- The website has no HumanityOS folder, so no `.gltf` with outside files and no catalog box.
- The website draws the overlay on a 2D canvas over the WebGL canvas; the desktop paints it with egui
  over the GPU's picture. Both project with the same camera, so lines land on the same places.
