//! Fibonacci homestead generator.
//!
//! Generates floor planes and walls for each room in the ship layout.
//! Rooms are colored by type. Walls are light grey dividers.
//!
//! Layout can be loaded from `data/blueprints/homestead_layout.ron` (data-driven)
//! or falls back to hardcoded positions when the file is absent.

use crate::renderer::mesh::Vertex;
use glam::Vec3;
use std::path::Path;

// ---------------------------------------------------------------------------
// Data-driven layout structs (loadable from RON)
// ---------------------------------------------------------------------------

/// Room layout entry from data file.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RoomConfig {
    pub id: String,
    pub position: [f32; 3],      // x, y, z in meters
    pub dimensions: [f32; 3],    // width, height, depth
    pub material_type: u32,      // PBR material type (0=grid, 1=metal, 2=concrete, 3=wood)
    pub color: [f32; 4],         // RGBA base color for PBR
    #[serde(default)]
    pub wall_height: f32,        // override wall height (0 = use dimensions.y)
}

/// Full homestead layout loaded from data/
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HomesteadLayout {
    pub rooms: Vec<RoomConfig>,
    #[serde(default)]
    pub hologram_room: Option<String>, // which room_id gets the hologram
    #[serde(default)]
    pub spawn_room: Option<String>,    // which room_id the player spawns in
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
// Legacy room definition (used by fallback path)
// ---------------------------------------------------------------------------

/// Room definition parsed from layout RON (simplified).
pub struct HomesteadRoom {
    pub id: String,
    pub position: Vec3,   // meters, local to ship origin
    pub dimensions: Vec3, // width (x), height (y), depth (z) in meters
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
    /// Room metadata for features (id, center_position, dimensions)
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
        "computer"    => [0.3, 0.4, 0.5, 1.0],   // blue-grey
        "network"     => [0.25, 0.35, 0.5, 1.0],  // darker blue-grey
        "respawner"   => [0.5, 0.55, 0.6, 1.0],   // white-blue
        "wetroom"     => [0.45, 0.5, 0.55, 1.0],  // light blue-grey
        "bathroom"    => [0.5, 0.55, 0.6, 1.0],   // white-blue (legacy alias)
        "bedroom"     => [0.5, 0.4, 0.3, 1.0],    // warm beige
        "kitchen"     => [0.6, 0.5, 0.25, 1.0],   // warm yellow
        "livingroom"  => [0.45, 0.35, 0.2, 1.0],  // warm wood
        "living_room" => [0.45, 0.35, 0.2, 1.0],  // legacy alias
        "study"       => [0.4, 0.4, 0.45, 1.0],   // cool grey
        "laboratory"  => [0.5, 0.5, 0.55, 1.0],   // cool white (legacy alias)
        "garden"      => [0.2, 0.45, 0.2, 1.0],   // green
        "garage"      => [0.35, 0.35, 0.35, 1.0], // industrial grey
        "depot"       => [0.25, 0.25, 0.25, 1.0], // dark grey
        "hangar"      => [0.55, 0.55, 0.6, 1.0],  // silver
        "ranch"       => [0.25, 0.5, 0.2, 1.0],   // green
        "workshop"    => [0.35, 0.35, 0.35, 1.0],  // industrial grey (legacy alias)
        _             => [0.3, 0.3, 0.3, 1.0],    // default grey
    }
}

// ---------------------------------------------------------------------------
// Mesh generation helpers
// ---------------------------------------------------------------------------

/// Generate floor quad vertices for a room.
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
    // CCW from above so front face points UP toward viewer standing on floor
    let indices = vec![0, 2, 1, 0, 3, 2];
    (vertices, indices)
}

/// Generate a wall box (configurable height, 0.1m thick).
fn wall_box(start: Vec3, end: Vec3, y_base: f32, height: f32, thickness: f32) -> (Vec<Vertex>, Vec<u32>) {
    let dir = (end - start).normalize();
    let perp = Vec3::new(-dir.z, 0.0, dir.x) * (thickness / 2.0);

    let p0 = start - perp;
    let p1 = start + perp;
    let p2 = end + perp;
    let p3 = end - perp;

    let y0 = y_base;
    let y1 = y_base + height;

    // 8 vertices (4 bottom, 4 top)
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
        // Front face (along dir)
        Vertex { position: [p1.x, y0, p1.z], normal: [perp.x, 0.0, perp.z], uv: [0.0, 0.0] },
        Vertex { position: [p2.x, y0, p2.z], normal: [perp.x, 0.0, perp.z], uv: [1.0, 0.0] },
        Vertex { position: [p2.x, y1, p2.z], normal: [perp.x, 0.0, perp.z], uv: [1.0, 1.0] },
        Vertex { position: [p1.x, y1, p1.z], normal: [perp.x, 0.0, perp.z], uv: [0.0, 1.0] },
        // Back face
        Vertex { position: [p3.x, y0, p3.z], normal: [-perp.x, 0.0, -perp.z], uv: [0.0, 0.0] },
        Vertex { position: [p0.x, y0, p0.z], normal: [-perp.x, 0.0, -perp.z], uv: [1.0, 0.0] },
        Vertex { position: [p0.x, y1, p0.z], normal: [-perp.x, 0.0, -perp.z], uv: [1.0, 1.0] },
        Vertex { position: [p3.x, y1, p3.z], normal: [-perp.x, 0.0, -perp.z], uv: [0.0, 1.0] },
        // Left end face
        Vertex { position: [p0.x, y0, p0.z], normal: [-dir.x, 0.0, -dir.z], uv: [0.0, 0.0] },
        Vertex { position: [p1.x, y0, p1.z], normal: [-dir.x, 0.0, -dir.z], uv: [1.0, 0.0] },
        Vertex { position: [p1.x, y1, p1.z], normal: [-dir.x, 0.0, -dir.z], uv: [1.0, 1.0] },
        Vertex { position: [p0.x, y1, p0.z], normal: [-dir.x, 0.0, -dir.z], uv: [0.0, 1.0] },
        // Right end face
        Vertex { position: [p2.x, y0, p2.z], normal: [dir.x, 0.0, dir.z], uv: [0.0, 0.0] },
        Vertex { position: [p3.x, y0, p3.z], normal: [dir.x, 0.0, dir.z], uv: [1.0, 0.0] },
        Vertex { position: [p3.x, y1, p3.z], normal: [dir.x, 0.0, dir.z], uv: [1.0, 1.0] },
        Vertex { position: [p2.x, y1, p2.z], normal: [dir.x, 0.0, dir.z], uv: [0.0, 1.0] },
    ];

    let indices = vec![
        // Bottom
        0, 1, 2, 0, 2, 3,
        // Top
        4, 6, 5, 4, 7, 6,
        // Front
        8, 9, 10, 8, 10, 11,
        // Back
        12, 13, 14, 12, 14, 15,
        // Left end
        16, 17, 18, 16, 18, 19,
        // Right end
        20, 21, 22, 20, 22, 23,
    ];

    (vertices, indices)
}

// ---------------------------------------------------------------------------
// Data-driven generation
// ---------------------------------------------------------------------------

/// Try to load the homestead layout from `data/blueprints/homestead_layout.ron`.
pub fn load_homestead_layout(data_dir: &Path) -> Option<HomesteadLayout> {
    let path = data_dir.join("blueprints").join("homestead_layout.ron");
    let text = std::fs::read_to_string(&path).ok()?;
    match ron::from_str::<HomesteadLayout>(&text) {
        Ok(layout) => {
            log::info!("Loaded homestead layout from {}: {} rooms", path.display(), layout.rooms.len());
            Some(layout)
        }
        Err(e) => {
            log::warn!("Failed to parse {}: {e}", path.display());
            None
        }
    }
}

/// Generate all floor and wall meshes from a data-driven layout.
pub fn generate_homestead_from_layout(layout: &HomesteadLayout) -> HomesteadMeshes {
    let wall_thickness = 0.1;

    let mut floors = Vec::new();
    let mut all_wall_verts = Vec::new();
    let mut all_wall_indices = Vec::new();
    let mut room_info = Vec::new();

    for rc in &layout.rooms {
        let pos = Vec3::new(rc.position[0], rc.position[1], rc.position[2]);
        let dim = Vec3::new(rc.dimensions[0], rc.dimensions[1], rc.dimensions[2]);
        let wall_height = if rc.wall_height > 0.0 { rc.wall_height } else { dim.y };

        // Floor
        let (verts, indices) = floor_quad(pos, dim);
        let color = room_color(&rc.id, Some(rc.color));
        floors.push((verts, indices, color, rc.material_type));

        // Room info
        room_info.push(RoomInfo {
            id: rc.id.clone(),
            center: pos + dim * 0.5,
            dimensions: dim,
            is_hologram_room: layout.hologram_room.as_deref() == Some(&rc.id),
            is_spawn_room: layout.spawn_room.as_deref() == Some(&rc.id),
        });

        // 4 walls along edges
        let x0 = pos.x;
        let z0 = pos.z;
        let x1 = x0 + dim.x;
        let z1 = z0 + dim.z;
        let y = pos.y;

        let walls = [
            (Vec3::new(x0, y, z0), Vec3::new(x1, y, z0)),
            (Vec3::new(x0, y, z1), Vec3::new(x1, y, z1)),
            (Vec3::new(x0, y, z0), Vec3::new(x0, y, z1)),
            (Vec3::new(x1, y, z0), Vec3::new(x1, y, z1)),
        ];

        for (start, end) in walls {
            let base_idx = all_wall_verts.len() as u32;
            let (wv, wi) = wall_box(start, end, y, wall_height, wall_thickness);
            all_wall_verts.extend(wv);
            all_wall_indices.extend(wi.iter().map(|i| i + base_idx));
        }

        // Ceiling quad at top of walls
        let ceil_y = y + wall_height;
        let ceil_verts = vec![
            Vertex { position: [x0, ceil_y, z0], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] },
            Vertex { position: [x1, ceil_y, z0], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [x1, ceil_y, z1], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [x0, ceil_y, z1], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0] },
        ];
        let ceil_base = all_wall_verts.len() as u32;
        all_wall_verts.extend(ceil_verts);
        // Visible from below (inside room looking up)
        all_wall_indices.extend([
            ceil_base, ceil_base + 1, ceil_base + 2,
            ceil_base, ceil_base + 2, ceil_base + 3,
        ]);
    }

    HomesteadMeshes {
        floors,
        walls: (all_wall_verts, all_wall_indices),
        room_info,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Generate all floor and wall meshes for the homestead layout.
///
/// Tries to load from `data/blueprints/homestead_layout.ron` first.
/// Falls back to hardcoded Fibonacci spiral positions if the file is missing.
pub fn generate_homestead() -> HomesteadMeshes {
    // Try data-driven path: look for data/ next to the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // Check exe-relative data/ first, then repo-root data/
            for candidate in [exe_dir.join("data"), exe_dir.join("..").join("data")] {
                if let Some(layout) = load_homestead_layout(&candidate) {
                    return generate_homestead_from_layout(&layout);
                }
            }
        }
    }
    // Also try CWD-relative data/
    if let Some(layout) = load_homestead_layout(Path::new("data")) {
        return generate_homestead_from_layout(&layout);
    }

    log::info!("No homestead_layout.ron found, using fallback rooms");
    generate_homestead_fallback()
}

// ---------------------------------------------------------------------------
// Fallback (hardcoded) path
// ---------------------------------------------------------------------------

/// Hardcoded fallback when no data file is available.
fn generate_homestead_fallback() -> HomesteadMeshes {
    let rooms = fallback_rooms();
    let wall_thickness = 0.1;

    let mut floors = Vec::new();
    let mut all_wall_verts = Vec::new();
    let mut all_wall_indices = Vec::new();
    let mut room_info = Vec::new();

    for room in &rooms {
        let wall_height = room.dimensions.y;

        // Floor
        let (verts, indices) = floor_quad(room.position, room.dimensions);
        let color = room_color(&room.id, None);
        // Fallback: material_type 0 for all rooms
        floors.push((verts, indices, color, 0u32));

        // Room info
        room_info.push(RoomInfo {
            id: room.id.clone(),
            center: room.position + room.dimensions * 0.5,
            dimensions: room.dimensions,
            is_hologram_room: room.id == "living_room" || room.id == "livingroom",
            is_spawn_room: room.id == "respawner" || room.id == "bathroom",
        });

        // 4 walls along edges
        let x0 = room.position.x;
        let z0 = room.position.z;
        let x1 = x0 + room.dimensions.x;
        let z1 = z0 + room.dimensions.z;
        let y = room.position.y;

        let walls = [
            (Vec3::new(x0, y, z0), Vec3::new(x1, y, z0)),
            (Vec3::new(x0, y, z1), Vec3::new(x1, y, z1)),
            (Vec3::new(x0, y, z0), Vec3::new(x0, y, z1)),
            (Vec3::new(x1, y, z0), Vec3::new(x1, y, z1)),
        ];

        for (start, end) in walls {
            let base_idx = all_wall_verts.len() as u32;
            let (wv, wi) = wall_box(start, end, y, wall_height, wall_thickness);
            all_wall_verts.extend(wv);
            all_wall_indices.extend(wi.iter().map(|i| i + base_idx));
        }

        // Ceiling quad at top of walls
        let ceil_y = y + wall_height;
        let ceil_verts = vec![
            Vertex { position: [x0, ceil_y, z0], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] },
            Vertex { position: [x1, ceil_y, z0], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [x1, ceil_y, z1], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [x0, ceil_y, z1], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0] },
        ];
        let ceil_base = all_wall_verts.len() as u32;
        all_wall_verts.extend(ceil_verts);
        // Visible from below (inside room looking up)
        all_wall_indices.extend([
            ceil_base, ceil_base + 1, ceil_base + 2,
            ceil_base, ceil_base + 2, ceil_base + 3,
        ]);
    }

    HomesteadMeshes {
        floors,
        walls: (all_wall_verts, all_wall_indices),
        room_info,
    }
}

/// Fibonacci spiral room layout (hardcoded fallback).
/// Rooms arranged in a golden spiral where each new room attaches
/// to the growing rectangle, alternating sides.
fn fallback_rooms() -> Vec<HomesteadRoom> {
    vec![
        // Center of spiral: two 1x1 rooms side by side
        HomesteadRoom { id: "computer".into(),    position: Vec3::new(0.0, 0.0, 0.0), dimensions: Vec3::new(1.0, 3.0, 1.0) },
        HomesteadRoom { id: "network".into(),     position: Vec3::new(1.0, 0.0, 0.0), dimensions: Vec3::new(1.0, 3.0, 1.0) },
        // F2: 2x2 below the two 1x1s
        HomesteadRoom { id: "bathroom".into(),    position: Vec3::new(0.0, 0.0, 1.0), dimensions: Vec3::new(2.0, 3.0, 2.0) },
        // F3: 3x3 to the right of the 2x2 + 1x1 stack
        HomesteadRoom { id: "bedroom".into(),     position: Vec3::new(2.0, 0.0, 0.0), dimensions: Vec3::new(3.0, 3.0, 3.0) },
        // F5: 5x5 above everything so far
        HomesteadRoom { id: "kitchen".into(),     position: Vec3::new(-3.0, 0.0, 0.0), dimensions: Vec3::new(5.0, 3.0, 5.0) },
        // F8: 8x8 to the left
        HomesteadRoom { id: "living_room".into(), position: Vec3::new(-3.0, 0.0, -8.0), dimensions: Vec3::new(8.0, 3.0, 8.0) },
        // F13: 13x13 below
        HomesteadRoom { id: "laboratory".into(),  position: Vec3::new(-3.0, 0.0, 5.0), dimensions: Vec3::new(13.0, 4.0, 13.0) },
        // F21: 21x21 to the right
        HomesteadRoom { id: "workshop".into(),    position: Vec3::new(10.0, 0.0, -8.0), dimensions: Vec3::new(21.0, 6.0, 21.0) },
        // F34: 34x34 above
        HomesteadRoom { id: "garden".into(),      position: Vec3::new(-24.0, 0.0, -8.0), dimensions: Vec3::new(34.0, 6.0, 34.0) },
    ]
}
