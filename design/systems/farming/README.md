# design/systems/farming/README.md

## Purpose

Farming models plant cultivation and harvest as a time-based, causal system under constraint. It produces food and fiber by transforming water, nutrients, energy, and labor into biological growth, with explicit failure modes and explainable outcomes.

---

## Scope

### Included
- Plant lifecycle state and transitions (germination → growth → reproduction → senescence)
- Soil/container state relevant to cultivation (moisture, nutrient availability, salinity/pH bands where modeled)
- Work actions: plant, transplant, water, fertilize, prune, harvest, remove
- Growth limiting factors and stress accumulation (water/light/temperature/nutrients/root-space/health)
- Yield and quality outputs as explicit fields for downstream systems
- Deterministic pest/disease pressure as bounded stressors when supplied via events or environment/ecology interfaces

### Excluded
- Full ecosystem simulation (Ecology system)
- Weather generation and seasonal dynamics (Environment system)
- Animal husbandry (Fauna/Husbandry system)
- Human nutrition/health outcomes from consumption (Health system)
- Storage and spoilage dynamics after harvest (Storage/Preservation system)
- Industrial processing of outputs (Industry/Manufacturing system)

---

## Interfaces

Farming consumes:
- Environment snapshots (temperature, light availability, humidity bands, CO2 if modeled)
- Soil substrate definitions and current soil state
- Water and nutrient resource inventories and qualities
- Action inputs (human work) with time/energy costs
- Optional event streams (pest pressure, disease exposure) when provided

Farming produces:
- Updated plant state
- Updated soil/container state
- Harvest outputs (quantity + quality attributes)
- Residues/byproducts (compostables, biomass)
- Signals and explanations (limiting factors, stress history, failure attribution)

---

## Primary System Invariants

- No free outputs: all growth and yield require time and declared inputs.
- Conservation within the declared model: water and nutrients must balance across pools.
- Determinism: identical state + identical actions + identical time progression produce identical outcomes.
- Explainability: every shortfall is attributable to limiting factors, stress, or defined failure cases.
- Low-power viability: per-tick work scales with active cultivated instances, with bounded constant factors.

---

## Data and Schema Dependencies

This system assumes schemas exist (or will exist) for:
- Plant definition (requirements, tolerances, stage model, yield parameters)
- Soil/container definition (capacity, drainage, nutrient pools)
- Resource definitions (water, nutrient mixtures, contaminants)
- Process/action definitions (inputs, time cost, outputs, failure conditions)
- Event definitions (optional stressors)

---

## Non-Goals

Farming does not attempt to be a botanical encyclopedia or a full fluid/chemistry simulator. Where abstraction is used, it must be:
- explicit
- bounded
- documented in the relevant data definitions

---

## Files in this Folder

- `README.md` — system scope, interfaces, invariants
- `states.md` — authoritative list of farming state variables, ranges, and invariants
- `processes.md` — authoritative list of farming processes/actions, inputs/outputs, and failure modes
