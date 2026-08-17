# OSM 3D Extrusion: the build plan (maps ladder rung 3, increment 2)

> Recon 2026-08-17, every finding cited from the actual code. Execute this
> plan as written; re-derive nothing. Companion: docs/design/maps-ladder.md.

## Findings

### 1. The chunked-LOD Earth and the lat/lon -> world mapping

**World frame.** Earth is the celestial frame origin: `body_center` for
earth is `DVec3::ZERO` (src/lib.rs:3957-3958), the camera's absolute
position is `state.ship_world_pos + camera.position` in f64
(src/lib.rs:3976-3981), and every celestial object is placed via
`render_off = rel_earth_m - state.ship_world_pos` (f64, src/lib.rs:8738).
renderer/floating_origin.rs:1-36 documents the DVec3-absolute /
f32-camera-relative discipline; CLAUDE.md states the hard rule: planet
positions and unit directions stay f64 until a coarse-data boundary.

**Chunked LOD.** src/terrain/planet_chunks.rs:1-74 is the design: a
quadtree over the 20 icosahedron faces, PATCH_TESS = 16 (line 105),
MAX_PATCH_DEPTH = 13 (~54 m triangles) rising to TILE_MAX_PATCH_DEPTH = 20
(~0.42 m) when the streamed tile tier is installed; split at 12 projected
px; 24 patch builds/frame; 1400 MiB LRU. Precision: each patch stores an
f64 anchor and f32 vertex offsets; the per-draw translation is composed in
f64 (`render_off + rot_d * e.anchor`) and narrowed to f32 at the very end
(src/lib.rs:9351-9364). Selection runs in the planet's unrotated local
frame (`cam_local = rot_d.inverse() * (cam_render - render_off)`,
src/lib.rs:8888), with `rot_d = DQuat::from_rotation_y(spin_f64)`
(src/lib.rs:8876; spin from current_planet_spin,
src/engine/frame_lock.rs:104-122). Patches draw batched
(renderer.patch_draws) or as classic RenderObjects in celestial_objects
(src/lib.rs:9341-9461), rendered by render_celestial_onto.

**The lat/lon mapping exists and is test-locked.**
`latlon_to_dir(lat_deg, lon_deg) -> Vec3` and `dir_to_latlon_deg` (+ an
f64 twin for the forward direction) live in
src/terrain/planet_heightmap.rs:47-82 (+Y = north pole, east = -z;
round-trip test at 320-340). The probe rig's camera_request uses exactly
this (src/engine/ipc.rs:823-891). `latlon_to_dir` is f32-only today; the
region pipeline needs an f64 twin.

### 2. Terrain height query

`ground_radius_m(def, hm, detail, tiles, unit_dir: DVec3) -> f64`
(src/engine/frame_lock.rs:48-95) is THE drawn-ground oracle: routes
through drawn_elevation_normalized (src/terrain/planet_chunks.rs:633-660)
= base grid or resident 460 m tile + land-masked fine detail noise at
FINEST_DETAIL_DEPTH = 24, then displaced_radius_f64_true
(src/terrain/planet_surface.rs:172-176). Earth's surface_relief 0.003123
(data/planets/earth.ron:83) is TRUE 1x vertical scale, so real building
heights read correctly. The player walk is an analytic clamp against this
same function; there is NO terrain collider at all (src/lib.rs:3985-4030).
Base grid 7200x3600 (~5.5 km cells); optional 15 arc-sec (~460 m) tiles
stream via terrain_tiles.ensure_region(lat, lon), called per frame from
the earth branch (src/lib.rs:8925-8942). Camera parking clamps below-sea
ground to radius_m + SURFACE_LIFT_M (src/engine/ipc.rs:888-890): the same
clamp waterfront buildings need.

### 3. How static geometry is meshed and drawn today

- Vertex = position/normal/uv, no per-vertex color (src/renderer/mesh.rs:
  9-13); Mesh::from_vertices; renderer.add_mesh (src/renderer/mod.rs:1669);
  RenderObject { position, rotation, scale, mesh, material, fade }.
- Materials via add_material_typed(base_color, metallic, roughness, type):
  0 panel grid, 1 brushed metal, 2 concrete, 3 wood
  (src/renderer/materials.rs:47-60). One material per RenderObject, so
  per-material batching = one mesh per material class.
- Ship rooms show the raw quad-emitter pattern (src/ship/rooms.rs:20 ->
  (Vec<Vertex>, Vec<u32>)), consumed via Mesh::from_vertices
  (src/engine/home_meshes.rs:23-183).
- **The on-planet precedent is near trees** (src/lib.rs:9640-9762):
  planet-local f64 base `dir * r_m`, per-frame
  `pos_render = render_off + rot_d * base_local` narrowed last,
  `obj_rot = rotation * Quat::from_rotation_arc(Vec3::Y, dir) * yaw`,
  pushed into celestial_objects. Buildings ride the same math.
- Background mesh-build precedent: sky-sphere on a std::thread + mpsc,
  GPU upload on arrival (src/lib.rs:10922-10943).
- No polygon triangulator exists in the repo; the ear clipper must be
  written.

### 4. Region data and where it loads

- HOSMREG1 spec + projection contract: scripts/fetch-osm-region.mjs:44-156;
  the constants `111320.0 * cos(origin_lat)` and `110540.0` are explicitly
  part of the CONTRACT; the Rust reader must use the same two numbers.
- The only Rust reader today is the Maps 2D page: parse_region +
  map_regions() (src/gui/pages/cosmos.rs); it is private to the page and
  must move to a shared module.
- Measured seattle-center.bin content: 8,966 roads, 1,457 buildings (711
  with height, max 259.0 m: Rainier Square Tower), 18,073 footprint
  points, half spans 1050.6 x 1271.2 m, origin 47.6165, -122.342.
  ~50k roof+wall triangles + ~100-150k road-ribbon triangles worst case:
  a handful of draw calls. Load: parse the 486 KB file once at world entry
  (the ocean_mask pattern, src/lib.rs:1672; state fields in
  src/engine/state.rs:178-241), distance-gate the MESH BUILD to proximity
  like terrain tiles.

## The increment plan

Realistic-first check: footprint -> ear-clipped roof cap + wall quads,
terrain-conformed bases, per-region batched draws, real geodetic placement
IS the reference architecture (OSM2World / Cesium do exactly this).
Nothing here is a throwaway.

### Build steps

**1. New pure module src/terrain/osm_region.rs** (ungated,
headless-testable, in the style of planet_heightmap.rs).
- Move the HOSMREG1 parser here from cosmos.rs; cosmos.rs re-uses it.
  Keep the exact-EOF walk.
- Projection contract in Rust:
  `region_meters_to_latlon(origin_lat, origin_lon, e_m, n_m) -> (f64, f64)`
  using literally 111320.0 * cos(origin_lat) and 110540.0 (all f64).
- Add `latlon_to_dir_f64(lat, lon) -> DVec3` beside the f32 version, same
  handedness, locked to the f32 version by a test. **f64 discipline per
  step**: region meters (f32 in file, exact) -> lat/lon f64 -> unit dir
  f64 -> drawn radius f64 -> `dir * r - region_anchor` f64 -> narrow the
  ANCHOR-RELATIVE offset to f32 (offsets <= ~1.7 km, ulp ~0.1 mm). The
  region anchor (origin_dir * r_origin, f64, planet-unrotated frame) is
  the only planet-scale quantity and never passes through f32.

**2. Ear-clipping triangulator** in the same module. Signed-area winding
normalization, reflex-aware ear test, O(n^2) (typical ring ~12 points).
Defensive: a ring that fails to clip (self-intersecting OSM data) is
skipped and counted, never panics. Unit tests: convex, L-shape, collinear
runs, both windings, degenerate.

**3. Extrusion mesher**
`build_region_meshes(region, &elev) -> Vec<(Vec<Vertex>, Vec<u32>)>` per
material class, where `elev` closes over ground_radius_m inputs (hm +
DetailNoise + resident tiles: the same drawn ground the walk clamp uses).
- **Buildings**: per ring vertex, f64 dir + drawn radius; base radius =
  min over the ring, clamped to >= sea radius + SURFACE_LIFT_M (coastal
  cells average in Puget Sound bathymetry); walls from base - 2.5 m
  (buried skirt hides slope/LOD disagreement) to base + height; height
  0.0 (unknown) -> class default ~6 m; roof = ear-clipped cap, normal =
  local radial up. Positions are f32 offsets from the region anchor.
- **Height-class material split** (one mesh per class; Vertex has no
  color channel): unknown/low (<12 m) warm masonry, mid (<50 m) concrete
  (type 2), high (>=50 m) glassy (metallic ~0.6, roughness ~0.25).
- **Roads: flat ribbon meshes, not texture overlay**: the terrain has no
  decal/splat path (an overlay would be a shader+pipeline change), while
  ribbons ride the classic-object path unchanged. Class widths (motorway
  14 m ... footway 2 m), lift +0.18 m vehicular / +0.12 m footway above
  drawn ground, mitered joins in the local tangent plane. **Resample
  polylines to <= ~15 m segments before draping** so ribbons follow the
  fine relief instead of chording through it. Two meshes: asphalt (dark,
  rough) + footway (light gray).

**4. Engine residency + draw** in a NEW src/engine/region_meshes.rs
(the engine/near_tree_models.rs extraction precedent; do not grow lib.rs),
with small state fields in src/engine/state.rs (osm_regions parsed at
world entry; osm_region_active: Option<RegionRuntime> holding mesh
indices, anchor, built_with_tiles, pending channel).
- Hook in the earth chunked branch beside terrain_tiles.ensure_region
  (src/lib.rs:8925-8942): camera within ~40 km of a region origin spawns
  a background build (thread + mpsc) with tiles if resident; upload on
  arrival. If terrain_tiles.poll() later delivers the region's tile
  (Seattle = N60W135) and the build was base-only, rebuild once.
- Per frame while active, push one RenderObject per class mesh into
  celestial_objects: position = (render_off + rot_d * anchor) narrowed
  last, rotation = the planet spin quat (vertices are unrotated-planet-
  frame offsets, exactly like classic patches at src/lib.rs:9372-9383),
  fade 0.0. ~6 objects vs MAX_OBJECTS 16384. **One batch per class for
  the whole 2.5 km region, distance-gated ~40 km in / ~60 km out**
  (eviction via replace_mesh + free-slot, src/lib.rs:10863-10873). No
  sub-region chunking at this size; that is the scaling lever later.
- ODbL: an in-world drawing of a region is a DISPLAY of OSM data; surface
  "Data (c) OpenStreetMap contributors, ODbL" in the F3/diag overlay
  while a region is drawn (license obligation).

**5. Colliders: none in this increment.** The planet surface itself has
no rapier colliders (walking is an analytic radial clamp) and trees do
not collide either. The cheap follow-up in the engine's analytic style:
point-in-footprint test in region meters to push the walker out (or
stand on the roof).

**6. Docs/bookkeeping**: maps-ladder rung 3 increment 2 status, FEATURES,
PRIORITIES; keep the new terrain module relay-safe (pure std; verify with
the relay feature check) and gate the engine/renderer parts native.

### Verification plan

- Unit tests (headless, osm_region.rs): parser rejects truncated/bad
  magic/trailing bytes; ear-clip suite; **meters -> geodetic -> world
  round-trip** (grid of region points through region_meters_to_latlon ->
  latlon_to_dir_f64 -> dir_to_latlon_deg_f64 -> back to meters, assert
  < 1e-4 m drift); shipped-file anchor test (8,966 roads / 1,457
  buildings, origin 47.6165/-122.342, Pike Street exists, tallest 259 m).
- Runtime: add vantages to tests/visual/vantages.json:
  seattle-osm-ground (earth, lat 47.6205, lon -122.3493, altitude_km 0.3;
  expect extruded blocks + a ~259 m tower + roads conforming, no shimmer)
  and seattle-osm-5km (overview). Probe-sweep both, panics=0, Read the
  PNGs. Plus just verify + the relay check before pushing.

### Genuine risks

1. **Heightmap coarseness vs building-scale placement (the big one).**
   Without tiles, ground under Seattle Center is a 5.5 km-cell
   interpolation; with tiles, 460 m cells + synthetic fine octaves.
   Absolute elevations off by meters-to-tens-of-meters; slope across a
   footprint is fictional at building scale. Mitigations baked in:
   min-over-ring base + 2.5 m skirt, sea clamp. Residual: buildings on
   synthetic micro-hills; honest for increment 1.
2. **Terrain LOD popping under buildings.** Bases sample
   FINEST_DETAIL_DEPTH (matches the walk clamp and finest patches), but
   coarser distant patches disagree by up to ~4 m, so mid-distance
   buildings can float/sink until LOD refines (near-trees accept the
   same). The skirt hides most; no per-frame re-anchoring this increment.
3. **Road z-fighting.** 0.12-0.18 m lift over reverse-Z depth is safe
   near, grazing-angle shimmer possible at distance; class-ordered lifts
   prevent road-on-road fights. If shimmer shows in the probe capture,
   the fix is a depth-bias in the pipeline, not more lift.
4. **Tile-arrival elevation shift**: the one-shot rebuild flag; without
   it pre-tile buildings hover over tile-refined ground.
5. **Bad rings** (self-intersecting footprints survive the fetcher): the
   ear clipper skips-and-counts, never panics; probe panics=0 gates the
   integration.

## Implementation files

- src/terrain/osm_region.rs (NEW: parser, projection contract, ear clip,
  extrusion mesher, unit tests)
- src/engine/region_meshes.rs (NEW: residency, background build,
  RenderObject push; state fields in src/engine/state.rs)
- src/lib.rs (celestial-pass hook ~8925-8942; classic-object push pattern
  9341-9463)
- src/engine/frame_lock.rs (ground_radius_m, the shared drawn-ground oracle)
- src/gui/pages/cosmos.rs (swap its private parser for the shared module)
- tests/visual/vantages.json (new Seattle vantages)
