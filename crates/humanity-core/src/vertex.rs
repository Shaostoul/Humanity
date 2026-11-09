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

/// A vertex that bends with a skeleton (character animation rung 1, 2026-11-08,
/// docs/design/character-animation.md section 2.2): today's `Vertex` plus the four joints that
/// move it and how much each one counts, the glTF `JOINTS_0` and `WEIGHTS_0` attributes.
///
/// 48 bytes, every field 4-byte aligned, so the GPU reads it two ways from ONE buffer: as vertex
/// attributes (`Float32x3, Float32x3, Float32x2, Uint16x4, Unorm16x4`) for the vertex-shader
/// skinning of middle-distance bodies, and as twelve `u32` words per vertex from a storage buffer
/// for the compute skinning of near bodies (`assets/shaders/skinning.wgsl`). `joints` index the
/// skin's joint list (the pose palette's rows for that body); `weights` are unorm16 (65535 = 1.0)
/// and sum to 65535, which the glTF reader makes true whatever the file wrote.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u16; 4],
    pub weights: [u16; 4],
}

impl SkinnedVertex {
    /// Bytes per vertex: the stride both GPU paths read with.
    pub const BYTES: usize = 48;
}

#[cfg(test)]
mod tests {
    /// The two GPU paths read a skinned vertex as 48 bytes, 12 words: a field added or reordered
    /// here must change the vertex layout and the compute shader in the same commit.
    /// Seen red 2026-11-08 with `BYTES` written as 40 (the layout without the weights).
    #[test]
    fn a_skinned_vertex_is_the_48_bytes_both_gpu_paths_read() {
        assert_eq!(std::mem::size_of::<super::SkinnedVertex>(), super::SkinnedVertex::BYTES);
        assert_eq!(std::mem::size_of::<super::SkinnedVertex>() % 4, 0);
        assert_eq!(std::mem::offset_of!(super::SkinnedVertex, joints), 32);
        assert_eq!(std::mem::offset_of!(super::SkinnedVertex, weights), 40);
    }
}
