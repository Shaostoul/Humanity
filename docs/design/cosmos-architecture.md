# Cosmos Architecture

Universal model for player position, ship movement, multi-system navigation,
and "what's nearby right now" — across scales from a player walking inside
a ship to a fleet crossing interstellar distances.

**Status:** Proposal (2026-05-09)
**Affects:** `src/gui/pages/maps.rs`, `src/gui/pages/inventory.rs`,
`src/ecs/`, `src/systems/`, `src/renderer/floating_origin.rs`,
`src/relay/handlers/game_state.rs`, `data/star_systems/`, `data/galaxy/`,
`data/ships/`
**Supersedes:** the Sol-hardcoded `data/solar_system/bodies.json` schema.
Layers underneath `docs/design/maps-multi-scale.md` (which describes the
web-side render hierarchy and continues to apply on top of this model).

---

## 1. Why this exists

The game spans 30 orders of magnitude: a player walks the bridge of their
ship at meter scale, the ship orbits at AU scale, the system sits inside a
galaxy at light-year scale. No single coordinate triple holds that range
without losing precision somewhere — `f32` breaks past ~10 km, `f64` past
~30 AU, fixed-point integers run out at galactic scales.

Every game that solves this (Star Citizen, Elite Dangerous, KSP) uses the
same two ideas in combination:

1. **Hierarchical positions** — store position as "what container am I
   inside, and where in that container?" Each level uses local coordinates
   relative to its parent, keeping precision tight.
2. **Floating origin** — for rendering, recenter the visible world around
   the player periodically so the rendered scene stays near the origin
   regardless of "absolute" galactic position.

We already have the floating-origin half (`src/renderer/floating_origin.rs`,
wired into `EngineState`). This doc specifies the hierarchy.

---

## 2. The container model

Every entity that has a position — players, ships, NPCs, dropped items —
stores its position as a **container reference + local offset**:

```rust
pub struct PositionInUniverse {
    /// What am I inside / on / near?
    pub container: ContainerRef,
    /// Where in the container's local frame? Units depend on container type.
    pub local_pos: glam::DVec3,
    /// Facing direction.
    pub local_rot: glam::DQuat,
}

pub enum ContainerRef {
    /// Inside or attached to a vessel. "Vessel" generalizes the original
    /// "Ship" name to cover anything mobile-and-inhabitable: spaceships,
    /// cars, trucks, tanks, fighter jets, space stations, walking mechs,
    /// even buildings (treated as a stationary vessel). The vessel's
    /// layout (rooms, corridors, seats) is defined in its RON file.
    /// local_pos is meters from the vessel's origin.
    /// Renamed from `Ship` per operator 2026-05-09.
    Vessel(VesselId),

    /// On the surface of a celestial body. Asteroids, planets, moons,
    /// comets — anything you can stand on. local_pos is east/north/up
    /// in meters from the body's surface origin (lat/lon → ECEF-style).
    Body { system_id: String, body_id: String },

    /// Free-floating in a star system. local_pos is meters from the
    /// system's barycenter (or its primary star, near enough at this scale).
    Space { system_id: String },

    /// Free-floating in interstellar space. `galaxy_pos_ly` is a
    /// continuous 3D vector in light-years from a chosen galactic
    /// origin (Sol by default — we can change the origin later
    /// without breaking the data model). f64 at 100 kly distance gives
    /// ~1 mm precision, which is far more than needed for ship-scale
    /// navigation. **No chunks at the data-model level.**
    ///
    /// Revised 2026-05-10 per operator pushback: the earlier chunk_coord
    /// model conflated "procedural generation seeding" with "position
    /// addressing." Position is now continuous; chunks are an internal
    /// implementation detail of the procedural rogue generator and
    /// sparse mutation persistence (§10), never surfaced here.
    Deep { galaxy_pos_ly: glam::DVec3 },

    /// Pocket dimension — an isolated coordinate space disconnected from
    /// the normal galaxy. Use for tutorial spaces, tech demos, instanced
    /// quest areas, or any "outside the main universe" gameplay. The
    /// dimension's id selects which pocket. Travel into/out of a Pocket
    /// is a portal event, not a continuous transit.
    /// Added per operator 2026-05-09 ("maybe other universes / pocket
    /// dimensions"). A future Dimension variant could sit at the top of
    /// the chain to model fully-alternate universes with their own
    /// galaxies; deliberately punted (premature complexity).
    Pocket(PocketId),
}

pub type VesselId = String;        // e.g. "pioneer-001", "ford-f150-abc"
pub type PocketId  = String;       // e.g. "tutorial-cave", "boss-arena-42"

// (GalaxyChunkCoord removed in 2026-05-10 revision — Deep now uses
//  continuous galaxy_pos_ly: DVec3. Chunks are an internal detail of
//  the procedural rogue generator only.)
```

Read this as **astronomical addressing**:
> "I'm in the captain's chair on the bridge of *Pioneer*, which is in
> orbit around Earth, which is in Sol, which is at galactic position (0, 0, 0) ly."

Each level only knows about its parent. To get a player's "world
position" for rendering, walk up the parent chain summing local frames.

---

## 3. Position composition

The world position of an entity is computed by recursively resolving its
container chain. Pseudo-code:

```rust
fn world_position(pos: &PositionInUniverse, world: &World, sim_time: SimTime) -> Vec3DeepSpace {
    let parent_world = match &pos.container {
        ContainerRef::Ship(id) => world_position(&world.ships[id].position, world, sim_time),
        ContainerRef::Body { system_id, body_id } => body_position(system_id, body_id, sim_time),
        ContainerRef::Space { system_id } => system_barycenter(system_id),
        ContainerRef::Deep { galaxy_pos_ly } => *galaxy_pos_ly, // already in galactic frame
    };
    let parent_rot = ...; // analogous
    parent_world + parent_rot * pos.local_pos
}
```

Notes:
- `body_position(system, body, t)` is deterministic from orbital elements
  (Kepler) — anyone with the system data + sim_time computes the same
  answer. No need to sync body positions.
- `world.ships[id].position` recurses; ship-in-ship (drone bay → fighter)
  works naturally up to whatever nesting depth we allow.
- The chain terminates at `Deep` — its `galaxy_pos_ly` IS the absolute
  galactic position. No further unwrapping needed.

---

## 4. Movement scenarios

### 4a. Player walks inside the ship
- `container` doesn't change. `local_pos` updates frame-by-frame from input.
- High-frequency: synced ~20 Hz over WebSocket like any FPS movement.
- Bandwidth scope: only peers whose `container` matches need this update.

### 4b. Player walks off the ship onto a planet's surface
- `container` changes from `Ship(pioneer)` → `Body { system: "sol", body: "earth" }`.
- This is a discrete event: synced once with `(new_container, new_local_pos)`.
- The transition point (airlock, ramp) is part of the ship layout; crossing
  it triggers the swap.

### 4c. Mothership accelerates / changes orbit
- The ship has its own `PositionInUniverse` with `container = Space { system: "sol" }`.
- `ship.position.local_pos` updates over time (via orbital mechanics or
  manual flight controls).
- **Players inside don't notice anything at the data level.** Their
  `container` is still `Ship(pioneer)`, their `local_pos` (5 m east of the
  bridge center) is unchanged.
- Their **rendered** world position updates because the parent chain
  resolves with the ship's new position. This is just standard parent-child
  rigid body relationship.

### 4d. Fleet jumps to deep space
- Each ship's container transitions `Space { system: "sol" }` → `Deep { galaxy_pos_ly: ... }`.
- Players' containers are still `Vessel(...)` — no per-player update needed.
- During transit, the ship's `galaxy_pos_ly` updates continuously as it
  moves through interstellar space. No discrete chunk crossings — the
  vector just changes over time like any continuous physics simulation.

### 4e. Ship arrives at a new system
- Ship's container transitions `Deep` → `Space { system: "alpha_centauri" }`.
- The system's data file (`data/star_systems/alpha_centauri.json`) is loaded
  if not already cached.
- Players still don't change container.

### 4f. Ship is destroyed
See §11. Players inside transition to a fallback container (escape pod,
nearest body, "void" container with timeout-to-respawn).

---

## 5. The cosmos UI is context-aware

There is **no fixed "Sol system" page**. There is a **Cosmos page** (working
name) that renders the contents of the player's current container at the
appropriate scale:

| Player's container | What the Cosmos page shows |
|--------------------|----------------------------|
| `Ship(X)` | Ship interior layout (from RON) + crew + the view "out the windows" of the ship's own neighbors |
| `Body { system, body }` | Local terrain around player + nearby surface entities; zoomable up to system view |
| `Space { system }` | Player as a dot in the system; all bound bodies + ships in that system |
| `Deep { galaxy_pos_ly }` | Player as a dot in deep space; rogue bodies within R light-years queried from the procedural generator + any stored mutations |

The same UI handles all four cases by branching on `container` type. Zoom
gestures cross the scale boundaries per `docs/design/maps-multi-scale.md`.

There is also an **Indoor Map** widget — a small panel showing the player's
ship interior with the player's `local_pos` as a dot — that's persistent
regardless of which container the ship itself is in. This is the "Ship Bridge
viewscreen" the operator described.

---

## 6. Time

**There is one universal simulation time:** `SimTime` in milliseconds since
the epoch. All position computations are functions of `SimTime`.

```rust
pub struct SimTime(pub u64); // milliseconds since J2000.0
```

Why a single global time:
- Body positions (orbital mechanics) need a time input. Two players looking
  at Mars must agree on where Mars is.
- Replays / synchronization need deterministic playback.

**Time speed is server-controlled** with per-region overrides:
- Default speed: 1× real time (a real second = a sim second).
- Inside a "fast travel" volume (between systems): 100×–10000× by
  vote / per-fleet captain authority.
- Combat / crowded scenes: forced to 1× to keep things fair.

The relay is the source of truth. Clients receive `(sim_time_ms, sim_speed)`
periodically; clients interpolate between updates.

**Trade-off accepted:** a single global time means players can't be on
different "speeds" at once. This is a deliberate simplification — relativistic
time dilation is out of scope. All players in a fleet experience the same
sim seconds.

---

## 7. Authority — who owns what mutable state

| State | Authority | Sync model |
|-------|-----------|------------|
| Player position (`local_pos` within container) | Player's own client | Client pushes ~20 Hz to relay; relay broadcasts to peers in same container |
| Player container change | Relay validates (legal transition?); relay broadcasts | Event-driven |
| Ship position when piloted | The piloting client | Client pushes ~5 Hz; relay broadcasts to all subscribed clients |
| Ship position when on autopilot | Relay-authoritative (NPC orchestrator) | Relay computes + broadcasts |
| Ship container change | Relay validates + broadcasts | Event-driven |
| Body position | Deterministic from orbital elements + sim_time | NEVER synced; computed locally |
| Rogue body position (procedural) | Deterministic from seed + galactic position + sim_time | NEVER synced; computed locally |
| NPC ships | Relay-authoritative | Relay broadcasts |
| Sim time + speed | Relay | Periodic gossip + on-change events |

**Rule of thumb:** if it's deterministic, don't sync it — re-derive it. If
multiple humans can mutate it, the relay validates and is the source of truth.

---

## 8. Multiplayer sync detail

Three frequency tiers:

**High-frequency (per-frame-ish):**
- Player local_pos within container — only sent to peers in the same container.
- Ship position when manually piloted — sent to all subscribers of that ship.

**Medium-frequency (~1 Hz):**
- Ship orbital state when on autopilot.
- Sim time gossip.

**Event-driven (sporadic, signed):**
- Container transitions (player boards ship; ship jumps to deep space).
- Ship destruction.
- Sim speed changes.

**Subscription model:** a client subscribes to update streams it cares about.
By default a client subscribes to:
- Its own player.
- Its current container's ship (if any) and that ship's neighbors in its
  parent container.
- All other players in the same container as the player.

**Bandwidth scaling:**
- N players in one ship = N × N high-freq position updates if everyone
  subscribes to everyone. Mitigation: spatial culling within the ship —
  only subscribe to peers within visible range or same room.
- Many ships in a system: subscribe coarsely to ship positions, finely
  only to the ship the player is on.

---

## 9. Persistence

**Stored per-player:**
- `PositionInUniverse` (the player's container ref + local_pos).
- Inventory, profile, etc. (already exists).

**Stored per-ship:**
- Ship ID, layout reference (which RON file), current `PositionInUniverse`,
  velocity vector if moving, owner public key, crew list (player keys with
  permissions).

**Stored per-system / per-body:**
- Static data: name, mass, orbital elements, atmosphere, etc. — all in
  `data/star_systems/{system}.json`. Never mutated at runtime.

**Computed, NOT stored:**
- Body world positions at any sim_time.
- Rogue body positions at any galactic point (procedural function).
- Player world positions (always computed from container chain).

**Universe state file growth:** scales with players + ships, not with
bodies or rogues. A million players + 100k ships ≈ ~100 MB of state.
Manageable.

---

## 10. Galaxy spatial index + procedural rogues

Revised 2026-05-10 per operator: **the position model is continuous
(galactic Cartesian DVec3); spatial indexing and procedural generation
are internal implementation details, not part of the public model.**

For "what's near my fleet right now in deep space" queries we need
efficient lookups. Two layers, each with its own internal indexing
strategy:

**Layer 1 — Known bodies (bound to systems):**
- Source: `data/star_systems/index.json` lists every system with its
  `galaxy_position_ly` (continuous DVec3).
- Loaded into an **octree at startup**. Fast point / sphere / k-nearest
  queries with O(log N) cost. Octree is right here because systems
  cluster non-uniformly (most near the galactic plane / spiral arms;
  empty void elsewhere) — adaptive subdivision matches the data.
- The octree is an internal index structure, not a data-model concept.
  The system positions themselves are still continuous DVec3 values.

**Layer 2 — Rogue interstellar bodies (NOT bound to any system):**
- Position is continuous; we never store the universe as a grid of
  chunks. But the procedural generator needs a deterministic function
  `bodies_near(p, r) → Vec<RogueBody>` that two different clients
  evaluate identically.
- Implementation: a deterministic **Poisson disc / noise field** keyed
  by `galaxy_seed`. Given a query sphere `(center, radius)`, the
  generator samples positions inside that sphere where the noise
  function exceeds the body-spawn threshold. Result is the same set of
  rogues regardless of who calls it.
- Internally, the generator buckets queries into ~1 ly voxels for cache
  efficiency (looking up the same region twice doesn't re-evaluate the
  noise field). Voxels are an **internal optimization**, not exposed
  in `ContainerRef` or any storage row.

**Persistence of rogue mutations** (when a player mines a rogue):
- Stored as `(quantized_position, mutation_payload)` rows in the relay's
  `rogue_state` table — sparse, only mutated positions get a row.
- The quantization (e.g. round to 0.01 ly = ~94 billion km) is fine
  enough that no two real rogues collide, but coarse enough to keep
  the row count bounded. Pure internal addressing — players see only
  continuous galactic positions.

**"What's nearby" query** (called when player container is `Deep`):
1. Compute systems within R ly via octree (Layer 1).
2. Compute rogue bodies within R ly via procedural sampling (Layer 2).
3. Apply any mutations from `rogue_state` to matching positions.
4. Return as the indoor map's content.

### Why this is better than the chunked model

Operator pushback (2026-05-10): "Why do we have to do cubic chunks
instead of relative positioning?" Answer: we don't. The earlier draft
made chunks part of the data model, which:
- forced ship positions to track `chunk_coord` and increment it on
  every chunk boundary crossing (artificial discreteness),
- made the universe LOOK gridded even though continuous physics is
  natural,
- conflated "procedural seeding bucket" with "position addressing".

Continuous positions in light-years (f64 DVec3) give ~1 mm precision
even at 100 kly galactic radius — far more than ship navigation needs.
Procedural generation still needs SOME bucketing internally for cache
efficiency and deterministic seeding, but that's a private detail of
the generator, not part of the position model. Orbital motion, smooth
FTL transit, and inter-system travel all become natural vector math
with no special-case "what chunk am I in?" bookkeeping.

---

## 11. Edge cases

### Container destroyed (ship blows up)
- All entities inside the ship transition to `Body(emergency_pod)` — a
  default escape vessel that drifts at the ship's last position.
- If no emergency pod available, transition to a `Void` container that
  schedules respawn at a player-chosen home location after N seconds.

### Container teleport (ship jumps to deep space)
- Smooth client-side: cross-fade between renderers (system view → deep
  space view) over ~1 second. Player sees the transition cinematically.
- The container ref change is a single discrete event regardless.

### Container nested deeply
- Support up to N=4 levels (player → fighter → carrier → space). Beyond
  that the position composition cost grows linearly with depth, which is
  fine but the UX gets confusing.
- Enforce N=4 limit in the validator (relay-side).

### Network partition while inside a moving ship
- Client keeps rendering at last-known ship velocity (dead reckoning).
- On reconnect, snap to authoritative position with a brief visual
  interpolation.

### Two players "piloting" the same ship
- Pilot is a role attached to a specific seat (defined in ship RON).
- Only the player currently occupying that seat has piloting authority.
- Sitting down acquires the role atomically (relay-validated).

### Cross-server fleet movement (federation)
- A ship in `Space { system: "sol" }` on relay A can transit to a system
  hosted on relay B.
- Mid-transit, the ship's container is `Deep` — no relay authority needed
  for the spatial position.
- On arrival, ship + crew migrate to relay B's authority. Federation
  handshake transfers ship state.
- Out of scope for v1, but the model accommodates it.

---

## 12. Renderer integration

The existing floating origin (`src/renderer/floating_origin.rs`) handles
the "keep render coordinates near zero" half. With this model:

- **Per frame**: compute the player's world position from their container chain.
- Pass that world position to `FloatingOrigin::recenter_if_needed()`.
- All other entities the renderer cares about (bodies, ships, other players)
  also resolve their world positions from their container chains; floating
  origin shifts them to the same render frame.
- LOD: nearby bodies render with detail, distant bodies as billboards.
- Skybox: in `Space` or `Deep` containers, the skybox is "what's far away"
  — paint stars from `data/galaxy/skybox_catalog.json` plus large nearby
  bodies as proper geometry.

---

## 13. Data layout

```
data/star_systems/
  index.json             # registry of all known bound systems with galaxy positions
  sol.json               # all bodies bound to Sol
  alpha_centauri.json    # placeholder — empty bodies list, just metadata
  trappist_1.json
  ...
  README.md              # schema docs so anyone can drop in new systems

data/galaxy/
  galaxy_seed.json       # deterministic seed for procedural rogues
  skybox_catalog.json    # named distant stars for skybox rendering
  rogue_density.json     # tunable parameters: bodies-per-ly³, types

data/ships/
  pioneer.ron            # ship layout (existing convention from src/ship/)
  freighter_class.ron
  fighter_class.ron
  emergency_pod.ron      # default escape vessel
  ...
```

**Per-system file shape** (`data/star_systems/sol.json`):

```json
{
  "id": "sol",
  "name": "Solar System",
  "primary_star": "sun",
  "galaxy_position_ly": [0, 0, 0],
  "epoch": "J2000.0",
  "bodies": [
    { "id": "sun",   "type": "star",   "physical": {...}, "orbit": null },
    { "id": "earth", "type": "planet", "physical": {...}, "orbit": {
        "parent": "sun",
        "semi_major_axis_au": 1.0,
        "eccentricity": 0.0167,
        "inclination_deg": 7.155,
        "longitude_of_ascending_node_deg": -11.26064,
        "argument_of_periapsis_deg": 114.20783,
        "mean_anomaly_at_epoch_deg": 358.617
    }},
    ...
  ]
}
```

**Index file shape** (`data/star_systems/index.json`):

```json
{
  "systems": [
    {
      "id": "sol",
      "name": "Solar System",
      "primary_star_name": "Sun",
      "spectral_class": "G2V",
      "galaxy_position_ly": [0, 0, 0],
      "distance_from_sol_ly": 0
    },
    {
      "id": "alpha_centauri",
      "name": "Alpha Centauri",
      "primary_star_name": "Alpha Centauri A",
      "spectral_class": "G2V",
      "galaxy_position_ly": [-1.348, -3.972, -1.535],
      "distance_from_sol_ly": 4.367
    }
  ]
}
```

---

## 14. Implementation phases

This is multi-month work. Phases sized to ship in 1–3 release cycles each.

**Phase 1 — Data restructure (no code change to position model yet)**
- Move `data/solar_system/bodies.json` → `data/star_systems/sol.json`
  with the wrapper schema above.
- Create `data/star_systems/index.json` with Sol entry.
- Update existing `parse_bodies()` loader to read from new paths.
- Verify: nothing visible to the user changes.

**Phase 2 — Position model in ECS**
- Add `PositionInUniverse` component.
- Add `Container` resource (the container graph: ship → parent, etc.).
- Refactor existing player position to use `PositionInUniverse` with
  default container `Body { system: "sol", body: "earth" }`.
- World-position resolver function.
- No UI change yet.

**Phase 3 — Cosmos page (context-aware)**
- New `pages/cosmos.rs`. Renders based on player's container.
- Replaces the dead-code orbit visualization in `maps.rs`.
- Multi-system data already loaded; system switcher dropdown for testing.
- Indoor Map widget for ship interior view.

**Phase 4 — Ship as a container**
- Add `ShipId` + `Ship` storage.
- Layout-driven interior (RON file → walkable rooms).
- Player can transition `Body(earth) ↔ Ship(pioneer)` via airlock event.

**Phase 5 — Ship movement + sync**
- ECS system updates ship `local_pos` based on velocity / orbital state.
- WS messages for `ship_position_update`, `container_transition`.
- Multiple clients see the same ship motion.

**Phase 6 — Deep space + galaxy octree**
- Galaxy octree built at startup from `index.json`.
- `Deep` container support.
- Rogue body procedural generation.
- Ship transit between systems.

**Phase 7 — Time controls**
- Sim time gossip.
- Sim speed control + voting.

**Phase 8 — Edge cases + polish**
- Ship destruction, escape pods, cross-server transit, etc.

Each phase is independently shippable — the model degrades gracefully.

---

## 15. Open questions (decisions still to make)

These are deliberate punts. Each will surface as a real choice during
implementation; capturing them here so we don't pretend they're settled.

1. **Time speed authority.** Server-wide vs per-fleet vs per-region? The
   doc proposes server-with-region-overrides; if the operator's vision is
   fleet-controlled time (a single ship can engage warp), that needs a
   different sync model.

2. **Procedural rogue density.** How many asteroids per cubic light-year?
   What fraction are minable? What fraction have rare materials? Tuning
   knob in `data/galaxy/rogue_density.json`.

3. **Save format.** Position state on the relay: SQLite columns vs separate
   binary file. SQLite is easier to query; binary is faster for periodic
   snapshots. Probably SQLite for now (we already have `relay.db`).

4. **NPC ship orchestration.** Per-relay (each relay simulates the NPCs in
   its systems) or per-fleet (a designated client AI runs the NPCs)?

5. **Player death + respawn.** Where does a dead player respawn? Home
   planet (chosen during onboarding)? Random nearby body? Owner's ship?

6. **Ship ownership / crew permissions.** Ship has owner + crew roles
   (captain, navigator, gunner, passenger). What's the permission model?
   Captain can promote crew? Owner can boot anyone?

7. **Cross-server fleet movement.** Federation handshake for ship transit
   between two relays. Out of scope for v1 but worth designing now so the
   model doesn't preclude it.

8. **Coordinate precision strategy at the edges.** `f64` for `local_pos`
   inside `Space` (~AU scale) is fine — 15 digits gives meter precision at
   AU. Inside `Deep`, `galaxy_pos_ly` is f64 ly; at galactic radius
   (100 kly) f64 still gives ~1 mm precision. Inside `Body` surface,
   `f64` gives sub-millimeter at planetary radii. **All good.**

9. **Unit conventions.** Suggest: SI throughout (meters, seconds, kg) for
   storage and math. Convert to AU / ly / km only for display. The current
   data files mix km and AU; restructure to standardize.

10. **Asteroid belt vs individual asteroid.** The Sol asteroid belt has
    millions of bodies. Storing each is impractical. Strategy: belt is a
    `region` with density parameters; individual asteroids generated
    procedurally from the noise field within the belt's volume. Major
    named asteroids (Ceres, Vesta, etc.) stored explicitly; the rest
    are procedural.

---

## 16. What changes vs today

| Today | After this design lands |
|-------|------------------------|
| Player has `Vec3` world position | Player has `PositionInUniverse` (container + local) |
| Sol bodies in `data/solar_system/bodies.json` | Sol bodies in `data/star_systems/sol.json`, plus `index.json` registry |
| `parse_bodies()` loads one embedded JSON | `load_system(id)` loads any system on demand |
| Map page renders Sol orbit hardcoded | Cosmos page renders contents of player's current container |
| Ship is a render-only model | Ship is a container with its own position; players can be inside |
| No fleet movement | Ships have velocity, update over time, players inherit motion |
| One solar system | Many systems + interstellar deep space + procedural rogues |
| Single-relay assumption | Federation-friendly transit between relays |

---

## 17. Things this doc does NOT cover (separate design needed)

- **Combat + damage.** Ships taking damage, how that affects movement,
  destruction conditions.
- **Resource gathering mechanics.** What it looks like when a ship
  approaches a rogue asteroid and starts mining.
- **Crafting + ship construction.** How players build new ships.
- **Economy / trade between fleets.** Marketplace model already exists for
  flat goods; ship-to-ship trade across systems is unspecified.
- **Stargate / wormhole / warp drive.** Faster-than-light specifics. The
  architecture supports any FTL model — it's just "ship transitions to
  Deep, eventually transitions out" — but the in-game mechanics aren't
  defined.
- **AI agents living in the world.** AI agents are first-class citizens
  per the platform mission; their position model is the same as players,
  but governance / permissions for AI movement isn't defined here.

---

## 17a. Locked decisions (2026-05-10)

Confirmed in the second design session, supersedes any earlier "tentative":

### Time

**Universal sim time, always 1× real time, never dilated, never accelerated.**

Sim time always advances at the same rate as real time, gossiped from the
relay, identical for every player on the server. No time speed regions, no
fleet-controlled time, no fast-forward. Operator: *"We should stay synced
with Earth regardless of the speed we're going or how close to the black
hole we are."*

Fast travel between systems still works — but via FTL drives that decouple
**real-time journey duration** from **sim-time advance**:

| FTL type | Real-time journey | Sim-time advance | Notes |
|----------|------------------|-----------------|-------|
| **Blink drive** (BSG-style FTL jump) | Instant (~0 s real) | 0 s sim | Tech-gated, resource-cost, cooldown. Container_swap directly: `Space{"sol"} → Space{"alpha_centauri"}`. Ship never enters `Deep`. |
| **Sublight / slow FTL** | Real time = distance / drive speed | Same as real time | Continuous deep-space travel. Ship's container goes `Space{"sol"} → Deep{galaxy_pos_ly}` (vector updates over time) `→ Space{"alpha_centauri"}` on arrival. Encounter rogue bodies en route via the procedural generator. |

Both keep `sim_time = real_time` globally. A 4-ly sublight trip at 1 ly/hour
takes 4 hours real AND 4 hours sim. Players can do other gameplay during
the journey (chat, craft, idle) or pay the blink-drive cost for instant.

### Vessels nest; rooms don't

Operator: *"a player home could be considered 1 room since it is in a
giant mothership. However each home has a bunch of rooms. And each
mothership has tons of rooms."*

Resolution: **homes are sub-Vessels of the mothership.** Rooms within a
home are NOT separate containers — they're spatial subdivisions of the
home's layout file.

```
Player
  → container: Vessel("alice-home-001")
  → local_pos: (3.5, 0.0, 2.1) m within the home
                                  ↓
Vessel("alice-home-001")          ← Alice's home, customizable RON layout
  → container: Vessel("mothership-pioneer")
  → local_pos: (210.0, 0.0, 480.0) m within the mothership
                                  ↓
Vessel("mothership-pioneer")      ← The mothership, hand-authored or procedural layout with N home slots
  → container: Space{system_id: "sol"}
  → local_pos: orbital position in meters from Sol barycenter
```

Container nesting depth here is 3 (Home → Mothership → Space), well within
the 4-level limit. Recursive position composition just walks the chain.

Crossing a vessel boundary = container swap:
- **Walk between rooms inside your home** → no swap, just `local_pos` update
- **Walk out your home's front door** into the mothership corridor → `Vessel(home)` → `Vessel(mothership)`
- **Step into the mothership's airlock** and spacewalk → `Vessel(mothership)` → `Space{system}`

Boundary coordinates are defined in the layout file (front door position,
airlock position). The ECS movement system detects boundary crossings and
swaps containers atomically.

**Why this satisfies infinite-of-x for homes:**
- Each home is its own RON file at `data/homes/<player-key>/layout.ron`
- Adding a home = drop in a file, no code change
- Players customize their home freely without touching anyone else's
- Homes are portable — transfer your home from Mothership-A to Mothership-B
  by reparenting the Vessel
- Mothership procedural generator allocates N "home slots" of bounded
  volume; each slot mounts a home Vessel

**Why this satisfies infinite-of-x for motherships:**
- Each mothership is a Vessel layout file
- Procedural mothership generator computes total floor area as
  `sum(homes) + common_area + utilities + bridge + cargo` and emits a
  layout that fits

### Precision at every scale (f64 budget)

The hierarchical container model preserves precision because every
`local_pos` stays small. f64 has ~16 significant decimal digits. Local
distances at every scale fit well within that:

**Revised 2026-05-10 — continuous positions everywhere.** Earlier draft
used chunk coordinates in Deep; that's gone. Every position is now a
continuous DVec3 in its parent frame.

| Scale | Local frame | f64 precision available |
|-------|-------------|------------------------|
| Inside a vessel (room → corridor) | meters from vessel origin (< 1 km) | sub-nanometer (10⁻¹³ m) |
| Within a system (Space) — outer planets | meters from barycenter (< 100 AU = 1.5 × 10¹³ m) | sub-millimeter (10⁻³ m) |
| In Deep — interstellar | light-years from galactic origin (< 100 kly) | sub-millimeter (~10⁻³ m) at galactic radius |
| Pocket dimensions | local coordinate space, semantics per-Pocket | configurable |

**The precision budget works at every scale because every local_pos is
expressed in the units natural to its parent.** Meters inside a vessel,
meters within a system, light-years at galactic scale. f64 gives 15-16
significant digits, which is ample whether the value is 5.0 (meters in
a room) or 27,400.0 (ly to the galactic center).

The 4-light-year trip from Sol to Alpha Centauri:
- Sol's galactic position: `(0.0, 0.0, 0.0)` ly
- Alpha Centauri's galactic position: `(-1.348, -3.972, -1.535)` ly
- During sublight FTL: ship's `galaxy_pos_ly` updates continuously from
  Sol's position toward Alpha Centauri's. No boundary crossings, no
  special bookkeeping.
- During blink drive: container_swap directly from `Space{"sol"}` to
  `Space{"alpha_centauri"}` — never enters Deep at all.

Floating origin handles render-side precision separately (everything
visible gets re-centered around the player to keep render coordinates
near zero — so even at 100 kly galactic position, what gets passed to
the GPU is small numbers near zero).

## 17a-bis. Locked decisions (2026-05-10 part 2)

Captured in the same session that landed Phase 2 of the implementation:

### Origin convention

**Technical origin: Sol at J2000.0**, fixed in space. Sol's galactic
motion (~0.0007 ly/yr around the galactic center) is ignored on game
timescales. All system positions in `data/star_systems/index.json` are
light-years from Sol.

**UI default center: Earth.** The map opens centered on Earth. Display
layer translates from the technical frame to whatever frame the user
needs.

This separation is deliberate: technical origin needs a fixed point
(orbital math, distances). UI origin can be wherever the player
expects (Earth, their ship, anywhere). They don't have to be the same.

For intergalactic (far future): when Sol-frame coordinates become
inconvenient (e.g. travel toward Andromeda where Sol-relative numbers
get awkwardly large), shift the technical frame to **CMB rest frame**
or **Local Group barycenter** — both are standard cosmology conventions.
The data model already supports this; only the loader needs an updated
frame-of-reference field.

### Rogue interstellar bodies = transient travel encounters

Operator 2026-05-10: *"I figured the rogue asteroids and stuff would
be temp objects in some situations. Like traveling between stars we
could spawn asteroids within the travel distance of the mothership/fleet."*

Locked. Replaces the original "infinite procedural rogue field" model
with something simpler and more practical:

- **Star systems**: persistent, in `data/star_systems/{id}.json`.
- **Persistent interstellar bodies**: small hand-authored set (named
  probes — Voyager 1/2, Pioneer 10/11; named drifting asteroids if
  astronomers find any; flavor content). Stored in
  `data/galaxy/persistent_drifters.json`. Small file, ~dozens of
  entries, not procedural.
- **Transient travel encounters**: when a vessel is in
  `Deep { galaxy_pos_ly }` actively transiting between systems, the
  FTL/travel system spawns ephemeral encounter content along the
  route — asteroids, debris, signal anomalies, derelicts. **Not
  persisted.** Each journey can have its own encounter density and
  difficulty.

Implications for the model:
- **No `rogue_state` mutation table needed.** A "mined" rogue during
  travel just yields its resources and the encounter is over.
- **No infinite-procedural-field complexity** for free-floating bodies.
- **Mark-and-return doesn't work for transients** — by design. Players
  who want a permanent reference point in interstellar space have to
  ask astronomers to add an entry to `persistent_drifters.json`
  (Voyager-style probes count) or wait until we ship marker buoys
  (deployable artifacts that anchor a coordinate).

This is in the spirit of the operator's earlier "we want fewer pages,
condensed simpler experiences" — it cuts a whole subsystem of
persistent-procedural-content that wasn't earning its complexity.

## 17b. Operator decisions (2026-05-09 session)

Locked in during the design discussion that produced this doc:

- **`ContainerRef` variants finalized**: `Vessel` (renamed from Ship —
  covers spaceships, cars, trucks, tanks, fighter jets, walking mechs,
  stations, buildings), `Body` (planet/asteroid/moon surface), `Space`
  (free-floating in a system), `Deep` (interstellar), `Pocket` (isolated
  dimension for tutorials / quest instances / tech demos). Alternate
  universes with their own galaxies are deliberately deferred — could
  add a `Dimension` variant at the top of the chain later.
- **Procedural + hand-authored bodies both supported** per body. Default
  is procedural (cheap, immediate); hand-authoring overrides procedural
  field-by-field. Earth ships first as procedural-Earth, gets hand-
  authored later when we have the data + tooling. Same body ID, two
  data sources.
- **SI units internally, display unit per context.** Storage and math
  use meters / seconds / kg throughout. Display layer (UI strings,
  tooltips, value formatters) converts to AU / ly / km / lightseconds /
  whatever reads naturally. Eliminates conversion bugs.
- **Sol restructure is safe** — operator confirmed nothing else depends
  on `data/solar_system/bodies.json`. Phase 1 already shipped (v0.199.0).

## 18. Pre-implementation checklist

Phase 1 status (2026-05-09):

- [x] **`ContainerRef` variants** — confirmed + extended: Vessel, Body,
      Space, Deep, Pocket (see §17b).
- [x] **Unit convention** — SI internally, display layer converts.
- [x] **Hand-authored AND procedural bodies** — both supported per body
      (procedural default, hand-authoring overrides field-by-field).
- [x] **Sol restructure safe** — no dependencies on the old path. Phase 1
      shipped in v0.199.0.
- [ ] **Universal sim_time vs fleet-controlled time** — under
      discussion; operator leaning toward universal. Pros/cons:
      | Universal | Fleet-controlled |
      |-----------|------------------|
      | Easy multiplayer (everyone agrees Mars is at the same place at sim time T) | Each fleet can fast-travel without affecting others |
      | Deterministic; replays / records work cleanly | More expressive (warp drive engaged → time accelerates for the crew) |
      | Fast travel still works via region-based speed multipliers | Hard sync; "what time is Mars?" depends on observer perspective |
      | Combat / shared scenes "just work" | Cross-fleet interaction across different speeds is undefined |
- [ ] **Anything missing** — see §17c for results of the doc audit.

---

## 19. References

- `docs/design/maps-multi-scale.md` — web-side render hierarchy (galaxy →
  street); continues to apply on top of this model
- `docs/design/infinite-of-x.md` — design rule this whole doc is in service of
- `src/renderer/floating_origin.rs` — existing floating origin
- `src/ship/` — ship layout parser (Fibonacci spiral generator)
- `src/ecs/` — hecs ECS where the position component will live
- `data/solar_system/bodies.json` — current single-system data, slated for
  restructure in Phase 1
