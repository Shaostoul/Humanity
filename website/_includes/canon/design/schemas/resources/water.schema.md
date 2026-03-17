# design/schemas/resources/water.schema.md

## Purpose

Defines the canonical data contract for Water resources used by farming and other systems.

Water is modeled as:
- quantity
- quality flags
- optional contaminant indicators

Water data describes what exists and what is true.  
It does not encode behavior.

---

## Schema: WaterResource

### Identity
- `water_id` (string, required, stable identifier)
- `common_name` (string, required)

### Quantity
- `quantity_l` (number, required, >= 0)

### Quality
- `quality_flags` (array of string, optional)

Common flags (non-exhaustive):
- `potable`
- `non_potable`
- `salty`
- `hard`
- `stagnant`
- `contaminated`
- `pathogen_risk`
- `chemical_risk`

### Optional indicators (bounded)
- `salinity_index` (number, optional, 0.0–1.0)
- `pathogen_index` (number, optional, 0.0–1.0)
- `chemical_index` (number, optional, 0.0–1.0)

---

## Global Invariants

- Unknown fields are forbidden unless explicitly added to the schema.
- Quantities must be finite numbers and non-negative.
- Index values must be within 0.0–1.0 when present.
