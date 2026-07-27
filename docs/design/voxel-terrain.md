# Voxel terrain on the icosphere (cliffs, overhangs, caves, digging)

Status: DIRECTION DOCUMENT (2026-07-27). Nothing here is built yet; this
records the operator's intent and the engineering path so increments can
start from a shared picture.

Operator ask (2026-07-26, bedtime directive): "can you try to recreate the
white cliffs of dover? We need to get cliffs and overhangs and tunnels/caves
working properly... more like voxels using an ico sphere instead of a cube
grid, so theoretically we should be able to dig through terrain no problems.
Eventually... asteroids striking the surface leaving their signature craters
... explosions... shockwave, smoke cloud, debris."

## What today's terrain can and cannot do

The chunked-LOD planet surface is a HEIGHTMAP: one radius value per
direction from the planet center (elevation sampled per icosphere-grid
point, displaced radially). That gives mountains, valleys, and trenches at
1 m fidelity, but structurally CANNOT express:

- Overhangs (two surface points on the same ray).
- Caves and tunnels (interior voids).
- Vertical-to-negative cliff profiles (Dover's chalk faces).
- Digging (removing material needs a volumetric representation).

The voxel asteroids (`src/terrain/` sparse octree) already do volumetric
terrain, but in a local Cartesian frame, planet-scale needs the icosphere
frame.

## Direction: prismatic voxels on the icosphere grid

The natural "voxel grid" for a sphere is the existing icosphere subdivision
crossed with radial shells: each cell is a spherical prism (a quad/tri
footprint on the sphere at some depth, extruded between two radii). This is
the "ico-voxel" the operator described:

- Column addressing reuses the PatchId scheme (face + quadtree path), so
  voxel chunks and heightmap patches share the same spatial index.
- Radial layers are fixed-thickness shells near the surface (1-2 m), growing
  geometrically with depth (nobody digs 100 km down at 1 m fidelity).
- Storage is sparse: a column stores only the layers that DIFFER from the
  heightmap default (solid below elevation, air above). An untouched planet
  costs ZERO extra bytes, which is the property that makes planet-scale
  voxels tractable at all.

## Hybrid, not replacement

The heightmap stays the base truth for the whole planet. Voxel data is an
OVERLAY that exists only where geology or gameplay created it:

1. Authored/procedural features (Dover cliffs, cave systems, lava tubes):
   generated voxel patches placed by the terrain seed, like vegetation
   streams are today.
2. Player edits (digging, explosions): sparse deltas written at runtime,
   persisted with the save.
3. Impact events (asteroid craters): a crater is mostly a heightmap
   DEPRESSION (cheap, no voxels needed) plus a voxel rim/overhang shell
   where the lip folds over.

Meshing: marching cubes (or dual contouring for sharp cliff edges) per
voxel chunk, in the patch's local frame, output as ordinary patch geometry
into the SAME patch-arena draw path (docs/design/terrain-draw-batching.md).
The chunked-LOD selection treats a voxel-overlaid patch exactly like any
other patch; only the builder differs.

## Increment ladder (each shippable)

1. **Voxel overlay data model + persistence**: sparse column store keyed by
   PatchId, save/load, no rendering yet. Tests: roundtrip, zero-cost when
   empty.
2. **Meshing bring-up in a sandbox**: one hand-authored cave/overhang patch
   meshed and drawn via the arena, LOD-locked (no seam handling yet).
   Proves the visual.
3. **Dover increment**: procedural chalk-cliff generator along selected
   coastlines (steep heightmap gradient + voxel overhang lip), seam-blended
   into neighboring heightmap patches with the existing skirt scheme.
4. **Digging tool**: sphere-subtract brush writing sparse deltas, remesh on
   edit, physics collider refresh (rapier local mesh swap).
5. **LOD + seams hardening**: cross-depth stitching between voxel and
   heightmap patches (the hard 20 percent; do it after the fun 80 has
   proven value).
6. **Impact events**: crater = heightmap depression + rim voxels + the
   existing particles/audio for shockwave, smoke, debris. Persist craters
   as world deltas.

## Physics and gameplay hooks

- Colliders come from the same meshed chunks (rapier trimesh per chunk,
  swapped on remesh), matching how ship interiors already work.
- Dug material becomes inventory items via the existing mining loop
  (ore/stone yields by biome + depth ties into data/chemistry).
- Translucent/volumetric ordering caution (operator): explosion smoke and
  debris volumes must respect the view-dependent transparent ordering the
  cloud/atmo shells use; craters themselves are opaque and safe.

## Non-goals for now

- Whole-planet uniform voxelization (storage suicide; the sparse overlay is
  the design).
- Real-time fluid/lava simulation in voxel space.
- Multiplayer edit sync (needs the ECS sync channel; design when digging
  ships).
