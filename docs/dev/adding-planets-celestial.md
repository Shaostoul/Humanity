# Adding Planets and Celestial Bodies

How the solar system is defined in data, how a body gets a walkable detailed
surface, and how the real-imagery pipelines work. Terrain internals live in
[docs/design/terrain-detail.md](../design/terrain-detail.md); this is the
content-adding workflow.

## Two layers of definition

**Layer 1 - the catalog: `data/star_systems/sol.json`** (69 bodies as of
2026-07-20: 1 star, 4 terrestrials, 40 moons, 2 gas giants, 2 ice giants,
5 dwarf planets, 10 asteroids, 5 comets). Loaded by `src/cosmos.rs`
(`sol_bodies()`, one Kepler propagator drives all orbits) and embedded at
compile time (`src/embedded_data.rs`). Each body entry:

- `id`, `name`, `type` (`star | terrestrial | moon | gas_giant | ice_giant |
  dwarf_planet | asteroid | comet`), `parent` (orbit center id)
- `orbit`: `semi_major_axis_au`, `eccentricity`, `inclination_deg`,
  `orbital_period_days`, `mean_anomaly_deg`,
  `longitude_ascending_node_deg`, `argument_perihelion_deg` (J2000 epoch,
  NASA/JPL Horizons data)
- `physical`: `mass_kg`, `radius_km`, `surface_gravity_ms2`,
  `rotation_period_hours`, `axial_tilt_deg`, `mean_temperature_k`, `albedo`
- `atmosphere` (composition percentages + surface pressure), `rings`,
  `moons` (child ids), `discovery`, `description`

**Adding a moon, asteroid, or comet = adding one JSON entry here** with real
orbital elements (JPL Horizons or the NASA fact sheets, the file's `meta`
block names its sources and units). Set `parent` correctly and add the id to
the parent's `moons` list for moons. It then exists in the Cosmos page, the
sky, and the 3D system with no code.

**Layer 2 - the detailed surface: `data/planets/<id>.ron`** (a `PlanetDef`,
struct in `src/terrain/planet.rs`). At world load, `src/lib.rs` walks every
sol.json body and checks whether `data/planets/<id>.ron` exists; if yes, the
body gets an icosphere LOD surface you can approach and walk on. Shipped:
`earth.ron`, `mars.ron`, `moon.ron`, `pluto.ron`. **Giving a body a real
surface = writing this RON file**, named exactly after the sol.json id.

## PlanetDef fields (see `src/terrain/planet.rs` for defaults and full docs)

Core: `name`, `radius` (meters), `gravity`, `terrain_seed`, `ore_seed`,
`orbital_radius`, `orbital_period`, `rotation_period`, `axial_tilt`.

Atmosphere: `atmosphere_color` (`Option`; rgb = relative per-channel
scattering strengths, a = overall density, consumed by the v0.807 analytic
scattering, material type 14), `atmosphere_scale` (shell thickness as radius
fraction), `scale_height_m`, `cloud_coverage` (`Option`; omit for cloudless
worlds, Mars deliberately has none).

Surface: `has_water`, `sea_level`, `surface_relief` (vertical scale as a
fraction of radius; Earth uses 0.003123 which is TRUE scale for its heightmap
window, see the long comment in `earth.ron` before touching this),
`noise_frequency`, `noise_octaves`, and the fallback band palette
(`land_color`, `water_color`, `shore_color`, `highland_color`,
`mountain_color`, `cap_color`, `polar_cap_latitude`).

Real-data grids (both `Option`, both bypass the procedural equivalents):
`heightmap` and `albedo`, paths relative to `data/`.

`data/planets/earth.ron` is heavily commented and is the reference example;
copy it and prune for a new body.

## The albedo pipeline (HOSALB1)

Real surface imagery ships as `data/planets/<id>_albedo.bin` in the HOSALB1
container (`src/terrain/planet_albedo.rs`):

```
bytes 0..7    magic b"HOSALB1"
bytes 7..11   u32 width  (longitude samples, wraps)
bytes 11..15  u32 height (latitude samples, clamps at poles)
bytes 15..    width*height*3 sRGB bytes, row-major RGB,
              row 0 = northernmost, col 0 = westernmost (lon -180)
```

Builders (node, no npm deps):

- `scripts/build-earth-albedo.js` - Earth only (NASA Blue Marble, needs the
  matching elevation grid for the per-texel water mask).
- `scripts/build-planet-albedo.js <in.png> <out.bin> <w> <h> [--body ...]
  [--roll180] [--fill-nodata]` - every other body. `--roll180` fixes sources
  published with lon 0 at column 0 (the New Horizons Pluto mosaic);
  `--fill-nodata` heals black no-data regions (Pluto's polar-night south).
  `--body moon|mars|pluto` samples real-feature anchor points and HARD-FAILS
  the bake if they read wrong (a flipped or channel-swapped file cannot
  pass). The script's header comment doubles as the source-URL + license
  record; keep it current, and mirror new sources in `CREDITS.md`.
- `scripts/dump-albedo-png.js` - visual inspection of a baked .bin.

At load, samples are decoded to linear and graded by
`grade_albedo` (`src/terrain/planet_surface.rs`): ocean-floor darkening, a
land gain with shadow/vegetation lift (`land_gain`, smoothstep on the
green-dominance measure), and the sea-ice cap blend from `cap_color`. The
same function feeds both the per-face fallback and the baked GPU texture, so
face colors and texture colors agree by construction.

## Heightmaps and streamed tiles (Earth today, any body tomorrow)

- Base grid: `data/planets/earth_heightmap.bin` from NOAA ETOPO1, built by
  `scripts/build-earth-heightmap.js` (0.05 degree, ~5.5 km cells; same header
  family as HOSALB1, see `src/terrain/planet_heightmap.rs`). When a heightmap
  loads, the RON's hand-tuned `sea_level` is OVERRIDDEN with the grid's true
  0 m position so the real coastline is exact; on load failure the game warns
  and falls back to procedural noise (a missing grid must never blank a
  planet).
- Detail tier: `data/planets/earth_tiles/` holds optional 15x15-degree tiles
  at 15 arc-seconds (~460 m cells, `N00E000.bin` naming), streamed by
  `src/terrain/terrain_tiles.rs` (center + 8 neighbors resident, ~26 MB
  each). Built locally by `examples/build_earth_tiles.rs` from NOAA
  ETOPO 2022.

## Gas giants: the type-18 shader path

Gas and ice giants have no PlanetDef surface; they render via material type
18 in `assets/shaders/pbr_simple.wgsl`. The four materials are created in
`src/lib.rs`:

```rust
renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 0.0) // jupiter
// params.w selects the palette: 0 jupiter, 1 saturn, 2 uranus, 3 neptune
```

The shader builds latitude-band palettes warped by value noise (Jupiter gets
the Great Red Spot, Neptune a dark storm oval) and falls through to the
shared sun-lit path, so the terminator and eclipse shading come free. A NEW
giant palette = a new `params.w` band in the type-18 branch (see
[adding-shaders.md](adding-shaders.md) for the dispatch rules and verify
bar).

## Checklist: adding a body

1. Catalog entry in `data/star_systems/sol.json` with real JPL elements
   (skip if the body already exists there; most do).
2. Want a walkable surface? Write `data/planets/<id>.ron` starting from
   `earth.ron` (or `moon.ron` for airless bodies). Procedural noise + the
   band palette is a perfectly good v1; grids are optional upgrades.
3. Real imagery available? Bake an albedo with `build-planet-albedo.js`
   (add anchor checks for the new body if you can name real features), add
   the source + license to the script header and `CREDITS.md`, reference it
   from the RON.
4. `just validate-data`, then boot and fly there: use the camera protocol
   (`debug/camera_request.json` with body/lat/lon/altitude, see
   [performance-profiling.md](performance-profiling.md)) + the screenshot
   protocol to verify from orbit and from the ground.
5. Description surfaces: `data/planets/tooltips.json` feeds the Cosmos page
   hover info; keep it in sync for new bodies.
