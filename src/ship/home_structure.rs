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
fn default_door_style() -> String {
    "swing".to_string()
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
}

/// A home (or any structure): a FIXED outer box + freely-placed interior walls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeStructure {
    /// Outer box footprint + height (metres): width (X), depth (Z), height (Y). Fixed aboard the ship.
    pub width: f32,
    pub depth: f32,
    pub height: f32,
    /// Shell (floor / outer walls / ceiling) material id. Steel (1) by default.
    #[serde(default = "default_steel")]
    pub shell_material: u32,
    /// The editable interior walls (segments in the floor plan).
    #[serde(default)]
    pub walls: Vec<InteriorWall>,
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

    /// Material color (rgba) for a material id. Steel grey is the default for the all-steel box.
    fn material_color(m: u32) -> [f32; 4] {
        match m {
            1 => [0.55, 0.57, 0.62, 1.0], // steel
            2 => [0.62, 0.62, 0.60, 1.0], // concrete
            3 => [0.55, 0.40, 0.24, 1.0], // wood
            _ => [0.50, 0.52, 0.56, 1.0], // grid / default
        }
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

        // Walls: the 4 outer box walls + every interior wall, merged into one mesh.
        let mut walls: (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
        let perimeter = [
            (Vec3::new(0.0, 0.0, 0.0), Vec3::new(w, 0.0, 0.0)),
            (Vec3::new(w, 0.0, 0.0), Vec3::new(w, 0.0, d)),
            (Vec3::new(w, 0.0, d), Vec3::new(0.0, 0.0, d)),
            (Vec3::new(0.0, 0.0, d), Vec3::new(0.0, 0.0, 0.0)),
        ];
        for (a, b) in perimeter {
            merge(&mut walls, wall_box(a, b, 0.0, h, WALL_THICKNESS));
        }
        for wseg in &self.walls {
            let a = Vec3::new(wseg.a.0, 0.0, wseg.a.1);
            let b = Vec3::new(wseg.b.0, 0.0, wseg.b.1);
            merge(
                &mut walls,
                wall_with_openings(a, b, wseg.height.max(0.1), WALL_THICKNESS, &wseg.openings),
            );
        }

        HomesteadMeshes {
            floors,
            walls,
            trim: (Vec::new(), Vec::new()),
            windows: (Vec::new(), Vec::new()),
            mirrors: (Vec::new(), Vec::new()),
            ceilings,
            room_info: vec![RoomInfo {
                id: "home".to_string(),
                center: Vec3::new(w * 0.5, h * 0.5, d * 0.5),
                dimensions: Vec3::new(w, h, d),
                is_hologram_room: false,
                is_spawn_room: true,
            }],
        }
    }
}

/// Append (verts, indices) onto an accumulator, offsetting the appended indices.
fn merge(acc: &mut (Vec<Vertex>, Vec<u32>), add: (Vec<Vertex>, Vec<u32>)) {
    let base = acc.0.len() as u32;
    acc.0.extend(add.0);
    acc.1.extend(add.1.into_iter().map(|i| i + base));
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
        HomeStructure { width: 55.0, depth: 89.0, height: 3.0, shell_material: 1, walls: Vec::new() }
    }

    #[test]
    fn box_generates_floor_ceiling_and_outer_walls() {
        let m = box_only().generate_meshes();
        assert_eq!(m.floors.len(), 1, "one floor for the box");
        assert!(!m.ceilings.0.is_empty(), "ceiling generated");
        assert!(!m.walls.0.is_empty(), "outer walls generated");
        assert_eq!(m.room_info.len(), 1, "one 'home' room for now");
        assert_eq!(m.room_info[0].id, "home");
        assert_eq!(m.room_info[0].dimensions, Vec3::new(55.0, 3.0, 89.0));
        assert_eq!(m.room_info[0].center, Vec3::new(27.5, 1.5, 44.5));
    }

    fn wall(a: (f32, f32), b: (f32, f32), openings: Vec<Opening>) -> InteriorWall {
        InteriorWall { a, b, height: 3.0, material: 1, openings }
    }

    #[test]
    fn an_interior_wall_adds_geometry() {
        let four_walls = box_only().generate_meshes().walls.0.len();
        let mut h = box_only();
        h.walls.push(wall((10.0, 0.0), (10.0, 40.0), Vec::new()));
        let with_wall = h.generate_meshes().walls.0.len();
        assert!(with_wall > four_walls, "an interior wall segment adds wall vertices");
    }

    #[test]
    fn openings_cut_the_wall() {
        let empty_box = box_only().generate_meshes().walls.0.len();
        // A door spanning the wall's full width + height -> the whole wall is the opening -> the
        // interior wall contributes NO geometry (only the box's outer walls remain).
        let mut full = box_only();
        full.walls.push(wall(
            (10.0, 0.0),
            (10.0, 40.0),
            vec![Opening { kind: OpeningKind::Door, at: 0.0, width: 40.0, sill: 0.0, height: 3.0, style: "swing".into() }],
        ));
        assert_eq!(full.generate_meshes().walls.0.len(), empty_box, "a full-size door leaves no wall");
        // A small centered door -> piers on both sides + a header -> more than the empty box.
        let mut partial = box_only();
        partial.walls.push(wall(
            (10.0, 0.0),
            (10.0, 40.0),
            vec![Opening { kind: OpeningKind::Door, at: 18.0, width: 1.0, sill: 0.0, height: 2.1, style: "slide".into() }],
        ));
        assert!(partial.generate_meshes().walls.0.len() > empty_box, "a partial door leaves piers + a header");
    }

    #[test]
    fn save_round_trips_with_openings() {
        let h = HomeStructure {
            width: 55.0,
            depth: 89.0,
            height: 3.0,
            shell_material: 1,
            walls: vec![wall(
                (5.0, 5.0),
                (5.0, 30.0),
                vec![
                    Opening { kind: OpeningKind::Door, at: 2.0, width: 1.0, sill: 0.0, height: 2.1, style: "iris".into() },
                    Opening { kind: OpeningKind::Window, at: 10.0, width: 1.5, sill: 1.0, height: 1.2, style: "fixed".into() },
                ],
            )],
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
    fn parses_the_shipped_home_structure() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("home_structure.ron");
        let h = HomeStructure::load(&path).expect("home_structure.ron parses");
        assert!(h.width > 0.0 && h.depth > 0.0 && h.height > 0.0);
    }
}
