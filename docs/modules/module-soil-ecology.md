# module-soil-ecology

## 1) Module identity
- **Module name:** Soil Ecology
- **Crate/package id:** `module-soil-ecology`
- **Domain:** homestead/life
- **Status:** draft

## 2) Purpose
Model soil as a living system: structure, nutrients, biology, moisture, and degradation/recovery cycles.

## 3) Scope
### In-scope
- Soil texture/structure classes
- Moisture retention/drainage
- Nutrient cycling
- Biological activity abstraction
- Erosion and compaction

### Out-of-scope
- Advanced geochemical lab simulation

## 4) Inputs / outputs
### Inputs
Weather, amendments, tillage intensity, crop usage, water inputs.

### Outputs
Fertility scores, stress flags, crop suitability, long-term soil trajectory.

## 5) Core simulation model
State-grid with seasonal transitions and process rates (decomposition, leaching, compaction recovery).

## 6) Lifeform parity requirements
- Plant-root ecology impact L2-L3
- Soil biota abstracted L1-L2
- Animal trampling/grazing impacts L2

## 7) Teaching design
Teach soil stewardship, amendment tradeoffs, regeneration timing.

## 8) Gameplay hooks
Land quality, yield potential, restoration missions, community planning.

## 9) API boundary
- `SoilCell`
- `SoilProfile`
- `apply_amendment(...)`
- `simulate_season(...)`

## 10) Test plan
Nutrient/mass sanity checks, erosion regression, recovery trajectory tests.

## 11) Performance budget
Chunked updates by plot/region.

## 12) Security/safety constraints
Provide educational guidance, not hazardous chemical handling instructions.

## 13) Documentation contract
Example scenario: degraded field recovery over 2 seasons.
