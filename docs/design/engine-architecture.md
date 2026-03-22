# HumanityOS v1.0 Game Engine Architecture

**Status:** Active Reference Document (design target for v1.0)
**Author:** Shaostoul + Claude
**Date:** 2026-03-18 (updated 2026-03-21 for v0.34.0 implementation status)
**Supersedes:** Project Universe approach
**Depends on:** [game-engine.md](game-engine.md) (engine decision), [graphics-pipeline.md](graphics-pipeline.md) (renderer design), [audio-engine.md](audio-engine.md) (audio stack), [educational-gameplay.md](educational-gameplay.md) (skill system philosophy)

> **Implementation vs Design:** This document describes the v1.0 target architecture. As of v0.34.0,
> the actual implementation lives in `engine/src/` (not the sub-crates in `engine/crates/` which are
> mostly scaffolds). Key differences from this design:
> - **ECS**: Uses `hecs` crate, not a custom archetypal ECS
> - **System runner**: Simple `System` trait with `tick(world, dt, data)`, no parallel scheduling yet
> - **Renderer**: Forward PBR-lite (not deferred), instanced batching, GLTF loading
> - **Terrain**: Icosphere planets with LOD + voxel asteroids with sparse octree (both implemented)
> - **Physics**: Rapier3d rigid bodies and colliders (fluid/fire/structural are scaffolds)
> - **Audio**: Module exists but not wired up yet
> - **Data loading**: AssetManager with CSV/TOML/RON/JSON/GLTF, FileWatcher hot-reload (working)
> - **Game systems**: 15 systems registered and ticking (farming, AI, vehicles, quests, ecology, etc.)
> - **Ship interiors**: Layout parser and room mesh generation from RON data files

---

## Table of Contents

1. [Design Principles](#1-design-principles)
2. [Crate Architecture](#2-crate-architecture)
3. [Hot-Reload System](#3-hot-reload-system)
4. [Construction System (Parametric CSG)](#4-construction-system-parametric-csg)
5. [Ship Systems (Real Technology)](#5-ship-systems-real-technology)
6. [Fluid and Physics Simulation](#6-fluid-and-physics-simulation)
7. [Supply Chain and Logistics](#7-supply-chain-and-logistics)
8. [Off-Screen Autonomy](#8-off-screen-autonomy)
9. [Multi-Scale Navigation](#9-multi-scale-navigation)
10. [Data File Formats](#10-data-file-formats)
11. [Educational Integration](#11-educational-integration)
12. [Dual Rendering Architecture](#12-dual-rendering-architecture)
13. [NPC and AI Systems](#13-npc-and-ai-systems)
14. [Combat and Damage](#14-combat-and-damage)
15. [Skill and Progression System](#15-skill-and-progression-system)
16. [Economy and Trade](#16-economy-and-trade)
17. [Build Phases](#17-build-phases)

---

## 1. Design Principles

These principles are non-negotiable. Every system in the engine must satisfy all of them.

**ZERO hardcoded content.** Every game constant, item definition, recipe, crop growth rate, ship component stat, NPC behavior, and quest objective lives in a data file. The Rust code is a generic simulation engine that reads data and executes rules. Adding a new crop, weapon, or ship module requires zero code changes — only a new data file entry.

**Hot-reloadable everything.** Any data file change takes effect immediately without restarting the game. Shaders, assets, game constants, item definitions, AI behavior trees — all hot-reloaded via file watchers. The dev loop is: edit a TOML file, save, see the change in-game within one second.

**Dual rendering.** The Tauri webview handles all social features, HUD, menus, inventory, and settings (the existing HTML/JS/CSS stack). A native wgpu window handles 3D rendering. They coexist in the same application, communicating via Tauri IPC. Neither owns the other.

**Parametric construction.** Building is not snapping cubes on a grid. Shapes have continuously adjustable dimensions. A wall is length x width x thickness, not a fixed-size block. CSG boolean operations combine primitives into complex geometry.

**Multi-scale.** The same engine renders a light switch at centimeter precision and a Dyson sphere at astronomical scale. No separate "space game" and "surface game" — one continuous simulation with LOD and streaming.

**Real technology only.** All ship systems, propulsion methods, life support, and engineering are based on real or near-proven physics. No magic shields, no FTL without theoretical basis, no handwaved energy sources. Advanced but real.

**Volumetric cargo.** Items have physical volume (cubic meters) and mass (kilograms). A cargo hold has a capacity in cubic meters, not abstract inventory slots. You cannot fit a turbine in a backpack.

**Off-screen autonomy.** When the player logs off, their character continues routine activities — tending gardens, repairing tools, mining asteroids. The simulation runs in compressed time on the server. Players return to meaningful progress.

**Education through gameplay.** Every game system teaches a real skill. Failure is educational, not punitive. The game creates situations where mastering real knowledge is the path to success. See [educational-gameplay.md](educational-gameplay.md) for the full philosophy.

---

## 2. Crate Architecture

### Current Implementation (v0.34.0)

The working engine code lives in `engine/src/`, structured as a single library crate:

```
engine/src/
  lib.rs                        # Engine init, main loop, system registration
  platform.rs                   # Cross-platform abstraction (native/WASM)
  wasm_entry.rs                 # WASM entry point (WebGPU)
  renderer/
    mod.rs                      # Renderer: wgpu device/surface, RenderObject, Material, InstanceBatch
    camera.rs                   # Three-mode camera (first-person, third-person, orbit)
    mesh.rs                     # Mesh: vertex buffers, index buffers, from_icosphere
    pipeline.rs                 # wgpu render pipeline, bind groups, uniforms
    shader_loader.rs            # WGSL shader loading
    multi_scale.rs              # Multi-scale rendering support
  ecs/
    mod.rs                      # Module exports
    components.rs               # 20 components: Transform, Velocity, PhysicsBody, Renderable,
                                # Name, Health, Faction, Controllable, AIBehavior, Interactable,
                                # Harvestable, GrowthStage, CropInstance, VehicleSeat, HardpointSlot,
                                # Hardpoints, VoxelBody, etc.
    systems.rs                  # System trait + SystemRunner (tick all systems per frame)
  physics/
    mod.rs                      # Rapier3d integration: rigid bodies, colliders, raycasting
    collision.rs                # Collision detection and response
    fluid.rs                    # Fluid simulation (scaffold)
  terrain/
    mod.rs                      # Terrain module exports
    icosphere.rs                # Icosahedron subdivision for planet surfaces
    planet.rs                   # PlanetDef (RON), PlanetRenderer, LOD levels
    asteroid.rs                 # Voxel asteroids: sparse octree, greedy meshing, ore veins
  ship/
    mod.rs                      # Ship interior module
    layout.rs                   # Layout parser: reads ship definitions from RON
    rooms.rs                    # Room mesh generation (bridge, reactor, quarters, cargo)
  assets/
    mod.rs                      # AssetManager: load_csv/toml/ron/json/gltf, cache, invalidate
    loader.rs                   # File parsers (CSV, TOML, RON)
    watcher.rs                  # notify-based FileWatcher for hot-reload (native only)
  systems/
    mod.rs                      # 15 game system modules
    time.rs                     # Day/night cycle, GameTime with seasons
    player.rs                   # Player controller (WASD, gravity, jump)
    interaction.rs              # Raycast interaction system
    ecology.rs                  # Ecosystem simulation
    farming/                    # Crop growth, soil chemistry, automation
    construction/               # CSG booleans, blueprints, structural analysis, routing
    inventory/                  # ItemStack, volumetric containers
    combat/                     # Damage types, status effects
    crafting/                   # Recipe matching, workstations
    quests/                     # Objectives, procedural generation
    ai/                         # Behavior trees, autonomy, flow-field pathfinding
    vehicles/                   # Ship piloting, mech control, propulsion
    economy/                    # Fleet-wide resource pools
    skills/                     # Learning-by-doing progression
    logistics/                  # Cargo management, shipping routes
    navigation/                 # Galaxy/system/orbital/surface scale transitions
  hot_reload/                   # DataStore, HotReloadCoordinator
  audio/                        # Audio module (scaffold, not yet wired)
  input/                        # InputState for cross-system input sharing
```

### v1.0 Target Architecture (Design)

```
crates/
  humanity-engine/              # Core engine — rendering, ECS, physics, audio, input
    src/
      lib.rs                    # Engine initialization, main loop, plugin registration
      renderer/
        mod.rs                  # Renderer orchestration, frame scheduling
        pipeline.rs             # wgpu render pipeline management
        gbuffer.rs              # Deferred rendering G-Buffer pass
        lighting.rs             # Lighting pass (directional, point, spot, clustered)
        raytracing.rs           # Hardware RT reflections, AO, GI (fallback: screen-space)
        postprocess.rs          # TAA, bloom, tone mapping, motion blur, color grading
        shader_reload.rs        # WGSL file watcher, pipeline recompilation on change
        materials.rs            # PBR material system, procedural texture generation
        terrain.rs              # Clipmap terrain, heightmap streaming, splatmap blending
        sky.rs                  # Procedural atmosphere, volumetric clouds, day-night cycle
        particles.rs            # GPU-driven particle system (fire, smoke, debris, weather)
        lod.rs                  # Level-of-detail management, mesh simplification
        streaming.rs            # Region-based asset streaming, VRAM budget management
        csg.rs                  # CSG boolean operations for parametric construction
      ecs/
        mod.rs                  # Archetypal ECS — entities, components, system scheduler
        world.rs                # World state container, entity creation/destruction
        query.rs                # Component query iterators
        schedule.rs             # System ordering, dependency resolution, parallel execution
        hot_reload.rs           # Component schema versioning, live data migration
      physics/
        mod.rs                  # Physics orchestration, timestep management
        rigid_body.rs           # Rapier3d integration — rigid bodies, colliders, joints
        fluid.rs                # Pressure-based fluid simulation between connected volumes
        pressure.rs             # Atmosphere pressure model, hull breach venting
        fire.rs                 # O2-dependent fire spread, suppression systems
        structural.rs           # Load-bearing analysis, stress visualization, failure
        gravity.rs              # Per-zone gravity (centrifugal sections, zero-G, planetary)
      audio/
        mod.rs                  # Audio orchestration, bus routing
        mixer.rs                # kira integration — voice pool, ducking, limiting
        spatial.rs              # Steam Audio FFI — HRTF, occlusion, reverb
        music.rs                # Adaptive music system, crossfade, stems
        hot_reload.rs           # Sound file watcher, live replacement
      input/
        mod.rs                  # Input orchestration, device detection
        mapping.rs              # Action-to-key bindings, rebindable, hot-reloadable
        gamepad.rs              # Gamepad support, dead zones, rumble
        vr.rs                   # OpenXR input — hand tracking, controllers
      assets/
        mod.rs                  # Asset manager orchestration
        loader.rs               # Async asset loading (GLB, KTX2, OGG, WGSL, TOML, CSV, RON)
        watcher.rs              # notify crate file watcher on data/ directory
        cache.rs                # LRU asset cache, reference counting, eviction
        hot_reload.rs           # Change detection, affected-system notification

  humanity-game/                # Game systems — ALL data-driven, no hardcoded content
    src/
      lib.rs                    # Game plugin registration, system wiring
      farming/
        mod.rs                  # Farming system orchestration
        crops.rs                # Crop growth simulation, stages, yield calculation
        soil.rs                 # Soil chemistry — pH, fertility, moisture, nutrients
        weather.rs              # Weather effects on crops, irrigation, frost damage
        automation.rs           # Sprinkler, harvester, drone automation
        companion.rs            # Companion planting bonuses, monoculture penalties
      construction/
        mod.rs                  # Construction system orchestration
        primitives.rs           # Parametric shapes: box, cylinder, sphere, wedge, torus
        csg.rs                  # CSG boolean operations: union, subtract, intersect
        blueprints.rs           # Hierarchical blueprints: fleet → ship → deck → room → detail
        structural.rs           # Load-bearing analysis, stress calculation, failure modes
        routing.rs              # Auto-routing: pipes, wiring, ventilation pathfinding
        stamps.rs               # Repeating section stamping for megastructures
      inventory/
        mod.rs                  # Inventory system orchestration
        items.rs                # Item definitions (from data files), physical properties
        containers.rs           # Volumetric containers — capacity in cubic meters
        stacking.rs             # Stack rules, weight limits, fragility
      combat/
        mod.rs                  # Combat system orchestration
        damage.rs               # Damage types: kinetic, thermal, radiation, explosive
        status.rs               # Status effects: irradiated, burning, decompression, bleeding
        weapons.rs              # Weapon definitions (from data), fire modes, ammo types
        armor.rs                # Armor values, penetration, degradation
      quests/
        mod.rs                  # Quest engine orchestration
        objectives.rs           # Objective types: deliver, build, repair, grow, discover
        triggers.rs             # Event triggers, condition evaluation
        rewards.rs              # Reward distribution, XP, items, reputation
        procedural.rs           # Procedural quest generation from templates
      crafting/
        mod.rs                  # Crafting system orchestration
        recipes.rs              # Recipe definitions (from data), input/output, quality
        workstations.rs         # Workstation types, tool requirements, energy costs
        quality.rs              # Output quality based on skill, tool quality, materials
      logistics/
        mod.rs                  # Logistics system orchestration
        supply_chain.rs         # Resource flow: mining → processing → manufacturing → distribution
        shipping.rs             # Package creation, routing, tracking, delivery
        transport.rs            # Transport types: rail, pneumatic tube, drone, conveyor
        tracking.rs             # Package tracking: origin, destination, location, ETA
      vehicles/
        mod.rs                  # Vehicle system orchestration
        ship.rs                 # Ship piloting: throttle, attitude, orbital maneuvers
        ground.rs               # Ground vehicles: rovers, trucks, construction equipment
        drones.rs               # Drone control: survey, cargo delivery, repair
        systems.rs              # Vehicle subsystems: engine, fuel, hull integrity
      navigation/
        mod.rs                  # Navigation orchestration, scale transitions
        galaxy.rs               # Galaxy view: spiral arms, star clusters, fleet markers
        system.rs               # System view: star, planets, asteroid belts, ships
        planet.rs               # Planet view: surface features, landing zones, cities
        surface.rs              # Surface view: terrain, buildings, vegetation
        interior.rs             # Interior view: rooms, corridors, equipment, wiring
        seamless.rs             # Seamless LOD transitions, streaming triggers
      ai/
        mod.rs                  # AI system orchestration
        behavior_tree.rs        # Behavior tree evaluator (from RON definitions)
        flow_field.rs           # Flow field pathfinding for million-agent crowds
        perception.rs           # NPC senses: sight, hearing, proximity
        autonomy.rs             # Off-screen character simulation
        scheduler.rs            # NPC daily schedules, task prioritization
        social.rs               # NPC relationships, faction reputation, morale
      skills/
        mod.rs                  # Skill system orchestration
        progression.rs          # Learning-by-doing XP, diminishing returns at higher levels
        proficiency.rs          # Skill proficiency effects on task outcomes
        teaching.rs             # Player-to-player teaching, cooperative skill transfer
      economy/
        mod.rs                  # Economy system orchestration
        markets.rs              # Market simulation: supply/demand, price discovery
        trade.rs                # Player-to-player and NPC trade, barter, contracts
        fleet.rs                # Fleet-wide resource pools, donation, collective upgrades
        mining.rs               # Asteroid mining yield, ore processing, refining chains

  humanity-relay/               # Existing web relay server — KEEP AS-IS
    src/                        # See existing codebase (relay.rs, api.rs, storage/)
    client/                     # Existing HTML/JS/CSS chat client

  humanity-data/                # Shared data types between engine and relay
    src/
      lib.rs                    # Re-exports
      identity.rs               # Ed25519 identity types, key rotation
      messages.rs               # Message types shared between client and server
      items.rs                  # Item definitions shared between game and relay
      profiles.rs               # Player profile data structures
```

### Dependency Graph

```
humanity-engine  ←──  humanity-game
       ↓                    ↓
  humanity-data  ←──────────┘
       ↑
  humanity-relay
```

- `humanity-data` is the shared foundation — types used by both the game client and the relay server.
- `humanity-engine` depends on `humanity-data` for identity and message types.
- `humanity-game` depends on `humanity-engine` for rendering, ECS, physics, and audio.
- `humanity-relay` depends on `humanity-data` but is otherwise independent. It does not depend on the engine or game crates.

### External Crate Stack

| Layer | Crate | Purpose |
|-------|-------|---------|
| GPU | `wgpu` | Cross-platform GPU (Vulkan/DX12/Metal/WebGPU) |
| Math | `glam` | SIMD vec/mat/quat |
| Physics | `rapier3d` | Rigid body, collision, raycasting, joints |
| Audio mixing | `kira` | Lock-free mixer, 256+ voices, bus routing |
| Spatial audio | `steam-audio` (FFI) | HRTF, occlusion, GPU reverb |
| Models | `gltf` | glTF 2.0 / GLB loading |
| Images | `image` | Texture loading (PNG, JPEG, HDR) |
| Windowing | `winit` | Window creation, input events |
| VR | `openxr` | Headset rendering, hand tracking |
| File watching | `notify` | Cross-platform filesystem events |
| Serialization | `serde` + `toml` + `ron` + `csv` | Data file parsing |
| ECS | `hecs` | Archetypal ECS (was planned custom, using hecs in v0.34.0) |
| Shaders | WGSL | Hand-written, no transpilation, hot-reloaded |

---

## 3. Hot-Reload System

### Architecture

```
data/                          # ALL game content lives here
  constants.toml               # Global game constants (gravity, speed, etc.)
  items/
    tools.csv                  # Tool definitions
    materials.csv              # Material properties
    crops.csv                  # Crop definitions
  recipes/
    crafting.csv               # Crafting recipes
    cooking.csv                # Cooking recipes
  ships/
    components.csv             # Ship component definitions
    propulsion.toml            # Propulsion system configs
    life_support.toml          # Life support system configs
  ai/
    behaviors/
      farmer.ron               # NPC farmer behavior tree
      miner.ron                # NPC miner behavior tree
      guard.ron                # NPC guard behavior tree
    schedules/
      civilian.ron             # Civilian daily schedule
  quests/
    templates.ron              # Quest templates for procedural generation
    story/
      chapter_01.ron           # Scripted story quests
  blueprints/
    rooms/
      bridge.ron               # Ship bridge blueprint
      engine_room.ron          # Engine room blueprint
    ships/
      corvette.ron             # Corvette class blueprint
  shaders/
    pbr.wgsl                   # PBR fragment shader
    terrain.wgsl               # Terrain rendering shader
    clouds.wgsl                # Volumetric cloud shader
  assets/
    models/                    # GLB models
    textures/                  # KTX2 textures (only where procedural falls short)
    audio/                     # OGG Vorbis sound effects and music
```

### File Watcher Pipeline

```
notify crate (filesystem events)
       │
       ▼
  ┌─────────────┐
  │ AssetWatcher │  Debounces events (50ms window), deduplicates paths
  └──────┬──────┘
         │
         ▼
  ┌──────────────┐
  │ ChangeRouter │  Maps file extension + path to affected system
  └──────┬───────┘
         │
    ┌────┴────┬──────────┬───────────┬──────────┐
    ▼         ▼          ▼           ▼          ▼
 .toml     .csv       .ron        .wgsl      .glb/.ogg
 Config    Tables     Complex     Shader     Asset
 Reload    Reload     Reload      Recompile  Reload
```

### Reload Behaviors by File Type

| Extension | Loader | Reload behavior |
|-----------|--------|-----------------|
| `.toml` | `toml::from_str` | Replace config struct in ECS resource. Systems read fresh values next tick. |
| `.csv` | `csv::Reader` | Rebuild lookup table (HashMap by ID). Existing entities with changed stats update in-place. |
| `.ron` | `ron::from_str` | Replace behavior tree / blueprint / quest template. Running instances continue with old data until restarted. |
| `.wgsl` | `wgpu::ShaderModule` | Recompile shader module, rebuild affected render pipeline. If compilation fails, keep old pipeline and log error. |
| `.glb` | `gltf::Gltf` | Reload mesh and texture data. Replace GPU buffers for all entities using this model. |
| `.ogg` | `symphonia` | Reload audio data. Replace sound in kira. Currently-playing instances finish, new plays use new data. |
| `.ktx2` | `basis-universal` | Reload compressed texture. Replace GPU texture for all materials referencing it. |

### Dev Console

Pressing the tilde key (`~`) opens a runtime console overlay rendered in the webview layer.

Capabilities:
- **Inspect any entity:** `inspect entity:1234` shows all components and their current values.
- **Modify values live:** `set entity:1234.health 100` changes a component value in real-time.
- **Reload specific files:** `reload data/items/tools.csv` forces an immediate reload.
- **Reload all:** `reload all` re-reads every data file.
- **Spawn entities:** `spawn item:steel_beam at 10,5,3` creates an entity at world coordinates.
- **Time control:** `timescale 0.1` for slow-motion, `timescale 10` for fast-forward.
- **Teleport:** `tp 100,50,200` moves the player camera.
- **Query:** `query has:CropComponent where stage > 3` lists matching entities.
- **Perf overlay:** `perf` toggles frame time, draw call count, entity count, memory usage.

Commands are defined in a data file (`data/console_commands.ron`) and are themselves hot-reloadable.

---

## 4. Construction System (Parametric CSG)

### Philosophy

Construction is NOT snapping cubes on a grid. Every shape is parametric — adjustable in every dimension with continuous precision. A wall is not "a wall block" but a rectangular prism with length, width, and thickness that the player sets to any value.

### Parametric Primitives

| Primitive | Parameters | Use cases |
|-----------|-----------|-----------|
| Box | length, width, height | Walls, floors, beams, plates, panels |
| Cylinder | radius, height, segments | Pipes, columns, tanks, hatches |
| Sphere | radius, segments | Domes, pressure vessels, observation ports |
| Wedge | length, width, height, angle | Ramps, hull fairings, aerodynamic surfaces |
| Torus | major_radius, minor_radius, segments | Ring stations, gaskets, reinforcement rings |
| Cone | top_radius, bottom_radius, height | Nozzles, funnels, transition pieces |

All parameters accept floating-point values. There is no grid snapping by default (optional snap-to-grid mode for convenience).

### CSG Boolean Operations

```
Union (A ∪ B)         — Combine two shapes into one solid
Subtract (A - B)      — Cut shape B out of shape A (doors, windows, pipe holes)
Intersect (A ∩ B)     — Keep only the overlapping region

Example: Hull panel with a porthole
  1. Create Box(3.0, 0.1, 2.0)          — wall panel
  2. Create Cylinder(0.3, 0.2, 32)      — porthole hole
  3. Position cylinder at center of box
  4. Subtract cylinder from box          — wall with circular window
  5. Create Torus(0.3, 0.02, 32)        — window frame ring
  6. Union torus with result             — finished porthole
```

The CSG tree is stored as a DAG (directed acyclic graph) in RON format. Each node is either a primitive with parameters or an operation referencing child nodes. This tree is re-evaluated on parameter change, enabling live-tweaking of any dimension.

### Hierarchical Blueprints

Blueprints are nested at five levels of detail. Each level can be designed, saved, shared, and instantiated independently.

```
Fleet Blueprint
├── Ship positions and formations
├── Communication links between ships
│
Ship Blueprint
├── Hull shape (CSG tree defining exterior)
├── Deck layout (vertical slicing of hull volume)
├── Section allocation (engineering, habitation, cargo, bridge)
│
Deck Blueprint
├── Room placement within deck boundaries
├── Corridor routing between rooms
├── Bulkhead positions (pressure boundaries)
│
Room Blueprint
├── Fixtures: consoles, workbenches, beds, toilets, storage
├── Equipment: reactor controls, life support panels, weapon mounts
├── Furniture: chairs, tables, shelving, decoration
│
Detail Blueprint
├── Wiring routes (power, data, control)
├── Piping routes (coolant, water, fuel, atmosphere)
├── Ventilation routing (air ducts, filters, fans)
├── Individual components: switches, valves, junction boxes
```

### Scale Adaptations

| Scale | Construction mode | Example |
|-------|------------------|---------|
| Home interior | Centimeter precision. Place individual switches, outlets, pipe fittings. | Bathroom renovation: lay tiles, route plumbing, wire outlets |
| Building | Meter precision. Walls, floors, roofing, structural framing. | Colony hab building with load-bearing walls and HVAC |
| Ship | Meter precision with deck-level planning. Hull CSG, room placement. | 200m corvette with 6 decks, 40 rooms |
| Station | Section-level stamping. Design a section, repeat it around the ring. | 500m ring station with 24 identical hab sections |
| Megastructure | Section stamping at kilometer scale. Design a segment, tile it. | Dyson ring: design a 10km segment, stamp 10,000 copies |

### Structural Analysis

Every constructed object undergoes real-time structural analysis:

- **Load paths:** Forces traced from load application through structural members to supports.
- **Stress visualization:** Color overlay showing stress levels (green = safe, yellow = marginal, red = failure imminent).
- **Material properties:** Each material (steel, aluminum, titanium, composite) has yield strength, elasticity, density defined in `data/items/materials.csv`.
- **Failure modes:** Exceeding yield strength causes deformation. Exceeding ultimate strength causes fracture. Failure is localized — a broken beam collapses its span, not the whole structure.
- **Safety factor display:** Shows the ratio of capacity to load for each structural member.

### Auto-Routing

When a player places two connected systems (e.g., reactor and radiator), the engine auto-routes pipes/wiring between them:

1. Build a nav mesh of available routing spaces (inside walls, under floors, in cable trays).
2. A* pathfind from source connector to destination connector.
3. Generate pipe/wire geometry along the path, respecting bend radius minimums.
4. Player can accept the auto-route or manually override any segment.
5. Routes update when rooms are rearranged — affected routes re-pathfind automatically.

### Blueprint Sharing

Blueprints serialize to RON files. Players can:
- Save any blueprint level (room, deck, ship, fleet) as a standalone file.
- Share blueprints through the relay server (upload as asset, link in chat).
- Import shared blueprints into their own constructions.
- Rate and comment on shared blueprints (uses existing relay social features).
- Fork and modify shared blueprints (version tracking via hash chain).

---

## 5. Ship Systems (Real Technology)

All ship systems are grounded in real or near-proven physics. Each system is defined entirely in data files — adding a new propulsion type means adding a TOML entry, not writing Rust code.

### System Definition Schema

```toml
# data/ships/propulsion.toml

[ion_drive]
name = "Hall-Effect Ion Thruster"
category = "propulsion"
thrust_kn = 0.5
specific_impulse_s = 3000
power_draw_kw = 40
mass_kg = 50
fuel_type = "xenon"
fuel_rate_kg_per_s = 0.001
heat_output_kw = 15
failure_modes = ["cathode_degradation", "grid_erosion"]
maintenance_interval_hours = 2000
repair_skill = "electronics"
repair_difficulty = "competent"
description = "High-efficiency electric propulsion. Low thrust but extraordinary fuel economy."
real_reference = "NASA NEXT-C ion thruster, demonstrated 2023"
```

### Propulsion Systems

| System | Thrust | Isp (s) | Status | Notes |
|--------|--------|---------|--------|-------|
| Chemical (bipropellant) | Very high | 300-450 | Current tech | Launch, emergency maneuvers |
| Ion drive (Hall-effect) | Very low | 1500-5000 | Current tech | Long-duration cruise |
| Nuclear thermal (NERVA) | High | 800-1000 | Proven concept | Interplanetary transfer |
| Nuclear pulse (Orion) | Extreme | 2000-10000 | Theoretical | Large ship acceleration |
| Fusion (D-T tokamak) | High | 10000+ | Near-future | Primary interplanetary drive |
| Fusion (D-He3) | High | 20000+ | Advanced future | Aneutronic, less shielding needed |
| Solar sail | Zero (photon pressure) | Infinite | Current tech | Inner system, no fuel cost |
| Alcubierre concept | N/A | N/A | Highly speculative | Research-tier only, exotic matter required |

Each propulsion type has realistic thrust curves, fuel consumption, heat generation, and failure modes. No propulsion system violates known physics — even the Alcubierre drive is presented as a research project with unsolved problems, not a working FTL drive.

### Power Generation

| System | Output (kW) | Mass (kg/kW) | Fuel | Notes |
|--------|-------------|-------------|------|-------|
| Solar photovoltaic | 0.1-100 per panel | 5 | Sunlight | Output drops with distance from star |
| RTG (radioisotope) | 0.1-2 | 200 | Pu-238 | Ultra-reliable, low power, decades of life |
| Fission reactor | 100-10000 | 10 | U-235 | Requires shielding, coolant loop, control rods |
| Fusion reactor | 1000-1000000 | 5 | D-T or D-He3 | Requires magnetic confinement, superconductors |
| Fuel cells | 1-100 | 20 | H2+O2 | Short-term, backup power |

Power systems feed a ship-wide electrical grid. Every device has a power draw. If total draw exceeds generation, systems shed load by priority (life support last, weapons first if not in combat).

### Life Support

| System | Function | Consumables | Failure consequence |
|--------|----------|------------|-------------------|
| O2 electrolysis | Split water into O2 + H2 | Water, electricity | Suffocation (minutes) |
| CO2 scrubbers | Remove CO2 from atmosphere | LiOH cartridges | CO2 poisoning (hours) |
| Water recycler | Reclaim water from waste | Filter cartridges, energy | Dehydration (days) |
| Temperature control | Maintain habitable temperature | Coolant, energy | Hypothermia/hyperthermia |
| Radiation shielding | Block cosmic and reactor radiation | Passive (mass), active (magnetic) | Radiation sickness (weeks) |
| Hydroponics | Grow food, supplement O2 | Seeds, nutrients, light | Starvation (weeks-months) |
| Waste processing | Process biological and industrial waste | Bacteria cultures, energy | Disease, contamination |

Each system has maintenance schedules, spare part requirements, and realistic failure cascades. A broken CO2 scrubber does not instantly kill the crew — it starts a clock during which CO2 levels rise, symptoms appear, and the crew must repair or improvise.

### Gravity Simulation

No artificial gravity generators. Gravity comes from real physics:

- **Centrifugal gravity:** Rotating sections of ships and stations produce pseudo-gravity proportional to rotation rate and radius. Smaller radius = more Coriolis effect (nausea, curved trajectories). Large stations (500m+ radius) feel natural.
- **Zero-G zones:** Non-rotating sections, ship cores, EVA. Movement uses push-off, handholds, thruster packs.
- **Magnetic boots:** Allow walking on metal surfaces in zero-G. Slower than floating, but hands-free.
- **Planetary gravity:** Each body has its gravity defined in data. Moon = 0.16g, Mars = 0.38g, Earth = 1.0g.

### Communications

| System | Range | Latency | Bandwidth | Notes |
|--------|-------|---------|-----------|-------|
| Radio (UHF/VHF) | 10 AU | Light-speed delay | Low | Standard, reliable |
| Laser comm | 100 AU | Light-speed delay | Very high | Line-of-sight required, weather-affected |
| Relay network | Unlimited (with relays) | Cumulative light-speed | Medium | Relay satellites at Lagrange points |
| Quantum-entangled (speculative) | Unlimited | Instant | Very low | Research-tier, limited bandwidth |

Communication delay is simulated realistically. Talking to Mars takes 4-24 minutes one-way. Orders to distant fleet elements are not instant — this drives gameplay (autonomy, pre-planned responses, trust in remote commanders).

### Defense Systems

| System | Type | Notes |
|--------|------|-------|
| Point defense (CIWS) | Kinetic | Automated tracking, high fire rate, effective against missiles/debris |
| Railgun | Kinetic | High velocity projectile, massive power draw, recoil compensation needed |
| Laser (COIL/fiber) | Directed energy | Speed-of-light engagement, thermal blooming at range, power-hungry |
| Missile/torpedo | Kinetic/explosive | Self-guided, high damage, counterable by point defense |
| Armor plating | Passive | Whipple shields for micrometeoroids, composite layers for combat |
| Magnetic deflection | Active | Deflects charged particles, requires superconducting magnets |

No "shields" in the Star Trek sense. Defense is layered: detect threat, evade if possible, intercept with point defense, absorb with armor.

---

## 6. Fluid and Physics Simulation

### Pressure-Based Fluid Flow

The ship interior is divided into connected volumes. Each volume tracks:

```toml
# Runtime state (not a data file — this is the ECS component schema)
[volume]
id = "engine_room_main"
pressure_kpa = 101.3         # Standard atmosphere
temperature_k = 293.0        # 20°C
o2_fraction = 0.21
co2_fraction = 0.0004
humidity = 0.45
contaminants = []             # Radiation, smoke, toxic gas
connections = [
  { target = "corridor_b3", aperture_m2 = 2.0, status = "open" },
  { target = "reactor_room", aperture_m2 = 0.5, status = "sealed" },
]
```

**Flow calculation per tick:**
1. For each connection between volumes, calculate pressure differential.
2. Flow rate = aperture area x pressure differential x flow coefficient.
3. Transfer gas mass between volumes proportionally.
4. Equalize temperature via heat transfer across the connection.
5. Contaminants flow with the gas (irradiated coolant, smoke, toxic fumes).

### Hull Breach Simulation

When a hull breach occurs:
1. Create a connection between the breached volume and vacuum (0 kPa external).
2. Flow rate is proportional to hole size and internal pressure.
3. Small breach (bullet hole): slow decompression over minutes. Sealant foam can patch it.
4. Large breach (railgun hit): explosive decompression in seconds. Emergency bulkheads close automatically if the system is powered.
5. Objects and people near the breach experience force proportional to pressure differential.
6. Sound attenuates as atmosphere thins — near-vacuum is nearly silent.

### Coolant Systems

```
Nuclear Reactor
      │
      ▼
Primary Coolant Loop (water/liquid metal)
      │ Heat exchanger
      ▼
Secondary Coolant Loop (water/CO2)
      │ Heat exchanger
      ▼
Radiator Panels (radiate heat to space)
```

If any segment of the coolant loop is breached:
- Coolant leaks into the surrounding volume.
- Primary loop coolant may be irradiated — contamination hazard.
- Reactor temperature rises without cooling — automatic SCRAM if sensors are functional.
- Leaked fluid flows through connected volumes following pressure/gravity.

### Fire Simulation

Fire requires three things (fire triangle): fuel, heat, O2.

- **Spread:** Fire spreads to adjacent flammable materials. Spread rate depends on material flammability (from `data/items/materials.csv`), O2 concentration, and temperature.
- **O2 consumption:** Fire consumes O2 in the volume. A sealed room fire will eventually self-extinguish as O2 drops below 16%.
- **Suppression:** Halon/CO2 fire suppression systems displace O2. Water suppression cools the fire below ignition temperature. Venting to vacuum kills fire instantly (but also kills crew).
- **Smoke:** Fire produces smoke that reduces visibility and is toxic. Smoke flows through connected volumes following air currents.
- **Damage:** Fire damages equipment, wiring, and structural elements over time. Damage rate depends on material heat resistance.

### Gravity Variation

Physics responds to local gravity conditions:

- **Centrifugal sections:** Gravity vector points outward from rotation axis. Magnitude = omega^2 x radius. Coriolis effects on moving objects (projectiles curve, thrown objects deflect).
- **Zero-G core:** No gravity. Objects float. Fluids form spheres. Fire burns in spheres (different from terrestrial fire — harder to fight).
- **Transition zones:** Moving between rotating and non-rotating sections involves passing through decreasing gravity. Handholds and guide rails assist.
- **Planetary surface:** Uniform downward gravity at the body's surface value.

---

## 7. Supply Chain and Logistics

### Volumetric Inventory

Every item has physical properties defined in data:

```csv
# data/items/tools.csv
id,name,volume_m3,mass_kg,category,durability,repair_skill,description
wrench_standard,Standard Wrench,0.002,0.8,tool,500,mechanics,"Adjustable wrench for general use"
welding_torch,Welding Torch,0.01,3.2,tool,200,fabrication,"Oxyacetylene cutting and welding"
circuit_board,Circuit Board,0.0005,0.05,component,1000,electronics,"General-purpose PCB"
steel_beam_1m,Steel Beam (1m),0.01,78.5,structural,10000,construction,"Structural steel I-beam, 1 meter"
```

### Container System

Containers have capacity in cubic meters and mass limits:

| Container | Volume (m3) | Mass limit (kg) | Notes |
|-----------|------------|-----------------|-------|
| Pocket | 0.002 | 2 | Small tools, keys, data chips |
| Tool belt | 0.01 | 10 | Hand tools, flashlight |
| Backpack | 0.04 | 25 | Personal carry |
| Locker | 0.5 | 200 | Personal storage |
| Cargo crate (small) | 1.0 | 500 | Standard shipping unit |
| Cargo crate (large) | 4.0 | 2000 | Heavy equipment, bulk materials |
| Cargo bay | 100-10000 | Variable | Ship-mounted, varies by ship class |
| Ore hopper | 50 | 50000 | Mining ship, raw ore storage |

Items must physically fit. You cannot carry a 2-meter beam in a backpack regardless of remaining volume, because the item's longest dimension exceeds the container's opening.

### Shipping and Tracking

Every item in transit is a tracked package:

```ron
Package(
  id: "PKG-2847291",
  contents: [
    ItemStack(id: "steel_plate", quantity: 50, volume_m3: 2.5, mass_kg: 3925.0),
    ItemStack(id: "welding_rod", quantity: 200, volume_m3: 0.1, mass_kg: 8.0),
  ],
  origin: Location(ship: "ISV Meridian", bay: "cargo_2"),
  destination: Location(ship: "ISV Resolve", bay: "cargo_1"),
  route: [
    Segment(type: "conveyor", from: "cargo_2", to: "dock_a", eta_s: 120),
    Segment(type: "shuttle", from: "dock_a", to: "dock_b", eta_s: 3600),
    Segment(type: "pneumatic", from: "dock_b", to: "cargo_1", eta_s: 60),
  ],
  status: InTransit(current_segment: 1, progress: 0.45),
  priority: Normal,
  sender_key: "a1b2c3...",
  timestamp: 1710000000,
)
```

### Internal Transport Systems

Within large ships and stations:

| System | Capacity | Speed | Best for |
|--------|----------|-------|----------|
| Pneumatic tubes | < 2 kg, < 0.01 m3 | Fast (30 m/s) | Data chips, small parts, samples |
| Conveyor belt | Any size/mass | Slow (1 m/s) | Bulk materials, ore, heavy equipment |
| Cargo drone | < 50 kg, < 0.5 m3 | Medium (5 m/s) | Medium packages, automated delivery |
| Rail car | < 5000 kg, < 10 m3 | Medium (10 m/s) | Heavy cargo between sections |
| Manual carry | Backpack limits | Varies | Personal errands, emergency |
| Forklift/loader | < 2000 kg | Slow (3 m/s) | Loading/unloading cargo bays |

### Supply Chain Flow

```
Asteroid Mining
  └── Raw ore (iron, nickel, platinum, ice)
       └── Processing Plant
            ├── Refined metals (steel, aluminum, titanium)
            ├── Water (from ice)
            ├── Rare elements (platinum group, rare earths)
            └── Waste rock (mass driver for reaction mass)
                 └── Manufacturing
                      ├── Structural components (beams, plates, pipes)
                      ├── Electronics (circuits, sensors, computers)
                      ├── Machinery (motors, pumps, actuators)
                      └── Consumables (filters, lubricants, seals)
                           └── Distribution
                                ├── Ship stores (maintenance inventory)
                                ├── Construction sites (building materials)
                                └── Market (player trade)
```

Every step in this chain is simulated. Mining yields depend on asteroid composition (data-defined). Processing requires energy and produces waste. Manufacturing requires recipes, workstations, and skilled workers. Distribution requires transport.

### Fleet-Wide Resource Pools

The fleet maintains shared resource pools that any member can contribute to or draw from:

- **Fuel reserve:** Shared fuel for fleet maneuvering.
- **Materials stockpile:** Common materials available for construction and repair.
- **Food supply:** Distributed across hydroponics bays fleet-wide.
- **Spare parts:** Critical components for fleet maintenance.

Players can donate resources to the pool. Fleet upgrade projects draw from pools. Resource allocation can be prioritized by community vote (uses existing relay voting mechanisms).

---

## 8. Off-Screen Autonomy

### Concept

When a player disconnects, their character does not freeze. The server simulates their activities in compressed time based on a priority list the player can configure before logging off.

### Activity Simulation

The server runs a simplified tick every game-hour for offline characters:

```ron
// Player-configurable priority list (saved to vault)
OfflinePriorities(
  activities: [
    Activity(type: "farming", priority: 1, details: "water and harvest garden plot A"),
    Activity(type: "mining", priority: 2, details: "mine asteroid belt sector 7"),
    Activity(type: "repair", priority: 3, details: "maintain all tools above 50% durability"),
    Activity(type: "crafting", priority: 4, details: "craft steel plates from stockpile"),
    Activity(type: "rest", priority: 5, details: "sleep 8 hours per day cycle"),
  ],
  restrictions: [
    "never sell items",
    "do not leave current ship",
    "maintain food reserves above 7 days",
  ],
)
```

### Activity Outcomes

Each offline tick produces outcomes based on:
- **Player skill level:** Higher skill = more efficient output, fewer failures.
- **Available resources:** Cannot mine without a functional mining ship. Cannot farm without seeds and water.
- **Tool condition:** Degraded tools produce less output and degrade further.
- **Character needs:** Hunger, rest, and health are simulated. The character eats, sleeps, and rests appropriately.

### What the Player Returns To

On reconnection, the player receives a summary:

```
While you were away (14 hours, 3 game-days):

Farming:
  - Harvested 24 tomatoes (garden plot A)
  - Planted 12 carrot seeds
  - Watered all plots daily

Mining:
  - Mined 450 kg iron ore, 12 kg nickel ore
  - Tool wear: mining laser at 62% (was 85%)

Repairs:
  - Sharpened hoe (now 78%, was 52%)
  - Replaced welding torch nozzle (now 95%, was 41%)

Resources contributed to fleet pool: 200 kg iron ore

Character status: Healthy, well-rested, fed
```

### Efficiency Scaling

| Skill level | Offline efficiency | Notes |
|-------------|-------------------|-------|
| Novice (0-25) | 40% of online rate | Frequent mistakes, slow work |
| Competent (26-50) | 60% of online rate | Reliable but not fast |
| Skilled (51-75) | 80% of online rate | Efficient routine work |
| Expert (76-90) | 90% of online rate | Near-optimal performance |
| Master (91-100) | 95% of online rate | Almost as good as being there |

Offline work is always slightly less efficient than active play — the player's real-time decisions and reactions cannot be fully replicated. This preserves the incentive to play actively while ensuring meaningful progress during absence.

### Server-Side Implementation

Offline simulation runs on the relay server as a periodic job:
1. Every 10 real-world minutes, process all offline characters.
2. Each character gets N game-hours of simulation (based on elapsed real time and time scale).
3. Results are stored in the database and delivered as a summary on reconnection.
4. Resource changes are applied to the shared fleet pool in real-time (online players see contributions appear).

---

## 9. Multi-Scale Navigation

### Scale Hierarchy

The game world spans 40+ orders of magnitude, from centimeter-precision interiors to galaxy-wide views. Navigation is seamless — no loading screens between scales.

```
Galaxy View         ~100,000 light-years across
  │                 Spiral arms, star clusters, fleet position marker
  │ zoom in
  ▼
System View         ~100 AU across
  │                 Star, planets, asteroid belts, ship positions
  │ zoom in
  ▼
Orbital View        ~100,000 km across
  │                 Planet globe, moons, orbital stations, approaching ships
  │ zoom in
  ▼
Planet View         ~1,000 km across
  │                 Surface features, cities, landing zones
  │ zoom in
  ▼
Surface View        ~10 km across
  │                 Terrain, buildings, vegetation, roads
  │ zoom in
  ▼
Exterior View       ~100 m across
  │                 Building exteriors, ship hulls, terrain detail
  │ enter
  ▼
Interior View       ~10 m across
                    Rooms, corridors, equipment, wiring, switches
```

### Rendering Strategy Per Scale

| Scale | Renderer | Data source | LOD strategy |
|-------|----------|-------------|-------------|
| Galaxy | Procedural spiral + particle field | Star catalog + galaxy parameters | Fixed ~5000 particles |
| System | Orbital mechanics + planet sprites | Solar system data (TOML) | All bodies always visible |
| Orbital | Planet globe + ship models | Planet textures, ship meshes | LOD meshes for distant ships |
| Planet | Terrain clipmap + atmosphere | Heightmaps, splatmaps (streamed) | Clipmap rings, 5-6 levels |
| Surface | Full terrain + vegetation + buildings | All asset types | Full LOD chain |
| Exterior | High-detail meshes | GLB models, procedural textures | Highest LOD |
| Interior | Full detail, no LOD needed | Room blueprints, furniture meshes | Everything visible |

### Seamless Transitions

Transitions between scales use continuous LOD, not discrete switches:

1. **Galaxy to System:** Stars fade from dots to labeled objects. Selected star's planets begin appearing as the star grows.
2. **System to Orbital:** Other planets shrink off-screen. Focus planet grows, atmosphere glow appears, surface features emerge.
3. **Orbital to Planet:** Globe fills viewport. Terrain detail loads in clipmap rings. Atmosphere transitions from glow ring to sky dome.
4. **Planet to Surface:** Camera descends through atmosphere. Terrain detail increases continuously. Buildings and vegetation pop in at appropriate distances.
5. **Surface to Interior:** Player walks through a door. Interior volumes load. Exterior LOD reduces behind the player.

Data streaming triggers based on camera distance and velocity. Predictive loading: if the player is flying toward a planet at high speed, begin loading surface data before they arrive.

### Camera Modes

All modes available at all scales, though some are more useful at certain scales:

| Mode | Default at | Controls | Notes |
|------|-----------|----------|-------|
| First-person | Interior, Surface | Mouse look, WASD move | Primary gameplay camera |
| Third-person | Surface, Exterior | Mouse orbit, WASD move | Better spatial awareness |
| Top-down | Surface, Planet | Mouse pan, scroll zoom | Strategic planning, base layout |
| Free camera | Any | Fly anywhere, no collision | Dev mode, screenshot mode |
| Cinematic | Any | Keyframed path | Replays, trailers |

---

## 10. Data File Formats

### Format Selection Rationale

| Format | Extension | Use case | Why this format |
|--------|-----------|----------|----------------|
| TOML | `.toml` | Game constants, system configs, ship component definitions | Human-readable, supports comments, flat key-value with sections. Easy to edit in any text editor. |
| CSV | `.csv` | Item tables, crop tables, recipe lists, material properties | Spreadsheet-editable. Designers can use Excel/Sheets, export CSV. Diffable in git. |
| RON | `.ron` | Complex nested structures: behavior trees, blueprints, quest definitions | Rust-native object notation. Supports enums, tuples, nested structs. More expressive than TOML for tree structures. |
| WGSL | `.wgsl` | Shaders | WebGPU standard shader language. Hot-reloadable. |
| GLB | `.glb` | 3D models | Binary glTF. Single file, includes mesh + materials + animations. Industry standard. |
| KTX2 | `.ktx2` | Compressed textures (where procedural is insufficient) | GPU-compressed texture container. BC7 for desktop, ASTC for mobile. Small files, fast GPU upload. |
| OGG | `.ogg` | Sound effects, music, dialog | Vorbis compression. Good quality at low bitrate. Open format. |
| FLAC | `.flac` | High-fidelity audio (archival, lossless music) | Lossless compression. For source assets and quality-critical audio. |

### Zero Magic Numbers Rule

Every numeric value in the codebase must come from a data file or be derived from data file values. The Rust code contains ZERO hardcoded gameplay numbers.

Bad:
```rust
// WRONG — hardcoded values
let growth_rate = 0.15;
let max_health = 100.0;
let gravity = 9.81;
```

Good:
```rust
// RIGHT — all values from data files
let growth_rate = crop_data.growth_rate;
let max_health = species_data.base_health;
let gravity = planet_data.surface_gravity;
```

### Path Convention

All data file paths are relative to the `data/` directory. The engine resolves `data/` relative to the executable location. No absolute paths in any data file.

```toml
# Correct
model = "models/ships/corvette.glb"

# Wrong
model = "C:/Humanity/data/models/ships/corvette.glb"
```

### Example Data Files

**Game constants:**
```toml
# data/constants.toml

[physics]
atmosphere_standard_kpa = 101.325
o2_fraction_standard = 0.2095
co2_lethal_fraction = 0.10
co2_warning_fraction = 0.03
fire_o2_minimum = 0.16
vacuum_kpa = 0.0
speed_of_light_m_s = 299792458.0

[gameplay]
offline_efficiency_novice = 0.4
offline_efficiency_competent = 0.6
offline_efficiency_skilled = 0.8
offline_efficiency_expert = 0.9
offline_efficiency_master = 0.95
skill_xp_diminishing_factor = 0.95
quality_perfect_threshold = 0.95
quality_good_threshold = 0.70

[rendering]
terrain_clipmap_levels = 6
terrain_cell_size_m = 500.0
vegetation_draw_distance_m = 200.0
lod_bias = 1.0
particle_budget = 50000
```

**Crop definitions:**
```csv
# data/items/crops.csv
id,name,season,growth_days,stages,water_need,ph_min,ph_max,base_value,seed_cost,companions,rivals,real_fact
tomato,Tomato,summer,8,5,medium,6.0,6.8,15,5,"basil,carrot","cabbage,fennel","Tomatoes are botanically berries and originated in South America."
lettuce,Lettuce,spring,3,3,high,6.0,7.0,5,2,"carrot,radish","celery","Lettuce is a member of the sunflower family."
carrot,Carrot,"spring,autumn",5,4,medium,6.0,6.8,8,3,"tomato,lettuce","dill","Carrots were originally purple, not orange."
pumpkin,Pumpkin,autumn,12,5,high,6.0,6.8,25,8,"corn,bean","potato","The largest pumpkin ever grown weighed 2,702 pounds."
```

**NPC behavior tree:**
```ron
// data/ai/behaviors/farmer.ron

BehaviorTree(
  name: "farmer_daily",
  root: Sequence([
    // Morning routine
    Condition(time_between: (6, 7)),
    Action(type: "eat", location: "mess_hall"),

    // Work shift
    Selector([
      // Priority 1: Harvest mature crops
      Sequence([
        Condition(any_crop_mature: true),
        Action(type: "harvest", target: "nearest_mature_crop"),
      ]),
      // Priority 2: Water dry crops
      Sequence([
        Condition(any_crop_dry: true),
        Action(type: "water", target: "driest_crop"),
      ]),
      // Priority 3: Plant empty tilled soil
      Sequence([
        Condition(any_soil_empty: true),
        Condition(seeds_available: true),
        Action(type: "plant", target: "nearest_empty_soil", seed: "best_available"),
      ]),
      // Priority 4: Till new soil
      Sequence([
        Condition(expansion_available: true),
        Action(type: "till", target: "nearest_untilled"),
      ]),
      // Priority 5: Idle tasks
      Action(type: "inspect_crops"),
    ]),

    // Evening
    Condition(time_between: (18, 20)),
    Action(type: "eat", location: "mess_hall"),
    Action(type: "socialize", duration_min: 60),
    Action(type: "sleep", location: "quarters", duration_hours: 8),
  ]),
)
```

---

## 11. Educational Integration

### Every System Teaches a Real Skill

This is not an afterthought — it is the core design goal. See [educational-gameplay.md](educational-gameplay.md) for the full philosophy.

| Game system | Real skill taught | How failure educates |
|-------------|------------------|---------------------|
| Construction | Structural engineering, building codes | Bad framing collapses under load. Player sees stress visualization showing why. |
| Farming | Soil science, botany, crop rotation | Monoculture depletes soil. Wrong pH stunts growth. In-game encyclopedia explains the chemistry. |
| Ship systems | Mechanical/electrical engineering | Skipping maintenance causes failures. Repair requires diagnosing the actual problem. |
| Navigation | Celestial navigation, orbital mechanics | Wrong burn vector wastes fuel or misses target. Orbital display shows the correct trajectory. |
| Welding | Metallurgy, welding technique | Bad weld fails under stress. Post-mortem shows porosity, undercut, or insufficient penetration. |
| Electronics | Circuit design, PCB repair | Miswired circuit overloads. Oscilloscope tool shows where the fault is. |
| Cooking | Nutrition science, food safety | Undercooked food causes illness. Nutrition panel shows macro/micronutrient balance. |
| First aid | Triage, wound care, pharmacology | Incorrect treatment worsens condition. Medical scanner shows what went wrong. |
| Piloting | Aerospace engineering, physics | Wrong approach angle causes crash or excessive fuel use. Flight recorder replays show the error. |

### Skill Proficiency Affects Outcomes

Player skill proficiency is not cosmetic — it directly determines success and failure.

| Proficiency | Effect on gameplay |
|-------------|-------------------|
| Novice (0-25) | High failure rate. Slow execution. Game provides step-by-step guidance. Mistakes are costly but survivable. |
| Competent (26-50) | Reliable results for routine tasks. Normal speed. Guidance available on request but not automatic. |
| Skilled (51-75) | Fast execution. Bonus quality on outputs. Can handle non-routine situations. |
| Expert (76-90) | Near-perfect execution. Can improvise under pressure. Unlocks advanced techniques. |
| Master (91-100) | Can teach other players. Handles edge cases. Creates custom solutions. Community recognition. |

### Cooperative Skill Application

Multiple players working on the same task mirrors real-world teamwork:

- **Multi-person welding:** One tacks, another runs the bead, a third inspects quality. Communication matters.
- **Surgical teams:** Operator, anesthetist, vitals monitor — each role requires different knowledge.
- **Construction crews:** Framing, wiring, plumbing happen in parallel. Sequencing matters — you cannot drywall before wiring inspection.
- **Ship bridge crew:** Helmsman, navigator, engineer, comms — each station has its own skill requirements.

### In-Game Encyclopedia

Every item, system, material, and process has an encyclopedia entry linking to real-world reference material:

```ron
EncyclopediaEntry(
  id: "welding_mig",
  title: "MIG Welding (GMAW)",
  summary: "Gas Metal Arc Welding uses a continuous wire electrode and shielding gas.",
  sections: [
    Section(title: "Technique", content: "Maintain 15-20 degree torch angle..."),
    Section(title: "Common Defects", content: "Porosity, undercut, lack of fusion..."),
    Section(title: "Safety", content: "UV radiation, fume extraction, fire hazards..."),
  ],
  real_world_references: [
    "AWS D1.1 Structural Welding Code",
    "Lincoln Electric Welding Handbook",
  ],
  related_skills: ["fabrication", "metallurgy"],
  related_items: ["welding_torch", "welding_wire", "welding_helmet"],
)
```

### Procedural Scenarios

Scenarios are procedurally varied so players cannot memorize solutions:

- Weld joint geometry, material, and ambient conditions vary each time.
- Medical patients have procedurally generated injuries with different severities and complications.
- Electrical faults are randomized across circuit layouts.
- Farming conditions (soil, weather, pests) differ each season.

This forces genuine understanding rather than rote pattern recognition.

---

## 12. Dual Rendering Architecture

### Overview

The application has two rendering surfaces that coexist:

```
┌──────────────────────────────────────────────────────────────┐
│  Tauri Application Window                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  WebView2 Layer (always on top, transparent background) │  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │  Chat overlay    Inventory     Health/status bar  │  │  │
│  │  │  Quest tracker   Minimap       Dev console (~)    │  │  │
│  │  │  Settings panel  Trade window  Blueprint editor   │  │  │
│  │  └──────────────────────────────────────────────────┘  │  │
│  ├────────────────────────────────────────────────────────┤  │
│  │  wgpu Native Window (renders behind webview)           │  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │  3D world: terrain, ships, characters, effects    │  │  │
│  │  │  Sky, weather, particles, volumetric clouds       │  │  │
│  │  │  CSG construction preview, structural analysis    │  │  │
│  │  └──────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### Communication (Tauri IPC)

The webview and the native renderer communicate through Tauri's command system:

**Webview to Renderer (commands):**
- Camera control: position, rotation, zoom, mode switches
- Construction: place primitive, adjust parameter, apply CSG operation
- Interaction: click target at screen coordinates, activate equipment
- Settings: quality level, render distance, particle budget

**Renderer to Webview (events):**
- Entity interaction results: what the player clicked on, damage numbers
- World state updates: nearby entities, environmental readings
- Performance metrics: FPS, draw calls, memory usage
- Screenshot capture for UI preview thumbnails

**Shared state** via `Arc<Mutex<GameState>>` or lock-free channels (`crossbeam::channel`). The game state is owned by the Rust side. The webview reads snapshots of it via Tauri commands.

### Why Dual Rendering

- The existing HTML/JS/CSS UI stack (chat, settings, social, tasks) is already built and working. Rewriting it in an immediate-mode GPU UI would be months of wasted effort.
- HTML/CSS is superior for text-heavy UI (chat, encyclopedia, quest logs, inventory lists). GPU renderers handle text poorly.
- The 3D renderer focuses on what HTML cannot do: terrain, PBR materials, physics visualization, volumetric effects.
- This architecture allows web-only users (no desktop app) to access all social features. The 3D game is a desktop-only enhancement.

For the full rendering pipeline design (G-Buffer, lighting, RT, post-processing), see [graphics-pipeline.md](graphics-pipeline.md).

For the audio architecture (kira + Steam Audio), see [audio-engine.md](audio-engine.md).

---

## 13. NPC and AI Systems

### Behavior Trees

All NPC behavior is defined in RON files (see section 10 for example). The behavior tree evaluator is a generic engine component — it reads RON definitions and executes them. Adding new NPC types requires only new RON files.

Node types:
- **Sequence:** Execute children in order. Fail on first failure.
- **Selector:** Try children in order. Succeed on first success.
- **Condition:** Check a world-state predicate. Succeed or fail instantly.
- **Action:** Perform a game action (move, harvest, repair, eat, sleep). Takes time.
- **Parallel:** Execute children simultaneously. Configurable success/failure policy.
- **Decorator:** Modify child behavior (repeat, invert, timeout, cooldown).

### Flow Field Pathfinding

For large-scale NPC movement (thousands to millions of agents):

1. **Flow field generation:** Compute a vector field over the nav mesh pointing toward the goal. One flow field serves all agents with the same destination.
2. **Agent steering:** Each agent reads the flow field at its position and moves accordingly. Local avoidance prevents collisions.
3. **Hierarchical:** Coarse flow field for long-distance, fine flow field for local navigation. Reduces computation for distant agents.
4. **Budget:** Flow field computation is O(cells), not O(agents). 1000 agents using the same flow field costs the same as 1 agent.

For small groups (< 20 NPCs), use standard A* pathfinding on the nav mesh. Flow fields are for crowd simulation.

### NPC Perception

NPCs sense the world through defined channels:

| Sense | Range | Update rate | Blocked by |
|-------|-------|-------------|------------|
| Sight | 50m (configurable per NPC) | Every 0.5s | Walls, smoke, darkness |
| Hearing | 30m | Every 0.2s | Walls (attenuated, not blocked) |
| Proximity | 3m | Every tick | Nothing |
| Radio | Ship-wide | Instant | Comm system failure |

Perception drives behavior tree conditions. An NPC cannot react to what it cannot perceive.

### Daily Schedules

Every NPC follows a daily schedule defined in RON:

```ron
Schedule(
  name: "civilian_default",
  blocks: [
    Block(time: (6, 0), activity: "wake_up", location: "quarters"),
    Block(time: (6, 30), activity: "eat", location: "mess_hall"),
    Block(time: (7, 0), activity: "work", location: "assigned_workstation"),
    Block(time: (12, 0), activity: "eat", location: "mess_hall"),
    Block(time: (12, 30), activity: "work", location: "assigned_workstation"),
    Block(time: (17, 0), activity: "recreation", location: "common_area"),
    Block(time: (19, 0), activity: "eat", location: "mess_hall"),
    Block(time: (20, 0), activity: "socialize", location: "common_area"),
    Block(time: (22, 0), activity: "sleep", location: "quarters"),
  ],
)
```

Schedules are interruptible by emergencies (hull breach, fire, combat). After the emergency resolves, NPCs resume their schedule at the appropriate block.

### Social Simulation

NPCs have relationships, morale, and faction standing:

- **Relationships:** Each NPC pair has a relationship value (-100 to +100). Increases through shared work, socializing, and gifts. Decreases through conflict and neglect.
- **Morale:** Affected by food quality, sleep quality, workload, safety, and social connections. Low morale reduces work efficiency and increases conflict.
- **Faction:** NPCs belong to factions. Faction reputation affects trade prices, quest availability, and NPC behavior toward the player.

---

## 14. Combat and Damage

### Damage Model

Damage is physical and location-specific. There are no abstract "hit points" for ships — every component has its own integrity.

**Damage types:**
| Type | Source | Effect |
|------|--------|--------|
| Kinetic | Railgun, debris, collision | Penetration, structural deformation, hull breach |
| Thermal | Laser, fire, reentry heating | Material degradation, equipment failure, burns |
| Explosive | Missile warhead, decompression | Area damage, pressure wave, shrapnel |
| Radiation | Reactor leak, solar event, nuclear weapon | Equipment degradation, biological damage over time |
| EMP | Dedicated EMP weapon, solar flare | Electronics disruption, system reboots |

### Damage Propagation

When a projectile hits a ship:
1. **Impact point:** Determine which hull section is hit.
2. **Penetration:** Compare projectile energy vs armor resistance. Penetrating hits continue into interior.
3. **Interior damage:** The projectile (or shrapnel) damages whatever it hits inside — pipes, wiring, equipment, crew.
4. **Cascading failure:** Damaged systems affect connected systems. A hit on the coolant tank floods the area. A hit on the power conduit blacks out the section.
5. **Hull breach:** If the hit penetrates both sides of a hull section, atmosphere vents (see section 6).

### Status Effects

| Effect | Cause | Duration | Treatment |
|--------|-------|----------|-----------|
| Bleeding | Kinetic/shrapnel injury | Until treated | First aid, medical bay |
| Burning | Fire, thermal weapon | Until extinguished | Fire suppression, medical |
| Irradiated | Reactor leak, radiation weapon | Persistent | Anti-radiation meds, decontamination |
| Decompression | Suit breach in vacuum | Until sealed | Emergency suit patch |
| Concussion | Explosion, impact | Minutes | Rest, medical |
| O2 deprivation | Low atmosphere O2 | Until O2 restored | Move to pressurized area |

---

## 15. Skill and Progression System

### Learning by Doing

Skills improve through practice, not through spending abstract "skill points." Every action that exercises a skill grants XP proportional to the difficulty of the task and the player's current level.

```
XP gained = base_xp * difficulty_multiplier * (1 - current_level / max_level)^diminishing_factor
```

Diminishing returns ensure that trivial tasks stop granting meaningful XP as skill increases. A master welder gains nothing from welding a simple butt joint — they need to attempt challenging techniques to progress.

### Skill Categories

Defined entirely in data:

```csv
# data/skills/categories.csv
id,name,parent,description
fabrication,Fabrication,,Manufacturing and repair of physical objects
welding,Welding,fabrication,Joining metals using heat and filler material
machining,Machining,fabrication,Cutting and shaping metal on lathes and mills
electronics,Electronics,,Design and repair of electronic systems
circuit_design,Circuit Design,electronics,Designing functional circuits
pcb_repair,PCB Repair,electronics,Diagnosing and repairing circuit boards
agriculture,Agriculture,,Growing food and managing soil
soil_science,Soil Science,agriculture,Understanding and managing soil chemistry
hydroponics,Hydroponics,agriculture,Soilless growing systems
medical,Medical,,Healthcare and emergency treatment
first_aid,First Aid,medical,Basic emergency treatment
surgery,Surgery,medical,Invasive medical procedures
navigation,Navigation,,Finding and following routes
celestial_nav,Celestial Navigation,navigation,Navigation by stars
orbital_mechanics,Orbital Mechanics,navigation,Calculating orbital trajectories
```

### Proficiency Effects

Skill level directly affects gameplay outcomes (no separate "stats"). A player with welding skill 30 produces welds with 30% of master quality — visible defects, lower structural rating, higher failure probability under stress.

---

## 16. Economy and Trade

### Market Simulation

Markets are simulated using supply and demand:

- Every item has a base price defined in data.
- Actual price = base_price * (demand_factor / supply_factor).
- Supply increases when items are sold to the market or produced locally.
- Demand increases based on population needs, construction projects, and fleet status.
- Prices update every game-hour.
- Price history is tracked and visible to players (chart in the trade UI).

### Trade Mechanisms

| Mechanism | Description |
|-----------|------------|
| Direct trade | Player-to-player barter, mediated by relay |
| Market sell | Sell items to the fleet market at current price |
| Market buy | Buy items from the fleet market at current price |
| Contracts | Agree to deliver X items at Y price by Z date |
| Donation | Contribute to fleet resource pool (no payment) |
| Auction | Time-limited bidding for rare items |

### Fleet Upgrade Funding

Large fleet projects (new ship construction, station expansion, Dyson sphere segments) require collective resource contributions:

1. A project is proposed with a bill of materials.
2. Players contribute resources toward the project.
3. Progress bar shows completion percentage.
4. When fully funded, construction begins (takes time, requires workers).
5. Contributors receive recognition and optional naming rights.

---

## 17. Build Phases

### Phase 1: Foundation (Weeks 1-2)

**Goal:** Tauri app with dual windows. Triangle rendered in wgpu. Webview sends commands to native window.

- Tauri app spawns wgpu window alongside webview.
- Basic wgpu renderer: clear color, single triangle, PBR shader.
- Tauri IPC: webview can set camera position, native window reports FPS.
- Custom ECS: entities, components, basic system scheduler.
- File watcher: detect changes in `data/` directory.

**Milestone:** A spinning textured cube controlled by buttons in the webview.

### Phase 2: Scene and Assets (Weeks 3-4)

**Goal:** Load and render glTF models. Camera controls. Hot-reload shaders.

- glTF loader: mesh, materials, textures from GLB files.
- Transform hierarchy: parent-child entity relationships.
- Camera: first-person WASD + mouse look, third-person orbit.
- Shader hot-reload: edit WGSL, see changes without restart.
- TOML/CSV loader with file watcher: change a value, see it update.

**Milestone:** Walk around a scene of loaded models. Edit a TOML file and see the change in-game.

### Phase 3: World Systems (Weeks 5-8)

**Goal:** Terrain, sky, physics, audio. The world feels real.

- Terrain: clipmap rendering, heightmap loading, splatmap texturing.
- Sky: procedural atmosphere, day-night cycle, sun/moon.
- Physics: Rapier3d integration, player controller, collision.
- Audio: kira mixer, spatial audio via Steam Audio, footsteps and ambient.
- Vegetation: GPU-instanced grass and trees.
- Weather: rain/snow particles, wind affecting vegetation.

**Milestone:** Walk across terrain with sky, weather, vegetation, and spatial audio.

### Phase 4: Construction (Weeks 9-12)

**Goal:** Parametric CSG construction system.

- Parametric primitives: place and adjust box, cylinder, sphere, wedge.
- CSG operations: union, subtract, intersect.
- Blueprint system: save/load room and ship designs.
- Structural analysis: stress visualization overlay.
- Auto-routing: pipe and wire pathfinding between connected systems.
- Material assignment: apply materials from data-defined palette.

**Milestone:** Design a ship room with walls, floor, door (CSG subtraction), furniture, and piping.

### Phase 5: Ship Systems (Weeks 13-16)

**Goal:** Functional ship interiors with real systems.

- Power grid: generators, distribution, load shedding.
- Life support: O2, CO2, temperature, water recycling.
- Propulsion: engine control, thrust vectoring, fuel management.
- Fluid simulation: pressure-based atmosphere, hull breach venting.
- Fire simulation: O2-dependent spread, suppression systems.
- Damage model: component-level damage, cascading failures.

**Milestone:** Walk through a ship with functioning life support, take damage from a hull breach, repair it.

### Phase 6: NPCs and Economy (Weeks 17-20)

**Goal:** Living world with NPC behavior and economic simulation.

- Behavior tree evaluator from RON definitions.
- NPC daily schedules, perception, social simulation.
- Flow field pathfinding for crowds.
- Market simulation: supply/demand, price discovery.
- Off-screen autonomy: offline character simulation.
- Farming system: crop growth, soil chemistry, weather effects.

**Milestone:** NPCs follow daily routines, trade at markets, tend farms. Log off and return to find your garden tended.

### Phase 7: Multi-Scale Navigation (Weeks 21-24)

**Goal:** Seamless zoom from interior to galaxy.

- Navigation system: all scale levels rendering.
- Seamless transitions: continuous LOD between scales.
- Region streaming: load/unload terrain and assets based on camera position.
- Galaxy view: procedural spiral, star catalog.
- System view: orbital mechanics, planet rendering.

**Milestone:** Zoom from reading a console screen to viewing the galaxy, and back, with no loading screens.

### Phase 8: Combat and Quests (Weeks 25-28)

**Goal:** Gameplay loop with objectives and conflict.

- Combat system: damage types, status effects, weapons.
- Quest engine: objectives, triggers, rewards, procedural generation.
- Skill progression: learning-by-doing, proficiency effects.
- Crafting system: recipes, workstations, quality.
- Educational integration: encyclopedia entries, failure analysis.
- Cooperative tasks: multi-player construction, repair, combat.

**Milestone:** Complete a quest chain involving mining, crafting, construction, and combat. Skills improve through practice.

### Phase 9: Polish and VR (Weeks 29-32)

**Goal:** Production quality. VR support.

- Deferred rendering: G-Buffer, clustered lighting.
- Hardware raytracing: reflections, AO, GI (with fallback path).
- Post-processing: TAA, bloom, tone mapping, motion blur.
- OpenXR integration: stereo rendering, hand tracking, head-locked UI.
- Performance optimization: profiling, LOD tuning, memory budget.
- Multiplayer sync: entity state via existing WebSocket relay.

**Milestone:** The full game running at 60fps desktop / 90fps VR with RT effects.

---

## Appendix A: Key Constraints

1. **One-person maintainable.** Every system must be understandable by a single developer. Complexity that requires a team is disqualified.
2. **No build step for web UI.** The HTML/JS/CSS layer uses plain `<script>` tags. No webpack, no npm, no transpilation.
3. **Rust-only for game logic.** No C++ dependencies except through well-isolated FFI (Steam Audio). No GDScript, no Lua scripting.
4. **Open source.** MIT/Apache dual license. No proprietary dependencies that restrict redistribution.
5. **Runs on mid-range hardware.** Target: GTX 1060 / RX 580 minimum. RT effects optional, graceful degradation.

## Appendix B: Related Documents

| Document | Location | Relevance |
|----------|----------|-----------|
| Engine decision | [game-engine.md](game-engine.md) | Why custom wgpu, not Bevy |
| Graphics pipeline | [graphics-pipeline.md](graphics-pipeline.md) | Renderer architecture, PBR, RT, VR |
| Audio engine | [audio-engine.md](audio-engine.md) | kira + Steam Audio stack |
| Educational gameplay | [educational-gameplay.md](educational-gameplay.md) | Skill-teaching philosophy |
| Gardening game | [gardening-game.md](gardening-game.md) | First minigame, 2D prototype |
| Multi-scale maps | [maps-multi-scale.md](maps-multi-scale.md) | Seamless cosmic-to-street navigation |
| VR tracking | [vr-tracking.md](vr-tracking.md) | VR input and rendering |
