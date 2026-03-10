# Planet Runtime Plan: Icosahedral + Procedural Voxel/Field Hybrid

## Yes — this is the right direction

Use an icosphere-based planetary surface (20 -> 80 -> 320 -> 1280 faces...) as the top-level spatial partition, then adaptive subdivision + procedural generation per patch.

## Key reality check

A full Earth mesh at ~1 meter edge length *globally* is not feasible as a single static mesh.

Approximation:
- Icosphere triangles at subdivision `s`: `T = 20 * 4^s`
- Earth-scale average edge length drops roughly with `~ 1 / 2^s`
- 1m global edge requires on the order of `s≈23` (astronomical triangle counts)

So the practical approach is:
- low-resolution global shell
- high-resolution local patch refinement around player
- procedural detail regenerated as needed

## Recommended architecture

1. **Planet shell** (icosphere faces as root patches)
2. **Patch quadtree per face** (LOD split/merge)
3. **Procedural displacement/noise stack** for terrain
4. **Biome system** driven by latitude/elevation/moisture/temp noise
5. **Optional voxel/material layer** for dig/build edits in local chunks

## Culling strategy

- Backface culling: enabled in normal rendering
- Frustum culling: reject patches outside camera frustum
- Horizon culling: reject patches hidden by planet curvature
- Hidden-face culling (voxel chunks): generate only exposed faces for edited voxel chunks

## LOD strategy

- Radius/distance thresholds select desired patch subdivision level
- Hysteresis to avoid LOD popping thrash
- Geomorph or skirts to hide cracks between neighboring LOD levels

## Performance target philosophy

- Render only local high-detail rings around player
- Keep far-field to low-poly shell + atmospheric scattering
- Stream/generate patch meshes asynchronously

## Immediate implementation phases

1. Core math crate for icosphere counts, edge-length estimates, and LOD selection
2. Runtime integration for single-planet shell + distance LOD rings
3. Add procedural biome/height synthesis
4. Add local voxel edit chunks + exposed-face meshing
