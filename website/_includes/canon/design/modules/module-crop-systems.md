# module-crop-systems

## 1) Module identity
- **Module name:** Crop Systems
- **Crate/package id:** `module-crop-systems`
- **Domain:** homestead/life
- **Status:** draft

## 2) Purpose
Simulate crop lifecycle outcomes from seed to harvest under weather, soil, pest, disease, and management conditions.

## 3) Scope
### In-scope
- Germination, growth stages, flowering, harvest
- Water/light/temp stress
- Pest and disease pressure abstraction
- Pollination effects

### Out-of-scope
- Full molecular plant biology

## 4) Inputs / outputs
### Inputs
Soil state, climate/weather, water plan, crop profile, interventions.

### Outputs
Yield/quality, failure reasons, nutrient extraction, teachable diagnostics.

## 5) Core simulation model
Stage-based growth model with stress accumulation and threshold-triggered outcomes.

## 6) Lifeform parity requirements
- Plant organisms modeled at L2-L3
- Pollinator impacts integrated at L1-L2
- Human labor/skill effects at L2

## 7) Teaching design
Teach planting timing, rotation, integrated risk mitigation, and diagnosis from symptoms.

## 8) Gameplay hooks
Food supply, trade goods, famine risk, seasonal planning gameplay loops.

## 9) API boundary
- `CropInstance`
- `GrowthStage`
- `apply_intervention(...)`
- `tick_growth(...)`
- `harvest_report(...)`

## 10) Test plan
Growth progression tests, stress edge-case tests, yield consistency tests.

## 11) Performance budget
Vectorized/aggregate simulation for large fields when distant.

## 12) Security/safety constraints
Avoid prescription-level pesticide/chemical instructions.

## 13) Documentation contract
Example scenario: three-plot rotation with drought + pollinator decline.
