# Star Systems Data

Per-system celestial body data for the Cosmos system. See
`docs/design/cosmos-architecture.md` for the full architectural context.

## Structure

```
data/star_systems/
  index.json         # registry of all known bound systems
  sol.json           # bodies bound to Sol
  alpha_centauri.json  # ← drop in to add a new system, no code changes needed
  ...
```

## Adding a new system

1. Pick an `id` (lowercase, snake_case, ASCII): `alpha_centauri`, `trappist_1`,
   `proxima_b_system`, etc.
2. Create `data/star_systems/{id}.json` with the schema below.
3. Add an entry to `index.json` with the system's galaxy position
   (light-years from Sol).
4. Restart the relay / app. The new system appears in the system switcher.

No Rust code changes required. This is the "infinite of x" rule applied to
star systems.

## Per-system file schema

```jsonc
{
  "id": "alpha_centauri",
  "name": "Alpha Centauri",
  "primary_star": "alpha_centauri_a",
  "galaxy_position_ly": [-1.348, -3.972, -1.535],
  "meta": {
    "epoch": "J2000.0",
    "source": "wherever the data came from",
    "units": {
      "mass": "kg",
      "radius": "km",
      "distance": "AU or km as labeled",
      "temperature": "K",
      "gravity": "m/s^2",
      "rotation": "hours",
      "orbital_period": "days",
      "pressure": "atm"
    }
  },
  "bodies": [
    {
      "id": "alpha_centauri_a",
      "name": "Alpha Centauri A",
      "type": "star",
      "parent": null,
      "orbit": null,
      "physical": { ... },
      "atmosphere": { ... },
      "rings": false,
      "moons": []
    },
    {
      "id": "alpha_centauri_b",
      "name": "Alpha Centauri B",
      "type": "star",
      "parent": "alpha_centauri_a",
      "orbit": { ... },
      ...
    }
  ]
}
```

## Index file schema

`index.json`:

```jsonc
{
  "epoch": "J2000.0",
  "systems": [
    {
      "id": "sol",
      "name": "Solar System",
      "primary_star_name": "Sun",
      "spectral_class": "G2V",
      "galaxy_position_ly": [0, 0, 0],
      "distance_from_sol_ly": 0,
      "data_source": "hand_authored",  // or "procedural"
      "data_file": "sol.json"
    }
  ]
}
```

## Data source: procedural vs hand-authored

Per operator (2026-05-09), each body / system can be EITHER:
- **`procedural`** — generated from a seed using rules (e.g. Earth-like
  planet from `radius=6371 km, atmosphere=O2/N2, surface=70% water`).
  Cheap to author, immediate visual results, no per-body data file.
- **`hand_authored`** — explicit data file with measured/sourced values
  (real elevation maps, real cloud cover, real atmosphere composition by
  trace gas). Higher fidelity but requires curation work.

Procedural is the default. Hand-authoring an existing procedural body is
non-destructive: the procedural data continues to work as a fallback if
the hand-authored file is missing fields.

## Migration history

- **2026-05-09 (v0.199)** — Sol moved here from `data/solar_system/bodies.json`.
  Old path removed. Wrapper schema (id / name / primary_star / galaxy_position_ly)
  added so the file is multi-system-ready.
