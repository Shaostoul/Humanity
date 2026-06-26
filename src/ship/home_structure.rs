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
        for wseg in &self.walls {
            let a = Vec3::new(wseg.a.0, 0.0, wseg.a.1);
            let b = Vec3::new(wseg.b.0, 0.0, wseg.b.1);
            let g = by_mat.entry(wseg.material).or_insert_with(|| (Vec::new(), Vec::new()));
            merge(g, wall_with_openings(a, b, wseg.height.max(0.1), wseg.resolved_thickness(), &wseg.openings));
        }
        // Corner columns (v0.549): fill each interior-wall JOIN -- a corner shared by >= 2 walls --
        // with a slim cylinder of the wall's half-thickness, so the overlapping square wall ends read
        // as a clean round column instead of clipping cubes (operator note). Only at joins (a free
        // wall end keeps its square cap), low-poly, in the most-OPAQUE meeting wall's material (so a
        // junction with any solid wall reads solid, never a see-through gap).
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
            if *count >= 2 {
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

/// A vertical cylinder SIDE surface (no caps -- the floor + ceiling hide them) at (cx, cz): the
/// corner column that fills an interior-wall join so overlapping square wall ends read as a clean
/// round column. Double-sided (the home is viewed from inside) and low-poly. (v0.549)
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

/// Build a wall from `a` to `b` (height `h`, given thickness) with door/window openings CUT OUT:
/// full-height piers between/around the openings, a header above each opening, and a sill panel
/// below a window. A door (sill 0, full height) leaves a clean gap; a window leaves sill + header.
/// Overlapping or off-end openings are skipped for a clean walk. (v0.533)
fn wall_with_openings(
    a: Vec3,
    b: Vec3,
    h: f32,
    thickness: f32,
    openings: &[Opening],
) -> (Vec<Vertex>, Vec<u32>) {
    let mut out: (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
    let total = (b - a).length();
    if total < 1e-4 {
        return out;
    }
    let dir = (b - a) / total;
    let pt = |s: f32| a + dir * s.clamp(0.0, total);

    let mut ops: Vec<&Opening> = openings.iter().filter(|o| o.width > 0.01).collect();
    ops.sort_by(|x, y| x.at.partial_cmp(&y.at).unwrap_or(std::cmp::Ordering::Equal));

    let mut cursor = 0.0f32;
    for op in ops {
        let raw_start = op.at.clamp(0.0, total);
        let end = (op.at + op.width).clamp(0.0, total);
        if end <= cursor || raw_start >= total {
            continue; // overlaps the previous opening or runs off the end
        }
        let start = raw_start.max(cursor);
        // Full-height pier before this opening.
        if start > cursor + 1e-4 {
            merge(&mut out, wall_box(pt(cursor), pt(start), 0.0, h, thickness));
        }
        // Around the aperture [start, end] x [sill, sill+height]: a sill panel (windows) + a header.
        let sill = op.sill.max(0.0).min(h);
        let top = (sill + op.height).clamp(0.0, h);
        if sill > 0.01 {
            merge(&mut out, wall_box(pt(start), pt(end), 0.0, sill, thickness));
        }
        if top < h - 0.01 {
            merge(&mut out, wall_box(pt(start), pt(end), top, h - top, thickness));
        }
        cursor = end;
    }
    // Remaining full-height pier after the last opening.
    if cursor < total - 1e-4 {
        merge(&mut out, wall_box(pt(cursor), pt(total), 0.0, h, thickness));
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
            vec![Opening { kind: OpeningKind::Door, at: 0.0, width: 40.0, sill: 0.0, height: 3.0, style: "swing".into(), open_dist: 2.6, locked: false }],
        ));
        assert_eq!(wall_vcount(&full.generate_meshes()), empty_box, "a full-size door leaves no wall");
        // A small centered door -> piers on both sides + a header -> more than the empty box.
        let mut partial = box_only();
        partial.walls.push(wall(
            (10.0, 0.0),
            (10.0, 40.0),
            vec![Opening { kind: OpeningKind::Door, at: 18.0, width: 1.0, sill: 0.0, height: 2.1, style: "slide".into(), open_dist: 2.6, locked: false }],
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
                    Opening { kind: OpeningKind::Door, at: 2.0, width: 1.0, sill: 0.0, height: 2.1, style: "iris".into(), open_dist: 2.6, locked: false },
                    Opening { kind: OpeningKind::Window, at: 10.0, width: 1.5, sill: 1.0, height: 1.2, style: "fixed".into(), open_dist: 2.6, locked: false },
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
