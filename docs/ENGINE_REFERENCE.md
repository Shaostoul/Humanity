# HumanityOS Native Engine Reference

> **Source of truth for AI agents and developers working on `src/`.**
> Updated: 2026-04-01 | Engine version: 0.88.0 | 32,361 LOC across 97 Rust files

## Quick Start for AI Agents

1. Read THIS file first for engine overview
2. Read `docs/STATUS.md` for feature completion status
3. Read `docs/BUGS.md` for known issues
4. Read `docs/SOP.md` for version/deploy procedures
5. Check `docs/design/engine-architecture.md` for v1.0 design targets
6. **Build command:** `just build-game` (or `cargo build -p HumanityOS --features native --release`)
7. **Run command:** `just play` (or `C:\Humanity\HumanityOS.exe`)
8. **Binary output:** `C:\Humanity\HumanityOS.exe`

---

## Architecture Overview

```
src/
  lib.rs              (1,943 LOC) -- Engine init, main loop, system registration
  main.rs             (17 LOC)    -- Entry point, calls lib::run()
  config.rs           (346 LOC)   -- App config, feature flags
  debug.rs            (128 LOC)   -- Debug overlay, frame stats
  persistence.rs      (233 LOC)   -- World save/load
  updater.rs          (716 LOC)   -- Auto-updater
  platform.rs         (80 LOC)    -- OS-specific paths
  embedded_data.rs    (217 LOC)   -- Fallback data when files missing
  wasm_entry.rs       (428 LOC)   -- WebAssembly entry (future)

  renderer/           -- wgpu rendering pipeline
  ecs/                -- hecs Entity Component System
  physics/            -- rapier3d physics simulation
  terrain/            -- Planet/asteroid mesh generation
  ship/               -- Ship interior generation
  gui/                -- egui immediate-mode UI
  systems/            -- 15+ game systems
  assets/             -- Data loading and hot-reload
  audio/              -- kira spatial audio
  net/                -- WebSocket multiplayer client
  input/              -- Key bindings
  hot_reload/         -- File watcher integration
  mods/               -- Mod manifest system
```

---

## Module Deep Dive

### Renderer (`renderer/`, 3,148 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 662 | **Working** | Pipeline init, render loop, bind groups, dynamic uniform buffers (256 slots) |
| camera.rs | 717 | **Working** | Three-mode camera (FPS/Orbit/Free), input handling, view/projection matrices |
| pipeline.rs | 154 | **Working** | Render pipeline creation, vertex layout, depth stencil (reverse-Z) |
| shader_loader.rs | 124 | **Working** | WGSL file loading from assets/shaders/ |
| mesh.rs | 164 | **Working** | Vertex struct (position, normal, uv), Mesh type, GPU buffer upload |
| sky.rs | 336 | **Working** | Procedural sky dome, day/night cycle gradient |
| stars.rs | 350 | **Working** | 119k star catalog rendered as point primitives (NOT particles) |
| hologram.rs | 576 | **Working** | Solar system hologram: planet spheres, orbit rings (tube torus), labels |
| floating_origin.rs | 36 | **Stub** | DVec3 world coords, f32 camera-relative rendering (declared, not wired) |
| multi_scale.rs | 29 | **Stub** | Multi-scale rendering transitions (declared, not wired) |

**Current pipeline:** Forward rendering, single PBR pass, no shadows, no post-processing.

**Key types:**
- `Vertex { position: [f32; 3], normal: [f32; 3], uv: [f32; 2] }` -- 32 bytes
- Dynamic uniform buffer: 256 object slots, 256-byte aligned `ObjectUniforms { model, normal_matrix }`
- Bind groups: Group 0 = Camera, Group 1 = Object (dynamic offset), Group 2 = Material
- Depth: reverse-Z (clear to 0.0, CompareFunction::Greater)

**Missing (not built):**
- Shadow mapping (no shadow pass, no shadow maps)
- Post-processing (no bloom, no SSAO, no FXAA)
- Deferred rendering (forward only)
- Frustum culling (all objects rendered every frame)
- Occlusion culling
- Skeletal animation / skinned meshes
- Particle system (NO particle emitters, NO GPU particles)
- Volumetric effects (fog, clouds, god rays)
- Screen-space reflections
- Global illumination

### ECS (`ecs/`, 256 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 17 | Working | Module exports |
| systems.rs | 47 | **Working** | `System` trait + `SystemRunner` (ordered tick) |
| components.rs | 192 | **Working** | 20+ component structs (Transform, Velocity, Health, Inventory, etc.) |

**System trait:**
```rust
pub trait System: Send + Sync {
    fn name(&self) -> &str;
    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore);
}
```

**Registered systems (in tick order):** TimeSystem, WeatherSystem, PlayerSystem, InteractionSystem, FarmingSystem, InventorySystem, CraftingSystem, ConstructionSystem, VehicleSystem, AISystem, EcologySystem, QuestSystem, CombatSystem, SkillsSystem, NavigationSystem

**Component list:** Transform, Velocity, Health, PlayerTag, NpcTag, Inventory, Equipment, SkillSet, QuestLog, FarmPlot, CropState, Blueprint, VehicleState, Interactable, PhysicsBody, Renderable, LightSource, AudioEmitter, NetworkId, AiBehavior

### Physics (`physics/`, 193 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 167 | **Working** | rapier3d wrapper: RigidBodySet, ColliderSet, step simulation, raycasting |
| collision.rs | 12 | **Stub** | Collision event handling (empty) |
| fluid.rs | 14 | **Stub** | Fluid simulation (empty, 14 LOC placeholder) |

**Working:** Rigid body creation, collider creation, simulation step, ray casting.
**Missing:** Fluid simulation, soft bodies, cloth physics, destruction physics.

### Terrain (`terrain/`, 1,367 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 10 | Working | Module exports |
| planet.rs | 194 | **Working** | Planet mesh generation from RON data |
| icosphere.rs | 158 | **Working** | Icosphere subdivision (LOD levels) |
| asteroid.rs | 670 | **Working** | Sparse octree voxel asteroids, greedy meshing, ore veins |
| heightmap.rs | 335 | **Working** | Heightmap terrain with 16 biome types |

**Working:** Icosphere planets with LOD, voxel asteroids with greedy meshing, heightmap terrain.
**Missing:** Terrain streaming (load/unload chunks), terrain deformation, vegetation placement, water surfaces.

### Ship (`ship/`, 612 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 8 | Working | Module exports |
| layout.rs | 185 | **Working** | RON layout parser for ship rooms |
| rooms.rs | 204 | **Working** | Room mesh generation (generic) |
| fibonacci.rs | 215 | **Working** | Fibonacci spiral homestead: 9 rooms, 3m walls, ceilings, floor quads |

**Room types:** computer, network, bathroom, bedroom, kitchen, living_room, laboratory, workshop, garden.
**Room colors:** Hard-coded per type. Fibonacci spiral layout with rooms sized F1 through F34.

### GUI (`gui/`, 11,790 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 1,068 | **Working** | GuiState, page routing, sidebar, nav |
| theme.rs | 352 | **Working** | Theme struct (70+ fields), hot-reload from theme.ron |
| widgets/mod.rs | 262 | **Working** | labeled_slider, toggle, section_header, settings_row, custom_slider, custom_checkbox |
| widgets/button.rs | 48 | **Working** | Styled button |
| widgets/row.rs | 296 | **Working** | Chat message row (bubble style, rounded rects) |
| pages/settings.rs | 1,065 | **Working** | Infinite scroll, TOC sidebar, 11 sections |
| pages/chat.rs | 2,088 | **Working** | Chat UI, message feed, input, stick-to-bottom |
| pages/studio.rs | 806 | **Working** | Streaming studio page |
| pages/maps.rs | 509 | **Working** | Multi-scale map view |
| pages/tasks.rs | 532 | **Working** | Task board |
| pages/main_menu.rs | 366 | **Working** | Main menu |
| pages/calendar.rs | 382 | **Working** | Calendar/planner |
| pages/profile.rs | 369 | **Working** | User profile |
| pages/market.rs | 387 | **Working** | Marketplace |
| pages/crafting.rs | 434 | **Working** | Crafting UI |
| pages/inventory.rs | 411 | **Working** | Inventory management |
| pages/guilds.rs | 466 | **Working** | Guild system |
| pages/calculator.rs | 345 | **Working** | Calculator |
| pages/notes.rs | 315 | **Working** | Notes/journal |
| pages/wallet.rs | 292 | **Working** | Wallet |
| pages/files.rs | 289 | **Working** | File browser |
| pages/trade.rs | 348 | **Working** | P2P trading |
| pages/donate.rs | 414 | **Working** | Donation page |
| pages/tools.rs | 249 | **Working** | Tools catalog |
| pages/bugs.rs | 245 | **Working** | Bug reporter |
| pages/escape_menu.rs | 230 | **Working** | Escape/pause menu |
| pages/civilization.rs | 230 | **Working** | Civilization dashboard |
| pages/resources.rs | 228 | **Working** | Resources page |
| pages/hud.rs | 179 | **Working** | In-game HUD overlay |
| pages/passphrase_modal.rs | 280 | **Working** | Seed phrase modal |
| pages/placeholder.rs | 32 | **Working** | Generic placeholder page |

### Game Systems (`systems/`, 4,662 LOC)

| System | File(s) | LOC | Status | Notes |
|--------|---------|-----|--------|-------|
| Time | time.rs | 186 | **Working** | Day/night cycle, game clock |
| Weather | weather.rs | 353 | **Working** | 7 conditions, seasonal variation |
| Player | player.rs | 140 | **Working** | Movement, input, camera control |
| Interaction | interaction.rs | 86 | **Working** | Raycast-based E-key interaction |
| Farming | farming/ | 255 | **Partial** | Core loop works; crops.rs, soil.rs, automation.rs are stubs |
| Inventory | inventory/ | 327 | **Working** | Item management; items.rs and containers.rs are stubs |
| Crafting | crafting/ | 291 | **Partial** | Core recipes work; workstations.rs is stub |
| Construction | construction/ | 240 | **Partial** | Blueprint placement works; csg.rs, routing.rs, structural.rs are stubs |
| Vehicles | vehicles/ | 323 | **Partial** | Core piloting works; ships.rs, propulsion.rs are stubs |
| AI | ai/ | 480 | **Partial** | Behavior trees work; autonomy.rs, flow_field.rs are stubs |
| Ecology | ecology.rs | 259 | **Working** | Ecosystem simulation |
| Quests | quests/ | 297 | **Partial** | Quest tracking works; objectives.rs is stub |
| Combat | combat/ | 70 | **Stub** | Mostly empty (damage.rs, effects.rs are stubs) |
| Skills | skills/ | 198 | **Partial** | XP/leveling works; learning.rs is stub |
| Economy | economy/ | 46 | **Stub** | Empty (16 LOC mod.rs + 30 LOC fleet.rs) |
| Logistics | logistics/ | 61 | **Stub** | Empty (shipping.rs, cargo.rs are stubs) |
| Navigation | navigation/ | 102 | **Stub** | Mostly empty (orbital.rs, galaxy.rs, surface.rs are stubs) |
| Hydrology | hydrology.rs | 420 | **Working** | Water flow simulation |
| Atmosphere | atmosphere.rs | 470 | **Working** | Atmospheric composition |
| Disasters | disasters.rs | 455 | **Working** | Natural disaster events |

### Audio (`audio/`, 237 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 159 | **Working** | kira audio manager, music/SFX playback |
| sounds.rs | 58 | **Working** | Sound effect definitions |
| spatial.rs | 20 | **Stub** | 3D spatial audio (declared, not wired) |

### Networking (`net/`, 2,834 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 17 | Working | Module exports |
| client.rs | 151 | **Working** | WebSocket client connection |
| ws_client.rs | 246 | **Working** | Message send/receive |
| sync.rs | 167 | **Working** | ECS state synchronization |
| protocol.rs | 104 | **Working** | Message protocol definitions |
| identity.rs | 99 | **Working** | Ed25519 key management |
| bip39_wordlist.rs | 2,050 | **Data** | BIP39 word list |

### Assets (`assets/`, 647 LOC)

| File | LOC | Status | Purpose |
|------|-----|--------|---------|
| mod.rs | 489 | **Working** | AssetManager: CSV/TOML/RON/JSON/GLTF loading |
| loader.rs | 85 | **Working** | File type dispatch |
| watcher.rs | 73 | **Working** | notify file watcher for hot-reload |

---

## Shader Inventory (`assets/shaders/`, 5,155 LOC across 40 files)

### Active Shaders (used by renderer)

| Shader | LOC | Bind Groups | Purpose |
|--------|-----|-------------|---------|
| pbr_simple.wgsl | 319 | Camera/Object/Material | **Main PBR shader.** Cook-Torrance GGX, procedural materials (grid/metal/concrete/wood), ACES tone mapping |
| basic.wgsl | 37 | Camera/Object | Flat color, used for debug geometry |
| stars.wgsl | 46 | Camera | Point primitive star rendering |
| star.wgsl | 48 | Camera/Object | Individual star billboard |
| ghost_preview.wgsl | 58 | Camera/Object | Transparent construction preview |
| constellation_lines.wgsl | 36 | Camera | Line rendering for constellations |

### Available But Not Wired

| Shader | LOC | Purpose |
|--------|-----|---------|
| pbr.wgsl | 199 | Full PBR with texture sampling (needs texture bind group) |
| procedural_material.wgsl | 319 | Standalone procedural material system |
| planet_surface.wgsl | 194 | Planet terrain rendering |
| planet_clouds.wgsl | 134 | Cloud layer rendering |
| earth.wgsl | 200 | Earth-specific procedural surface |
| mars.wgsl | 179 | Mars-specific surface |
| moon.wgsl | 155 | Moon surface |
| sun_surface.wgsl | 211 | Solar surface with convection |
| sun_glow.wgsl | 84 | Sun corona/glow effect |
| 6 procedural/ shaders | ~947 | Aperiodic, brick, metal, wood, concrete, fabric |
| 10 surface material shaders | ~760 | Drywall, wood plank, stone, rubber, concrete, carpet, marble, turf, granite, steel |
| 5 outer planet shaders | ~822 | Mercury, Venus, Jupiter, Saturn, Uranus, Neptune, Pluto |

### Material Type System (pbr_simple.wgsl)

The `params.z` field selects procedural material:
- **0** = Grid panels (1m panels with 3cm seam lines) -- walls, floors
- **1** = Brushed metal (directional micro-scratches) -- metallic surfaces
- **2** = Concrete (FBM noise, speckled) -- structural surfaces
- **3** = Wood grain (ring pattern + fine grain) -- furniture, flooring

To add a new material type: add an `else if` branch in `fs_main()` checking `material_type < N.5`.

---

## Data File Inventory (`data/`, 128,660 lines across 94 files)

### Critical Game Data

| File/Dir | Entries | Format | Hot-Reload | Used By |
|----------|---------|--------|------------|---------|
| items.csv | 306 items | CSV | Yes | InventorySystem, CraftingSystem |
| recipes.csv | 227 recipes | CSV | Yes | CraftingSystem |
| materials.csv | 92 materials | CSV | Yes | ConstructionSystem |
| components.csv | 102 components | CSV | Yes | CraftingSystem |
| plants.csv | 52 plants | CSV | Yes | FarmingSystem |
| chemistry/*.csv | 396 entries | CSV | Yes | Content reference |
| stars.csv | 119,627 stars | CSV | Yes | Stars renderer |
| solar_system/bodies.json | 70+ bodies | JSON | Yes | Hologram, navigation |
| world/solar_system.ron | 645 lines | RON | Yes | World spawning |
| world/spawn.ron | 19 lines | RON | Yes | Player spawn point |
| world/player.ron | 15 lines | RON | Yes | Player defaults |
| ships/*.ron | 191 lines | RON | Yes | Ship generation |
| quests/*.ron | 245 lines | RON | Yes | QuestSystem |
| blueprints/*.ron | 1,363 lines | RON | Yes | ConstructionSystem |
| gui/theme.ron | 71 lines | RON | Yes | GUI theme |
| config.toml | 26 lines | TOML | Yes | Engine config |
| input.toml | 123 lines | TOML | Yes | Key bindings |
| skills/skills.csv | 20 skills | CSV | Yes | SkillsSystem |

### Localization

5 languages: `data/i18n/{en,es,fr,ja,zh}.json` (45 lines each)

---

## Build & Run

```bash
# Build game + copy to repo root (preferred)
just build-game

# Build and launch
just play

# Or manually:
cargo build -p HumanityOS --features native --release
cp target/release/HumanityOS.exe HumanityOS.exe

# Quick check (no binary, fast)
just check-game
```

### Feature Flags
- `native` -- Enables desktop windowing (winit), GPU rendering (wgpu), audio (kira), physics (rapier3d)
- Without `native` -- Library-only mode for server-side ECS simulation

### Dependencies (key crates)
- `wgpu 24.x` -- GPU rendering
- `egui 0.31` -- Immediate-mode GUI (note: `Rounding::same()` takes `u8`, `rect_stroke` needs `StrokeKind`)
- `hecs` -- ECS
- `rapier3d` -- Physics
- `kira` -- Audio
- `glam` -- Math (Vec3, Mat4, Quat)
- `ron` -- Rusty Object Notation parser
- `notify` -- File system watcher
- `winit` -- Window management
- `tokio` -- Async runtime (networking)

---

## Common Patterns

### Adding a New Game System

1. Create `systems/my_system.rs` (or `systems/my_system/mod.rs` for multi-file)
2. Implement `System` trait:
```rust
pub struct MySystem { /* state */ }
impl System for MySystem {
    fn name(&self) -> &str { "MySystem" }
    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // Query ECS, update components
    }
}
```
3. Register in `lib.rs`: `runner.add(Box::new(MySystem::new()));`
4. Add `pub mod my_system;` to `systems/mod.rs`

### Adding a New GUI Page

1. Create `gui/pages/my_page.rs`
2. Add draw function: `pub fn draw(ui: &mut Ui, theme: &Theme, state: &mut MyState)`
3. Add state to `GuiState` in `gui/mod.rs`
4. Add page variant to routing in `gui/mod.rs`
5. Add nav entry in sidebar

### Adding a New Shader

1. Create `assets/shaders/my_shader.wgsl`
2. Define bind groups matching existing layout (Groups 0/1/2) or create new pipeline
3. Load via `ShaderLoader::load("my_shader")` in renderer
4. Create render pipeline with appropriate vertex layout

### Adding a New ECS Component

1. Add struct to `ecs/components.rs`
2. Derive `Debug, Clone` (and `Default` if appropriate)
3. Spawn entities with component in `lib.rs` or relevant system
4. Query in systems: `for (id, (transform, my_comp)) in world.query::<(&Transform, &MyComponent)>().iter()`

### Adding a New Data File

1. Create file in `data/` (CSV, TOML, RON, or JSON)
2. Add loading logic in `assets/mod.rs` AssetManager
3. Store in DataStore for system access
4. File watcher auto-detects changes for hot-reload

---

## Critical Architecture Decisions

1. **Forward rendering only** -- No deferred pass. Add shadow map pass before main pass when implementing shadows.
2. **Dynamic uniform buffer** -- 256 object slots with 256-byte alignment. Increase if > 256 objects needed.
3. **Reverse-Z depth** -- Clear to 0.0, use `CompareFunction::Greater`. Better far-field precision.
4. **Point primitives for stars** -- 119k stars rendered as GL_POINTS, not instanced meshes or particles.
5. **egui on top of wgpu** -- GUI rendered as egui overlay after 3D scene. No render-to-texture for GUI.
6. **Hot-reload everything** -- All data files watched by notify. Theme, items, recipes, shaders all reload on save.
7. **No texture pipeline yet** -- All materials are procedural in shader. No texture loading, no UV mapping pipeline.
8. **Camera-relative rendering** -- World positions are DVec3, rendering uses f32 offset from camera.

---

## Priority Gaps (what to build next)

### Tier 1: Visual Quality
- [ ] **Shadow mapping** -- Single directional shadow map (sun), 2048x2048 depth texture
- [ ] **Particle system** -- GPU instanced quads with lifetime, velocity, gravity. Needed for: engine exhaust, dust, sparks, rain, snow, fire, explosions
- [ ] **Room-specific PBR materials** -- Assign material_type per room in fibonacci.rs (currently all default)
- [ ] **Frustum culling** -- Skip draw calls for objects outside camera view

### Tier 2: Gameplay
- [ ] **Solar system navigation** -- Click hologram planet pin -> orbit camera transition
- [ ] **Combat system** -- Fill out damage.rs, effects.rs stubs
- [ ] **Economy system** -- Fill out economy/mod.rs stub
- [ ] **Logistics system** -- Fill out logistics/ stubs

### Tier 3: Polish
- [ ] **Post-processing** -- Bloom for bright objects (sun, lights), basic FXAA
- [ ] **Skeletal animation** -- GLTF skinned mesh support for characters
- [ ] **Spatial audio** -- Wire up audio/spatial.rs to 3D positions
- [ ] **Floating origin** -- Wire up floating_origin.rs for large-scale world

### Tier 4: Scale
- [ ] **Terrain streaming** -- Load/unload terrain chunks based on camera distance
- [ ] **Deferred rendering** -- G-buffer pass for many dynamic lights
- [ ] **LOD system** -- Mesh LOD switching based on distance
- [ ] **Occlusion culling** -- Skip draw calls for occluded objects
