# farming.md

## Purpose

Model food production and plant cultivation as a causal, time-based system under constraint.

The farming system exists to answer:
- What happens to plants when inputs (water, light, nutrients, temperature) change over time?
- What are the costs, risks, and failure modes of producing food and fiber?

---

## Scope

### Included
- Plant lifecycle: germination → vegetative growth → flowering → fruiting/seed → senescence
- Soil as a resource container: structure, moisture, nutrients, salinity, pH
- Watering and irrigation as processes with time/energy/material costs
- Nutrition flows: uptake, deficiency, toxicity
- Environmental effects: temperature bands, light exposure, humidity (as declared by environment system)
- Yield, quality, and spoilage inputs for downstream systems
- Pests/disease pressure as bounded stressors (probabilistic only if bounded and deterministic via seeded events)

### Explicitly Excluded
- Full ecosystem simulation (handled by ecology system)
- Animal husbandry (separate system)
- Human diet/health impacts (health system)
- Weather generation (environment system)
- Storage/spoilage dynamics (storage/preservation system)

---

## Inputs (schemas)

The farming system consumes validated, canonical data instances:

### Entities
- Plant definition (species/cultivar, growth stages, requirements, tolerances)
- Soil definition (texture class, organic matter, nutrient pools, pH range)
- Plot/bed/container definition (volume, drainage, insulation, cover)
- Tool definitions (watering can, drip line, lamp, sensor) as capability descriptors (not magic bonuses)

### State
- Time (tick / day index)
- Environment snapshot: temperature, humidity, light availability, CO2 (as provided)
- Soil state: water content, nutrient availability, salinity, contamination flags
- Plant state: stage, biomass proxies, root depth proxy, stress accumulators, disease/pest flags
- Work actions: plant, transplant, water, fertilize, prune, harvest, treat

### Resources
- Water (quantity, quality)
- Nutrients/fertilizers (composition with units)
- Energy (if lighting/pumps are used)
- Labor/time budget (work actions have declared time cost)

---

## Outputs

Farming produces:
- Updated plant states (growth, stress, stage transitions)
- Updated soil states (water depletion, nutrient uptake, salt accumulation)
- Harvest outputs (items with quantity, quality attributes, moisture level)
- Byproducts (compostables, residues)
- Risk signals (disease onset, pest pressure, deficiency warnings)
- Educational explanations (why outcomes occurred) derived from causal traces

---

## Core Mechanics

### 1) Growth as constrained accumulation
Growth per tick is bounded by limiting factors.

Define a normalized factor for each driver in [0, 1]:
- f_water, f_light, f_temp, f_nutrients, f_rootspace, f_health

Effective growth factor:
- f_growth = min(f_water, f_light, f_temp, f_nutrients, f_rootspace, f_health)

Biomass increment is a deterministic function of:
- cultivar potential × f_growth × stage multiplier × time step

### 2) Water balance
Soil water changes per tick:
- water_next = clamp(water_now + inputs - evapotranspiration - drainage, 0, capacity)

Evapotranspiration is bounded and computed from:
- temperature, humidity, plant stage/leaf proxy, cover type

### 3) Nutrient balance
Nutrient pools change per tick:
- uptake is limited by root proxy, water availability, and nutrient mobility
- deficiency occurs when uptake < stage demand for sustained periods
- toxicity occurs when concentrations exceed cultivar tolerance

### 4) Stress accumulation and recovery
Stress is cumulative and decays when conditions improve.
Stress affects:
- growth rate
- disease susceptibility
- yield quality

### 5) Stage transitions
Stage transitions occur when:
- accumulated thermal time (degree-days) or tick count thresholds are met
- and minimum health conditions are satisfied

### 6) Yield and quality
Yield is derived at harvest from:
- biomass proxy
- stress history
- stage timing (early/late harvest penalties)
- nutrient sufficiency and water stability during critical windows

Quality attributes are explicit, bounded fields (example set):
- size grade
- sugar/starch proxy
- moisture
- defect rate
- contamination flags

---

## Constraints

- Deterministic outcomes: all randomness must be seeded and recorded as events.
- No free production: every output requires time and declared inputs.
- Conservation within declared model: water and nutrients must balance across pools.
- Explainability: every failure/shortfall must map to a limiting factor or stress trace.
- Low-power viability: computations must be O(active_plants) per tick with bounded constant factors.

---

## Failure Modes

- Drought stress → stunting → crop failure
- Overwatering → oxygen stress → root damage → disease susceptibility
- Nutrient deficiency/toxicity → reduced yield/quality
- Temperature extremes → stage disruption → sterility/fruit drop
- Contamination (salinity/heavy metals/pathogens) → flagged outputs, restricted use
- Neglect (missed critical windows) → irreversible yield loss

Failures must be:
- visible
- attributable
- reversible only when reality supports reversal

---

## Interactions

- Ecology: pest/disease pressure drivers, soil biota modifiers
- Storage/Preservation: moisture level and quality influence spoilage curves
- Health: consumption impacts, poisoning risks (downstream)
- Construction/Industry: irrigation, lighting, tooling, containers
- Economy: pricing, labor allocation, resource scarcity
- Education: competency progression tied to demonstrated causal understanding

---

## Abstractions

Allowed abstractions if explicitly documented in data:
- Soil as layered buckets rather than full fluid dynamics
- Root growth as a proxy value rather than geometry
- Disease modeled as state machine with thresholds rather than microbiology

Forbidden:
- Hidden bonuses, “rarity yield multipliers,” or unexplained production

---

## Verification Requirements (tests)

Minimum suite:
- Water conservation properties
- Nutrient conservation properties
- Replay determinism for identical action logs
- Limiting-factor explanations match actual min-factor
- Bounds: no negative quantities, no growth beyond cultivar maxima

---
