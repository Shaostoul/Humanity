# 3D Model Pipeline (GLB)

> Status as of v0.728 (2026-07-06): the engine's GLB loader is BUILT and
> verified (`src/assets/mod.rs::load_gltf`), but nothing references model
> files from data yet — machines and vehicles render as data-driven
> primitives (`shape`/`size` in `data/machines/home.ron`, `body_m`/`cabin_m`
> in `data/vehicles/kits.ron`). This guide documents the format rules that
> are ALREADY enforced by the loader, so models authored today load
> correctly the day the wiring lands. The wiring plan + its one known hazard
> are at the bottom.

## The format decision (operator, 2026-07-04)

- **GLB (binary glTF) is THE game format.** Never FBX.
- **STL stays for 3D printing** (the Prusa). GLB is not a print format; an
  export path from game models to STL can come later.
- `.gltf` (JSON + external buffers) also loads, but prefer single-file `.glb`
  so a model never ships with missing buffer files.

## Authoring rules (what the loader actually does)

These come from reading `load_gltf` — they are the engine's real behavior,
not aspiration:

1. **Units are meters.** The engine applies NO scaling on load: a 4.85 m car
   must be 4.85 m in the file. Blender: work in meters, apply scale
   (Ctrl+A → Scale) before export.
2. **Y is up, -Z is forward** (glTF convention; Blender's exporter converts
   automatically when "+Y up" is checked, which is its default).
3. **Only the FIRST mesh's FIRST primitive is used.** Join your objects into
   one mesh before export (Blender: select all → Ctrl+J). Multi-material
   models: not yet — one material slot.
4. **Indexed triangles are required.** Export as triangles (the Blender GLB
   exporter does this by default). A mesh without indices is rejected.
5. **Normals are optional** — missing normals get flat face normals
   generated. Smooth-shaded models should export their own normals.
6. **UVs are optional** — missing UVs get simple planar projection. Textured
   models should export real UVs (TEXCOORD_0).
7. **Textures/images inside the GLB are currently ignored** (the loader
   reads geometry only; materials come from the engine's typed material
   system). Don't waste file size embedding 4K textures yet.
8. **Keep it light.** These render alongside a whole homestead: aim for
   game-prop budgets (a vehicle in the low tens of thousands of triangles,
   a machine well under that), not sculpt-resolution meshes.

## Where model files live

- **Contributors / dev repo:** `assets/models/<domain>/<name>.glb`
  (e.g. `assets/models/vehicles/nova_1975.glb`). `assets/` is the shared
  media tree; the repo is the source of truth.
- **Players / mods (once wired):** the loader resolves paths against the
  game's DATA directory, which is the hot-reloadable, player-moddable tree —
  so distributed model references will live under `data/models/…`. Portable
  installs keep `assets/` next to the exe; installed builds get models
  shipped into `data/`.
- **Sharing:** `.glb`/`.gltf` (plus `.blend`, `.stl`, `.obj`) attached in
  chat auto-publish to the server's public Shared Files library
  (`share=1` upload; see the Files page) — that's the community exchange
  path for models today.

## How a model will attach to a game object (the wiring plan)

Planned shape (same infinite-of-X pattern as everything else — a data field,
no code per model):

- `data/machines/home.ron`: a machine def gains `model: Some("models/….glb")`
  → the walk-up world object renders the mesh instead of its primitive
  (primitive stays as the fallback when the file is absent/broken).
- `data/vehicles/kits.ron`: a kit row gains the same optional `model` field
  for the deployed vehicle body.

**Known hazard for whoever wires it (found 2026-07-06):** `load_gltf`
caches one mesh index per path, so two silos share ONE renderer mesh. The
construction editor's rebuild fast-path calls `renderer.replace_mesh(mi, …)`
per machine object — run over a model-backed machine it would overwrite the
SHARED cached GLB mesh with a primitive (visual revert + a stale cache that
maps the path to a primitive). The wiring must either exempt model-backed
objects from the replace/reuse path, or give the editor its own primitive
proxies while dragging. Plan accordingly; don't ship the naive version.

## Checklist for adding a model (today)

1. Author in Blender at real-world meters, Y-up export defaults.
2. Join to one mesh, triangulate, apply transforms.
3. Export GLB → `assets/models/<domain>/<name>.glb`, snake_case name.
4. Commit it (Git handles binaries fine at these sizes) — or attach it in
   chat to publish it to the server's Shared Files for others.
5. When the `model:` field lands, reference the path from the RON row and
   the object renders it.
