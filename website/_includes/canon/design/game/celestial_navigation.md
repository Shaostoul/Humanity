# Celestial Navigation System

> Design specification for the Humanity celestial navigation and star map system.

## Overview

A hierarchical space navigation system that lets players explore the universe from cosmic web down to planetary surfaces. Shared engine powers both **Reality** (real astronomical data) and **Fantasy** (game worlds with fictional/magic-infused celestial bodies).

---

## Hierarchical Navigation

```
Universe (observable)
  → Supercluster (Laniakea)
    → Galaxy Cluster (Virgo / Local Group)
      → Galaxy (Milky Way)
        → Sector (Orion Arm / local neighborhood)
          → Star System (Sol)
            → Planet (Earth)
              → Surface (icosphere coordinates)
```

### Zoom Levels

| Level | View | Content |
|-------|------|---------|
| 1 | **Universe** | Stylized cosmic web filaments, superclusters as nodes |
| 2 | **Supercluster** | Laniakea boundary, galaxy clusters highlighted |
| 3 | **Galaxy** | Milky Way top-down, spiral arms, star density heat map |
| 4 | **Sector** | Local stellar neighborhood (~20 pc), individual stars visible |
| 5 | **Star System** | Orbital view, planets with elliptical orbits |
| 6 | **Planet** | Globe view with icosphere grid overlay |
| 7 | **Surface** | Zoomed terrain (placeholder for future 3D) |

Zoom transitions are continuous — scroll wheel or pinch smoothly interpolates between levels. Breadcrumb navigation shows current location path and allows jumping up.

---

## Icosphere Coordinate System

A universal addressing scheme for any point on any spherical body.

### Base Geometry

- Start with a regular **icosahedron** (20 equilateral triangular faces)
- Each face can be **recursively subdivided**: every triangle splits into 4 sub-triangles
- Vertices are projected onto the unit sphere after each subdivision

### Address Format

**Long form:** `F{face}.L{level}.T{triangle}`
**Compact form:** `F{face}.{level_letter}{path}`

- **F** = face index (0–19)
- **L** / level letter = subdivision depth (A=1, B=2, ... J=10, K=11, ...)
- **T** / path = triangle index within that face at that level

Examples:
- `F13.L5.T2048` — Face 13, level 5, triangle 2048
- `F13.J4235` — Face 13, level 10 (J), triangle 4235
- `F0.A0` — Face 0, level 1, first sub-triangle

### Resolution by Level

| Level | Total Triangles | Area per Triangle (Earth) | Use Case |
|-------|----------------|--------------------------|----------|
| 0 | 20 | ~25.5M km² | Hemisphere-scale |
| 5 | 20,480 | ~24,900 km² | Country-scale |
| 8 | 1,310,720 | ~389 km² | City-scale |
| 10 | 20,971,520 | ~24.3 km² | Neighborhood |
| 12 | 335,544,320 | ~1.52 km² | Block-scale |
| 15 | 21,474,836,480 | ~0.024 m² (24 mm²) | Sub-millimeter |

### Storage

- Face: 5 bits (0–19)
- Level: 5 bits (0–31)
- Triangle path: up to 50 bits (2 bits per subdivision step × 25 levels)
- **Total: 8 bytes** encodes any point on any sphere to sub-meter precision

### Binary Encoding

```
Byte layout (64 bits):
[FFFFF][LLLLL][PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP PP]
 face   level  path (2 bits per level, up to 27 levels in remaining 54 bits)
```

### Universality

Works for any spherical body — planets, stars, moons, asteroids, Dyson spheres, magic orbs. The only parameter that changes is the body's radius (which determines physical area per triangle).

---

## Real Astronomical Data Sources

### HYG Star Database
- **Source**: github.com/astronexus/HYG-Database (`hygdata_v41.csv`)
- **Coverage**: ~120,000 stars with 3D positions
- **Key fields**: StarID, proper name, ra, dec, distance (parsecs), apparent magnitude, absolute magnitude, spectral type, luminosity, color index
- **MVP subset**: Nearest ~200 stars within 20 parsecs

### Solar System (JPL Horizons)
- 8 planets + Pluto + dwarf planets (Ceres, Eris, Haumea, Makemake)
- Major moons: Luna, Phobos, Deimos, Io, Europa, Ganymede, Callisto, Titan, Enceladus, Triton, Charon
- Keplerian orbital elements for position calculation at any date
- Accurate physical parameters (mass, radius, temperature, atmosphere)

### Future Data Sources
- **Exoplanets**: NASA Exoplanet Archive (~5,500 confirmed)
- **Nebulae/Clusters**: NGC/IC catalogs (~13,000 deep-sky objects)
- **Galaxies**: NGC catalog for nearby galaxies
- **Cosmic Web**: Simulated large-scale structure (Illustris/EAGLE data)

---

## Data Models

### Star

```javascript
{
  id: 'SOL',                    // unique ID (HYG_{id} for catalog stars)
  name: 'Sol',                  // display name
  properName: 'Sun',            // common name if any
  position: { x: 0, y: 0, z: 0 }, // parsecs from Sol (galactic coords)
  ra: 0,                        // right ascension (degrees)
  dec: 0,                       // declination (degrees)
  distance: 0,                  // parsecs from Sol
  magnitude: -26.74,            // apparent magnitude
  absMagnitude: 4.85,           // absolute magnitude
  spectralType: 'G2V',          // MK spectral classification
  luminosity: 1.0,              // solar luminosities
  temperature: 5778,            // effective temperature (K)
  mass: 1.0,                    // solar masses
  radius: 1.0,                  // solar radii
  planets: ['mercury', 'venus', 'earth', 'mars', 'jupiter', 'saturn', 'uranus', 'neptune'],
  color: '#FFF5E0',             // rendered color (derived from spectral type)
}
```

### Planet

```javascript
{
  id: 'earth',
  name: 'Earth',
  symbol: '🌍',
  starId: 'SOL',
  type: 'terrestrial',          // terrestrial | gas_giant | ice_giant | dwarf
  radius: 6371,                 // km
  mass: 5.97e24,                // kg
  gravity: 9.81,                // m/s² surface
  dayLength: 24.0,              // hours (sidereal)
  atmosphere: {
    composition: { N2: 78.08, O2: 20.95, Ar: 0.93, CO2: 0.04 },
    pressure: 101.325,          // kPa
    description: 'Nitrogen-oxygen, breathable'
  },
  temperature: { min: -89.2, avg: 14.9, max: 56.7 }, // °C
  orbit: {
    semiMajor: 1.000,           // AU
    eccentricity: 0.0167,
    period: 365.256,            // days
    inclination: 0.0,           // degrees to ecliptic
    longitudeOfPerihelion: 102.9, // degrees
    meanLongitude: 100.5,       // degrees at J2000
  },
  moons: ['luna'],
  rings: false,
  water: true,
  life: true,
  colonized: true,              // game context
  resources: ['iron', 'silicon', 'aluminum', 'calcium', 'sodium', 'magnesium'],
  surfaceFeatures: [],          // named locations / points of interest
  bounties: [],                 // linked bounty IDs
  color: '#4488ff',             // render color
}
```

### Moon

```javascript
{
  id: 'luna',
  name: 'Moon',
  planetId: 'earth',
  radius: 1737.4,               // km
  mass: 7.342e22,               // kg
  gravity: 1.62,                // m/s²
  orbit: {
    semiMajor: 384400,          // km from parent
    period: 27.322,             // days
    eccentricity: 0.0549,
    inclination: 5.145,         // degrees
  },
  atmosphere: null,
  temperature: { min: -173, avg: -23, max: 127 },
  color: '#aaaaaa',
}
```

---

## Navigation UI

### Layout

```
┌──────────────────────────────────────────────────┐
│ 🌌 Celestial Map            [Reality] [Fantasy]   │
├──────────────────────────────────────────────────┤
│                                                    │
│  📍 Universe › Milky Way › Sol                     │
│                                                    │
│  ┌─────────────────────────────────────────────┐  │
│  │                                             │  │
│  │            ☀ Sol                            │  │
│  │     ·                                       │  │
│  │   ☿   ♀   🌍   ♂        ♃    ♄    ⛢   ♆  │  │
│  │                                             │  │
│  │              [orbit ellipses]               │  │
│  │                                             │  │
│  └─────────────────────────────────────────────┘  │
│                                                    │
│  ┌─ Earth ─────────────────────────────────────┐  │
│  │ Type: Terrestrial   Radius: 6,371 km        │  │
│  │ Mass: 5.97×10²⁴ kg  Gravity: 9.81 m/s²     │  │
│  │ Temp: -89°C to 57°C (avg 15°C)             │  │
│  │ Atmosphere: N₂ 78%, O₂ 21%, Ar 0.9%        │  │
│  │ Moons: Luna   Water: Yes   Life: Yes        │  │
│  │ Resources: Fe, Si, Al, Ca, Na...            │  │
│  │ [View Surface] [Bounties (3)] [Wiki]        │  │
│  └──────────────────────────────────────────────┘ │
│                                                    │
│  Nearby Stars: α Centauri (1.34 pc) ·              │
│  Barnard's Star (1.83 pc) · Wolf 359 (2.39 pc)    │
└──────────────────────────────────────────────────┘
```

### Interaction Model

- **Pan**: Click + drag on canvas
- **Zoom**: Scroll wheel / pinch gesture
- **Select**: Click on star/planet
- **Navigate in**: Double-click or click breadcrumb "drill down"
- **Navigate out**: Click breadcrumb ancestor or zoom out past threshold
- **Detail panel**: Appears below canvas when object selected

### Star System View

- Orbital paths drawn as ellipses (Keplerian elements)
- Planets rendered as colored circles
- Size toggle: artistic (visible) vs. to-scale (realistic)
- Optional animation of orbital motion
- Click planet → detail panel

### Sector View (Stellar Neighborhood)

- 2D projection of 3D star positions (top-down galactic plane by default)
- Stars colored by spectral type: O=blue, B=blue-white, A=white, F=yellow-white, G=yellow, K=orange, M=red
- Star size proportional to inverse magnitude (brighter = bigger dot)
- Hover → star name tooltip
- Click → select + show info
- Double-click → enter star system

---

## Fantasy vs Reality Mode

### Reality Mode
- Real star positions from HYG database
- Accurate solar system data (JPL-derived)
- Real distances, real physics
- Educational tooltips and data citations

### Fantasy Mode
- Same rendering engine and navigation
- Custom/fictional star systems (player-created or lore-driven)
- Magic-infused planets, impossible orbits, exotic resources
- Player-built space stations and outposts
- Ties into character progression and game economy

### Shared Engine

Both modes use the same:
- Canvas renderer
- Pan/zoom controls
- Data model structures
- Detail panel UI
- Icosphere coordinate system
- Breadcrumb navigation

The only difference is the **data source** fed to the engine.

---

## Per-Planet Bounties

Each celestial body can have associated bounties:

- "Map the surface of Europa"
- "Establish mining operation on asteroid Vesta"
- "Catalog stellar spectra in Sector 7"
- "Build relay station in Alpha Centauri system"

### Integration with Community Bounties (#31)

- Bounties visible on planet detail card
- Completing bounties grants XP, resources, achievements
- Bounty count shown as badge on planet in system view

---

## Storage

### Client-Side

- **Static data**: Star catalog and solar system baked into JavaScript
- **User state**: `localStorage` key `humanity_celestial`
  ```javascript
  {
    homePlanet: 'earth',
    bookmarks: ['proxima_centauri_b', 'titan'],
    visited: ['SOL', 'alpha_centauri'],
    mode: 'reality',           // or 'fantasy'
    viewState: { level: 'system', target: 'SOL', pan: {x:0,y:0}, zoom: 1.0 },
  }
  ```
- Add `humanity_celestial` to `SYNC_KEYS` for cross-device sync

### Future Server-Side

- Full HYG catalog served via API
- Player-created systems stored in database
- Shared bounty state synced across players

---

## Implementation Plan

### Phase 1 (MVP) — Current
- [x] Design document
- [x] Solar system data (hardcoded, accurate)
- [x] ~200 nearest stars from HYG
- [x] Canvas-based interactive star map in Fantasy tab
- [x] Sector view (stellar neighborhood)
- [x] Star system view (orbital diagram)
- [x] Planet detail panel
- [x] Pan, zoom, click interaction
- [x] Breadcrumb navigation
- [x] localStorage persistence

### Phase 2
- [ ] Planet globe view with icosphere overlay
- [ ] Icosphere wireframe visualization (rotate + hover addressing)
- [ ] Galaxy view (Milky Way spiral)
- [ ] Animated orbital positions (Keplerian propagation)
- [ ] Reality/Fantasy mode toggle with separate data

### Phase 3
- [ ] Full HYG catalog via API (~120K stars)
- [ ] Exoplanet data integration
- [ ] Player-created star systems (Fantasy mode)
- [ ] Bounty system integration
- [ ] Surface view placeholder (3D terrain)
- [ ] Deep-sky objects (nebulae, clusters)

### Phase 4
- [ ] Cosmic web / universe view
- [ ] Supercluster navigation
- [ ] Multi-player shared exploration state
- [ ] VR/AR compatibility
