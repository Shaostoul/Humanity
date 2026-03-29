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
    let indices = vec![0, 2, 1, 0, 3, 2];
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

/// Default room layout matching layout_medium.ron.
fn default_rooms() -> Vec<HomesteadRoom> {
    vec![
        HomesteadRoom { id: "computer".into(),    position: Vec3::new(1.0, 0.0, 1.0),   dimensions: Vec3::new(1.0, 3.0, 1.0) },
        HomesteadRoom { id: "network".into(),     position: Vec3::new(3.0, 0.0, 1.0),   dimensions: Vec3::new(1.0, 3.0, 1.0) },
        HomesteadRoom { id: "bathroom".into(),    position: Vec3::new(5.0, 0.0, 1.0),   dimensions: Vec3::new(2.0, 3.0, 2.0) },
        HomesteadRoom { id: "bedroom".into(),     position: Vec3::new(8.0, 0.0, 1.0),   dimensions: Vec3::new(3.0, 3.0, 3.0) },
        HomesteadRoom { id: "kitchen".into(),     position: Vec3::new(12.0, 0.0, 1.0),  dimensions: Vec3::new(5.0, 3.0, 5.0) },
        HomesteadRoom { id: "living_room".into(), position: Vec3::new(18.0, 0.0, 1.0),  dimensions: Vec3::new(8.0, 3.0, 8.0) },
        HomesteadRoom { id: "laboratory".into(),  position: Vec3::new(27.0, 0.0, 1.0),  dimensions: Vec3::new(13.0, 4.0, 13.0) },
        HomesteadRoom { id: "workshop".into(),    position: Vec3::new(1.0, 0.0, 15.0),  dimensions: Vec3::new(21.0, 6.0, 21.0) },
        HomesteadRoom { id: "garden".into(),      position: Vec3::new(23.0, 0.0, 15.0), dimensions: Vec3::new(34.0, 6.0, 34.0) },
    ]
}
