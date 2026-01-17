# design/systems/farming/processes.md

## Purpose

This document defines the authoritative processes/actions of the Farming system.

Processes describe what can happen, what they require, what they change, and how they fail.

All processes must declare:
- inputs (resources, tools, time)
- preconditions
- outputs (state changes, produced items, byproducts)
- failure modes
- explanation obligations

---

## Process Classes

- Establishment (starting cultivation)
- Care and Management (maintaining growth conditions)
- Protection and Treatment (responding to stressors)
- Harvest and Cleanup (extracting outputs, resetting sites)

---

## Establishment Processes

### 1) prepare_plot
Purpose: make a plot ready for planting.

Inputs:
- time (labor)
- optional materials (mulch, compost, substrate amendments)

Preconditions:
- plot exists and is accessible

State changes:
- plot cover_state may change
- substrate nutrient pools may change (if amendments applied)
- substrate structure indices may change (bounded)

Failure modes:
- insufficient time
- missing required materials
- contamination detected (flag)

Explanation:
- must state what changed and why it matters (water retention, nutrients, protection)

---

### 2) plant_seed
Purpose: place seeds into a plot/substrate.

Inputs:
- seed items (quantity)
- time (labor)

Preconditions:
- plot has available capacity/density margin
- substrate water_l above minimum germination threshold OR irrigation planned within window

State changes:
- new plant instance created with stage = `seed`
- stage_progress initialized
- plot occupancy updated

Failure modes:
- no capacity
- substrate unsuitable (pH band, salinity, contamination) if modeled
- insufficient moisture within germination window (may create delayed failure)

Explanation:
- must identify limiting precondition if failure occurs

---

### 3) transplant
Purpose: move a plant to a new plot/container.

Inputs:
- time (labor)
- optional container/materials

Preconditions:
- source plant exists and is not `dead`
- destination plot has capacity

State changes:
- plant plot_id updated
- temporary transplant shock applied as stress increment

Failure modes:
- no capacity
- plant too fragile (definition threshold)
- temperature extremes at time of transplant increase shock beyond tolerance

Explanation:
- must state shock cause and recovery conditions

---

## Care and Management Processes

### 4) water
Purpose: add water to substrate.

Inputs:
- water resource (liters)
- time (labor) or energy (pumps) depending on method

Preconditions:
- water inventory available
- plot accessible

State changes:
- substrate water_l increases, clamped to capacity
- water_quality_flags may update based on water source properties

Failure modes:
- insufficient water
- contaminated water source triggers contamination flags

Explanation:
- must indicate whether watering resolved deficit or caused saturation risk

---

### 5) fertilize
Purpose: add nutrients to substrate.

Inputs:
- nutrient resource (composition with units)
- time (labor)

Preconditions:
- nutrient inventory available

State changes:
- nutrient pools increase based on composition
- salinity_index may increase (if modeled)
- contamination flags may propagate if fertilizer is contaminated

Failure modes:
- insufficient fertilizer
- toxicity risk if pools exceed tolerance bands

Explanation:
- must state which nutrient limitation was targeted and risk tradeoffs

---

### 6) manage_cover
Purpose: modify plot protection and microclimate modifiers.

Inputs:
- time (labor)
- optional materials (cover, mulch, shade cloth)

Preconditions:
- materials present if required

State changes:
- plot cover_state updated
- light_access_factor may change
- evapotranspiration modifiers may change (bounded)

Failure modes:
- missing materials
- increased humidity risk may elevate disease susceptibility (tracked as explanation)

Explanation:
- must state tradeoff (water retention vs disease risk, light reduction vs heat protection)

---

### 7) prune
Purpose: remove plant biomass to influence growth and health.

Inputs:
- time (labor)
- optional tool capability (cutting)

Preconditions:
- plant stage supports pruning
- plant health above minimum threshold

State changes:
- canopy_index reduced
- short-term stress may increase
- long-term yield_potential may increase or decrease depending on timing (definition-driven)

Failure modes:
- pruning at wrong stage causes yield loss
- insufficient tool capability causes damage_flags

Explanation:
- must state stage timing and predicted outcome

---

## Protection and Treatment Processes

### 8) inspect
Purpose: observe plant and substrate conditions.

Inputs:
- time (labor)
- optional sensors/tools

Preconditions:
- plot accessible

State changes:
- none canonical (inspection creates observations/events/logs)

Failure modes:
- none (except accessibility)

Explanation:
- must report current limiting factor(s) and stress trend if requested

---

### 9) treat_pests_or_disease
Purpose: reduce pest pressure or disease state progression.

Inputs:
- time (labor)
- treatment resource (biological, mechanical, chemical) with declared effects and risks

Preconditions:
- target condition exists (pressure or disease_state != none)
- treatment permitted under contamination policy

State changes:
- pest_pressure decreases or disease_state regresses (bounded, definition-driven)
- contamination flags may be added depending on treatment type
- health may decrease short-term due to treatment stress

Failure modes:
- missing treatment
- late intervention (condition beyond reversible stage)
- inappropriate treatment increases harm (health drop, contamination)

Explanation:
- must state the risk tradeoff and whether containment or cure was achieved

---

## Harvest and Cleanup Processes

### 10) harvest
Purpose: convert plant growth into outputs.

Inputs:
- time (labor)
- optional tool capability

Preconditions:
- plant harvest_ready = true OR harvest allowed early/late with penalties
- edibility_state not `unsafe` for edible harvest outputs

State changes:
- outputs created with quantity and quality fields
- plant state updates (e.g., to `senescent`, `maturing`, or regrowth stage for perennials)
- residue/byproducts created

Failure modes:
- harvesting too early yields low quantity/quality
- contamination flags may mark outputs as restricted/unsafe

Explanation:
- must report yield drivers: stress history, timing, limiting factors

---

### 11) remove_crop
Purpose: clear a plant from a plot.

Inputs:
- time (labor)

Preconditions:
- plant exists

State changes:
- plant marked removed or moved to compost stream
- plot occupancy freed
- substrate may gain residue nutrients (bounded)

Failure modes:
- none except access/time

Explanation:
- must state what was removed and what residue remains

---

### 12) compost_or_recycle_biomass
Purpose: convert residues into substrate amendments (bounded).

Inputs:
- biomass items
- time (labor)
- optional space/container

Preconditions:
- composting capability exists (may be a separate system later; here it is bounded)

State changes:
- creates amendment resource
- may reduce pathogen risk over time if modeled

Failure modes:
- contamination present prevents safe compost output
- insufficient time leads to incomplete compost (lower quality)

Explanation:
- must state safety status and time dependency

---

## System Update (per tick) Responsibilities

Even without explicit actions, the system updates:
- growth accumulation based on limiting factors
- water depletion via evapotranspiration/drainage
- nutrient uptake and deficiency/toxicity progression
- stress accumulation/decay
- stage progression and transition checks
- disease/pest progression if pressure present

All tick updates must remain deterministic and traceable.

---

## Global Process Constraints

- No process may create resources without consuming declared inputs and time.
- All changes must be explainable via logged factors.
- All randomness must be seeded and recorded as events.
- Safety and contamination must propagate to outputs.

---

## Verification Requirements (tests)

- Deterministic replay of action logs
- Conservation of water and nutrient pools
- Harvest yields respond monotonically to limiting factor improvements (within declared bands)
- Stress accumulation/decay bounded and explainable
- Contamination flags propagate correctly to outputs
