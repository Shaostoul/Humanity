# design/schemas/resources/nutrients.schema.md

## Purpose

Defines the canonical data contract for nutrient inputs (fertilizers/amendments) used by farming.

Nutrients are modeled as explicit pools with units and optional risk flags.

---

## Schema: NutrientResource

### Identity
- `nutrient_id` (string, required, stable identifier)
- `common_name` (string, required)

### Quantity and Composition

All nutrient values are in milligrams unless otherwise specified.

- `quantity_units` (string, required)  
  Allowed: `mg`, `g`, `kg`
- `quantity_value` (number, required, > 0)

Composition pools (mg-per-quantity, required minimum set):
- `nitrate_mg`
- `phosphate_mg`
- `potassium_mg`

Optional:
- `calcium_mg`
- `magnesium_mg`
- `sulfur_mg`
- `micros_mg`

### Delivery characteristics (optional)
- `release_profile` (enum, optional)  
  Allowed: `immediate`, `slow_release`, `composting`
- `salinity_risk_index` (number, optional, 0.0–1.0)

### Contamination risk (optional)
- `contamination_flags` (array of string, optional)

Common flags (non-exhaustive):
- `heavy_metals`
- `pathogen_risk`
- `chemical_residue`

---

## Global Invariants

- Unknown fields are forbidden unless explicitly added to the schema.
- All composition pools must be finite numbers and >= 0.
- `quantity_value` must be > 0.
- Index values must be within 0.0–1.0 when present.
