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
#[derive(Debug, Clone, serde::Deserialize, Default)]
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

/// Room definition for the layout. Positions can be explicit or computed from LayoutStyle.
#[derive(Debug, Clone, serde::Deserialize)]
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
}

/// Full homestead layout loaded from data/
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HomesteadLayout {
    #[serde(default)]
    pub layout_style: LayoutStyle,
    pub rooms: Vec<RoomConfig>,
    #[serde(default)]
    pub hologram_room: Option<String>,
    #[serde(default)]
    pub spawn_room: Option<String>,
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

// ---------------------------------------------------------------------------
// Mesh assembly from resolved room positions
// ---------------------------------------------------------------------------

/// Generate all floor, wall, and ceiling meshes from a layout with resolved positions.
fn build_meshes(
    rooms: &[RoomConfig],
    positions: &[Vec3],
    hologram_room: Option<&str>,
    spawn_room: Option<&str>,
) -> HomesteadMeshes {
    let wall_thickness = 0.1;

    let mut floors = Vec::new();
    let mut all_wall_verts = Vec::new();
    let mut all_wall_indices = Vec::new();
    let mut room_info = Vec::new();

    for (i, rc) in rooms.iter().enumerate() {
        let pos = positions[i];
        let dim = Vec3::new(rc.dimensions[0], rc.dimensions[1], rc.dimensions[2]);
        let wall_height = if rc.wall_height > 0.0 {
            rc.wall_height
        } else if dim.y > 0.0 {
            dim.y
        } else {
            continue; // Skip rooms with zero height (open-air like ranch)
        };

        // Floor
        let (verts, indices) = floor_quad(pos, dim);
        let color = room_color(&rc.id, Some(rc.color));
        floors.push((verts, indices, color, rc.material_type));

        // Room info
        room_info.push(RoomInfo {
            id: rc.id.clone(),
            center: pos + Vec3::new(dim.x * 0.5, wall_height * 0.5, dim.z * 0.5),
            dimensions: Vec3::new(dim.x, wall_height, dim.z),
            is_hologram_room: hologram_room == Some(rc.id.as_str()),
            is_spawn_room: spawn_room == Some(rc.id.as_str()),
        });

        // 4 walls
        let x0 = pos.x;
        let z0 = pos.z;
        let x1 = x0 + dim.x;
        let z1 = z0 + dim.z;
        let y = pos.y;

        let walls = [
            (Vec3::new(x0, y, z0), Vec3::new(x1, y, z0)), // North (min Z)
            (Vec3::new(x0, y, z1), Vec3::new(x1, y, z1)), // South (max Z)
            (Vec3::new(x0, y, z0), Vec3::new(x0, y, z1)), // West (min X)
            (Vec3::new(x1, y, z0), Vec3::new(x1, y, z1)), // East (max X)
        ];

        for (start, end) in walls {
            let base_idx = all_wall_verts.len() as u32;
            let (wv, wi) = wall_box(start, end, y, wall_height, wall_thickness);
            all_wall_verts.extend(wv);
            all_wall_indices.extend(wi.iter().map(|idx| idx + base_idx));
        }

        // Ceiling (visible from below, inside room looking up)
        let ceil_y = y + wall_height;
        let ceil_verts = vec![
            Vertex { position: [x0, ceil_y, z0], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] },
            Vertex { position: [x1, ceil_y, z0], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [x1, ceil_y, z1], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [x0, ceil_y, z1], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0] },
        ];
        let cb = all_wall_verts.len() as u32;
        all_wall_verts.extend(ceil_verts);
        all_wall_indices.extend([cb, cb + 2, cb + 1, cb, cb + 3, cb + 2]);
    }

    // Also generate floor-only for zero-height rooms (outdoor spaces like ranch)
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
        walls: (all_wall_verts, all_wall_indices),
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

/// Generate all floor, wall, and ceiling meshes for the homestead.
///
/// Tries to load from `data/blueprints/homestead_layout.ron` first.
/// Falls back to hardcoded Fibonacci spiral if the file is missing.
pub fn generate_homestead() -> HomesteadMeshes {
    // Try data-driven path
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for candidate in [exe_dir.join("data"), exe_dir.join("..").join("data")] {
                if let Some(layout) = load_homestead_layout(&candidate) {
                    return generate_from_layout(&layout);
                }
            }
        }
    }
    if let Some(layout) = load_homestead_layout(Path::new("data")) {
        return generate_from_layout(&layout);
    }

    log::info!("No homestead_layout.ron found, using fallback");
    generate_fallback()
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

    build_meshes(
        &layout.rooms,
        &positions,
        layout.hologram_room.as_deref(),
        layout.spawn_room.as_deref(),
    )
}

// ---------------------------------------------------------------------------
// Hardcoded fallback
// ---------------------------------------------------------------------------

fn generate_fallback() -> HomesteadMeshes {
    // Build a minimal layout matching the RON format, then use the same pipeline
    let layout = HomesteadLayout {
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
    };
    generate_from_layout(&layout)
}

fn room_cfg(id: &str, dims: [f32; 3], material: u32, color: [f32; 4]) -> RoomConfig {
    RoomConfig {
        id: id.to_string(),
        position: None,    // computed from layout_style
        dimensions: dims,
        material_type: material,
        color,
        wall_height: 0.0, // use dimensions.y
    }
}
