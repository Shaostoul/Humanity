# design/systems/construction/README.md

## Purpose

Construction models the creation, modification, and maintenance of structures and built
objects under constraint: what can be built from available materials, tools, time, and
energy; what fails, why; and how built form affects safety, comfort, capability, and
resource flow. See `../construction.md` for the full top-level design law this system
implements.

---

## Scope

### Included
- Blueprint-driven assembly: foundations, frames, walls, floors, roofs, openings
- Utility routing as physical installation (conduit, pipes, wiring) with real
  code-grounded geometry (up-over-down risers, elbows, support spacing)
- Structural integrity analysis via a node-beam load-bearing solver
- Grid-snapped placement of buildable structures

### Excluded
- Extraction/refinement of raw materials (materials/industry system)
- Full CFD/thermal simulation (environment/HVAC system)
- Social/legal permitting regimes (governance system, if modeled)
- Detailed combat damage mechanics beyond a declared "damage events" interface

---

## Interfaces

Construction consumes:
- Blueprint definitions (`Blueprint` in `src/systems/construction/mod.rs`): id, name,
  category, bill-of-materials, build_time, size, snap_to tags, health, provides
- Material strength properties (`data/materials.csv`) for structural analysis
- Routing rules (`data/routing_rules.ron`) for utility-run lane assignment

Construction produces:
- `Construction` component state (a build in progress: progress, build_time, builder_key)
- `Structure` component state (a completed build: health, max_health, provides)
- Structural verdicts (`Stable` / `Unstable` / `Collapsed`) from the framing solver
- Routed utility geometry (pipe/wire/ventilation segments) for the renderer

---

## Primary System Invariants

- Determinism: identical plans + actions + inputs produce identical outcomes.
- Conservation: materials consumed/wasted/salvaged must balance (design law; current
  implementation gap: material consumption is not yet deducted at build time, see
  `processes.md`).
- No free upgrades: improvements require added material/time/energy.
- Explainability: failures map to load exceedance, join weakness, seal failure, or
  degradation state (design law; several failure paths are not yet wired, see
  `processes.md`'s Non-Goals section for the honest current list).

---

## Data and Schema Dependencies

This system depends on:
- Blueprint RON definitions (e.g. `data/blueprints/home_structure.ron`,
  `data/blueprints/materials.ron`, `data/blueprints/wall_materials.ron`)
- `data/materials.csv` (structural strength properties)
- `data/routing_rules.ron` (utility lane rules)

---

## Non-Goals

Construction does not attempt full finite-element structural analysis or a full
fluid/electrical simulation. Where abstraction is used (a three-state structural verdict,
rule-derived utility routing instead of continuous physics), it is explicit and bounded,
matching `../construction.md`'s design law.

---

## Files in this Folder

- `README.md`, system scope, interfaces, invariants (this file)
- `states.md`, authoritative list of construction state variables, ranges, and invariants
- `processes.md`, authoritative list of construction processes/actions, inputs/outputs,
  and failure modes
