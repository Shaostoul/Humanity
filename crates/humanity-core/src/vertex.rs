//! The one mesh vertex: position, normal and UV, the shape every CPU-side mesh builder writes
//! (the ship's structure and hull, the terrain's patches and asteroids, the plant and tree
//! generators, the asset manager's glTF reader) and the renderer uploads as it is.
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was `renderer::mesh::Vertex`, so the ship and the terrain
//! reached up into the renderer for a plain struct of eight floats. A vertex is data; the GPU
//! side is the buffer layout that describes it to wgpu, which stays in the renderer
//! (`renderer::mesh::vertex_layout`). `bytemuck`'s `Pod` and `Zeroable` are a plain byte-casting
//! crate, no GPU: they let the renderer hand a slice of these to a buffer without a copy.

use bytemuck::{Pod, Zeroable};

/// Vertex with position, normal, and UV coordinates.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}
