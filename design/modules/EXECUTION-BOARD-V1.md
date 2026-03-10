# Execution Board V1 (Spec -> Crate -> Tests -> Scenario)

This board tracks conversion from module specs into runnable Rust modules.

Status key: `planned` | `in_progress` | `blocked` | `done`

---

## Workstream A: Core engines

## A1) core-lifeform-model
- Spec: `design/modules/core-lifeform-model.md`
- Crate path: `crates/core-lifeform-model/`
- Current status: done
- Progress:
  - [x] crate skeleton + `lib.rs` trait interfaces
  - [x] implemented `LifeformState` and `SpeciesProfile`
  - [x] deterministic tick tests passing (`cargo test -p core-lifeform-model`)
  - [x] example scenario fixture (`examples/human_livestock_crop_stress.rs`)
- Depends on: none (foundation)

## A2) core-skill-progression
- Spec: `design/modules/core-skill-progression.md`
- Crate path: `crates/core-skill-progression/`
- Current status: done
- Progress:
  - [x] authored spec
  - [x] scaffolded crate
  - [x] implemented XP/mastery interface
  - [x] progression regression tests passing (`cargo test -p core-skill-progression`)
  - [x] scenario fixture (`examples/progression_walkthrough.rs`)
- Depends on: A1 interfaces for capability ties

## A3) core-teaching-graph
- Spec: `design/modules/core-teaching-graph.md`
- Crate path: `crates/core-teaching-graph/`
- Current status: done
- Progress:
  - [x] authored spec
  - [x] defined competency DAG types
  - [x] prerequisite/cycle validation tests passing (`cargo test -p core-teaching-graph`)
  - [x] recommendation implementation + fixture (`examples/competency_recommendations.rs`)
- Depends on: A1, A2

---

## Workstream B: Homestead essentials

## B1) module-water-systems
- Spec: `design/modules/module-water-systems.md`
- Crate path: `crates/module-water-systems/`
- Current status: done
- Progress:
  - [x] scaffolded crate
  - [x] implemented water quality state + treatment/routing functions
  - [x] contamination/shortage tests passing (`cargo test -p module-water-systems`)
  - [x] drought/contamination scenario (`examples/drought_contamination.rs`)
- Depends on: core units/time/materials

## B2) module-soil-ecology
- Spec: `design/modules/module-soil-ecology.md`
- Crate path: `crates/module-soil-ecology/`
- Current status: done
- Progress:
  - [x] scaffolded crate
  - [x] implemented soil cell/profile + seasonal updates
  - [x] erosion/regeneration tests passing (`cargo test -p module-soil-ecology`)
  - [x] degraded-field recovery scenario (`examples/degraded_field_recovery.rs`)
- Depends on: core units/time/weather

## B3) module-crop-systems
- Spec: `design/modules/module-crop-systems.md`
- Crate path: `crates/module-crop-systems/`
- Current status: done
- Progress:
  - [x] scaffolded crate
  - [x] implemented growth stage transitions and interventions
  - [x] stress/yield regression tests passing (`cargo test -p module-crop-systems`)
  - [x] three-plot rotation scenario (`examples/three_plot_rotation.rs`)
- Depends on: B1, B2, core time/weather

---

## Workstream C: Trade essentials

## C1) module-carpentry
- Spec: `design/modules/module-carpentry.md`
- Crate path: `crates/module-carpentry/`
- Current status: done
- Progress:
  - [x] scaffolded crate
  - [x] implemented task + tolerance/quality model
  - [x] quality/defect tests passing (`cargo test -p module-carpentry`)
  - [x] frame-wall scenario (`examples/frame_wall.rs`)
- Depends on: core materials, skill progression

## C2) module-electrical-basics
- Spec: `design/modules/module-electrical-basics.md`
- Crate path: `crates/module-electrical-basics/`
- Current status: done
- Progress:
  - [x] scaffolded crate
  - [x] implemented circuit graph + fault logic
  - [x] overload/protection tests passing (`cargo test -p module-electrical-basics`)
  - [x] microgrid scenario (`examples/microgrid.rs`)
- Depends on: core units/time

## C3) module-plumbing-basics
- Spec: `design/modules/module-plumbing-basics.md`
- Crate path: `crates/module-plumbing-basics/`
- Current status: done
- Progress:
  - [x] scaffolded crate
  - [x] implemented network flow + leak logic
  - [x] flow/contamination tests passing (`cargo test -p module-plumbing-basics`)
  - [x] off-grid loop scenario (`examples/off_grid_loop.rs`)
- Depends on: B1, core materials

---

## Workstream D: Health and safety

## D1) module-health-first-aid
- Spec: `design/modules/module-health-first-aid.md`
- Crate path: `crates/module-health-first-aid/`
- Current status: done
- Progress:
  - [x] scaffolded crate
  - [x] implemented triage + intervention state transitions
  - [x] intervention timing/quality tests passing (`cargo test -p module-health-first-aid`)
  - [x] workshop incident scenario (`examples/workshop_incident.rs`)
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
