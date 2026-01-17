# system_inventory.md

## Purpose

This document lists candidate systems expected to exist over time.

This is a **non-binding catalog**:
- it does not define behavior
- it does not grant authority
- it does not replace system specifications in `systems/`

Its purpose is navigational:
- to prevent scope confusion
- to expose missing system coverage
- to help contributors choose bounded work

All systems listed here must comply with:
- `design/accord_constraints.md`
- `design/simulation_laws.md`
- `design/realism_constraints.md`

---

## System Catalog

### Farming
Scope: plant growth, soil, water, nutrients, pests, harvest, preservation inputs.  
Interfaces: ecology, economy, storage, health, education.

### Construction
Scope: assembly of structures/tools/machines from materials under load and wear constraints.  
Interfaces: resources, energy, transport, storage, economy.

### Health
Scope: injury, illness, fatigue, recovery, nutrition effects, medical interventions.  
Interfaces: farming, economy, education, population.

### Energy
Scope: generation, storage, conversion, distribution, efficiency, maintenance.  
Interfaces: construction, transport, storage, ecology.

### Storage
Scope: capacity, spoilage, preservation conditions, loss, contamination.  
Interfaces: farming, health, economy, transport.

### Transport
Scope: movement costs (time/energy), capacity, routing, wear, logistics.  
Interfaces: economy, construction, energy, storage.

### Ecology
Scope: environment states, cycles, biodiversity effects, regeneration/depletion.  
Interfaces: farming, health, resources, long-term viability.

### Population
Scope: demographics, roles, skill distribution, care load, social stability indicators.  
Interfaces: governance, education, economy, health.

### Governance
Scope: decision processes, delegation, accountability mechanics as modeled constraints.  
Interfaces: conflict resolution, economy, population.

### Conflict Resolution
Scope: non-violent resolution pathways, mediation, separation, containment thresholds.  
Interfaces: governance, population, harm minimization.

### Education
Scope: competency development, assessment, apprenticeship structures, explanation rules.  
Interfaces: all systems (education is cross-cutting).

---

## Candidate Systems (Later)

These are likely needed but should remain bounded and justified:

- Water (treatment, distribution, contamination)
- Waste (sanitation, recycling loops, hazards)
- Materials (extraction/processing/refinement under constraint)
- Manufacturing (process chains, tooling, quality)
- Communications (information flow constraints, reliability)
- Safety (hazard modeling, prevention, incident response)
- Climate/Atmosphere (local environmental modeling where relevant)

---

## How to Add a System

Add a system here only when:
- the domain cannot be cleanly represented as a submodule of an existing system
- it requires distinct rules, constraints, and failure modes
- it has clear interfaces to other systems

Then create the authoritative specification in:
- `design/systems/<system>.md`

---

## Closing Note

Systems are the engines of consequence.

This inventory exists to keep consequence organized and bounded.
