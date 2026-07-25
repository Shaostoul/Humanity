use glam::{Quat, Vec3};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;
use crate::assets::AssetManager;
use crate::ecs::GameWorld;
use crate::ecs::systems::SystemRunner;
use crate::gui::{GuiState};
use crate::gui::theme::Theme;
use crate::hot_reload::HotReloadCoordinator;
use crate::hot_reload::data_store::DataStore;
use crate::input::InputState;
use crate::renderer::camera::{Camera, CameraController};
use crate::renderer::{RenderObject, Renderer};
use crate::ship::ship_structure::ShipStructure;
use crate::systems::time::GameTime;
use crate::systems::weather::Weather;
use crate::terrain::planet::PlanetDef;

/// One placed machine's world anchor for the plant pass (v0.863).
pub(crate) struct GrowSpot {
    pub(crate) ty: String,
    pub(crate) id: String,
    pub(crate) pos: Vec3,
    pub(crate) yaw: f32,
    pub(crate) top_y: f32,
    pub(crate) size: (f32, f32, f32),
}

/// Construction 3D drag state (v0.466): which editor room is grabbed + the world floor
/// plane and the offset from the room's min-corner to the grab hit point, so the room
/// tracks the cursor without jumping.
#[derive(Clone, Copy)]
pub(crate) struct ConstructionGrab {
    pub(crate) room_index: usize,
    pub(crate) floor_y: f32,
    pub(crate) offset_x: f32,
    pub(crate) offset_z: f32,
}

/// Which part of an opening gizmo is grabbed. (v0.469)
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum GizmoRole {
    Move,
    ResizeLeft,
    ResizeRight,
    ResizeBottom,
    ResizeTop,
}

/// Which build-mode gizmo the cursor is hovering (v0.569), so a gizmo reads idle -> hover ->
/// active (grabbed) by colour, like the menu header buttons. Computed each frame by
/// `compute_construction_hover` and consumed by the gizmo render.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum HoverGizmo {
    None,
    Corner(f32, f32),
    Opening(usize, usize),
    Char,
}

/// A draggable build-mode object (v0.593): a placed light (by index), a machine (by id), or a
/// structural piece (by index). Dragging its gizmo moves its X/Z on the floor, keeping its height.
#[derive(Clone, Debug)]
pub(crate) enum ObjectGrab {
    Light(usize),
    Machine(String),
    Structure(usize),
    RoadNode(u32),
    ConduitNode(String),
    Zone(String),
    /// A corridor DOOR-MOUTH handle (v0.790): index into `ship_structure.corridors`. Dragging
    /// slides the corridor's world `lat` along the wall (the axis ACROSS the run) -- the same
    /// field the Corridors panel's Lat DragValue edits.
    CorridorMouth(usize),
    /// A STRIP control-point handle (v0.790): `point` 0 is the strip's start (`pos`);
    /// 1.. index `path[point - 1]`. Dragging moves that point across the floor (y preserved).
    StripPoint { light: usize, point: usize },
    /// A light ROTATION ring (v0.790; any light kind + viewport-fixed rings v0.792): `axis`
    /// 0/1/2 = the X/Y/Z ring. Sweeping the cursor around the ring rotates the aim about
    /// that axis by the frame-to-frame angle delta. `prev_angle` carries last frame's cursor
    /// angle in the ring's plane (None until the first drag frame) -- state rides in the
    /// variant, like the opening grab's wall plane.
    LightAim { light: usize, axis: u8, prev_angle: Option<f32> },
}

/// Construction opening-gizmo drag (v0.468, rebuilt v0.469): which room+opening is grabbed,
/// the captured wall-face plane (so the cursor projects onto the VERTICAL wall, giving u along
/// + v up), and the grab role. `room_index` indexes `gui_state.construction_rooms` (the editor
/// mirror). `opening_index` Some(i) drives `rooms[ri].openings[i]` (move + resize); None is a
/// legacy `WallSet.offsets` slide (back-compat, Move only).
#[derive(Clone, Copy)]
pub(crate) struct ConstructionGizmoGrab {
    pub(crate) room_index: usize,
    pub(crate) opening_index: Option<usize>,
    pub(crate) wall_index: usize,
    pub(crate) role: GizmoRole,
    pub(crate) snap_floor: bool,
    pub(crate) wall_start: Vec3,
    /// Unit vector along the wall (start -> end).
    pub(crate) u_hat: Vec3,
    /// Wall-face plane normal (horizontal) the pick ray intersects.
    pub(crate) n: Vec3,
    pub(crate) wall_len: f32,
    pub(crate) wall_height: f32,
    /// Legacy slide base (offset = u - base_t); only used when `opening_index` is None.
    pub(crate) base_t: f32,
    /// Opening extents captured at grab time (for resize anchoring).
    pub(crate) grab_u: f32,
    pub(crate) grab_v: f32,
    pub(crate) grab_w: f32,
    pub(crate) grab_h: f32,
}

pub(crate) struct EngineState {
    /// Game audio (v0.960, first CC0 sounds): None when the machine has no
    /// audio device (headless rig, some VMs) - play sites skip gracefully.
    pub(crate) audio: Option<crate::audio::AudioManager>,
    /// Data-driven sound catalog (data/sounds.toml, ids like
    /// "sfx.button_click" -> assets/audio paths).
    pub(crate) sound_catalog: crate::audio::sounds::SoundCatalog,
    /// Last (master, music, sfx) volumes pushed into the audio engine, so
    /// the per-frame sync only touches kira when a slider actually moved.
    pub(crate) audio_volumes_applied: (f32, f32, f32),
    /// One-shot guard for the tree-card atlas bake (v0.961): a FAILED bake
    /// must log once and stop, not re-parse 12 GLTFs every frame.
    pub(crate) tree_atlas_attempted: bool,
    /// Ambient particle simulation (v0.966): drifting leaves near trees,
    /// space dust for motion reference in the void. CPU-ticked each frame,
    /// drawn by the renderer's particle post-pass.
    pub(crate) particle_system: crate::renderer::particles::ParticleSystem,
    /// Last frame's ship_world_pos, for riding particles through
    /// floating-origin rebases (render space moves opposite the ship).
    pub(crate) prev_ship_world_pos: glam::DVec3,
    /// Dev test-light COUNT from the showcase lights:N hook (clustering
    /// dev-aid). The grid REGENERATES around the camera every frame - render
    /// space rebases with the floating origin, so stored positions go stale
    /// within seconds (the lit-then-dark capture mystery). 0 in play.
    pub(crate) debug_test_light_count: usize,
    /// Test-light intensity (lights_intensity IPC field; default 3.0). The
    /// outdoor-attenuation suspect: interiors lit at 3.0, open ground may not.
    pub(crate) debug_test_light_intensity: f32,
    pub(crate) window: Arc<Window>,
    pub(crate) renderer: Renderer,
    pub(crate) camera: Camera,
    pub(crate) controller: CameraController,
    pub(crate) asset_manager: AssetManager,
    pub(crate) hot_reload: HotReloadCoordinator,
    pub(crate) game_world: GameWorld,
    pub(crate) system_runner: SystemRunner,
    pub(crate) data_store: DataStore,
    pub(crate) star_renderer: Option<crate::renderer::stars::StarRenderer>,
    /// Background star-sky prebuild (v0.865 boot speed): a thread spawned
    /// at startup loads the catalog + Milky Way glow and builds the whole
    /// StarRenderer while the user reads the chat screen; world entry then
    /// just receives it (or blocks briefly if they beat the thread).
    /// None once consumed - a later world entry (tier change) rebuilds
    /// synchronously as before.
    pub(crate) star_preload_rx: Option<std::sync::mpsc::Receiver<Option<crate::renderer::stars::StarRenderer>>>,
    /// Streamed high-detail terrain tiles for EARTH (the downloadable
    /// terrain tier, ~460 m cells): region residency follows the camera,
    /// deep patches + the ground clamp sample it via tile_or_base. An
    /// empty/absent tile dir simply leaves the base grid in charge.
    pub(crate) terrain_tiles: crate::terrain::terrain_tiles::TerrainTiles,
    /// Connected-ocean mask for EARTH (v0.876 real-water Stage 1):
    /// flips the chunked terrain to true bathymetry and gates the
    /// water-shell patches. None = the pre-v0.876 clamped sea sphere.
    pub(crate) ocean_mask: Option<crate::terrain::ocean_mask::OceanMask>,
    /// Live Earth weather (v0.874): the background NASA GIBS fetcher
    /// delivers RG8 cloud-fraction grids here (cache first, then fresh
    /// every 30 min). None when the setting is off. Polled per frame;
    /// each grid is one queue.write_texture into the weather map.
    pub(crate) weather_rx: Option<std::sync::mpsc::Receiver<Vec<u8>>>,
    /// Latest uploaded live-weather grid (RG8, WEATHER_MAP_W x H), kept
    /// CPU-side so the god-ray pass can sample the overhead cloud cover
    /// (v0.897): thick real cloud masses BLOCK the sun, so the shafts
    /// fade out under an overcast instead of blasting through the deck.
    pub(crate) weather_grid: Option<Vec<u8>>,
    /// Orbital home station (v0.881): the homestead's Earth-centered
    /// position on its 400 km LEO orbit, recomputed each frame (phase =
    /// wall-clock UTC x the real orbital rate, so the orbit persists
    /// across sessions). DVec3::ZERO until the first frame propagates it.
    pub(crate) station_world_pos: glam::DVec3,
    /// True while the player frame RIDES the station (within the aboard
    /// radius): ship_world_pos advances by the station's orbital delta
    /// each frame so every home-local system works unchanged.
    pub(crate) station_ride: bool,
    /// One-shot: set at world load; the next frame snaps the player
    /// frame onto the station (the spawn room is aboard by definition).
    pub(crate) station_spawn_snap: bool,
    /// (station - ship frame) this frame, f32: the scene pass renders
    /// the whole home at this offset; ~zero while riding aboard.
    pub(crate) station_off: Vec3,
    /// Aboard flag for gating home-local physics (walls, floors,
    /// elevators) - false when the home is far away in orbit.
    pub(crate) aboard_station: bool,
    /// THIS frame's planet spin angle (v0.878.2): computed ONCE per
    /// frame at the top of RedrawRequested and read by every consumer
    /// (surface lock, frame-lock captures, render, camera requests).
    /// v0.878.0 computed it at each site; sites straddle the TimeSystem
    /// tick, so physics used the pre-tick hour and the render the
    /// post-tick hour - a one-tick ground offset (~0.7 m per frame,
    /// dt-jittered) that read as constant world flicker + swimming
    /// shorelines (operator report). One cached value, zero straddle.
    pub(crate) current_spin: f64,
    /// True while the player is in the surface WALK band (< 10 km): edge-
    /// detects touchdown for the gear reset (v0.872 band split).
    pub(crate) surface_walk_band: bool,
    /// True while the co-rotate band OWNS translation (walk + atmospheric
    /// flight, < 100 km): gates the world-scale FTL integration. One frame
    /// stale by design (the frame-lock section runs after the fly
    /// integration each frame).
    pub(crate) surface_owns_translation: bool,
    pub(crate) floating_origin: crate::renderer::floating_origin::FloatingOrigin,
    /// Procedural surface parameters per sky body, loaded from
    /// `data/planets/<body_id>.ron` at world load (v0.763). Bodies
    /// without a def render as smooth LOD spheres with the coarse
    /// `solar_body_materials` below.
    pub(crate) planet_defs: std::collections::HashMap<String, PlanetDef>,
    /// Loaded real-elevation grids for defs whose `heightmap` field
    /// points at a data file (Earth ships one: NOAA ETOPO1 downsampled
    /// to 0.1 degrees, ~12.4 MB resident). Keyed by body id; a body
    /// absent here falls back to procedural noise, so a missing or
    /// corrupt grid degrades gracefully instead of blanking the planet.
    // Arc-wrapped (v0.930): background sky-sphere builds share these
    // grids with worker threads at zero copy cost.
    pub(crate) planet_heightmaps: std::collections::HashMap<String, std::sync::Arc<crate::terrain::planet_heightmap::PlanetHeightmap>>,
    /// Loaded real surface-color grids for defs whose `albedo` field
    /// points at a data file (Earth ships one: NASA Blue Marble
    /// downsampled to 4096x2048, ~25 MB resident). Keyed by body id; a
    /// body absent here falls back to the elevation-band classifier,
    /// so a missing or corrupt grid degrades gracefully.
    pub(crate) planet_albedos: std::collections::HashMap<String, std::sync::Arc<crate::terrain::planet_albedo::PlanetAlbedo>>,
    /// Cached sky-body meshes keyed by (body id, subdivision level).
    /// Bodies WITHOUT a procedural def share plain icosphere meshes
    /// under the reserved id "_flat". LOD switches therefore never
    /// regenerate a mesh that was already built this session; only a
    /// first visit to a (body, level) pair pays the build cost.
    pub(crate) planet_mesh_cache: std::collections::HashMap<(String, u32), usize>,
    /// Background sky-sphere builds in flight (v0.930): key = (body, level),
    /// worker delivers the CPU mesh via the channel; frame thread uploads.
    pub(crate) sky_mesh_pending: std::collections::HashMap<(String, u32), std::sync::mpsc::Receiver<crate::terrain::planet_surface::SurfaceMeshData>>,
    /// Chunked-LOD state per heightmap-bearing body (2026-07-11): the
    /// quadtree patch cache + detail noise that takes over from the
    /// uniform sphere when the planet's disc overflows the screen (the
    /// old ladder's level-8 rung). See terrain::planet_chunks for the
    /// full architecture (depth-cap math, culling, streaming, skirts).
    pub(crate) planet_chunk_states: std::collections::HashMap<String, crate::terrain::planet_chunks::ChunkState>,
    /// Renderer mesh slots freed by patch LRU eviction, recycled via
    /// replace_mesh so the renderer's append-only mesh Vec stays
    /// bounded no matter how long a flight streams patches in and out.
    pub(crate) planet_patch_free_slots: Vec<usize>,
    /// Shared vertex-color material (shader type 12) for every
    /// procedural planet surface; per-face colors ride in the mesh.
    pub(crate) planet_surface_material: usize,
    /// Per-body TEXTURED surface materials (v0.811): planets shipping
    /// BOTH real grids (elevation + albedo -- Earth today) get their
    /// imagery baked (orbital-look grading applied per texel, see
    /// terrain::planet_surface::bake_albedo_rgba) and uploaded as a
    /// group-3 texture on a dedicated type-12 material whose params.w
    /// flags the shader's per-pixel path. base_color.xyz of these
    /// materials is REPURPOSED as the planet center in render space,
    /// rewritten every frame by the sky loop (the floating origin moves
    /// it); the shader needs the center because chunk-patch meshes are
    /// anchored at patch centers, not the planet center. Bodies absent
    /// here draw through `planet_surface_material` (per-face colors).
    pub(crate) planet_textured_materials: std::collections::HashMap<String, usize>,
    /// Per-body atmosphere-shell materials, created lazily from the
    /// def's atmosphere color. Keyed by (body id, scattering-mode flag)
    /// so flipping Settings > Graphics > "Scattering atmosphere" swaps
    /// between the analytic-scattering material (shader type 14) and the
    /// fresnel fallback (type 13) without touching the other variant --
    /// stale entries are ~100-byte uniform buffers, not worth evicting.
    pub(crate) planet_atmo_materials: std::collections::HashMap<(String, bool), usize>,
    /// Per-body cloud-deck materials (clouds increment 1, shader type
    /// 15), created lazily from the def's cloud_coverage. Keyed by
    /// (body id, quality tier 0/1/2) since increment 3: flipping
    /// Settings > Graphics > "Cloud quality" swaps materials without
    /// touching the other tiers (same pattern as the atmosphere map
    /// above); reload_planet_defs clears this map so RON tuning
    /// hot-reloads.
    pub(crate) planet_cloud_materials: std::collections::HashMap<(String, u8), usize>,
    /// Water-shell material per planet (v0.876, material type 16): its
    /// base_color.xyz is the planet center in render space, rewritten
    /// every frame like the textured-planet material above.
    pub(crate) planet_water_materials: std::collections::HashMap<String, usize>,
    /// World-space position of the Sun (Earth-centred coordinates).
    pub(crate) sun_world_pos: glam::DVec3,
    /// Emissive material index for the Sun core.
    pub(crate) sun_material: usize,
    /// Emissive material index for the Sun halo (larger sphere, warmer,
    /// lower emissive — gives the Sun a faked corona without bloom).
    pub(crate) sun_halo_material: usize,
    /// Materials for the real solar-system bodies rendered around the
    /// home (v0.262.9, map sync increment B): [0]=rocky, [1]=gas
    /// giant, [2]=icy/dwarf, [3]=default grey. Picked by SolBody
    /// `body_type`. The Sun reuses `sun_material`.
    pub(crate) solar_body_materials: [usize; 8],
    /// Orbit paths for the FPS world (v0.262.20 — thin world-space
    /// lines, replacing the old too-thick tube meshes). Each entry
    /// is (PARENT-frame ellipse points in metres, parent_id);
    /// per frame they're offset to the parent's Earth-relative
    /// position and drawn as a single-edge LineList that is
    /// depth-occluded behind planets.
    /// (ellipse points in parent-frame metres as f64 -- f32 at this
    /// magnitude cancels against the offset, v0.791; parent body id, kind
    /// "planet"|"moon", body id) -- the Sky settings filter by kind at
    /// draw time; the body id anchors the direction-of-motion trail fade
    /// (v0.790) to the body's live position on its ring.
    pub(crate) solar_orbit_paths: Vec<(Vec<glam::DVec3>, String, String, String)>,
    /// Homestead floor meshes (mesh_idx, material_idx) per room.
    pub(crate) homestead_floors: Vec<(usize, usize)>,
    /// Placeholder world objects (mesh_idx, material_idx, world position) drawn
    /// alongside the homestead. Used for simple-shape stand-ins like the
    /// aeroponic tower cylinders + plant-marker spheres (v0.383).
    pub(crate) placeholder_objects: Vec<(usize, usize, Vec3)>,
    /// Home machine meshes, kept SEPARATE from `placeholder_objects` so the construction editor
    /// can rebuild JUST the machines on an edit (a move/add/remove) without touching towers,
    /// pipes, or the avatar. Built by load_world on entry + rebuild_machine_objects on edit;
    /// positions come from the tested `MachineHome::placements`. Drawn when not in the showroom.
    /// (v0.525, the live-edit preview that makes the build mode feel real.)
    pub(crate) machine_objects: Vec<(usize, usize, Vec3, f32)>,
    /// Photoscanned decoration plants (v0.909): (mesh, material, world
    /// pos, yaw deg, uniform scale) scattered from
    /// data/entities/decorations.ron at home build.
    pub(crate) decoration_objects: Vec<(usize, usize, Vec3, f32, f32)>,
    /// Mesh/material cache for decoration models so world reloads reuse
    /// GPU resources instead of re-appending them.
    pub(crate) decoration_mesh_cache: std::collections::HashMap<String, (usize, usize)>,
    /// Near-field REAL tree models on planet surfaces (v0.911): the
    /// planet-fixed vegetation stream re-enumerated around the camera
    /// so photoscanned conifers stand where the silhouette cards are.
    pub(crate) near_trees: Vec<crate::terrain::planet_chunks::NearTree>,
    /// Planet-local camera position at the last near-tree recompute
    /// (recompute when the camera moves far enough).
    pub(crate) near_trees_center: glam::DVec3,
    /// Pick volumes for viewport machine SELECTION (v0.553): (id, world center, bounding radius)
    /// per placed machine. Rebuilt alongside machine_objects; the build-mode click ray-tests this
    /// to select a machine (its detail then shows on the right panel).
    pub(crate) machine_pick: Vec<(String, Vec3, f32)>,
    /// Machine world anchors for procedural PLANTS (v0.862/0.863), recorded by
    /// rebuild_machine_objects: catalog type + instance id + placement, so the
    /// plant pass can dress towers (helix) and beds/fields (footprint grid).
    pub(crate) grow_positions: Vec<GrowSpot>,
    /// Procedural plant meshes (v0.862): one merged world-space mesh per planted
    /// tower config, one draw each: (mesh_idx, mat_idx). See rebuild_plant_meshes.
    pub(crate) plant_objects: Vec<(usize, usize)>,
    /// Change signature of the last plant build; 0 forces a rebuild (hot reload).
    pub(crate) plant_mesh_sig: u64,
    /// Port pick volumes for viewport DRAG-TO-CONNECT (v0.625): (machine id, port index, the Port,
    /// world gizmo position) for every machine's derived ports. Only the SELECTED machine's ports
    /// render + are grab-able, but building all is cheap. Drag a port onto another machine to wire
    /// them. Rebuilt with `machine_pick`.
    pub(crate) port_pick: Vec<(String, usize, crate::utilities::Port, Vec3)>,
    /// Static wall/perimeter collision segments for the home (v0.556): the player (= the camera)
    /// is pushed out of these in first person so you can no longer walk through walls. Rebuilt
    /// from the home_structure on every structural edit + on world load. Doors collide live.
    pub(crate) wall_colliders: Vec<crate::ship::wall_collision::WallSegment>,
    /// Home machine CONNECTIONS as live colored cylinders (v0.530): (mesh, material, position,
    /// rotation, scale). Replaces the static routed pipes so connections appear immediately +
    /// follow rooms in the editor. Rebuilt with the machines; uses one cached unit cylinder mesh
    /// (`connection_cyl`) transformed per link + a material cached per kind (`connection_mats`),
    /// so a per-frame drag does not leak meshes.
    pub(crate) connection_objects: Vec<(usize, usize, Vec3, Quat, Vec3)>,
    pub(crate) connection_cyl: Option<usize>,
    pub(crate) connection_mats: std::collections::HashMap<String, usize>,
    /// Conduit FLOW paths (v0.622, declutter v0.623): the routed polyline + the from/to machine ids
    /// for every connection. Only the connection(s) touching the SELECTED machine animate (RGB flow
    /// markers); the rest are just their static pipe -- so a busy home is not a sphere-soup. Rebuilt
    /// with `connection_objects`.
    pub(crate) connection_flow_paths: Vec<(Vec<Vec3>, String, String)>,
    /// Cached small sphere mesh for the flow markers.
    pub(crate) connection_flow_sphere: Option<usize>,
    /// Cached small sphere mesh (~0.05 r) for the central node of a machine PORT gizmo (v0.627) --
    /// the solid "node" the 4 in/out arrows radiate from. Created on first build-mode render.
    pub(crate) port_node_mesh: Option<usize>,
    /// Cached rail-car mesh + material (v0.637, M2b): a small box that animates along each rail edge
    /// in build mode so the rail line reads as ALIVE. Created on first render.
    pub(crate) rail_car_mesh: Option<usize>,
    pub(crate) rail_car_mat: Option<usize>,
    /// Cached mining-drone meshes + materials (v0.639): a small multi-part placeholder (body,
    /// nose, 4 rotor pods) shown DOCKED at the home's drone hangar whenever no drone is currently
    /// in flight (`gui_state.drone_active == false`), and hidden the instant one launches --
    /// undocking on Outbound, docking again on delivery. Built once on first render; each part's
    /// mesh/material index is reused every frame (see `render_drone_dock`). Real geometry, not a
    /// borrowed machine primitive, so it visually reads as "a drone", not "a box".
    pub(crate) drone_dock_meshes: Option<[usize; 3]>, // [body, nose, rotor_pod]
    pub(crate) drone_dock_mats: Option<[usize; 2]>, // [body/nose, rotor]
    /// Docking sequence state (v0.681.x): 1 = drone settled on the pad, 0 = away.
    /// Eases toward drone_active each frame so launch/return animate, never pop.
    pub(crate) drone_dock_anim: f32,
    /// Deployed-vehicle primitives (economy Phase 2 Stage 1, v0.677): one unit box +
    /// one unit wheel cylinder, scaled per vehicle from the kit registry's body
    /// proportions each frame — same build-once pattern as the drone dock above.
    pub(crate) vehicle_meshes: Option<[usize; 2]>, // [unit_box, wheel_cylinder]
    /// Blueprint structures render (v0.746, ladder rung 2): one unit box,
    /// scaled per Construction/Structure Transform each frame.
    pub(crate) structure_mesh: Option<usize>,
    /// [scaffold amber, wood, stone, metal] — picked by blueprint category.
    pub(crate) structure_mats: Option<[usize; 4]>,
    /// Stage 3 take-over (v0.690): the vehicle the player is DRIVING (camera
    /// glued to the cab, WASD steers the vehicle). Deliberately NOT the
    /// Controllable-transfer path -- moving Controllable off the player would
    /// make extract_world_save find no player and wipe the periodic save.
    pub(crate) driving_vehicle: Option<hecs::Entity>,
    /// Chase-cam target while a vehicle self-drives (Stage 3 follow mode).
    pub(crate) follow_vehicle: Option<hecs::Entity>,
    /// The vehicle the crosshair currently targets (within reach + look cone).
    pub(crate) targeted_vehicle: Option<hecs::Entity>,
    pub(crate) vehicle_mats: Option<[usize; 3]>, // [body paint, cabin glass, wheel rubber]
    /// Animal in reach + look cone (v0.751): drives the "[E] collect" prompt.
    pub(crate) targeted_livestock: Option<hecs::Entity>,
    /// E pressed on a targeted animal; the frame bridge settles the collect.
    pub(crate) pending_livestock_harvest: Option<hecs::Entity>,
    /// Placeholder animal bodies: one lazy unit box + a material per tint.
    pub(crate) livestock_mesh: Option<usize>,
    pub(crate) livestock_mats: std::collections::HashMap<u64, usize>,
    /// Rainbow emissive materials (v0.623) cycled along the SELECTED connection's flow markers, so
    /// the active line reads as highlighted/animated. Created once on the first rebuild.
    pub(crate) flow_rgb_mats: Vec<usize>,
    /// Door + window panels (v0.537): each opening's world placement + its current open fraction
    /// (0 closed, 1 open). Doors animate open on the player's approach by their data-driven style
    /// (systems::door_anim); windows are fixed glass. One cached unit-box mesh + a slab + a glass
    /// material, reused (scaled/rotated/animated per frame), so it never leaks.
    pub(crate) door_panels: Vec<(crate::ship::door_panels::PanelPlacement, f32)>,
    /// Runtime "opened via its control panel" flag per door (v0.567), parallel to door_panels. A
    /// MANUAL door with this set opens; the player toggles it at the panel. Reset on rebuild.
    pub(crate) door_manual_open: Vec<bool>,
    /// Live LOCK STATE per door (v0.570): door_locks[i][j] is the runtime state of door i's lock j,
    /// parallel to door_panels[i].0.locks. The player unlocks/breaks locks at runtime; reset to the
    /// authored states on a structural rebuild (mirrors door_manual_open). A door is passable only
    /// when all of its locks are open.
    pub(crate) door_locks: Vec<Vec<crate::ship::lock_types::LockState>>,
    pub(crate) door_panel_mesh: Option<usize>,
    pub(crate) door_slab_mat: Option<usize>,
    pub(crate) door_glass_mat: Option<usize>,
    /// Energy/nanowall door materials (v0.554): glowing green (open) / red (locked) energy field
    /// + a metallic semi-transparent nanowall. All render in the transparent pass.
    pub(crate) door_energy_open_mat: Option<usize>,
    pub(crate) door_energy_locked_mat: Option<usize>,
    pub(crate) door_nanowall_mat: Option<usize>,
    /// Accumulated time (s) driving the nanowall's shifting "water" shimmer. (v0.554)
    pub(crate) door_anim_time: f32,
    /// Index in `placeholder_objects` where the player avatar's parts begin (the avatar
    /// is added last in load_world). Lets the showroom render only the avatar + rebuild
    /// it on appearance change by truncating to this index. (v0.441)
    pub(crate) avatar_obj_start: usize,
    /// Podium floor position the avatar stands on (respawner center). (v0.441)
    pub(crate) avatar_base: Vec3,
    /// First-person spawn position to drop the player at when leaving the showroom.
    pub(crate) fps_spawn: Vec3,
    /// Loaded character-select showroom backdrops. (v0.441)
    pub(crate) showroom_backdrops: Vec<crate::showroom::Backdrop>,
    /// The showroom ground disc (mesh, material), material rebuilt on backdrop change.
    pub(crate) showroom_ground: Option<(usize, usize)>,
    /// The showroom planet-body sphere mesh (v0.449): used instead of the flat disc when
    /// the backdrop is a body (Earth/Mars/Moon), so the avatar stands on a planet.
    pub(crate) showroom_body: Option<usize>,
    /// Last backdrop index the ground material was built for (usize::MAX = none yet).
    pub(crate) showroom_last_backdrop: usize,
    /// Cosmetic outfit catalog (data/cosmetics/cosmetics.csv). (v0.442)
    pub(crate) cosmetics: Vec<crate::cosmetics::Cosmetic>,
    /// First-person position to return to when leaving the showroom (the spawn for the
    /// initial character-select, or where you were standing when you opened the mirror /
    /// wardrobe from the wetroom / bedroom). (v0.442)
    pub(crate) showroom_return_pos: Vec3,
    /// Tracks whether the OS cursor is currently freed (visible + ungrabbed), so the
    /// per-frame reconciliation only toggles grab on a real change. (v0.443)
    pub(crate) cursor_free: bool,
    /// Homestead walls mesh + material (legacy fibonacci ship path).
    pub(crate) homestead_walls: Option<(usize, usize)>,
    /// Per-material home walls (v0.552): (mesh, material, is_transparent) for each picked wall
    /// material, so each wall renders in its own color (the home-structure path).
    pub(crate) homestead_material_walls: Vec<(usize, usize, bool)>,
    /// Homestead trim mesh (baseboards, crown, door/window frames) + material. (v0.453)
    pub(crate) homestead_trim: Option<(usize, usize)>,
    /// Homestead window-glass mesh + material. (v0.453)
    pub(crate) homestead_windows: Option<(usize, usize)>,
    /// Homestead mirror / portal panel mesh + material. (v0.453)
    pub(crate) homestead_mirrors: Option<(usize, usize)>,
    /// Homestead ceiling mesh + material — drawn only when `gui_state.show_roof`. (v0.453)
    /// On the ship path (v0.754) this slot holds only the GLASS-roof zones' ceilings.
    pub(crate) homestead_ceiling: Option<(usize, usize)>,
    /// OPAQUE-roof zones' ceilings (v0.754, per-zone glass-or-steel roofs): their own slot,
    /// rendered with the opaque ceiling material and gated by `show_roof` exactly like the old
    /// single opaque roof. None when every zone's roof is glass (the shipped default).
    pub(crate) homestead_ceiling_opaque: Option<(usize, usize)>,
    /// True when the ceiling is a CLEAR/GLASS roof (v0.539): the renderer draws it in the
    /// transparent pass (you see the stars through it) instead of as an opaque ceiling. Set by
    /// apply_homestead_meshes from the HomeStructure's roof_material.
    pub(crate) homestead_ceiling_glass: bool,
    /// HULL WRAP plating mesh + material (ship-superstructure increment D): the generated
    /// exterior shell around the whole zone cluster, lofted from data/blueprints/
    /// hull_profile.ron with cutouts over glass roofs + glass corridor lids. Opaque pass,
    /// gated by `gui_state.show_hull` (H key / Settings). Purely visual -- no collision.
    /// Rebuilt by `rebuild_hull` whenever the ship structure rebuilds; the slot is reused
    /// in place across rebuilds like every other homestead slot (no GPU-buffer leak).
    pub(crate) homestead_hull: Option<(usize, usize)>,
    /// The loaded hull profile (disk first, embedded fallback -- see HullProfile::load).
    /// Loaded lazily on the first `rebuild_hull`; None only if BOTH copies fail to parse
    /// (the hull is then simply absent).
    pub(crate) hull_profile: Option<crate::ship::hull::HullProfile>,
    /// The live homestead layout (v0.455). Held so the construction editor can mutate it
    /// (per-wall kinds, heights) and regenerate the meshes without a restart.
    pub(crate) homestead_layout: Option<crate::ship::fibonacci::HomesteadLayout>,
    /// Astral-projection construction camera (v0.464): true while the orbit cam is engaged
    /// for the editor, so we switch in/out exactly once (works for B AND the panel Close).
    pub(crate) construction_cam_active: bool,
    /// First-person position to return to when leaving the construction editor. (v0.464)
    pub(crate) construction_return_pos: Vec3,
    /// Last cursor position in physical pixels (top-left origin), for 3D picking. (v0.466)
    pub(crate) cursor_pos: (f32, f32),
    /// The room currently grabbed (left-drag) in the 3D astral editor. (v0.466)
    pub(crate) construction_grab: Option<ConstructionGrab>,
    /// Cached placement-ghost mesh for the held palette item (v0.529): (machine type, mesh idx,
    /// material idx). Rebuilt only when the held type changes, so the cursor-following ghost
    /// does not leak a fresh mesh every frame.
    pub(crate) construction_ghost: Option<(String, usize, usize)>,
    /// Held-STRUCTURE placement ghost (v0.583): (type_id ‖ yaw key, mesh idx, material idx).
    /// Rebuilt when the held type OR the placement yaw changes (the key encodes both), so the
    /// cursor-following structure preview is correct without leaking a mesh per frame.
    pub(crate) construction_structure_ghost: Option<(String, usize, usize)>,
    /// Teleporter re-fire cooldown in seconds (v0.584): set when the player jumps through a
    /// teleport pad; counts down each frame so standing on the destination pad doesn't ping-pong.
    pub(crate) teleport_cooldown: f32,
    /// ELEVATOR runtime state (v0.590), keyed by (ship-zone index, placed-structure index)
    /// (v0.754 -- every zone's elevators run): (anim 0..1 between base + top, target 0/1,
    /// was_riding). Pure runtime -- not saved with the home; resynced each frame during play.
    /// An idle elevator stays put (anim == target); stepping on toggles the target (ride to
    /// the other end); waiting in the shaft at floor level recalls the car.
    pub(crate) elevator_state: std::collections::HashMap<(usize, usize), (f32, f32, bool)>,
    /// Cached unit-box mesh + material for the moving elevator CAR slab (v0.590), scaled + posed
    /// per elevator per frame so the moving floor never leaks a mesh.
    pub(crate) elevator_car_mesh: Option<usize>,
    pub(crate) elevator_car_mat: Option<usize>,
    /// Cached unit-box mesh + translucent material for the wall-drawing tool (v0.534): the corner
    /// node marker under the cursor and the preview wall from the pending start to the cursor.
    /// Lazy-created once and reused (scaled/rotated per frame) so the preview never leaks a mesh.
    pub(crate) wall_tool_mesh: Option<usize>,
    pub(crate) wall_tool_mat: Option<usize>,
    /// Corner-node editing (v0.541): the position of the wall corner currently grabbed by its
    /// gizmo (None = not dragging). Dragging moves EVERY wall endpoint at this position (shared
    /// corners move together), snapped to the grid / box edges / other corners. Plus the cached
    /// gizmo sphere mesh + its normal + highlighted materials.
    pub(crate) construction_node_grab: Option<(f32, f32)>,
    pub(crate) construction_node_mesh: Option<usize>,
    pub(crate) construction_node_mat: Option<usize>,
    pub(crate) construction_node_mat_hot: Option<usize>,
    /// A grabbed OBJECT (light / machine / structure) being dragged across the floor (v0.593):
    /// drag moves its X/Z, keeping its Y (height). Armed by the pick fns, moved per frame by
    /// apply_object_drag past the tap-vs-drag threshold, cleared on release. Walls keep their own
    /// corner-orb drag; this is for the other object types the operator wanted movable.
    pub(crate) construction_object_grab: Option<ObjectGrab>,
    /// Viewport DRAG-TO-CONNECT (v0.625): the machine port currently being dragged to make a wire --
    /// (source machine id, utility, port direction, world start point). While Some, a rubber-band
    /// line follows the cursor; releasing over a machine that has a compatible (same-utility) port
    /// creates the connection. None when no port drag is in flight.
    pub(crate) construction_port_drag: Option<(String, crate::utilities::Utility, crate::utilities::PortDir, Vec3)>,
    /// Alignment-snap guides (v0.613): while dragging an object, if its X (or Z) lines up with
    /// another object's, the drag snaps to it and we stash the snapped coord here so the overlay can
    /// draw a guide line along that axis. None = no snap this frame. Cleared when no drag is active.
    pub(crate) construction_snap_x: Option<f32>,
    pub(crate) construction_snap_z: Option<f32>,
    /// Placed-LIGHT gizmo (v0.572): a cached DIAMOND (octahedron) centre-marker mesh + an emissive
    /// material, drawn at each placed light in build mode (the range "sphere" is RGB line circles).
    pub(crate) construction_light_mesh: Option<usize>,
    pub(crate) construction_light_mat: Option<usize>,
    /// Corridor DOOR-MOUTH handle material (v0.790): accent-toned emissive, cached once (the
    /// mesh is the shared light-gizmo octahedron). Materials are append-only in the renderer,
    /// so a theme accent edit re-tints on the next launch, like the other cached gizmo mats.
    pub(crate) construction_corridor_mouth_mat: Option<usize>,
    /// PHYSICAL light fixtures (v0.780, operator: "none of the lights have
    /// physical bulbs, they're all just empty points"). Cached UNIT meshes
    /// scaled per light every frame: a sphere (bulb), a thin panel box, and
    /// a unit tube along X (spot snout + straight strip bar). Play mode AND
    /// build mode -- the fixture is the real in-world look.
    pub(crate) light_fixture_sphere: Option<usize>,
    pub(crate) light_fixture_panel: Option<usize>,
    pub(crate) light_fixture_tube: Option<usize>,
    /// Emissive fixture materials keyed by quantized light color (ON), plus
    /// one shared dark material for OFF fixtures. Materials are append-only
    /// in the renderer, so cache per color instead of re-adding per frame.
    pub(crate) light_fixture_mats: std::collections::HashMap<[u8; 3], usize>,
    pub(crate) light_fixture_mat_off: Option<usize>,
    /// STRIP tube meshes (v0.781), keyed by (zone index, light index) ->
    /// (renderer mesh index, path hash). A strip's tube is world-space
    /// geometry built from its control path; rebuilt via replace_mesh only
    /// when the quantized path/subdivision hash changes (never per frame).
    pub(crate) light_strip_meshes: std::collections::HashMap<(usize, usize), (usize, u64)>,
    /// Wall-SELECT gizmo material (v0.573): a RED sphere at each wall's bottom-middle so you can
    /// click the wall (surface or orb) to select it; the SELECTED wall's orb uses the RGB hot mat.
    pub(crate) construction_wall_mat: Option<usize>,
    /// Build-mode gizmo HOVER material (v0.569): a brightened idle colour shown on the gizmo the
    /// cursor is over (idle -> hover -> active, like the header buttons).
    pub(crate) construction_node_mat_hover: Option<usize>,
    /// Build-mode player avatar (v0.557): whether its pyramid gizmo is grabbed, plus its cached
    /// body box / pyramid-gizmo meshes + material.
    pub(crate) construction_char_grab: bool,
    pub(crate) construction_char_mesh: Option<usize>,
    pub(crate) construction_char_pyramid_mesh: Option<usize>,
    pub(crate) construction_char_mat: Option<usize>,
    /// Door/window OPENING editing (v0.546): the (wall index, opening index) of the opening whose
    /// gizmo is grabbed -- dragging slides it ALONG its wall (updates `at`). A visually distinct
    /// cube gizmo (vs the corner spheres) + its cached mesh/material.
    pub(crate) construction_opening_grab: Option<(usize, usize)>,
    /// Opening RESIZE-handle grab (v0.578): (wall, opening, edge) where edge 0=left 1=right 2=top
    /// 3=bottom. Dragging a left/right handle changes width+at; top/bottom changes height+sill.
    pub(crate) construction_opening_resize: Option<(usize, usize, u8)>,
    pub(crate) construction_opening_mesh: Option<usize>,
    pub(crate) construction_opening_mat: Option<usize>,
    /// Cursor pixel position when a corner/opening gizmo was first pressed (v0.549). While this is
    /// Some, the press has NOT moved past the drag threshold yet -- so a release here is a CLICK
    /// (select + show on the right panel), not a move. It clears to None once the cursor moves past
    /// the threshold, which is what arms the actual drag (so click-and-HOLD moves, a tap selects).
    pub(crate) construction_grab_press: Option<(f32, f32)>,
    /// The door/window slide-gizmo handle currently grabbed in the 3D editor. (v0.468)
    pub(crate) construction_gizmo_grab: Option<ConstructionGizmoGrab>,
    /// Cached (mesh, material) for the gizmo MOVE handle marker, built once. (v0.468)
    pub(crate) construction_gizmo_handle: Option<(usize, usize)>,
    /// Cached (mesh, material) for the gizmo RESIZE handle markers (warning-tinted). (v0.469)
    pub(crate) construction_gizmo_resize_handle: Option<(usize, usize)>,
    /// Cached (mesh, material) for the selected-room highlight quad, built once. (v0.466)
    pub(crate) construction_hilite: Option<(usize, usize)>,
    // ── Multiplayer co-presence (v0.472) ──
    /// Processes inbound game messages + interpolates remote players. Reuses the authenticated
    /// chat WebSocket (`gui_state.ws_client`) -- no second socket, no second auth.
    pub(crate) net_sync: crate::net::sync::NetSyncSystem,
    /// True once we have sent `game_join` for this world session (cleared on leave/disconnect).
    pub(crate) game_joined: bool,
    /// Throttle for outbound position updates (send ~15/sec).
    pub(crate) game_pos_timer: f32,
    /// Cached (body_mesh, head_mesh, material) for the remote-player avatar marker, built once.
    pub(crate) remote_avatar: Option<(usize, usize, usize)>,
    /// Cached (body_mesh, head_mesh, material) for crew NPC markers (chore AI, v0.663), built once.
    pub(crate) remote_npc_avatar: Option<(usize, usize, usize)>,
    /// Solar system hologram bodies (mesh_idx, material_idx, local_position, name).
    pub(crate) hologram_objects: Vec<(usize, usize, Vec3, String)>,
    /// Hologram orbit rings (mesh_idx, material_idx).
    pub(crate) hologram_orbits: Vec<(usize, usize)>,
    /// Hologram pin markers (mesh_idx, material_idx, local_position, name).
    pub(crate) hologram_pins: Vec<(usize, usize, Vec3, String)>,
    /// Currently targeted hologram planet (name, if crosshair is on a pin).
    pub(crate) targeted_planet: Option<String>,
    /// Hologram room center (from data-driven layout).
    pub(crate) hologram_room_center: Vec3,
    /// Room lights currently lit (point or spot, see `RoomLight`).
    pub(crate) room_lights: Vec<crate::renderer::light::RoomLight>,
    /// Sealed homestead volume AABB (min, max), encompassing all rooms — the
    /// survival environment context: inside = oxygenated/heated, outside =
    /// vacuum/cold. None until the homestead generates.
    pub(crate) homestead_bounds: Option<(Vec3, Vec3)>,
    /// Live screenshot command counter (v0.639): monotonic per session, names
    /// `debug/screenshot_N.png` so repeated requests never collide.
    pub(crate) screenshot_counter: u32,
    /// Ship world position (GEO orbit coordinates).
    pub(crate) ship_world_pos: glam::DVec3,
    /// Dev travel home stash (v0.791.x): (ship_world_pos, camera position,
    /// yaw, pitch) captured on the FIRST teleport / FTL departure, so the
    /// Dev page's "Return home" restores the exact pre-travel viewpoint.
    /// None = at home (never traveled, or already returned).
    pub(crate) dev_travel_home: Option<(glam::DVec3, Vec3, f32, f32)>,
    /// Travel STEPPED OUT of the shared world (v0.801): set when engaging
    /// travel flipped copresence_solo (admin moderation path); Return home
    /// clears solo again so the avatar rejoins. Launcher-chosen solo is
    /// deliberately NOT cleared by returning home.
    pub(crate) dev_travel_stepped_out: bool,
    /// Frame-lock (v0.819): while Some(body_id), each frame moves the local
    /// scene to ride that body's rotating + orbiting frame, so its surface
    /// holds still relative to the viewer instead of the planet's ~64 km/s
    /// surface spin (and any orbital drift) sweeping it past. Set by the
    /// dev Travel teleport + `camera_request`; cleared by Return home.
    /// `frame_lock_anchor` is the camera's position in the body's UNROTATED
    /// local frame; `frame_lock_last_spin` tracks the spin for the view
    /// co-rotation. See `dev_travel::frame_lock_*`.
    pub(crate) frame_lock_body: Option<String>,
    /// Dev/showcase pin for the ocean sea state (None = follow the game
    /// weather's wind). Set via showcase_request {"sea":"0.8"|"auto"}.
    pub(crate) sea_state_override: Option<f32>,
    /// F6 pressed (v0.890): save a location bookmark next frame, where
    /// current_spin and the frame-lock state are fresh.
    pub(crate) bookmark_save_requested: bool,
    pub(crate) frame_lock_anchor: glam::DVec3,
    pub(crate) frame_lock_last_spin: f64,
    pub(crate) start_time: Instant,
    pub(crate) last_frame: Instant,
    // egui integration
    pub(crate) egui_ctx: egui::Context,
    pub(crate) egui_state: egui_winit::State,
    pub(crate) egui_renderer: egui_wgpu::Renderer,
    pub(crate) gui_state: GuiState,
    pub(crate) theme: Theme,
    /// Whether the 3D world has been fully initialized.
    pub(crate) world_loaded: bool,
    /// Boot-phase timer (dev tooling): accumulates named spans from
    /// `resumed()` + `load_world()` and emits a summary + debug/boot_timing.json.
    pub(crate) boot_timer: crate::boot_timing::BootTimer,
    /// Reserved for future use.
    pub(crate) window_shown: bool,
    /// Data directory path (resolved once at startup, used for deferred loading).
    pub(crate) data_dir: PathBuf,
    /// Whether a Ctrl/Cmd modifier key is currently held. Tracked from
    /// raw winit KeyboardInput because egui-winit swallows Ctrl+V at
    /// the winit layer (translates it to Event::Paste(text) and returns
    /// early WITHOUT pushing the V key event) — so egui's input never
    /// sees Ctrl+V for an image clipboard. We detect it here instead
    /// and set gui_state.pending_clipboard_paste. v0.234.
    pub(crate) ctrl_held: bool,
    /// Shift modifier state (v0.575), for Ctrl+Shift+Z redo in the construction editor.
    pub(crate) shift_held: bool,
    /// Alt modifier state (v0.735, operator directive): HOLDING Alt in
    /// first-person frees the OS cursor so the pinned machine card's
    /// buttons/dropdowns are clickable without leaving FPS; releasing Alt
    /// re-grabs. Mouse look is suppressed while held (reaching for a
    /// button must not spin the camera).
    pub(crate) alt_held: bool,
    /// Left-mouse-button held state (v0.575): true while dragging a gizmo or a slider, so the undo
    /// history coalesces a continuous drag into one step (checkpoint on release).
    pub(crate) lmb_held: bool,
    /// Construction-editor undo/redo history (v0.575): bounded snapshot stacks of the editable
    /// home (structure + machines), captured at the dirty-flag choke point.
    pub(crate) construction_history: ConstructionHistory,
    /// Live broadcast (v0.853). Present only while actually streaming; dropping it
    /// stops the stream. Lives here rather than in GuiState because it needs the
    /// renderer's swapchain texture, which the GUI never sees.
    pub(crate) live_publisher: Option<crate::net::live::LivePublisher>,
    /// Reusable async readback slot for the broadcast. Cheap when idle (it holds no
    /// buffer until the first capture).
    pub(crate) stream_capture: crate::renderer::stream_capture::StreamCapture,
}

/// One captured editor state for undo/redo (v0.575): a clone of the editable SHIP (all zones,
/// v0.754 -- so zone add/delete/origin edits undo too) + machines. Selection is intentionally
/// NOT captured -- restoring it would yank the right panel to a stale wall; the current
/// selection is kept (and self-clamps if it falls out of range).
#[derive(Clone, Default)]
pub(crate) struct EditorSnapshot {
    pub(crate) structure: Option<ShipStructure>,
    pub(crate) machines: Option<crate::machines::MachineHome>,
}

/// Bounded undo/redo history for the construction editor (v0.575). Snapshot model: cheap (the home
/// is tens of KB) and robust (no per-action inverse). A continuous DRAG -- a gizmo OR a slider --
/// is coalesced into ONE step by only checkpointing while the left mouse button is NOT held, plus
/// once on release if an edit happened during the hold.
#[derive(Default)]
pub(crate) struct ConstructionHistory {
    pub(crate) undo: std::collections::VecDeque<EditorSnapshot>,
    pub(crate) redo: Vec<EditorSnapshot>,
    /// The committed state as of the last checkpoint -- pushed onto `undo` when the next edit lands.
    pub(crate) baseline: EditorSnapshot,
    /// Whether an edit happened during the current LMB hold (so a click/drag that changed nothing
    /// won't checkpoint).
    pub(crate) edited_during_hold: bool,
    /// Whether the LMB was held last frame (to detect release).
    pub(crate) prev_held: bool,
    /// Whether the editor was open last frame (to reset history on open).
    pub(crate) prev_active: bool,
}

/// Borrowed views of THIS frame's already-built scene draw lists (v0.810).
/// The hi-res offscreen capture re-runs the exact same passes the live frame
/// just ran -- sky (galaxy glow, stars, halos, constellations), celestial
/// bodies + atmospheres, orbit lines, world geometry, glass, gizmos, door
/// rings -- so the PNG shows precisely what the player sees, only at the
/// requested resolution. Zero cost when no capture is pending.
pub(crate) struct SceneDrawLists<'a> {
    pub(crate) celestial: &'a [RenderObject],
    pub(crate) celestial_transparent: &'a [RenderObject],
    pub(crate) orbit_lines: &'a [crate::renderer::line::LineVertex],
    pub(crate) opaque: &'a [RenderObject],
    pub(crate) transparent: &'a [RenderObject],
    pub(crate) overlay: &'a [RenderObject],
    pub(crate) ring_lines: &'a [crate::renderer::line::LineVertex],
}
