//! Skinned models and animation clips as plain imported data (character animation rung 1,
//! 2026-11-08, docs/design/character-animation.md section 2.2).
//!
//! WHY IT LIVES IN THE CORE: two crates meet over it and neither may reach the other. The glTF
//! reader (`assets::skin`, which the crate split files under the renderer) fills it from a file,
//! and the animation crate (`humanity-animation`, which uses only the core and which the relay
//! never compiles) builds skeletons and clips from it. Plain data, no GPU and no file reading:
//! the same shape whether the file was a MakeHuman body, a Quaternius clip library or the
//! procedural test figure (`scripts/gen-test-figure.js`).
//!
//! Matrices are column-major `[[f32; 4]; 4]` (glam's `Mat4::from_cols_array_2d` order, the glTF
//! order); rotations are quaternions `[x, y, z, w]`.

use crate::vertex::SkinnedVertex;

/// One joint of a skin: a node the file names, where it rests, and the inverse bind matrix that
/// takes a vertex from the model's bind space into the joint's own space.
#[derive(Debug, Clone, PartialEq)]
pub struct SkinJointData {
    /// The node's name in the file (a role map turns it into a humanoid role).
    pub name: String,
    /// The parent joint, an index into the same joint list; None for a root joint (its parent
    /// node, if any, is not a joint of the skin: see `outer`).
    pub parent: Option<usize>,
    /// The node's rest transform, as the file wrote it.
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// glTF `inverseBindMatrices[i]`; identity when the file gave none.
    pub inverse_bind: [[f32; 4]; 4],
    /// For a root joint whose parent node is NOT a joint (an "Armature" node, often carrying a
    /// rotation or Mixamo's 0.01 scale): the world matrix of that ancestor chain, so the joint's
    /// model-space matrix includes it. None means identity (the joint has no non-joint ancestor
    /// with a transform). Ignored for a joint with a joint parent.
    pub outer: Option<[[f32; 4]; 4]>,
}

/// One drawable part of a skinned model: the glTF primitive and the name of its material, which
/// the body's data file maps to a region (skin, hair, a cosmetic slot) for its tint.
#[derive(Debug, Clone, PartialEq)]
pub struct SkinnedPrimitiveData {
    pub material: String,
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
}

/// Which property an animation channel moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelPath {
    Translation,
    Rotation,
    Scale,
}

/// How values between two keys are found (glTF `sampler.interpolation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    Step,
    /// Hermite spline: each key holds an in-tangent, the value and an out-tangent, in that order.
    CubicSpline,
}

/// One animation channel: a node, the property it moves, and its keys.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimChannelData {
    /// The target node's name (matched to a joint by name).
    pub node: String,
    pub path: ChannelPath,
    pub interpolation: Interpolation,
    /// Key times, seconds, rising.
    pub times: Vec<f32>,
    /// The values, flat: 3 floats a key for translation and scale, 4 for a rotation; three times
    /// that for `CubicSpline` (in-tangent, value, out-tangent).
    pub values: Vec<f32>,
}

/// One named animation of a file.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimClipData {
    pub name: String,
    pub channels: Vec<AnimChannelData>,
}

/// Everything the animation system reads from one skinned glTF file: its skin's joints, the
/// primitives drawn with that skin, and every animation in the file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkinnedModelData {
    pub joints: Vec<SkinJointData>,
    pub primitives: Vec<SkinnedPrimitiveData>,
    pub clips: Vec<AnimClipData>,
}

impl SkinnedModelData {
    /// Vertices across every primitive.
    pub fn vertex_count(&self) -> usize {
        self.primitives.iter().map(|p| p.vertices.len()).sum()
    }
}

/// The identity matrix in the column-major form above.
pub const IDENTITY: [[f32; 4]; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
