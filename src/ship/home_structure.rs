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
}

impl InteriorWall {
    /// Resolved thickness (metres): the wall's explicit override, else its material's default, else
    /// the legacy 0.15 m fallback.
    pub fn resolved_thickness(&self) -> f32 {
        self.thickness
            .filter(|t| *t > 0.0)
            .unwrap_or_else(|| wall_material(self.material).map_or(WALL_THICKNESS, |m| m.default_thickness_m))
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
            let g = by_mat.entry(wseg.material).or_insert_with(|| (Vec::new(), Vec::new()));
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
            for cp in [wseg.a, wseg.b] {
                let key = ((cp.0 * 20.0).round() as i32, (cp.1 * 20.0).round() as i32);
                let e = joins.entry(key).or_insert((0.0, 0, cp, wseg.material, wt));
                e.0 = e.0.max(wseg.height.max(0.1));
                e.1 += 1;
                // Prefer the higher-alpha (more opaque) material; ties keep the first seen, so the
                // result is deterministic regardless of wall order.
                if Self::material_color(wseg.material)[3] > Self::material_color(e.3)[3] {
                    e.3 = wseg.material;
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
        let material_walls: Vec<(Vec<Vertex>, Vec<u32>, [f32; 4])> = groups
            .into_iter()
            .map(|(mid, (v, i))| (v, i, Self::material_color(mid)))
            .collect();

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
        HomeStructure { width: 55.0, depth: 89.0, height: 3.0, shell_material: 1, roof_material: 4, walls: Vec::new(), shell_thickness: None }
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
        InteriorWall { a, b, height: 3.0, material: 1, openings, thickness: None }
    }

    #[test]
    fn clip_pulls_a_mid_span_t_back_to_the_face() {
        // M runs along +X at z=0, 0.4 m thick (half 0.2). W comes up from (5,-3) and its end (5,0.1)
        // lands INSIDE M's body; it should clip back to M's NEAR face at z = -0.2.
        let m = InteriorWall { a: (0.0, 0.0), b: (10.0, 0.0), height: 3.0, material: 1, openings: vec![], thickness: Some(0.4) };
        let w = InteriorWall { a: (5.0, -3.0), b: (5.0, 0.1), height: 3.0, material: 1, openings: vec![], thickness: Some(0.1) };
        let walls = vec![m, w];
        let (clipped, dist) = clip_end_to_walls((5.0, 0.1), (5.0, -3.0), 1, &walls);
        assert!((clipped.1 + 0.2).abs() < 1e-3, "clipped to M near face z=-0.2, got {clipped:?}");
        assert!(dist > 0.0, "end was pulled back, dist {dist}");
    }

    #[test]
    fn clip_leaves_a_free_end_alone() {
        let m = InteriorWall { a: (0.0, 0.0), b: (10.0, 0.0), height: 3.0, material: 1, openings: vec![], thickness: Some(0.4) };
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
            shell_thickness: None,
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
            width: 20.0, depth: 20.0, height: 3.0, shell_material: 1, roof_material: 4, shell_thickness: None,
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
}
