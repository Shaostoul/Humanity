# NPC crowd stress test: groundwork design note

Status: DESIGN ONLY (v0.976.x, queue item [d] from the 2026-07-26 loop).
No content ships with this note. It defines what the first mall/hangar
crowd test needs so the build order is agreed before any NPC work starts.

## Why a crowd test

The homestead proves the world works at population ~1 (the player, a few
crew, livestock). The next scale jump - a mall, a hangar deck, a market
street - is population 50..250 in ONE interior volume. Every system that
is O(n) per NPC (ticking, pathing, nameplates, draw submission) meets its
first real bill there. The stress test exists to find those bills BEFORE
gameplay is built on top of crowd scenes, the same way the probe rig found
the terrain patch-cache bills before the planet shipped.

## What exists today (inventory, verified 2026-07-26)

| Piece | Where | State |
|---|---|---|
| Creature AI (passive/aggressive/herd/predator/guard) | `src/systems/ai/mod.rs` (`AISystem`) | Local ECS system, ticks per frame |
| Behavior trees | `src/systems/ai/behavior.rs` | **CORRECTION 2026-09-15: a 26-line dead stub.** `BehaviorNode`/`BehaviorStatus` are type definitions with NO evaluator and ZERO call sites outside their own file; `data/behaviors.ron` (named in the module doc) does not exist. NOT used by creatures. |
| Flow-field pathfinding | `src/systems/ai/flow_field.rs` | **CORRECTION 2026-09-15: a 26-line dead stub.** `FlowField::new` allocates a zeroed direction grid and that is the entire implementation: no cost field, no integration, no solve, zero call sites, and `config/flow_field.toml` does not exist. Its "supports million-agent navigation" doc comment describes an intention, not code. **Nothing in this repo can path an agent around a wall.** |
| Character animation | nowhere | **Does not exist.** No skeletal/skinned pipeline, no GLTF animation import (`grep animation src/assets/*.rs` is empty). NPCs and remote players draw as a two-primitive body+head marker (`src/lib.rs:8795` teal players, `src/lib.rs:8833` amber crew). A crowd today is sliding markers. |
| Crew NPCs (chores, dialogue) | relay-driven, streamed as `net::sync::RemoteNpc`; chore AI server-side (v0.663+) | Works at ~5 crew; never load-tested |
| NPC nameplates | `src/gui/pages/hud.rs` crew loop | v0.975 sightline occlusion applies (O(plates x wall segments) per frame) |
| Interval chores | `ecs::components` `IntervalAction` (AFK NPC chores) | **CORRECTION 2026-09-15: wrong on the name AND on "data-driven".** No `IntervalAction` exists anywhere in `src/`. The real type is `AutonomousTask` (`src/ecs/components.rs:1060`) driven by `OfflineSystem` (`src/systems/offline.rs`), and it is dead three ways over: nothing ever attaches the component, `OfflineSystem` is never registered in `src/lib.rs`, and the tick only increments a counter and logs at `trace!` (its own doc says "each preset id should map to a real action callback"). Crew chores are unrelated: they are relay-side JSON state from `data/npc/chores.ron`. |
| Crew spawn count | `game_state.rs:503` | **One crew member per matching room**, not a count. `data/npc/crew.ron` has 6 entries keyed by `room_type`, so the starter ship caps at ~6 crew. There is no knob that reaches 500; adding one is a prerequisite for this test, not an optimisation. |
| WS message ceiling | `src/relay/mod.rs:1217` | `max_message_size` is 128 KB (64 KB frames). The `game_welcome` snapshot sends every entity's full components JSON including `dialog[]`/`greetings[]` at ~640 B per crew member, so the join handshake breaks somewhere around 200 crew, well before the render cost bites. |
| Wall collision | `ship::wall_collision::resolve` | PLAYER only; NPCs do not collide with walls client-side |
| Dev-rig spawn knob precedent | showcase `lights:N` (camera-local test grid, `engine/ipc.rs`) | The pattern to copy for `npcs:N` |
| Perf instrumentation precedent | F2 overlay, `[ChunkDiag]`, `[lights-diag]` throttled log lines | The pattern to copy for `[npc-diag]` |

Key architectural fact: crew NPCs are simulated on the RELAY and streamed
to clients. A 250-NPC crowd therefore stresses BOTH sides: relay tick +
message fan-out, and client interpolation + render. The test must measure
them separately or the numbers blame the wrong side.

## The three knobs the test needs (build order)

1. **NPC count knob** - showcase `npcs:N`: spawn N LOCAL walker NPCs
   (amber capsule markers, the existing crew visual) in the camera's
   surroundings, exactly like `lights:N`. Local-only spawns first: they
   isolate client cost from relay cost and need zero server work. A later
   `npcs_relay:N` asks the relay to simulate the same count for the
   networked half of the bill.

2. **Task assignment** - a data-driven micro-roster per test NPC, cycling
   states: idle (stand), wander (walk to random reachable point), queue
   (walk to a shared point, stand in line offset), work (stand at a point,
   play the chore label). This is deliberately the smallest state set that
   produces crowd-shaped motion (streams, clumps, queues). Reuse
   `BehaviorNode` if it fits in an afternoon; hand-rolled match-on-enum if
   not. Roster in RON (infinite-of-x: states + weights are data).

3. **Perf counters** - a throttled `[npc-diag]` line + F3 rows:
   - `npc_tick_ms` (client AI + interpolation)
   - `path_queries_per_s`
   - `nameplate_ms` (the v0.975 sight tests are O(plates x segments) -
     measure, do not assume)
   - `draw_submitted` NPC count vs culled
   - relay side (when `npcs_relay:N` lands): tick ms + bytes/s fan-out.

## Test protocol (once the knobs exist)

Probe rig, fixed interior vantage (the ten-room house or a hangar zone),
ramp `npcs:` 10 / 50 / 100 / 250. Capture FPS + `[npc-diag]` at each rung,
with nameplates on and off (isolates HUD cost). Gate to declare victory:
100 NPCs at >= 60 FPS on the operator's RTX 4070 with all counters
attributed (no unexplained frame cost). 250 is the aspiration rung, not
the gate.

## Deliberately out of scope for the first test

- Visuals: capsule markers are enough; no rigs, no animation work.
- Dialogue/interaction: walk-up talk stays crew-only.
- Wall collision for NPCs: expected to be the first REAL finding (walkers
  will stream through walls); measure the cost of adding
  `wall_collision::resolve` per NPC as a test OUTcome, not a precondition.
- Persistence: test NPCs despawn with the knob, save nothing.

## Exit criteria for this note

The note graduates to an implementation arc when the operator green-lights
crowd content (mall/market/hangar gameplay). Until then the knobs are
parked as cheap, self-contained increments any future session can pick up
in order (1 -> 3 -> 2; counters before tasks, so even the wander rung is
measured).
