# Real oceans (design, 2026-07-17)

> Operator direction (2026-07-16 night): "we don't actually have an swimmable
> ocean but, a solid textured shader. Can we some how have an actual ocean with
> physics similar to the video game From The Depths but, better? I'd love to be
> able to sail ships, pilot submarines, crash spaceships, and drop asteroids in
> the oceans with semi-realistic wave effects and displacement. I'd love to
> experiment with deep diving vehicles and extreme pressures on vessels deep
> under water (or elsewhere.) Obviously we can't do real but, we at least
> actually want water."

## The one structural insight

**No uniform water sphere.** Parts of Earth's land sit below sea level (Death
Valley, the Dead Sea shore, the Caspian depression); a sphere of water at
0 m would flood them. The fix is an **ocean mask**: flood-fill the heightmap
from a known open-ocean seed cell across every cell with elevation <= 0 m.
Cells reached by the fill are ocean; below-sea-level cells NOT reached are dry
basins. Precompute once to `data/planets/earth_ocean_mask.bin` (1 bit per
heightmap cell, ~810 KB at today's 3600x1800 grid) with a build script beside
`build-earth-heightmap.js`. ETOPO already carries bathymetry, so depth under
the ocean surface is real data - submarines get a real seafloor.

## The golden rule (from the v0.835 see-through-ground incident)

Whatever the water DRAWS must be exactly what physics SAMPLES. The wave field
is an analytic function `wave_height(lat, lon, t)` (sum of Gerstner waves)
evaluated identically in the water shader (vertex displacement) and on the CPU
(buoyancy, swim, ship physics). Never two sources of truth.

## Stages

**Stage 0 - groundwork (cheap, do first)**
- `scripts/build-ocean-mask.js`: flood fill -> `earth_ocean_mask.bin`; loader +
  tests in `terrain/` mirroring `planet_heightmap.rs`.
- Data plumbing: `PlanetDef.ocean_mask: Option<String>`.

**Stage 1 - a real water surface + swimming**
- Ocean rendered as its own chunked patch sphere at sea-level radius, only
  where the mask says water (reuses the terrain quadtree/skirt machinery).
- Gerstner wave displacement in the shader; the SAME function on CPU.
- Player: below `wave_height` = submerged -> buoyancy toward the surface,
  swim controls (slower, damped), camera underwater tint + fog.

**Stage 2 - things float**
- Archimedes on a submerged-volume proxy (AABB slices or a few sample
  points per vehicle): buoyant force, water drag, wave-following tilt.
- Ships sail (thrust first, wind later); crates/debris bob.

**Stage 3 - depth, pressure, submarines**
- Pressure p = rho * g * depth; every vessel gets a data-driven
  `hull_rating_m` (kits.ron / machines): past rating -> creak warnings ->
  damage -> implosion. Ballast control for subs.
- Two-realities tie-in: the Library teaches the REAL dive/pressure math the
  sim runs (it is the same formula).

**Stage 4 - displacement events**
- Impacts (crashing spaceship, dropped asteroid) spawn analytic radial wave
  packets superposed on the Gerstner field, decaying with distance/time -
  localized math, not a global fluid sim. Splash particles + sound.

**Non-goals:** real CFD, global dynamic sea level, erosion. Analytic waves +
masks + Archimedes gets "semi-realistic" at simulation-game cost.

## Higher-detail terrain for the same space (the star-catalog pattern)

Current: one global 0.1-degree grid (11.1 km cells) - why Mount Fuji renders
as a pyramid; the mesh (54 m triangles) far out-resolves the data.

Planned tiers, mirroring the star catalogs (ship small, download big):
1. **Shipped base**: rebuild at 0.05 deg (7200x3600, ~52 MB) from a better
   source; bicubic sampling (shipped v0.868) smooths between cells and
   mildly restores averaged-away peaks.
2. **Downloadable region tiles**: quadtree tiles cut from ETOPO 2022
   15-arc-second (~460 m cells, global INCLUDING bathymetry - the successor
   to ETOPO1), int16 + compression, streamed by the existing patch quadtree
   (tile granularity matches patch granularity). Fuji becomes a cone here.
3. (Later, optional) land-only 30 m tiles (Copernicus GLO-30) for hero
   regions.

Source data: ETOPO 2022 (NOAA, public domain). The 15-arc-second global
grid is ~7 GB - a one-time download to the dev machine; we cut and ship
derived artifacts only.
