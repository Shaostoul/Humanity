//! Poses: a transform per joint, blended, carried up the hierarchy into model space, and written
//! as the pose palette the GPU skins with (plan section 2.2).
//!
//! THE PALETTE: one 3x4 matrix per joint, 48 bytes, as three `vec4` rows (the matrix's first three
//! rows; the fourth of an affine matrix is always 0 0 0 1). Row r holds `(m[0][r], m[1][r], m[2][r],
//! m[3][r])` of the column-major matrix, so a point is moved by three dot products:
//! `p' = (dot(row0, (p, 1)), dot(row1, (p, 1)), dot(row2, (p, 1)))`. The compute skinning shader and
//! the vertex-shader skinning read it the same way (`assets/shaders/skinning.wgsl`,
//! `assets/shaders/skinning_vs.wgsl`).

use glam::{Mat4, Quat, Vec3, Vec4};

/// Rows of the palette per joint (three `vec4`, 48 bytes).
pub const PALETTE_ROWS_PER_JOINT: usize = 3;

/// A joint's local transform: translation, rotation, scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trs {
    pub t: Vec3,
    pub r: Quat,
    pub s: Vec3,
}

impl Trs {
    pub const IDENTITY: Trs = Trs { t: Vec3::ZERO, r: Quat::IDENTITY, s: Vec3::ONE };

    pub fn from_arrays(t: [f32; 3], r: [f32; 4], s: [f32; 3]) -> Self {
        Trs { t: Vec3::from(t), r: Quat::from_array(r).normalize(), s: Vec3::from(s) }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.s, self.r, self.t)
    }

    /// `a` to `b` by `w` (0 = a, 1 = b): translations and scales straight, rotations by nlerp along
    /// the shorter way round. Nlerp, not slerp: for the small angles between neighbouring frames and
    /// between clips of one blend space the two agree to well under a degree, at a fraction of the cost.
    pub fn lerp(a: &Trs, b: &Trs, w: f32) -> Trs {
        Trs { t: a.t.lerp(b.t, w), r: nlerp(a.r, b.r, w), s: a.s.lerp(b.s, w) }
    }
}

/// Normalised linear interpolation of two rotations along the shorter arc.
pub fn nlerp(a: Quat, b: Quat, w: f32) -> Quat {
    let b = if a.dot(b) < 0.0 { -b } else { b };
    let q = Quat::from_vec4(Vec4::from(a).lerp(Vec4::from(b), w));
    let len = q.length();
    if len > 1e-8 {
        q / len
    } else {
        a
    }
}

/// A transform per joint of one skeleton, in the skeleton's joint order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pose {
    pub joints: Vec<Trs>,
}

impl Pose {
    pub fn with_len(n: usize) -> Self {
        Pose { joints: vec![Trs::IDENTITY; n] }
    }
}

/// `out = a` blended toward `b` by `w`, joint by joint. `out` is resized to fit.
pub fn blend_into(a: &Pose, b: &Pose, w: f32, out: &mut Pose) {
    let n = a.joints.len().min(b.joints.len());
    out.joints.resize(n, Trs::IDENTITY);
    if w <= 0.0 {
        out.joints.copy_from_slice(&a.joints[..n]);
        return;
    }
    if w >= 1.0 {
        out.joints.copy_from_slice(&b.joints[..n]);
        return;
    }
    for i in 0..n {
        out.joints[i] = Trs::lerp(&a.joints[i], &b.joints[i], w);
    }
}

/// `a` moved toward `b` by `w`, in place (no allocation on the per-frame path).
pub fn blend_toward(a: &mut Pose, b: &Pose, w: f32) {
    let n = a.joints.len().min(b.joints.len());
    for i in 0..n {
        a.joints[i] = Trs::lerp(&a.joints[i], &b.joints[i], w);
    }
}

/// Each joint's model-space matrix: its parent's (or, for a root, the skeleton's `outer` matrix)
/// times its own local transform. `out` is resized to the joint count.
pub fn model_matrices(skel: &crate::Skeleton, pose: &Pose, out: &mut Vec<Mat4>) {
    let n = skel.joint_count();
    out.resize(n, Mat4::IDENTITY);
    for &j in &skel.order {
        let local = pose.joints.get(j).copied().unwrap_or(skel.rest[j]).matrix();
        out[j] = match skel.parents[j] {
            Some(p) => out[p] * local,
            None => skel.outer[j] * local,
        };
    }
}

/// The skinning matrix of each joint (model matrix times inverse bind), appended to `out` as
/// palette rows, three per joint (see the module notes).
pub fn palette_rows(skel: &crate::Skeleton, model: &[Mat4], out: &mut Vec<[f32; 4]>) {
    for (j, m) in model.iter().enumerate().take(skel.joint_count()) {
        out.extend_from_slice(&affine_rows(&(*m * skel.inverse_bind[j])));
    }
}

/// The top three rows of an affine matrix, as the palette stores them.
pub fn affine_rows(m: &Mat4) -> [[f32; 4]; 3] {
    let c = m.to_cols_array_2d();
    [
        [c[0][0], c[1][0], c[2][0], c[3][0]],
        [c[0][1], c[1][1], c[2][1], c[3][1]],
        [c[0][2], c[1][2], c[2][2], c[3][2]],
    ]
}

/// Moves a point by palette rows, the way both shaders do: the CPU twin the tests check the
/// palette with.
pub fn apply_rows(rows: &[[f32; 4]; 3], p: Vec3) -> Vec3 {
    let h = p.extend(1.0);
    Vec3::new(Vec4::from(rows[0]).dot(h), Vec4::from(rows[1]).dot(h), Vec4::from(rows[2]).dot(h))
}
