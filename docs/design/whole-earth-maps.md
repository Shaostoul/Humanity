# Whole-Earth maps: one grid for the map and the ground

Written 2026-11-08 from a read of the region pipeline, the terrain quadtree and the history of the
Silverdale waterfront, together with the dated licence research in
[`2026-11-08-whole-earth-map-data.md`](../reference/findings/2026-11-08-whole-earth-map-data.md)
(the finding; every licence statement below is quoted and dated there). **Rung 1 is built
(2026-11-09: the coast yardstick, the tile key and the credit; its outcome closes section 13);
nothing after it is.** The rungs in section 13 are the build order; PRIORITIES.md ranks when.

The operator, verbatim (2026-11-08):

> "What was the best fix for no longer having OpenStreetMap issues? We're eventually wanting to do
> the whole Earth so, we seriously need to figure out how we're going to sort out the problems we
> have here. Like, we have the Earth made of the triangular mesh for terrain height and what not.
> Right now we have a weird rectangle of Silverdale for a map but, we need to figure out how we're
> going to match the maps better."

> "We did have a problem where the heightmap of the world and the Silverdale waterfront area wasn't
> even. Like in the water front initially didn't have any water and then we had weird sheer cliffs
> along water area instead of nice slopes like in real life."

> "Let's figure out which one would actually be best for us."

Companions: [maps-ladder.md](maps-ladder.md) (rung 3, the OSM work this replaces),
[osm-extrusion-plan.md](osm-extrusion-plan.md) (the extrusion mesher, kept),
[maps-zoom-to-anything.md](maps-zoom-to-anything.md) (the Maps page's one camera; its increment 7
draws these tiles), [terrain-detail.md](terrain-detail.md) (the Earth-under-10-GB budget and the
cliff strategy), [terrain-draw-batching.md](terrain-draw-batching.md) (the patch arena the map
geometry rides).

## 0. In short

- **The OSM answer.** Keep complying (credit where drawn, ODbL on the derived files) and finish the
  two gaps still open: the ODbL offer must travel in the release archive and inside every map pack,
  and the map credit must not vanish with the HUD in the construction editor and the showroom.
  Nothing permissive replaces OSM for roads, water and names. What we split is not OSM out but the
  other layers away from it: heights, seabed and land cover stay their own layers in every file and
  are joined to OSM only at run time, so share-alike never reaches them.
- **One grid.** Map data is cut into tiles keyed by the terrain's own `PatchId` (one of the 20
  icosahedron faces, a depth, a path). The tile a patch needs is its ancestor at a data level,
  found by masking the path bits. The map, the heights and the mesh then share one grid, one level
  of detail and one set of streaming decisions. The rectangle disappears because tiles cover the
  whole sphere.
- **One height source per domain, one formula.** Land heights come from one bare-earth model at
  every zoom, seabed from one bathymetry, and the OSM coastline decides which owns each point. The
  shore is blended at run time inside the one drawn-elevation formula (the BUG-156 rule), so the
  ground, the water, the roads, the buildings, the trees and the player's feet cannot disagree.
- **No more sheer walls or dry harbours.** The walls came from a coarse grid meeting a vector carve
  at a polygon edge; the dry inlet came from 460 m cells averaging a 1.5 km inlet onto land. With
  30 m land heights that already agree with the coast, a nearshore profile instead of a constant
  carve depth, and sea polygons assembled for the whole planet instead of clipped to a box, both
  causes are gone, and a rig gate (section 5.5) proves it at Silverdale, Dover and Norderney.
- **One source per layer:** GEDTM30 for land heights, ETOPO 2022 for the seabed, OpenStreetMap for
  coastlines, water, roads and names, Overture for buildings, ESA WorldCover for land cover (the
  table and reasons are in section 3.4).
- **Players get it** as a small base layer in the app, plus region and local packs fetched from our
  own GitHub release assets (the star catalogue's and the instrument packs' way), never from OSM.

## 1. What exists today, and what went wrong

### 1.1 The pieces (facts from the code, 2026-11-08)

- **Regions are rectangles.** `scripts/fetch-osm-region.mjs` asks Overpass for one bounding box and
  writes HOSMREG2 (`data/maps/regions/silverdale.bin` 1.96 MB, `seattle-center.bin` 490 KB):
  roads, buildings and water in f32 metres on a flat tangent plane around the box centre, using the
  contract constants `111320.0 * cos(origin_lat)` and `110540.0` (a 0.6% north scale, uniform and
  documented). Sea polygons are assembled from coastline ways and **clipped to the exact box**.
  `scripts/fetch-region-dem.mjs` adds a HOSDEM1 elevation grid (~13 m) from AWS Terrain Tiles for
  the box plus 2 km.
- **Regions load by distance.** `src/engine/region_meshes.rs` builds a region when the camera is
  within 40 km of its origin (`BUILD_RANGE_M`) and draws it within 60 km (`DRAW_RANGE_M`), on a
  512 x 512 elevation grid sampled from the drawn-ground oracle; one RenderObject per material class.
- **Water is an override.** `crates/humanity-terrain/src/water_carve.rs` rasterizes each region's
  water into a 1024 x 1024 mask; the sea bed is pressed to `SEA_CARVE_M` (5 m) below sea level, lakes
  1.5 m below their sheet; a shore taper (`SHORE_TAPER_WATER_M` 90 m, `SHORE_TAPER_LAND_M` 60 m,
  0.7 m deep at the waterline) shapes the edge; inside the DEM's box the ground IS the DEM, blended
  back to the coarse ground over `DEM_EDGE_BLEND_M` (400 m), with DEM bathymetry floored at
  `DEM_FLOOR_M` (-2 m) "because underwater depth is the carve's job".
- **The terrain is a quadtree over an icosahedron.** `crates/humanity-terrain/src/planet_chunks.rs`:
  20 root faces, `PatchId { face, depth, path: u64 }` (2 bits per level), children by normalized
  midpoints (`patch_corners`, `child_corners`), 16 segments per patch edge, depth 13 (~54 m
  triangles) or 20 (~0.42 m) with the tile tier. Heights come from a 7200 x 3600 lat/lon grid
  (0.05 degree, ~5.5 km, `earth_heightmap.bin`) or, at depth 9 and deeper, from optional 15 x 15
  degree ETOPO 2022 tiles at 15 arc-seconds (~460 m, `terrain_tiles.rs`, keyed by lat/lon, not by
  patch), plus procedural fine octaves. Every consumer reaches the ground through
  `drawn_elevation_at_depth` (the one formula since BUG-156).

### 1.2 The waterfront story

- **v0.1148 (2026-08-16):** Silverdale stood up in 3D. Operator: "the water isn't right ... the south
  middle side is missing from Dyes Inlet. Lakes are missing too." Cause: the sea draws only where
  the ground dips below sea level, and 460 m cells average a 1.5 km inlet up onto dry land.
- **v0.1149:** the vector water carve (flight-simulator style), a constant 5 m bed.
- **v0.1153 (2026-08-17):** Operator: "the water edge looks like a sheer cliff instead of tapering
  into the water. Our height map is all wrong." Cause: the coarse ground stood as a bluff right up to
  the polygon edge, where the carve dropped it to -5 m. Fix: the distance-field shore taper and the
  13 m DEM inside the box.
- **BUG-156 (2026-10-05):** trees and the player stood up to ~40 m above the drawn ground beside
  the waterfront, because one consumer had its own copy of the formula without the carve and the
  DEM. Fix: one formula for every consumer.
- **Still open:** the carve ends at the box edge mid-inlet, so the sea floor steps up there
  (maps-ladder.md, "the region-boundary water seam").

### 1.3 The root causes, which a whole Earth would multiply

1. **Three height sources meet at seams** (the 5.5 km grid, 460 m tiles, a 13 m box), each with
   its own blend, and none of them agrees with the coastline polygons by construction.
2. **Water is pressed into heights that never knew about it,** to a constant depth, with a taper
   tuned by hand for Puget Sound.
3. **Boxes.** Sea polygons closed along a box edge, a DEM with a 400 m edge blend, and 40/60 km
   distance gates: every box has an edge, and the whole Earth would be millions of edges.
4. **Two grids, two streamers:** heights stream by 15-degree lat/lon tiles, the mesh by quadtree
   patches, regions by distance from a centre.

## 2. The design in one picture

```
             offline, on the operator's PC (E:)                        in the game
 planet.osm.pbf ─┐                                       ┌──────────────────────────────────────┐
 Overture bldgs ─┼─► builder ─► tiles keyed by PatchId ─►│ patch P needs tile = P.ancestor(L)   │
 GEDTM30 (land) ─┤   (Rust)     layers kept separate:    │ heights: lattice = patch vertices    │
 ETOPO (seabed) ─┤              osm | buildings |        │ shore band, road beds, building pads │
 WorldCover ─────┘              land_h | sea_h | cover   │   inside drawn_elevation_at_depth    │
                          │                              │ meshes built in memory, batched      │
                          ▼                              └──────────────────────────────────────┘
        base (in app) + region packs (depth 3) + local packs (depth 6) on GitHub release assets
```

## 3. Tiles keyed to the terrain's own quadtree

### 3.1 The key and the levels

A map tile's id is a `PatchId`. Data is stored at four levels, each a factor of 8 in size from the
last, and each serves three patch depths (a patch of depth `d` reads level
`clamp(3 * floor(d / 3), 3, 12)`; depths below 3 read several level-3 tiles, depths above 12 read
level 12):

| Level | Tile edge | Tiles on the sphere | Mean tile area | Serves patch depths | Height lattice (64 per edge) |
|---|---|---|---|---|---|
| L3 | 882 km | 1,280 | 398,500 km² | 0 to 5 | 13.8 km |
| L6 | 110 km | 81,920 | 6,226 km² | 6 to 8 | 1.72 km |
| L9 | 13.8 km | 5,242,880 | 97 km² | 9 to 11 | 215 m |
| L12 | 1.72 km | 335,544,320 (only where data exists) | 1.52 km² | 12 to 20 | 26.9 m |

(Edges from `patch_edge_arc_m`: 6,371 km x 1.1071487 rad / 2^d.) The tile for a patch is
`PatchId { face, depth: L, path: path & ((1 << 2L) - 1) }`, a mask. The tile containing a lat/lon
is found by the same firmest-child descent `drawn_leaf_containing` already does. The levels, their
lattices, tolerances and contents are data (`data/maps/levels.ron`), not code (infinite-of-x).

### 3.2 Exact edges for free

Patch corners are made by normalizing the midpoint of two corners, and the normalized midpoint of a
great-circle arc lies on that arc, so **every patch edge, at every depth, is a great-circle arc**.
Clipping a road or a polygon to a tile is then an exact intersection of great circles, done in f64
at build time (or as straight lines in the root face's gnomonic plane, where great circles are
straight). Two neighbouring tiles compute the same crossing point bit for bit, so roads and
coastlines meet across tile borders with no seam.

### 3.3 Coordinates: OSM's own integers, f64 at run time

Points are stored as OSM stores them, latitude and longitude times 10^7 in 32-bit integers (the OSM
wiki's [Node](https://wiki.openstreetmap.org/wiki/Node) page puts the worst longitude error at about
±5.6 mm at the equator), delta-coded and varint-packed per feature. That is
lossless against the source and retires HOSMREG2's flat plane and its two constants. At run time the
chain follows CLAUDE.md's planet-scale rule: integer degrees to f64 lat/lon, `latlon_to_dir_f64`,
drawn radius in f64, minus the tile's f64 anchor, and only that small offset narrowed to f32 (a
tile is at most 1.7 km across at L12, so the f32 step is under a millimetre). Never f32 latitudes:
at 180 degrees an f32 step is about 1.7 m.

### 3.4 Layers, each its own database, one source each

Every tile holds up to seven layers. Each carries its own source, snapshot date and licence tag, and
**no layer is ever edited using another layer's data in the files** (findings 1 and 4 of the
finding: that keeps a pack a Collective Database, and the joining happens at run time as a Produced
Work).

| Layer | Source (the one recommended) | Why this one | Licence | Runner-up |
|---|---|---|---|---|
| `osm_ways`: roads, rail, paths, bridges, tunnels | OpenStreetMap | complete, named, classed; daily updates; no permissive equivalent exists | ODbL 1.0 | Overture transportation (ODbL, OSM plus TomTom) |
| `osm_water`: sea and land polygons assembled for the whole planet, lakes, rivers, coastline type tags | OpenStreetMap | the only open coastline at mapping precision; one coastline at every zoom | ODbL 1.0 | Natural Earth 1:10m (public domain), far zooms only |
| `names`: places, streets, water bodies, in many languages | OpenStreetMap | the names sit on the very features drawn | ODbL 1.0 | Wikidata (CC0) |
| `buildings`: footprints, height, levels, roof tags | Overture Maps buildings theme | OSM plus Microsoft, Google and Esri footprints in one theme (how it resolves overlaps and which heights it carries: read its docs before rung 3) | ODbL 1.0 (the theme) | OSM alone, Microsoft's footprints (CDLA Permissive 2.0) merged by us where OSM has none |
| `land_h`: land heights | GEDTM30 v1.2 (OpenGeoHub, 2026) | the only global 30 m bare-earth model found that any use may carry; bare earth because we draw our own trees and buildings | CC BY 4.0 (credit Copernicus too, until the finding's open question 1 is settled) | Copernicus GLO-30 plus our own removal of forest and buildings |
| `sea_h`: seabed | ETOPO 2022 (NOAA) | one bare-earth surface for land and sea, CC0, already holds NOAA's fine US coastal models, already our base | CC0 1.0 | GEBCO 2026 (public domain, newer offshore) |
| `cover`: land cover classes | ESA WorldCover 2021 v200 | one consistent global year at 10 m | CC BY 4.0, its own credit line | OSM land-use polygons (ODbL, patchy) |

Ruled out (finding): FABDEM is non-commercial; Copernicus GLO-30 is a surface model (canopy and
roofs) with conditions that bind every fork; NASADEM stops at 60° N; the AWS Terrain Tiles carry two
"All rights reserved" national sources; GRIP4's CC0 label sits on data built partly from OSM.

### 3.5 Margins, and clip edges that are not coastlines

Each tile carries its features clipped to the tile plus a margin (the widest shore band, road
corridor or building pad, about 150 m at L12), so a modifier near a border is computed identically
in both tiles. Every edge a clip created is flagged; the shore distance field (section 5) measures
distance to **real** shoreline segments only. This flag is what removes the region-boundary water
seam: a tile border running through an inlet is invisible to the water.

### 3.6 One set of streaming decisions

- The terrain's restricted descent already decides which patches to draw and waits for children to
  be resident before splitting. A patch now also waits for its map tile and height tile (or draws
  from the coarser level it has), exactly as it waits for a child.
- A tile is resident while any resident patch needs it (a reference count beside the patch LRU), and
  it leaves with them; map geometry lives in the patch arena and counts against the same byte cap.
- When the selection needs a tile from a pack that is not installed, that same request queues the
  download (section 9.3). There is no other streamer: the 40/60 km region gate and the 15-degree
  height tiles are retired.

## 4. Heights from one source

### 4.1 One pyramid, one source per domain

The builder resamples GEDTM30 onto the land lattices of every level (downsampled for L3 to L9, as
is at L12) and ETOPO 2022 onto the sea lattices. **Land always comes from GEDTM30 and sea always from
ETOPO, at every zoom**, so no hand-over between two land sources ever happens as the camera
descends. Each lattice stores residuals against its parent level's interpolation (small numbers that
pack tightly), quantized to 0.1 m (GEDTM30's own error is metres).

### 4.2 The lattice is the patch's vertices

**One change to the mesher makes this exact.** Today a patch's 153 vertices are normalized
barycentric blends of its three corners (`planet_chunks.rs:1772-1777`), which are not nested across
depths: the vertex a quarter of the way along an edge, `normalize(3a + b)`, is not the corner of the
grandchild patch there, `normalize(a + normalize(a + b))`. Rung 2 builds each patch's 16-segment
lattice instead by four rounds of the same normalized-midpoint subdivision the tree itself uses, so
a patch's vertices are exactly the corners of its depth+4 descendants. The midpoint is commutative,
so every path to a point gives the same bits, and every coarse vertex is exactly a fine vertex.

Then a level-L tile's 64-segment lattice is exactly the set of vertices of the patches it serves:
a depth-L patch reads every 4th sample, depth L+1 every 2nd, depth L+2 every sample, with no
interpolation, and the next level takes over at depth L+3. Patches deeper than 14 interpolate on the
L12 lattice and add the existing procedural fine octaves (`DetailNoise`), masked off wherever a
modifier (shore, road bed, building pad) owns the ground. Nested vertices also make the LOD hand-over
a pure refinement: a parent's vertex heights reappear unchanged in its children, which is what
vertex morphing between depths needs later.

### 4.3 What retires

`earth_heightmap.bin` (5.5 km), `earth_ocean_mask.bin`, the 15-degree tile tier
(`terrain_tiles.rs` and `examples/build_earth_tiles.rs`), HOSDEM1 and `fetch-region-dem.mjs`. The
base layer's heights (section 9.1) replace the shipped grid at a finer 3.4 km spacing.

## 5. Coastlines right everywhere

### 5.1 Who owns each point

The OSM coastline (with lakes and river areas) decides land from water. On land the ground is
`land_h`; on the water side it is `sea_h`. The two never fight, because each is only ever read on
its own side, and the blend between them is one function of distance to the real shoreline. The
water surface likewise draws only where the water layer says water, never merely where the ground
dips below sea level (today's ocean shell draws wherever it does), so Dutch polders behind their
dikes, the Dead Sea shore and the Caspian depression stay dry land.

### 5.2 The shore band (run time, per tile, inside the one formula)

On tile load, a worker builds a signed distance field to the tile's true shoreline segments
(positive on land) at one eighth of the finest lattice spacing (about 3.4 m at L12). Then, for a
point at signed distance `s`:

- **Land side.** The ground is `land_h`, which at 30 m already carries the real slope down to the
  water. Within the first two lattice cells it is floored at the shoreline height (a datum above
  mean sea level, one number per coast type in `levels.ron`, 1 m to start; the tide line OSM's
  coastline follows is to be read in rung 2), so low land never floods and a noisy coastal DEM pixel
  never digs a moat.
- **Water side.** A nearshore profile falls from the shoreline height as `h = datum - A * x^(2/3)`
  (the equilibrium beach profile of coastal engineering, `x` metres offshore), with `A` chosen per
  shore type: sand from OSM `natural=beach` and WorldCover's bare class, rock and cliff from
  `natural=cliff` and `natural=bare_rock`, mud from `wetland=tidalflat`. Over about one bathymetry
  cell (460 m) it blends into `sea_h`. A **wet guarantee** holds every point more than 15 m offshore
  at least 0.3 m below sea level, so no harbour is ever dry.
- **Cliffs stay cliffs.** Where `land_h` is already 60 m up within two cells of the water (Dover,
  Big Sur), the face is steep because the data is, and the water meets its foot at the profile's
  depth. Steepness comes from the data, never from a mismatch between sources.

### 5.3 Lakes and rivers

A lake's surface is the median of `land_h` along its own shoreline, computed on load (deterministic,
the same in every tile that holds the lake); its bed follows the same profile from its shore. River
areas take a surface that only falls downstream (a running minimum along the flow line); line
rivers carve a channel of their tagged width. Lake depths beyond the profile need a lake bathymetry
source, a later rung.

### 5.4 Why this cannot become BUG-156 again

The shore band, the road beds and the building pads are applied inside `drawn_elevation_at_depth`,
the one formula every consumer calls (patch mesher, walk clamp, trees, grass, the water shell's depth
bake, the region grids). There is nothing to forget to call.

### 5.5 The rig gate (built first, seen red first)

A new probe, `shore_probe`, samples the drawn ground through the same oracle the player stands on
(an IPC request returning `ground_radius_m - sea radius` along a transect, 2.5 m apart) and judges:

- **wet:** every sample more than 15 m on the water side of the coastline is at least 0.3 m below
  sea level;
- **dry** (added in rung 1, after Norderney passed `wet` and `shallow` with its island drowned):
  every sample more than 15 m on the land side is at least 0.3 m above sea level. Not for a polder
  or a depression that is dry below sea level; there the question is whether water is drawn;
- **no false wall:** within 60 m of the waterline, no 10 m stretch rises more than the unmodified
  land layer's own rise over the same stretch plus 2 m (the probe reads both, the drawn ground and
  the raw source: today the region DEM, later `land_h`);
- **kept cliff** (cliff vantages only): the transect reaches at least 60 m within 150 m inland;
- **shallow** (shallow vantages only): the slope within 300 m either side of the waterline stays
  under 3 degrees, and no step between samples exceeds 0.3 m;
- **no seam** (seam vantage): no step over 0.5 m anywhere along a transect laid along the water
  across a tile border or the old box edge.

Each vantage first checks its transect's two ends against the coastline data (land end on land,
water end in water) and fails as misconfigured otherwise, so a wrong coordinate can never pass. The
four vantages, for `tests/visual/vantages.json` (coordinates approximate and camera fields
indicative; the rung fixes both from the data and the rig's current vantage schema):

```json
[
  {"id": "coast-silverdale-waterfront",
   "desc": "On foot at noon on the Dyes Inlet shore in Silverdale, looking across the waterline. The waterfront the operator reported twice: dry in v0.1148, a sheer wall in v0.1149.",
   "camera": {"body": "earth", "lat": 47.6445, "lon": -122.6950, "altitude_km": 0.3, "look_offset_deg": 80, "time": 12},
   "settle_s": 14,
   "shore_probe": {"land": [47.6450, -122.6950], "water": [47.6400, -122.6950], "rules": ["wet", "no_false_wall"]},
   "expect": "Water in the inlet right up to the shore; the ground walks down into it at a gentle slope; no wall, no dry flat where water should be; trees and houses stand on the ground."},
  {"id": "coast-silverdale-old-edge",
   "desc": "300 m above Dyes Inlet where the old Silverdale rectangle's southern edge (latitude 47.590) crossed open water, looking along the water across that line.",
   "camera": {"body": "earth", "lat": 47.5960, "lon": -122.6650, "altitude_km": 0.3, "look_offset_deg": 60, "time": 12},
   "settle_s": 14,
   "shore_probe": {"along_water": [[47.5930, -122.6650], [47.5870, -122.6650]], "rules": ["no_seam"]},
   "expect": "One continuous sheet of water with an even bed; no step, no shelf, no change of colour at the line."},
  {"id": "coast-dover-cliffs",
   "desc": "The White Cliffs east of Dover from 150 m over the Channel, 1 km offshore: a steep coast elsewhere, on the global sources only.",
   "camera": {"body": "earth", "lat": 51.1270, "lon": 1.3600, "altitude_km": 0.15, "look_offset_deg": 75, "time": 12},
   "settle_s": 20,
   "shore_probe": {"land": [51.1380, 1.3600], "water": [51.1300, 1.3600], "rules": ["wet", "kept_cliff"]},
   "expect": "A near-vertical pale face about 100 m tall rising from the water; the sea at its foot; no ramp, no beach plain in front of it."},
  {"id": "coast-norderney-beach",
   "desc": "Norderney's North Sea beach from 150 m: a shallow coast elsewhere, wide sand and a gently shelving sea, on the global sources only.",
   "camera": {"body": "earth", "lat": 53.7250, "lon": 7.1800, "altitude_km": 0.15, "look_offset_deg": 75, "time": 12},
   "settle_s": 20,
   "shore_probe": {"land": [53.7100, 7.1800], "water": [53.7250, 7.1800], "rules": ["wet", "shallow"]},
   "expect": "A broad, nearly flat beach running into shallow water with no step at the waterline; water everywhere seaward."}
]
```

**As built in rung 1 (2026-11-09; `tests/visual/vantages.json` is the source of truth, the block
above is the plan as written).** Four things changed against the data:
- **Cameras.** The camera park only tilts toward north, so a camera north of its coast looks
  inland. The Silverdale waterfront camera sits over the inlet (47.6405, 60 m up), the old-edge
  camera south of the line (47.5875), Norderney's over the island (53.7155); Dover's was right.
- **The seam transect** at longitude -122.665 lay on the inlet's west shore at the box edge (the
  region's sea begins at -122.66488 there). It moved to -122.658, the middle of the 1.1 km of
  water the edge crosses; OSM's coastline puts the water's south shore 145 m past its south end.
- **Where the coast is.** The game's coastline outside a region is the 0.05-degree ocean mask
  (5.5 km cells), which cannot place a coast to the metre, so a vantage on it must declare the
  real crossing with its source (`shore_probe.coast`), or it fails as misconfigured. Dover's and
  Norderney's were read from OSM's `natural=coastline` through the Overpass API on 2026-11-09
  (51.13502 and 53.72176 on their meridians; the plan's ends straddle both). Where a region's OSM
  polygons cover the ends, the game's own reading places the coast and a declaration must agree
  with it to 40 m.
- **A caution for Norderney:** OSM's coastline there is the high-water line, and the dunes behind
  the beach may be steeper than 3 degrees within the 300 m `shallow` window; check the landward
  half against GEDTM30 before rung 4 expects it green.

**Red evidence to record before any fix** (the house rule): `coast-silverdale-old-edge` should fail
`no_seam` on today's build (the carve ends at the box edge); Dover and Norderney have no region, so
their coasts come from the 460 m tiles or the 5.5 km grid and should fail `kept_cliff` and `shallow`
(or `wet`, where a coarse cell puts the water end on land); `coast-silverdale-waterfront` likely
passes today (the v0.1153 taper), so its red is taken by switching the taper off for one run.

## 6. Draping and flattening

### 6.1 Road beds

On tile load each road gets a bed profile computed from `land_h` only (never stored in the files,
see section 3.4): the centreline heights smoothed along the road with a grade limit per class
(`levels.ron`: motorways about 6%, residential about 15%), and a cross-section of flat carriageway at
the road's tagged or class width, a shoulder, and cut or fill slopes back to natural ground at 1 in
2. The terrain inside the corridor takes the bed's height, so the road surface lies exactly on the
ground: the +0.18 m and +0.12 m lift hacks and their shimmer go away. A road is pinned to natural
ground where it crosses the tile margin, so the profile is identical in both tiles. Bridges
(`bridge=*`, `layer>0`) leave the ground alone and get a deck mesh; tunnels (`tunnel=*`) leave it
alone and hide the road (the under-surface view is maps-zoom-to-anything increment 7).

### 6.2 Building plinths

Each building's floor height is the median of `land_h` over its footprint. Inside the footprint plus
a 1.5 m apron the ground is set to that height; outside it returns to natural ground at 1 in 2;
where natural ground is lower the wall continues down to it as a visible plinth, where higher the
ground is cut back. Today's fixed 2.5 m buried skirt is no longer needed because base and ground now
agree exactly.

### 6.3 When a road is geometry and when it is paint

A modifier changes the mesh only when it is wider than about one vertex spacing of the patch being
built (a motorway from about depth 15, 13 m spacing; a footpath from about depth 18). Coarser
patches show roads, water edges and land cover through a small per-patch material mask rasterized
from the same tile when the patch builds, so a town reads correctly from 10 km up with no extra
triangles. The skirts and the existing LOD crossfade hide the hand-over.

## 7. Generalization per level, continents to doorsteps

| Level | Roads | Water | Buildings | Names | Land cover | Tolerance |
|---|---|---|---|---|---|---|
| L3 | motorway, trunk | coastline, lakes over 10 km², major rivers | none | countries, cities over about 100,000 | 1 km classes | 860 m |
| L6 | down to secondary | lakes over 0.1 km², rivers | built-up areas as outlines | towns | 250 m | 108 m |
| L9 | all public roads (no service roads or paths) | all water areas, streams | blocks merged from footprints, mean height | villages, neighbourhoods, main streets | 30 m | 13 m |
| L12 | everything: service roads, paths, steps, barriers | everything | every footprint with height, levels, roof tags | every name, house numbers | 20 m | none (OSM's own precision) |

Simplification preserves topology: roads simplify only between junctions, polygons that share an
edge share the simplified edge, and the coastline and river network never cross themselves. The
contents of each level (by OSM key and value), minimum sizes and tolerances are rows in
`levels.ron`, not code.

## 8. The offline build pipeline

### 8.1 Inputs (on E:, the operator's data drive)

| Input | Size | Licence | Status |
|---|---|---|---|
| OSM planet PBF, weekly | 89 GB (planet-261102) | ODbL | download, operator's permission |
| Overture buildings theme | not yet measured (finding, open question 6) | ODbL | download, operator's permission |
| GEDTM30 30 m | one global COG; its 120 m file is 19.3 GB, the 30 m is larger (estimate a few hundred GB); read by range requests where possible | CC BY 4.0 | download, operator's permission |
| ETOPO 2022 15 arc-second | already on disk for the tile tier | CC0 | have |
| ESA WorldCover 2021 v200 | not yet measured | CC BY 4.0 | download, operator's permission |

### 8.2 The builder

Our own Rust tool, beside `examples/build_earth_tiles.rs` today (CLAUDE.md: open source only, build
what does not exist). Planetiler (Apache 2.0) is the strongest open planet tiler and the yardstick
for speed, but it writes Web Mercator vector tiles, the wrong grid for a sphere of icosahedral
patches; its published methods (single pass, node locations in a memory-mapped array, sort by tile)
are what we borrow. Stages:

1. **Node index:** a memory-mapped array of node locations by id (OSM's
   [statistics](https://planet.openstreetmap.org/statistics/data_stats.html) counted 10,988,345,996
   nodes and 1,230,777,320 ways on 2026-11-09, so about 100 GB at 8 bytes per id) on an NVMe drive.
2. **Features:** ways and relations to roads, water, names; multipolygons assembled; **the coastline
   assembled once for the whole planet** into land and sea polygons (the land-left rule
   `fetch-osm-region.mjs` already proves), never closed along a box.
3. **Buildings** from Overture's GeoParquet (Rust `parquet` crate, Apache 2.0).
4. **Clip to L12 tiles** with the great-circle clipper (section 3.2), margins and clip-edge flags.
5. **Generalize upward** to L9, L6, L3 by the `levels.ron` rules.
6. **Heights and cover** resampled onto the lattices; residual coding.
7. **Pack** (section 9.2) with a manifest per pack and the licence texts inside.
8. **Verify:** byte-identical rebuild (built twice, compared, the house rule of both fetchers),
   decode round trip, and geography gates in the style of HOSMREG2's (a known open-water point in
   the sea, a known town point on land, Dyes Inlet wet, Rainier Square Tower 259 m tall).

It runs below normal priority, leaves the operator's core free (the share guard), and waits for
memory through the build slot like any compile.

### 8.3 Time and space on the operator's PC (estimates, measured in rung 3)

- **Vectors:** Planetiler builds a 69 GB planet in 2h38m on 16 cores and 32 GB. The operator gives
  the build about 5 of his 6 cores (a third of the cores, partly made up by faster cores and
  hyper-threads) on a file 1.3 times bigger: roughly 3 to 4 times as long, so **an overnight run,
  about 8 to 12 hours**.
- **Heights and cover:** reading a few hundred GB of rasters and writing lattices is disk-bound;
  estimate a few hours, a second night at most.
- **Disk:** about 1 TB of inputs and work files plus about 250 GB of output (one of the three 2 TB
  drives).

### 8.4 Incremental updates

- **OSM:** apply the daily diffs (the day feed runs at 00:05 UTC) to the build's node and way
  index, mark the L12 tiles each changed element touches, rebuild those tiles and their L9, L6 and
  L3 ancestors, and republish only packs whose content hash changed.
- **Overture:** monthly releases with stable GERS ids, diffed by id.
- **GEDTM30, ETOPO, WorldCover:** static; rebuilt only when a new version is adopted.
- **Cadence** is the operator's call (section 14); recommended monthly for changed packs.

### 8.5 Output size per tier (estimates)

| Tier | What | Typical | Worst | Whole planet |
|---|---|---|---|---|
| Base (in the app) | L3 vectors; heights L3 plus L6 at 32 per edge (3.4 km, finer than today's 5.5 km) | about 43 MB heights + 10 to 30 MB vectors | | about 55 to 75 MB (today's Earth grids are 55 MB) |
| Region pack (one per depth-3 tile with land, about 500) | L6 and L9 vectors; heights to 215 m; cover to 30 m | 30 to 130 MB | about 250 MB | about 25 GB |
| Local pack (one per depth-6 tile with land, about 26,000) | L12 vectors; heights 26.9 m; cover 20 m | 10 to 30 MB where people live | about 200 MB in the densest cities; a pack over 256 MB splits into its four children | about 200 to 250 GB |

Reasoning: L12 land heights at 26.9 m are 2.4 x 10^11 samples (475 GB as raw 16-bit, about 120 GB
at 4 bits per residual), about 5 MB per local pack; OSM's own compact PBF averages 1.84 KB per km²
for Washington and 3.17 for Europe, so a local pack's vectors run about 10 to 25 MB where people
live; WorldCover at 20 m is about 1 MB per local pack.

## 9. How players get the data

### 9.1 The base layer, in the app

The whole planet at continent-to-country detail: L3 vectors and the L3 and L6 heights (section 8.5),
replacing `earth_heightmap.bin` and `earth_ocean_mask.bin`. Every install sees every coast, country
and city with no download. It contains generalized OSM data, so the release archive must carry the
ODbL text and offer (rung 1 makes it).

### 9.2 Packs on our release assets

- **Region packs** (L6 and L9 for one depth-3 tile) for flying and the map at altitude; **local
  packs** (L12 for one depth-6 tile) for the ground.
- Hosted as GitHub release assets under non-`v*` tags (`assets-maps-r1`, `assets-maps-l1` ...),
  the star catalogue's and the instrument packs' way: no CI runs, GitHub's CDN serves them, and
  `--latest=false` keeps `releases/latest` on the game. A release holds at most 1,000 files under
  2 GiB each, with no limit on total size or bandwidth (finding 26), so about 27 asset releases.
  **Uploading is the top-level session's step** (CLAUDE.md standing permissions, 2026-11-07);
  producing is an agent's (docs/ai/release.md, instrument
  packs).
- A data catalog, `data/maps/packs.ron`, lists each pack: its `PatchId`, file, bytes, SHA-256, build
  id, and its sources with their licences and snapshot dates, the `data/music/packs.ron` pattern.
  The app refuses a pack whose hash does not match. A list of mirrors in the catalog lets the packs
  move host without a code change.
- **Each pack carries its own notices** (finding 1, ODbL 4.2): a manifest naming every source and
  licence, the ODbL text (or its URI), the CC BY 4.0 URI, WorldCover's credit line, the Copernicus
  notice and liability sentence, and the format specification's location (so the pack itself is
  the 4.6 offer).
- **Never** OSM's tile or API servers, at run time or for packs (finding 8).

### 9.3 When a pack is fetched

The streaming request of section 3.6 is the trigger. **Privacy:** a fetch tells the download host
which area the player asked for (a 110 km local tile, or an 880 km region). The game world is not the
player's home, but the Maps page's real-life GPS use is. So the first time, the app asks, in plain
words by key: download this area (its size), whether to fetch as you go or ask each time, and that
the download server can see which area was asked for. Settings > Maps lists installed packs with
sizes, Update and Delete, a Download area button (a whole region or country at once, which reveals
only that it was wanted), and the map-detail setting below.

### 9.4 The hardware notice

Settings > Maps > Map detail: **Base** (in the app, no downloads), **Regional** (region packs only),
**Full** (local packs too, the default where the machine allows). Each says what it needs (disk per
pack and in total for what is installed, memory and graphics memory for full city detail) and what
this machine has (free disk, graphics memory from the adapter), and if Full is out of reach it says
so and names Regional as what works instead (the CLAUDE.md hardware rule). Simplified mode, per the
two-modes rule: Regional is the simplified mode for weak machines and slow links.

### 9.5 Packs are data, not a cache

Downloaded packs are installed data like the star tiers and instrument packs: listed in Settings,
verified by hash on load, replaced only by a newer build from the catalog. **Nothing derived is
kept across runs**: meshes, distance fields and decoded tiles are built in memory each session. The
in-memory reuse (a decoded tile kept while patches need it) is version-tagged by the pack's hash and
obeys the Dev page's "turn off all reuse" switch (`HUMANITY_NO_REUSE=1`): with it on, every patch
build decodes its tile afresh.

### 9.6 Offline, and the website

Installed packs work offline; with no pack, the base layer draws. The website's Maps page fetches
CARTO tiles at run time today (attribution audit item 12; CARTO's terms not read). Its mirror rung
reads the same base layer and packs (from our own host, not GitHub, if GitHub's download links do
not allow cross-origin reads), so the web and the app draw the same map.

### 9.7 The whole planet, for a server or anyone who wants it

Asked by the operator on 2026-11-09 ("What happens if someone wants the whole Earth? Like for their
dedicated MMO server?"). Three ways, all reaching the same files:

- **Download all of Earth**, a button in Settings > Maps beside Download area: every region and local
  pack (about 25 GB plus 200 to 250 GB, section 8.5), queued, resumable, checked against the
  catalog's SHA-256 file by file, with the size and the free disk space shown before it starts. A
  headless server gets the same action through the in-app ops registry (the GUI-first rule), so
  its admin page offers it too.
- **Build it yourself** with our builder from the OSM planet file and the other sources (section 7),
  an overnight job on a desktop: the ODbL offer made real, and a server operator who wants fresher
  data than our monthly packs gets it.
- **Serve it onward:** a server holding every pack can be listed in its players' catalogs as a
  mirror (9.2), so its players fetch from it instead of GitHub; the packs are the same files with
  the same hashes, so a mirror can never hand out different data unnoticed.

A server needs the heights and the footprints for collision and placement; it never draws, so a
later "server set" could skip land cover and names. One format until a measurement says the saving
matters.

## 10. Budgets

| Budget | Limit | How it holds |
|---|---|---|
| Disk, in the app | base layer at most 80 MB (today's Earth grids are 55 MB of a 586 MB data folder) | L6 heights at 32 per edge; vectors generalized at 860 m |
| Disk, downloads | user's choice; Settings shows the total; no automatic deletion | packs are installed data (9.5) |
| Memory, decoded map data | at most 256 MB | only tiles referenced by resident patches; L12 near the camera, coarser beyond |
| Graphics memory, map geometry | at most 15% of the patch arena's byte cap (1,400 MiB), about 200 MB, densest city | geometry in the shared arena, counted by real bytes, evicted with its patches |
| Frame time, main thread | at most 0.5 ms with a city in view; about 0 where there is no map data | decoding, distance fields and meshing on the patch-build workers within the existing per-frame build budget |
| Frame time, GPU | at most 2 ms at 2560 x 1440 for buildings and roads at the densest ground vantage, on the operator's RTX 4070 | one batched draw per material per L12 tile through the indirect patch draw |
| Network, flying | an airliner at 250 m/s crosses a local tile in about 7 minutes: about 10 to 30 MB each, under 1 Mbit/s | local packs ahead of the camera's track; regional packs at altitude |
| Network, a jump somewhere new | one region pack and one to four local packs: about 50 to 250 MB; the base layer draws at once, local detail in seconds to a minute on a 25 to 100 Mbit/s link | local pack first when on the ground |
| Network footprint in play | none: packs are static files; no map data is sent between players | gameplay sync unchanged |

Frame and memory figures are design limits for the rungs to measure against with
`game_frame_costs` at the new vantages, not measurements.

## 11. Credits wherever the data is drawn

- **In view:** a short line in a corner of the 3D view and the Maps page, built from the sources of
  the tiles actually drawn (the OSM guideline's "optimally" case): "© OpenStreetMap contributors"
  always when OSM layers are on screen, "Overture Maps Foundation" when Overture buildings are, and a
  "Map credits" link. It is drawn by its own always-on layer through `src/gui/layers.rs`, not inside
  the HUD, so the construction editor and the showroom cannot hide it (closing the open audit gap).
- **In detail:** Settings > Credits lists every source with its exact notice from
  `data/credits.ron`: OSM (with openstreetmap.org/copyright), Overture's own line, GEDTM30, the
  Copernicus notice and liability sentence, ETOPO, WorldCover's line. The OSM guideline for games
  accepts a credits page with a short notice in view.
- **In every file:** the pack manifest (9.2), and LICENSES.md copied into the release archive.
- `data/credits.ron` gains rows for GEDTM30, Copernicus, ETOPO (already owed, audit item 14),
  WorldCover and Overture, each with its `shown_in` surfaces; the credits unit test keeps them honest.

## 12. Migrating Silverdale and Seattle Center

No backwards compatibility before v1.0, so the regions are rebuilt, not converted.

- The depth-6 tiles that contain them (about 27 km apart, so one or two local packs) become the
  first local packs, built from Geofabrik's Washington extract (348 MB) in rung 2, shipped in
  `data/maps/packs/` until the downloader exists, as the region files are today. The ship hangs over
  Silverdale, so its home packs ship with the app even after the downloader.
- **Deleted:** `data/maps/regions/*.bin` and `*.dem.bin`, `scripts/fetch-osm-region.mjs`,
  `scripts/fetch-region-dem.mjs`, the HOSMREG2 and HOSDEM1 parsers, the region list and the 40/60 km
  gates in `src/engine/region_meshes.rs`, the per-region 1024 x 1024 masks.
- **Kept and moved to tile scope:** the ear clipper and extrusion mesher (`osm_region.rs`), the
  coastline assembly rules and every self-check of both fetchers (ported into the builder's tests),
  the water-carve distance-field code (now per tile, true shorelines only), the BUILT vegetation
  channel (per tile), the Maps Planet view's drawing.
- **Vantages:** `silverdale-osm-ground`, `silverdale-osm-2km`, `silverdale-osm-night`,
  `seattle-osm-5km`, `silverdale-home-marker` and the BUG-156 ground probe keep their coordinates;
  their expectations change only where the region edge was visible.
- **Words:** `data/credits.ron`'s OSM `used_for` says the whole Earth instead of two towns;
  LICENSES.md's OSM section stops naming the `.dem.bin` files (audit item 10) and describes packs.

## 13. Rungs, in build order

1. **The yardstick and the credit** (rung 1, below).
2. **Home packs:** the builder over Geofabrik's Washington extract plus GEDTM30 and WorldCover range
   reads for those tiles and ETOPO from disk; the mesher's nested vertex lattice (section 4.2); tile
   residency riding the patches; the shore band, road beds and building pads in the one formula;
   Silverdale and Seattle Center migrated (section 12).
   Goes green: `coast-silverdale-waterfront`, `coast-silverdale-old-edge`.
3. **The planet build:** the whole planet, Overture buildings, generalization L3 to L12, measured
   time and sizes written back into section 8, incremental updates from daily diffs. The
   `just` recipe is developer tooling; its in-app button is logged as CLI debt in
   `docs/design/in-app-ops.md`.
4. **The downloader:** `data/maps/packs.ron`, Settings > Maps, the privacy question, the hardware
   notice, fetch from release assets (uploaded by the top-level session under its standing
   permissions, section 9.2; an agent produces the packs). Goes green: `coast-dover-cliffs`,
   `coast-norderney-beach`.
5. **The base layer** replaces `earth_heightmap.bin`, the ocean mask and the 15-degree tile tier.
6. **One map:** the Maps page Planet view draws these tiles as the straight-down view of the same
   camera (maps-zoom-to-anything increment 7); the website mirrors it from the same files.
7. **Detail inserts:** public-domain national lidar (USGS 3DEP, 1 m) as an L15 height level inside
   the United States, NOAA's fine coastal bathymetry, lake depths, the intertidal zone with tides,
   DeltaDTM for low coasts after the finding's open question 1.
8. **Buildings in 3D:** roof shapes and levels from OSM's 3D tags, bridges and tunnels as real
   geometry, the under-surface view. Not boxes (operator, 2026-11-09: "How do we create the 3D buildings that are
   visually interesting instead of simple cubes?"): each footprint becomes a building by published
   methods, built in our own code: roofs from the footprint's straight skeleton (hipped, gabled,
   pyramidal, from `roof:shape` or the regional default), walls split into storeys and bays by a
   facade grammar (the published shape-grammar method of Müller et al., 2006, not any tool), each bay
   filled from a kit of modular pieces (windows, doors, porches, trim, chimneys, garages) instanced
   across the city, a regional style file per area and era (Pacific Northwest wood siding and gabled
   roofs, European brick and render) chosen by location and building type, seeded variation so
   neighbours differ, interior mapping behind windows for lit rooms at night, and a hand-made model
   in place of the procedural one for a landmark. Styles and kits are data (infinite-of-x). Open
   renderers that already do this (OSM2World, Streets GL) are compared, licences quoted, in their own
   finding before rung 8 starts (the compare-alternatives rule). **One generator with the ship's
   homes** (operator, same day: "this kind of plays into our procedural ship generation stuff ...
   That way homes are studios but, actually have interiors with walls, doors, windows, and
   furniture"): the footprint-to-building path is shared. A map building and a ship home both go
   outline, then floor plan (rooms from a programme by home type: bedroom, bathroom, kitchen,
   living), then `InteriorWall`s with doors and windows, then furniture placed by room purpose
   (`room-purpose-architecture.md`) from the model kit (the SimReady props). A map building needs
   its outside everywhere and its inside only where someone can enter or a lit window shows it; a
   ship home needs its inside first. Today a cabin is a box with "no interior walls"
   (`ship-homes-increment-6-plan.md`); this generator is what gives it rooms.

### Rung 1 in detail: the yardstick and the credit

Small, measurable, no large downloads, and it makes every later rung provable.

- **The gate:** the `shore_probe` IPC request and judge (section 5.5) in `scripts/probe-sweep.js`
  beside the BUG-156 ground probe; the four vantages added to `tests/visual/vantages.json`; the red
  results recorded in each vantage's comment.
- **The tile key, as a pure module** (`crates/humanity-terrain/src/map_tiles.rs`, relay-safe, std and
  glam only): level for a depth, the ancestor tile by mask, the tile containing a lat/lon, the
  nested lattice's sample directions (section 4.2), the great-circle clipper for polylines and rings
  with clip-edge flags, the 10^7-degree coordinate codec. Unit tests: ancestor and containment agree
  with `drawn_leaf_containing` on a grid of points; lattice directions equal the corners
  `patch_corners` gives the depth L+6 descendants, bit for bit (rung 2 adds the same test against the
  mesher once it uses the nested lattice); a road crossing a tile border clips to the same point from
  both sides; a ring clipped into four tiles and reassembled keeps its area to a relative 1e-9; codec
  round trip within 6 mm.
- **The credit:** the map credit drawn by its own layer, not the HUD; LICENSES.md (and the ODbL
  text) copied into the release archive by `build-desktop.yml`; LICENSES.md's `.dem.bin` sentence
  corrected; `data/credits.ron` rows for ETOPO (owed since the audit).
- **Proof:** the vantages red where section 5.5 predicts; the module's tests seen red against a
  deliberate mutation each; `just verify`, the relay check, the doc-link check.

### Rung 1 outcome (2026-11-09)

Built: `src/engine/shore_probe.rs` (the verb), `scripts/lib/shore-probe-check.js` (the judge, in
`just rig-tests`), the four `coast-*` vantages, `crates/humanity-terrain/src/map_tiles.rs` (the tile
key, each test seen red against a mutation), `src/gui/map_credit.rs` (the credit's own layer, shown
only while a region is within draw range), LICENSES.md's Heights section and the `noaa_etopo` and
`aws_terrain_tiles` rows. The readings, measured on two grounds because of BUG-287 (a release draws
Earth to patch depth 13, the operator's checkout to depth 20); every number is in the vantage's
`_red` note:

| Vantage | Shipped | Why |
|---|---|---|
| `coast-silverdale-waterfront` | green at depths 13 and 20, as predicted | the taper and the DEM hold: the steepest 10 m inland rises 0.70 m (13) or 1.48 m (20) |
| (taper off) | red at depth 20, green at 13 | the v0.1153 wall, 8.39 m in 10 m at the waterline; a 54 m triangle cannot draw it |
| (taper and DEM off) | red at depth 13 | the v0.1149 state: the coarse grid at 40 m to the shore, a 45 m face, and wet fails |
| `coast-silverdale-old-edge` | red, as predicted | the bed stops at the box edge and the ground climbs to +25 m, dry; at depth 20 a 16 m step in 2.5 m |
| `coast-dover-cliffs` | red (wet, kept_cliff), as predicted | a dry ramp from 28 m to 16 m: no cliff (25 m at most within 150 m) and no sea |
| `coast-norderney-beach` | **passed the plan's rules**; red on `dry` | the island is a sea-level flat, 836 m of it under water; coarse data smears a coast flat, so `wet` and `shallow` pass it |

What the next rungs should know: the gate measures the walk oracle, so its depth follows the
camera and the tile folder (the readout records `walk_depth` and `tile_tier`); a vantage outside a
region must declare its coast (the game's coastline there is the 5.5 km ocean mask); the old box
edge's seam is a 30 m wall at depth 20, worth a capture in rung 2's before and after.

## 14. Questions only the operator can answer

1. **Land heights: GEDTM30 or Copernicus GLO-30?** GEDTM30 is bare earth (the ground under the
   trees) under CC BY 4.0 but labelled "for testing" by its makers; Copernicus is the
   better-established survey but a surface model (the top of the forest and the roofs), so we would
   have to strip trees and buildings ourselves. **Recommendation: GEDTM30**, with Copernicus plus our
   own stripping as the fallback if GEDTM30's next release disappoints.
2. **Buildings: Overture's merged set, or OpenStreetMap alone?** Overture puts Microsoft's, Google's
   and Esri's machine-found footprints together with OSM's in one theme, under the licence we
   already meet; it is run by a foundation of large companies (we read a snapshot, nothing at run
   time). **Recommendation: Overture.**
3. **Hosting about 250 GB of packs as GitHub release assets** (about 27 asset releases, uploaded by the top-level session),
   with the catalog able to name a mirror later? **Recommendation: yes.** GitHub states no limit on
   total size or bandwidth; the alternative, our own servers, costs money the project does not have.
4. **Privacy when fetching:** fetch as you go (the host sees each 110 km area) or ask, with a
   "download my whole region" choice that reveals only the 880 km region? **Recommendation: ask the
   first time, default to whole-region downloads for the Maps page's real-life use, fetch as you go
   only inside the game world if the player chooses it.**
5. **Downloads for the build** (each needs your permission): the planet (89 GB), Overture buildings,
   GEDTM30, WorldCover, all onto E:. **Recommendation: yes for rung 2's small set now** (the 348 MB
   Washington extract and range reads of GEDTM30 and WorldCover for the home tiles), **the full set
   when rung 3 starts.**
6. **How often packs update.** **Recommendation: monthly for changed packs**, a full rebuild only
   when the format changes; OSM edits reach players within a month.

### Answers (operator, 2026-11-09)

- 4, privacy: "Sounds good." Ask the first time; whole-region downloads by default on the Maps page.
- 5, downloads: "Let's get Washington state! I'd love to see it in all of its glory." Fetched the
  same day onto `E:\HumanityOS\maps-src\`: Geofabrik's Washington extract (365,725,774 bytes,
  dated 2026-11-09 00:21 UTC, with its MD5 file) and the WorldCover home tile `N45W123`
  (64,998,919 bytes). Measured for the whole state, not yet fetched: WorldCover's eight tiles
  covering it, 470 MB together; GEDTM30's 30 m heights are one global 432,407,644,715-byte file
  (`s3.opengeohub.org`, range requests accepted), so Washington's window is read by range
  requests, an estimated 0.5 GB. On his "Yes" the same day, all of it was fetched: the eight
  WorldCover tiles (470 MB, `maps-src/worldcover/`) and Washington's GEDTM30 window, 45.4 to
  49.1 N and 124.9 to 116.8 W, as 112 of the file's own 2048-pixel tiles, kept byte for byte
  (601 MB, `maps-src/gedtm30-v1.2-washington/`, with `index.json` and the fetch scripts beside it).
  **A trap for the builder:** the file's GDAL metadata says SCALE 0.1, but the stored float32 values
  are already metres: decoded, Mount Rainier reads 4,412.7 and Mount Adams 3,737.2 (summits 4,392 m
  and 3,743 m), Dyes Inlet 0, the open Pacific the no-data value. Applying the scale makes Rainier
  441 m. Rung 2's reader must ignore that tag for this file, with a unit test on those two summits.
- 6, cadence: "Makes sense." Monthly for changed packs.
- 1, 2 and 3: after asking why (GEDTM30 is bare earth under any-use CC BY 4.0, where Copernicus is
  the top of the forest; Overture fills the rural houses OSM lacks; packs are release files cut by
  tile, fetched only where a player goes), "Yes." to all three. GEDTM30 for land heights, Overture
  for buildings, packs as GitHub release assets.

## 15. Risks

1. **GEDTM30 is young** (version 1, "for testing", a 0.9 arc-second grid quirk its makers are
   fixing, a 7 m mean bias). The coastline decides where land ends, so the bias does not move coasts;
   the fallback is section 14's runner-up.
2. **The Copernicus pass-through question** (finding, open question 1) could add a notice to every
   pack; the plan already carries it.
3. **Pack sizes are estimates.** Rung 3 measures; the 256 MB cap splits a pack into its children.
4. **GitHub could object** to a quarter-terabyte of assets despite its documented limits; the mirror
   list in the catalog is the way out.
5. **The base and detail coastlines** are the same source generalized, so the hand-over between L3
   and L6 moves a coast by at most the L3 tolerance (860 m), at altitudes where that is about a
   pixel; the crossfade covers it.
6. **Overture's licence page lags Microsoft's relicence** (finding 10); harmless for an ODbL theme,
   but credits must follow the theme, not the old Microsoft line.
