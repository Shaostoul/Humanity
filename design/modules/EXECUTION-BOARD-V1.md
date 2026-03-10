# Execution Board V1 (Spec -> Crate -> Tests -> Scenario)

This board tracks conversion from module specs into runnable Rust modules.

Status key: `planned` | `in_progress` | `blocked` | `done`

---

## Workstream A: Core engines

## A1) core-lifeform-model
- Spec: `design/modules/core-lifeform-model.md`
- Crate path: `crates/core-lifeform-model/`
- Current status: planned
- Next actions:
  1. create crate skeleton + lib.rs trait interfaces
  2. implement `LifeformState` and `SpeciesProfile`
  3. add deterministic tick tests
  4. add example scenario fixture
- Depends on: none (foundation)

## A2) core-skill-progression
- Spec: (to be created)
- Crate path: `crates/core-skill-progression/`
- Current status: planned
- Next actions:
  1. author spec
  2. scaffold crate
  3. implement XP/mastery interface
  4. add progression regression tests
- Depends on: A1 interfaces for capability ties

## A3) core-teaching-graph
- Spec: (to be created)
- Crate path: `crates/core-teaching-graph/`
- Current status: planned
- Next actions:
  1. author spec
  2. define competency DAG types
  3. add prerequisite validation tests
  4. add lesson recommendation stub
- Depends on: A1, A2

---

## Workstream B: Homestead essentials

## B1) module-water-systems
- Spec: `design/modules/module-water-systems.md`
- Crate path: `crates/module-water-systems/`
- Current status: planned
- Next actions:
  1. scaffold crate
  2. implement water quality state + treatment functions
  3. add contamination risk tests
  4. create drought/contamination scenario
- Depends on: core units/time/materials

## B2) module-soil-ecology
- Spec: `design/modules/module-soil-ecology.md`
- Crate path: `crates/module-soil-ecology/`
- Current status: planned
- Next actions:
  1. scaffold crate
  2. implement soil cell/profile and seasonal updates
  3. add erosion/regeneration tests
  4. create degraded-field recovery scenario
- Depends on: core units/time/weather

## B3) module-crop-systems
- Spec: `design/modules/module-crop-systems.md`
- Crate path: `crates/module-crop-systems/`
- Current status: planned
- Next actions:
  1. scaffold crate
  2. implement growth stage transitions
  3. add stress/yield regression tests
  4. create 3-plot rotation scenario
- Depends on: B1, B2, core time/weather

---

## Workstream C: Trade essentials

## C1) module-carpentry
- Spec: `design/modules/module-carpentry.md`
- Crate path: `crates/module-carpentry/`
- Current status: planned
- Next actions:
  1. scaffold crate
  2. implement task + tolerance model
  3. add defect distribution tests
  4. create frame-wall scenario
- Depends on: core materials, skill progression

## C2) module-electrical-basics
- Spec: `design/modules/module-electrical-basics.md`
- Crate path: `crates/module-electrical-basics/`
- Current status: planned
- Next actions:
  1. scaffold crate
  2. implement circuit graph + fault logic
  3. add overload/protection tests
  4. create microgrid scenario
- Depends on: core units/time

## C3) module-plumbing-basics
- Spec: `design/modules/module-plumbing-basics.md`
- Crate path: `crates/module-plumbing-basics/`
- Current status: planned
- Next actions:
  1. scaffold crate
  2. implement network flow + leak logic
  3. add flow/contamination tests
  4. create off-grid loop scenario
- Depends on: B1, core materials

---

## Workstream D: Health and safety

## D1) module-health-first-aid
- Spec: `design/modules/module-health-first-aid.md`
- Crate path: `crates/module-health-first-aid/`
- Current status: planned
- Next actions:
  1. scaffold crate
  2. implement triage state transitions
  3. add intervention timing tests
  4. create workshop incident scenario
- Depends on: A1 lifeform core

---

## Global implementation order (recommended)

1. A1 core-lifeform-model
2. B1 water + B2 soil
3. B3 crops
4. C1 carpentry
5. C2 electrical
6. C3 plumbing
7. D1 health-first-aid
8. A2 skill progression + A3 teaching graph

## Acceptance gates per module

A module can move to `done` only if all are true:

- Spec exists and is current
- Crate compiles in workspace
- Unit tests pass
- At least one scenario test passes
- README explains usage in plain language
- Linkage back to design spec is present
