//! Flow field pathfinding — supports million-agent navigation.
//!
//! Grid parameters loaded from `config/flow_field.toml`.

use glam::Vec2;

/// A flow field for mass agent navigation on a 2D grid.
pub struct FlowField {
    pub width: u32,
    pub height: u32,
    pub cell_size: f32,
    /// Direction vectors per cell (flattened row-major).
    pub directions: Vec<Vec2>,
}

impl FlowField {
    pub fn new(width: u32, height: u32, cell_size: f32) -> Self {
        let count = (width * height) as usize;
        Self {
            width,
            height,
            cell_size,
            directions: vec![Vec2::ZERO; count],
        }
    }
}
