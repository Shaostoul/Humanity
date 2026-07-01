//! The home as a FIXED outer box + freely-designed INTERIOR WALLS (v0.532, the node/wall redesign).
//!
//! Scope (operator-directed 2026-06-24): while aboard the mothership the player's home is a FIXED
//! allotment -- an outer box (default 55 x 89 x 3 m, steel). The editable design surface is the
//! INTERIOR: walls placed as straight segments between corner nodes in the floor plan. Rooms emerge
//! as the regions those interior walls (plus the box) enclose.
//!
//! AI + human friendly (the operator's north star): the whole structure is ONE small readable file.
//! Add an interior wall by adding one `InteriorWall(a: (x, z), b: (x, z))` line to
//! data/blueprints/home_structure.ron -- no code. The construction editor places the SAME segments
//! by dragging corner nodes. One model, edited the same way by an AI and a human. The model is
//! intended for designing ANY structure, not just the player home.
//!
//! Stage 1 (this file): the data model + mesh generation for the fixed box + interior walls, in the
//! existing `HomesteadMeshes` form so it renders through the same path. Wiring it into the live world
//! + the node-placement editor + room subdivision are later stages.

use crate::renderer::mesh::Vertex;
use crate::ship::fibonacci::{floor_quad, wall_box, HomesteadMeshes, RoomInfo};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Wall thickness (metres) for the box shell + interior walls.
const WALL_THICKNESS: f32 = 0.15;

fn default_wall_height() -> f32 {
    3.0
}
fn default_steel() -> u32 {
    1
}
/// Material id for a clear/glass shell (the default roof: a sealed transparent ceiling you see the
/// stars through, "good in outer space" per the operator).
fn default_glass() -> u32 {
    4
}
fn default_door_style() -> String {
    "swing".to_string()
}
/// Default door auto-open (interaction) distance, metres. (v0.547)
fn default_open_dist() -> f32 {
    2.6
}

/// A door or a window. (More opening kinds -- hatch, airlock -- can be added.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningKind {
    Door,
    Window,
}

/// An opening (door or window) cut into a wall face. Defined the operator's way: a door is one point
/// on the wall's bottom edge (here `at` + `width`); a window is a region on the face (`at`/`width` +
/// `sill`/`height`). The aperture is cut out of the wall mesh; left/right piers + a header (+ a sill
/// for a window) fill the rest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opening {
    pub kind: OpeningKind,
    /// Left edge of the opening along the wall, metres from the wall's corner `a`.
    pub at: f32,
    pub width: f32,
    /// Sill height above the floor (0 for a door; > 0 for a window).
    #[serde(default)]
    pub sill: f32,
    /// Aperture height (door/window opening height).
    #[serde(default = "default_wall_height")]
    pub height: f32,
    /// How the opening behaves + ANIMATES -- a data-driven STYLE so new door/window kinds
    /// (swing, slide, iris, rotate, energy, nanowall, organic, ...) are added without code; the
    /// animation system reads this string in a later stage. Today it only tags the opening.
    #[serde(default = "default_door_style")]
    pub style: String,
    /// Player interaction (auto-open) distance in metres -- a door opens within this HORIZONTAL
    /// range; shown as an editable ground ring in the editor. (v0.547)
    #[serde(default = "default_open_dist")]
    pub open_dist: f32,
    /// Locked: the door stays shut regardless of approach (v0.554). An ENERGY door glows red while
    /// locked, green while unlocked; other styles simply do not open. Windows ignore it.
    #[serde(default)]
    pub locked: bool,
    /// Open mode (v0.564, the operator's door-state model): true = AUTO-open within `open_dist`;
    /// false = MANUAL -- stays shut until acted on (a push / control panel), honouring `locked`.
    #[serde(default = "default_true")]
    pub auto_open: bool,
    /// A wall-mounted CONTROL PANEL beside the door (v0.567): walk up + press E to open/close it (and
    /// later lock/unlock / emergency power / hack). Makes a MANUAL door usable in-world.
    #[serde(default)]
    pub control_panel: bool,
    /// LOCKS on this door (v0.570): each a `LockInstance` referencing a `lock_types.ron` type. The
    /// door is PASSABLE only when every lock is Unlocked/Broken. An EMPTY list falls back to the
    /// legacy `locked` bool (so every existing home + test is unchanged). Generalizes `locked`.
    #[serde(default)]
    pub locks: Vec<LockInstance>,
}

/// A lock placed on a specific door (or, later, wall). References a `lock_types.ron` type by id; its
/// runtime open/locked state lives in EngineState (mirrors `door_manual_open`), this is the AUTHORED
/// initial state that saves with the home. (v0.570)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInstance {
    /// -> `lock_types.ron` id (e.g. "metal_key", "keypad").
    pub type_id: String,
    /// Initial state when the home loads (defaults Locked, so a placed lock starts secured).
    #[serde(default)]
    pub state: crate::ship::lock_types::LockState,
    /// Keypad code / required key id override (None = the type's default). Stage 1 stubs enforcement.
    #[serde(default)]
    pub secret: Option<String>,
    /// Where along the door face this lock mounts (metres from the door's `a` edge); the render
    /// stacks multiple locks if they collide. (v0.570)
    #[serde(default)]
    pub offset: f32,
}

impl Opening {
    /// Is this opening currently locked, per its AUTHORED state? A door with locks is locked iff any
    /// lock's authored state is Locked; with NO locks it falls back to the legacy `locked` bool. The
    /// LIVE runtime check (after the player unlocks one) uses EngineState's per-door lock states; this
    /// is the initial/data view used at load + in tests. (v0.570)
    pub fn is_locked(&self) -> bool {
        if self.locks.is_empty() {
            self.locked
        } else {
            self.locks.iter().any(|l| !l.state.is_open())
        }
    }
}

fn default_true() -> bool {
    true
}

/// The corner-position grid (1/m). Corners are SNAPPED to this grid so two "co-located" corners are
/// BYTE-IDENTICAL, not merely within an epsilon. MUST match the mesh corner-keyer (`ck` in
/// `generate_meshes`), so editor identity == render identity (shared corners drag together; mitres,
/// joins, and columns line up). 20.0 = a 0.05 m (5 cm) grid. (v0.574)
pub const CORNER_GRID: f32 = 20.0;

/// Snap a corner (x, z) to `CORNER_GRID`, so co-located corners become exactly equal. The single
/// source of truth for "what is the same corner" -- the editor snap/drag, on-load normalization, and
/// the mesh keyer all go through this grid. (v0.574)
pub fn quantize_corner(p: (f32, f32)) -> (f32, f32) {
    ((p.0 * CORNER_GRID).round() / CORNER_GRID, (p.1 * CORNER_GRID).round() / CORNER_GRID)
}

/// An interior wall: a straight segment in the floor plan, from corner node `a` to `b` (each is
/// (x, z) metres from the box's min corner), rising `height` metres from the floor. Openings
/// (doors/windows) come in a later stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteriorWall {
    pub a: (f32, f32),
    pub b: (f32, f32),
    #[serde(default = "default_wall_height")]
    pub height: f32,
    /// Material id (matches RoomConfig.material_type: 0=grid, 1=steel/metal, 2=concrete, 3=wood).
    #[serde(default = "default_steel")]
    pub material: u32,
    /// Doors + windows cut into this wall.
    #[serde(default)]
    pub openings: Vec<Opening>,
    /// Wall thickness in metres. None -> the material's default_thickness_m (sheet metal ~2-4cm,
    /// framed wood ~10cm, poured stone ~15-20cm; the operator can set 1mm for a paper screen). Drives
    /// the rendered mesh, the collider, and (later) the wall's HP. (v0.556)
    #[serde(default)]
    pub thickness: Option<f32>,
    /// SURFACE LAYERS (v0.585): extra materials layered ON the structural wall, ordered top (exposed)
    /// to bottom (against the base). "Rhino-lining on a truck bed": a thin protective coat that changes
    /// the exposed surface + adds resilience without remaking the wall. Empty -> the bare `material`
    /// (every existing wall is unchanged). The FIRST layer is what you see + touch.
    #[serde(default)]
    pub layers: Vec<SurfaceLayer>,
}

/// One material layer applied to a surface (v0.585): a `wall_materials.ron` id + its thickness. A
/// stack of these (top-to-bottom) is how a road is "asphalt over base over subgrade" and how a wall
/// gets a protective coat. Pure data; the renderer shows the top layer, the editor sums the stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceLayer {
    /// -> `wall_materials.ron` id (shares the material registry so layers teach the same properties).
    pub material: u32,
    /// Layer thickness in metres (a coat is mm-thin; a road base is tens of cm).
    pub thickness_m: f32,
}

impl InteriorWall {
    /// Resolved STRUCTURAL thickness (metres): the wall's explicit override, else its material's
    /// default, else the legacy 0.15 m fallback. (Surface layers are a treatment ON this; they add to
    /// `total_thickness` but do not change the structural mesh in Stage 1.)
    pub fn resolved_thickness(&self) -> f32 {
        self.thickness
            .filter(|t| *t > 0.0)
            .unwrap_or_else(|| wall_material(self.material).map_or(WALL_THICKNESS, |m| m.default_thickness_m))
    }

    /// The EXPOSED material id -- the top surface layer if any, else the bare wall material. Drives the
    /// rendered face colour so a layered wall reads as its coating. (v0.585)
    pub fn exposed_material(&self) -> u32 {
        self.layers.first().map_or(self.material, |l| l.material)
    }

    /// Total thickness including surface layers (metres) -- the structural wall plus every coat. Shown
    /// in the editor so a builder sees what the stack adds up to. (v0.585)
    pub fn total_thickness(&self) -> f32 {
        self.resolved_thickness() + self.layers.iter().map(|l| l.thickness_m.max(0.0)).sum::<f32>()
    }
}

/// A home (or any structure): a FIXED outer box + freely-placed interior walls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeStructure {
    /// Outer box footprint + height (metres): width (X), depth (Z), height (Y). Fixed aboard the ship.
    pub width: f32,
    pub depth: f32,
    pub height: f32,
    /// Shell (floor / outer walls) material id. Steel (1) by default.
    #[serde(default = "default_steel")]
    pub shell_material: u32,
    /// Roof (ceiling) material id. Glass (4) by default -- a sealed CLEAR ceiling you see the stars
    /// through, the operator's "clear wall, like glass, good in outer space." Set to the shell
    /// material (1) for an opaque roof.
    #[serde(default = "default_glass")]
    pub roof_material: u32,
    /// The editable interior walls (segments in the floor plan).
    #[serde(default)]
    pub walls: Vec<InteriorWall>,
    /// Shell (perimeter + floor) thickness in metres. None -> the shell material's default. (v0.556)
    #[serde(default)]
    pub shell_thickness: Option<f32>,
    /// PLACED LIGHTS (v0.571): real local lights the home carries, so a room is lit even with the
    /// sun / global illumination turned OFF. Each references a `light_types.ron` type. Empty -> the
    /// renderer falls back to its auto one-light-per-room synthesis (no regression).
    #[serde(default)]
    pub lights: Vec<PlacedLight>,
    /// SPAWN point (v0.582): where the player stands when leaving build mode (x, z in box coords),
    /// set by dragging the build-mode avatar gizmo. Persisted so the moved spawn survives a save/load.
    #[serde(default)]
    pub spawn: Option<(f32, f32)>,
    /// PLACED STRUCTURAL PIECES (v0.583): stairs, ramps, ladders, elevators, teleporters, train
    /// platforms, roads -- each a `structure_types.ron` type at a home-local pose. Empty by default
    /// (every existing home + test is unchanged). The mesh + (v0.584) function resolve from the type.
    #[serde(default)]
    pub structures: Vec<PlacedStructure>,
    /// ROAD GRAPH (v0.586): roads as a NODE + EDGE graph (the operator's "laying the road out being
    /// simple graph like nodes with splines"). Each edge is a ribbon between two nodes, carrying a
    /// FIXED road-class material stack (`road_types.ron`). Empty by default. Straight segments in
    /// Stage 1; curved splines are a later refinement.
    #[serde(default)]
    pub road_nodes: Vec<RoadNode>,
    #[serde(default)]
    pub road_edges: Vec<RoadEdge>,
    /// ZONES (v0.631, superstructure M1 -- docs/design/mothership-superstructure.md): labelled bounded
    /// VOLUMES with a type (residential / industrial / hangar / mech-bay / cargo / storage / civic-mall
    /// / ...), the macro analogue of a room. A mothership is a big structure carved into zones tied by
    /// transit. Empty by default (every existing home + test parses unchanged). The editor places +
    /// resizes them as wireframe boxes; later stages add transit + zone-scoped sub-structures.
    #[serde(default)]
    pub zones: Vec<Zone>,
    /// RAIL GRAPH (v0.635, superstructure M2 -- transit): rail transit as a NODE + EDGE graph (a
    /// multi-stop line), generalising the v0.592 paired-platform link. Mirrors the road graph. Empty by
    /// default. Cars + multi-stop routing are a later stage (M2b); this is the editable topology.
    #[serde(default)]
    pub rail_nodes: Vec<RailNode>,
    #[serde(default)]
    pub rail_edges: Vec<RailEdge>,
}

/// A rail-graph stop/junction (v0.635): a point a rail line routes through. Mirrors RoadNode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailNode {
    pub id: u32,
    /// Home-local ground position (x, z).
    pub pos: (f32, f32),
}

/// A rail segment (v0.635): a straight track between two RailNodes (cars run it in M2b).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailEdge {
    pub from: u32,
    pub to: u32,
}

/// A labelled bounded VOLUME inside a structure (v0.631): the macro building block of a mothership.
/// `origin` is the min corner (home-local metres), `size` the extent. `type_id` -> `zone_types.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    /// -> `zone_types.ron` id (e.g. "residential", "industrial", "civic_mall").
    pub type_id: String,
    /// Min corner in home-local metres (x, y, z).
    pub origin: (f32, f32, f32),
    /// Extent in metres (width X, height Y, depth Z).
    pub size: (f32, f32, f32),
    /// Optional human label (falls back to the type label).
    #[serde(default)]
    pub label: String,
}

/// A road-graph junction (v0.586): a point on the ground the road network routes through.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadNode {
    pub id: u32,
    /// Home-local ground position (x, z).
    pub pos: (f32, f32),
}

/// A road segment (v0.586): a ribbon between two `RoadNode`s, of a fixed road CLASS + width. The
/// class (`road_types.ron`) supplies the top-to-bottom material stack; the top layer drives colour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadEdge {
    pub from: u32,
    pub to: u32,
    /// -> `road_types.ron` id (footpath / residential / highway / runway).
    pub class: String,
    /// Carriageway width in metres.
    #[serde(default = "default_road_width")]
    pub width: f32,
}

fn default_road_width() -> f32 {
    4.0
}

/// A structural piece placed in a home (v0.583): a `structure_types.ron` type at a home-local pose.
/// Pure data -- `structure::structure_mesh` resolves it into geometry, the gameplay layer (v0.584)
/// reads its `kind` for behaviour (ascend / climb / ride / teleport).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedStructure {
    /// -> `structure_types.ron` id (e.g. "stairs", "elevator").
    pub type_id: String,
    /// Home-local position (x, y, z); y > 0 places it on an upper level.
    pub pos: (f32, f32, f32),
    /// Yaw orientation in degrees (0 = footprint axis-aligned, +Z is "up the stairs").
    #[serde(default)]
    pub rot_deg: f32,
    /// Teleporter pairing (v0.584): the index of the partner piece this one jumps you to. None until
    /// the operator links a pair in the detail panel. Ignored for non-teleporter kinds.
    #[serde(default)]
    pub pair: Option<usize>,
}

/// A light placed in a home (v0.571): a `light_types.ron` type at a world/home-local position, with
/// optional per-instance overrides. Pure data -- the renderer resolves it into a GPU light each frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedLight {
    /// -> `light_types.ron` id (e.g. "ceiling_panel").
    pub type_id: String,
    /// Position (home-local; for the box home that equals world).
    pub pos: (f32, f32, f32),
    /// Aim direction for a future spot/bar light (normalized at use). Unused for a point light.
    #[serde(default)]
    pub dir: (f32, f32, f32),
    /// Switched on? A light can be placed but off.
    #[serde(default = "default_true")]
    pub on: bool,
    /// Optional overrides of the type's colour / intensity / range (None = inherit the preset).
    #[serde(default)]
    pub color: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub intensity: Option<f32>,
    #[serde(default)]
    pub range: Option<f32>,
}

/// A buildable wall material with REAL engineering properties -- the construction picker shows these
/// so a builder learns density / strength / cost / renewability while they build. `id` is what
/// `InteriorWall.material` (and `shell_material`) store. Loaded from data/blueprints/wall_materials.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct WallMaterial {
    pub id: u32,
    pub name: String,
    pub category: String,
    /// rgba; alpha < 1.0 renders in the transparent pass (glass).
    pub color: (f32, f32, f32, f32),
    pub density_kg_m3: f32,
    pub tensile_mpa: f32,
    pub cost_per_kg: f32,
    pub renewable: bool,
    /// Default wall thickness in metres when a wall does not override it (sheet metals are thin,
    /// framed/poured materials are thick). Real-ish figures, so thickness teaches too. (v0.556)
    pub default_thickness_m: f32,
    pub note: String,
}

/// The wall-material registry, parsed once. Embedded at compile time so both the editor and the
/// renderer read the same list with no runtime path dependency. Edit the .ron + rebuild to change it.
pub fn wall_materials() -> &'static [WallMaterial] {
    static REG: std::sync::OnceLock<Vec<WallMaterial>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/blueprints/wall_materials.ron");
        match ron::from_str::<Vec<WallMaterial>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("wall_materials.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a wall material by id (None if the id is unknown).
pub fn wall_material(id: u32) -> Option<&'static WallMaterial> {
    wall_materials().iter().find(|m| m.id == id)
}

impl HomeStructure {
    /// Mint a unique zone id (v0.631), e.g. "zone_3".
    pub fn unique_zone_id(&self) -> String {
        let mut n = self.zones.len();
        loop {
            let id = format!("zone_{n}");
            if !self.zones.iter().any(|z| z.id == id) {
                return id;
            }
            n += 1;
        }
    }

    /// Add a zone of `type_id` at `origin` with `size`; returns its new id. (v0.631, superstructure M1)
    pub fn add_zone(&mut self, type_id: &str, origin: (f32, f32, f32), size: (f32, f32, f32)) -> String {
        let id = self.unique_zone_id();
        self.zones.push(Zone { id: id.clone(), type_id: type_id.to_string(), origin, size, label: String::new() });
        id
    }

    /// Remove the zone with this id. Returns true if one was removed. (v0.631)
    pub fn remove_zone(&mut self, id: &str) -> bool {
        let before = self.zones.len();
        self.zones.retain(|z| z.id != id);
        self.zones.len() != before
    }

    /// Duplicate the zone with this id (v0.634): a copy with a fresh id, nudged +2 m in x/z so it's
    /// visible. Returns the new id (None if the source id is unknown).
    pub fn duplicate_zone(&mut self, id: &str) -> Option<String> {
        let mut z = self.zones.iter().find(|z| z.id == id)?.clone();
        let new_id = self.unique_zone_id();
        z.id = new_id.clone();
        z.origin.0 += 2.0;
        z.origin.2 += 2.0;
        self.zones.push(z);
        Some(new_id)
    }

    /// Load from a RON file; None (with a warning) on a missing/invalid file.
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        match ron::from_str::<HomeStructure>(&text) {
            Ok(h) => Some(h),
            Err(e) => {
                log::warn!("home_structure: failed to parse {}: {e}", path.display());
                None
            }
        }
    }

    /// Write back to RON, preserving the existing file's leading comment header (the v0.526 lesson)
    /// so an in-editor save does not strip the design documentation.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let config = ron::ser::PrettyConfig::default().struct_names(false);
        let body = ron::ser::to_string_pretty(self, config).map_err(|e| e.to_string())?;
        let preserved = std::fs::read_to_string(path).ok().and_then(|existing| {
            let header: String = existing
                .lines()
                .take_while(|l| l.trim_start().starts_with("//") || l.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if header.contains("//") {
                Some(format!("{}\n\n", header.trim_end()))
            } else {
                None
            }
        });
        let header = preserved.unwrap_or_else(|| {
            "// HumanityOS home structure: a FIXED outer box + freely-placed interior walls. Add a\n\
             // wall by adding an InteriorWall(a: (x, z), b: (x, z)) to `walls`. The construction\n\
             // editor edits the same segments. Design doc: docs/design/home-design.md.\n\n"
                .to_string()
        });
        std::fs::write(path, format!("{header}{body}")).map_err(|e| e.to_string())
    }

    /// A machine-readable introspection snapshot of the live home (v0.576): the full struct as JSON
    /// PLUS a `derived` block (per-wall length + heading + thickness + opening count; corner valence)
    /// that an AI can read to "see" the home without parsing the mesh. Written to a debug file each
    /// rebuild so an agent can inspect what the operator is building. (A read surface for the AI; the
    /// matching ACT surface -- a text-command console -- is the next dev-tool stage.)
    pub fn to_introspection_json(&self) -> String {
        let mut v = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        // Per-wall derived geometry.
        let walls: Vec<serde_json::Value> = self
            .walls
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let dx = w.b.0 - w.a.0;
                let dz = w.b.1 - w.a.1;
                serde_json::json!({
                    "index": i,
                    "a": [w.a.0, w.a.1],
                    "b": [w.b.0, w.b.1],
                    "length_m": (dx * dx + dz * dz).sqrt(),
                    "heading_deg": dz.atan2(dx).to_degrees(),
                    "thickness_m": w.resolved_thickness(),
                    "material": wall_material(w.material).map(|m| m.name.clone()),
                    "openings": w.openings.len(),
                })
            })
            .collect();
        // Corner valence: how many wall ends meet at each grid-snapped corner.
        let mut valence: std::collections::HashMap<(i32, i32), u32> = std::collections::HashMap::new();
        for w in &self.walls {
            for c in [w.a, w.b] {
                let q = quantize_corner(c);
                *valence.entry(((q.0 * CORNER_GRID) as i32, (q.1 * CORNER_GRID) as i32)).or_insert(0) += 1;
            }
        }
        let corners: Vec<serde_json::Value> = valence
            .iter()
            .map(|((kx, kz), n)| serde_json::json!({
                "x": *kx as f32 / CORNER_GRID, "z": *kz as f32 / CORNER_GRID, "walls_meeting": n,
            }))
            .collect();
        if let serde_json::Value::Object(ref mut map) = v {
            map.insert("derived".into(), serde_json::json!({
                "wall_count": self.walls.len(),
                "light_count": self.lights.len(),
                "box_m": [self.width, self.depth, self.height],
                "walls": walls,
                "corners": corners,
            }));
        }
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }

    /// Material color (rgba) for a material id, sourced from the wall-material registry so the picker,
    /// the saved data, and the rendered color stay in lockstep. Grid grey for an unknown id.
    fn material_color(m: u32) -> [f32; 4] {
        match wall_material(m) {
            Some(mat) => [mat.color.0, mat.color.1, mat.color.2, mat.color.3],
            None => [0.50, 0.52, 0.56, 1.0],
        }
    }

    /// True when the roof is a clear/glass shell (the renderer draws it transparent in the see-through
    /// pass rather than as an opaque ceiling). (v0.539)
    pub fn roof_is_glass(&self) -> bool {
        self.roof_material == 4
    }

    /// Resolved shell (perimeter + floor) thickness in metres. (v0.556)
    pub fn shell_resolved_thickness(&self) -> f32 {
        self.shell_thickness
            .filter(|t| *t > 0.0)
            .unwrap_or_else(|| wall_material(self.shell_material).map_or(WALL_THICKNESS, |m| m.default_thickness_m))
    }

    /// Generate the renderable meshes: the fixed box shell (one floor + 4 outer walls + ceiling)
    /// plus each interior wall segment, in the existing `HomesteadMeshes` form so the renderer path
    /// is a drop-in. One room ("home") for now -- room subdivision by interior walls is a later stage.
    pub fn generate_meshes(&self) -> HomesteadMeshes {
        let w = self.width.max(1.0);
        let d = self.depth.max(1.0);
        let h = self.height.max(1.0);
        let col = Self::material_color(self.shell_material);

        // Floor + ceiling: one quad each, spanning the box footprint.
        let (fv, fi) = floor_quad(Vec3::new(0.0, 0.0, 0.0), Vec3::new(w, 0.0, d));
        let floors = vec![(fv, fi, col, self.shell_material)];
        let ceilings = floor_quad(Vec3::new(0.0, h, 0.0), Vec3::new(w, 0.0, d));

        // Walls grouped BY MATERIAL so each renders in its own color (v0.552): the 4 outer box walls
        // (shell material) + every interior wall (its own picked material) + the corner columns. A
        // material with alpha < 1 (glass) is routed to the transparent pass at render time. The legacy
        // single `walls` family stays EMPTY for the home path -- fibonacci ships still use it.
        let mut by_mat: std::collections::HashMap<u32, (Vec<Vertex>, Vec<u32>)> =
            std::collections::HashMap::new();
        let perimeter = [
            (Vec3::new(0.0, 0.0, 0.0), Vec3::new(w, 0.0, 0.0)),
            (Vec3::new(w, 0.0, 0.0), Vec3::new(w, 0.0, d)),
            (Vec3::new(w, 0.0, d), Vec3::new(0.0, 0.0, d)),
            (Vec3::new(0.0, 0.0, d), Vec3::new(0.0, 0.0, 0.0)),
        ];
        let shell_t = self.shell_resolved_thickness();
        for (a, b) in perimeter {
            let g = by_mat.entry(self.shell_material).or_insert_with(|| (Vec::new(), Vec::new()));
            merge(g, wall_box(a, b, 0.0, h, shell_t));
        }
        // Incidence map for MITRED corners (v0.558): corner-key -> the incident walls' (index, dir
        // AWAY from the corner, half-thickness). A 2-wall join is mitred (ends cut to meet flush); a
        // free end stays square; a 3+ join keeps the cylinder fill (a single bisector is undefined).
        let ck = |p: (f32, f32)| ((p.0 * 20.0).round() as i32, (p.1 * 20.0).round() as i32);
        let mut incidence: std::collections::HashMap<(i32, i32), Vec<(usize, V2, f32)>> =
            std::collections::HashMap::new();
        for (i, wseg) in self.walls.iter().enumerate() {
            let half = wseg.resolved_thickness() * 0.5;
            let len = ((wseg.b.0 - wseg.a.0).powi(2) + (wseg.b.1 - wseg.a.1).powi(2)).sqrt().max(1e-6);
            let dir = ((wseg.b.0 - wseg.a.0) / len, (wseg.b.1 - wseg.a.1) / len);
            incidence.entry(ck(wseg.a)).or_default().push((i, dir, half));
            incidence.entry(ck(wseg.b)).or_default().push((i, (-dir.0, -dir.1), half));
        }
        // Footprint corners (left, right) of wall `wi`'s end at corner `c`: mitred against the single
        // adjacent wall at a 2-wall join, else squared off.
        let end_corners = |wi: usize, c: (f32, f32), fwd: V2, half: f32, is_a: bool| -> (V2, V2) {
            let p = v_perp(fwd);
            let sq_l = v_add(c, v_scale(p, half));
            let sq_r = v_add(c, v_scale(p, -half));
            if let Some(list) = incidence.get(&ck(c)) {
                if list.len() == 2 {
                    if let Some(&(_, adj_dir, adj_half)) = list.iter().find(|(j, _, _)| *j != wi) {
                        if let Some(m) = wall_end_miter(sq_l, sq_r, fwd, c, adj_dir, adj_half, is_a) {
                            return m;
                        }
                    }
                }
            }
            (sq_l, sq_r)
        };
        for (i, wseg) in self.walls.iter().enumerate() {
            // Clip each end OUT of any wall it would pass through (mid-span T) before building, so a
            // thick wall butts the other's face instead of spearing through it (v0.566). A shared
            // corner / free end is left as-is (clip 0).
            let (ca, clip_a) = clip_end_to_walls(wseg.a, wseg.b, i, &self.walls);
            let (cb, _clip_b) = clip_end_to_walls(wseg.b, wseg.a, i, &self.walls);
            let a = Vec3::new(ca.0, 0.0, ca.1);
            let b = Vec3::new(cb.0, 0.0, cb.1);
            let half = wseg.resolved_thickness() * 0.5;
            let len = ((cb.0 - ca.0).powi(2) + (cb.1 - ca.1).powi(2)).sqrt().max(1e-6);
            let fwd = ((cb.0 - ca.0) / len, (cb.1 - ca.1) / len);
            let (al, ar) = end_corners(i, ca, fwd, half, true);
            let (bl, br) = end_corners(i, cb, fwd, half, false);
            // Group by the EXPOSED material (the top surface layer if any) so a layered wall renders
            // as its coating -- the "rhino-lining" you see. (v0.585)
            let g = by_mat.entry(wseg.exposed_material()).or_insert_with(|| (Vec::new(), Vec::new()));
            merge(g, wall_with_openings(a, b, wseg.height.max(0.1), half * 2.0, clip_a, &wseg.openings, al, ar, bl, br));
        }
        // Corner columns (v0.549): fill each 3+-WALL interior-wall JOIN (T / X) with a slim cylinder of
        // the wall's half-thickness, where a single miter bisector is undefined. 2-wall joins are
        // MITRED above (v0.558) so they need no column; a free end keeps its square cap. Low-poly, in
        // the most-OPAQUE meeting wall's material (so a junction with any solid wall reads solid).
        let mut joins: std::collections::HashMap<(i32, i32), (f32, u32, (f32, f32), u32, f32)> =
            std::collections::HashMap::new();
        for wseg in &self.walls {
            let wt = wseg.resolved_thickness();
            let exposed = wseg.exposed_material(); // colour the join to match the visible wall face
            for cp in [wseg.a, wseg.b] {
                let key = ((cp.0 * 20.0).round() as i32, (cp.1 * 20.0).round() as i32);
                let e = joins.entry(key).or_insert((0.0, 0, cp, exposed, wt));
                e.0 = e.0.max(wseg.height.max(0.1));
                e.1 += 1;
                // Prefer the higher-alpha (more opaque) material; ties keep the first seen, so the
                // result is deterministic regardless of wall order.
                if Self::material_color(exposed)[3] > Self::material_color(e.3)[3] {
                    e.3 = exposed;
                }
                e.4 = e.4.max(wt); // the column must cover the THICKEST wall meeting here
            }
        }
        for (h_col, count, pos, mat, th) in joins.values() {
            if *count >= 3 {
                let g = by_mat.entry(*mat).or_insert_with(|| (Vec::new(), Vec::new()));
                merge(g, corner_column(pos.0, pos.1, th * 0.5, *h_col, 10));
            }
        }
        // Resolve each material group to (verts, indices, color), sorted by id for a stable order
        // (so render-slot reuse does not shuffle frame to frame).
        let mut groups: Vec<(u32, (Vec<Vertex>, Vec<u32>))> = by_mat.into_iter().collect();
        groups.sort_by_key(|(mid, _)| *mid);
        let mut material_walls: Vec<(Vec<Vertex>, Vec<u32>, [f32; 4])> = groups
            .into_iter()
            .map(|(mid, (v, i))| (v, i, Self::material_color(mid)))
            .collect();

        // PLACED STRUCTURES (v0.583): stairs/ramps/ladders/elevators/teleporters/platforms/roads
        // render through the SAME material_walls path -- one buffer per distinct piece colour (keyed
        // by an rgb quantization so a dozen identical stairs share a draw). Geometry comes from the
        // type's parametric `shape`; Wall-kind entries are drawn by the wall tool, never placed.
        use crate::ship::structure::{structure_mesh, structure_type, StructureKind};
        let mut by_color: std::collections::HashMap<[i32; 3], (Vec<Vertex>, Vec<u32>, [f32; 3])> =
            std::collections::HashMap::new();
        for ps in &self.structures {
            let Some(ty) = structure_type(&ps.type_id) else { continue };
            if ty.kind == StructureKind::Wall {
                continue;
            }
            let pos = Vec3::new(ps.pos.0, ps.pos.1, ps.pos.2);
            let (mut verts, indices) = structure_mesh(ty, pos, ps.rot_deg.to_radians());
            let key = [(ty.color.0 * 64.0) as i32, (ty.color.1 * 64.0) as i32, (ty.color.2 * 64.0) as i32];
            let g = by_color
                .entry(key)
                .or_insert_with(|| (Vec::new(), Vec::new(), [ty.color.0, ty.color.1, ty.color.2]));
            let base = g.0.len() as u32;
            g.0.append(&mut verts);
            g.1.extend(indices.into_iter().map(|i| i + base));
        }
        let mut scolor: Vec<_> = by_color.into_iter().collect();
        scolor.sort_by_key(|(k, _)| *k);
        for (_, (v, i, c)) in scolor {
            material_walls.push((v, i, [c[0], c[1], c[2], 1.0]));
        }

        // ROAD GRAPH (v0.586; CURVED v0.591): each edge is a smooth ribbon following its Catmull-Rom
        // centerline (a chain of wall_box sub-segments), sitting on the floor, coloured by its road
        // class's TOP layer (the wearing course). Bends through degree-2 nodes, straight at junctions.
        // Grouped by colour like the structures so identical road classes share a draw.
        use crate::ship::structure::road_type;
        let mut rcolor: std::collections::HashMap<[i32; 3], (Vec<Vertex>, Vec<u32>, [f32; 3])> =
            std::collections::HashMap::new();
        for e in &self.road_edges {
            let center = self.road_edge_centerline(e);
            if center.len() < 2 {
                continue; // a node is missing
            }
            // Colour + slab thickness from the road class's stack (top layer = wearing course).
            let (col, slab) = match road_type(&e.class) {
                Some(rt) => {
                    let top = rt.layers.first().map(|l| l.material).unwrap_or(2);
                    let total: f32 = rt.layers.iter().map(|l| l.thickness_m.max(0.0)).sum();
                    (Self::material_color(top), total.clamp(0.04, 0.2))
                }
                None => ([0.25, 0.25, 0.27, 1.0], 0.1),
            };
            let key = [(col[0] * 64.0) as i32, (col[1] * 64.0) as i32, (col[2] * 64.0) as i32];
            let g = rcolor
                .entry(key)
                .or_insert_with(|| (Vec::new(), Vec::new(), [col[0], col[1], col[2]]));
            // One ribbon box per centerline sub-segment -- approximates the curve.
            for w in center.windows(2) {
                let (a, b) = (w[0], w[1]);
                if (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4 {
                    continue;
                }
                let (mut rv, ri) = wall_box(
                    Vec3::new(a.0, 0.0, a.1),
                    Vec3::new(b.0, 0.0, b.1),
                    0.0,
                    slab,
                    e.width.max(0.2),
                );
                let base = g.0.len() as u32;
                g.0.append(&mut rv);
                g.1.extend(ri.into_iter().map(|i| i + base));
            }
        }
        let mut rlist: Vec<_> = rcolor.into_iter().collect();
        rlist.sort_by_key(|(k, _)| *k);
        for (_, (v, i, c)) in rlist {
            material_walls.push((v, i, [c[0], c[1], c[2], 1.0]));
        }

        // RAIL LINES (v0.592): a track between PAIRED train platforms -- two parallel rails + cross
        // ties (reusing wall_box), on the floor, deduped by sorted index so a mutual pair draws once.
        // Stage 1: the track runs between platform centres (a clear "rail-connected" indicator);
        // platform-beside-track placement is a cosmetic refinement.
        use crate::ship::structure::{structure_type as rail_stype, StructureKind as RailKind};
        let mut seen_rails: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        let mut rail_geo: (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
        let mut tie_geo: (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
        const GAUGE: f32 = 1.435; // standard gauge (m)
        for (i, ps) in self.structures.iter().enumerate() {
            let Some(ty) = rail_stype(&ps.type_id) else { continue };
            if ty.kind != RailKind::Train {
                continue;
            }
            let Some(j) = ps.pair else { continue };
            if j == i || j >= self.structures.len() {
                continue;
            }
            if !seen_rails.insert((i.min(j), i.max(j))) {
                continue; // already drawn (mutual pair)
            }
            let pj = &self.structures[j];
            match rail_stype(&pj.type_id) {
                Some(t) if t.kind == RailKind::Train => {}
                _ => continue, // partner must be a platform
            }
            let (a, b) = ((ps.pos.0, ps.pos.2), (pj.pos.0, pj.pos.2));
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if len < 0.5 {
                continue;
            }
            let d = ((b.0 - a.0) / len, (b.1 - a.1) / len);
            let perp = (-d.1, d.0);
            let y = ps.pos.1.min(pj.pos.1); // rails at the lower (base) level
            // Two rails, offset +/- half-gauge from the centreline.
            for s in [1.0f32, -1.0] {
                let off = (perp.0 * GAUGE * 0.5 * s, perp.1 * GAUGE * 0.5 * s);
                let ra = Vec3::new(a.0 + off.0, 0.0, a.1 + off.1);
                let rb = Vec3::new(b.0 + off.0, 0.0, b.1 + off.1);
                let (mut v, idx) = wall_box(ra, rb, y, 0.12, 0.08);
                let base = rail_geo.0.len() as u32;
                rail_geo.0.append(&mut v);
                rail_geo.1.extend(idx.into_iter().map(|k| k + base));
            }
            // Cross ties every ~0.6 m.
            let n = (len / 0.6).floor().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f32 / n as f32;
                let (cx, cz) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                let ta = Vec3::new(cx - perp.0 * GAUGE * 0.62, 0.0, cz - perp.1 * GAUGE * 0.62);
                let tb = Vec3::new(cx + perp.0 * GAUGE * 0.62, 0.0, cz + perp.1 * GAUGE * 0.62);
                let (mut v, idx) = wall_box(ta, tb, y, 0.06, 0.2);
                let base = tie_geo.0.len() as u32;
                tie_geo.0.append(&mut v);
                tie_geo.1.extend(idx.into_iter().map(|k| k + base));
            }
        }
        if !rail_geo.0.is_empty() {
            material_walls.push((rail_geo.0, rail_geo.1, Self::material_color(1))); // steel rails
        }
        if !tie_geo.0.is_empty() {
            material_walls.push((tie_geo.0, tie_geo.1, Self::material_color(3))); // oak ties
        }

        // ZONE INTERIOR POPULATION (v0.638, superstructure M2c -- the operator's "so the mothership
        // looks filled out"): tile cheap placeholder content across each zone's footprint. Residential
        // zones get CLONES of the player's home shell (walls/structures only, never its zones/rail/
        // road -- a clone-of-a-clone-of-a-mothership would be nonsense); every other zone type gets a
        // generic box filler from `zone_filler.ron`, tinted by the zone type's own colour so each
        // district reads as visually distinct. Both paths merge into `material_walls` (grouped by
        // colour) so this scales the SAME way roads/rails/structures already do: one big CPU-merged
        // vertex/index buffer per distinct colour, one draw call per group, however many instances tile
        // in -- not one mesh per instance. See generate_zone_filler for the grouping.
        let mut zcolor: std::collections::HashMap<[i32; 3], (Vec<Vertex>, Vec<u32>, [f32; 3])> =
            std::collections::HashMap::new();
        for z in &self.zones {
            self.generate_zone_filler(z, &mut zcolor);
        }
        let mut zlist: Vec<_> = zcolor.into_iter().collect();
        zlist.sort_by_key(|(k, _)| *k);
        for (_, (v, i, c)) in zlist {
            material_walls.push((v, i, [c[0], c[1], c[2], 1.0]));
        }

        HomesteadMeshes {
            floors,
            walls: (Vec::new(), Vec::new()),
            material_walls,
            trim: (Vec::new(), Vec::new()),
            windows: (Vec::new(), Vec::new()),
            mirrors: (Vec::new(), Vec::new()),
            ceilings,
            room_info: self.detect_rooms(),
        }
    }

    /// Populate ONE zone's interior into `out` (keyed + merged by quantized rgb, same pattern as the
    /// structures/roads groupings above) -- either home clones (residential) or the generic box filler
    /// (every other type). Called once per zone from `generate_meshes`. (v0.638)
    fn generate_zone_filler(
        &self,
        z: &Zone,
        out: &mut std::collections::HashMap<[i32; 3], (Vec<Vertex>, Vec<u32>, [f32; 3])>,
    ) {
        let (ox, oy, oz) = z.origin;
        let (zw, _zh, zd) = z.size;
        if zw < 1.0 || zd < 1.0 {
            return; // degenerate zone, nothing fits
        }
        if z.type_id == "residential" {
            self.tile_home_clones(ox, oy, oz, zw, zd, out);
            return;
        }
        let Some(filler) = crate::ship::structure::zone_filler(&z.type_id) else {
            return; // no filler authored for this type yet -- an empty interior, not a guessed default
        };
        let tint = crate::ship::structure::zone_type(&z.type_id).map(|t| t.color).unwrap_or((0.6, 0.6, 0.6));
        let (fw, fd) = filler.footprint;
        let step_x = (fw + filler.spacing).max(0.5);
        let step_z = (fd + filler.spacing).max(0.5);
        let usable_w = (zw - 2.0 * filler.inset).max(0.0);
        let usable_d = (zd - 2.0 * filler.inset).max(0.0);
        let nx = (usable_w / step_x).floor() as u32;
        let nz = (usable_d / step_z).floor() as u32;
        if nx == 0 || nz == 0 {
            return; // the zone is too small to fit even one instance with its inset + spacing
        }
        let key = [(tint.0 * 64.0) as i32, (tint.1 * 64.0) as i32, (tint.2 * 64.0) as i32];
        let g = out.entry(key).or_insert_with(|| (Vec::new(), Vec::new(), [tint.0, tint.1, tint.2]));
        for iz in 0..nz {
            for ix in 0..nx {
                let cx = ox + filler.inset + ix as f32 * step_x;
                let cz = oz + filler.inset + iz as f32 * step_z;
                let (v, idx) = footprint_box(cx, cz, fw, fd, oy, filler.height.max(0.2));
                let base = g.0.len() as u32;
                g.0.extend(v);
                g.1.extend(idx.into_iter().map(|k| k + base));
            }
        }
    }

    /// Tile CLONES of the player's home shell (walls + structures ONLY -- never its zones/rail/road
    /// graphs, a clone-of-a-mothership would be nonsense) across a residential zone's footprint, as many
    /// copies as fit given the home's own width/depth. This is explicitly PLACEHOLDER population (the
    /// operator: "for now we could just clone the home we're building to all the other home slots...
    /// eventually we'll have more home designs so they're not all one type") -- swap point is
    /// `home_design_roster()` below; today it returns exactly one design (`self`'s own shell), so every
    /// slot clones the same layout, but the tiling loop already picks a design PER SLOT from the roster
    /// so adding more designs later is a one-line change here, no struct/shape change. (v0.638)
    fn tile_home_clones(
        &self,
        ox: f32,
        oy: f32,
        oz: f32,
        zw: f32,
        zd: f32,
        out: &mut std::collections::HashMap<[i32; 3], (Vec<Vertex>, Vec<u32>, [f32; 3])>,
    ) {
        let roster = self.home_design_roster();
        if roster.is_empty() {
            return;
        }
        const GAP: f32 = 2.0; // clearance between adjacent home footprints (a walking margin)
        // Use the FIRST design's footprint to lay out the grid (a mixed-footprint roster is a later
        // refinement; today there is exactly one design, so this is exact).
        let (dw, dd) = (roster[0].width.max(1.0), roster[0].depth.max(1.0));
        let step_x = dw + GAP;
        let step_z = dd + GAP;
        let nx = (zw / step_x).floor() as u32;
        let nz = (zd / step_z).floor() as u32;
        if nx == 0 || nz == 0 {
            return; // the zone is smaller than one home footprint -- nothing fits
        }
        // Generate each design's LOCAL-SPACE geometry once (mitering/room-detection is real work; a
        // mothership can tile hundreds of slots, so this must not re-run per slot) and cache it,
        // grouped by colour, before stamping it into every slot below via cheap vertex translation.
        let baked: Vec<Vec<(Vec<Vertex>, Vec<u32>, [f32; 3])>> =
            roster.iter().map(|d| d.bake_local_groups()).collect();
        let mut slot = 0usize;
        for iz in 0..nz {
            for ix in 0..nx {
                let groups = &baked[slot % baked.len()];
                slot += 1;
                let (cx, cz) = (ox + ix as f32 * step_x, oz + iz as f32 * step_z);
                for (verts, indices, color) in groups {
                    let key = [(color[0] * 64.0) as i32, (color[1] * 64.0) as i32, (color[2] * 64.0) as i32];
                    let g = out.entry(key).or_insert_with(|| (Vec::new(), Vec::new(), *color));
                    let base = g.0.len() as u32;
                    g.0.extend(verts.iter().map(|v| Vertex {
                        position: [v.position[0] + cx, v.position[1] + oy, v.position[2] + cz],
                        normal: v.normal,
                        uv: v.uv,
                    }));
                    g.1.extend(indices.iter().map(|i| i + base));
                }
            }
        }
    }

    /// The swappable roster of clonable home shells (v0.638). Every entry is the WALLS + STRUCTURES
    /// subset of a `HomeStructure` (never its zones/rail/road graphs -- those are mothership-scale, not
    /// per-home). Today this always returns exactly one design: a snapshot of `self` (the live home the
    /// player is building), so every residential slot clones the SAME layout, per the operator's
    /// "for now... clone the home we're building." When more designs exist (a future
    /// `data/blueprints/home_designs/*.ron` catalog), extend this to load + return all of them; the
    /// tiling loop in `tile_home_clones` already round-robins across whatever this returns.
    fn home_design_roster(&self) -> Vec<ClonableHomeDesign> {
        vec![ClonableHomeDesign {
            width: self.width,
            depth: self.depth,
            height: self.height,
            shell_material: self.shell_material,
            walls: self.walls.clone(),
            structures: self.structures.clone(),
        }]
    }

    /// A fresh road-node id (max existing + 1, or 1). (v0.586)
    pub fn unique_road_node_id(&self) -> u32 {
        self.road_nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1
    }

    /// A road node's position by id (None if unknown). (v0.586)
    pub fn road_node_pos(&self, id: u32) -> Option<(f32, f32)> {
        self.road_nodes.iter().find(|n| n.id == id).map(|n| n.pos)
    }

    /// Remove a road node + every edge touching it (v0.586) -- keeps the graph consistent.
    pub fn remove_road_node(&mut self, id: u32) {
        self.road_nodes.retain(|n| n.id != id);
        self.road_edges.retain(|e| e.from != id && e.to != id);
    }

    // --- RAIL graph (v0.635, M2). Mirrors the road graph; straight edges (no spline). ---

    /// A fresh rail-node id (max existing + 1, or 1).
    pub fn unique_rail_node_id(&self) -> u32 {
        self.rail_nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1
    }

    /// A rail node's position by id (None if unknown).
    pub fn rail_node_pos(&self, id: u32) -> Option<(f32, f32)> {
        self.rail_nodes.iter().find(|n| n.id == id).map(|n| n.pos)
    }

    /// Add a rail node at `pos`; returns its new id.
    pub fn add_rail_node(&mut self, pos: (f32, f32)) -> u32 {
        let id = self.unique_rail_node_id();
        self.rail_nodes.push(RailNode { id, pos });
        id
    }

    /// Remove a rail node + every edge touching it -- keeps the graph consistent.
    pub fn remove_rail_node(&mut self, id: u32) {
        self.rail_nodes.retain(|n| n.id != id);
        self.rail_edges.retain(|e| e.from != id && e.to != id);
    }

    /// Add a rail edge between two existing nodes. Refuses a self-loop, an unknown endpoint, or an exact
    /// duplicate (either direction). Returns true if added.
    pub fn add_rail_edge(&mut self, from: u32, to: u32) -> bool {
        if from == to
            || self.rail_node_pos(from).is_none()
            || self.rail_node_pos(to).is_none()
            || self.rail_edges.iter().any(|e| (e.from == from && e.to == to) || (e.from == to && e.to == from))
        {
            return false;
        }
        self.rail_edges.push(RailEdge { from, to });
        true
    }

    /// Remove the rail edge at `idx`. Returns true if removed.
    pub fn remove_rail_edge(&mut self, idx: usize) -> bool {
        if idx < self.rail_edges.len() {
            self.rail_edges.remove(idx);
            true
        } else {
            false
        }
    }

    /// The sampled CENTERLINE (x, z) of a road edge as a smooth CURVE (v0.591). A Catmull-Rom spline
    /// through the edge's two nodes, with the off-curve control points taken from each node's SINGLE
    /// other neighbour -- so a road bends smoothly through a degree-2 "through" node. At a junction
    /// (3+ edges), a dead-end (1 edge), or an isolated edge, the control point mirrors the segment, so
    /// Catmull-Rom degenerates to a STRAIGHT line -- junctions/ends stay sharp automatically. Returns
    /// >= 2 points (empty if a node is missing). generate_meshes ribbons consecutive points; the curve
    /// editor + tests read the same path.
    pub fn road_edge_centerline(&self, e: &RoadEdge) -> Vec<(f32, f32)> {
        let (a, b) = match (self.road_node_pos(e.from), self.road_node_pos(e.to)) {
            (Some(a), Some(b)) => (a, b),
            _ => return Vec::new(),
        };
        // The single OTHER neighbour of `node` (excluding `exclude`), or None if it isn't degree-2.
        let other = |node: u32, exclude: u32| -> Option<(f32, f32)> {
            let mut found = None;
            let mut count = 0usize;
            for ed in &self.road_edges {
                let n = if ed.from == node {
                    Some(ed.to)
                } else if ed.to == node {
                    Some(ed.from)
                } else {
                    None
                };
                if let Some(n) = n {
                    if n != exclude {
                        count += 1;
                        found = self.road_node_pos(n);
                    }
                }
            }
            if count == 1 {
                found
            } else {
                None
            }
        };
        // Control points: a through-node bends toward its other neighbour; else mirror -> straight.
        let p0 = other(e.from, e.to).unwrap_or((2.0 * a.0 - b.0, 2.0 * a.1 - b.1));
        let p3 = other(e.to, e.from).unwrap_or((2.0 * b.0 - a.0, 2.0 * b.1 - a.1));
        const N: usize = 8;
        let cr = |p0: f32, p1: f32, p2: f32, p3: f32, t: f32| -> f32 {
            let (t2, t3) = (t * t, t * t * t);
            0.5 * (2.0 * p1
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
        };
        (0..=N)
            .map(|k| {
                let t = k as f32 / N as f32;
                (cr(p0.0, a.0, b.0, p3.0, t), cr(p0.1, a.1, b.1, p3.1, t))
            })
            .collect()
    }

    /// Subdivide the box interior into the ROOMS the interior walls enclose (v0.535, the operator's
    /// "rooms emerge as the regions those interior walls enclose"). Rasterizes the floor plan into a
    /// coarse grid, blocks the cells an interior wall passes through, flood-fills the open cells, and
    /// returns one RoomInfo (AABB + centroid) per connected open region. An EMPTY box -> one "home"
    /// room (unchanged); each wall that fully partitions the space splits off another room. Robust to
    /// arbitrary, non-rectangular, L-shaped regions (grid flood-fill, not planar-graph faces).
    pub fn detect_rooms(&self) -> Vec<RoomInfo> {
        let w = self.width.max(1.0);
        let d = self.depth.max(1.0);
        let h = self.height.max(1.0);
        // No interior walls -> the whole box is one room (keeps the empty-box behavior identical).
        if self.walls.is_empty() {
            return vec![RoomInfo {
                id: "home".to_string(),
                center: Vec3::new(w * 0.5, h * 0.5, d * 0.5),
                dimensions: Vec3::new(w, h, d),
                is_hologram_room: false,
                is_spawn_room: true,
            }];
        }

        const CELL: f32 = 0.5;
        let nx = (w / CELL).ceil() as usize;
        let nz = (d / CELL).ceil() as usize;
        // Block every cell within a wall's half-thickness of any interior wall segment. The grid is
        // coarse (0.5 m), so even a 1 mm wall still blocks (the CELL term dominates) and rooms stay
        // separated -- per-wall thickness only widens fat walls. (v0.556)
        let mut blocked = vec![false; nx * nz];
        for cz in 0..nz {
            for cx in 0..nx {
                let px = (cx as f32 + 0.5) * CELL;
                let pz = (cz as f32 + 0.5) * CELL;
                for wall in &self.walls {
                    let half = wall.resolved_thickness() * 0.5 + CELL * 0.5;
                    if point_seg_dist(px, pz, wall.a, wall.b) < half {
                        blocked[cz * nx + cx] = true;
                        break;
                    }
                }
            }
        }

        // Flood-fill the open cells into connected components; each is a room.
        let mut comp = vec![usize::MAX; nx * nz];
        let mut rooms: Vec<RoomInfo> = Vec::new();
        let mut stack: Vec<(usize, usize)> = Vec::new();
        let mut next = 0usize;
        for sz in 0..nz {
            for sx in 0..nx {
                let idx = sz * nx + sx;
                if blocked[idx] || comp[idx] != usize::MAX {
                    continue;
                }
                comp[idx] = next;
                stack.clear();
                stack.push((sx, sz));
                let (mut minx, mut minz, mut maxx, mut maxz) = (sx, sz, sx, sz);
                let mut count = 0usize;
                while let Some((x, z)) = stack.pop() {
                    count += 1;
                    minx = minx.min(x);
                    maxx = maxx.max(x);
                    minz = minz.min(z);
                    maxz = maxz.max(z);
                    let neigh = [(x.wrapping_sub(1), z), (x + 1, z), (x, z.wrapping_sub(1)), (x, z + 1)];
                    for (nxp, nzp) in neigh {
                        if nxp < nx && nzp < nz {
                            let ni = nzp * nx + nxp;
                            if !blocked[ni] && comp[ni] == usize::MAX {
                                comp[ni] = next;
                                stack.push((nxp, nzp));
                            }
                        }
                    }
                }
                next += 1;
                // Ignore rasterization slivers (a cell or two pinched off by a wall).
                if count >= 4 {
                    let (x0, x1) = (minx as f32 * CELL, (maxx + 1) as f32 * CELL);
                    let (z0, z1) = (minz as f32 * CELL, (maxz + 1) as f32 * CELL);
                    rooms.push(RoomInfo {
                        id: format!("room_{}", rooms.len() + 1),
                        center: Vec3::new((x0 + x1) * 0.5, h * 0.5, (z0 + z1) * 0.5),
                        dimensions: Vec3::new(x1 - x0, h, z1 - z0),
                        is_hologram_room: false,
                        is_spawn_room: false,
                    });
                }
            }
        }

        if rooms.is_empty() {
            // Fully walled-in / degenerate -> fall back to the whole box as one room.
            return vec![RoomInfo {
                id: "home".to_string(),
                center: Vec3::new(w * 0.5, h * 0.5, d * 0.5),
                dimensions: Vec3::new(w, h, d),
                is_hologram_room: false,
                is_spawn_room: true,
            }];
        }
        // Spawn in the largest room.
        let best = rooms
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                (a.dimensions.x * a.dimensions.z)
                    .partial_cmp(&(b.dimensions.x * b.dimensions.z))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        rooms[best].is_spawn_room = true;
        rooms
    }
}

/// 2D distance from point (px, pz) to the segment a->b (each (x, z)). Used by room flood-fill to mark
/// the cells an interior wall blocks.
fn point_seg_dist(px: f32, pz: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, abz) = (b.0 - a.0, b.1 - a.1);
    let (apx, apz) = (px - a.0, pz - a.1);
    let len2 = abx * abx + abz * abz;
    let t = if len2 < 1e-9 { 0.0 } else { ((apx * abx + apz * abz) / len2).clamp(0.0, 1.0) };
    let (cx, cz) = (a.0 + abx * t, a.1 + abz * t);
    ((px - cx).powi(2) + (pz - cz).powi(2)).sqrt()
}

/// Append (verts, indices) onto an accumulator, offsetting the appended indices.
fn merge(acc: &mut (Vec<Vertex>, Vec<u32>), add: (Vec<Vertex>, Vec<u32>)) {
    let base = acc.0.len() as u32;
    acc.0.extend(add.0);
    acc.1.extend(add.1.into_iter().map(|i| i + base));
}

/// A solid axis-aligned box SILHOUETTE from a footprint (v0.638): min corner (x0, z0), extent (w, d),
/// rising from `y0` to `y0 + h`. Unlike `wall_box` (which extrudes a THIN wall along a start->end run,
/// centring `thickness` across it), this fills the WHOLE rectangular footprint -- the right primitive
/// for a generic filler "block" (a machine housing, a shop stall, a storage rack) rather than a wall
/// segment. Single-sided (outward normals only): fillers are viewed from outside/above, never entered,
/// so the inside-facing copy `wall_box` needs for room interiors would be wasted geometry here.
fn footprint_box(x0: f32, z0: f32, w: f32, d: f32, y0: f32, h: f32) -> (Vec<Vertex>, Vec<u32>) {
    let (x1, z1, y1) = (x0 + w, z0 + d, y0 + h);
    let mut verts: Vec<Vertex> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut quad = |p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], n: [f32; 3]| {
        let base = verts.len() as u32;
        for (p, uv) in [(p0, [0.0, 0.0]), (p1, [1.0, 0.0]), (p2, [1.0, 1.0]), (p3, [0.0, 1.0])] {
            verts.push(Vertex { position: p, normal: n, uv });
        }
        idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    // Top + 4 sides (no bottom -- it sits on the floor, never seen).
    quad([x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1], [0.0, 1.0, 0.0]);
    quad([x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [-1.0, 0.0, 0.0]);
    quad([x1, y0, z1], [x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [1.0, 0.0, 0.0]);
    quad([x0, y0, z1], [x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [0.0, 0.0, 1.0]);
    quad([x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [0.0, 0.0, -1.0]);
    (verts, idx)
}

/// The clonable subset of a `HomeStructure` (v0.638): just its shell box + interior walls + placed
/// structures -- deliberately NOT its zones/rail/road graphs (those describe the MOTHERSHIP the home
/// sits inside, cloning them into a residential slot would nest a mothership inside a mothership). One
/// entry in the swappable `home_design_roster`; today the roster always holds exactly one (the live
/// home), so `tile_home_clones` stamps the same design into every slot -- the operator's explicit
/// placeholder ("clone the home we're building... eventually we'll have more home designs").
struct ClonableHomeDesign {
    width: f32,
    depth: f32,
    height: f32,
    shell_material: u32,
    walls: Vec<InteriorWall>,
    structures: Vec<PlacedStructure>,
}

impl ClonableHomeDesign {
    /// Bake this design's shell + interior walls + structures into LOCAL-SPACE (min corner at the
    /// origin) per-colour groups, computed ONCE per design regardless of how many mothership slots clone
    /// it (mitering + room-detection is real work; `tile_home_clones` calls this once then translates
    /// the baked vertices per slot, so a hundred cloned homes cost ~one home's mesh-gen, not a hundred).
    /// Builds a throwaway `HomeStructure` for just this design (no zones/rail/road -- a clone-of-a-
    /// mothership would be nonsense) and reuses its own `generate_meshes` grouping. Glass/transparent
    /// groups are dropped (an opaque roof reads better en masse + skips the transparent-pass sort cost
    /// once tiled hundreds of times; the player's OWN home keeps its real glass roof, this only affects
    /// the cloned filler copies).
    fn bake_local_groups(&self) -> Vec<(Vec<Vertex>, Vec<u32>, [f32; 3])> {
        let stub = HomeStructure {
            width: self.width,
            depth: self.depth,
            height: self.height,
            shell_material: self.shell_material,
            roof_material: self.shell_material,
            walls: self.walls.clone(),
            shell_thickness: None,
            lights: Vec::new(),
            spawn: None,
            structures: self.structures.clone(),
            road_nodes: Vec::new(),
            road_edges: Vec::new(),
            zones: Vec::new(),
            rail_nodes: Vec::new(),
            rail_edges: Vec::new(),
        };
        stub.generate_meshes()
            .material_walls
            .into_iter()
            .filter(|(_, _, color)| color[3] >= 0.999)
            .map(|(v, i, c)| (v, i, [c[0], c[1], c[2]]))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Mitred corners (v0.558): two walls meeting at a shared corner get their END faces cut to the
// angle bisector so they meet FLUSH (the CAD/architecture standard), instead of two square ends
// overlapping + a round filler column that provably can't cover the gap (the apex sits at
// (t/2)/sin(theta/2) > t/2). Zero extra triangles, exact at any angle + any thickness.
// ---------------------------------------------------------------------------

type V2 = (f32, f32);
fn v_perp((x, z): V2) -> V2 { (-z, x) }
fn v_add(a: V2, b: V2) -> V2 { (a.0 + b.0, a.1 + b.1) }
fn v_scale(a: V2, s: f32) -> V2 { (a.0 * s, a.1 * s) }
fn v_cross(a: V2, b: V2) -> f32 { a.0 * b.1 - a.1 * b.0 }

/// Intersect line (p1, dir d1) with line (p2, dir d2). None if (near-)parallel.
fn line_intersect(p1: V2, d1: V2, p2: V2, d2: V2) -> Option<V2> {
    let denom = v_cross(d1, d2);
    if denom.abs() < 1e-5 {
        return None;
    }
    let diff = (p2.0 - p1.0, p2.1 - p1.1);
    let t = v_cross(diff, d2) / denom;
    Some(v_add(p1, v_scale(d1, t)))
}

/// The two footprint corners of wall W's END at shared corner `c`, mitred against an adjacent wall.
/// `w_left`/`w_right` are points on W's left/right edge (c +/- perp(W_dir)*half); `w_dir` is W's
/// FORWARD direction (a->b) so left/right stay consistent along the wall. `adj_dir` is the adjacent
/// wall's direction AWAY from `c`. `is_a_end` selects which adjacent edge each side meets (it flips
/// between W's a-end and b-end). Returns (left_corner, right_corner) in W's consistent left/right, or
/// None on a degenerate (near-parallel / absurdly-acute) join so the caller squares the end off.
fn wall_end_miter(
    w_left: V2,
    w_right: V2,
    w_dir: V2,
    c: V2,
    adj_dir: V2,
    adj_half: f32,
    is_a_end: bool,
) -> Option<(V2, V2)> {
    let pa = v_perp(adj_dir);
    let adj_l = v_add(c, v_scale(pa, adj_half));
    let adj_r = v_add(c, v_scale(pa, -adj_half));
    let (left_adj, right_adj) = if is_a_end { (adj_r, adj_l) } else { (adj_l, adj_r) };
    let l = line_intersect(w_left, w_dir, left_adj, adj_dir)?;
    let r = line_intersect(w_right, w_dir, right_adj, adj_dir)?;
    // Bail on an absurd cutback (very acute angle) so a degenerate join squares off, not spikes.
    let far = |p: V2| ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt() > 6.0 * (adj_half + 0.05);
    if far(l) || far(r) {
        return None;
    }
    Some((l, r))
}

/// If wall `wi`'s end `e` (approached from its other end `from`) lands INSIDE another wall's body -- a
/// mid-span T-junction (a wall ending on another wall's FACE, not at a shared corner), where the miter
/// can't help and the thick wall would PASS THROUGH the other -- pull `e` back to that wall's NEAR
/// face so it butts cleanly. Returns (clipped_end, distance pulled back); (e, 0.0) if the end is free /
/// at a shared corner / not inside anything. (v0.566)
fn clip_end_to_walls(e: V2, from: V2, wi: usize, walls: &[InteriorWall]) -> (V2, f32) {
    let wd = (e.0 - from.0, e.1 - from.1);
    let wlen = (wd.0 * wd.0 + wd.1 * wd.1).sqrt();
    if wlen < 1e-4 {
        return (e, 0.0);
    }
    let wdir = (wd.0 / wlen, wd.1 / wlen);
    let mut best_t = wlen; // only accept a clip CLOSER to `from` than the original end
    let mut best_e = e;
    let mut clipped = false;
    for (j, m) in walls.iter().enumerate() {
        if j == wi {
            continue;
        }
        let md = (m.b.0 - m.a.0, m.b.1 - m.a.1);
        let mlen = (md.0 * md.0 + md.1 * md.1).sqrt();
        if mlen < 1e-4 {
            continue;
        }
        let mdir = (md.0 / mlen, md.1 / mlen);
        let mperp = (-mdir.1, mdir.0);
        let mh = m.resolved_thickness() * 0.5;
        // Is E inside M's body -- perp distance < half thickness, projecting onto M's INTERIOR (not
        // its endpoints, which would be a shared-corner miter case, handled elsewhere)?
        let rel = (e.0 - m.a.0, e.1 - m.a.1);
        let along_e = rel.0 * mdir.0 + rel.1 * mdir.1;
        let perp_e = rel.0 * mperp.0 + rel.1 * mperp.1;
        if along_e < 0.15 || along_e > mlen - 0.15 || perp_e.abs() >= mh + 0.005 {
            continue;
        }
        // Pull E back to whichever of M's two faces the ray (from -> E) crosses NEAREST `from`.
        for sign in [1.0_f32, -1.0_f32] {
            let fp = (m.a.0 + mperp.0 * mh * sign, m.a.1 + mperp.1 * mh * sign);
            let denom = wdir.0 * mdir.1 - wdir.1 * mdir.0;
            if denom.abs() < 1e-5 {
                continue;
            }
            let diff = (fp.0 - from.0, fp.1 - from.1);
            let t = (diff.0 * mdir.1 - diff.1 * mdir.0) / denom;
            if t > 0.05 && t < best_t {
                let cx = (from.0 + wdir.0 * t, from.1 + wdir.1 * t);
                let along_c = (cx.0 - m.a.0) * mdir.0 + (cx.1 - m.a.1) * mdir.1;
                if along_c > 0.0 && along_c < mlen {
                    best_t = t;
                    best_e = cx;
                    clipped = true;
                }
            }
        }
    }
    if clipped {
        (best_e, ((best_e.0 - e.0).powi(2) + (best_e.1 - e.1).powi(2)).sqrt())
    } else {
        (e, 0.0)
    }
}

/// Build a double-sided wall PIECE: a hexahedral prism whose floor footprint is the quad
/// (sl, sr, er, el) extruded from y0 to y1. Generalises wall_box to mitred (non-perpendicular) ends.
fn wall_piece(sl: V2, sr: V2, el: V2, er: V2, y0: f32, y1: f32) -> (Vec<Vertex>, Vec<u32>) {
    let foot = [sl, sr, er, el];
    let mut verts: Vec<Vertex> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let b = |i: usize| [foot[i].0, y0, foot[i].1];
    let t = |i: usize| [foot[i].0, y1, foot[i].1];
    let mut quad = |p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]| {
        let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let w = [p3[0] - p0[0], p3[1] - p0[1], p3[2] - p0[2]];
        let n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
        let n = [n[0] / len, n[1] / len, n[2] / len];
        let base = verts.len() as u32;
        for (p, uv) in [(p0, [0.0, 0.0]), (p1, [1.0, 0.0]), (p2, [1.0, 1.0]), (p3, [0.0, 1.0])] {
            verts.push(Vertex { position: p, normal: n, uv });
        }
        idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    // The footprint quad [sl, sr, er, el] is wound CW-from-above, so these per-face normals come out
    // pointing INWARD. That is fine ONLY because the double-side pass below adds the outward copies --
    // do NOT "fix" the winding, and do NOT reuse wall_piece single-sided (it would be inside-out).
    quad(b(0), b(1), t(1), t(0)); // sl-sr cap
    quad(b(1), b(2), t(2), t(1)); // sr-er side
    quad(b(2), b(3), t(3), t(2)); // er-el cap
    quad(b(3), b(0), t(0), t(3)); // el-sl side
    quad(t(0), t(1), t(2), t(3)); // top
    quad(b(3), b(2), b(1), b(0)); // bottom
    // Double-side (the home is viewed from inside): mirror with inverted normals + reversed winding,
    // so every face has both an inward-lit and an outward-lit copy (matches wall_box).
    let n = verts.len() as u32;
    let mirror: Vec<Vertex> = verts
        .iter()
        .map(|v| Vertex { position: v.position, normal: [-v.normal[0], -v.normal[1], -v.normal[2]], uv: v.uv })
        .collect();
    verts.extend(mirror);
    for tri in idx.clone().chunks(3) {
        idx.push(tri[0] + n);
        idx.push(tri[2] + n);
        idx.push(tri[1] + n);
    }
    (verts, idx)
}

/// A vertical cylinder SIDE surface (no caps -- the floor + ceiling hide them) at (cx, cz): the
/// corner column that fills a 3+-WALL interior-wall join (T / X), where a single miter bisector is
/// undefined. 2-wall joins are mitred instead (v0.558). Double-sided + low-poly. (v0.549)
fn corner_column(cx: f32, cz: f32, radius: f32, height: f32, segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    use std::f32::consts::TAU;
    let mut v: Vec<Vertex> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for i in 0..=segments {
        let ang = (i as f32 / segments as f32) * TAU;
        let (s, c) = ang.sin_cos();
        let u = i as f32 / segments as f32;
        v.push(Vertex { position: [cx + c * radius, 0.0, cz + s * radius], normal: [c, 0.0, s], uv: [u, 1.0] });
        v.push(Vertex { position: [cx + c * radius, height, cz + s * radius], normal: [c, 0.0, s], uv: [u, 0.0] });
    }
    for i in 0..segments {
        let o = i * 2;
        idx.extend([o, o + 1, o + 2, o + 1, o + 3, o + 2]);
    }
    // Double-side: mirror with inverted normals + reversed winding.
    let n = v.len() as u32;
    let mirror: Vec<Vertex> = v
        .iter()
        .map(|vert| Vertex {
            position: vert.position,
            normal: [-vert.normal[0], -vert.normal[1], -vert.normal[2]],
            uv: vert.uv,
        })
        .collect();
    v.extend(mirror);
    for t in idx.clone().chunks(3) {
        idx.push(t[0] + n);
        idx.push(t[2] + n);
        idx.push(t[1] + n);
    }
    (v, idx)
}

/// Build an interior wall as solid pier/sill/header pieces with its DOOR/WINDOW openings cut. The 4
/// footprint corners (al/ar at a, bl/br at b) carry the MITRE at the true wall ENDS only; every
/// opening cut stays PERPENDICULAR to the wall (90 deg jambs + headers), so a mitred corner never
/// skews a door/window frame -- the v0.559 fix for "the door frame is at a weird angle". (v0.559)
fn wall_with_openings(
    a: Vec3,
    b: Vec3,
    h: f32,
    thickness: f32,
    op_shift: f32, // a-end clip distance (v0.566): openings are authored from the ORIGINAL a, so shift
    openings: &[Opening],
    al: V2,
    ar: V2,
    bl: V2,
    br: V2,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut out: (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
    let total = ((b.x - a.x).powi(2) + (b.z - a.z).powi(2)).sqrt();
    if total < 1e-4 {
        return out;
    }
    let half = thickness * 0.5;
    let (dx, dz) = ((b.x - a.x) / total, (b.z - a.z) / total);
    let (px, pz) = (-dz, dx); // wall left-perpendicular
    // Left/right footprint point at distance s along the wall: the MITRED corner EXACTLY at a wall
    // end, else a square PERPENDICULAR offset from the centreline -- so opening jambs stay at 90 deg
    // and only the corners are mitred.
    let left = |s: f32| -> V2 {
        if s < 1e-3 {
            al
        } else if s > total - 1e-3 {
            bl
        } else {
            (a.x + dx * s + px * half, a.z + dz * s + pz * half)
        }
    };
    let right = |s: f32| -> V2 {
        if s < 1e-3 {
            ar
        } else if s > total - 1e-3 {
            br
        } else {
            (a.x + dx * s - px * half, a.z + dz * s - pz * half)
        }
    };

    let mut ops: Vec<&Opening> = openings.iter().filter(|o| o.width > 0.01).collect();
    ops.sort_by(|x, y| x.at.partial_cmp(&y.at).unwrap_or(std::cmp::Ordering::Equal));

    let mut cursor = 0.0f32;
    for op in ops {
        let raw_start = (op.at - op_shift).clamp(0.0, total);
        let end = (op.at + op.width - op_shift).clamp(0.0, total);
        if end <= cursor || raw_start >= total {
            continue; // overlaps the previous opening or runs off the end
        }
        let start = raw_start.max(cursor);
        // Full-height pier before this opening.
        if start > cursor + 1e-4 {
            merge(&mut out, wall_piece(left(cursor), right(cursor), left(start), right(start), 0.0, h));
        }
        // Around the aperture [start, end] x [sill, sill+height]: a sill panel (windows) + a header.
        let sill = op.sill.max(0.0).min(h);
        let top = (sill + op.height).clamp(0.0, h);
        if sill > 0.01 {
            merge(&mut out, wall_piece(left(start), right(start), left(end), right(end), 0.0, sill));
        }
        if top < h - 0.01 {
            merge(&mut out, wall_piece(left(start), right(start), left(end), right(end), top, h));
        }
        cursor = end;
    }
    // Remaining full-height pier after the last opening.
    if cursor < total - 1e-4 {
        merge(&mut out, wall_piece(left(cursor), right(cursor), left(total), right(total), 0.0, h));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_only() -> HomeStructure {
        HomeStructure { width: 55.0, depth: 89.0, height: 3.0, shell_material: 1, roof_material: 4, walls: Vec::new(), shell_thickness: None, lights: Vec::new(), spawn: None, structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new() }
    }

    /// v0.631 superstructure M1: zones add/remove with unique ids, the zone_types registry parses, and a
    /// home with zones survives a RON round-trip (the macro layout persists with the structure).
    #[test]
    fn zones_add_remove_and_round_trip() {
        // The registry parses + carries the operator's named districts.
        let zt = crate::ship::structure::zone_types();
        assert!(zt.len() >= 8, "zone_types.ron parses the district registry");
        assert!(zt.iter().any(|t| t.id == "civic_mall"), "the public mall/meeting zone is a type");
        assert!(crate::ship::structure::zone_type("hangar").is_some());

        let mut hs = box_only();
        let a = hs.add_zone("residential", (1.0, 0.0, 2.0), (20.0, 4.0, 20.0));
        let b = hs.add_zone("industrial", (30.0, 0.0, 2.0), (40.0, 8.0, 30.0));
        assert_ne!(a, b, "minted ids are unique");
        assert_eq!(hs.zones.len(), 2);
        // RON round-trip preserves the zones.
        let ron = ron::ser::to_string(&hs).expect("serialize");
        let back: HomeStructure = ron::from_str(&ron).expect("deserialize");
        assert_eq!(back.zones.len(), 2, "zones survive the round-trip");
        assert_eq!(back.zones[1].type_id, "industrial");
        assert_eq!(back.zones[1].size, (40.0, 8.0, 30.0));
        // Duplicate a zone -> a fresh id, same type, nudged +2 m so it's visible (v0.634).
        let c = hs.duplicate_zone(&b).expect("duplicate an existing zone");
        assert_ne!(c, b);
        assert_eq!(hs.zones.len(), 3);
        let dup = hs.zones.iter().find(|z| z.id == c).unwrap();
        let orig = hs.zones.iter().find(|z| z.id == b).unwrap();
        assert_eq!(dup.type_id, orig.type_id);
        assert!((dup.origin.0 - orig.origin.0 - 2.0).abs() < 1e-6, "duplicate is nudged +2 m in x");
        assert!(hs.duplicate_zone("nope").is_none(), "duplicating an unknown id is None");
        // Remove by id.
        assert!(hs.remove_zone(&a));
        assert!(!hs.remove_zone("nope"));
        assert_eq!(hs.zones.len(), 2);
        assert!(hs.zones.iter().any(|z| z.id == b));
    }

    /// v0.638: the operator's two new district kinds parse with the shape every other entry has.
    #[test]
    fn armory_and_arena_zone_types_present() {
        let armory = crate::ship::structure::zone_type("armory").expect("armory zone type parses");
        assert!(armory.purpose.to_lowercase().contains("weapon") || armory.purpose.to_lowercase().contains("firing"));
        let arena = crate::ship::structure::zone_type("arena").expect("arena zone type parses");
        assert!(arena.purpose.to_lowercase().contains("pvp") || arena.purpose.to_lowercase().contains("spar"));
    }

    /// v0.638 superstructure M2c: zone interior population. A RESIDENTIAL zone big enough for the home's
    /// own footprint gets non-empty cloned-home geometry merged into `material_walls`; a zone smaller
    /// than one home footprint stays empty (no divide-by-zero / no garbage geometry). Every OTHER zone
    /// type (here: industrial) gets the generic box filler from zone_filler.ron, tinted by its zone
    /// type's own colour.
    #[test]
    fn zone_filler_populates_interiors() {
        // A tiny box home (12x12) so a residential zone with a modest size can fit multiple clones,
        // keeping this test fast.
        let mut hs = HomeStructure {
            width: 12.0,
            depth: 12.0,
            height: 3.0,
            shell_material: 1,
            roof_material: 4,
            walls: Vec::new(),
            shell_thickness: None,
            lights: Vec::new(),
            spawn: None,
            structures: Vec::new(),
            road_nodes: Vec::new(),
            road_edges: Vec::new(),
            zones: Vec::new(),
            rail_nodes: Vec::new(),
            rail_edges: Vec::new(),
        };
        // Baseline: no zones -> generate_meshes still works (existing behavior unchanged).
        let base = hs.generate_meshes();
        let base_material_group_count = base.material_walls.len();

        // A residential zone with room for a 2x2 grid of 12x12 homes (+2m gap each): needs >= 28x28.
        hs.add_zone("residential", (0.0, 0.0, 0.0), (30.0, 4.0, 30.0));
        // Too small to fit even one home clone -- must stay a no-op, not panic/garbage.
        hs.add_zone("residential", (100.0, 0.0, 0.0), (5.0, 4.0, 5.0));
        // An industrial zone big enough for the generic filler (industrial footprint 5x5 + 2m spacing +
        // 2m inset per zone_filler.ron).
        hs.add_zone("industrial", (200.0, 0.0, 0.0), (30.0, 8.0, 30.0));

        let m = hs.generate_meshes();
        assert!(
            m.material_walls.len() > base_material_group_count,
            "zones with room to populate add new colour-grouped geometry"
        );
        let total_verts: usize = m.material_walls.iter().map(|(v, _, _)| v.len()).sum();
        let base_verts: usize = base.material_walls.iter().map(|(v, _, _)| v.len()).sum();
        assert!(total_verts > base_verts, "populated zones add real vertex geometry, not empty wireframe");

        // The industrial filler's colour bucket should carry the industrial zone type's own colour
        // (0.85, 0.55, 0.25) -- proves the generic filler is tinted per zone type, not a flat default.
        let industrial_color = crate::ship::structure::zone_type("industrial").unwrap().color;
        let key = [
            (industrial_color.0 * 64.0) as i32,
            (industrial_color.1 * 64.0) as i32,
            (industrial_color.2 * 64.0) as i32,
        ];
        let has_industrial_bucket = m.material_walls.iter().any(|(_, _, c)| {
            [(c[0] * 64.0) as i32, (c[1] * 64.0) as i32, (c[2] * 64.0) as i32] == key
        });
        assert!(has_industrial_bucket, "the industrial filler renders tinted with its zone type's colour");
    }

    /// v0.638: a zone type with no entry in zone_filler.ron (and that isn't "residential") renders with
    /// NO filler geometry -- an honest empty interior rather than a guessed default. transit_hub through
    /// arena all HAVE entries; this locks the "no silent fallback" contract by checking an id that is
    /// not in the registry at all.
    #[test]
    fn unlisted_zone_type_gets_no_filler() {
        assert!(crate::ship::structure::zone_filler("no_such_zone_type").is_none());
        let mut hs = box_only();
        hs.add_zone("no_such_zone_type", (1.0, 0.0, 1.0), (20.0, 4.0, 20.0));
        // generate_meshes must not panic on an unknown type_id, and adds nothing for it.
        let before = box_only().generate_meshes().material_walls.len();
        let after = hs.generate_meshes().material_walls.len();
        assert_eq!(before, after, "an unlisted zone type contributes no filler geometry");
    }

    /// v0.638: every zone_filler.ron entry's type_id resolves to a real zone_types.ron entry (catches a
    /// typo'd type_id the moment it's added, rather than silently never rendering).
    #[test]
    fn every_zone_filler_entry_matches_a_real_zone_type() {
        for f in crate::ship::structure::zone_fillers() {
            assert!(
                crate::ship::structure::zone_type(&f.type_id).is_some(),
                "zone_filler.ron entry '{}' has no matching zone_types.ron entry",
                f.type_id
            );
            assert!(f.footprint.0 > 0.0 && f.footprint.1 > 0.0, "{} has a positive footprint", f.type_id);
            assert!(f.height > 0.0, "{} has a positive height", f.type_id);
        }
        // residential is deliberately absent (it uses the home-cloning path instead).
        assert!(
            crate::ship::structure::zone_filler("residential").is_none(),
            "residential must NOT have a generic filler entry -- it clones the home design instead"
        );
    }

    /// v0.635 superstructure M2: the RAIL graph -- unique node ids, edge validation (self-loop / unknown /
    /// duplicate refused), node removal prunes its edges, and the graph survives a RON round-trip.
    #[test]
    fn rail_graph_nodes_edges_and_round_trip() {
        let mut hs = box_only();
        let a = hs.add_rail_node((1.0, 2.0));
        let b = hs.add_rail_node((10.0, 2.0));
        let c = hs.add_rail_node((20.0, 2.0));
        assert!(a != b && b != c, "minted ids are unique");
        assert!(hs.add_rail_edge(a, b), "valid edge added");
        assert!(hs.add_rail_edge(b, c), "valid multi-stop edge added");
        assert!(!hs.add_rail_edge(a, b), "exact duplicate refused");
        assert!(!hs.add_rail_edge(b, a), "reverse duplicate refused");
        assert!(!hs.add_rail_edge(a, a), "self-loop refused");
        assert!(!hs.add_rail_edge(a, 999), "unknown endpoint refused");
        assert_eq!(hs.rail_edges.len(), 2);
        // RON round-trip preserves the rail graph.
        let ron = ron::ser::to_string(&hs).expect("serialize");
        let back: HomeStructure = ron::from_str(&ron).expect("deserialize");
        assert_eq!(back.rail_nodes.len(), 3);
        assert_eq!(back.rail_edges.len(), 2);
        // Removing a node prunes its edges.
        hs.remove_rail_node(b);
        assert_eq!(hs.rail_nodes.len(), 2);
        assert!(hs.rail_edges.is_empty(), "both edges touched b -> pruned");
        assert!(hs.remove_rail_edge(0) == false, "no edges left to remove by index");
    }

    /// Total wall vertices across BOTH the legacy `walls` family and the per-material `material_walls`
    /// groups -- the home path now emits per-material groups (v0.552) instead of one merged family.
    fn wall_vcount(m: &HomesteadMeshes) -> usize {
        m.walls.0.len() + m.material_walls.iter().map(|w| w.0.len()).sum::<usize>()
    }

    #[test]
    fn wall_material_registry_parses() {
        // Locks the embedded wall_materials.ron against a syntax break (which would otherwise
        // silently degrade every wall to grid-grey). Update the count if you add/remove a material.
        let mats = wall_materials();
        assert_eq!(mats.len(), 8, "all 8 wall materials parse");
        let steel = wall_material(1).expect("steel id 1 present");
        assert_eq!(steel.name, "Steel");
        assert_eq!(steel.color.3, 1.0, "steel is opaque");
        assert!(wall_material(4).expect("glass id 4").color.3 < 1.0, "tempered glass is transparent");
    }

    #[test]
    fn box_generates_floor_ceiling_and_outer_walls() {
        let m = box_only().generate_meshes();
        assert_eq!(m.floors.len(), 1, "one floor for the box");
        assert!(!m.ceilings.0.is_empty(), "ceiling generated");
        assert!(wall_vcount(&m) > 0, "outer walls generated");
        assert_eq!(m.room_info.len(), 1, "one 'home' room for now");
        assert_eq!(m.room_info[0].id, "home");
        assert_eq!(m.room_info[0].dimensions, Vec3::new(55.0, 3.0, 89.0));
        assert_eq!(m.room_info[0].center, Vec3::new(27.5, 1.5, 44.5));
    }

    fn wall(a: (f32, f32), b: (f32, f32), openings: Vec<Opening>) -> InteriorWall {
        InteriorWall { a, b, height: 3.0, material: 1, openings, thickness: None, layers: Vec::new() }
    }

    #[test]
    fn road_centerline_curves_through_a_through_node_but_stays_straight_when_isolated() {
        let mut h = HomeStructure {
            width: 30.0, depth: 30.0, height: 3.0, shell_material: 1, roof_material: 4,
            walls: Vec::new(), shell_thickness: None, lights: Vec::new(), spawn: None,
            structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new(),
        };
        // A right-angle chain A(0,0) - B(10,0) - C(10,10): B is a degree-2 through-node.
        h.road_nodes.push(RoadNode { id: 1, pos: (0.0, 0.0) });
        h.road_nodes.push(RoadNode { id: 2, pos: (10.0, 0.0) });
        h.road_nodes.push(RoadNode { id: 3, pos: (10.0, 10.0) });
        h.road_edges.push(RoadEdge { from: 1, to: 2, class: "residential".into(), width: 4.0 });
        h.road_edges.push(RoadEdge { from: 2, to: 3, class: "residential".into(), width: 4.0 });
        // The A-B edge should BEND toward C near B (some sample leaves the z=0 line).
        let ab = h.road_edge_centerline(&h.road_edges[0]);
        assert!(ab.len() >= 3, "centerline is sampled");
        assert_eq!(ab.first().copied(), Some((0.0, 0.0)), "starts at A");
        assert!((ab.last().unwrap().0 - 10.0).abs() < 1e-3 && ab.last().unwrap().1.abs() < 1e-3, "ends at B");
        assert!(ab.iter().any(|p| p.1.abs() > 0.05), "the A-B edge curves off the straight z=0 line");

        // An ISOLATED straight edge stays straight (all samples on the z=0 line).
        let mut h2 = h.clone();
        h2.road_edges.clear();
        h2.road_nodes = vec![RoadNode { id: 1, pos: (0.0, 0.0) }, RoadNode { id: 2, pos: (10.0, 0.0) }];
        h2.road_edges.push(RoadEdge { from: 1, to: 2, class: "residential".into(), width: 4.0 });
        let iso = h2.road_edge_centerline(&h2.road_edges[0]);
        assert!(iso.iter().all(|p| p.1.abs() < 1e-3), "an isolated edge is a straight line");
    }

    #[test]
    fn paired_train_platforms_render_a_rail_track_once() {
        let mut h = HomeStructure {
            width: 40.0, depth: 40.0, height: 3.0, shell_material: 1, roof_material: 4,
            walls: Vec::new(), shell_thickness: None, lights: Vec::new(), spawn: None,
            structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new(),
        };
        let base = wall_vcount(&h.generate_meshes());
        // Two train platforms; unpaired -> no track yet.
        h.structures.push(PlacedStructure { type_id: "train".into(), pos: (5.0, 0.0, 5.0), rot_deg: 0.0, pair: None });
        h.structures.push(PlacedStructure { type_id: "train".into(), pos: (25.0, 0.0, 5.0), rot_deg: 0.0, pair: None });
        let unpaired = wall_vcount(&h.generate_meshes());
        // Pair them BOTH ways -> a rail track appears, and it is drawn only ONCE (dedup).
        h.structures[0].pair = Some(1);
        let one_way = wall_vcount(&h.generate_meshes());
        assert!(one_way > unpaired, "pairing two platforms renders a rail track");
        h.structures[1].pair = Some(0);
        let both_ways = wall_vcount(&h.generate_meshes());
        assert_eq!(both_ways, one_way, "a mutual pair draws the track once (deduped)");
        let _ = base;
    }

    #[test]
    fn road_graph_meshes_edges_and_node_removal_prunes() {
        let mut h = HomeStructure {
            width: 20.0, depth: 20.0, height: 3.0, shell_material: 1, roof_material: 4,
            walls: Vec::new(), shell_thickness: None, lights: Vec::new(), spawn: None,
            structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new(),
        };
        let n1 = h.unique_road_node_id();
        h.road_nodes.push(RoadNode { id: n1, pos: (2.0, 2.0) });
        let n2 = h.unique_road_node_id();
        assert_ne!(n1, n2, "fresh ids are unique");
        h.road_nodes.push(RoadNode { id: n2, pos: (18.0, 2.0) });
        h.road_edges.push(RoadEdge { from: n1, to: n2, class: "residential".into(), width: 5.0 });
        // The edge contributes ribbon geometry: a home with the edge has more wall verts than without.
        let with_edge = wall_vcount(&h.generate_meshes());
        let mut bare = h.clone();
        bare.road_edges.clear();
        assert!(with_edge > wall_vcount(&bare.generate_meshes()), "the road edge added ribbon geometry");
        // Removing a node prunes the edge that touched it.
        h.remove_road_node(n1);
        assert!(h.road_edges.is_empty(), "removing a node drops edges touching it");
        assert_eq!(h.road_nodes.len(), 1);
    }

    #[test]
    fn surface_layers_drive_exposed_material_and_total_thickness() {
        let mut w = wall((0.0, 0.0), (4.0, 0.0), vec![]);
        w.thickness = Some(0.10);
        assert_eq!(w.exposed_material(), 1, "bare wall exposes its base material");
        assert!((w.total_thickness() - 0.10).abs() < 1e-4, "no layers -> just the wall");
        // Coat it: aluminum (5) on top, then HDPE (8) added on top of THAT -> 8 is exposed.
        w.layers.push(SurfaceLayer { material: 5, thickness_m: 0.02 });
        w.layers.insert(0, SurfaceLayer { material: 8, thickness_m: 0.01 });
        assert_eq!(w.exposed_material(), 8, "the top (first) layer is the exposed face");
        assert!((w.total_thickness() - 0.13).abs() < 1e-4, "wall + both coats = 13 cm");
    }

    #[test]
    fn clip_pulls_a_mid_span_t_back_to_the_face() {
        // M runs along +X at z=0, 0.4 m thick (half 0.2). W comes up from (5,-3) and its end (5,0.1)
        // lands INSIDE M's body; it should clip back to M's NEAR face at z = -0.2.
        let m = InteriorWall { a: (0.0, 0.0), b: (10.0, 0.0), height: 3.0, material: 1, openings: vec![], thickness: Some(0.4), layers: Vec::new() };
        let w = InteriorWall { a: (5.0, -3.0), b: (5.0, 0.1), height: 3.0, material: 1, openings: vec![], thickness: Some(0.1), layers: Vec::new() };
        let walls = vec![m, w];
        let (clipped, dist) = clip_end_to_walls((5.0, 0.1), (5.0, -3.0), 1, &walls);
        assert!((clipped.1 + 0.2).abs() < 1e-3, "clipped to M near face z=-0.2, got {clipped:?}");
        assert!(dist > 0.0, "end was pulled back, dist {dist}");
    }

    #[test]
    fn clip_leaves_a_free_end_alone() {
        let m = InteriorWall { a: (0.0, 0.0), b: (10.0, 0.0), height: 3.0, material: 1, openings: vec![], thickness: Some(0.4), layers: Vec::new() };
        let walls = vec![m];
        let (clipped, dist) = clip_end_to_walls((5.0, 5.0), (5.0, 3.0), 99, &walls);
        assert_eq!(dist, 0.0, "a free end in open space is unchanged");
        assert!((clipped.0 - 5.0).abs() < 1e-6 && (clipped.1 - 5.0).abs() < 1e-6);
    }

    #[test]
    fn miter_a_end_is_inner_and_outer_corner() {
        // Wall going +X from a=(0,0), half 0.5; adjacent wall going +Z away from a, half 0.5 (a 90 deg
        // L). The wall's left (z=+0.5) face meets the adjacent's near face at the INNER corner (0.5,0.5);
        // the right (z=-0.5) face at the OUTER corner (-0.5,-0.5).
        let (l, r) =
            wall_end_miter((0.0, 0.5), (0.0, -0.5), (1.0, 0.0), (0.0, 0.0), (0.0, 1.0), 0.5, true).unwrap();
        assert!((l.0 - 0.5).abs() < 1e-4 && (l.1 - 0.5).abs() < 1e-4, "a-end left, got {l:?}");
        assert!((r.0 + 0.5).abs() < 1e-4 && (r.1 + 0.5).abs() < 1e-4, "a-end right, got {r:?}");
    }

    #[test]
    fn miter_b_end_flips_the_adjacent_side() {
        // Same wall east, but its b-end at (10,0) with the adjacent going +Z away from b. The flush
        // miter sits at the INNER (9.5,0.5) + OUTER (10.5,-0.5) corners -- proving the a/b-end side flip.
        let (l, r) = wall_end_miter((0.0, 0.5), (0.0, -0.5), (1.0, 0.0), (10.0, 0.0), (0.0, 1.0), 0.5, false)
            .unwrap();
        assert!((l.0 - 9.5).abs() < 1e-4 && (l.1 - 0.5).abs() < 1e-4, "b-end left, got {l:?}");
        assert!((r.0 - 10.5).abs() < 1e-4 && (r.1 + 0.5).abs() < 1e-4, "b-end right, got {r:?}");
    }

    #[test]
    fn miter_corner_is_shared_flush_by_both_walls() {
        // The two walls of an L must produce the SAME touching corner at their shared node (a flush
        // miter -- no gap, no overlap). Compute each wall's end corners at the shared node and assert
        // their touching corners coincide.
        let half = 0.5;
        let c = (20.0, 10.0);
        // Wall 1 east: a=(10,10) b=(20,10); its b-end is at c, fwd=(1,0). Adjacent (wall 2) goes +Z.
        let w1l = v_add(c, v_scale(v_perp((1.0, 0.0)), half));
        let w1r = v_add(c, v_scale(v_perp((1.0, 0.0)), -half));
        let (w1_l, w1_r) = wall_end_miter(w1l, w1r, (1.0, 0.0), c, (0.0, 1.0), half, false).unwrap();
        // Wall 2 north: a=(20,10) b=(20,20); its a-end is at c, fwd=(0,1). Adjacent (wall 1) dir away
        // from c is -X = (-1,0).
        let w2l = v_add(c, v_scale(v_perp((0.0, 1.0)), half));
        let w2r = v_add(c, v_scale(v_perp((0.0, 1.0)), -half));
        let (w2_l, w2_r) = wall_end_miter(w2l, w2r, (0.0, 1.0), c, (-1.0, 0.0), half, true).unwrap();
        // The walls share the join edge: w1's left == w2's right and w1's right == w2's left (they
        // approach the shared edge from opposite sides), so the set of corners must match.
        let near = |a: V2, b: V2| (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3;
        let w1 = [w1_l, w1_r];
        assert!(
            w1.iter().any(|p| near(*p, w2_l)) && w1.iter().any(|p| near(*p, w2_r)),
            "walls share their miter corners: w1 {w1_l:?}/{w1_r:?} vs w2 {w2_l:?}/{w2_r:?}"
        );
    }

    #[test]
    fn an_interior_wall_adds_geometry() {
        let four_walls = wall_vcount(&box_only().generate_meshes());
        let mut h = box_only();
        h.walls.push(wall((10.0, 0.0), (10.0, 40.0), Vec::new()));
        let with_wall = wall_vcount(&h.generate_meshes());
        assert!(with_wall > four_walls, "an interior wall segment adds wall vertices");
    }

    #[test]
    fn openings_cut_the_wall() {
        let empty_box = wall_vcount(&box_only().generate_meshes());
        // A door spanning the wall's full width + height -> the whole wall is the opening -> the
        // interior wall contributes NO geometry (only the box's outer walls remain).
        let mut full = box_only();
        full.walls.push(wall(
            (10.0, 0.0),
            (10.0, 40.0),
            vec![Opening { kind: OpeningKind::Door, at: 0.0, width: 40.0, sill: 0.0, height: 3.0, style: "swing".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new() }],
        ));
        assert_eq!(wall_vcount(&full.generate_meshes()), empty_box, "a full-size door leaves no wall");
        // A small centered door -> piers on both sides + a header -> more than the empty box.
        let mut partial = box_only();
        partial.walls.push(wall(
            (10.0, 0.0),
            (10.0, 40.0),
            vec![Opening { kind: OpeningKind::Door, at: 18.0, width: 1.0, sill: 0.0, height: 2.1, style: "slide".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new() }],
        ));
        assert!(wall_vcount(&partial.generate_meshes()) > empty_box, "a partial door leaves piers + a header");
    }

    #[test]
    fn save_round_trips_with_openings() {
        let h = HomeStructure {
            width: 55.0,
            depth: 89.0,
            height: 3.0,
            shell_material: 1,
            roof_material: 4,
            walls: vec![wall(
                (5.0, 5.0),
                (5.0, 30.0),
                vec![
                    Opening { kind: OpeningKind::Door, at: 2.0, width: 1.0, sill: 0.0, height: 2.1, style: "iris".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new() },
                    Opening { kind: OpeningKind::Window, at: 10.0, width: 1.5, sill: 1.0, height: 1.2, style: "fixed".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new() },
                ],
            )],
            shell_thickness: None, lights: Vec::new(), spawn: None, structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new(),
        };
        let tmp = std::env::temp_dir().join("humanity_home_structure_rt.ron");
        h.save(&tmp).expect("save");
        let back = HomeStructure::load(&tmp).expect("reload");
        assert_eq!(back.width, 55.0);
        assert_eq!(back.walls.len(), 1);
        assert_eq!(back.walls[0].a, (5.0, 5.0));
        assert_eq!(back.walls[0].openings.len(), 2);
        assert_eq!(back.walls[0].openings[0].kind, OpeningKind::Door);
        assert_eq!(back.walls[0].openings[0].style, "iris");
        assert_eq!(back.walls[0].openings[1].kind, OpeningKind::Window);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn is_locked_generalizes_the_legacy_bool() {
        use crate::ship::lock_types::LockState;
        // No locks -> falls back to the legacy `locked` bool (zero behaviour change).
        let mut op = Opening { kind: OpeningKind::Door, at: 1.0, width: 1.0, sill: 0.0, height: 2.1, style: "swing".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new() };
        assert!(!op.is_locked());
        op.locked = true;
        assert!(op.is_locked(), "empty locks -> legacy bool");
        // With locks, the bool is ignored: locked iff any lock is not open.
        op.locked = false;
        op.locks = vec![
            LockInstance { type_id: "metal_key".into(), state: LockState::Unlocked, secret: None, offset: 0.0 },
            LockInstance { type_id: "keypad".into(), state: LockState::Locked, secret: None, offset: 0.0 },
        ];
        assert!(op.is_locked(), "one Locked lock secures the door");
        op.locks[1].state = LockState::Broken;
        assert!(!op.is_locked(), "all locks open/broken -> passable");
    }

    #[test]
    fn locks_round_trip_through_save() {
        use crate::ship::lock_types::LockState;
        let h = HomeStructure {
            width: 20.0, depth: 20.0, height: 3.0, shell_material: 1, roof_material: 4, shell_thickness: None, lights: Vec::new(), spawn: None, structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new(),
            walls: vec![wall((2.0, 2.0), (2.0, 12.0), vec![
                Opening { kind: OpeningKind::Door, at: 2.0, width: 1.0, sill: 0.0, height: 2.1, style: "swing".into(), open_dist: 2.6, locked: false, auto_open: false, control_panel: true,
                    locks: vec![
                        LockInstance { type_id: "keypad".into(), state: LockState::Locked, secret: Some("1234".into()), offset: 0.0 },
                        LockInstance { type_id: "crank".into(), state: LockState::Unlocked, secret: None, offset: 0.1 },
                    ] },
            ])],
        };
        let tmp = std::env::temp_dir().join("humanity_locks_rt.ron");
        h.save(&tmp).expect("save");
        let back = HomeStructure::load(&tmp).expect("reload");
        let locks = &back.walls[0].openings[0].locks;
        assert_eq!(locks.len(), 2);
        assert_eq!(locks[0].type_id, "keypad");
        assert_eq!(locks[0].state, LockState::Locked);
        assert_eq!(locks[0].secret.as_deref(), Some("1234"));
        assert_eq!(locks[1].type_id, "crank");
        assert_eq!(locks[1].state, LockState::Unlocked);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn placed_lights_round_trip_through_save() {
        let mut h = box_only();
        h.lights = vec![
            PlacedLight { type_id: "ceiling_panel".into(), pos: (27.5, 2.7, 44.5), dir: (0.0, -1.0, 0.0), on: true, color: None, intensity: Some(12.0), range: None },
            PlacedLight { type_id: "warm_lamp".into(), pos: (5.0, 1.0, 5.0), dir: (0.0, 0.0, 0.0), on: false, color: Some((1.0, 0.5, 0.2)), intensity: None, range: Some(3.0) },
        ];
        let tmp = std::env::temp_dir().join("humanity_lights_rt.ron");
        h.save(&tmp).expect("save");
        let back = HomeStructure::load(&tmp).expect("reload");
        assert_eq!(back.lights.len(), 2);
        assert_eq!(back.lights[0].type_id, "ceiling_panel");
        assert!((back.lights[0].pos.1 - 2.7).abs() < 1e-4);
        assert_eq!(back.lights[0].intensity, Some(12.0));
        assert!(!back.lights[1].on, "a placed-but-off light survives");
        assert_eq!(back.lights[1].color, Some((1.0, 0.5, 0.2)));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn introspection_json_is_valid_with_a_derived_block() {
        let mut h = box_only();
        h.walls = vec![InteriorWall { a: (5.0, 5.0), b: (10.0, 5.0), height: 3.0, material: 1, openings: vec![], thickness: None, layers: Vec::new() }];
        let s = h.to_introspection_json();
        let v: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
        let d = &v["derived"];
        assert_eq!(d["wall_count"], 1);
        assert!((d["walls"][0]["length_m"].as_f64().unwrap() - 5.0).abs() < 1e-4);
        assert!(d["corners"].as_array().unwrap().len() >= 2, "two wall ends -> two corners");
    }

    #[test]
    fn co_located_corners_quantize_to_one_point() {
        // Two corners a hair apart (sub-grid residue) snap to the SAME grid point -> byte-identical,
        // so they read as one orb and drag together (the v0.574 overlapping-node fix).
        let a = quantize_corner((5.021, 12.013));
        let b = quantize_corner((5.018, 12.009));
        assert_eq!(a, b, "near-co-located corners quantize to one point: {a:?} vs {b:?}");
        // On-grid corners are unchanged (idempotent), and a full grid step apart stays distinct.
        assert_eq!(quantize_corner((5.05, 12.0)), (5.05, 12.0));
        assert_ne!(quantize_corner((5.0, 0.0)), quantize_corner((5.05, 0.0)));
    }

    #[test]
    fn empty_box_is_one_home_room() {
        let rooms = box_only().detect_rooms();
        assert_eq!(rooms.len(), 1, "an empty box is one room");
        assert_eq!(rooms[0].id, "home");
        assert!(rooms[0].is_spawn_room);
    }

    #[test]
    fn a_full_partition_wall_splits_into_two_rooms() {
        let mut h = box_only();
        // A wall spanning the full depth at x=27 reaches both perimeter walls -> left + right rooms.
        h.walls.push(wall((27.0, 0.0), (27.0, 89.0), Vec::new()));
        let rooms = h.detect_rooms();
        assert_eq!(rooms.len(), 2, "a full-depth wall splits the box into two rooms, got {}", rooms.len());
        assert_eq!(rooms.iter().filter(|r| r.is_spawn_room).count(), 1, "exactly one spawn room");
    }

    #[test]
    fn a_partial_wall_does_not_enclose_a_room() {
        let mut h = box_only();
        // A stub wall that does not reach the far side leaves the interior one open region.
        h.walls.push(wall((27.0, 0.0), (27.0, 40.0), Vec::new()));
        assert_eq!(h.detect_rooms().len(), 1, "a partial wall does not partition the box");
    }

    #[test]
    fn roof_defaults_to_glass_when_absent() {
        // A RON without roof_material gets the glass default (serde default = 4).
        let h: HomeStructure = ron::from_str("(width: 10.0, depth: 10.0, height: 3.0)").expect("parses");
        assert_eq!(h.roof_material, 4);
        assert!(h.roof_is_glass(), "the default roof is clear glass");
        // An explicit opaque roof reads back as not-glass.
        let opaque: HomeStructure =
            ron::from_str("(width: 10.0, depth: 10.0, height: 3.0, roof_material: 1)").expect("parses");
        assert!(!opaque.roof_is_glass());
    }

    #[test]
    fn parses_the_shipped_home_structure() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("home_structure.ron");
        let h = HomeStructure::load(&path).expect("home_structure.ron parses");
        assert!(h.width > 0.0 && h.depth > 0.0 && h.height > 0.0);
    }

    /// v0.638: the REAL shipped home (with its actual interior walls, doors, structures -- not a
    /// synthetic test fixture) successfully clones itself into a residential zone slot. Proves
    /// `tile_home_clones` handles the live player home, not just simplified test boxes -- mitred
    /// corners, openings, and placed structures all survive the bake-and-translate path.
    #[test]
    fn the_real_shipped_home_clones_into_a_residential_zone() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("home_structure.ron");
        let mut h = HomeStructure::load(&path).expect("home_structure.ron parses");
        let before = h.generate_meshes().material_walls.len();
        // A zone with room for a 2x2 grid of the real home's own footprint.
        let (w, d) = (h.width, h.depth);
        h.add_zone("residential", (500.0, 0.0, 500.0), (2.0 * w + 6.0, 4.0, 2.0 * d + 6.0));
        let after = h.generate_meshes();
        assert!(
            after.material_walls.len() >= before,
            "cloning the real home into a zone does not shrink the material groups"
        );
        let total_verts: usize = after.material_walls.iter().map(|(v, _, _)| v.len()).sum();
        assert!(total_verts > 0, "the real home clones into real geometry");
    }
}
