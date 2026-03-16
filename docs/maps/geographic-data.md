---
title: Geographic Data — Open Source Map Stack
category: design
status: active
updated: 2026-03-13
---

# Geographic Data Architecture

HumanityOS uses a fully open-source, self-hosted map stack.
No dependency on Google Maps, Mapbox, or any commercial platform.
The same data serves both real-world applications (Sponsor-A-Can, location-based services)
and the game's Earth twin simulation.

---

## Data Sources (all free, open license)

### Vector / Road / Building Data
**OpenStreetMap (OSM)**
- License: ODbL (Open Database License) — free to use, share, attribute
- Contains: roads, buildings, POIs, boundaries, waterways, parks, land use
- Download: https://download.geofabrik.de/north-america/us/washington.html
  - Washington state .osm.pbf (~200MB compressed)
  - Or extract just Kitsap County bounding box using osmium-tool
- Format: .pbf (binary) → convert to GeoJSON with osmtogeojson or ogr2ogr
- Kitsap County bbox: approx -122.95, 47.35, -122.30, 47.80

### Elevation / Height Map Data
**USGS 3DEP (3D Elevation Program)**
- License: Public Domain
- Coverage: Full Kitsap County at 1-meter lidar resolution
- Download: https://apps.nationalmap.gov/downloader/
  - Product: 1/3 arc-second DEM (10m) for quick loads
  - Product: 1-meter IfSAR/lidar for full detail
- Format: GeoTIFF → process to PNG heightmap tiles

**NASA SRTM**
- License: Public Domain
- Coverage: Global, 30-meter resolution
- Good for: planet-scale rendering, space/orbit views
- Download: https://dwtkns.com/srtm30m/ (tile picker)

### Coastlines / Natural Features
Already have: `game/data/coastlines.json` (global, low-res)
OSM provides high-res coastlines for Puget Sound / Kitsap specifically.

---

## Kitsap County Pilot

**Priority area:** Silverdale Urban Growth Boundary → Illahee State Park, Bremerton
- WA-303 / Bucklin Hill Road corridor
- Silverdale Way, Kitsap Mall area
- North along WA-303 to Illahee State Park boundary

**Why Kitsap first:**
- Sponsor-A-Can.org operates here
- Michael (Shaostoul) knows the area in detail
- Manageable scale (~400 sq km county)
- USGS has 1-meter lidar coverage

**OSM extract for Kitsap only:**
```bash
# Install osmium-tool
apt install osmium-tool

# Extract Kitsap County from WA state file
osmium extract \
  --bbox=-122.95,47.35,-122.30,47.80 \
  washington-latest.osm.pbf \
  -o kitsap.osm.pbf

# Convert to GeoJSON
osmtogeojson kitsap.osm.pbf > kitsap-full.geojson

# Split by type for efficient loading
# Roads: highway=*, name=*
# Buildings: building=*
# Parks: leisure=park, natural=*
# POIs: amenity=*, shop=*, tourism=*
```

---

## Tile Architecture (Performance)

The core problem: rendering all of Kitsap's roads (~40,000 way segments)
and buildings (~120,000 polygons) simultaneously would destroy performance.

### Solution: Zoom-Level Tiles (like real map apps)

```
Zoom 6  (county overview):  major highways only
Zoom 10 (city view):        + arterials, parks, water
Zoom 13 (neighborhood):     + local streets, land use
Zoom 15 (street level):     + buildings, footpaths, POIs
Zoom 17 (walking):          + building details, addresses
```

### Tile Grid
Each zoom level divides the map into 256×256px tiles using the Web Mercator
tile system (same as Google Maps / OpenStreetMap). Each tile is a PNG image
OR a GeoJSON vector tile stored at a known path:

```
/game/tiles/{zoom}/{x}/{y}.png    (raster — fast, large storage)
/game/tiles/{zoom}/{x}/{y}.json   (vector — slower render, smaller storage, interactive)
```

**Recommended:** Pre-rasterized PNG tiles for Kitsap at zoom 6–14 (fast loading),
vector GeoJSON tiles at zoom 15+ (needed for interactivity — click a building, see its data).

### Storage Estimate for Kitsap
```
Zoom  6–10:  ~50 tiles   × ~20KB = ~1MB
Zoom 11–13:  ~500 tiles  × ~50KB = ~25MB
Zoom 14–15:  ~4000 tiles × ~30KB = ~120MB
Total: ~150MB for full Kitsap raster coverage
```
This fits easily on the VPS (currently has ~50GB free).

### Vector Tile Storage (for game interactivity)
Store buildings + POIs in SQLite with R-tree spatial index:
```sql
CREATE VIRTUAL TABLE locations_rtree USING rtree(
  id, min_lat, max_lat, min_lng, max_lng
);
-- Query: "give me all buildings in this viewport"
SELECT * FROM locations_rtree
WHERE min_lat >= ? AND max_lat <= ?
  AND min_lng >= ? AND max_lng <= ?;
```
This returns only buildings in the current camera view — handles 100k+ buildings
without performance issues.

---

## Service Area Painting (for Sponsor-A-Can)

A "service area" is a named GeoJSON polygon stored server-side.

### Data Format
```json
{
  "id": "sponsor-a-can-silverdale",
  "name": "Sponsor-A-Can — Silverdale Service Area",
  "description": "...",
  "scope": "region",
  "color": "#22aa66",
  "opacity": 0.25,
  "geometry": {
    "type": "Polygon",
    "coordinates": [[[lng, lat], [lng, lat], ...]]
  }
}
```

### UI: Polygon Draw Mode
On `maps.html`, a "Draw Area" button enters polygon-draw mode:
1. Click to place vertices on the map
2. Double-click or click first vertex to close the polygon
3. Name the area, choose color, assign to a project (Sponsor-A-Can, etc.)
4. Save → POST to `/api/areas` (new endpoint needed)
5. Polygon renders as a colored overlay on the map

### Sponsor-A-Can specific layers
- **Service area boundary** — the overall coverage polygon
- **Can locations** — point markers at each sponsored bin (GPS coordinates)
- **Routes** — polylines showing pickup routes
- **Coverage gaps** — heat map of underserved areas

---

## Dual Use: Real Life + Game

The same geographic data serves two purposes:

| Feature | Real-world use | Game use |
|---|---|---|
| OSM roads | Sponsor-A-Can routing, navigation | Player movement, pathfinding |
| Buildings | Can placement, addresses | Enterable structures, ownership |
| Terrain (DEM) | Topographic display | Elevation gameplay, flooding sim |
| Service areas | Cooperative coverage zones | Territory / faction zones |
| POIs | Local businesses, parks | Quests, resources, NPCs |

**The Earth twin is literal** — Kitsap County in the game IS Kitsap County in reality.
A trash can at 47.6543° N, 122.6941° W in Sponsor-A-Can is the same point
on the game map. Players can verify real-world actions in the game world.

---

## Road Data Performance in Game

OSM roads in Kitsap = ~40,000 segments. Options:

### Option A: Pre-rasterized road tiles (recommended for v1)
Render roads to PNG tiles server-side (using Python + Pillow or Node + sharp).
Client just requests tile images — no road geometry processing at runtime.
Pro: very fast, works on any device. Con: roads aren't clickable/interactive.

### Option B: Simplified GeoJSON per tile (v2)
Each tile at zoom 15 contains only the roads within that 256×256px region.
Client renders road geometry via Canvas API. Roads are clickable.
Pro: interactive, updatable without re-rendering tiles.
Con: needs spatial indexing + tile server logic.

### Option C: Vector tiles (MVT format, v3)
Mapbox Vector Tile format — the industry standard for this. Libraries: MapLibre GL.
Pro: full zoom interpolation, very efficient. Con: adds a JS library dependency.

**Recommended path:** Start with Option A (raster tiles, fast), add Option B for
the local Silverdale pilot area, graduate to Option C when performance demands it.

---

## Implementation Plan

### Phase 1 — Foundation (Kitsap pilot)
1. Download Kitsap OSM .pbf from Geofabrik
2. Extract to GeoJSON (roads, buildings, POIs separately)
3. Add a server-side tile endpoint: `/api/tiles/{zoom}/{x}/{y}`
4. Update `maps.html` to use tile-based rendering instead of current simple canvas
5. Show Kitsap County at zoom 10–14

### Phase 2 — Sponsor-A-Can integration
1. Add polygon draw tool to maps.html
2. POST `/api/areas` endpoint
3. Render service area overlay
4. Add can-location markers

### Phase 3 — Game integration
1. maps.html and game/js/map.js share the same tile server
2. In-game Earth twin uses real Kitsap OSM data
3. Buildings become enterable game objects
4. Sponsor-A-Can cans appear at real GPS coordinates in game

### Phase 4 — Height map
1. Download USGS 10m DEM for Kitsap
2. Process to PNG heightmap tiles
3. Render elevation shading on map
4. 3D terrain in game camera modes

---

## Quick Start: Get Kitsap Data Today

```bash
# On the VPS (has python3, curl):
cd /opt/Humanity

# 1. Download Kitsap area from OSM via Overpass API (no large download needed)
curl "https://overpass-api.de/api/map?bbox=-122.95,47.35,-122.30,47.80" \
  -o data/kitsap.osm

# 2. Or use Geofabrik for the full WA state extract
curl -O https://download.geofabrik.de/north-america/us/washington-latest.osm.pbf
# Then extract Kitsap with osmium (apt install osmium-tool)
```
