# design/systems/construction/processes.md

## Purpose

This document defines the authoritative processes/actions of the Construction system.

Processes describe what can happen, what they require, what they change, and how they fail.

All processes must declare:
- inputs (resources, tools, time)
- preconditions
- outputs (state changes, produced items, byproducts)
- failure modes
- explanation obligations

> **Grounding note (2026-06-30):** processes below are named to track the real
> `ConstructionSystem::tick` implementation in `src/systems/construction/mod.rs`, plus the
> analysis/routing helpers in `structural.rs` and `routing.rs`. Where a process is design
> intent beyond what ships today, it is marked "not yet implemented."

---

## Process Classes

- Planning (choosing what and where to build)
- Building (converting materials + time into a structure)
- Analysis (checking structural/utility validity)
- Maintenance and Deconstruction (not yet implemented, see Non-Goals)

---

## Planning Processes

### 1) queue_build
Purpose: register an intent to build a blueprint at a location.

Implementation: `ConstructionSystem::queue_build(blueprint_id, position)`.

Inputs:
- `blueprint_id` (must resolve in `BlueprintRegistry`)
- target `position` (world Vec3)

Preconditions:
- blueprint exists in the registry (loaded from data at startup/hot-reload)

State changes:
- appends `(blueprint_id, position)` to the pending-builds queue; no world entity exists
  yet

Failure modes:
- unknown `blueprint_id`, the build is silently dropped on the next tick (see
  `advance_construction` below); this is a real current gap, not a design choice, a
  build queued against a bad id produces no user-visible error today

Explanation:
- none yet surfaced to the player; queuing is fire-and-forget in the current
  implementation

---

### 2) snap_to_grid
Purpose: normalize a requested placement position to the 1m build grid.

Implementation: `ConstructionSystem::snap_to_grid`, rounds each axis to the nearest
whole meter.

Inputs:
- raw world position

Outputs:
- grid-snapped position, used for the spawned entity's `Transform`

Failure modes:
- none (pure function, always succeeds)

---

## Building Processes

### 3) spawn_construction
Purpose: materialize a queued build as an in-progress `Construction` entity.

Implementation: the first half of `ConstructionSystem::tick`, drains pending builds,
resolves each blueprint, and spawns an entity with `Transform` + `Construction`.

Inputs:
- resolved `Blueprint` (materials list, build_time, size)
- snapped position

Preconditions:
- blueprint still present in the registry at spawn time

State changes:
- new entity with `Construction { blueprint_id, progress: 0.0, build_time, builder_key:
  None }` and a `Transform` scaled to the blueprint's `size`

Failure modes:
- blueprint missing from registry → build silently skipped (`continue`)

Explanation:
- none surfaced yet; this is a gap versus the top-level `construction.md` design law
  ("failures must be detectable... attributable")

Not yet implemented (design intent, no current code path):
- material consumption at spawn time, the `materials` bill-of-materials on `Blueprint`
  is not currently deducted from any inventory by `ConstructionSystem`; that
  reconciliation is a design gap, not a shipped feature

---

### 4) advance_construction
Purpose: accumulate build progress over time and complete builds that reach their
`build_time`.

Implementation: the second half of `ConstructionSystem::tick`, for every entity with a
`Construction` component, `progress += dt`; entities whose `progress >= build_time` are
queued for completion.

Inputs:
- `dt` (frame/tick delta time)

Preconditions:
- entity carries a `Construction` component

State changes:
- `Construction.progress` increases monotonically each tick

Failure modes:
- none modeled currently, construction cannot stall, fail, or be interrupted by missing
  labor/tools/materials in the current implementation (design intent in `construction.md`
  calls for tool/skill/material-driven failure; not yet wired)

---

### 5) complete_construction
Purpose: convert a finished `Construction` into a permanent `Structure`.

Implementation: for each entity whose progress reached `build_time`, removes the
`Construction` component and inserts a `Structure { blueprint_id, health, max_health,
provides }`, reading `health`/`provides` back from the blueprint (falling back to
`(100.0, None)` if the blueprint has since disappeared from the registry).

Inputs:
- completed `Construction` entity
- blueprint lookup (for `health`/`provides`)

State changes:
- entity transitions from build-in-progress to a completed, health-tracked `Structure`

Failure modes:
- blueprint missing at completion time → falls back to default health 100.0, no
  `provides` capability (silent degradation, not a hard failure)

Explanation:
- none surfaced yet

---

## Analysis Processes

### 6) analyze_structural_integrity
Purpose: validate a framing graph against load-bearing rules.

Implementation: `StructuralAnalyzer::analyze(nodes, members)` → `super::solver::solve(...)`
→ reduces `StructuralVerdict` to the public `StructuralResult` (`Stable` / `Unstable` /
`Collapsed`).

Inputs:
- `FramingNode` list (support points)
- `FramingMember` list (structural members, referencing `data/materials.csv` strength
  properties)

Outputs:
- one of `Stable`, `Unstable`, `Collapsed`

Preconditions:
- caller constructs the framing graph explicitly; this is not automatically invoked on
  every placement today (a real, working solver that is not yet universally wired in,
  confirm current call sites before assuming every build is checked)

Failure modes:
- `Unstable`/`Collapsed` verdicts are the explicit failure signal; the solver itself does
  not panic on malformed graphs (bounded by construction)

---

### 7) route_utility
Purpose: auto-route a pipe/wire/ventilation run between two machine ports through a
structure.

Implementation: `AutoRouter` (`routing.rs`) turns a straight point-to-point connection
into an orthogonal "up-over-down" run: riser at the source, overhead run along a
service-lane band, riser down at the destination, decorated with elbows, fitting collars,
code-spaced support brackets, and (for fluid lines) a shutoff valve at the destination
inlet.

Inputs:
- `RouteType` (`Pipe` | `Wire` | `Ventilation`)
- source and destination machine port positions
- rules from `data/routing_rules.ron` (lane assignment per route type, so power/water/
  ventilation don't overlap in the same riser lane)

Outputs:
- an ordered list of `PipePart`-style segments the renderer turns into meshes
  (`lib.rs::load_world`, native only)

Failure modes:
- not explicitly modeled as a failable process in code today; routing always produces a
  path (no "no valid route" rejection path currently exists, which is a gap versus
  `construction.md`'s "utilities fail via... poor routing" design law)

---

## Non-Goals / Not Yet Implemented

The following are described as design intent in `construction.md` but have no
corresponding runtime code as of 2026-06-30:
- Deconstruction/salvage (no reverse of `complete_construction` exists)
- Maintenance actions (inspect, tighten, reseal, replace, treat, reinforce), no
  degradation-over-time tick exists for `Structure.health` from environment/wear
- Join-level quality/defect modeling (skill-driven tolerance, seeded defect distribution)
- Seal/containment degradation curves

Treat this section as the authoritative list of what is genuinely missing so future work
doesn't have to re-derive it from source.
