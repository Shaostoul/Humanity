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

## 17a-sexies. Time controls + seamless scale + AR overlays (2026-05-12)

Operator vision: *"could we add a way to watch the orbits play fast
forward, rewind, and skip/seek along days/weeks/years/etc? Like, if we
could have all the planets line up in real life as they are today on May
12, 2026 then we could see what it'd look like on a great conjunction.
Then they could see the conjunction in real life and in game. We also
would love to experience solar eclipses on the Earth ... the whole
experience to be seamless."*

This section lays out the sim_time / scrubber UX (shipped v0.208.0), the
sprite→mesh seamless-scale plan, the pill-style body labels with
expand-to-card, the AR FPS-mode overlay plan, and the conjunction +
eclipse detection plan.

### Universal sim_time clock — landed in v0.208.0

The cosmos page owns a `sim_time_seconds` clock measured in **seconds
since the J2000.0 epoch** (2000-01-01 12:00:00 UTC = UNIX timestamp
946,728,000). J2000 is the standard astronomical epoch — every orbital
element in `data/star_systems/sol.json` references it, so position math
is "advance the body's mean anomaly by `n × sim_time`" where `n = 360°
/ (period_days × 86,400)` is the mean motion.

State (added to `GuiState`):

```rust
pub cosmos_sim_time_seconds: f64,            // J2000-relative
pub cosmos_sim_speed: f64,                   // 0 = paused, 1 = real-time, 86400 = 1 day/sec
pub cosmos_last_real_instant: Option<Instant>,
pub cosmos_sim_time_initialized: bool,
```

On the first frame the System view is shown, `sim_time` is initialized
to **the user's real wall-clock time** (`SystemTime::UNIX_EPOCH` minus
J2000), so planets render where they actually are right now. From then
on, each frame:

```rust
let dt_real = now - last_real_instant;
sim_time_seconds += dt_real * sim_speed;
```

Pause = `sim_speed = 0`. Reverse = negative speed. Step = bump
`sim_time_seconds` by a constant (1 day = 86,400 s, 1 week = 604,800 s,
1 year ≈ 31,557,600 s). All body positions are pure functions of
`sim_time`, so changing it freely re-renders the whole system without
any state to invalidate.

### Time-controls UI — landed in v0.208.0

A horizontal control strip at the top of the System view contains:

1. **Date display** — formatted via Howard Hinnant's days-from-civil
   algorithm (no `chrono` dependency, accurate for any proleptic-
   Gregorian date). Shows `YYYY-MM-DD HH:MM UTC` with a hairline
   monospace render.
2. **Now button** — snaps `sim_time` back to real-world wall clock.
   Critical for the operator's stated use case ("see what it looks
   like today, May 12 2026").
3. **Transport** — ⏮ rewind 1 yr / ⏪ rewind 1 mo / Play|Pause / ⏩
   advance 1 mo / ⏭ advance 1 yr (using only **lint-safe glyphs**:
   plain ASCII ←/→ via the keyboard fallback when needed).
4. **Speed presets** — 1 h/s · 1 d/s · 1 mo/s · 1 y/s · 10 y/s. Each
   button sets `sim_speed` directly; the current preset highlights.
5. **Reverse toggle** — flips the sign of `sim_speed` (works with any
   preset, so "Reverse + 1 y/s" reads as "−1 year per real second").
6. **Scrubber slider** — drag along a ±10-year range centered on today
   to seek to any date. Releases of the drag snap `sim_time` to the
   chosen offset from real-now. Hold Shift to drag at hour-precision;
   default is day-precision.

> Glyph note: avoid the `U+FE0F` variation selector and `Dingbats`
> tofu hazards (see `tests/icon_glyph_lint.rs`). The transport icons
> use either plain text labels (`"Play"`, `"Pause"`, `"<<"`, `">>"`)
> or paint shapes via `widgets::icons::paint_*` SVG helpers.

### Universal "body pill + info card" widget (Phase 4d-bis + extraction in v0.213.0)

Operator insight 2026-05-12: *"This planet icon pill to card GUI thing could be used at the dedicated page, in the player home map room, and like when we're looking at the starry night sky with AR glasses."*

The pill-to-card pattern was first built inline on the Cosmos page (v0.209.0), but the same data shape works on every surface that needs to label celestial bodies:

| Surface | Canvas | Screen-position source |
|---------|--------|------------------------|
| Cosmos page (shipped) | egui Painter on the page's central rect | `project_to_screen` via the 3D camera (yaw/pitch/distance + perspective) |
| In-ship Map Room HUD (planned) | egui Painter on top of the FPS viewport | Player's first-person camera view + projection (the room is a hologram tank — bodies project onto a contained volume around the player) |
| AR-glasses sky overlay (planned, Phase 4g) | Painter on a camera-passthrough texture | AR headset's pose + projection matrices |

In every case the compute is identical: project a body's world position to 2D screen, render a pill at that anchor, hit-test, optionally expand into an info card. Only the source of `screen_pos` differs.

**Extracted to `src/gui/widgets/body_pill.rs` in v0.213.0.** The widget owns the visual layout (collision-dodge, rounded-rect bg with theme tokens, info-card auto-position with canvas clamping) so callers just produce input data and consume the response.

Public API:

```rust
pub struct BodyPill<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub color: Color32,
    pub body_screen: Pos2,
    pub body_radius_px: f32,
    pub priority: u8,     // collision-dodge sort key
    pub forced: bool,     // hover/select/expanded — never hidden
    pub expanded: bool,   // styling differs (accent border)
}

pub fn place_and_draw_pills(
    ui: &mut Ui,
    painter: &Painter,
    theme: &Theme,
    pills: &[BodyPill],
    interact_id_salt: &str,  // scopes interaction ids per-canvas
) -> PillsLayout {
    pub placed: Vec<PlacedPill { id, rect }>,
    pub clicked_id: Option<String>,
}

pub struct BodyCardData<'a> {
    pub heading: &'a str,
    pub color: Color32,
    pub subtitle: Option<String>,
    pub stats: Vec<(String, String)>,
    pub description: Option<&'a str>,
    pub actions: Vec<(String, bool)>,  // (label, enabled)
}

pub fn draw_body_card(
    ui: &mut Ui, painter: &Painter, theme: &Theme,
    data: &BodyCardData,
    body_screen: Pos2, body_radius_px: f32,
    canvas_rect: Rect,
) -> BodyCardResponse { closed, action_clicked: Option<usize> }
```

The Cosmos page now uses these widgets; the v0.209.0 inline code (~200 lines) moved verbatim into the widget module. Future surfaces (Map Room HUD, AR-glasses) will produce their own `Vec<BodyPill>` + `BodyCardData` from their domain types (ECS components / AR headset metadata) and consume the same response shape.

Why this matters for *infinite-of-x*: a label widget that's hardcoded to one surface is the start of a UI maintenance nightmare. By the time we ship three "list a celestial body" surfaces, any inconsistency between them (a different border-radius here, a different connector-line angle there) becomes user-visible noise. One widget = one source of truth, restyleable via the theme editor.

### Pill-style body labels with expand-to-card (Phase 4d-bis)

Operator: each label should be a **pill UI — planet sprite on the
left, name on the right, click expands to an info card.**

Current Phase 4 renders each body as a colored dot plus a text label
adrift in canvas space. The pill replaces the dot+label with a single
rounded-rect widget:

```
┌────────────────┐
│ 🪐  Jupiter    │   ← collapsed (~24 px tall)
└────────────────┘

┌─────────────────────────────────┐
│ 🪐  Jupiter                     │
│ Gas giant · 5.20 AU · 11.86 y   │   ← expanded card
│ 79 moons · Galilean: Io, Europa,│
│ Ganymede, Callisto · Great Red  │
│ Spot · No solid surface         │
│ [Focus] [Track]  [Open page]    │
└─────────────────────────────────┘
```

Implementation: each body draws its own pill via egui's
`Frame::popup` + `Sense::click()`. State for "which body is expanded"
lives in `GuiState::cosmos_expanded_body: Option<String>`. Only one
pill expands at a time. The card content reuses the existing sidebar
detail-panel renderer (DRY). `Open page` jumps to a body-specific
wiki page if available.

Pills also avoid label-overlap collisions — when two bodies are
within label-bbox of each other on screen, the smaller-magnitude
body's pill hides until the camera pans or one of them is clicked
(similar to KSP's overlap dodge). Implementation uses egui's painter
to layout pills, then a per-frame `Vec<Rect>` is checked for overlap
before each pill is drawn.

### Sprite-to-mesh seamless LOD (Phase 4b)

Operator: *"2D sprite at distance, transitions to 3D mesh when close
enough (seamless loading)."*

Bodies render in three regimes depending on **screen-space radius**
(pixels covered):

| Regime | Screen radius | Render |
|--------|---------------|--------|
| Far | < 2 px | A single bright dot, magnitude-scaled. ~119k stars from `data/cosmos/stars.csv` (Hipparcos catalog, planned Phase 4b) render exclusively in this regime. |
| Sprite | 2 – 64 px | Pre-rendered RGBA texture of the body (round disk with shading, low-cost). Loaded from `assets/bodies/<id>.png`, falls back to procedural disk colored by `SolBody::color`. Lit by the Sun's screen-position via a single directional-light shader uniform. |
| Mesh | ≥ 64 px | Full 3D textured sphere mesh — wgpu pipeline, PBR shader, planet texture + normal + roughness maps, sun-as-directional-light. |

The transition zone (around 64 px) cross-fades sprite-α down to 0 as
mesh-α ramps to 1 over ~10 frames, hiding the swap. Below ~4 px the
sprite also cross-fades into the bright dot.

Mesh data is **streamed**: first time a body crosses the 64-px
threshold, the engine kicks off an async load of its mesh + textures
from `assets/bodies/<id>/`. While loading, the sprite continues to
render. Once loaded, the cross-fade begins. Unloads are lazy —
meshes outside a ring of ~3× their pop-in radius get evicted after
30 s of inactivity.

This integrates with **Phase 4b's wgpu-in-egui-canvas integration**
(currently the System view is pure egui `Painter` 2D, no wgpu). Two
viable paths:

1. **Render to texture, blit into egui** — render wgpu scene to an
   offscreen texture, then `Painter::image()` it as a layer. Egui
   draws pills + labels on top. Simpler, slight latency cost.
2. **Custom egui callback** — `egui_wgpu::CallbackTrait` lets us
   inject a wgpu pass mid-paint. Lower latency, more wiring.

Path 1 ships first because it's strictly simpler and the latency cost
is invisible at 60 fps. Path 2 is a later optimization if needed.

### FPS AR-glasses overlays (Phase 4g)

Operator: *"FPS AR-glasses style overlays — faintly see orbital paths
while in FPS mode, with asteroid trajectories highlighted (no Blender-
style controls, just there visually)."*

In FPS mode (player walking on a ship deck or planet surface), the
HUD overlays:

- **Orbital paths** of nearby bodies, projected onto the player's
  screen via the camera transform. Drawn with **low alpha** (~10%)
  and a **subtle pulse** so they don't fight the scene. The same
  Kepler solve that draws the Cosmos page generates these — the
  shader just projects (x,y,z)_sim into (u,v)_screen using the
  player camera's view + projection matrices.
- **Asteroid trajectories highlighted** when an NEA or other tracked
  body is on a near-Earth approach. Color brightens with proximity
  ('near' < 100 lunar distances pulls a thicker line + label).
- **Conjunction predictions** (see next section) shown as a discrete
  "next event" badge with a countdown timer.

No controls — overlays are purely informational. A single FPS HUD
toggle (`H` key, already used for the HUD) cycles: full HUD → minimal
HUD → no HUD. Orbit overlays are part of the full HUD only.

Performance: nearby orbital paths only — never the full ~10k Tier-1
asteroids, only those within 0.5 AU of the player. Filter by
sphere-of-influence (Phase 4d): only show bodies whose SoI we're in
or adjacent to.

### Conjunction + eclipse detection (Phase 4d-tri)

A **conjunction** is when two bodies appear close in the sky from a
third observer's viewpoint. Two key angles:

1. **Geocentric angular separation** between two bodies as seen
   from Earth — for a great conjunction (Jupiter-Saturn 2020, etc.)
2. **Heliocentric longitude** — when planets are at the same orbital
   longitude (true syzygy)

A **solar eclipse** is the special case where the Moon's angular size
covers the Sun's from Earth, with the Moon's shadow tracing a path on
Earth's surface.

Algorithm (planned Phase 4d-tri):

```rust
fn angular_separation(observer: DVec3, a: DVec3, b: DVec3) -> f64 {
    let to_a = (a - observer).normalize();
    let to_b = (b - observer).normalize();
    to_a.dot(to_b).acos()    // radians
}
```

For conjunctions: every N sim_time-seconds (N adaptive, ~1 game-day at
1× speed), check every (body_a, body_b) pair from Earth's POV. If
their angular separation drops below a threshold (~1° = "conjunction",
~0.1° = "tight conjunction", ~0° = "occultation"), emit a
`ConjunctionEvent` with the bodies, the angle, and the sim_time.

For eclipses: a solar eclipse happens when the **Moon's center**
appears within the **Sun's angular radius** as seen from a point on
Earth's surface. The umbra path is the locus of Earth-surface points
where the Moon fully occludes the Sun. Real-world ephemerides match
this to ~1 km accuracy; ours should match to ~100 km (visible-from-
this-continent precision) at first.

UI surface: a **"Sky Events" panel** in the Cosmos page listing
upcoming events. Click → set sim_time to ~1 hour before the event,
play at 1 h/s, watch it unfold. From FPS mode, the next event shows
as a HUD badge.

Eclipses also drive **gameplay**: the day/night system already exists
(`src/systems/time.rs`); an eclipse darkens the sky over the umbra
path. The ECS reads the cosmos sim_time → checks if Earth's player
position is under the Moon's shadow → applies a sky-darkening
modulation. Players in the umbra at the right sim_time see totality
in-game; their location on the umbra map is shareable. *Compelling
because it ties the game's clock to a real-world astronomical event
that millions of people will be experiencing on the same day.*

### What ships when (updated)

| Phase | Scope | Status |
|-------|-------|--------|
| 4a | 3D camera + perspective + cosmetic moon rings | ✅ v0.206.0 |
| 4c | Real Kepler orbital elements + ellipses + asteroid subcat | ✅ v0.207.0 |
| 4d-time | sim_time clock + time controls + scrubber + Now button | ✅ v0.208.0 |
| 4d-bis | Pill-style body labels with expand-to-card | ✅ v0.209.0 |
| 4d-tri | Conjunction + eclipse detection (current-moment, no scan) | ✅ v0.210.0 |
| 4d-lag | Lagrange points overlay (L1-L5 for 5 interesting pairs) | ✅ v0.211.0 |
| 4d-ref | Reference-orbit rings (LEO/MEO/GEO/HEO around Earth + analogues for Mars/Jupiter/Moon) | ✅ v0.212.0 |
| 4d-soi | Sphere-of-influence sort (dynamic categorization for transient objects) | When ships/stations/dropped items move | 
| 4d-quad | Eclipse umbra path on Earth's surface + Sky Events scan | After 4d-soi |
| 4e | Tier-1 minor-body catalog (~10k) + streaming spatial index | After 4d |
| 4f | Trajectory overrides + N-body impact for story arcs | When a story arc is being authored |
| 4b | Sprite→mesh seamless LOD + wgpu-in-egui-canvas | When visual fidelity is worth the integration cost |
| 4g | FPS AR-glasses orbit-path overlays | After 4b ships |

### Phase 4d-ref implementation notes (landed v0.212.0)

- `reference_orbits_for(body_id)` returns a `&'static [ReferenceOrbit]` for any body that has named navigation orbits. Currently Earth (LEO low/high, MEO, GEO, HEO, Lunar TLI), Mars (LMO, areostationary), Jupiter (low Jovian, Jovo-stationary), and the Moon (low lunar orbit, NRHO apoapsis). Empty for everything else — no rings draw on Pluto, the Sun, asteroids, etc.
- Rings render as 96-segment polylines in the body's local XY plane (currently approximated as the ecliptic plane; real equatorial planes for tilted bodies like Earth's 23.4° obliquity are a later refinement).
- Each ring is labeled at the screen-position of its theta=0 (right-side) point. Hovering near any ring segment surfaces a tooltip with the ring name and a one-line blurb explaining its role ("ISS altitude — 90 min period", "GPS / GNSS satellite belt — 12 h period", etc.).
- Threshold: rings only render when the body's apparent radius is > 12 px on screen — below that the rings would overlap the body itself or be sub-pixel.
- Toggle button next to the Lagrange toggle in the top-right of the canvas, off by default.
- All theme tokens (info color for rings + labels).
- **Note**: Phase 4d-soi (SoI-based dynamic categorization in the sidebar) was originally bundled with this work but split out — SoI sort only matters for transient objects (ships, stations, dropped items) which don't exist in the cosmos data yet. The infrastructure (`compute_lagrange_points` already uses mass ratios — same math as `r_hill = a * cbrt(mu/3)` from which the SoI parent walk derives) is in place; the sidebar grouping just doesn't need to change today.

### Phase 4d-lag implementation notes (landed v0.211.0)

- Five Lagrange-pair entries hardcoded at the top of `cosmos.rs` (`LAGRANGE_PAIRS`): Sun-Earth, Earth-Moon, Sun-Mars, Sun-Jupiter, Sun-Saturn. Each entry carries a `pair_label`, the `parent_id`/`child_id`, and a small list of `notable` parking (JWST at Sun-Earth L2, Greek Trojans at Sun-Jupiter L4, etc.). Could move to a data file when the list grows past ~10 pairs; for now a const is fine.
- `compute_lagrange_points(parent, child, sim_time)` returns `[DVec3; 5]` in heliocentric AU. L1/L2/L3 use the cube-root-of-mass-ratio approximation (accurate to ~1% for μ < 0.01, covers all current pairs). L4/L5 are exact 60°-ahead/behind equilateral points; rotation uses Rodrigues' formula about the orbit-plane normal (approximated as the ecliptic normal for now — sub-arcminute error for the ~5° tilted Earth-Moon).
- Toggle button overlaid in the top-right of the canvas — "Lagrange: ON/OFF". Default OFF so the wide view stays clean.
- Each point renders as a small `×` (two crossed line segments) + a center dot + label when zoomed in (parent-child screen-space distance > 50 px, otherwise labels overlap). Hover gives a tooltip with the pair label, notable parking, and a one-line explainer of what that L-point is.
- All theme tokens; no hardcoded colors.
- **Limitation**: orbit-plane normal is approximated as ecliptic for L4/L5. For tilted orbits (lunar, asteroid systems) the L4/L5 markers will be a few arcminutes off true. Trivial to fix later by deriving normal from the body's `inclination_deg` + `longitude_ascending_node_deg` — deferred until it's visibly wrong.

### Phase 4d-tri implementation notes (landed v0.210.0)

- O(n²) pairwise scan of 11 named bodies (~55 pairs) per render frame. Each pair computes one acos for angular separation from Earth's barycentric position. Cheap enough to run every frame even as the user scrubs.
- Conjunction tightness tiers — Close (< 5°), Conjunction (< 1°), Tight (< 0.5°), Occultation (< 0.1°). The tightest one any body is involved in drives its highlight ring color (warning tone) and ring thickness.
- Solar eclipse detector: Sun-Moon apparent-radius math from Earth's POV. If disks overlap, classify Partial / Annular / Total based on whether the Moon disk is fully inside the Sun disk and whose apparent radius is larger. Coverage fraction approximated linearly from separation/(sum-of-apparent-radii); good enough for the HUD.
- Visual treatments — Sun gets a dark disc overlay scaled by sqrt(coverage); Moon gets a danger-tone outline; Earth gets a thick danger-tone ring + "ECLIPSE" badge.
- HUD readout in the bottom-right corner of the canvas: section header "Sky events (from Earth's POV)" + up to 3 tightest conjunctions + any eclipse line. Empty state reads "Sky is quiet (no conjunctions within 5°)".
- **Limitation**: detection is current-moment only — there's no scan for "next eclipse" or "next great conjunction". Users have to scrub sim_time manually to find events. Phase 4d-quad will add a forward-search button + Sky Events panel listing upcoming events within a user-set window.
- **Limitation**: eclipse detection is "is the Moon shadow geometry such that an eclipse exists somewhere on Earth?", not "is the umbra over this lat/lon?". The latter requires Earth's rotation phase + the observer's surface position; that ships in Phase 4d-quad along with in-game day/night system integration (sky-darkening over the umbra during real-world events).

---

## 17a-quinque. Real astronomy, infinite asteroids, and story-arc support (2026-05-10)

Operator vision: "as close to realistic as we can ... eventually include
a crazy story arc that involves an alien species redirecting a planet
to crash into another planet. Maybe we should also include stuff like
lagrange points and ... geostationary orbit ... sort them in a nested
list based on distance from main gravitational body ... include
theoretically infinite planets/asteroids/etc. ... include all the
real-life asteroids ... seamless."

### Real orbital mechanics — landed in v0.207.0

Every body's position is now computed from its real orbital elements:
**semi-major axis, eccentricity, inclination, longitude of ascending
node, argument of perihelion, mean anomaly at epoch**. Snapshot
positions today (mean anomaly fixed at J2000); Phase 4d advances mean
anomaly over sim_time so bodies actually orbit. Kepler's equation
solved via Newton-Raphson per render. Orbit ellipses now show real
eccentricity and inclination (Pluto's tilted ~17° orbit, Ryugu's
elongated path that crosses Earth's orbit at perihelion, etc.).

### Asteroid sub-categorization — landed in v0.207.0

Sidebar groups asteroids by region — **Near-Earth Asteroids** (a < 1.3 AU,
e.g. Ryugu, Bennu, Itokawa, Eros — these were what confused users when
lumped with main-belt rocks), **Main Belt** (1.3 ≤ a < 4 AU, e.g. Vesta,
Pallas, Juno, Psyche, Hygiea), **Trans-Neptunian** (a ≥ 4 AU, currently
empty — will populate when we add Kuiper Belt objects + scattered disk
+ Sedna-class bodies). Buckets follow standard planetary-science
convention. Empty sub-regions are hidden so the sidebar doesn't grow
to show categories we have no data for.

### Sphere-of-influence sorting (planned Phase 4d)

Currently bodies are categorized by their direct parent (Sun for most,
the planet for moons). The operator's request: "sort them in a nested
list based on distance from main gravitational body — if near Earth,
it'd be Earth but out far enough it'd just become the sun."

This is the **Hill sphere / sphere of influence (SoI)** concept from
real astrodynamics. A body's SoI radius is approximately
`a × (m_body / m_parent)^(2/5)`. Inside that radius, the body's gravity
dominates; outside, the parent's does. Practical effects:

- An object 100,000 km from Earth is in Earth's SoI → list it under "Earth"
- The same object 2 million km from Earth is outside Earth's SoI → list it under "Sun"
- A ship in transit between systems is in interstellar space (no SoI) →
  list it under the galactic frame

Implementation plan for Phase 4d:
- Each body gets a derived `hill_sphere_au` field (computed once at
  load time from mass ratios)
- A spatial query "what's the deepest SoI this point is inside?" walks
  the parent chain. For a position P:
  1. Start at the top (Sun)
  2. For each child of the current parent, if P is inside that child's SoI,
     recurse into the child
  3. The deepest match is P's effective gravitational parent
- The sidebar then groups dynamically: each entry's category is its
  SoI-parent, not its fixed orbital parent
- Useful for ships, stations, missions, marker buoys, dropped items —
  anything that moves

### Lagrange points (planned Phase 4d)

Five gravitational sweet spots in any two-body system (Sun-Earth, Earth-
Moon, Sun-Jupiter, etc.). L1/L2/L3 are unstable (need active station-
keeping); L4/L5 are stable (objects naturally accumulate there — Jupiter's
**Trojan asteroids** at Sun-Jupiter L4 and L5 are the classic example).

For HumanityOS this is a navigation + content opportunity. JWST sits at
Sun-Earth L2. SOHO sits at Sun-Earth L1. Future colonies / megastructures
in stable orbits use L4/L5. Real positions are formulas:
- L1, L2, L3: along the line through the two bodies, computed via the
  five-roots-of-a-quintic (numerical solve)
- L4, L5: 60° ahead / behind the smaller body in its orbit

Phase 4d will:
1. Add a "Lagrange Points" overlay toggle in the Cosmos page
2. Compute L1-L5 for selected interesting pairs (Sun-Earth, Earth-Moon, Sun-Mars, Sun-Jupiter)
3. Render them as small × markers with labels
4. Make them clickable (open details: "Sun-Earth L2 — JWST's home, ~1.5M km from Earth")

### Reference orbits (planned Phase 4d)

Standard altitudes / orbits commonly used IRL — show them as concentric
rings around a focused planet when the camera is close enough:

| Orbit | Altitude above Earth | Period |
|-------|---------------------|--------|
| **LEO** (Low Earth Orbit) | 160 – 2,000 km | 90 – 130 min |
| **MEO** (Medium Earth Orbit) | 2,000 – 35,786 km | 2 – 24 h |
| **GEO** (Geostationary) | 35,786 km | 24 h (matches Earth's rotation) |
| **GSO** (Geosynchronous) | 35,786 km | 24 h (inclined variants of GEO) |
| **HEO** (High Earth Orbit) | > 35,786 km | > 24 h |
| **Lunar transfer** | ~384,400 km apoapsis | 5 days transit |

Same approach scales to other bodies: Mars-GSO is at a different altitude
(20,427 km because Mars's day is 24.6 hr but its gravity is weaker).
Useful for satellite gameplay, station placement, navigation training.

### Theoretically infinite asteroids (planned Phase 4e)

Operator: "If we could add all the real-life asteroids to our video
game then people could naturally explore our real solar system and see
what places people end up establishing permanent bases in game
throughout our solar system."

The JPL Small-Body Database has **~1.3 million known asteroids** with
catalogued orbits. Bundling all 1.3M is impractical — the data alone
is hundreds of MB, render cost is huge — but we don't have to bundle
or render them all at once. Streaming strategy:

1. **Tier-0 (always loaded, ~64 bodies)**: planets, moons, dwarf
   planets, named/notable asteroids (Ceres, Vesta, Eros, Ryugu, etc.).
   Stays in `data/star_systems/sol.json`. Today's behavior.
2. **Tier-1 (region streaming, ~10k bodies)**: every named or
   numbered minor body within a given AU range. Stored in
   `data/star_systems/sol/minor_bodies.csv` (~5-20 MB). Loaded on
   first cosmos-page open into a spatial index.
3. **Tier-2 (full catalog, ~1.3M bodies)**: full JPL SBDB. Stored as a
   separate downloadable asset bundle (`humanity-asteroids.tar.zst`,
   ~50-100 MB). User opts in via Settings. Loaded into a chunked
   spatial index — only chunks near the camera get bodies loaded.
4. **Tier-3 (procedural fill)**: between tier-2 catalogued bodies,
   procedural minor bodies generated from `hash(seed, position)` so
   density looks physically reasonable (BAO correlation baked in for
   any galactic-scale generation; for in-system asteroids the
   distribution follows the real heliocentric density profile).

Rendering: at solar-system zoom, only show bright/notable ones. As the
camera zooms into a region, more catalog bodies pop in. As you zoom into
a single asteroid, its procedural surface (Phase 4b/5) loads. **Seamless
transition from "the asteroid belt is a vague cloud" at wide zoom to
"this specific rock is here, mineable, with these surface features" at
close zoom.**

Gameplay payoff: players establish bases on real asteroids (Ryugu has
real-world significance — JAXA visited it; Psyche is a planned NASA
target; Ceres might harbor sub-surface water ice). Test-flight + FTL
training between named locations becomes real navigation practice.

### Trajectory overrides for story arcs (planned Phase 4f)

Operator: "an alien species redirecting a planet to crash into another
planet" — supported by treating orbital elements as **mutable per-body
state**, not immutable physics constants.

Data model: each body has its *default* orbital elements from the data
file. The relay can broadcast a **trajectory override** event:

```json
{
  "type": "trajectory_override",
  "body_id": "mars",
  "effective_sim_time": 1234567890000,
  "new_elements": {
    "semi_major_axis_au": 1.524,
    "eccentricity": 0.9,        // alien deflection cranks up eccentricity
    "inclination_deg": 25.0,
    // ... new mean anomaly, etc.
  },
  "narrative": "Alien dreadnought-class vessel deploys gravitational manipulation array; Mars orbit destabilized."
}
```

Every client applies the override to their local SolBody state. From
that sim_time forward, Mars renders along its new orbit instead of its
default. Persistence: overrides stored in the relay's `body_overrides`
table so a fresh connect catches up.

For the actual "planet crashes into planet" arc: we'd add
**N-body simulation** for the affected bodies during the impact window
(~weeks of sim_time), then snap back to Kepler orbits for the survivors.
Computationally expensive but bounded — only the 2-3 bodies in the
collision region need N-body; everything else stays Kepler.

This whole subsystem is **infinite-of-x compatible** — story arcs are
just data files describing trajectory overrides keyed by sim_time. New
arcs ship as JSON, no code change.

### What ships when

| Phase | Scope | Status |
|-------|-------|--------|
| 4a | 3D camera + perspective + cosmetic moon rings | ✅ v0.206.0 |
| 4c | Real Kepler orbital elements + ellipses + asteroid subcat | ✅ v0.207.0 |
| 4d | Sphere-of-influence sort + Lagrange overlay + reference-orbit rings + sim_time orbit evolution | Next |
| 4e | Tier-1 minor-body catalog (~10k) + streaming spatial index | Soon after |
| 4f | Trajectory overrides + N-body impact simulation for story arcs | When a story arc is being authored |
| 4b | wgpu-in-egui-canvas for textured planets / sun lighting / skybox | When the visual fidelity is worth the integration cost |

## 17a-quater. Cosmos UI: 3D System view shipped (Phase 4, v0.206.0)

### Phase 3 was 2D top-down (replaced)

The Cosmos page renders 2D top-down using egui's `Painter`. Three views
share one canvas: System (Sol's planets, AU scale), Galactic (Sol-centered
nearby stars, ly scale), Night Sky (Earth-centered RA/Dec celestial sphere
with constellations). Pan + zoom + click-to-select via the body browser
sidebar.

**The limitation** (operator question 2026-05-10): in a strict top-down
2D projection, a moon directly "below" its planet would render on top of
the planet and be unclickable. Same for any spaceship at an inclined
orbit, or anything off the ecliptic plane.

**Mitigation in Phase 3:** moons render in a small **cosmetic ring**
around their parent planet rather than at scaled true positions —
guarantees every moon is visible and clickable regardless of the
real orbital geometry. This loses positional accuracy on purpose
(scale would put moons sub-pixel close to their planet anyway) but
keeps every body interactive. Real orbital positions of moons within
their parent's frame land in Phase 7 alongside live sim_time gossip,
and are presented via planet-zoom (clicking a planet zooms into its
moon system at AU-fraction scale).

### Phase 4 (v0.206.0) — pseudo-3D System view shipped

The System view is now a true 3D scene:
- **`Cosmos3DCamera`** with target / yaw / pitch / distance — turntable
  camera matching Blender / KSP / Star Citizen conventions.
- **Real 3D body positions** — planets at their actual semi-major-axis
  distances + slight inclination wobbles. Moons at their actual
  km-scale offsets relative to their parent planet (Earth-Moon = 384,400 km
  = 0.00257 AU = sub-pixel at solar-system scale, but visible as soon
  as you zoom in close).
- **Perspective projection** — apparent body size scales with depth.
  Pluto looks tiny when viewed from above the ecliptic; Earth looks
  big when you zoom into it.
- **Mouse interaction:**
  - Drag = rotate camera (yaw + pitch)
  - Scroll = zoom (multiplicative, multi-orders-of-magnitude range)
  - Shift+drag or middle-drag = pan target across the scene
- **Orbit ellipses** drawn as 96-segment polylines through 3D-projected
  points (so they correctly tilt as the camera rotates).
- **Auto-zoom on focus** — clicking Phobos in the sidebar moves the
  camera to Mars + zooms to 8× Phobos's orbital radius (so the moon
  is meaningfully visible). Clicking Earth zooms to ~0.3 AU view.
- **Depth-sorted draw order** — bodies sorted back-to-front so closer
  ones occlude farther ones.

The 2D Galactic + Night Sky views stay in 2D — those are inherently
flat presentations (galactic plane projection, celestial sphere
projection). 3D would add nothing.

### What's still missing — Phase 4b (textures + lighting)

The current implementation uses egui's 2D `Painter` with perspective
math. Bodies are flat-shaded colored disks. Phase 4b adds:
- **wgpu-in-egui-canvas integration** — render a real 3D scene to a
  texture inside the cosmos canvas using egui's `PaintCallback`
- **Textured planets** — Earth's coastlines, Mars's surface, Jupiter's
  bands. The existing renderer pipeline already has these; just needs
  to be hooked into the cosmos panel.
- **Sun lighting** — phase shadows (the night side of Earth is dark).
- **True skybox** — the 119k stars from `data/stars.csv` rendered as
  a backdrop sphere instead of a flat dark color.
- **Real orbital mechanics** — Kepler's equations evolved over sim_time
  so planets actually move along their orbits.

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

## 17a-ter. Cosmological structure + intergalactic frame (2026-05-10 part 3)

For the eventual move to intergalactic gameplay (when Sol-frame
coordinates become awkwardly large), here's what real cosmology gives
us. Most of this also has a teaching role — these concepts get
glossary entries so the UX/UI can surface them naturally on hover.

### Comoving coordinates — the intergalactic frame

In an expanding universe, two galaxies' physical distance grows over
time even when neither is "moving." The standard solution is **comoving
coordinates**: a reference frame that stretches with the universe's
expansion, so a galaxy at fixed comoving position stays at that
position even as the physical distance to other galaxies grows.

We adopt comoving coordinates whenever multi-galaxy gameplay arrives.
The data model is unchanged — `galaxy_pos_ly: DVec3` still works; we
just interpret the values as comoving distances and apply the cosmic
scale factor `a(t)` when converting to physical distance for any
particular sim_time. For everything inside a single galaxy (where we
work today), the distinction doesn't matter — Hubble expansion is
negligible at galactic scale.

### Baryon Acoustic Oscillations (BAO)

In the first ~380,000 years after the Big Bang, the universe was a
hot dense plasma where pressure waves (literal sound) propagated.
When the universe cooled enough for atoms to form, photons decoupled
from matter, the pressure dropped to ~zero, and the sound waves
froze in place at their then-current radius — about 150 megaparsecs
in today's expanded coordinates.

Galaxies today show a slight statistical preference (~1%) for being
that distance apart from each other. **BAO are not navigable
structures** — they're a statistical correlation pattern across the
entire galaxy distribution, not bubbles you can fly into. They're
useful as a "standard ruler" for measuring the expansion of the
universe at different epochs.

**For us:** when we eventually generate procedural galaxy
distributions (multi-galaxy gameplay, cosmic web visualizations),
**bake the BAO correlation into the distribution** so it looks
physically realistic rather than uniform random. Real galaxy survey
maps have a faint ~150-Mpc bump in their two-point correlation
function; ours should too.

### Cosmic voids and the cosmic web

The visible bubble-like structure of the universe: galaxies cluster
in **filaments** and **walls** around vast underdense **voids**.
Examples: Boötes Void (~330 Mly across), Local Void (~150 Mly).
Together they form the **cosmic web** — the largest visible structure
in the universe.

These look like bubbles in 3D maps, but they don't have well-defined
centers. They're irregular blob shapes, fractal-like (sub-voids in
voids), and they expand with the universe. Useful as visual content
in the multi-scale map; not useful as coordinate origins.

### CMB rest frame

The cosmic microwave background looks isotropic (the same in every
direction) only from one particular reference frame — the **CMB rest
frame**. Earth is moving relative to it at ~370 km/s, which we can
measure from the CMB's dipole anisotropy.

This is the closest thing the universe has to an "absolute rest
frame." When we adopt comoving coordinates for intergalactic gameplay,
the CMB rest frame is the natural anchor for the frame's velocity
calibration. Practical effect on the game: invisible. Educational
effect: a fun thing to surface in glossary tooltips.

### Educational opportunity

All of these concepts get glossary entries (`data/glossary.json`,
"cosmology" category) so the in-app dictionary widget
(`widgets::definition_text` / Alt+hover) surfaces them naturally
wherever they appear in the UI. When the user opens a galaxy map
and sees "Local Group" or "Boötes Void" or "comoving coordinate frame"
in a label, holding Alt + hovering should give them a 2-3 sentence
explanation. This is part of the broader "teach naturally through UX"
goal — every novel concept the app exposes should be one hover away
from a definition.

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
