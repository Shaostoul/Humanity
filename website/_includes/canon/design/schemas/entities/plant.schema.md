# design/schemas/entities/plant.schema.md

## Purpose

Defines the canonical data contract for a Plant **definition** and a Plant **instance state** used by the Farming system.

This schema enforces:
- explicitness (no hidden fields)
- bounded values
- stable enums
- unit consistency

Plant data describes what exists and what is true.  
It does not encode behavior. Behavior lives in system specs.

---

## Schema: PlantDefinition

### Identity
- `species_id` (string, required, stable identifier)
- `cultivar_id` (string, optional)
- `common_name` (string, required)
- `scientific_name` (string, optional)
- `tags` (array of string, optional)

### Lifecycle Model
- `stages` (array of StageDefinition, required, non-empty)

StageDefinition:
- `stage` (enum, required)  
  Allowed: `seed`, `germinating`, `seedling`, `vegetative`, `flowering`, `fruiting`, `maturing`, `senescent`, `dead`
- `order` (integer, required, >= 0, unique across stages)
- `min_ticks` (integer, required, >= 0)
- `max_ticks` (integer, optional, >= min_ticks)
- `transition_conditions` (array of string, optional)  
  Human-readable declarative conditions; must not encode engine logic.

### Growth Proxy Bounds
- `biomass_max` (number, required, > 0)
- `root_max` (number, required, > 0)
- `canopy_max` (number, required, > 0)

### Environmental Requirements (bands, bounded)
All requirement bands are inclusive and must be coherent (min <= opt_min <= opt_max <= max).

- `temperature_c` (Band, required)
- `light_factor` (Band01, required)  
  Normalized 0.0–1.0 availability.
- `humidity_factor` (Band01, optional)
- `co2_factor` (Band01, optional)

Band:
- `min` (number, required)
- `opt_min` (number, required)
- `opt_max` (number, required)
- `max` (number, required)

Band01:
- same as Band, but all values must be within 0.0–1.0

### Water Requirements
- `water_demand_l_per_tick` (BandNonNeg, required)
- `water_tolerance` (Band01, required)  
  Represents how sensitive growth is to water deviation.

BandNonNeg:
- same as Band, but all values must be >= 0

### Nutrient Requirements (minimum pools)
All values are in milligrams per tick demand bands unless otherwise specified.

- `nitrate_mg_per_tick` (BandNonNeg, required)
- `phosphate_mg_per_tick` (BandNonNeg, required)
- `potassium_mg_per_tick` (BandNonNeg, required)

Optional:
- `calcium_mg_per_tick` (BandNonNeg, optional)
- `magnesium_mg_per_tick` (BandNonNeg, optional)
- `sulfur_mg_per_tick` (BandNonNeg, optional)
- `micros_mg_per_tick` (BandNonNeg, optional)

### Soil Compatibility (optional)
- `ph_band` (BandPH, optional)  
  If present, constrains allowable substrate pH.
- `salinity_tolerance` (Band01, optional)

BandPH:
- same as Band, but all values must be within 0.0–14.0

### Stress Model (bounds)
- `stress_max` (number, required, > 0)
- `stress_decay_per_tick` (number, required, >= 0)
- `stress_accumulation_rates` (object, required)
  Keys (required): `water`, `nutrients`, `temperature`, `light`, `space`, `health`
  Values: numbers >= 0 (rate per tick under full deficit)

### Yield Model (bounded)
- `yield_type` (enum, required)  
  Allowed: `leaf`, `root`, `fruit`, `seed`, `fiber`, `multi`
- `yield_units` (string, required)  
  Example: `kg`, `count`
- `base_yield` (number, required, >= 0)
- `yield_sensitivity` (Band01, required)  
  Sensitivity of yield to stress history and timing.
- `harvest_window_ticks` (object, required)
  - `early_penalty_factor` (number, required, 0.0–1.0)
  - `late_penalty_factor` (number, required, 0.0–1.0)
  - `ideal_start_tick` (integer, required, >= 0)
  - `ideal_end_tick` (integer, required, >= ideal_start_tick)

### Safety and Contamination Policy (optional)
- `edible` (boolean, required)
- `contamination_rules` (array of string, optional)  
  Declarative rules describing how contamination affects edibility state.

---

## Schema: PlantInstanceState

### Identity and References
- `plant_id` (string, required, stable identifier)
- `species_id` (string, required, must match a PlantDefinition)
- `cultivar_id` (string, optional)
- `plot_id` (string, required)

### Lifecycle
- `stage` (enum, required; same enum as definition)
- `age_ticks` (integer, required, >= 0)
- `stage_progress` (number, required, 0.0–1.0)

### Growth Proxies
- `biomass_index` (number, required, >= 0, <= PlantDefinition.biomass_max)
- `root_index` (number, required, >= 0, <= PlantDefinition.root_max)
- `canopy_index` (number, required, >= 0, <= PlantDefinition.canopy_max)

### Stress Accumulators
Each required, 0.0–PlantDefinition.stress_max:
- `water_stress`
- `nutrient_stress`
- `temperature_stress`
- `light_stress`
- `space_stress`
- `health_stress`

### Health and Condition
- `health` (number, required, 0.0–1.0)
- `damage_flags` (array of string, optional)
- `disease_state` (enum, required)  
  Allowed: `none`, `exposed`, `incubating`, `active`, `recovering`, `chronic`
- `pest_pressure` (number, required, 0.0–1.0)

### Reproduction and Harvest
- `reproductive_index` (number, required, 0.0–1.0)
- `yield_potential` (number, required, 0.0–1.0)
- `harvest_ready` (boolean, required)

### Safety
- `contamination_flags` (array of string, optional)
- `edibility_state` (enum, required)  
  Allowed: `safe`, `restricted_use`, `unsafe`

---

## Global Invariants

- Unknown fields are forbidden unless explicitly added to the schema.
- All quantities must be finite numbers (no NaN/Inf).
- All enum values must match allowed sets.
- All references must resolve to existing definitions.
- Any violation is a validation failure.

---

## Notes on Abstraction

This schema intentionally uses bounded proxies and bands:
- to keep computation low-cost
- to keep explanations legible
- to preserve determinism

Higher fidelity can be introduced only by extending schemas explicitly and updating system specs and tests together.
