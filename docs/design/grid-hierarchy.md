# Utility grid hierarchy -- home -> substation -> generator -> fleet (vision, 2026-06-29)

> Operator vision (verbatim intent, v0.626 session): "a main line for electricity, internet, and water
> that connects the house to the mothership's (or town/city) grid. Homes could have breakers, then
> residential areas could have substations, then those get fed by one or more big generators. Players
> could generate and provide supply to the fleet. If people exclusively consume, we track how much they
> actually use and teach supply/demand, local self-sustaining, and being part of a community. We don't
> punish consuming -- we help them understand how much they consume. If we can simulate an entire
> civilization on a single mothership we'll have succeeded in our task of creating a game able to
> teach/help humanity."

This is a STRATEGIC north-star, captured for staging. The local home utility sim (power/water/air, the
conduit node-graph, per-island flow gating) is the bottom tier of it; this doc says how it scales up to a
whole mothership and how the teaching framing works. Build it in stages; nothing here is one increment.

## The tiers (bottom to top)
1. **Appliance/port** -- a machine's IN/OUT ports (the `Port` model, the v0.627 sphere + in/out arrow
   node gizmo). The atom: a thing that draws or supplies a utility.
2. **Home circuit** -- a dwelling's internal wiring + a BREAKER PANEL / service entrance: one grid-tie
   port per utility (power/water/data) where the home meets the outside line. Today this is the per-home
   `PowerCircuit`/`PlumbingCircuit` islands; the grid-tie is a new node TYPE (a "service entrance" node).
3. **Local distribution** -- a residential block's SUBSTATION / main that several homes tie into. A node
   of a higher tier (the conduit-node `tier` field: 0 main / 1 sub / 2 subsub already exists for exactly
   this). Aggregates the homes' draw + local generation.
4. **Generation** -- one or more big GENERATORS (or a fusion/solar farm) feeding the substations. A home's
   own panels/wind are micro-generation that can BACKFEED up the tiers (net metering).
5. **Fleet/mothership grid** -- the top bus. The whole ship's supply/demand balance; a town/city analogue.
   Players who generate surplus PROVIDE to the fleet; players who only consume DRAW from it.

The same `Utility` enum + conduit kinds run at every tier; only the capacity (cable ampacity / pipe bore /
data bandwidth) and the node `tier` change. A grid-tie is just a port where one tier's island connects to
the next tier's bus -- so the existing union-find island model extends upward by treating each tier's bus
as a node every lower island reaches through its service entrance.

## Teaching framing (NON-PUNITIVE -- this is the whole point)
- METER every consumer: track kWh, litres, GB actually used to do what the player already does (cook,
  grow, browse). Surface it as understanding, not a penalty -- "your grow used 4.2 kWh today; your panels
  made 6.1, so you exported 1.9 to the fleet."
- Show the SUPPLY/DEMAND balance at each tier (home self-sufficiency %, block import/export, fleet margin)
  so the lesson is visible: local generation + storage = resilience; over-consumption = leaning on the
  community's shared supply.
- Reward CONTRIBUTION (exporting surplus, running a substation, maintaining a generator) so being part of
  a self-sustaining community is the attractive path -- not by punishing consumption, but by making
  contribution legible + valued. Ties to the reputation/guild systems.
- A player CAN choose to be a pure consumer; the sim just makes the real cost (someone has to supply it)
  visible, the way a real off-grid vs on-grid choice teaches itself.

## What it builds on (already shipped -- do NOT rebuild)
- `src/utilities.rs` -- `Utility`, `Port`/`PortDir`, conduit/cable physics (ampacity, bore, bandwidth).
- Per-island runtime flow gating -- `PowerCircuit`/`PlumbingCircuit` + the balance systems (a tier IS an
  island; a grid-tie joins two islands).
- The conduit NODE GRAPH (`docs/design/conduits-node-graph.md`) with `ConduitNode.tier` (0/1/2) -- the
  data model for main/sub/subsub is ALREADY there; this is its gameplay purpose.
- Buildability checks (power source present, energy balances over a day, wiring intact) -- extend to "does
  this home's generation + import cover its demand," then to block + fleet scale.
- The v0.627 port node gizmo (central sphere + in/out arrows) -- the visual language for every tie point.

## Staging (rough, each its own arc)
- **S1 (next, small):** a "service entrance" node type per home (one power/water/data grid-tie). Pipes
  terminate at port nodes (the operator's "cables/pipes go to the input/output nodes"), not the floor
  anchor -- so a home visibly plugs into a main line.
- **S2:** metering -- accumulate per-consumer usage + a home self-sufficiency readout (uses the existing
  ECS balance tick; just record + surface it).
- **S3:** tiers above the home -- a substation node aggregating several homes; net export/import up the
  chain; the conduit `tier` drives routing (trunk-and-branch, the Stage-2 hierarchy in conduits-node-graph).
- **S4:** fleet bus + contribution economy -- surplus export credited; the mothership supply/demand margin
  as a shared, visible civic stat. The "simulate a civilization" milestone.

## Open questions
- Performance at fleet scale: thousands of meters can't tick per-frame -- aggregate per tier on a coarse
  cadence (mirrors the detection-sensing perf concern). A home reports a rolled-up draw to its substation,
  not every appliance to the fleet.
- Persistence/federation: is the fleet grid per-server, or a shared federated object? (Likely per-mothership
  instance first; federation later.)
- Economy coupling: does exported energy pay in the existing token/market system, or a separate utility
  credit? Keep it teaching-first; avoid making it a grind.
