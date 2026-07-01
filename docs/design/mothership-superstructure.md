# Mothership superstructure layout (vision + plan, 2026-06-29)

> Operator scope (v0.629 session): "figure out how we lay out the superstructure of the mothership -- how
> to lay out rail lines, elevators, teleporters, hangar bays, mech bays, cargo tunnels, storage areas,
> massive industrial factory areas. We'll want to figure out how to build the public meeting zone / the
> shopping mall." Tied to grid-hierarchy.md ("simulate an entire civilization on a single mothership").

The home build editor (a `HomeStructure` box + walls + rooms + placed machines + the road/conduit node
graphs) is the MICRO scale. The mothership is the MACRO scale: a huge volume divided into ZONES, tied
together by TRANSIT networks. The same data-driven, gizmo-first editor philosophy scales up -- a zone is a
big sub-box, transit is a node graph (exactly like conduits/roads), a "shop" or "factory cell" is a
placed sub-structure. Reuse, don't reinvent.

## The model (proposed, additive)
- **Mothership** = an outer hull volume + a stack of DECKS (levels), each a large floor plate. We already
  have multi-level foundations + `level` on rooms/structures (v0.588). A deck is just a big level.
- **Zone** = a labelled, bounded volume on one or more decks with a TYPE (residential / industrial /
  hangar / mech-bay / cargo / storage / agriculture / civic-mall / power / medical / ...). A zone is the
  macro analogue of a room: a sub-box with a purpose, an access policy, and an expected utility profile.
  Data-driven (a `zone_types` registry, like `room_types`), infinite-of-X.
- **Transit networks** tie zones together, each its OWN node graph (mirror `conduit_nodes/edges` +
  `road_nodes/edges`, which already exist and already have draggable gizmos + auto-routing):
  - **Rail**: stations (we have train platforms + rail links, v0.592) -> a rail GRAPH (line nodes + edges)
    so a line can have many stops, not just a pair. Cars run the graph.
  - **Elevators / lifts**: vertical transit between decks (we have the elevator ride, v0.590). A shaft is
    a node spanning decks; a bank of lifts serves a zone.
  - **Teleporters**: instant point-to-point (we have the teleporter machine). A teleporter PAIR/graph is a
    transit edge with an energy cost (ties to the grid: a teleporter draws power, grid-hierarchy.md).
  - **Cargo tunnels**: automated freight runs between industrial/storage/hangar zones -- a conveyor/maglev
    graph carrying ITEMS, not people. New; routes like a rail graph but moves inventory.
- **Sub-structures** inside a zone: shop stalls (civic mall), factory cells (industrial), storage racks,
  hangar cradles, mech bays -- placed pieces from the structure palette (extend `structure_types`).

## The zones the operator named
- **Hangar bays**: large open volumes with big doors to space; ship cradles + a launch/recovery lane.
  Needs huge clear span + an airlock-to-vacuum boundary (ties to atmosphere/air sim).
- **Mech bays**: maintenance cells for mechs/vehicles -- a cradle + tool/power/fluid hookups (ports!).
- **Cargo tunnels**: the freight arteries between hangars, factories, and storage; the cargo-transit graph.
- **Storage areas**: dense racking; an inventory volume (ties to the nested-container inventory redesign).
- **Industrial factory areas**: massive zones of machine arrays (we have machine `arrays` -- a factory IS
  a big array grid) + power/water/material feeds from the grid; the production backbone.
- **Public meeting zone / shopping mall** (the civic heart): a large OPEN social space -- a concourse with
  shop stalls (each a sub-structure with an owner + a market listing, ties to the existing market), a
  gathering plaza, seating, info boards, transit hub access. This is where the "community" of grid-
  hierarchy.md becomes a PLACE. Probably the first macro zone to prototype because it is social + visible.

## What it builds on (do NOT rebuild)
- `HomeStructure` box + walls + multi-level (`level`) + the structural piece system (`structure_types`:
  stairs, ladders, elevators, train platforms, ramps, decks -- v0.583-592).
- The THREE existing node graphs with gizmos + auto-routing: roads (`road_nodes/edges`), conduits
  (`conduit_nodes/edges`, tier 0/1/2), and rail links. Transit networks are more of the same pattern --
  and the v0.629 viewport node-placement + drag-to-node workflow is exactly the editor for them.
- Per-island utility flow + the grid hierarchy (grid-hierarchy.md): a zone draws/supplies utilities; a
  mothership grid ties zones the way the home grid ties machines.
- The market + guild + reputation systems: shops, zone ownership, civic roles.
- The fibonacci/auto-layout (`resolve_positions`) for sensible default placement of zones/decks.

## Staging (each its own arc; the loop will pick them up in order)
- **M1 -- ZONE primitive**: a `Zone` (labelled bounded volume + type from a `zone_types` registry) on the
  HomeStructure/mothership model; place + resize + label it in the editor with the existing box/gizmo
  tooling. A deck is a big level; a zone is a sub-box. Prove the macro editor scales.
- **M2 -- TRANSIT graphs**: generalise the rail link into a rail NODE GRAPH (multi-stop), add an elevator
  shaft node (vertical), and a teleporter edge; all reuse the v0.629 node-placement + drag workflow + the
  route_conduit-style auto-routing. Cars/lifts animate along the graph (we have the elevator ride + rail).
- **M3 -- CIVIC MALL prototype**: one public meeting zone -- a concourse with placeable shop stalls (each
  owner + market listing), a plaza, transit-hub access. The social heart; first real macro zone.
- **M4 -- INDUSTRIAL + CARGO**: factory zones (big machine arrays fed by the grid) + cargo-tunnel freight
  graph moving inventory between factory/storage/hangar. Ties production -> storage -> market.
- **M5 -- HANGAR/MECH bays**: vacuum-boundary hangars with ship cradles + mech maintenance cells (port
  hookups). Ties to atmosphere (airlock to space) + vehicles.

## Open questions
- Scale + performance: a mothership is far bigger than a home -- LOD the macro view (zones as blocks until
  you enter one), don't render every factory machine at city scale (mirrors the detection/grid perf rule).
  Proposed answer (see reconciliation below): only render the current zone + adjacent zones in detail; a
  distant zone renders as a simplified block/skybox-style placeholder. This is a rendering LOD strategy,
  not a data-model change, and is NOT yet implemented, flagged here so M-stage work doesn't silently skip
  it until it becomes a real performance wall.
- One editor or two? Likely ONE editor with a ZOOM/scale switch (mothership view <-> zone view <-> room
  view), since the data model is the same box+graph pattern at every scale. Decide before M1. **Still
  open as of 2026-07-01**, the operator has not resolved this fork yet.
- Persistence/authority: is the mothership a server-owned shared object (one per relay/instance) that many
  players co-build, or a single-author blueprint? (Likely shared + server-authoritative; ties federation.)
- Ownership/governance of zones: who can edit the mall vs a private shop vs a factory? Ties to the
  guild/accord/governance systems.
- Population scale target: is "10 billion" meant as literally-rendered/simulated individuals, or as an
  aggregate capacity number the resource-flow math (food/water/power per zone) should be validated
  against? A 2026-07-01 research pass on this concluded the LATTER is what's actually achievable: the
  renderer's instancing path is confirmed dead code, and a single home's ~104 machine meshes already hit
  the draw-call cap once (had to be raised 256 -> 1024 at v0.528). An aggregate `population: u64` rolled
  up per zone/deck, feeding a per-capita resource model, is unbounded by population size (only by zone
  count) and reuses the exact aggregation pattern the utility-trio's `PowerCircuit`/`PlumbingCircuit`
  island system already implements one tier down. Individual "living their lives" NPCs (schedules, needs,
  riding the rail cars) stay a small, deliberately-bounded population layered on top for flavor, not the
  mechanism computing whether the ship's numbers close. Full writeup: ask for the 2026-07-01 mothership
  simulation research findings, not yet split into its own design doc.

## Reconciliation with docs/game/humanity_one.md (2026-07-01)

`docs/game/humanity_one.md` is an older lore/vision doc (10 billion population, 500km x 100km cylindrical
ring-ship, a `Ring > Sector > Deck > Block > Room` addressing scheme) that predates this doc and was never
wired to the `Zone` system that actually shipped (v0.631+). It is NOT a competing technical spec, treat it
as vision/flavor context with a few genuinely useful ideas worth pulling forward:

- Its **district list** (residential towers, agricultural rings, industrial sector, commercial district,
  medical, docking bay) maps almost 1:1 onto the `zone_types.ron` registry that actually shipped
  (residential / agriculture / industrial / civic_mall / medical / hangar / power / ...). Good alignment,
  no action needed, the real system already covers this ground.
- Its **LOD/performance strategy** ("only render the current section + adjacent sections; far sections as
  simplified geometry") is the best existing answer to this doc's own "scale + performance" open question
  above and should be the starting point when that gets built, not reinvented from scratch.
- Its **Ring/Sector addressing tier** does NOT exist in the shipped model (which is `Deck > Zone > Room`)
  and should not be assumed as current. If the mothership ever needs a tier ABOVE Deck (grouping many
  decks, e.g. for federation/multi-server sharding of a huge ship), Ring/Sector is a reasonable name to
  revisit, but it is not built and not scheduled.
- Its **Hub-tab-to-ship-section mapping table** (Bridge=Map, Comms=Chat, Fantasy, Lore, Source, ...) is
  decorative flavor text from an earlier nav era and does NOT match the current `GuiPage` enum (see
  `docs/PAGES.md`, 52 real pages, none named this way). Do not treat that table as a current UI spec.
