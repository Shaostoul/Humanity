# Agartha: the artificial planet (and per-planet physics in general)

> Created 2026-08-12 from operator direction. This is the doc that
> `terrain-detail.md` promised ("Agartha designs move to the artificial-planet
> doc when the story arc starts"). It covers two things that turned out to be
> the same engineering problem: making every planet's physical profile real,
> data-driven, and tunable in-game, and specifying the manufactured world the
> main story arc lands on.

## Operator direction (2026-08-12, condensed)

The space-to-surface-to-underground experience should feel natural: gravity
transition, atmosphere, temperature, magnetic field, all of it, and all of it
readable in-game. At the same time the data model must allow UNREALISTIC
worlds on purpose (an Earth-sized world with crushing gravity, or the story
planet: a manufactured structure roughly twice Earth's physical size with
Earth-like surface gravity, possibly varying dynamically as you wander).
Values should be adjustable quickly, ideally live in-game; hot reload is the
preferred path over restarts.

## Agartha: the physical spec

Numbers derived 2026-08-12; these are the first written spec, treat them as
canon until the story arc revises them.

| Property | Value | Why |
|---|---|---|
| Radius | 12,742 km (2.0x Earth) | operator direction |
| Surface gravity | 9.81 m/s2 (1.00 g) | operator direction |
| Implied mass | 2.39e25 kg (4.0x Earth) | g = GM/R^2; doubling R at fixed g needs exactly 4x M |
| Mean density | ~2,760 kg/m3 (0.50x Earth) | 4x mass in 8x volume |
| Escape velocity | ~15.8 km/s (1.41x Earth) | sqrt(2GM/R) |
| Surface area | 4x Earth (about 2.04 billion km2) | area scales with R^2 |

The density line is the tell, and it is story gold: no rocky planet forms at
half Earth's density at that size (self-compression pushes big rocky worlds
DENSER, not lighter). Any spacefaring observer with a telescope and a moon to
watch can compute Agartha's density and know something is wrong with it
before ever landing. The planet publicly advertises its own artificiality to
anyone doing physics homework, which fits "the artificial planet IS the
fleet's destination mystery" (terrain-detail.md).

### Gravity inside a hollow world is manufactured, by physics

The shell theorem says a uniform hollow shell exerts ZERO net gravity
anywhere inside it. So the moment Agartha is hollow (and at half Earth
density it must be), the inner surface CANNOT have natural gravity pointing
outward at your feet. Rotation cannot fake it either: spin gravity on a
sphere varies with latitude and would leave the poles weightless. Therefore:

- Outer surface: natural gravity, a true 1 g from the 4-Earth-mass shell.
- Interior: whatever the builders' machinery supplies. Canonically this is
  manufactured gravity, which justifies the operator's "dynamically variable
  as the user wanders" as a FEATURE of the machine: gradients in transit
  shafts, weightless mid-shell bands, sections where the machinery is
  damaged or tuned differently, story events that change it.

### How the engine expresses this: `gravity_curve`

Shipped 2026-08-12 in `PlanetDef` (`src/terrain/planet.rs`):

```ron
// data/planets/agartha.ron (illustrative)
gravity: 9.81,                       // outer-surface baseline
gravity_curve: Some([
    (-400000.0, 9.81),   // inner surface: machine-supplied 1 g
    (-250000.0, 0.0),    // mid-shell transit band: weightless
    (0.0,       9.81),   // outer surface: natural 1 g
    (100000.0,  8.2),    // falls off with altitude above the surface
]),
```

Control points are (altitude_m, g_m_s2), piecewise-linear, clamped flat past
the ends, negative altitude = below the nominal surface. `None` keeps the
constant `gravity` field, which is correct for every natural body at
walkable scales (real 1/r^2 falloff across a 10 km walk band is under 0.4%,
imperceptible). Curves are sanitized at load (`normalize_gravity_curve`), so
live hand-edits cannot crash the sampler. Lateral variation (different g in
different REGIONS at the same altitude) is deliberately out of scope until
the story needs it; when it does, add optional region overrides keyed off
the same data file rather than inventing a second mechanism.

## What is already data-driven today (verified by code audit, 2026-08-12)

- `data/planets/<id>.ron` (earth, moon, mars, pluto): radius, gravity,
  terrain seeds, atmosphere color + density + scale height, cloud coverage,
  water, palette, real heightmap/albedo grids. Surface walking genuinely
  uses per-body gravity from this file (Moon 1.62 works today).
- HOT RELOAD IS LIVE for these files: the watcher rebuilds the whole planet
  within a frame of saving the RON, and the in-app Files page can edit them,
  so planet tuning without restart already works end to end.
- `data/star_systems/sol.json`: orbital elements, mass, radius, temperature,
  atmosphere summaries for 69 bodies. Display + orbit propagation only.

## The gap list (audit findings the arc must close)

1. FOUR DISCONNECTED GRAVITIES. Planet walk reads the def; ship-interior
   walking hardcodes 12.0 m/s2 (`renderer/camera.rs`); ocean wave dispersion
   hardcodes 9.81; the dormant rapier world hardcodes -9.81 world-Y. Unify:
   one `gravity_at` source, ship interior follows the frame-locked body when
   landed (its own field when in space), waves take g at spectrum build.
2. NO ALTITUDE TRANSITION. g is constant in the walk band and simply absent
   above it. Wire `gravity_at(alt)` into the walk-band integrator
   (`lib.rs` ~4512 -> `surface_walk::vertical_step`) so approach, surface,
   and underground all sample one continuous profile.
3. TEMPERATURE IS ONE GLOBAL EARTH TABLE. WeatherSystem's hardcoded season
   -> Celsius table feeds vitals, farming, and evaporation on every body.
   Needs a per-body model: baseline from sol.json `mean_temperature_k`,
   modulated by latitude, altitude (lapse rate), season, and day/night.
   `data/biomes.ron` temperature ranges exist but nothing loads them.
4. MAGNETIC FIELD: ZERO CODE. Values exist only in the orphaned lore files
   (`data/solar_system/*.ron`, no deserializer). Add `magnetic_field_t` to
   the parsed profile; first consumers are the info readout and a compass;
   radiation shielding and aurorae later.
5. WEATHER IS BODY-BLIND. It can rain on the Moon (the precipitation gate
   checks altitude only). Gate weather on the frame-locked body's
   atmosphere; per-body weather profiles (Mars dust, airless none) follow.
6. BREATHABILITY HOOK UNUSED. `AtmosphereSystem::set_outside_atmosphere`
   was built for exactly this and has no production caller; Mars breathes
   like Earth today. Feed it from the body's atmosphere fields.
7. `sol.json` IS COMPILE-TIME EMBEDDED (include_str! + OnceLock), so
   adding a body or editing mass needs a rebuild. Load from disk with the
   embedded copy as fallback, like other data, so new worlds (and the
   destination system) are pure data drops.
8. DEAD/DUPLICATED SHAPES. PlanetDef's four orbital fields are consumed by
   nothing (Kepler truth lives in sol.json); `terrain/planet_registry.rs` is
   never instantiated; the native Maps page reads a `bodies.json` that no
   longer ships (its planet list renders empty). Delete or repoint.

## In-game tuning (the planet tuner)

The plumbing already exists: PlanetDef derives Serialize, the watcher
regenerates on save, and repeated reloads reuse material slots (no VRAM
leak). What is missing is only UI: a Dev-page Planet Tuner that shows the
frame-locked body's def as sliders/fields and writes the RON on change.
One caveat: a whole-struct serialize would destroy the extensive hand-written
comments in earth.ron, so the tuner must do targeted single-field value
rewrites (find the `field:` line, replace the value), not reserialize.

Readout side (the "look at that info in-game" half): the body info card /
HUD should show live values at the player's position: current g (from
gravity_at), altitude, air pressure and breathability, temperature, magnetic
field, day length. Every value from data, none hardcoded in the UI.

## Increment ladder

1. DONE (2026-08-12): `gravity_curve` + `gravity_at` + load-time
   normalization + tests. Data model accepts manufactured worlds.
2. One gravity truth: wire `gravity_at` into the walk integrator; ship
   interior g becomes data (body-following when landed); expose current g
   in the F2/debug readout. (Ocean-wave g stays Earth-constant until a
   water world other than Earth exists; the FFT lockstep tests make that a
   deliberate, separate change.)
3. Planet Tuner dev page + live info readout (needs the targeted-field RON
   rewriter).
4. Per-body environment: weather gating by atmosphere, breathability via
   set_outside_atmosphere, temperature model v1 (baseline + latitude +
   altitude + season).
5. sol.json disk-load + parsed physical profile (mass, magnetic field,
   pressure) + Maps-page bodies.json fix + dead-shape cleanup.
6. Agartha authoring: destination system data file, agartha.ron with the
   curve above, terrain seeds; interior spaces ride the voxel-terrain arc
   (docs/design/voxel-terrain.md) when it starts.
