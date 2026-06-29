# Utility wiring -- connection ports + cables/conduit with real specs

> **Status:** Stage 1 SHIPPED (v0.604): the data model + cable registry + physics, isolated + tested
> (`src/utilities.rs`, `data/utilities/conduits.ron`). Stages 2-4 below are the wiring-in plan. Read
> this before touching any cable / port / conduit code. Companion: `docs/design/sim-realism-roadmap.md`
> (gap #2, #11, #15), `docs/design/conduits-node-graph.md` (the routing graph this sits on top of).

## The rule

Power, water, air, and data do NOT magically transmit through the air. They travel through **cables**
and **plumbing** that have real limits -- volts, watts, amps, gauge (AWG), ampacity, shielded vs
unshielded. A machine declares physical IN/OUT **ports** by utility: a teleporter needs electricity;
an aeroponic tower needs water + electricity; a sink needs hot AND cold water. Cables are rated and a
run can be **under-spec** (it fails the buildability check / trips at runtime). We start with **copper**
grounded in real NEC ampacity; a late-game mission upgrades all copper to a **room-temperature
superconductor**. Home-grade vs industrial-grade is a real distinction (a home doesn't need a 55 A
shielded industrial feeder for a lamp).

## Data model (`src/utilities.rs`)

- **`Utility`** -- a deliberately CLOSED enum (Electricity, Water, HotWater, Air, Data, Fuel, Nutrient,
  Waste). Closed on purpose: each utility has *distinct physics* (electricity = ampacity + voltage
  drop; water = flow + pressure), so adding one is a real code decision. Infinite-of-X lives in the
  conduit *catalog*, not here.
- **`Port { utility, dir: In|Out|Bidirectional, label, watts, flow_lpm, anchor }`** -- a physical
  connection point on a machine, with a local `anchor` offset so it has a real SPOT on the body.
- **`ConduitType`** (registry rows) -- material (Copper/Aluminum/Superconductor), `awg`, `ampacity_a`,
  `voltage_max`, `ohm_per_m` (for voltage drop), `diameter_mm`/`flow_max_lpm` (fluids), `shielded`,
  `grade` (Home/Commercial/Industrial), `cost_per_m`.
- **Registry:** `data/utilities/conduits.ron` (infinite-of-X). Copper 14/12/10 AWG (home), 6 AWG
  (industrial, shielded), the `sc_room_temp` superconductor upgrade target, plus two water pipes
  (Stage 2). Real NEC ampacities + resistances.

## Physics (tested)

- `check_cable(cable, load_watts, volts, length_m) -> CableCheck { Pass | Warn | Fail }`: amps =
  watts/volts; round-trip voltage drop = I*R*2L; **Pass** if amps <= 80% ampacity (NEC continuous
  derate) AND drop < 3%, **Warn** to ampacity / 5% drop, **Fail** beyond (or over the voltage rating).
- `cheapest_cable_for(load, volts, length)` -- the auto-picker the buildability report uses (cheapest
  copper that Passes).
- `awg_to_mm2(awg)` -- standard geometric AWG -> cross-section (for display + future resistance).

## Staged plan

- **Stage 1 (v0.604, SHIPPED):** the module above -- pure data + physics + the registry, fully
  unit-tested, NOT yet wired into machines. Zero risk to existing data/behaviour.
- **Stage 2 (v0.605, SHIPPED):** `MachineDef` gained `#[serde(default)] ports: Vec<Port>` + a
  `derive_ports()` fallback (electrical ports inferred from `power`; fluid ports must be declared).
  `MachineConnection` gained `#[serde(default)] spec: Option<String>` (a pinned conduit id; None =
  auto-pick). `buildability_report()` gained a **"Conduits"** check: per power run, compute the load
  the cable serves + the run length (from the machines' world offsets), then validate the pinned cable
  or auto-pick the cheapest copper via `cheapest_cable_for()` -> Pass/Warn/Fail. The water pump +
  aeroponic tower carry explicit ports in `home.ron`; the editor's info cards show every port.
- **Stage 2b (v0.606, SHIPPED):** a **"Power circuit"** buildability check -- union-find over the
  power graph (connections + power conduit edges, traversing junction nodes); every electrical LOAD
  must share a component with real generation (a battery is storage, not a source). This is the
  design-time half of "no magic transmission". The seed `home.ron` was rewired from a symbolic diagram
  into a physically connected network (PV array + wind + generator -> battery bus -> loads).
- **Stage 3 (next):** runtime graph-gating -- `ElectricalSystem`/`PlumbingSystem` stop summing globally
  and flow only through kind-matched, in-spec components (sim-realism-roadmap gap #2). Needs the spawned
  power entities to carry their instance id so the tick can group them by the union-find islands above.
  This is where the rating actually *trips* under load, not just warns at design time.
- **Stage 4:** editor UX -- wire port A -> port B with a gizmo, pick the cable type, per-port render
  anchors, and the **superconductor upgrade mission** (swap every copper run for `sc_room_temp`).

## Why keep `MachinePower` AND add `ports`

`MachinePower` (the live `ElectricalSystem` role) stays; `ports` is additive with a `derive_ports()`
bridge. That's the only migration that doesn't break the live electrical sim, `buildability_report`,
or the byte-identical `home.ron` round-trip tests. Every new field is `#[serde(default)]` so every
existing data file parses unchanged -- non-negotiable.
