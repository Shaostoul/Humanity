//! Room mesh generation — procedural box rooms with door openings.
//!
//! Generates vertex/index data for a room defined by a `RoomDef`. Each room
//! is an axis-aligned box with floor, ceiling, and four walls. Walls have
//! rectangular openings where doors are placed.

use crate::renderer::mesh::Vertex;
use crate::ship::layout::{Direction, RoomDef};

/// Door opening dimensions (meters). All doors are a uniform 2m x 2.5m.
const DOOR_WIDTH: f32 = 2.0;
const DOOR_HEIGHT: f32 = 2.5;

/// Generate room geometry as raw vertex + index arrays.
///
/// The room is centered at `room.position` with extents `room.size`.
/// Walls with doors get a rectangular opening cut into them.
/// UV.y encodes surface type: floor ~0.0, wall ~0.5, ceiling ~1.0
/// so a single material can tint by region.
pub fn generate_room_mesh(room: &RoomDef) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let half = room.size * 0.5;
    let pos = room.position;

    // --- Floor (Y-) ---
    {
        let y = pos.y - half.y;
        emit_quad(
            &mut vertices,
            &mut indices,
            glam::Vec3::new(pos.x - half.x, y, pos.z + half.z),
            glam::Vec3::new(pos.x + half.x, y, pos.z + half.z),
            glam::Vec3::new(pos.x + half.x, y, pos.z - half.z),
            glam::Vec3::new(pos.x - half.x, y, pos.z - half.z),
            glam::Vec3::new(0.0, 1.0, 0.0),
            0.05,
        );
    }

    // --- Ceiling (Y+) ---
    {
        let y = pos.y + half.y;
        emit_quad(
            &mut vertices,
            &mut indices,
            glam::Vec3::new(pos.x - half.x, y, pos.z - half.z),
            glam::Vec3::new(pos.x + half.x, y, pos.z - half.z),
            glam::Vec3::new(pos.x + half.x, y, pos.z + half.z),
            glam::Vec3::new(pos.x - half.x, y, pos.z + half.z),
            glam::Vec3::new(0.0, -1.0, 0.0),
            0.95,
        );
    }

    // --- Walls ---
    let wall_faces = [
        (WallFace::North, Direction::North),
        (WallFace::South, Direction::South),
        (WallFace::East, Direction::East),
        (WallFace::West, Direction::West),
    ];

    for (face, dir) in &wall_faces {
        let has_door = room.doors.iter().any(|d| d.direction == *dir);
        emit_wall(&mut vertices, &mut indices, pos, half, *face, has_door);
    }

    (vertices, indices)
}

#[derive(Clone, Copy)]
enum WallFace {
    North,
    South,
    East,
    West,
}

/// Emit a single wall face, optionally with a centered door opening.
fn emit_wall(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    pos: glam::Vec3,
    half: glam::Vec3,
    face: WallFace,
    has_door: bool,
) {
    let uv_y = 0.5;
    let (corners, normal) = wall_corners(pos, half, face);
    let [bl, br, tr, tl] = corners;

    if !has_door {
        emit_quad(vertices, indices, bl, br, tr, tl, normal, uv_y);
        return;
    }

    // Wall with door: split into 3 panels around a centered rectangular opening.
    // The door sits on the floor (bottom of wall) centered horizontally.
    let wall_w = (br - bl).length();
    let wall_h = (tl - bl).length();
    let door_half_w = (DOOR_WIDTH * 0.5).min(wall_w * 0.5);
    let door_h = DOOR_HEIGHT.min(wall_h);

    // Fractional positions
    let left_frac = 0.5 - door_half_w / wall_w;
    let right_frac = 0.5 + door_half_w / wall_w;
    let top_frac = door_h / wall_h;

    // Horizontal direction along wall (BL -> BR)
    let h_dir = (br - bl) / wall_w;
    // Vertical direction along wall (BL -> TL)
    let v_dir = (tl - bl) / wall_h;

    // Key points
    let door_bl = bl + h_dir * (wall_w * left_frac);
    let door_br = bl + h_dir * (wall_w * right_frac);
    let door_tl = door_bl + v_dir * door_h;
    let door_tr = door_br + v_dir * door_h;

    // Left strip: bl -> door_bl -> (door_bl projected to top) -> tl
    let left_top = tl;
    let left_top_right = bl + h_dir * (wall_w * left_frac) + v_dir * wall_h;
    emit_quad(vertices, indices, bl, door_bl, left_top_right, left_top, normal, uv_y);

    // Right strip: door_br -> br -> tr -> (door_br projected to top)
    let right_top_left = bl + h_dir * (wall_w * right_frac) + v_dir * wall_h;
    emit_quad(vertices, indices, door_br, br, tr, right_top_left, normal, uv_y);

    // Top strip above door: door_tl -> door_tr -> right_top_left -> left_top_right
    if top_frac < 1.0 {
        emit_quad(vertices, indices, door_tl, door_tr, right_top_left, left_top_right, normal, uv_y);
    }
}

/// Emit a single quad (4 vertices, 6 indices) with CCW winding.
fn emit_quad(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    bl: glam::Vec3,
    br: glam::Vec3,
    tr: glam::Vec3,
    tl: glam::Vec3,
    normal: glam::Vec3,
    uv_y: f32,
) {
    let base = vertices.len() as u32;
    vertices.push(Vertex { position: [bl.x, bl.y, bl.z], normal: [normal.x, normal.y, normal.z], uv: [0.0, uv_y] });
    vertices.push(Vertex { position: [br.x, br.y, br.z], normal: [normal.x, normal.y, normal.z], uv: [1.0, uv_y] });
    vertices.push(Vertex { position: [tr.x, tr.y, tr.z], normal: [normal.x, normal.y, normal.z], uv: [1.0, uv_y] });
    vertices.push(Vertex { position: [tl.x, tl.y, tl.z], normal: [normal.x, normal.y, normal.z], uv: [0.0, uv_y] });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Returns the 4 corners of a wall face (BL, BR, TR, TL viewed from inside)
/// and the inward-facing normal.
fn wall_corners(
    pos: glam::Vec3,
    half: glam::Vec3,
    face: WallFace,
) -> ([glam::Vec3; 4], glam::Vec3) {
    let y_bot = pos.y - half.y;
    let y_top = pos.y + half.y;

    match face {
        WallFace::North => {
            let z = pos.z - half.z;
            ([
                glam::Vec3::new(pos.x + half.x, y_bot, z),
                glam::Vec3::new(pos.x - half.x, y_bot, z),
                glam::Vec3::new(pos.x - half.x, y_top, z),
                glam::Vec3::new(pos.x + half.x, y_top, z),
            ], glam::Vec3::new(0.0, 0.0, 1.0))
        }
        WallFace::South => {
            let z = pos.z + half.z;
            ([
                glam::Vec3::new(pos.x - half.x, y_bot, z),
                glam::Vec3::new(pos.x + half.x, y_bot, z),
                glam::Vec3::new(pos.x + half.x, y_top, z),
                glam::Vec3::new(pos.x - half.x, y_top, z),
            ], glam::Vec3::new(0.0, 0.0, -1.0))
        }
        WallFace::East => {
            let x = pos.x + half.x;
            ([
                glam::Vec3::new(x, y_bot, pos.z - half.z),
                glam::Vec3::new(x, y_bot, pos.z + half.z),
                glam::Vec3::new(x, y_top, pos.z + half.z),
                glam::Vec3::new(x, y_top, pos.z - half.z),
            ], glam::Vec3::new(-1.0, 0.0, 0.0))
        }
        WallFace::West => {
            let x = pos.x - half.x;
            ([
                glam::Vec3::new(x, y_bot, pos.z + half.z),
                glam::Vec3::new(x, y_bot, pos.z - half.z),
                glam::Vec3::new(x, y_top, pos.z - half.z),
                glam::Vec3::new(x, y_top, pos.z + half.z),
            ], glam::Vec3::new(1.0, 0.0, 0.0))
        }
    }
}
