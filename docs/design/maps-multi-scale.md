# Multi-Scale Map Redesign

Design doc for seamless cosmic-to-street navigation in the maps page.

**Status:** Proposal
**Affects:** `pages/maps.html`, `game/js/map.js`, `game/js/celestial.js`, `game/data/*.json`

---

## 1. Current State

The maps page (`pages/maps.html`) has two independent rendering systems:

- **map.js** — Five discrete modes selected by button click: Surface (Earth equirectangular), Solar System, Stellar Neighborhood, Galaxy, Sky View. Each mode has its own render function, zoom/pan state, and zero transition logic between them. Duplicates planet/star data inline because it runs in a separate IIFE from celestial.js.
- **celestial.js** — Three-level drill-down (sector -> system -> planet) on a separate canvas in the game page's celestial tab. Loads data from `game/data/solar-system.json` and `game/data/stars-nearby.json`. Breadcrumb navigation, click-to-drill.

Key problems:
- No seamless zoom between scales. Mode switches are jarring button clicks.
- Duplicated planet/star data between map.js (inline `MAP_PLANETS`, `MAP_STARS`) and celestial.js (JSON files).
- Earth surface is coastline-only vector rendering. No tile-based map for zoom below continent level.
- Galaxy view is a static procedural spiral with no real structure.
- No elevation data, no terrain, no street-level detail.

---

## 2. Target Experience

A single continuous zoom from galaxy to street level, driven by scroll wheel or pinch gesture. The user sees one canvas that smoothly transitions between rendering modes as zoom level changes.

### Scale Hierarchy

| Level | Scale Range | What renders | Transition trigger |
|-------|------------|--------------|-------------------|
| Galaxy | 10^21 m (100 kly) | Milky Way spiral, arm structure, Sol marker | Default widest zoom |
| Stellar | 10^17 m (10 ly) | Nearby star field, spectral colors, labels | Zoom past galaxy threshold |
| System | 10^13 m (100 AU) | Sun + all planets/dwarf planets with orbits | Click star or zoom past stellar threshold |
| Planet | 10^7 m (10,000 km) | Globe rendering with atmosphere, surface color | Click planet or zoom past system threshold |
| Globe | 10^6 m (1000 km) | Earth (or Mars etc.) as 3D sphere with coastlines | Zoom into planet |
| Continental | 10^5 m (100 km) | Equirectangular or Mercator with OSM tiles | Zoom past globe threshold |
| Regional | 10^4 m (10 km) | OSM tiles, cities, roads visible | Continuous tile zoom |
| Street | 10^1 m (10 m) | OSM max zoom, individual buildings | Continuous tile zoom |

### Zoom Factor Mapping

Use a single `scaleMeters` value representing meters-per-pixel at screen center. The scroll wheel adjusts this exponentially:

```
scaleMeters = baseScale * Math.pow(zoomFactor, -zoomLevel)
```

Where `zoomFactor = 1.5` per wheel notch. This gives ~60 notches from galaxy to street level:
- Galaxy: scaleMeters ~ 10^18
- Street: scaleMeters ~ 1

Each rendering mode activates when `scaleMeters` crosses its threshold. During transition, both the outgoing and incoming renderers draw simultaneously with cross-fade alpha over ~3 zoom steps.

---

## 3. Data Sources

### 3.1 Galaxy Structure

**Source:** Procedural generation based on Milky Way parameters (already implemented).

Enhancement: Use the logarithmic spiral model with 4 major arms (Perseus, Sagittarius-Carina, Scutum-Centaurus, Norma-Outer). Parameters from Reid et al. 2014.

- Pitch angle: ~12 degrees
- Sol position: 8.15 kpc from center, between Perseus and Sagittarius arms
- No external data file needed; generate from parameters

**Existing asset:** `game/data/milky-way.json` (30 Milky Way band points for sky view) — repurpose for galaxy arm density.

### 3.2 Star Catalogs

**Primary: HYG Database v3.x**
- URL: https://github.com/astronexus/HYG-Database
- License: CC BY-SA 2.5
- Content: ~120,000 stars with positions (x,y,z in parsecs), magnitudes, spectral types, proper names
- Format: CSV, convertible to compact JSON
- Fields needed: `id, x, y, z, mag, absmag, spectral, proper_name, constellation`

**Currently loaded:**
- `game/data/stars-nearby.json` — Small catalog used by celestial.js, ~20 nearby stars in compact array format `[id, x, y, z, spectral, mag, absmag, proper_name]`
- `game/data/stars-catalog.json` — ~300 brightest stars with RA/Dec for sky view rendering

**Strategy:**
- Ship a pre-processed HYG subset: all stars within 50 pc (~5000 stars) in the compact array format. ~200 KB gzipped.
- For the full 120K catalog, load on demand when sector zoom > threshold. Split into spatial octree chunks, fetch as needed.
- LOD: At galaxy scale show only the ~50 brightest. At sector scale show all within viewport frustum.

### 3.3 Solar System — Planets and Dwarf Planets

**Primary: NASA JPL Horizons**
- URL: https://ssd.jpl.nasa.gov/horizons/
- License: Public domain (US government work)
- Provides: Keplerian orbital elements, physical parameters, ephemerides
- API: https://ssd-api.jpl.nasa.gov/doc/horizons.html

**Alternative: NASA Planetary Fact Sheets**
- URL: https://nssdc.gsfc.nasa.gov/planetary/factsheet/
- Simple HTML tables with radius, mass, orbit, atmosphere for each body

**Currently loaded:** `game/data/solar-system.json` — Sun + 8 planets + Pluto + moons. Contains orbital elements (semiMajor, eccentricity, period, meanLongitude), physical data (radius, mass, gravity, atmosphere, temperature, resources), and rendering hints (color, symbol).

**What to add:**
- Dwarf planets: Ceres, Eris, Haumea, Makemake (add to solar-system.json)
- Asteroid belt: Procedural scatter between 2.1-3.3 AU, use IAU minor planet center data for major asteroids (https://minorplanetcenter.net/data)
- Kuiper belt: Procedural scatter 30-55 AU
- More moons: Currently only major moons. Add at least all moons > 100 km radius
- Comet orbits: Optional. Halley, Hale-Bopp for visual interest

### 3.4 Earth Map Tiles (Street-Level Zoom)

**Primary: OpenStreetMap (OSM)**
- Tile URL: `https://tile.openstreetmap.org/{z}/{x}/{y}.png`
- License: ODbL (attribution required: "(c) OpenStreetMap contributors")
- Zoom levels: 0-19 (z0 = whole world in 1 tile, z19 = building-level)
- Tile size: 256x256 px
- Usage policy: Max 2 requests/second for light use. For heavier use, self-host tiles or use a provider.

**Alternative tile providers (no API key for light use):**
- Stamen Toner (good for dark theme): `https://tiles.stadiamaps.com/tiles/stamen_toner/{z}/{x}/{y}.png` (requires free API key now)
- CartoDB Dark Matter: `https://basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png` (fits dark UI theme, free for light use)
- MapTiler: `https://api.maptiler.com/maps/toner-v2/{z}/{x}/{y}.png?key=KEY` (free tier: 100K tiles/month)

**Recommended:** CartoDB Dark Matter for consistency with the dark theme. No API key needed for reasonable usage.

**Satellite imagery:**
- Mapbox Satellite: Requires API key, free tier 200K tiles/month
- ESRI World Imagery: `https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}` (free for non-commercial)

### 3.5 Elevation Data

**Primary: Mapzen Terrain Tiles (now AWS open data)**
- URL: https://registry.opendata.aws/terrain-tiles/
- Format: Terrarium or Mapzen encoding in PNG tiles
- Tile URL: `https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png`
- License: Various (public domain for SRTM-derived data)
- Resolution: ~30m at z15

**Alternative: OpenTopography**
- URL: https://opentopography.org/
- Higher resolution DEMs available via API
- Requires registration

**Strategy:** Elevation data is a stretch goal. Initially render flat map tiles. Add terrain shading later by sampling elevation tile RGB values and computing hillshade normals.

### 3.6 Mars / Other Planet Surface Data

**Mars:**
- MOLA elevation data: https://astrogeology.usgs.gov/search/map/Mars/GlobalSurveyor/MOLA
- Mars global color mosaic: https://astrogeology.usgs.gov/search/map/Mars/Viking/MDIM21
- Both available as tiled WMS services from USGS Astrogeology

**Moon:**
- LRO WAC mosaic: https://wms.lroc.asu.edu/lroc/

**Strategy:** For non-Earth bodies, render the globe view with procedural surface coloring (already done in celestial.js). Link out to NASA/USGS viewers for detailed surface exploration. Full tile-based rendering for Mars/Moon is a future phase.

---

## 4. Rendering Architecture

### 4.1 Unified Zoom Controller

Replace the current five discrete modes with a single `ZoomController` that manages one continuous state:

```js
const zoomState = {
  scaleMeters: 1e18,       // meters per pixel at center
  centerRA: 0,              // right ascension (galaxy/stellar)
  centerDec: 0,             // declination (galaxy/stellar)
  centerLat: 0,             // latitude (planet surface)
  centerLng: 0,             // longitude (planet surface)
  focusBody: null,           // 'earth', 'mars', etc.
  focusSystem: 'SOL',       // which star system
  animating: false,
  targetScale: null,         // for smooth zoom animation
};
```

The controller determines which renderer(s) to activate:

```js
function getActiveRenderers(scaleMeters) {
  if (scaleMeters > 1e19) return ['galaxy'];
  if (scaleMeters > 1e16) return ['stellar'];
  if (scaleMeters > 1e11) return ['system'];
  if (scaleMeters > 5e6)  return ['globe'];
  if (scaleMeters > 1e1)  return ['tiles'];   // OSM
  return ['tiles'];
}
```

During transitions (scale within 0.5 orders of magnitude of a threshold), both adjacent renderers draw with blended alpha.

### 4.2 Renderer Modules

Each renderer is a standalone function: `render(ctx, width, height, zoomState)`.

**GalaxyRenderer**
- Draws procedural spiral arms using logarithmic spiral math
- Particle system: ~2000 points distributed along arms with gaussian spread
- Central bulge as radial gradient
- Sol marker with pulsing highlight
- Performance: Pre-compute arm particle positions once, transform on render

**StellarRenderer**
- Plots stars from HYG database subset
- Orthographic projection from 3D galactic coordinates to 2D
- Star size = function of absolute magnitude and distance
- Spectral color mapping (already implemented)
- Click-to-select shows info panel
- LOD: Filter to visible magnitude based on zoom level

**SystemRenderer**
- Logarithmic radial scale for orbit visibility (already implemented)
- Animated planet positions from Keplerian elements
- Asteroid belt as scattered dots between Mars and Jupiter orbits
- Kuiper belt ring beyond Neptune
- Dwarf planets (Pluto, Ceres, Eris) with proper orbits
- Moon orbits visible when zoomed into a planet's vicinity

**GlobeRenderer**
- 3D sphere projection on 2D canvas using standard spherical math
- For Earth: draw coastlines from `coastlines.json`, color land/ocean
- For other planets: procedural surface with correct base color
- Atmosphere glow ring (already implemented)
- Icosphere grid overlay (already implemented)
- As zoom increases, transition to flat map projection

**TileRenderer**
- Slippy map using OSM tile coordinates: `z, x, y`
- Tile zoom level = `Math.floor(Math.log2(earthCircumference / (scaleMeters * tileSize)))`
- Load tiles as Image objects, cache in Map<string, HTMLImageElement>
- Draw visible tiles to canvas at correct positions
- Tile cache: LRU with max ~200 tiles in memory
- Loading states: draw placeholder (dark rectangle) while tile loads
- Attribution overlay: "(c) OpenStreetMap contributors"

### 4.3 Coordinate Systems

| Scale | Coordinate System | Units |
|-------|-------------------|-------|
| Galaxy | Galactic cartesian (x, y, z) | kiloparsecs |
| Stellar | Galactic cartesian | parsecs |
| System | Heliocentric ecliptic | AU |
| Globe | Spherical (lat, lng) on body surface | degrees |
| Tiles | Web Mercator (EPSG:3857) | tile x, y, z |

Conversion functions needed:
- `galacticToScreen(x, y, z, zoom)` — for galaxy and stellar views
- `heliocentricToScreen(au_x, au_y, zoom)` — for system view
- `latLngToScreen(lat, lng, zoom)` — for globe view (equirectangular or orthographic)
- `latLngToTile(lat, lng, z)` — for tile view (Web Mercator)
- `screenToLatLng(sx, sy, zoom)` — inverse for click detection

### 4.4 Transition Logic

When `scaleMeters` crosses a threshold:

1. **Galaxy -> Stellar:** Galaxy fades out, stars scale up from dots to labeled points. Camera centers on Sol's galactic position.
2. **Stellar -> System:** Stars fade except Sol. Sol expands, planet orbits appear. Smooth interpolation of Sol from star-dot to sun-with-glow.
3. **System -> Globe:** Other planets shrink off-screen. Focus planet grows to fill view. Orbit lines fade. Surface features begin to appear.
4. **Globe -> Tiles:** Sphere flattens to equirectangular projection. Coastline vectors fade as raster tiles load and fade in. Grid lines persist briefly then fade.

Each transition uses `requestAnimationFrame` with eased interpolation over ~500ms. The transition is interruptible — if the user keeps scrolling, skip to next scale.

### 4.5 Click Navigation Shortcuts

In addition to scroll zoom, clicking objects provides instant navigation:
- Click star in stellar view -> animate zoom to that star's system
- Click planet in system view -> animate zoom to that planet's globe
- Click "Zoom to Surface" on Earth globe -> animate to tile view centered on home location
- Breadcrumb bar at top allows jumping back to any scale level

---

## 5. Performance Considerations

### 5.1 Level of Detail (LOD)

| Renderer | LOD Strategy |
|----------|-------------|
| Galaxy | Fixed ~2000 particles. No LOD needed. |
| Stellar | Filter stars by viewport bounds + magnitude cutoff. At low zoom show only mag < 2, at high zoom show all within 50 pc. Max ~5000 stars rendered. |
| System | Always render all planets (< 20 objects). Moons only when zoomed close to parent. Asteroid belt particles capped at 500. |
| Globe | Coastline simplification via Douglas-Peucker at low zoom. Full detail coastlines only when globe fills > 50% of screen. |
| Tiles | Standard tile LOD via z-level. Only load tiles visible in viewport + 1 tile margin. |

### 5.2 Tile Loading

```js
class TileCache {
  constructor(maxSize = 200) {
    this.cache = new Map();   // key -> {img, lastUsed}
    this.loading = new Set(); // keys currently being fetched
    this.maxSize = maxSize;
  }

  getTile(z, x, y) {
    const key = `${z}/${x}/${y}`;
    if (this.cache.has(key)) {
      this.cache.get(key).lastUsed = Date.now();
      return this.cache.get(key).img;
    }
    if (!this.loading.has(key)) {
      this.loading.add(key);
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.onload = () => {
        this.evictIfNeeded();
        this.cache.set(key, { img, lastUsed: Date.now() });
        this.loading.delete(key);
        requestRender(); // trigger re-draw
      };
      img.src = `https://basemaps.cartocdn.com/dark_all/${z}/${x}/${y}.png`;
    }
    return null; // not yet loaded
  }

  evictIfNeeded() {
    if (this.cache.size < this.maxSize) return;
    // Remove least recently used
    let oldest = Infinity, oldestKey = null;
    for (const [k, v] of this.cache) {
      if (v.lastUsed < oldest) { oldest = v.lastUsed; oldestKey = k; }
    }
    if (oldestKey) this.cache.delete(oldestKey);
  }
}
```

### 5.3 Render Throttling

- Galaxy/Stellar/System: Render on demand (zoom/pan change). No continuous animation unless planet orbits are animated.
- Globe: Render on demand.
- Tiles: Render on demand + on tile load callback.
- Sky View: Keep as separate mode (time-dependent), animate at 30 fps when active.
- Use `requestAnimationFrame` for all rendering, skip frames if previous frame not complete.
- Debounce wheel events to max 1 render per 16ms.

### 5.4 Memory Budget

- Star catalog (5000 stars, 8 fields each): ~400 KB JSON, ~100 KB gzipped
- Solar system data: ~50 KB (current file is fine)
- Coastline data: Already loaded, ~200 KB
- Tile cache: 200 tiles * ~20 KB avg = ~4 MB
- Total JS heap for map data: < 10 MB target

### 5.5 Canvas vs WebGL

Stick with Canvas 2D for now. Reasons:
- Current codebase is 100% Canvas 2D — no WebGL anywhere
- All renderers are simple enough (circles, lines, images) for Canvas 2D
- Galaxy particle rendering is the most intensive at ~2000 circles — well within Canvas 2D budget
- Tile rendering is just `drawImage` calls
- WebGL would be warranted if we add: 3D globe rotation with textures, real-time terrain shading, or > 50K particles. Save for Phase 2.

---

## 6. Integration with Existing Code

### 6.1 What Changes

**`pages/maps.html`:**
- Remove the five mode buttons (Surface, Solar System, Stellar, Galaxy). Replace with a single scale indicator bar.
- Keep Sky View as a separate toggle (it's a different projection — alt/az dome, not spatial zoom).
- Add breadcrumb navigation bar showing current scale context.
- Add OSM attribution text in bottom corner.

**`game/js/map.js`:**
- Refactor into modular renderers. The current `renderSurface`, `renderSystem`, `renderSector`, `renderGalaxy` functions become the basis for the new renderer modules.
- Remove inline `MAP_PLANETS` and `MAP_STARS` arrays. Load from shared JSON files.
- Replace `mapView` string state with `zoomState` object.
- Replace `mapSetView()` button handler with `zoomTo(scaleMeters, options)`.
- Keep `renderSkyView` as a standalone mode (toggle, not part of zoom continuum).
- Keep all existing features: GPS location, pins, home marker, weather, icosphere grid, draw mode, search.

**`game/js/celestial.js`:**
- Deprecate in favor of the unified map.js renderers. The celestial tab in the game page can embed the same map component with initial zoom set to stellar level.
- Port the info panel rendering (planet details, star details) into the unified sidebar.

**`game/data/solar-system.json`:**
- Add dwarf planets: Ceres, Eris, Haumea, Makemake with orbital elements from JPL.
- Add more moons (all > 100 km radius).

**New file: `game/data/stars-hyg-50pc.json`:**
- Pre-processed HYG subset: all stars within 50 parsecs.
- Compact array format matching existing convention: `[id, x, y, z, spectral, mag, absmag, proper_name]`.

### 6.2 What Stays the Same

- Single-page architecture with `<canvas>` rendering (no framework)
- Dark theme styling
- Sidebar with context-sensitive info panels
- GPS geolocation integration
- Pin/bookmark system with localStorage
- Icosphere addressing system (`F0.L5.T142`)
- Weather overlay (surface only)
- Touch support (drag, pinch zoom)
- Sky View mode (separate toggle)
- All data loaded via `fetch()` from `/game/data/`

### 6.3 Migration Path

1. **Phase 1 — Unified zoom state:** Replace the five mode buttons with scroll-driven scale transitions. Keep existing render functions, wire them to scale thresholds. No new data.
2. **Phase 2 — Tile renderer:** Add OSM tile loading for Earth surface zoom below continent level. This is the biggest user-facing improvement.
3. **Phase 3 — Enhanced star catalog:** Ship HYG 50pc subset, replace inline star data, improve stellar neighborhood rendering.
4. **Phase 4 — Dwarf planets and belts:** Expand solar-system.json, add asteroid/Kuiper belt rendering.
5. **Phase 5 — Globe renderer:** 3D sphere with rotation for planet view, smooth flatten-to-tiles transition.
6. **Phase 6 — Elevation/terrain:** Add terrain tile loading and hillshade rendering.

---

## 7. UI Wireframe

```
+------------------------------------------------------------------+
| [shell nav bar]                                                   |
+------------------------------------------------------------------+
| Galaxy > Milky Way > Sol > Earth                    [Sky View] [?]|
| |==========|----|----|----|----|----|----|===| zoom: 10 km/px      |
+------------------------------------------------------------------+
|                                                   |               |
|                                                   | Earth         |
|              [Canvas: map tiles or                | Radius: 6371  |
|               space rendering]                    | Gravity: 9.81 |
|                                                   | Temp: 14.9C   |
|                                                   | ...           |
|                                                   |               |
|                                                   | [Pins]        |
|                                                   | Home: Rainier |
|                                                   | Pin 1: ...    |
|                                                   |               |
+------------------------------------------------------------------+
| (c) OpenStreetMap contributors    46.78N 121.74W  | Scale: 10 km  |
+------------------------------------------------------------------+
```

The zoom slider bar at top shows the full scale range (galaxy to street) with the current position highlighted. Dragging the slider jumps to that scale. The breadcrumb updates to show context at the current scale.

---

## 8. Open Questions

1. **Tile provider choice:** CartoDB Dark Matter fits the theme but has usage limits. Self-hosting tiles on the VPS would remove limits but requires ~80 GB storage for a full planet tileset. Start with CartoDB, monitor usage.
2. **Mars tiles:** Should we render Mars at tile level? USGS provides WMS but latency may be high. Could pre-render a low-res Mars tileset (~z0-z6) from MOLA data.
3. **Fantasy mode:** celestial.js has a reality/fantasy toggle. The fantasy layer (game world overlay) needs its own tile source or procedural generation. Defer to game engine design.
4. **Offline support:** The service worker (`shared/sw.js`) could cache visited tiles for offline use. Tile cache strategy TBD.
5. **3D globe interaction:** Should the globe rotate with mouse drag, or stay fixed with pan? Mouse drag rotation feels more natural but complicates the flatten-to-tiles transition.
