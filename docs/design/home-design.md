# Home Design: one home, one surface, real-world-valid

> **Goal.** Anything the AI designs home-wise must be inherently buildable + editable by a
> player, and every system (electrical, plumbing, structure, food) must be real-world-valid
> so the same design works on an Earth homestead, not just in the game. This doc is the
> north star + the staged plan. Status as of v0.518.x: the architecture below is the target;
> the build is staged (see "Staged plan").

## The problem (today)

A "home" is **three disconnected data models**, only one of which a player can edit:

| Layer | File | Captures | Player-editable? | AI-editable? |
|-------|------|----------|------------------|--------------|
| **Rooms** | `data/blueprints/homestead_layout.ron` (`RoomConfig`, `src/ship/fibonacci.rs`) | geometry (AABB), walls, openings (door/window/airlock), level/storey, `material_type` | YES — the construction editor (`src/gui/pages/construction.rs`) round-trips it | yes (edit RON) |
| **Machines** | `data/machines/home.ron` (`MachineHome`, `src/machines.rs`) | machine catalog + instances (`{id, machine, room, offset}`) + arrays + connections (power/water/nutrient) + the live electrical role | **NO — there is no in-app way to place a machine** | yes (edit RON) |
| **Blueprint** | `data/blueprints/fibonacci_homestead.ron` (`HomesteadDesign`, `src/gui/mod.rs`) | per-room materials BOM + design-time power/water targets + Fibonacci tiers + build order | NO (read-only on the Home page) | yes (edit RON) |

Consequences:

- **The AI's designs are not player-reproducible.** A player can edit rooms but cannot place a single machine, so the machine half of any AI home is unreachable to them.
- **The AI's job is harder than it should be.** Designing a home means hand-editing three RON files that don't know about each other (move a room and its machines don't follow), with no validation and no result until a restart.
- **The one real seam already exists:** `MachineInstance.room` references a `homestead_layout.ron` room id (`src/machines.rs`). Machines are already *placed in rooms*. The editor already knows rooms. So "edit the machines in this room" is a natural extension, not a new world.

## North star: one home, one surface

1. **One Home representation** that the construction editor fully round-trips: rooms **and** the machines in them **and** the connections **and** the materials. The three files above either merge or become views of one coherent model.
2. **The construction editor is the single design surface.** It edits rooms today; it grows to place machines + draw connections. A player builds a complete home — shell *and* systems — in one place.
3. **AI designs through the same surface.** The repo rule (`docs/design/in-app-ops.md`): *every action lives in a data-driven registry the GUI renders AND an AI can enumerate.* Applied here: room/machine/connection placement are enumerable actions. The AI either (a) edits the same unified file the editor round-trips, or (b) invokes the same placement actions. Either way the AI's design is a player design **by construction** — it can be loaded, edited, and re-saved in the editor.

The litmus test for "done": **an AI-authored home opens in the construction editor, a player can move a room / add a tank / reroute a pipe, hit Save, and it round-trips losslessly.**

## Real-world-valid by validation, not by faith

The numbers are already real; the gap is wiring + a check, not the model.

- **Energy** — kWh/day, not nameplate. `home.ron` (per `docs/design/self-sufficiency.md`): 8×400W panels ≈ 11.5 kWh/day at 4.5 sun-hours. Live: `ElectricalSystem` publishes `PowerStatus {generation, consumption, balance, battery_wh, autonomy_hours}` (`src/systems/electrical.rs`); `integrate_battery` is unit-tested.
- **Water** -- L/day. 8000 L cistern. Live (v0.608): `PlumbingSystem` (`src/systems/plumbing.rs`) fills the cistern from powered producers (the well pump) and drains it for demand (household + irrigation), per plumbing island, publishing `WaterStatus`. Coupled to power -- cut the grid and the pump stops, the cistern drains. Machines spawn as `WaterTank`/`WaterProducer`/`WaterConsumer`/`PlumbingCircuit` entities (derived from machine ports + `MachineStorage`).
- **Food** — kcal/day with honest limits (indoor canopy is light-capped, grow-lights are EROI-negative, aquaponics closes B12/omega-3). Static specs today, no live FoodSystem.
- **Structure** — `materials.csv` carries real density / tensile strength / cost (steel 7850 kg/m³, 500 MPa; oak 750, 100 MPa). `src/systems/construction/solver.rs` already derives member capacity from yield strength; it's a pure de-risk spike, not yet wired to room/machine edits.

**The buildability validator** runs on any design — AI or player — and answers "could you actually build + run this on Earth?":

- Power **balances** over a representative day (generation + storage ≥ consumption, with real losses).
- Water loop **closes** (supply + storage ≥ demand; greywater/rain accounted).
- Structure **holds** (the solver: members carry their load given the chosen material; flag overloads).
- Materials are **accounted** (the BOM is buildable from available stock / is internally consistent).
- Atmosphere **seals** (in the ship context: the pressurized envelope has no unintended breach).

A pass is the guarantee that makes an AI design trustworthy and a player design honest.

## Materials: steel-primary, wood where it earns it

The material model already supports the operator's framing:

- **Steel** is the default frame/hull material — abundant on Earth *and* in space, cheap, strong (500 MPa). `RoomConfig.material_type` already has a `metal` option.
- **Wood** (oak: renewable, 100 MPa) for furniture and structures that don't need steel's strength. `material_type` has a `wood` option.
- The structural validator uses each member's real material, so "this wood beam is overloaded, use steel" is feedback the design surface can give.

## The ship as the wrapper (Earth ≡ ship)

The "fully enclosed homestead in a steel spaceship" is the **enclosure context**, not a different home:

- The homestead is a **pressurized volume** (`EnclosedSpace.sealed`, `src/ecs/components.rs`) inside a steel hull; vacuum outside; `AtmosphereSystem` already models pressure/O₂ and the `DECOMPRESSION_DAMAGE` path on a breach.
- A wood-framed home (`material_type: wood`) is **anchored inside the ship's homestead bay** (`homestead_bounds`) — anchored to the steel hull rather than an Earth foundation.
- **The systems are identical between Earth and ship.** A 12V pump, a 400W panel, a 8000L cistern behave the same; only what the home is bolted to differs (hull vs foundation) and whether the envelope must stay sealed. So we design the **real Earth-homestead systems** once, and the ship is a context flag (sealed envelope + hull anchor), not a fork.

This is the mission alignment: the plumbing + electrical we design here are buildable on a real Earth homestead.

## Parity principle (AI == player)

- The unified Home file is the **single source of truth** the editor round-trips. The AI edits that file; because the editor round-trips it losslessly, the AI's edit is player-editable.
- Home-design **actions** (`place_room`, `place_machine`, `connect`, `place_opening`, `set_material`) join the in-app action registry (`docs/design/in-app-ops.md`) so the AI can enumerate + invoke exactly what a player's buttons do.
- No AI-only path: if the AI can do it, a player can do it in the editor, and vice versa.

## Staged plan

Each stage ships independently + leaves the home buildable.

1. **Machines in the editor (the #1 parity gap).** Add a Machines panel to the construction editor: for the selected room, list its `home.ron` instances; add (from the catalog), remove, nudge offset; render them in the 3D view; Save writes `home.ron` instances. Players can finally place machines; the AI keeps editing the same file. **SHIPPED v0.519.0** (list / add-from-catalog / remove per selected room, persisted via `MachineHome::save` to `home.ron` behind a "Save machines" button; `MachineHome` is now `Serialize` + round-trip tested). Still to do in this stage: offset-nudge + rendering the placed machine in the editor's 3D view.
2. **Connections in the editor.** Draw a power/water/nutrient pipe between two machines; Save writes `home.ron` connections.
3. **Buildability validator. SHIPPED (v0.524 to v0.608).** Machines spawn as the ECS bodies the sims need (power roles + `WaterTank`/`WaterProducer`/`WaterConsumer`); the validation pass (Power source / Energy balance / Wiring / Conduits / Power circuit) is in `MachineHome::buildability_report`, surfaced in the editor + callable by the AI. Remaining: a water-closes check + structure/materials checks.
4. **Unify the model.** Collapse rooms + machines + connections + BOM into one Home representation (or one coordinated save transaction), so a room move re-roots its machines and the blueprint BOM derives from the actual machines + walls.
5. **Action registry.** Register the home-design actions so the AI invokes the same surface the player uses; document in `in-app-ops.md`.

## Open questions

- **One file or coordinated three?** Stage 4 can merge into a single `home.ron`-superset, or keep three files written atomically by one "Save home" transaction. The merge is cleaner long-term; the transaction is less migration risk. Decide at Stage 4.
- **Material on machines.** Machines carry no `material_type` today; the structural validator wants one. Add when Stage 3 needs it.
- **Losses model.** Inverter/wiring/conversion losses (energy) and pipe pressure (water) are not modeled yet; the validator should fold them in for real-world honesty.

## See also

- `docs/design/self-sufficiency.md` — the real-world energy/water/food/nutrient model the numbers come from.
- `docs/design/in-app-ops.md` — the GUI-first + AI-enumerable action-registry principle.
- `docs/design/infinite-of-x.md` — why the home is data, not code.
