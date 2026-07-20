# Adding 3D Models

The real glTF pipeline: what the engine loader actually supports, how to
prepare a model so all of it renders, and where files go. Companion to
[docs/game/model-pipeline.md](../game/model-pipeline.md) (the GLB authoring
rules + the machine `model:` field); this doc covers the loader internals and
the photoscanned-plant pipeline added v0.904-v0.909.

## The loader, exactly (`src/assets/mod.rs`)

Four entry points, all native-gated:

- `parse_gltf_mesh(device, relative_path)` - geometry only, returns an
  uncached, caller-owned `Mesh`. Used for per-instance machine models (a
  shared cache would let the construction editor's `replace_mesh` corrupt
  every copy, see the hazard note in model-pipeline.md).
- `parse_gltf_mesh_with_texture(relative_path)` - CPU-only: geometry plus the
  base-color texture decoded to RGBA8, `Some((rgba, width, height))` when the
  first primitive's material has a `pbrMetallicRoughness.baseColorTexture`,
  `None` otherwise.
- `parse_gltf_mesh_textured(device, relative_path)` - GPU convenience over the
  above: uploads the mesh, hands back `(Mesh, Option<texture>)` ready for
  `Renderer::add_textured_material`.
- `load_gltf(renderer, relative_path)` - the CACHED path: registers the mesh
  on the renderer and returns a mesh index, cached per path. Use for shared,
  never-mutated geometry.

Hard limits every model must respect:

1. **Only the FIRST mesh's FIRST primitive is read**
   (`decode_first_primitive`). Everything else in the file is silently
   dropped. Multi-mesh or multi-primitive sources must be repacked first (next
   section).
2. **Indexed triangles required.** No indices = rejected with an error.
3. **Missing normals** get generated flat face normals; **missing UVs** get a
   planar projection. Export real ones for anything smooth or textured.
4. **One material.** A single base-color texture at most; normal/ARM maps and
   `KHR_*` extensions are ignored.
5. **Textures decode to RGBA8 and downscale to max 1024x1024**
   (`decode_base_color_texture` + `downscale_rgba_if_needed`, Triangle filter,
   aspect preserved) to keep per-model VRAM sane. jpg and png sources work
   (whatever the `image` crate decodes); jpg decodes with alpha = 255.
6. **Units are meters, Y up.** No scaling applied on load.
7. **Path resolution** (`resolve_model_path`): the game DATA dir first
   (`data/models/...`, the distributed + moddable tree), then the data dir's
   PARENT (so `assets/models/...` works in a dev checkout).

Prefer single-file `.glb` for shipped models (never loses its buffers);
`.gltf` + external `.bin` + jpg textures is the working format for the
photoscan pipeline below.

## Repacking multi-mesh sources: `scripts/repack-plant-gltf.js`

Poly Haven photoscans (and most downloaded assets) are multi-node, multi-mesh,
multi-primitive scenes. Because of limit 1 above they render as one fragment
unless repacked. The repack script (node built-ins only, no npm install) has
two modes:

- **Merge mode** - `node scripts/repack-plant-gltf.js <folder|slug> ...` or
  `--all`: bakes every node's world transform into the vertices (positions
  via the 4x4, normals via inverse-transpose, winding flipped on mirrored
  transforms) and writes `<slug>_merged.gltf` + `.bin`, one mesh, one
  primitive, no materials. It re-parses its own output and hard-fails unless
  the triangle count matches the source sum.
- **Split mode** - `node scripts/repack-plant-gltf.js --split <folder|--all>`:
  source scenes often lay several plant VARIANTS side by side (5 grass clumps
  in a row). Split clusters mesh nodes by world X/Z (center-inside-bbox test,
  union-find), writes each variant as its own `<slug>_v1.gltf`, `_v2`, ...
  re-centered so its base sits at x=0, z=0 (y untouched, y=0 stays the ground
  plane), each carrying a minimal material that references the source
  `textures/` folder's base-color jpg by relative URI. Verified the same way
  (variant triangle sum must equal the source total).

Use the `_vN` variant files for scattering/spawning (they load through
`parse_gltf_mesh_with_texture` and bring their texture along); `_merged` keeps
the whole side-by-side scene for reference. Single-primitive means one
material per variant: multi-material variants carry the dominant-by-triangles
material (a sapling's ~500-tri trunk borrows the twig texture, acceptable
until a multi-primitive loader exists).

## Rendering textured models: material type 19

`Renderer::add_textured_material(base_color, metallic, roughness,
material_type, emissive, rgba, width, height)` (`src/renderer/mod.rs`) uploads
the RGBA8 texture and binds it as the draw's group-3 albedo texture. Material
type **19.0** is the textured-mesh path in `assets/shaders/pbr_simple.wgsl`:
albedo texture times `base_color`, **alpha cutout at a < 0.35** (photoscan
leaf textures carry alpha; fragments below the cutoff `discard`), then the
normal sun-lit PBR path. See [adding-shaders.md](adding-shaders.md) for the
full material-type table.

## Scattering models in the world: `data/entities/decorations.ron`

Pure-visual decoration scatter (v0.909), no code per model:

```ron
(model: "grass_medium_02_v1", near: "grain_field_1", count: 8, spread: 5.0),
(model: "fir_sapling_v1", near: "grain_field_1", offset: (8.0, 0.0, 4.0), count: 1, spread: 1.0),
```

Each row scatters `count` copies of a variant near a machine instance from
`data/machines/home.ron`, within `spread` meters (+ optional `offset`). Yaw
and scale jitter are deterministic, so layouts are stable across reloads.
Adding greenery = adding a row here.

## Triangle budgets

Measured guidance from the shipped plant set (see the budget comment at the
top of `decorations.ron`):

- **Scatter-safe: under ~3k triangles per instance.** The grass variants are
  714-2489 tris, ferns 784-2384. Scatter these freely.
- **Hero accents: up to ~200k.** The fir/pine saplings are 122k-157k tris,
  place them individually and rarely (each is a full extra draw's worth of
  geometry).
- Machines/props: game-prop budgets, a vehicle in the low tens of thousands,
  a machine well under that (model-pipeline.md rule 8).

## Folder layout and manifest

- Models live in `assets/models/<domain>/<name>/` (e.g.
  `assets/models/plants/fern_02/`): source `.gltf` + `.bin` + `textures/`
  folder, plus the generated `_merged` and `_vN` repacks next to them.
- `assets/models/plants/manifest.json` is the per-folder record: slug, source
  URL, license, file list, triangle counts, dimensions, and the generated
  `merged` + `variants` entries (the repack script updates those fields
  itself). New model families should copy this manifest pattern: it is how a
  future reader knows what a folder contains without opening Blender.
- Blender sources (`.blend`) may sit in `assets/models/` too; they are source
  of truth for hand-authored props.

## Licensing

Same rule as all imported media: **CC0 preferred** (Poly Haven is CC0), and
every imported model family gets an entry in `CREDITS.md` (repo root) with
source URL and license, plus the `license` field in its manifest.json. No
rips, no unlicensed "free" models.

## Checklist for adding a model

1. Get/author the model. Meters, Y-up, indexed triangles.
2. Put it in `assets/models/<domain>/<name>/`; write/extend the folder's
   `manifest.json`; add the credit line in `CREDITS.md`.
3. Multi-mesh source? Run the repack script (merge, or `--split` for variant
   sheets). Single-mesh single-primitive exports from Blender (join objects,
   Ctrl+J) can skip this.
4. Reference it from data (a `model:` field in `data/machines/home.ron`, or a
   scatter row in `data/entities/decorations.ron`).
5. Verify by booting the release exe and looking at it (screenshot protocol:
   drop `debug/screenshot_request.json`, read the PNG). Check
   `%APPDATA%/HumanityOS/logs/run.log` for loader warnings; a bad model falls
   back to the primitive shape rather than blanking the object.

## Known doc drift (as of 2026-07-20)

`docs/game/model-pipeline.md` rule 7 says "textures/images inside the GLB are
currently ignored". That was true at v0.734; since v0.904 the
`parse_gltf_mesh_with_texture` path DOES decode the base-color texture (this
doc is current). The machine `model:` path still loads geometry-only via
`parse_gltf_mesh`.
