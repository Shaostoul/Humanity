# design/systems/farming/states.md

## Purpose

This document defines the authoritative state variables for the Farming system.

States describe what can exist and what can be true at a given time.  
States do not define behavior. Behavior belongs in `processes.md`.

All state fields must be:
- explicit
- bounded
- unit-consistent
- explainable

---

## State Model Overview

Farming state is organized into:
- Plant state (per cultivated plant instance)
- Substrate state (soil/bed/container medium)
- Plot state (the cultivation site holding plants + substrate)
- Derived indicators (computed for explanation, not authoritative)

---

## Plant State (per plant instance)

### Identity
- `plant_id` (stable identifier)
- `species_id` (definition reference)
- `cultivar_id` (optional definition reference)
- `plot_id` (current location reference)

### Lifecycle
- `stage` (enum)  
  Allowed: `seed`, `germinating`, `seedling`, `vegetative`, `flowering`, `fruiting`, `maturing`, `senescent`, `dead`
- `age_ticks` (non-negative integer)
- `stage_progress` (0.0–1.0)  
  Progress within current stage.

### Growth Proxies (bounded)
These are proxies that support causal modeling without requiring full physiology.
- `biomass_index` (0.0–B_max)  
  Canonical proxy for total growth.
- `root_index` (0.0–R_max)  
  Proxy for root development / access to substrate.
- `canopy_index` (0.0–C_max)  
  Proxy for leaf area / transpiration demand.

All maxima (B_max/R_max/C_max) are defined by the plant definition.

### Hydration and Nutrition (plant-local accumulators)
- `water_stress` (0.0–S_max)
- `nutrient_stress` (0.0–S_max)
- `temperature_stress` (0.0–S_max)
- `light_stress` (0.0–S_max)
- `space_stress` (0.0–S_max)
- `health_stress` (0.0–S_max)

Stress values accumulate under deficit/excess and decay under favorable conditions.

### Health and Damage
- `health` (0.0–1.0)  
  1.0 = fully healthy relative to definition.
- `damage_flags` (set)  
  Allowed flags are definition-driven, but common flags include:
  - `frost_damage`
  - `heat_damage`
  - `root_rot_risk`
  - `pest_damage`
  - `disease_symptoms`
- `disease_state` (enum)  
  Allowed: `none`, `exposed`, `incubating`, `active`, `recovering`, `chronic`
- `pest_pressure` (0.0–1.0)  
  Represents current pressure when applicable.

### Reproduction and Yield Readiness
- `reproductive_index` (0.0–1.0)  
  Proxy for readiness to flower/fruit.
- `yield_potential` (0.0–1.0)  
  Canonical proxy influenced by stress history.
- `harvest_ready` (boolean)

### Contamination and Safety
- `contamination_flags` (set)  
  Example flags:
  - `heavy_metals`
  - `pathogens`
  - `chemical_residue`
- `edibility_state` (enum)  
  Allowed: `safe`, `restricted_use`, `unsafe`

---

## Substrate State (per soil/container medium)

### Identity
- `substrate_id`
- `substrate_type_id` (definition reference)
- `plot_id`

### Capacity and Structure
- `volume_m3` (>= 0)
- `water_capacity_l` (>= 0)
- `drain_rate` (0.0–1.0)  
  Proportion lost per tick under saturation conditions (bounded abstraction).
- `aeration_index` (0.0–1.0)  
  Proxy for oxygen availability at roots.

### Water State
- `water_l` (0.0–water_capacity_l)
- `water_quality_flags` (set)  
  Example: `salty`, `contaminated`, `stagnant`

### Nutrient Pools (bounded)
Nutrients are represented as explicit pools with units.
Minimum pools:
- `nitrate_mg` (>= 0)
- `phosphate_mg` (>= 0)
- `potassium_mg` (>= 0)

Optional pools:
- `calcium_mg`, `magnesium_mg`, `sulfur_mg`, `micros_mg`

### Chemistry Bands (optional, bounded)
- `ph` (0.0–14.0) if modeled
- `salinity_index` (0.0–1.0) if modeled
- `organic_matter_index` (0.0–1.0) if modeled

### Contamination
- `contamination_flags` (set)  
  Example: `heavy_metals`, `pathogens`, `chemical_spill`

---

## Plot State (per cultivation site)

### Identity and Layout
- `plot_id`
- `plot_type` (enum)  
  Allowed: `ground_bed`, `raised_bed`, `container`, `hydroponic_unit` (if supported)
- `area_m2` (>= 0)
- `plant_slots` (>= 0 integer) or `plant_density_limit` (>= 0)

### Exposure and Protection
- `cover_state` (enum)  
  Allowed: `open`, `mulched`, `row_cover`, `greenhouse`, `shade_cloth`
- `light_access_factor` (0.0–1.0)  
  Plot-level modifier; environment provides the base.

### Infrastructure Attachments (references)
- `irrigation_attachment_id` (optional)
- `lighting_attachment_id` (optional)
- `sensor_attachment_ids` (optional set)

Attachments provide capabilities but do not override constraints.

---

## Derived Indicators (non-authoritative)

Derived indicators are computed for explanation and UI. They are not canonical state.

Examples:
- `limiting_factor` (enum) derived from min of growth drivers
- `water_balance_delta` per tick
- `nutrient_balance_delta` per tick
- `stress_trend` (rising/stable/falling)

---

## Invariants (must always hold)

- All quantities with units must remain non-negative unless explicitly modeled otherwise.
- `water_l` must remain within `[0, water_capacity_l]`.
- `health` remains within `[0, 1]`.
- `stage_progress` remains within `[0, 1]`.
- Enum values must be from declared sets; unknown values are invalid.
- Contamination flags propagate to outputs via defined rules in processes.

---

## Notes on Abstraction

This state model is intentionally bounded:
- proxies are used instead of full physiology
- chemistry is represented by bands/pools, not continuous reaction simulation
- pest/disease can be deterministic event-driven stress unless higher-fidelity ecology is enabled

All abstraction must remain explicit and explainable.
