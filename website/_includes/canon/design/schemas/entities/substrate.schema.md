# design/schemas/entities/substrate.schema.md

## Purpose

Defines the canonical data contract for Substrate (soil or growth medium) **definitions** and Substrate **instance state** used by the Farming system.

Substrate represents the medium that holds:
- water
- nutrient pools
- chemistry bands (optional)
- contamination flags

Substrate data describes what exists and what is true.  
It does not encode behavior.

---

## Schema: SubstrateDefinition

### Identity
- `substrate_type_id` (string, required, stable identifier)
- `common_name` (string, required)
- `tags` (array of string, optional)

### Physical Capacity
- `default_volume_m3` (number, required, > 0)
- `water_capacity_l_per_m3` (number, required, > 0)
- `drain_rate` (number, required, 0.0–1.0)
- `aeration_index` (number, required, 0.0–1.0)

### Nutrient Pool Defaults (mg per m3)
Minimum pools (required, >= 0):
- `nitrate_mg_per_m3`
- `phosphate_mg_per_m3`
- `potassium_mg_per_m3`

Optional (>= 0):
- `calcium_mg_per_m3`
- `magnesium_mg_per_m3`
- `sulfur_mg_per_m3`
- `micros_mg_per_m3`

### Chemistry Bands (optional)
- `ph` (number, optional, 0.0–14.0)
- `salinity_index` (number, optional, 0.0–1.0)
- `organic_matter_index` (number, optional, 0.0–1.0)

### Contamination Defaults
- `contamination_flags` (array of string, optional)
- `water_quality_flags` (array of string, optional)

---

## Schema: SubstrateInstanceState

### Identity and References
- `substrate_id` (string, required, stable identifier)
- `substrate_type_id` (string, required, must match a SubstrateDefinition)
- `plot_id` (string, required)

### Capacity and Structure
- `volume_m3` (number, required, > 0)
- `water_capacity_l` (number, required, > 0)
- `drain_rate` (number, required, 0.0–1.0)
- `aeration_index` (number, required, 0.0–1.0)

Invariant:
- `water_capacity_l` must equal `volume_m3 * water_capacity_l_per_m3` within declared tolerance, unless explicitly overridden.

### Water State
- `water_l` (number, required, 0.0–water_capacity_l)
- `water_quality_flags` (array of string, optional)

### Nutrient Pools (mg)
Minimum pools (required, >= 0):
- `nitrate_mg`
- `phosphate_mg`
- `potassium_mg`

Optional (>= 0):
- `calcium_mg`
- `magnesium_mg`
- `sulfur_mg`
- `micros_mg`

### Chemistry Bands (optional but bounded)
- `ph` (number, optional, 0.0–14.0)
- `salinity_index` (number, optional, 0.0–1.0)
- `organic_matter_index` (number, optional, 0.0–1.0)

### Contamination
- `contamination_flags` (array of string, optional)

---

## Global Invariants

- Unknown fields are forbidden unless explicitly added to the schema.
- All quantities must be finite numbers (no NaN/Inf).
- All numeric pools must be non-negative.
- All enum-like fields must be represented as explicit sets where applicable.
- All references must resolve to existing definitions.
- Any violation is a validation failure.

---

## Notes on Abstraction

Substrate uses:
- pooled nutrients (not full chemistry)
- bounded drain/aeration indices (not full fluid dynamics)

If higher fidelity is needed, extend the schema explicitly and update system behavior and tests together.
