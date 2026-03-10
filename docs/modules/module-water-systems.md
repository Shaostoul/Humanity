# module-water-systems

## 1) Module identity
- **Module name:** Water Systems
- **Crate/package id:** `module-water-systems`
- **Domain:** homestead
- **Status:** draft
- **Owners:** sustainability systems

## 2) Purpose
Simulate water sourcing, storage, filtration, transport, contamination, and sanitation outcomes for self-sustaining settlements.

## 3) Scope
### In-scope
- Rain catchment, wells, surface collection
- Storage vessel integrity and spoilage risk
- Filtration/boil/chlorination abstractions
- Potable vs non-potable water states

### Out-of-scope
- City-scale municipal networks (later module)

## 4) Inputs / outputs
### Inputs
- Weather and source availability
- Container/material properties
- User handling and treatment actions

### Outputs
- Water quantity/quality state
- Disease risk modifiers
- Teachable warnings and remediation options

## 5) Core simulation model
Mass-balance + contamination probability + treatment efficacy curves with deterministic seedable randomness.

## 6) Lifeform parity requirements
- Humans: hydration and illness impacts L3
- Livestock: hydration and contamination impacts L2-L3
- Crops: irrigation quality/quantity impacts L2

## 7) Teaching design
Concepts: safe water chain, contamination vectors, emergency purification, conservation.

## 8) Gameplay hooks
- Settlement health and labor productivity
- Quest events: drought, contamination incident, repair missions

## 9) API boundary
- `WaterNode`
- `WaterQuality`
- `treat_water(...)`
- `route_water(...)`
- `risk_report(...)`

## 10) Test plan
- Conservation checks
- Treatment efficacy tests
- Outbreak risk regression tests

## 11) Performance budget
Low-medium per-node tick cost; scalable by region aggregation.

## 12) Security/safety constraints
No unsafe real-world procedure detail beyond high-level safety principles.

## 13) Documentation contract
Example scenario: rain catchment to household + livestock trough + contamination event.
