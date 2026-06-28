# Sim-Realism Roadmap — master gap-analysis + build order

> **Status:** synthesis of 7 per-domain audits + the utility-wiring design + the
> whole-systems sweep (2026-06-28). This is the canonical "what's fake, what's
> next, in what order" reference for making HumanityOS a believable real-life sim.
> Read this before opening any simulation-system or build-editor work.
>
> **Root cause, stated once:** the codebase is not missing code. It has ~40
> `impl System` types but only ~12 tick (verified against
> `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS` + the `system_runner.register(...)`
> calls in `src/lib.rs:4025-4150`). The dominant failure mode across EVERY domain is
> the same triad: **(a) no entity spawner, (b) no consequence coupling into
> Vitals/EnvironmentContext/grid, (c) rich `.ron`/`.csv` data parsed as opaque
> `ron::Value` and never read.** "Teleporting resources" (power with no wire, water
> with no pipe, instant free builds) is this triad showing through. The fix is a
> wiring + spawning + data-consumption discipline, not a rewrite.

---

## 1. TOP 20 cross-domain gaps, ranked by leverage

Ranked by believability/foundation impact: the higher an item, the more downstream
systems it unblocks or the more glaring the "magic" it removes. Effort: S (≤1
release), M (2-3), L (multi-release arc).

| # | Gap (one line) | Domain | Effort |
|---|---|---|---|
| 1 | **No entity-spawner + consequence-coupling primitive.** ~28 systems tick on entities nothing spawns and outputs nothing reads. Build one "fully-simulated room" spawner (RoomEnvironment + WaterFixture + WasteSource + Needs + Flammable) and the consequence bus that feeds Vitals/EnvironmentContext — this converts ~6 deferred systems to live at once. | cross-cutting | L |
| 2 | **Resource flow ignores the graph (power + water teleport).** `ElectricalSystem` sums ALL generators vs ALL consumers globally; `PlumbingSystem` draws from nearest tank within 12 m by straight-line distance. Neither reads `connections`/`conduit_edges`. Gate both on per-island graph reachability (union-find / BFS over kind-matched edges). Single biggest "magic" kill. | electrical + water | M |
| 3 | **Maintenance/wear/repair is wholly ABSENT** despite `items.csv` carrying a `durability` column on every item (explicitly *ignored* at load, `src/systems/inventory/mod.rs:255`) and the mission naming "infinite maintenance ahead." Nothing decrements durability, ages a machine, or needs repair. Makes every other built system consequential over time. | cross-cutting | M |
| 4 | **Needs/comfort never wired:** `PsychologySystem` (Maslow needs, real decay+morale) is deferred and tracks needs in a side `HashMap` because **there is no `Needs` ECS component** (confirmed absent in `components.rs`). Add the component, register the system. The felt heartbeat of a life sim; tiny wiring. | people | S |
| 5 | **Builds are instant + free.** Walls, decks, elevators, roads consume zero materials/energy/time/labor; `cost_per_kg` is a label. Deduct inventory + charge build-time via the existing (unregistered) ECS `ConstructionSystem` build-over-time path. Enforces conservation, the design doc's hard constraint. | construction | M |
| 6 | **Structural solver exists but is never run on real geometry.** `src/systems/construction/solver.rs` (node-beam, capacity, cascade, island flood-fill) is a tested dead spike. Add `generate_framing()` + ground anchors, run on `rebuild_homestead`, surface the red/green overlay. A paper-thin wall, a 100 m unsupported steel span, a floating room all stand forever. | construction | M |
| 7 | **Health has no input chain:** `MedicalSystem` ticks but nothing *creates* conditions — fire/infection/injury/bad-water/sanitation never call `apply_condition`. Wire waste→`unsanitary`→illness and bad-water→illness. Sanitation+health is a mission pillar. | people | M |
| 8 | **Water/waste loops are asserted, not simulated.** `home.ron` claims a closed 240 L/day + 155 L/day greywater loop; in reality `PlumbingSystem`/`HydrologySystem`/`WasteSystem` are all deferred, no entities spawn, tanks never refill, greywater is a number. Spawn entities, wire rain/well refill + greywater recycle. | water + waste | M |
| 9 | **Solar ignores weather + place; wind is fully decoupled.** `SolarSystem` reads only clock hour (sine 06h-18h) — identical noon power in a blizzard, in winter, at the pole, on Mars. `Weather.wind_speed` (simulated 0-25 m/s) drives no turbine. Multiply by cloud/season/latitude derate; add a `WindSystem`. | electrical | S |
| 10 | **Crafting is location-free magic:** `required_station` only filters the GUI menu — no proximity check, so you smelt titanium anywhere with no smelter. Enforce station proximity in `CraftingSystem`. Makes every `build_smelter`/`build_forge` recipe meaningful. | crafting | S |
| 11 | **No declared connection PORTS.** Machines infer hookups from `stats` strings; no IN/OUT direction, no per-port anchor, no rating. Add `ports: Vec<Port>` to `MachineDef` with `#[serde(default)]` + `derive_ports()` fallback. Foundation for all wiring realism (see §2). | wiring | M |
| 12 | **Plants grow with zero light + inert climate.** No photosynthesis input; DLI/PPFD in `tomato.ron` unused; temp/humidity/pH loaded, shown, never applied; season computed then discarded (`let _growth_mult`). A tomato grows full-speed in a dark sealed basement at -40 C in winter. | farming | M |
| 13 | **Crafting breaks conservation of mass + emits no waste.** Outputs can outweigh inputs (`make_plastic` 1→3; `vulcanize_rubber` net gain); `byproducts`/`failure_chance`/`power_required_watts` in `schemas/recipe.toml` are dropped by the parser. Add a mass-balance lint + emit slag/scrap into `WasteSystem`. | crafting | M |
| 14 | **Atmosphere/HVAC never tick + draw no power.** Both deferred; no `RoomEnvironment`/`EnclosedSpace` spawn; `power_kw` never reaches the grid; breach never flips `sealed=false` so the decompression path the atmosphere sim already has never fires. | air-hvac | M |
| 15 | **Conduit ratings are cosmetic.** `electrical.ron` carries `resistance_ohm_per_m`, `capacity_watts`, voltages; `plumbing.ron` carries diameter/pressure — ALL parsed as opaque `ron::Value`, zero code reads them. No voltage drop, no I²R loss, no breaker trip, no pipe flow cap. Read the data that already exists. | electrical + water | M |
| 16 | **Battery is a perfect lossless bucket** that doesn't even prevent load-shed (its own code comment concedes this), 100% round-trip, no DoD floor/degradation, and SoC resets to 50% on world entry (`lib.rs:219`) — no persistence. Let it carry load before shedding; persist `charge_wh`. | electrical | S |
| 17 | **No user-to-user block/mute/report.** Only admin ban/mute exist; no personal block, no abuse-report pipeline, no message-request inbox for stranger first-contact. Biggest social-safety gap before scaling. | chat-social | M |
| 18 | **NPCs have no schedules.** `npcs.ron` has no sleep/shift/mealtime fields; no day/night routine sim. Turns a static settlement into a living one; high immersion-per-effort; makes lighting/comfort/economy legible. | people | M |
| 19 | **No dynamic light-source component + lighting isn't a need.** No `PointLight`/`LightSource` in `src/`; lamps are build-editor toggles, not a simulated need drawing power over night hours. Cheap immersion; ties to the already-live electrical load. | environment | S |
| 20 | **In-world comms is absent / messages teleport.** No antenna/relay/range/line-of-sight; chat rides a magic always-on WebSocket; LoRa exists but is feature-gated and not in the normal path; light-speed delay (critical for the solar-system setting) is never applied. | internet-comms | L |

**Honorable mentions (21+):** spoiled food still gives full nutrition (`food.rs:526` TODO); nutrition is calories-only despite full vitamin/mineral data; generator `fuel_per_second` is a hardcoded 0.0 stub; hot/cold water never split (no water heater = a top-2 missing home energy demand); off-screen farm growth undefined (`FarmAutomation` empty stub); openings are structurally weightless (a 4 m window in a load-bearing wall is free).

---

## 2. The wiring system — verdict + first increment

**Verdict: sound, and unusually well-targeted — adopt it as specified.** The design
extends the real skeleton (`MachineDef`, `MachineConnection`, the `ConduitNode`/
`ConduitEdge` graph, `route_conduit`, `buildability_report`) instead of replacing
it, so the home-design parity guarantee (editor + AI edit the same
`data/machines/home.ron`) and the byte-identical round-trip tests survive. The two
self-resolved forks are both correct: **`Utility` as a closed enum** is right —
each utility has *distinct physics* (ampacity+voltage-drop vs flow+pressure), so a
new utility genuinely is a code decision; infinite-of-X correctly lives in the
conduit *catalog* (`conduits.ron`) and in machines/ports/connections (all data).
**Keeping `MachinePower` AND adding `ports`** with a `derive_ports()` bridge is the
only migration that doesn't break the live `ElectricalSystem` + `buildability_report`
or the round-trip tests. The `#[serde(default)]` discipline on every new field means
every existing `home.ron` entry parses unchanged — non-negotiable and correctly
specified. The one caveat: the design's own validation (ampacity/voltage-drop) is
*design-time* via `buildability_report`; it does NOT by itself fix gap #2 (runtime
power/water teleporting through the global sum) — that runtime graph-gating is Stage 3
+ a separate `ElectricalSystem`/`PlumbingSystem` change. Land them in that order so
the editor teaches correct wiring before the runtime starts enforcing it.

**Single best FIRST increment — exactly the design's Stage 1, scoped to the minimum
that adds real ports + a copper power cable with a rating check:**

1. New `src/utilities.rs` (pure serde/data, NO `#[cfg(native)]` gate so it compiles
   under `relay` — verify with `cargo check --features relay --no-default-features`):
   `Utility`, `PortDir`, `Port`, `ElecSpec`, `ConduitType`, `ConductorMaterial`,
   `Grade`, the `OnceLock`+`include_str!` loader (clone `src/ship/structure.rs:86`),
   and `check_cable()` + `awg_to_mm2()` with unit tests (mirror the
   `electrical.rs::integrate_battery` test style).
2. New `data/utilities/conduits.ron` — copper rows only (`cu_awg14/12/10`,
   `cu_awg6_ind`) plus the `sc_room_temp` superconductor row as the upgrade target.
3. `MachineDef`: add `#[serde(default)] ports: Vec<Port>` + `derive_ports()`.
   `MachineConnection`: add `#[serde(default)] spec: Option<ConduitSpec>`.
4. `MachineHome::buildability_report()` (`machines.rs:499`): add a "Conduits" check
   group that, per connection, auto-picks the cheapest copper type, measures routed
   length via `route_conduit().points`, sums the destination port load, and pushes a
   Pass/Warn/Fail `BuildabilityCheck` — appearing right next to "Energy balance."
5. Add explicit `ports` to exactly TWO seed machines in `home.ron` (one
   `solar_panel`: Electricity Out; one `water_pump`: Electricity In) to prove the
   round-trip; everything else auto-derives.
6. Tests (run via `cargo test --features native --lib`, the Windows-PDB-safe path):
   extend `save_round_trips_the_home_layout`; an under-spec cable Fails; an adequate
   cable Passes; auto-pick returns cheapest-that-fits.

That is the smallest slice that ships a real declared port, a real copper cable
referencing real NEC ampacity, and a real rating check the editor + an AI both see —
with zero risk to existing data or tests. Defer per-port render anchors, the
gauge-picker UI, voltage-drop, pipe flow, and the superconductor unlock to Stages
2-4 as the design lays out.

---

## 3. Foundational order — build these first (everything depends on them)

These are the shared primitives the rest of the roadmap stands on. Build in this
sequence; each unblocks several Top-20 items.

1. **Entity-spawner + consequence-coupling harness (gap #1).** A function that
   spawns a "fully-simulated room" — `RoomEnvironment`, `WaterFixture`,
   `WasteSource`, `Needs`, `Flammable` — hooked into `load_world`/save_load, plus
   the bus that feeds those system outputs into `Vitals`/`EnvironmentContext`.
   *Unblocks #4, #7, #8, #14, #18, #19.* This is the master key.
2. **The `Needs` ECS component (gap #4).** Tiny, but `PsychologySystem` is blocked
   on it and it's part of every spawned agent. Add it inside primitive #1.
3. **The generic network-graph + per-island flow solver (gap #2, #15).** Union-find
   the kind-matched `conduit_edges` into components; balance supply/demand *within*
   each island; read `resistance`/`capacity`/diameter from the existing `.ron`. One
   engine reused by `ElectricalSystem` AND `PlumbingSystem` (kills two teleport bugs
   with one substrate). *Unblocks #8, #11's runtime half, #16, #20's bandwidth model.*
4. **Declared ports on `MachineDef` (gap #11).** The wiring design's Stage 1 (see
   §2). The graph solver in #3 terminates at ports, so land this alongside it.
5. **Conservation/cost enforcement primitive (gap #5, #13).** A single "this build/
   craft consumes X and emits Y" path: inventory deduction + energy/time charge +
   waste emission + mass-balance check. Reuse the unregistered ECS
   `ConstructionSystem` build-over-time path and feed `WasteSystem`. *Unblocks #5,
   #6's BOM, #13.*
6. **The maintenance/wear tick (gap #3).** A `DurabilitySystem` that decrements the
   already-present `durability` column by use/age/environment and emits repair
   tasks. Depends on #1 (entities to age) and gives consequence to everything #5
   builds. *Mission-named; makes the whole sim consequential over time.*
7. **Weather→consumer coupling (gap #9, #12 partial).** Weather already ticks; wire
   its output into solar derate, body-temp/EnvironmentContext, farming suitability,
   and hydrology. Pure "connect the existing generator's output," no new generator.
8. **Time/scheduling backbone (gap #18 prerequisite).** The clock ticks; add the
   recurring-event/routine scheduler so NPC schedules, duty cycles, and calendar→
   world events have a spine. *Unblocks #18 and every duty-cycle/demand-coupling ask.*

Rationale for the order: 1-2 create the *things* to simulate; 3-4 make resources
*flow correctly between* them; 5-6 make creating/operating them *cost* something and
*decay*; 7-8 make the *environment and time* act on them. Skipping ahead (e.g.
building NPC schedules before the spawner+scheduler exist, or voltage-drop before
the graph solver) produces throwaway work.

---

## 4. Docs to create / update

**Create:**
- `docs/design/utility-wiring.md` — the canonical wiring design (§2 here is the
  summary; the full design text is the source). Required by the "read the design
  doc before touching wiring code" rule. Write it alongside Stage 1.
- `docs/design/self-sufficiency.md` — already exists; cross-link it from this doc and
  from `utility-wiring.md` (the water/energy loops are the consumer of the graph
  solver).
- This file (`docs/design/sim-realism-roadmap.md`) — created.

**Update:**
- `docs/FEATURES.md` — currently silent on `wear/durability/maintenance` and on the
  wiring port model (grep returns nothing). Add: (a) a "Utility wiring / ports" row
  when Stage 1 ships; (b) honest status rows for the ~28 deferred systems marking
  them *scaffold, not ticking* (today the doc partially overstates the homestead
  coupled loops as working — the audit found `home.ron` *asserts* loops that don't
  run). Keep it the never-rebuild-what-exists registry it's meant to be.
- `docs/ROADMAP.md` (+ regenerate `data/roadmap.json` via
  `scripts/roadmap-to-json.js`) — the "Survival and self-sufficiency" and "The
  simulation" sections claim coupled water/waste/energy loops; mark them *designed,
  wiring pending* and add the §3 foundational order as the public build sequence. Add
  Maintenance/Wear and Utility-Wiring as named roadmap items (both are mission-named
  and currently invisible on the public roadmap).
- `docs/design/infinite-of-x.md` — add the note that conduit types are now a registry
  (`data/utilities/conduits.ron`) and that `Utility` is a deliberately-closed enum
  with the documented physics justification, so the next author doesn't "fix" it into
  a data file.
- `docs/design/conduits-node-graph.md` — exists (Stage 1-3 of the node graph); add a
  forward-pointer to `utility-wiring.md` for the port/rating layer that sits on top.
- `docs/PRIORITIES.md` + `data/coordination/orchestrator_state.json` — set TIER 0 to
  the §3 foundational order (start: entity-spawner harness + `Needs` component +
  wiring Stage 1), with the WHY (the wiring-triad root cause) in the journal.

**Lint/test debt to track as docs update:** add `tests/recipe_lint.rs` (the
referenced-but-nonexistent `recipe_skill_lint`) and a mass-balance warn-lint; both
belong in the crafting increment (gap #13) and should be noted in `docs/SOP.md`'s
pre-push checklist once they exist.

---

### Cross-reference index (domain → its Top-20 items)
- **Construction:** #5, #6 (+ openings, foundation, framing in honorable mentions)
- **Electrical:** #2, #9, #15, #16 (+ fuel stub, hot-water demand)
- **Water/plumbing:** #2, #8, #15 (+ hot/cold split, pressure)
- **Air/HVAC/atmosphere:** #14
- **Internet/comms:** #20
- **Crafting:** #10, #13
- **Farming/food:** #12 (+ spoilage, nutrition, off-screen growth)
- **Chat-social:** #17
- **Wiring (connection ports):** #11
- **People (whole-systems):** #4, #7, #18
- **Cross-cutting / environment:** #1, #3, #19
