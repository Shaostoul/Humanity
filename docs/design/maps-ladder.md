# The Maps Ladder

> Operator vision (2026-08-16): "Ideally it supports planets (generate real
> roads and buildings using OpenStreetMap... view that in the game AND on the
> maps page like a GPS map), solar system (needs improvement), galaxy
> (populate the map with the real stars database we have so it looks real),
> and eventually multiple galaxies... I'd love to some day visualize the
> cosmic web."
>
> One page ("Maps", `cosmos.rs`), one view ladder, every rung backed by REAL
> data. This doc is the build order; PRIORITIES.md ranks when.

## The ladder (surface scales, small to large)

```
Planet (GPS)  ->  Solar System  ->  Galaxy  ->  Cosmic Web
   OSM              ephemeris        HYG/ATHYG/Gaia   SDSS/2MASS (someday)
```

## Rung 1: Galaxy view on the real catalog (SHIPPED v0.1145)

The Galaxy view (was "Galactic") previously drew `data/stars-nearby.json`,
a curated ~50-star list. Now it draws `data/stars-map.bin`: the full HYG
catalog (~119k stars) with true 3D galactic positions in light-years,
generated from `data/stars.csv` by `scripts/build-stars-map-bin.js`
(equatorial parsecs -> galactic light-years, brightest-first ordering,
proper names for the ~360 named stars). Renderer rules: magnitude cutoff
scales with zoom (zoom out = only bright stars, zoom in = the deep field),
viewport culling, named stars labeled when zoomed. Sol stays the anchor at
origin. The same file is the seed for rung 4's third dimension.

## Rung 2: Solar System improvements

"Mostly have, needs improvement." Current: 3D orbit view + planet details.
Candidate improvements, in rough order (operator to rank when this rung is
fenced): true-scale/log-scale toggle, live planet positions from the same
ephemeris the sky uses, moons beyond the majors, asteroid-belt density from
the real MPC distribution, transfer-window overlay (ties into gameplay),
click-through to the in-game body (the Inventory/Maps cross-links exist).

## Rung 3: Planet GPS view (OpenStreetMap)

The big one: real roads and buildings, on the Maps page like a GPS map AND
in the 3D world.

- **Data**: OpenStreetMap extracts (ODbL license, attribution required).
  Do NOT hit the live OSM tile servers from the app (their policy forbids
  heavy app use); pipeline instead: `scripts/` fetcher pulls a bounding-box
  extract (Overpass or Geofabrik .pbf) -> compact vector format per region
  (roads as polylines with class, buildings as footprint polygons with
  height when tagged) -> served like the star-catalog tiers (release-asset
  download, in-app install per region).
- **Maps page 2D**: a "Planet" view rung: pan/zoom slippy map rendering the
  vector data (egui painter: polylines + filled polygons; label ladder like
  the galaxy view). GPS-style: given the operator's lat/lon (manual pin
  first; device GPS someday), center-on-me.
- **In-game 3D**: the same vector data drives world generation on Earth
  terrain: roads as flattened splines textured onto the terrain patches,
  buildings extruded from footprints (height tag, else floors x 3 m, else
  a class default). This rides the existing chunked-LOD Earth: a region's
  vector pack loads with its terrain chunks.
- **First increment when fenced**: one hand-picked region (the operator's
  home area), fetcher + format + 2D Maps view; 3D extrusion is the second
  increment; region browser + downloads third.

## Rung 4: Cosmic web (far future)

Multiple galaxies -> large-scale structure. Real data exists and is small
enough to ship: the 2MASS Redshift Survey (~45k galaxies) or SDSS main
sample subset gives the filaments-and-voids web in a few MB. A "Universe"
view above Galaxy: fly-through point cloud, Milky Way highlighted, the
same brightest-first/zoom-cutoff pattern as the galaxy view. No new
technique needed, just the data pipeline and a 3D camera. Park until the
galaxy view has had its second polish pass.

## Naming (settled 2026-08-16)

Page = **Maps** everywhere (button, heading, web route /maps). Views =
Solar System | Galaxy | Night Sky (+ Planet when rung 3 lands). The
`GuiPage::Cosmos` duplicate variant is deleted; "Cosmos" survives only as
the module filename and the legacy config string.
