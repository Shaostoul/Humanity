# construction.md

## Purpose

Model the creation, modification, and maintenance of structures and built objects under constraint.

Construction exists to answer:
- What can be built from available materials, tools, time, and energy?
- What fails, why, and with what consequences?
- How does built form affect safety, comfort, capability, and resource flow?

Construction is a system of consequence, not a building toy.

---

## Scope

### Included
- Assembly of structures: foundations, frames, walls, floors, roofs, openings
- Functional installations: doors, windows, fixtures, work surfaces, storage
- Utility routing as physical installation (conduit, pipes, wiring) with constraints
- Structural integrity within declared abstraction level
- Wear, degradation, and maintenance requirements
- Deconstruction, salvage, and waste generation

### Explicitly Excluded
- Extraction/refinement of raw materials (materials/industry system)
- Full CFD/thermal simulation (environment/HVAC system)
- Social/legal permitting regimes (governance system, if modeled)
- Detailed combat damage mechanics beyond declared “damage events” interface

---

## Inputs (schemas)

### Definitions
- Material definitions: density, strength bands, corrosion/rot behavior, thermal properties, toxicity flags, embodied energy (optional)
- Component definitions: beams, panels, fasteners, sealants, pipes, cables
- Tool definitions: capability sets and precision limits
- Process definitions (recipes): required inputs, time, skill requirements, byproducts

### State
- Current built objects (structure graph)
- Connection topology (joins, fasteners, welds, seals)
- Utility networks (routes, capacities, failure points)
- Environment snapshot (temperature, moisture, pressure if applicable)
- Action log (place, cut, join, seal, route, inspect, repair)

### Resources
- Materials and components with units
- Energy (for powered tools, fabrication, lifting)
- Time/labor budget
- Workspace constraints (clearance, access, staging)

---

## Outputs

Construction produces:
- Updated structure graph and component states
- Updated resource inventories (consumption, waste, salvage)
- Safety and integrity signals (overload, leak, collapse risk)
- Utility network changes (new routes, capacities, constraints)
- Maintenance schedules and degradation states
- Exportable plans as representations (non-authoritative artifacts)

---

## Core Model

### 1) Objects and joins
A built object is a graph:
- Nodes: components (beam, panel, fastener, seal)
- Edges: joins (bolted, nailed, welded, bonded, friction-fit)

Join types declare:
- strength band
- failure mode
- inspection/maintenance needs

### 2) Structural integrity (bounded)
Integrity is validated at build time and over time against:
- dead load (self weight)
- live load (occupancy/use)
- environmental load (wind/snow equivalents if modeled)
- point loads from mounted equipment

Abstraction rule:
- Small projects can use simplified load bands.
- Large/critical structures require higher fidelity or conservative margins.

### 3) Seals and containment
Where containment matters (water/air/humidity), openings and penetrations require:
- a declared seal type
- install time
- test/inspection procedure
- degradation curve

### 4) Utilities as capacity networks
Wiring/piping/conduit are routed as paths with:
- diameter/gauge
- bend radius limits
- junction types
- capacity and loss model (bounded)

Utilities fail via:
- overload
- leakage
- corrosion
- mechanical damage
- poor installation (skill/quality)

### 5) Quality and skill
Quality is explicit and explainable:
- dimensional accuracy
- join quality
- seal quality
- finish quality

Skill affects:
- time cost
- defect probability (seeded and logged)
- achievable tolerances

No hidden “+10% strength” without a declared causal field.

### 6) Degradation and maintenance
Components degrade based on:
- material properties
- environment exposure
- load history
- maintenance compliance

Maintenance actions:
- inspect, tighten, reseal, replace, treat, reinforce

---

## Constraints

- Determinism: identical plans + actions + inputs produce identical outcomes.
- Conservation: materials consumed/wasted/salvaged must balance.
- No free upgrades: improvements require added material/time/energy.
- Explainability: failures map to load exceedance, join weakness, seal failure, or degradation state.
- Accessibility: the system must support simple assembly workflows without hiding consequence.

---

## Failure Modes

- Structural overload → deformation → progressive failure → collapse
- Poor joins → loosening/shear failure → localized collapse
- Seal failure → leaks → mold/corrosion → health/safety risk
- Utility overload → heat/fire risk (if modeled) or circuit failure
- Poor routing → kinked lines, short circuits, blocked access for repairs
- Neglected maintenance → accelerated degradation → catastrophic failure at stress events

Failures must:
- be detectable (signals)
- be attributable (cause chain)
- create real costs (repair time/material, downtime, risk)

---

## Interactions

- Materials/Industry: provides components and defines their properties
- Energy: powers tools, lifting, fabrication, utilities
- Storage: stores components; poor storage can degrade materials
- Environment/HVAC: construction affects insulation, airflow, moisture behavior
- Health/Safety: hazards, exposure, injury risks, shelter adequacy
- Economy: labor allocation, pricing, depreciation, salvage value
- Education: competency validated by correct builds and accurate causal explanations

---

## Abstractions

Allowed when explicit:
- Modular components representing assemblies (e.g., “wall panel”) so long as internal composition is declared in data.
- Simplified load categories instead of full finite-element analysis.
- Utility flow as capacity thresholds rather than full physics.

Forbidden:
- Geometry-only building where structural validity is implied by placement.
- Cosmetic assets changing mechanics.

---

## Verification Requirements (tests)

Minimum suite:
- Conservation across build/deconstruct cycles
- Deterministic replay of build actions
- Integrity checks fail when load bands exceeded
- Seal degradation produces leaks on schedule (given exposure)
- Utility capacities enforce overload failures
- Explanation traces reference declared joins/material properties

---
