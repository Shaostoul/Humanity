//! Fibonacci homestead generator.
//!
//! Generates floor planes and 0.5m walls for each room in the ship layout.
//! Rooms are colored by type. Walls are light grey dividers.

use crate::renderer::mesh::{Mesh, Vertex};
use glam::Vec3;

/// Room definition parsed from layout RON (simplified).
pub struct HomesteadRoom {
    pub id: String,
    pub position: Vec3,   // meters, local to ship origin
    pub dimensions: Vec3, // width (x), height (y), depth (z) in meters
}

/// Color for each room type (RGBA).
fn room_color(id: &str) -> [f32; 4] {
    match id {
        "computer"    => [0.3, 0.4, 0.5, 1.0],  // blue-grey
        "network"     => [0.25, 0.35, 0.5, 1.0], // darker blue-grey
        "bathroom"    => [0.5, 0.55, 0.6, 1.0],  // white-blue
        "bedroom"     => [0.5, 0.4, 0.3, 1.0],   // warm beige
        "kitchen"     => [0.6, 0.5, 0.25, 1.0],  // warm yellow
        "living_room" => [0.45, 0.35, 0.2, 1.0], // warm wood
        "laboratory"  => [0.5, 0.5, 0.55, 1.0],  // cool white
        "workshop"    => [0.35, 0.35, 0.35, 1.0], // industrial grey
        "garden"      => [0.2, 0.45, 0.2, 1.0],  // green
        _             => [0.3, 0.3, 0.3, 1.0],   // default grey
    }
}

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
    // Double-sided floor: visible from both above and below
    let indices = vec![
        0, 1, 2, 0, 2, 3, // top face (CCW from above)
        0, 2, 1, 0, 3, 2, // bottom face (CCW from below)
    ];
    (vertices, indices)
}

/// Generate a thin wall box (0.5m tall, 0.1m thick).
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

/// Result of generating the homestead: meshes and their associated colors.
pub struct HomesteadMeshes {
    /// (vertices, indices, color) for each room floor
    pub floors: Vec<(Vec<Vertex>, Vec<u32>, [f32; 4])>,
    /// (vertices, indices) for all walls combined
    pub walls: (Vec<Vertex>, Vec<u32>),
}

/// Generate all floor and wall meshes for the homestead layout.
pub fn generate_homestead() -> HomesteadMeshes {
    let rooms = default_rooms();
    let wall_height = 0.5;
    let wall_thickness = 0.1;

    let mut floors = Vec::new();
    let mut all_wall_verts = Vec::new();
    let mut all_wall_indices = Vec::new();

    for room in &rooms {
        // Floor
        let (verts, indices) = floor_quad(room.position, room.dimensions);
        let color = room_color(&room.id);
        floors.push((verts, indices, color));

        // 4 walls along edges
        let x0 = room.position.x;
        let z0 = room.position.z;
        let x1 = x0 + room.dimensions.x;
        let z1 = z0 + room.dimensions.z;
        let y = room.position.y;

        let walls = [
            // North wall (along X at z0)
            (Vec3::new(x0, y, z0), Vec3::new(x1, y, z0)),
            // South wall (along X at z1)
            (Vec3::new(x0, y, z1), Vec3::new(x1, y, z1)),
            // West wall (along Z at x0)
            (Vec3::new(x0, y, z0), Vec3::new(x0, y, z1)),
            // East wall (along Z at x1)
            (Vec3::new(x1, y, z0), Vec3::new(x1, y, z1)),
        ];

        for (start, end) in walls {
            let base_idx = all_wall_verts.len() as u32;
            let (wv, wi) = wall_box(start, end, y, wall_height, wall_thickness);
            all_wall_verts.extend(wv);
            all_wall_indices.extend(wi.iter().map(|i| i + base_idx));
        }
    }

    HomesteadMeshes {
        floors,
        walls: (all_wall_verts, all_wall_indices),
    }
}

/// Fibonacci spiral room layout.
/// Each new room attaches to the growing golden rectangle, cycling
/// through directions: right, up, left, down (when viewed from above).
///
/// The Fibonacci sequence: 1, 1, 2, 3, 5, 8, 13, 21, 34
/// Each room is a square of that size (on X and Z axes).
fn default_rooms() -> Vec<HomesteadRoom> {
    // Build the spiral programmatically.
    // Start with two 1x1 rooms side by side, then spiral outward.
    let sizes: &[(f32, &str)] = &[
        (1.0, "computer"),
        (1.0, "network"),
        (2.0, "bathroom"),
        (3.0, "bedroom"),
        (5.0, "kitchen"),
        (8.0, "living_room"),
        (13.0, "laboratory"),
        (21.0, "workshop"),
        (34.0, "garden"),
    ];

    let mut rooms = Vec::new();

    // Track the bounding box of all placed rooms
    let mut min_x: f32 = 0.0;
    let mut max_x: f32 = 0.0;
    let mut min_z: f32 = 0.0;
    let mut max_z: f32 = 0.0;

    // Place first two 1x1 rooms side by side
    rooms.push(HomesteadRoom {
        id: sizes[0].1.into(),
        position: Vec3::new(0.0, 0.0, 0.0),
        dimensions: Vec3::new(1.0, 3.0, 1.0),
    });
    rooms.push(HomesteadRoom {
        id: sizes[1].1.into(),
        position: Vec3::new(1.0, 0.0, 0.0),
        dimensions: Vec3::new(1.0, 3.0, 1.0),
    });
    min_x = 0.0; max_x = 2.0;
    min_z = 0.0; max_z = 1.0;

    // Direction cycle: 0=below(+Z), 1=right(+X), 2=above(-Z), 3=left(-X)
    let mut direction = 0;

    for i in 2..sizes.len() {
        let size = sizes[i].0;
        let height = if size <= 5.0 { 3.0 } else if size <= 13.0 { 4.0 } else { 6.0 };

        let (px, pz) = match direction % 4 {
            0 => {
                // Below: place at min_x, below max_z
                let px = min_x;
                let pz = max_z;
                max_z = pz + size;
                if min_x + size > max_x { max_x = min_x + size; }
                (px, pz)
            }
            1 => {
                // Right: place at max_x, at min_z
                let px = max_x;
                let pz = max_z - size;
                max_x = px + size;
                if pz < min_z { min_z = pz; }
                (px, pz)
            }
            2 => {
                // Above: place at max_x - size, above min_z
                let px = max_x - size;
                let pz = min_z - size;
                min_z = pz;
                if px < min_x { min_x = px; }
                (px, pz)
            }
            3 => {
                // Left: place at left of min_x, at min_z
                let px = min_x - size;
                let pz = min_z;
                min_x = px;
                if min_z + size > max_z { max_z = min_z + size; }
                (px, pz)
            }
            _ => unreachable!(),
        };

        rooms.push(HomesteadRoom {
            id: sizes[i].1.into(),
            position: Vec3::new(px, 0.0, pz),
            dimensions: Vec3::new(size, height, size),
        });

        direction += 1;
    }

    rooms
}
