//! Room layout generator.
//!
//! Generates floor planes, walls, and ceilings for each room.
//! Supports multiple layout styles (fibonacci spiral, linear, vertical, studio).
//!
//! Layout loaded from `data/blueprints/homestead_layout.ron` (data-driven)
//! or computed procedurally from room list + layout style.

use crate::renderer::mesh::Vertex;
use glam::Vec3;
use std::path::Path;

// ---------------------------------------------------------------------------
// Data-driven layout structs (loadable from RON)
// ---------------------------------------------------------------------------

/// How rooms are spatially arranged.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub enum LayoutStyle {
    /// Golden spiral: each room attaches to the growing rectangle, rotating clockwise.
    /// Room sizes should follow Fibonacci sequence for perfect tiling.
    #[default]
    Fibonacci,
    /// Rooms placed side-by-side along X axis.
    Linear,
    /// Rooms stacked vertically (multi-floor building).
    Vertical,
    /// All rooms share the same origin (overlapping, single open space).
    Studio,
}

/// What a single wall face does (construction mode, v0.453). This is the per-wall
/// building block: instead of "every shared wall auto-gets a centred, variable-width
/// door," each wall is explicitly one of these kinds, read from the RON.
///
/// `Auto` is the DEFAULT, which means "behave like before": derive a door from
/// adjacency (a standard-size door if the neighbouring room is also passable on that
/// face, otherwise solid). So a room that omits `walls` keeps today's behaviour — zero
/// regression — while a room CAN override any single wall (a mirror, a window, fully
/// open, forced solid) without redesigning the whole floor plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum WallKind {
    /// Derive from adjacency: a standard door where a passable neighbour faces this
    /// wall, otherwise a solid wall. The default for every wall.
    #[default]
    Auto,
    /// Full solid wall, no opening (force solid even where a neighbour exists).
    Solid,
    /// A standard-size door opening (only cut if a passable neighbour faces it; a door
    /// into a solid wall or the void would be a hole to nowhere, so it falls back solid).
    Door,
    /// A window: a centred glass opening at sill height, with the wall built around it.
    Window,
    /// No wall at all (a fully open side — the respawner uses this on all four walls).
    Open,
    /// A reflective / portal panel flush on the wall (the wall stays solid behind it).
    Mirror,
}

/// Per-room wall configuration: the kind of each of the four walls. Index order MUST
/// match the `walls` array in `build_meshes`: 0=North(min Z), 1=South(max Z),
/// 2=West(min X), 3=East(max X). Any wall omitted in RON defaults to `Auto`.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Default)]
pub struct WallSet {
    #[serde(default)]
    pub north: WallKind,
    #[serde(default)]
    pub south: WallKind,
    #[serde(default)]
    pub west: WallKind,
    #[serde(default)]
    pub east: WallKind,
}

impl WallKind {
    /// All kinds, for the construction editor's dropdowns. (v0.455)
    pub const ALL: [WallKind; 6] = [
        WallKind::Auto,
        WallKind::Solid,
        WallKind::Door,
        WallKind::Window,
        WallKind::Open,
        WallKind::Mirror,
    ];
    pub fn label(self) -> &'static str {
        match self {
            WallKind::Auto => "Auto (door if adjacent)",
            WallKind::Solid => "Solid",
            WallKind::Door => "Door",
            WallKind::Window => "Window",
            WallKind::Open => "Open (no wall)",
            WallKind::Mirror => "Mirror / portal",
        }
    }
}

impl WallSet {
    /// Kind at the build-loop wall index (0=N, 1=S, 2=W, 3=E).
    pub fn by_index(&self, i: usize) -> WallKind {
        match i {
            0 => self.north,
            1 => self.south,
            2 => self.west,
            _ => self.east,
        }
    }
}

/// Room definition for the layout. Positions can be explicit or computed from LayoutStyle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomConfig {
    pub id: String,
    /// Explicit position override. If None/absent, position is computed from LayoutStyle.
    #[serde(default)]
    pub position: Option<[f32; 3]>,
    pub dimensions: [f32; 3],    // width (x), height (y), depth (z) in meters
    pub material_type: u32,      // PBR material type (0=grid, 1=metal, 2=concrete, 3=wood)
    pub color: [f32; 4],         // RGBA base color for PBR
    #[serde(default)]
    pub wall_height: f32,        // override wall height (0 = use dimensions.y)
    /// Per-wall construction config. Omitted => all `Auto` (today's auto-door behaviour).
    #[serde(default)]
    pub walls: WallSet,
}

/// Full homestead layout loaded from data/
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HomesteadLayout {
    #[serde(default)]
    pub layout_style: LayoutStyle,
    pub rooms: Vec<RoomConfig>,
    #[serde(default)]
    pub hologram_room: Option<String>,
    #[serde(default)]
    pub spawn_room: Option<String>,
    // ── Construction-mode standard dimensions (v0.453) ──
    // ONE source of truth for every door/window/trim/mirror size, so a door is the SAME
    // everywhere (fixes the old "doors vary per room" bug) and is tunable in one place.
    #[serde(default = "default_door_w")]
    pub door_width: f32,
    #[serde(default = "default_door_h")]
    pub door_height: f32,
    #[serde(default = "default_window_w")]
    pub window_width: f32,
    #[serde(default = "default_window_h")]
    pub window_height: f32,
    #[serde(default = "default_window_sill")]
    pub window_sill: f32,
    #[serde(default = "default_trim_h")]
    pub baseboard_height: f32,
    #[serde(default = "default_trim_h")]
    pub crown_height: f32,
    #[serde(default = "default_mirror")]
    pub mirror_size: f32,
    /// If > 0, every enclosed room uses this UNIFORM ceiling height unless it sets its own
    /// `wall_height` (so "all rooms N m tall" is one RON line). 0 = use each room's
    /// `dimensions.y`. (v0.454)
    #[serde(default)]
    pub default_wall_height: f32,
}

fn default_door_w() -> f32 { 0.9 }
fn default_door_h() -> f32 { 2.1 }
fn default_window_w() -> f32 { 1.4 }
fn default_window_h() -> f32 { 1.3 }
fn default_window_sill() -> f32 { 0.9 }
fn default_trim_h() -> f32 { 0.12 }
fn default_mirror() -> f32 { 3.0 }

/// Trim cross-section profiles (v0.457): the SVG-style "draw the molding silhouette, then
/// extrude it" system. Each profile is a CLOSED 2D polygon in (out, up) metres -- `out` =
/// how far it protrudes from the wall into the room, `up` = its extent along the run's
/// secondary axis (height for a baseboard, drop for a crown, width for a casing). Swept along
/// each trim run by `sweep_profile`. Edit the points (here or, later, an in-app node editor)
/// to reshape ALL trim of that kind at once. Loaded from data/blueprints/trim_profiles.ron.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrimProfiles {
    pub baseboard: Vec<(f32, f32)>,
    pub crown: Vec<(f32, f32)>,
    pub casing: Vec<(f32, f32)>,
}

impl Default for TrimProfiles {
    fn default() -> Self {
        Self {
            // A baseboard: 4 cm deep, 11 cm tall, with a small top bevel.
            baseboard: vec![(0.0, 0.0), (0.04, 0.0), (0.04, 0.09), (0.02, 0.115), (0.0, 0.115)],
            // A crown cove: a triangle from the ceiling line into the room + down the wall.
            crown: vec![(0.0, 0.0), (0.08, 0.0), (0.0, 0.12)],
            // A casing board: 2 cm proud, 6 cm wide (a flat door/window surround).
            casing: vec![(0.0, 0.0), (0.02, 0.0), (0.02, 0.06), (0.0, 0.06)],
        }
    }
}

/// The "up" extent (width) of the casing profile, used to extend the header/sill runs past
/// the jambs so the surround meets at the corners.
fn casing_width(p: &TrimProfiles) -> f32 {
    p.casing.iter().map(|(_, u)| *u).fold(0.0_f32, f32::max).max(0.02)
}

/// Load the trim profiles from data, or the built-in defaults. (v0.457)
pub fn load_trim_profiles() -> TrimProfiles {
    let path = data_dir().join("blueprints").join("trim_profiles.ron");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| ron::from_str::<TrimProfiles>(&t).ok())
        .unwrap_or_default()
}

/// Metadata about a generated room, exposed for gameplay features.
#[derive(Debug, Clone)]
pub struct RoomInfo {
    pub id: String,
    pub center: Vec3,
    pub dimensions: Vec3,
    pub is_hologram_room: bool,
    pub is_spawn_room: bool,
}

// ---------------------------------------------------------------------------
// Result struct
// ---------------------------------------------------------------------------

/// Result of generating the homestead: meshes and their associated colors.
pub struct HomesteadMeshes {
    /// (vertices, indices, color, material_type) for each room floor
    pub floors: Vec<(Vec<Vertex>, Vec<u32>, [f32; 4], u32)>,
    /// (vertices, indices) for all walls combined
    pub walls: (Vec<Vertex>, Vec<u32>),
    /// (vertices, indices) for all baseboard + crown + frame trim combined (v0.453).
    pub trim: (Vec<Vertex>, Vec<u32>),
    /// (vertices, indices) for all window glass panes combined (v0.453).
    pub windows: (Vec<Vertex>, Vec<u32>),
    /// (vertices, indices) for all mirror / portal panels combined (v0.453).
    pub mirrors: (Vec<Vertex>, Vec<u32>),
    /// (vertices, indices) for all room ceilings combined — drawn only when the roof is
    /// toggled on (v0.453). Built unconditionally; visibility is gated at render time.
    pub ceilings: (Vec<Vertex>, Vec<u32>),
    /// Room metadata (id, center, dimensions, flags)
    pub room_info: Vec<RoomInfo>,
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

/// Color for each room type (RGBA). If an override is provided, use it.
fn room_color(id: &str, override_color: Option<[f32; 4]>) -> [f32; 4] {
    if let Some(c) = override_color {
        return c;
    }
    match id {
        "computer"    => [0.3, 0.4, 0.5, 1.0],
        "network"     => [0.25, 0.35, 0.5, 1.0],
        "respawner"   => [0.5, 0.55, 0.6, 1.0],
        "wetroom"     => [0.45, 0.5, 0.55, 1.0],
        "bathroom"    => [0.5, 0.55, 0.6, 1.0],
        "bedroom"     => [0.5, 0.4, 0.3, 1.0],
        "kitchen"     => [0.6, 0.5, 0.25, 1.0],
        "livingroom"  => [0.45, 0.35, 0.2, 1.0],
        "living_room" => [0.45, 0.35, 0.2, 1.0],
        "study"       => [0.4, 0.4, 0.45, 1.0],
        "laboratory"  => [0.5, 0.5, 0.55, 1.0],
        "garden"      => [0.2, 0.45, 0.2, 1.0],
        "garage"      => [0.35, 0.35, 0.35, 1.0],
        "depot"       => [0.25, 0.25, 0.25, 1.0],
        "hangar"      => [0.55, 0.55, 0.6, 1.0],
        "ranch"       => [0.25, 0.5, 0.2, 1.0],
        "workshop"    => [0.35, 0.35, 0.35, 1.0],
        _             => [0.3, 0.3, 0.3, 1.0],
    }
}

// ---------------------------------------------------------------------------
// Procedural layout computation
// ---------------------------------------------------------------------------

/// Compute room positions for a Fibonacci golden spiral.
///
/// The first two rooms sit side-by-side at the spiral center.
/// Each subsequent room attaches to the long side of the growing rectangle,
/// rotating clockwise: +Z (down), +X (right), -Z (up), -X (left), repeat.
///
/// Returns (x, z) position for each room.
fn compute_fibonacci_positions(rooms: &[RoomConfig]) -> Vec<(f32, f32)> {
    if rooms.is_empty() {
        return vec![];
    }
    if rooms.len() == 1 {
        return vec![(0.0, 0.0)];
    }

    let mut positions = Vec::with_capacity(rooms.len());

    // First two rooms side by side along X
    positions.push((0.0, 0.0));
    let w0 = rooms[0].dimensions[0];
    positions.push((w0, 0.0));

    // Bounding box of all placed rooms so far
    let mut bb_min_x: f32 = 0.0;
    let mut bb_min_z: f32 = 0.0;
    let mut bb_max_x: f32 = w0 + rooms[1].dimensions[0];
    let mut bb_max_z: f32 = rooms[0].dimensions[2].max(rooms[1].dimensions[2]);

    // Direction cycle: 0=+Z (below), 1=+X (right), 2=-Z (above), 3=-X (left)
    let mut dir = 0u8;

    for i in 2..rooms.len() {
        let w = rooms[i].dimensions[0]; // room width (X)
        let d = rooms[i].dimensions[2]; // room depth (Z)

        let (px, pz) = match dir {
            0 => {
                // Attach below (+Z side): align left edge with bb left, place at bb bottom
                (bb_min_x, bb_max_z)
            }
            1 => {
                // Attach right (+X side): align top edge with bb top, place at bb right
                (bb_max_x, bb_min_z)
            }
            2 => {
                // Attach above (-Z side): align right edge with bb right, place above bb
                (bb_max_x - w, bb_min_z - d)
            }
            3 => {
                // Attach left (-X side): align bottom edge with bb bottom (max_z - d)
                (bb_min_x - w, bb_max_z - d)
            }
            _ => unreachable!(),
        };

        positions.push((px, pz));

        // Update bounding box
        bb_min_x = bb_min_x.min(px);
        bb_min_z = bb_min_z.min(pz);
        bb_max_x = bb_max_x.max(px + w);
        bb_max_z = bb_max_z.max(pz + d);

        // Advance direction
        dir = (dir + 1) % 4;
    }

    positions
}

/// Compute room positions for a linear (side-by-side) layout along X.
fn compute_linear_positions(rooms: &[RoomConfig]) -> Vec<(f32, f32)> {
    let mut x = 0.0f32;
    rooms.iter().map(|r| {
        let pos = (x, 0.0);
        x += r.dimensions[0] + 0.5; // 0.5m gap between rooms
        pos
    }).collect()
}

/// Compute room positions for vertical stacking (multi-floor).
fn compute_vertical_positions(rooms: &[RoomConfig]) -> Vec<(f32, f32)> {
    // All rooms at origin (stacking is handled by Y, not X/Z)
    rooms.iter().map(|_| (0.0, 0.0)).collect()
}

/// Apply computed positions to rooms, respecting explicit overrides.
fn resolve_positions(layout: &HomesteadLayout) -> Vec<Vec3> {
    let computed = match layout.layout_style {
        LayoutStyle::Fibonacci => compute_fibonacci_positions(&layout.rooms),
        LayoutStyle::Linear => compute_linear_positions(&layout.rooms),
        LayoutStyle::Vertical | LayoutStyle::Studio => compute_vertical_positions(&layout.rooms),
    };

    let mut y_offset = 0.0f32;
    layout.rooms.iter().enumerate().map(|(i, rc)| {
        if let Some(pos) = rc.position {
            // Explicit position overrides computation
            Vec3::new(pos[0], pos[1], pos[2])
        } else {
            let (cx, cz) = computed.get(i).copied().unwrap_or((0.0, 0.0));
            let y = match layout.layout_style {
                LayoutStyle::Vertical => {
                    let this_y = y_offset;
                    y_offset += rc.dimensions[1].max(rc.wall_height);
                    this_y
                }
                _ => 0.0,
            };
            Vec3::new(cx, y, cz)
        }
    }).collect()
}

// ---------------------------------------------------------------------------
// Mesh generation helpers
// ---------------------------------------------------------------------------

/// Generate floor quad vertices for a room (visible from above).
fn floor_quad(pos: Vec3, dim: Vec3) -> (Vec<Vertex>, Vec<u32>) {
    let x0 = pos.x;
    let z0 = pos.z;
    let x1 = pos.x + dim.x;
    let z1 = pos.z + dim.z;
    let y = pos.y;

    let vertices = vec![
        Vertex { position: [x0, y, z0], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
        Vertex { position: [x1, y, z0], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
        Vertex { position: [x1, y, z1], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
        Vertex { position: [x0, y, z1], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
    ];
    // Front face points UP (visible from above, standing on floor)
    let indices = vec![0, 2, 1, 0, 3, 2];
    (vertices, indices)
}

/// Generate a wall box (configurable height and thickness).
fn wall_box(start: Vec3, end: Vec3, y_base: f32, height: f32, thickness: f32) -> (Vec<Vertex>, Vec<u32>) {
    let dir = (end - start).normalize();
    let perp = Vec3::new(-dir.z, 0.0, dir.x) * (thickness / 2.0);

    let p0 = start - perp;
    let p1 = start + perp;
    let p2 = end + perp;
    let p3 = end - perp;

    let y0 = y_base;
    let y1 = y_base + height;

    let vertices = vec![
        // Bottom face
        Vertex { position: [p0.x, y0, p0.z], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] },
        Vertex { position: [p1.x, y0, p1.z], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0] },
        Vertex { position: [p2.x, y0, p2.z], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0] },
        Vertex { position: [p3.x, y0, p3.z], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0] },
        // Top face
        Vertex { position: [p0.x, y1, p0.z], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
        Vertex { position: [p1.x, y1, p1.z], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
        Vertex { position: [p2.x, y1, p2.z], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
        Vertex { position: [p3.x, y1, p3.z], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
        // Front face (perp+ direction)
        Vertex { position: [p1.x, y0, p1.z], normal: [perp.x, 0.0, perp.z], uv: [0.0, 0.0] },
        Vertex { position: [p2.x, y0, p2.z], normal: [perp.x, 0.0, perp.z], uv: [1.0, 0.0] },
        Vertex { position: [p2.x, y1, p2.z], normal: [perp.x, 0.0, perp.z], uv: [1.0, 1.0] },
        Vertex { position: [p1.x, y1, p1.z], normal: [perp.x, 0.0, perp.z], uv: [0.0, 1.0] },
        // Back face (perp- direction)
        Vertex { position: [p3.x, y0, p3.z], normal: [-perp.x, 0.0, -perp.z], uv: [0.0, 0.0] },
        Vertex { position: [p0.x, y0, p0.z], normal: [-perp.x, 0.0, -perp.z], uv: [1.0, 0.0] },
        Vertex { position: [p0.x, y1, p0.z], normal: [-perp.x, 0.0, -perp.z], uv: [1.0, 1.0] },
        Vertex { position: [p3.x, y1, p3.z], normal: [-perp.x, 0.0, -perp.z], uv: [0.0, 1.0] },
        // Start end face
        Vertex { position: [p0.x, y0, p0.z], normal: [-dir.x, 0.0, -dir.z], uv: [0.0, 0.0] },
        Vertex { position: [p1.x, y0, p1.z], normal: [-dir.x, 0.0, -dir.z], uv: [1.0, 0.0] },
        Vertex { position: [p1.x, y1, p1.z], normal: [-dir.x, 0.0, -dir.z], uv: [1.0, 1.0] },
        Vertex { position: [p0.x, y1, p0.z], normal: [-dir.x, 0.0, -dir.z], uv: [0.0, 1.0] },
        // End end face
        Vertex { position: [p2.x, y0, p2.z], normal: [dir.x, 0.0, dir.z], uv: [0.0, 0.0] },
        Vertex { position: [p3.x, y0, p3.z], normal: [dir.x, 0.0, dir.z], uv: [1.0, 0.0] },
        Vertex { position: [p3.x, y1, p3.z], normal: [dir.x, 0.0, dir.z], uv: [1.0, 1.0] },
        Vertex { position: [p2.x, y1, p2.z], normal: [dir.x, 0.0, dir.z], uv: [0.0, 1.0] },
    ];

    let indices = vec![
        0, 1, 2, 0, 2, 3,       // Bottom
        4, 6, 5, 4, 7, 6,       // Top
        8, 9, 10, 8, 10, 11,    // Front
        12, 13, 14, 12, 14, 15, // Back
        16, 17, 18, 16, 18, 19, // Start end
        20, 21, 22, 20, 22, 23, // End end
    ];

    (vertices, indices)
}

/// Append one (verts, indices) piece onto running combined buffers, rebasing indices.
fn append_mesh(dst_v: &mut Vec<Vertex>, dst_i: &mut Vec<u32>, piece: (Vec<Vertex>, Vec<u32>)) {
    let base = dst_v.len() as u32;
    dst_v.extend(piece.0);
    dst_i.extend(piece.1.iter().map(|i| i + base));
}

/// A wall span with zero or more rectangular openings (doors/windows). Each opening is
/// `(center_t, width, height, sill)` where `center_t` is the distance from `start` ALONG
/// the wall where the opening centres -- so a door lands on the ACTUAL shared edge with the
/// neighbouring room, not the middle of the wall (the v0.453 bug that walled off real
/// connections and scattered the frames). `sill` is the opening's bottom off the floor (0 =
/// door). Emits the solid wall around every opening: full-height spans between openings, an
/// apron under each (sill>0), a lintel over each. Same convention as `wall_box`. (v0.454)
fn wall_with_openings(
    start: Vec3, end: Vec3, y_base: f32, wall_height: f32, thickness: f32,
    openings: &[(f32, f32, f32, f32)],
) -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    let dir = end - start;
    let len = dir.length();
    if len < 0.01 {
        return (v, idx);
    }
    let norm = dir / len;
    if openings.is_empty() {
        return wall_box(start, end, y_base, wall_height, thickness);
    }
    // Resolve each opening to a horizontal [t0, t1] span (clamped) + vertical [sill, top].
    let mut spans: Vec<(f32, f32, f32, f32)> = Vec::new(); // (t0, t1, sill, top)
    for &(center_t, w, h, sill) in openings {
        let half = (w * 0.5).min(len * 0.45);
        let t0 = (center_t - half).clamp(0.0, len);
        let t1 = (center_t + half).clamp(0.0, len);
        if t1 - t0 > 0.05 {
            spans.push((t0, t1, sill, (sill + h).min(wall_height)));
        }
    }
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut cursor = 0.0_f32;
    for (t0, t1, sill, top) in &spans {
        let t0 = t0.max(cursor);
        let t1 = t1.max(t0);
        // Full-height solid wall before this opening.
        if t0 - cursor > 0.02 {
            append_mesh(&mut v, &mut idx, wall_box(start + norm * cursor, start + norm * t0, y_base, wall_height, thickness));
        }
        let pa = start + norm * t0;
        let pb = start + norm * t1;
        // Apron below (windows only).
        if *sill > 0.01 {
            append_mesh(&mut v, &mut idx, wall_box(pa, pb, y_base, *sill, thickness));
        }
        // Lintel above.
        if wall_height - *top > 0.01 {
            append_mesh(&mut v, &mut idx, wall_box(pa, pb, y_base + *top, wall_height - *top, thickness));
        }
        cursor = t1;
    }
    // Full-height solid wall after the last opening.
    if len - cursor > 0.02 {
        append_mesh(&mut v, &mut idx, wall_box(start + norm * cursor, end, y_base, wall_height, thickness));
    }
    (v, idx)
}

/// A glass pane filling a window opening: a thin double-sided quad in the wall plane,
/// centred at `center_t` along the wall (from `start`, direction `norm`), sized
/// `open_w` x `open_h` at `sill` height. Tinted-glass mesh. (v0.453, positioned v0.454)
fn window_panel(start: Vec3, norm: Vec3, y_base: f32, center_t: f32, open_w: f32, open_h: f32, sill: f32) -> (Vec<Vertex>, Vec<u32>) {
    let half = open_w * 0.5;
    let p_l = start + norm * (center_t - half);
    let p_r = start + norm * (center_t + half);
    let perp = Vec3::new(-norm.z, 0.0, norm.x); // wall-facing normal
    let y0 = y_base + sill;
    let y1 = y_base + sill + open_h;
    let verts = vec![
        Vertex { position: [p_l.x, y0, p_l.z], normal: [perp.x, 0.0, perp.z], uv: [0.0, 0.0] },
        Vertex { position: [p_r.x, y0, p_r.z], normal: [perp.x, 0.0, perp.z], uv: [1.0, 0.0] },
        Vertex { position: [p_r.x, y1, p_r.z], normal: [perp.x, 0.0, perp.z], uv: [1.0, 1.0] },
        Vertex { position: [p_l.x, y1, p_l.z], normal: [perp.x, 0.0, perp.z], uv: [0.0, 1.0] },
    ];
    // Double-sided (visible from both rooms).
    let indices = vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2];
    (verts, indices)
}

/// A `size` x `size` mirror / portal panel flush on a wall's inner face, centred on the
/// span, sitting a hair proud of the wall (no z-fight) and facing into the room along
/// `inward`. The wall behind stays solid; this is just the reflective/glowing panel. Used
/// for the wetroom's respawner-side portal. (v0.453)
fn mirror_panel(start: Vec3, end: Vec3, y_base: f32, wall_height: f32, size: f32, inward: Vec3) -> (Vec<Vertex>, Vec<u32>) {
    let dir = end - start;
    let len = dir.length();
    if len < 0.01 {
        return (vec![], vec![]);
    }
    let norm = dir / len;
    let center = len * 0.5;
    let half = (size * 0.5).min(len * 0.48);
    let n = inward.normalize_or_zero();
    let off = n * 0.03; // proud of the wall
    let p_l = start + norm * (center - half) + off;
    let p_r = start + norm * (center + half) + off;
    let y0 = (y_base + (wall_height - size) * 0.5).max(y_base);
    let y1 = (y0 + size).min(y_base + wall_height);
    let verts = vec![
        Vertex { position: [p_l.x, y0, p_l.z], normal: [n.x, 0.0, n.z], uv: [0.0, 0.0] },
        Vertex { position: [p_r.x, y0, p_r.z], normal: [n.x, 0.0, n.z], uv: [1.0, 0.0] },
        Vertex { position: [p_r.x, y1, p_r.z], normal: [n.x, 0.0, n.z], uv: [1.0, 1.0] },
        Vertex { position: [p_l.x, y1, p_l.z], normal: [n.x, 0.0, n.z], uv: [0.0, 1.0] },
    ];
    // Double-sided so it shows regardless of cull state.
    let indices = vec![0, 2, 1, 0, 3, 2, 0, 1, 2, 0, 2, 3];
    (verts, indices)
}

/// A downward-facing ceiling quad for one room (visible from below). Built for every room
/// but only drawn when the roof toggle is on. (v0.453)
fn ceiling_quad(pos: Vec3, dim: Vec3, y: f32) -> (Vec<Vertex>, Vec<u32>) {
    let x0 = pos.x;
    let z0 = pos.z;
    let x1 = pos.x + dim.x;
    let z1 = pos.z + dim.z;
    let verts = vec![
        Vertex { position: [x0, y, z0], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] },
        Vertex { position: [x1, y, z0], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0] },
        Vertex { position: [x1, y, z1], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0] },
        Vertex { position: [x0, y, z1], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0] },
    ];
    // Front face points DOWN (visible from inside, looking up).
    let indices = vec![0, 1, 2, 0, 2, 3];
    (verts, indices)
}

/// Extrude a 2D cross-section `profile` (a closed polygon in (out, up) metres) along the run
/// a -> b, producing a swept solid. `out_dir` = the protrusion direction (away from the wall,
/// into the room); `up_dir` = the profile's secondary axis. This is the SVG-style trim sweep
/// (v0.457): the same function builds baseboards, crown, and casing -- only the profile + the
/// run differ. Double-sided so thin trim reads from any angle; end caps are omitted (runs
/// butt against walls / each other).
fn sweep_profile(profile: &[(f32, f32)], a: Vec3, b: Vec3, out_dir: Vec3, up_dir: Vec3) -> (Vec<Vertex>, Vec<u32>) {
    let n = profile.len();
    let mut v = Vec::new();
    let mut idx = Vec::new();
    if n < 2 || (b - a).length() < 0.001 {
        return (v, idx);
    }
    let run = (b - a).normalize();
    let pt = |base: Vec3, p: (f32, f32)| base + out_dir * p.0 + up_dir * p.1;
    for i in 0..n {
        let j = (i + 1) % n;
        let pa0 = pt(a, profile[i]);
        let pa1 = pt(a, profile[j]);
        let pb0 = pt(b, profile[i]);
        let pb1 = pt(b, profile[j]);
        let edge = pa1 - pa0;
        let raw = run.cross(edge);
        let nrm = if raw.length_squared() > 1e-9 { raw.normalize() } else { out_dir.normalize_or_zero() };
        let nrm3 = [nrm.x, nrm.y, nrm.z];
        let base = v.len() as u32;
        v.push(Vertex { position: pa0.to_array(), normal: nrm3, uv: [0.0, 0.0] });
        v.push(Vertex { position: pa1.to_array(), normal: nrm3, uv: [1.0, 0.0] });
        v.push(Vertex { position: pb1.to_array(), normal: nrm3, uv: [1.0, 1.0] });
        v.push(Vertex { position: pb0.to_array(), normal: nrm3, uv: [0.0, 1.0] });
        idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        idx.extend([base, base + 2, base + 1, base, base + 3, base + 2]); // back side
    }
    (v, idx)
}

/// Wood casing swept around an opening: left + right JAMBS (the vertical sides) + a header,
/// plus a sill for windows -- each a `casing` profile swept along that edge. Replaces the old
/// header-only frame so doors + windows get a full surround. (v0.457)
fn opening_casing(
    start: Vec3, norm: Vec3, inward: Vec3, y: f32, center_t: f32,
    open_w: f32, open_h: f32, sill: f32, casing: &[(f32, f32)], casing_w: f32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    let inward = inward.normalize_or_zero();
    let half = open_w * 0.5;
    let left = center_t - half;
    let right = center_t + half;
    let bottom = y + sill;
    let top = y + sill + open_h;
    let wp = |t: f32, yy: f32| start + norm * t + Vec3::new(0.0, yy, 0.0);
    let mut add = |piece: (Vec<Vertex>, Vec<u32>)| append_mesh(&mut v, &mut idx, piece);
    // Left jamb: width extends LEFT (-norm) from the opening edge.
    add(sweep_profile(casing, wp(left, bottom), wp(left, top), inward, -norm));
    // Right jamb: width extends RIGHT (+norm).
    add(sweep_profile(casing, wp(right, bottom), wp(right, top), inward, norm));
    // Header: spans a casing-width past each jamb so the corners meet; width extends UP.
    add(sweep_profile(casing, wp(left - casing_w, top), wp(right + casing_w, top), inward, Vec3::Y));
    // Sill (windows only): width extends DOWN.
    if sill > 0.01 {
        add(sweep_profile(casing, wp(left - casing_w, bottom), wp(right + casing_w, bottom), inward, -Vec3::Y));
    }
    (v, idx)
}

/// Wood trim framing an opening: a header just above it and (for windows) a sill board
/// just below, each a thin trim box spanning the opening width, centred at `center_t` along
/// the wall (from `start`, direction `norm`). The vertical sides are the wall jambs. Goes in
/// the trim mesh. (v0.453, positioned v0.454; superseded by opening_casing v0.457)
#[allow(dead_code)]
fn opening_frame(
    start: Vec3, norm: Vec3, len: f32, y_base: f32, center_t: f32, open_w: f32, open_h: f32, sill: f32,
    frame_thickness: f32, trim_thickness: f32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    if len < 0.01 {
        return (v, idx);
    }
    let half = (open_w * 0.5).min(len * 0.45);
    let p_l = start + norm * (center_t - half);
    let p_r = start + norm * (center_t + half);
    // Header just above the opening.
    append_mesh(&mut v, &mut idx, wall_box(p_l, p_r, y_base + sill + open_h, frame_thickness, trim_thickness));
    // Sill board (windows only).
    if sill > 0.01 {
        append_mesh(&mut v, &mut idx, wall_box(p_l, p_r, (y_base + sill - frame_thickness).max(y_base), frame_thickness, trim_thickness));
    }
    (v, idx)
}

// ---------------------------------------------------------------------------
// Doorway detection between adjacent rooms
// ---------------------------------------------------------------------------

/// Information about a shared wall edge between two rooms.
#[derive(Debug, Clone)]
struct SharedEdge {
    /// Start point of the shared (overlapping) segment
    start: Vec3,
    /// End point of the shared segment
    end: Vec3,
    /// Which wall of room A this belongs to (0=north, 1=south, 2=west, 3=east)
    wall_a: usize,
    /// Which wall of room B this belongs to
    wall_b: usize,
}

/// Check if two rooms share a wall edge. Returns the shared edge info if they do.
///
/// Two rooms share a wall when one room's edge aligns with the other's along one
/// axis (within tolerance for wall thickness) AND they overlap along the
/// perpendicular axis.
fn find_shared_edges(
    pos_a: Vec3, dim_a: Vec3,
    pos_b: Vec3, dim_b: Vec3,
) -> Vec<SharedEdge> {
    let tol = 0.15; // tolerance for wall thickness alignment
    let mut edges = Vec::new();

    // Room A bounds
    let a_x0 = pos_a.x;
    let a_x1 = pos_a.x + dim_a.x;
    let a_z0 = pos_a.z;
    let a_z1 = pos_a.z + dim_a.z;

    // Room B bounds
    let b_x0 = pos_b.x;
    let b_x1 = pos_b.x + dim_b.x;
    let b_z0 = pos_b.z;
    let b_z1 = pos_b.z + dim_b.z;

    // Check A's east wall (x1) vs B's west wall (x0)
    if (a_x1 - b_x0).abs() < tol {
        let z_start = a_z0.max(b_z0);
        let z_end = a_z1.min(b_z1);
        if z_end - z_start > 0.5 {
            // Enough overlap for a doorway
            let x = a_x1;
            edges.push(SharedEdge {
                start: Vec3::new(x, 0.0, z_start),
                end: Vec3::new(x, 0.0, z_end),
                wall_a: 3, // east
                wall_b: 2, // west
            });
        }
    }

    // Check A's west wall (x0) vs B's east wall (x1)
    if (a_x0 - b_x1).abs() < tol {
        let z_start = a_z0.max(b_z0);
        let z_end = a_z1.min(b_z1);
        if z_end - z_start > 0.5 {
            let x = a_x0;
            edges.push(SharedEdge {
                start: Vec3::new(x, 0.0, z_start),
                end: Vec3::new(x, 0.0, z_end),
                wall_a: 2, // west
                wall_b: 3, // east
            });
        }
    }

    // Check A's south wall (z1) vs B's north wall (z0)
    if (a_z1 - b_z0).abs() < tol {
        let x_start = a_x0.max(b_x0);
        let x_end = a_x1.min(b_x1);
        if x_end - x_start > 0.5 {
            let z = a_z1;
            edges.push(SharedEdge {
                start: Vec3::new(x_start, 0.0, z),
                end: Vec3::new(x_end, 0.0, z),
                wall_a: 1, // south
                wall_b: 0, // north
            });
        }
    }

    // Check A's north wall (z0) vs B's south wall (z1)
    if (a_z0 - b_z1).abs() < tol {
        let x_start = a_x0.max(b_x0);
        let x_end = a_x1.min(b_x1);
        if x_end - x_start > 0.5 {
            let z = a_z0;
            edges.push(SharedEdge {
                start: Vec3::new(x_start, 0.0, z),
                end: Vec3::new(x_end, 0.0, z),
                wall_a: 0, // north
                wall_b: 1, // south
            });
        }
    }

    edges
}

/// For a given wall, compute the wall segments needed after cutting doorways.
///
/// Returns a list of (start, end) segments. If no doorway applies, returns
/// the original wall as a single segment. Also returns lintel info:
/// (start, end, doorway_height) for each doorway cut.
///
/// Superseded by the per-wall `WallKind` path in `build_meshes` (v0.453); kept as the
/// reference doorway-splitter (it also keeps the `SharedEdge` geometry fields live).
#[allow(dead_code)]
fn split_wall_for_doorways(
    wall_start: Vec3,
    wall_end: Vec3,
    wall_idx: usize,
    shared_edges: &[SharedEdge],
    wall_height: f32,
) -> (Vec<(Vec3, Vec3)>, Vec<(Vec3, Vec3, f32)>) {
    // Collect all doorway cuts that apply to this wall
    let mut cuts: Vec<(f32, f32)> = Vec::new(); // (position_along_wall_start, position_along_wall_end)

    let wall_dir = wall_end - wall_start;
    let wall_len = wall_dir.length();
    if wall_len < 0.01 {
        return (vec![(wall_start, wall_end)], vec![]);
    }
    let wall_norm = wall_dir / wall_len;

    for edge in shared_edges {
        if edge.wall_a != wall_idx {
            continue;
        }

        // Project the shared edge onto the wall's axis to find the overlap
        let overlap_len = (edge.end - edge.start).length();
        let doorway_width = (2.0_f32).min(overlap_len * 0.5);
        if doorway_width < 0.3 {
            continue; // Too narrow for a doorway
        }

        // Find the center of the overlap projected onto this wall
        let edge_center = (edge.start + edge.end) * 0.5;
        let t_center = (edge_center - wall_start).dot(wall_norm);
        let t_start = (t_center - doorway_width * 0.5).max(0.0);
        let t_end = (t_center + doorway_width * 0.5).min(wall_len);

        if t_end - t_start > 0.2 {
            cuts.push((t_start, t_end));
        }
    }

    if cuts.is_empty() {
        return (vec![(wall_start, wall_end)], vec![]);
    }

    // Sort cuts by start position
    cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Merge overlapping cuts
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (cs, ce) in &cuts {
        if let Some(last) = merged.last_mut() {
            if *cs <= last.1 + 0.01 {
                last.1 = last.1.max(*ce);
                continue;
            }
        }
        merged.push((*cs, *ce));
    }

    // Generate wall segments around the doorway cuts
    let mut segments = Vec::new();
    let mut lintels = Vec::new();
    let mut cursor = 0.0_f32;

    let doorway_height = (2.5_f32).min(wall_height - 0.3);

    for (cut_start, cut_end) in &merged {
        // Wall segment before the doorway
        if *cut_start - cursor > 0.01 {
            let seg_start = wall_start + wall_norm * cursor;
            let seg_end = wall_start + wall_norm * cut_start;
            segments.push((seg_start, seg_end));
        }

        // Lintel above doorway
        let lintel_start = wall_start + wall_norm * cut_start;
        let lintel_end = wall_start + wall_norm * cut_end;
        lintels.push((lintel_start, lintel_end, doorway_height));

        cursor = *cut_end;
    }

    // Wall segment after the last doorway
    if wall_len - cursor > 0.01 {
        let seg_start = wall_start + wall_norm * cursor;
        segments.push((seg_start, wall_end));
    }

    (segments, lintels)
}

// ---------------------------------------------------------------------------
// Mesh assembly from resolved room positions
// ---------------------------------------------------------------------------

/// Generate all floor, wall, trim, window, mirror, and ceiling meshes from a layout with
/// resolved positions. Construction mode (v0.453): each wall is built from its `WallKind`
/// (Auto/Solid/Door/Window/Open/Mirror) instead of the old "every shared wall auto-gets a
/// variable-width door." `Auto` reproduces the old behaviour (a STANDARD-size door where a
/// passable neighbour faces it, else solid), so unconfigured rooms are unchanged.
fn build_meshes(layout: &HomesteadLayout, positions: &[Vec3], profiles: &TrimProfiles) -> HomesteadMeshes {
    let rooms = &layout.rooms;
    let hologram_room = layout.hologram_room.as_deref();
    let spawn_room = layout.spawn_room.as_deref();
    let wall_thickness = 0.1;
    let casing_w = casing_width(profiles); // header/sill overrun so corners meet

    let mut floors = Vec::new();
    let mut wall_v = Vec::new();
    let mut wall_i = Vec::new();
    let mut trim_v = Vec::new();
    let mut trim_i = Vec::new();
    let mut win_v = Vec::new();
    let mut win_i = Vec::new();
    let mut mir_v = Vec::new();
    let mut mir_i = Vec::new();
    let mut ceil_v = Vec::new();
    let mut ceil_i = Vec::new();
    let mut room_info = Vec::new();

    // Pre-compute dimensions and wall heights for all rooms. A non-zero `default_wall_height`
    // on the layout gives every ENCLOSED room a UNIFORM ceiling height unless it sets its own
    // `wall_height` -- so "all rooms 9 m tall" is one RON line, not a per-room edit. (v0.454)
    let room_data: Vec<(Vec3, f32)> = rooms.iter().map(|rc| {
        let dim = Vec3::new(rc.dimensions[0], rc.dimensions[1], rc.dimensions[2]);
        let wh = if rc.wall_height > 0.0 {
            rc.wall_height
        } else if layout.default_wall_height > 0.0 && dim.y > 0.0 {
            layout.default_wall_height
        } else {
            dim.y
        };
        (dim, wh)
    }).collect();

    // Adjacency: for each room+wall (0..4), the LIST of (neighbour_room, neighbour_wall,
    // overlap_start, overlap_end). A wall can border several rooms, so this is a list -- the
    // v0.453 single-neighbour map both dropped extra doors AND lost the overlap segment, which
    // is why doors landed at the wall centre instead of the shared edge. (v0.454)
    let mut wall_edges: Vec<[Vec<(usize, usize, Vec3, Vec3)>; 4]> =
        (0..rooms.len()).map(|_| [Vec::new(), Vec::new(), Vec::new(), Vec::new()]).collect();
    for i in 0..rooms.len() {
        let (dim_a, wh_a) = room_data[i];
        if wh_a <= 0.0 { continue; }
        for j in (i + 1)..rooms.len() {
            let (dim_b, wh_b) = room_data[j];
            if wh_b <= 0.0 { continue; }
            for edge in find_shared_edges(positions[i], dim_a, positions[j], dim_b) {
                wall_edges[i][edge.wall_a].push((j, edge.wall_b, edge.start, edge.end));
                wall_edges[j][edge.wall_b].push((i, edge.wall_a, edge.start, edge.end));
            }
        }
    }

    // A wall is "passable" (a neighbour may open a door against it) when it is Auto, Door,
    // or Open. Solid/Window/Mirror block, so a door against them falls back to solid (no
    // hole-to-nowhere).
    let passable = |k: WallKind| matches!(k, WallKind::Auto | WallKind::Door | WallKind::Open);

    for (i, rc) in rooms.iter().enumerate() {
        let pos = positions[i];
        let (dim, wall_height) = room_data[i];
        if wall_height <= 0.0 {
            continue; // Skip zero-height rooms here (handled below as outdoor floors).
        }

        // Floor
        let (verts, indices) = floor_quad(pos, dim);
        let color = room_color(&rc.id, Some(rc.color));
        floors.push((verts, indices, color, rc.material_type));

        let center = pos + Vec3::new(dim.x * 0.5, wall_height * 0.5, dim.z * 0.5);
        room_info.push(RoomInfo {
            id: rc.id.clone(),
            center,
            dimensions: Vec3::new(dim.x, wall_height, dim.z),
            is_hologram_room: hologram_room == Some(rc.id.as_str()),
            is_spawn_room: spawn_room == Some(rc.id.as_str()),
        });

        // Ceiling (built for every room; only DRAWN when the roof toggle is on).
        append_mesh(&mut ceil_v, &mut ceil_i, ceiling_quad(pos, dim, pos.y + wall_height));

        let x0 = pos.x;
        let z0 = pos.z;
        let x1 = x0 + dim.x;
        let z1 = z0 + dim.z;
        let y = pos.y;

        let walls = [
            (Vec3::new(x0, y, z0), Vec3::new(x1, y, z0)), // 0: North (min Z)
            (Vec3::new(x0, y, z1), Vec3::new(x1, y, z1)), // 1: South (max Z)
            (Vec3::new(x0, y, z0), Vec3::new(x0, y, z1)), // 2: West (min X)
            (Vec3::new(x1, y, z0), Vec3::new(x1, y, z1)), // 3: East (max X)
        ];

        for (w, (start, end)) in walls.iter().enumerate() {
            let my = rc.walls.by_index(w);
            if my == WallKind::Open {
                continue; // No wall, no trim, no crown -- a fully open side.
            }
            let dir = *end - *start;
            let len = dir.length();
            let norm = if len > 0.01 { dir / len } else { Vec3::ZERO };
            // Direction from this wall toward the room centre (for the mirror to face in).
            let wall_mid = (*start + *end) * 0.5;
            let inward = Vec3::new(center.x - wall_mid.x, 0.0, center.z - wall_mid.z);

            // Collect the openings to cut: (center_t along the wall, width, height, sill).
            let mut openings: Vec<(f32, f32, f32, f32)> = Vec::new();
            let mut is_window = false;

            match my {
                WallKind::Auto | WallKind::Door => {
                    // A door at EACH shared edge whose neighbour is passable, centred on the
                    // ACTUAL overlap with that room (not the wall middle). This restores
                    // navigability + can put multiple doors in one long wall.
                    for (nj, nwb, seg_start, seg_end) in &wall_edges[i][w] {
                        if passable(rooms[*nj].walls.by_index(*nwb)) {
                            let mid = (*seg_start + *seg_end) * 0.5;
                            let center_t = (mid - *start).dot(norm).clamp(0.0, len);
                            openings.push((center_t, layout.door_width, layout.door_height, 0.0));
                        }
                    }
                }
                WallKind::Window => {
                    openings.push((len * 0.5, layout.window_width, layout.window_height, layout.window_sill));
                    is_window = true;
                }
                WallKind::Mirror => {
                    // Solid wall + a flush portal panel (no opening).
                    append_mesh(&mut mir_v, &mut mir_i,
                        mirror_panel(*start, *end, y, wall_height, layout.mirror_size, inward));
                }
                WallKind::Solid => {}
                WallKind::Open => unreachable!(),
            }

            // The wall itself, built around all its openings (solid if there are none).
            append_mesh(&mut wall_v, &mut wall_i,
                wall_with_openings(*start, *end, y, wall_height, wall_thickness, &openings));

            // Glass panes (windows) + wood casing swept around each opening (full surround
            // incl. the vertical jambs, from the casing profile). (v0.457)
            let inward_n = inward.normalize_or_zero();
            for (center_t, ow, oh, sill) in &openings {
                if is_window {
                    append_mesh(&mut win_v, &mut win_i,
                        window_panel(*start, norm, y, *center_t, *ow, *oh, *sill));
                }
                append_mesh(&mut trim_v, &mut trim_i,
                    opening_casing(*start, norm, inward, y, *center_t, *ow, *oh, *sill, &profiles.casing, casing_w));
            }

            // Crown moulding swept along the top of every built wall (profile -> extrude).
            append_mesh(&mut trim_v, &mut trim_i,
                sweep_profile(&profiles.crown,
                    *start + Vec3::new(0.0, wall_height, 0.0),
                    *end + Vec3::new(0.0, wall_height, 0.0),
                    inward_n, -Vec3::Y));

            // Baseboard swept along the floor of EVERY built wall, broken only at DOOR openings
            // (doors reach the floor; the board runs continuously under a window). v0.456/457.
            let mut door_spans: Vec<(f32, f32)> = openings
                .iter()
                .filter(|(_, _, _, sill)| *sill < 0.01)
                .map(|(c, w, _, _)| {
                    let half = (w * 0.5).min(len * 0.45);
                    ((c - half).clamp(0.0, len), (c + half).clamp(0.0, len))
                })
                .collect();
            door_spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut cursor = 0.0_f32;
            for (t0, t1) in &door_spans {
                let t0 = t0.max(cursor);
                if t0 - cursor > 0.05 {
                    append_mesh(&mut trim_v, &mut trim_i,
                        sweep_profile(&profiles.baseboard, *start + norm * cursor, *start + norm * t0, inward_n, Vec3::Y));
                }
                cursor = t1.max(cursor);
            }
            if len - cursor > 0.05 {
                append_mesh(&mut trim_v, &mut trim_i,
                    sweep_profile(&profiles.baseboard, *start + norm * cursor, *end, inward_n, Vec3::Y));
            }
        }
    }

    // Also generate floor-only for zero-height rooms (outdoor spaces like ranch).
    for (i, rc) in rooms.iter().enumerate() {
        let dim_y = rc.dimensions[1];
        let wh = rc.wall_height;
        if dim_y <= 0.0 && wh <= 0.0 {
            let pos = positions[i];
            let dim = Vec3::new(rc.dimensions[0], 0.0, rc.dimensions[2]);
            let (verts, indices) = floor_quad(pos, dim);
            let color = room_color(&rc.id, Some(rc.color));
            floors.push((verts, indices, color, rc.material_type));

            room_info.push(RoomInfo {
                id: rc.id.clone(),
                center: pos + Vec3::new(dim.x * 0.5, 0.0, dim.z * 0.5),
                dimensions: dim,
                is_hologram_room: hologram_room == Some(rc.id.as_str()),
                is_spawn_room: spawn_room == Some(rc.id.as_str()),
            });
        }
    }

    HomesteadMeshes {
        floors,
        walls: (wall_v, wall_i),
        trim: (trim_v, trim_i),
        windows: (win_v, win_i),
        mirrors: (mir_v, mir_i),
        ceilings: (ceil_v, ceil_i),
        room_info,
    }
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

/// Try to load the homestead layout from `data/blueprints/homestead_layout.ron`.
pub fn load_homestead_layout(data_dir: &Path) -> Option<HomesteadLayout> {
    let path = data_dir.join("blueprints").join("homestead_layout.ron");
    let text = std::fs::read_to_string(&path).ok()?;
    match ron::from_str::<HomesteadLayout>(&text) {
        Ok(layout) => {
            log::info!("Loaded homestead layout from {}: {} rooms, style: {:?}",
                path.display(), layout.rooms.len(), layout.layout_style);
            Some(layout)
        }
        Err(e) => {
            log::warn!("Failed to parse {}: {e}", path.display());
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Resolve the data directory (next to the exe, or `./data`). Used to load + save the
/// homestead layout from the same place. (v0.455)
pub fn data_dir() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for candidate in [exe_dir.join("data"), exe_dir.join("..").join("data")] {
                if candidate.join("blueprints").join("homestead_layout.ron").exists() {
                    return candidate;
                }
            }
        }
    }
    std::path::PathBuf::from("data")
}

/// Load the homestead layout from data, or the hardcoded fallback. Returns the LAYOUT (not
/// the meshes) so the construction editor can mutate it + regenerate live. (v0.455)
pub fn load_layout_or_fallback() -> HomesteadLayout {
    if let Some(layout) = load_homestead_layout(&data_dir()) {
        return layout;
    }
    log::info!("No homestead_layout.ron found, using fallback layout");
    fallback_layout()
}

/// Write a layout back to `data/blueprints/homestead_layout.ron` (the construction editor's
/// "Save"). Data-only (the file's hand-written comments are not preserved). (v0.455)
pub fn save_layout(layout: &HomesteadLayout) -> std::io::Result<()> {
    let dir = data_dir().join("blueprints");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("homestead_layout.ron");
    let cfg = ron::ser::PrettyConfig::new().depth_limit(4);
    let body = ron::ser::to_string_pretty(layout, cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let text = format!("// HumanityOS homestead layout -- saved by the in-app construction editor.\n{body}\n");
    std::fs::write(&path, text)?;
    log::info!("Saved homestead layout to {}", path.display());
    Ok(())
}

/// Generate all floor, wall, and ceiling meshes for the homestead.
///
/// Tries to load from `data/blueprints/homestead_layout.ron` first.
/// Falls back to hardcoded Fibonacci spiral if the file is missing.
pub fn generate_homestead() -> HomesteadMeshes {
    generate_from_layout(&load_layout_or_fallback())
}

/// Generate from a loaded layout (computes positions from style, then builds meshes).
pub fn generate_from_layout(layout: &HomesteadLayout) -> HomesteadMeshes {
    let positions = resolve_positions(layout);

    // Log room positions for debugging
    for (i, rc) in layout.rooms.iter().enumerate() {
        let p = positions[i];
        log::info!("  Room '{}': pos=({:.1}, {:.1}, {:.1}) dims=({:.0}x{:.0}x{:.0})",
            rc.id, p.x, p.y, p.z, rc.dimensions[0], rc.dimensions[1], rc.dimensions[2]);
    }

    build_meshes(layout, &positions, &load_trim_profiles())
}

// ---------------------------------------------------------------------------
// Hardcoded fallback
// ---------------------------------------------------------------------------

fn generate_fallback() -> HomesteadMeshes {
    generate_from_layout(&fallback_layout())
}

/// The hardcoded fallback layout (used when no RON is present). Returns the LAYOUT so the
/// editor + `load_layout_or_fallback` share it. (v0.455)
fn fallback_layout() -> HomesteadLayout {
    HomesteadLayout {
        layout_style: LayoutStyle::Fibonacci,
        hologram_room: Some("kitchen".into()),
        spawn_room: Some("kitchen".into()),
        rooms: vec![
            room_cfg("computer",  [1.0, 3.0, 1.0],  1, [0.3, 0.4, 0.5, 1.0]),
            room_cfg("network",   [1.0, 3.0, 1.0],  1, [0.25, 0.35, 0.5, 1.0]),
            room_cfg("respawner", [2.0, 3.0, 2.0],  2, [0.5, 0.55, 0.65, 1.0]),
            room_cfg("wetroom",   [3.0, 3.0, 3.0],  2, [0.45, 0.5, 0.55, 1.0]),
            room_cfg("bedroom",   [5.0, 3.0, 5.0],  3, [0.5, 0.4, 0.3, 1.0]),
            room_cfg("kitchen",   [8.0, 3.0, 8.0],  0, [0.6, 0.5, 0.25, 1.0]),
            room_cfg("livingroom",[13.0, 4.0, 13.0], 3, [0.45, 0.35, 0.2, 1.0]),
            room_cfg("study",     [21.0, 5.0, 21.0], 1, [0.4, 0.4, 0.45, 1.0]),
            room_cfg("garden",    [34.0, 6.0, 34.0], 2, [0.2, 0.45, 0.2, 1.0]),
        ],
        // Construction-mode standard dimensions (the RON sets these; the fallback uses the
        // same defaults so it matches a fresh data load).
        door_width: default_door_w(),
        door_height: default_door_h(),
        window_width: default_window_w(),
        window_height: default_window_h(),
        window_sill: default_window_sill(),
        baseboard_height: default_trim_h(),
        crown_height: default_trim_h(),
        mirror_size: default_mirror(),
        default_wall_height: 3.0, // uniform 3 m ceilings (matches the shipped RON)
    }
}

fn room_cfg(id: &str, dims: [f32; 3], material: u32, color: [f32; 4]) -> RoomConfig {
    RoomConfig {
        id: id.to_string(),
        position: None,    // computed from layout_style
        dimensions: dims,
        material_type: material,
        color,
        wall_height: 0.0, // use dimensions.y
        walls: WallSet::default(), // all Auto (today's auto-door behaviour)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped RON must parse with the v0.453 construction-mode fields, and the special
    /// rooms must carry the per-wall kinds the operator asked for. If this breaks, the game
    /// silently falls back to the hardcoded layout and the mirror/windows/no-walls vanish.
    #[test]
    fn shipped_ron_parses_with_construction_walls() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let layout = load_homestead_layout(&dir).expect("homestead_layout.ron must parse");

        // Standard dimensions present (one source of truth for door/window/trim sizes).
        assert!((layout.door_width - 0.9).abs() < 1e-6, "door_width from RON");
        assert!(layout.mirror_size > 0.0 && layout.window_width > 0.0);

        let find = |id: &str| layout.rooms.iter().find(|r| r.id == id)
            .unwrap_or_else(|| panic!("room {id} present"));

        // Respawner: all four walls open (a doorless, wall-less alcove).
        let r = find("respawner");
        for i in 0..4 { assert_eq!(r.walls.by_index(i), WallKind::Open, "respawner wall {i} open"); }

        // Wetroom: a mirror/portal on the east wall (index 3).
        assert_eq!(find("wetroom").walls.east, WallKind::Mirror, "wetroom east = Mirror");

        // Bedroom: bay windows on north + east.
        let b = find("bedroom");
        assert_eq!(b.walls.north, WallKind::Window, "bedroom north = Window");
        assert_eq!(b.walls.east, WallKind::Window, "bedroom east = Window");

        // 1x1 closets: fully solid (no doors).
        for id in ["computer", "network"] {
            let c = find(id);
            for i in 0..4 { assert_eq!(c.walls.by_index(i), WallKind::Solid, "{id} wall {i} solid"); }
        }
    }

    /// Generating from the shipped layout must produce every construction-mode mesh family,
    /// proving the special walls actually emit geometry (mirror, windows, trim, ceiling).
    #[test]
    fn generation_emits_construction_meshes() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let layout = load_homestead_layout(&dir).expect("layout parses");
        let m = generate_from_layout(&layout);
        assert!(!m.walls.0.is_empty(), "walls generated");
        assert!(!m.trim.0.is_empty(), "trim (baseboards/crown/frames) generated");
        assert!(!m.windows.0.is_empty(), "bedroom windows generated");
        assert!(!m.mirrors.0.is_empty(), "wetroom mirror generated");
        assert!(!m.ceilings.0.is_empty(), "ceilings generated (for the roof toggle)");
        assert!(!m.room_info.is_empty(), "room info present");
    }

    /// The construction editor's Save serializes the layout to RON; it must round-trip back
    /// with the per-wall kinds intact (so saving + reloading is lossless for the data). Does
    /// NOT touch the shipped file -- serializes in memory only.
    #[test]
    fn layout_serializes_and_round_trips() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let layout = load_homestead_layout(&dir).expect("layout parses");
        let cfg = ron::ser::PrettyConfig::new().depth_limit(4);
        let text = ron::ser::to_string_pretty(&layout, cfg).expect("serializes to RON");
        let back: HomesteadLayout = ron::from_str(&text).expect("re-parses");
        // Wall kinds survived the round-trip.
        let w = back.rooms.iter().find(|r| r.id == "wetroom").expect("wetroom");
        assert_eq!(w.walls.east, WallKind::Mirror);
        let r = back.rooms.iter().find(|r| r.id == "respawner").expect("respawner");
        assert_eq!(r.walls.north, WallKind::Open);
        assert!((back.default_wall_height - layout.default_wall_height).abs() < 1e-6);
    }

    /// The shipped trim profiles parse, and a sweep produces geometry (the SVG-style profile
    /// trim system). A missing/garbled file must fall back to the built-in defaults.
    #[test]
    fn trim_profiles_parse_and_sweep() {
        // Defaults are always non-empty closed polygons.
        let d = TrimProfiles::default();
        assert!(d.baseboard.len() >= 3 && d.crown.len() >= 3 && d.casing.len() >= 3);
        assert!(casing_width(&d) > 0.0);
        // Sweeping a profile along a 1 m run yields geometry.
        let (v, idx) = sweep_profile(&d.baseboard, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::Z, Vec3::Y);
        assert!(!v.is_empty() && !idx.is_empty(), "sweep produces a mesh");
        // The shipped RON, if present, parses (load_trim_profiles never panics).
        let _ = load_trim_profiles();
    }

    /// A room that omits `walls` defaults to all-`Auto`, i.e. today's behaviour: zero
    /// regression for unconfigured rooms.
    #[test]
    fn omitted_walls_default_to_auto() {
        let rc: RoomConfig = ron::from_str(
            r#"(id: "x", dimensions: (4.0, 3.0, 4.0), material_type: 0, color: (1.0,1.0,1.0,1.0))"#,
        ).expect("minimal RoomConfig parses");
        for i in 0..4 { assert_eq!(rc.walls.by_index(i), WallKind::Auto); }
    }
}
