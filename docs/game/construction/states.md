# design/systems/construction/states.md

## Purpose

This document defines the authoritative state variables for the Construction system.

States describe what can exist and what can be true at a given time.
States do not define behavior. Behavior belongs in `processes.md`.

All state fields must be:
- explicit
- bounded
- unit-consistent
- explainable

> **Grounding note (2026-06-30):** field names below track the real Rust types in
> `src/systems/construction/` (`mod.rs`, `structural.rs`, `routing.rs`, `solver.rs`) plus
> `src/ship/structure.rs`. Where the design calls for more fidelity than currently
> implemented, that gap is called out explicitly rather than implied as shipped.

---

## State Model Overview

Construction state is organized into:
- Blueprint definitions (what CAN be built)
- Construction state (a build in progress)
- Structure state (a completed built object)
- Structural/framing state (load-bearing graph, for integrity checks)
- Routing state (utility runs threaded through the structure)
- Derived indicators (computed for explanation, not authoritative)

---

## Blueprint Definition (data, not runtime state)

Loaded from RON via `Blueprint` (`src/systems/construction/mod.rs`):
- `id` (stable identifier)
- `name`
- `category`
- `materials` (list of `(item_id, quantity)` pairs, the bill of materials)
- `build_time` (seconds of accumulated construction progress required)
- `size` (`[f32; 3]`, world-space footprint)
- `snap_to` (list of surface/attachment tags this blueprint can snap onto)
- `health` (max structural health once completed)
- `provides` (optional capability tag, e.g. a room/utility this structure grants)

---

## Construction State (per active build, `Construction` component)

### Identity
- `blueprint_id` (definition reference)
- `builder_key` (optional, identity of the player/agent building it)

### Progress
- `progress` (0.0 – `build_time`, accumulates by `dt` per tick while active)
- Implicit completion: `progress >= build_time` converts `Construction` into `Structure`
  and removes the `Construction` component (see `processes.md::advance_construction`).

### Placement
- `position` (world Vec3, snapped to the 1m grid via `snap_to_grid`)
- `rotation` (currently always `Quat::IDENTITY`, free rotation not yet implemented)
- `scale` (from blueprint `size`)

---

## Structure State (per completed build, `Structure` component)

### Identity
- `blueprint_id` (definition reference)

### Health
- `health` (0.0 – `max_health`)
- `max_health` (set at completion from blueprint `health`)

### Capability
- `provides` (optional, carried over from blueprint; downstream systems read this to
  know what a structure grants, e.g. shelter, a utility hookup point)

### Not yet modeled (design intent, no runtime fields today)
- Wear/degradation over time (`construction.md` describes this; no tick-based decay
  exists yet for `Structure.health` outside of external damage events)
- Join-level state (bolted/welded/etc. per-component graph), the real system tracks
  one `health` per structure, not a fine-grained join graph
- Seal/containment state (leak risk, moisture), not represented

---

## Structural / Framing State (load-bearing analysis)

Used by `StructuralAnalyzer::analyze` (`structural.rs`), which delegates to the node-beam
solver in `solver.rs`:

- `FramingNode`, a point in the load-bearing graph (position + support/fixed flags)
- `FramingMember`, an edge between two nodes (the structural member itself, with
  material/strength properties from `data/materials.csv`)
- `StructuralVerdict` (enum, from `solver.rs`): `Stable`, `Unstable`, `Collapsed`
- `StructuralResult` (enum, `structural.rs`): mirrors `StructuralVerdict` 1:1, this is
  the public three-state verdict a build check reduces to.

This is a real, working solver, not aspirational, but it is invoked as an explicit
analysis step, not yet wired into every placement automatically (see `PRIORITIES.md`/
`STATUS.md` for current wiring status before assuming full automatic enforcement).

---

## Routing State (utility runs through a structure)

Used by `AutoRouter` (`routing.rs`), which turns a straight machine-to-machine connection
into a realistic orthogonal run:

- `RouteType` (enum): `Pipe`, `Wire`, `Ventilation`
- Per-run geometry: riser up, right-angle overhead run, riser down (the "up-over-down"
  pattern), with elbows at corners, fitting collars at machine ports, support brackets at
  code-derived spacing, and shutoff valves on fluid lines at destination inlets
- Rules loaded from `data/routing_rules.ron`; vertical lane assignment keeps power, water,
  and ventilation from overlapping in the same riser lane

Grounded in real code standards cited in the source: ASME A13.1 (pipe color), IPC 308.5 /
NEC 358.30 (support spacing), NEC 300.4 (electrical-above-water separation).

---

## Derived Indicators (non-authoritative)

- `structural_verdict`, computed on demand from the framing graph, not stored per-tick
- `build_percent`, `progress / build_time`, for UI display
- `missing_materials`, computed by diffing blueprint `materials` against current
  inventory at build-queue time

---

## Invariants (must always hold)

- `progress` must remain within `[0, build_time]`.
- `health` must remain within `[0, max_health]`.
- A `Construction` and a `Structure` component are mutually exclusive on the same entity
  at any given tick (the system removes one before inserting the other).
- Position is always grid-snapped to whole-meter coordinates at placement time.
- `StructuralVerdict`/`StructuralResult` values must be one of the three declared states;
  no partial/interpolated structural states exist.

---

## Notes on Abstraction

This state model is intentionally bounded:
- One scalar `health` represents a whole structure rather than per-join health
- The framing solver's `Stable`/`Unstable`/`Collapsed` three-state verdict stands in for
  full finite-element stress analysis
- Utility routing is a rule-derived geometric path with code-grounded decoration, not a
  full fluid/electrical simulation

All abstraction must remain explicit and explainable, matching `construction.md`'s
top-level design law.
