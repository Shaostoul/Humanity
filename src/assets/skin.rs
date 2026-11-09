//! glTF skins: the skinned model reader (character animation rung 1, 2026-11-08;
//! docs/design/character-animation.md section 2.2).
//!
//! The rest of the asset manager reads the first primitive of the first mesh and nothing that moves
//! ("skins, morph targets and animations are not played", model-viewer.md). This reads what a
//! character needs, into the plain data of `humanity_core::skin_data` that the animation crate builds
//! skeletons and clips from:
//!
//! - the file's first skin: its joints in the skin's own order (the order `JOINTS_0` indexes and
//!   the pose palette is written in), each joint's parent joint, rest transform and inverse bind
//!   matrix (glTF 2.0: "Each skin contains a REQUIRED `joints` property that lists the indices of
//!   nodes used as joints and an OPTIONAL `inverseBindMatrices` property"), and, for a root joint
//!   under a non-joint node (an "Armature", Mixamo's 0.01 scale), that node chain's world matrix;
//! - EVERY primitive of every mesh node drawn with that skin, with `JOINTS_0` and `WEIGHTS_0` (four
//!   influences a vertex; the weights renormalised to sum exactly 65535 in unorm16, whatever the
//!   file wrote), and the name of its material, which the body's data file maps to a tint;
//! - every animation, channel by channel (translation, rotation, scale; morph-target weights are a
//!   later rung), with its interpolation.
//!
//! Plain CPU work, no device: tests and worker threads call it.

use std::path::Path;

use humanity_core::skin_data::{
    AnimChannelData, AnimClipData, ChannelPath, Interpolation, SkinJointData, SkinnedModelData, SkinnedPrimitiveData, IDENTITY,
};
use humanity_core::vertex::SkinnedVertex;

/// Read a skinned glTF (`.gltf` with its `.bin`, or `.glb`).
pub fn read_skinned_gltf(path: &Path) -> Result<SkinnedModelData, String> {
    let (doc, buffers, _images) = gltf::import(path).map_err(|e| format!("Failed to load glTF {}: {e}", path.display()))?;
    let what = path.display().to_string();
    let skin = doc.skins().next().ok_or_else(|| format!("{what}: the file has no skin"))?;
    let node_count = doc.nodes().len();
    let mut parent_of: Vec<Option<usize>> = vec![None; node_count];
    for n in doc.nodes() {
        for c in n.children() {
            parent_of[c.index()] = Some(n.index());
        }
    }
    let local: Vec<glam::Mat4> = doc.nodes().map(|n| glam::Mat4::from_cols_array_2d(&n.transform().matrix())).collect();
    let world_of = |mut node: usize| -> glam::Mat4 {
        let mut m = local[node];
        while let Some(p) = parent_of[node] {
            m = local[p] * m;
            node = p;
        }
        m
    };
    let joint_nodes: Vec<usize> = skin.joints().map(|n| n.index()).collect();
    let ibms: Vec<[[f32; 4]; 4]> = skin
        .reader(|b| Some(&buffers[b.index()].0))
        .read_inverse_bind_matrices()
        .map(|it| it.collect())
        .unwrap_or_else(|| vec![IDENTITY; joint_nodes.len()]);
    if ibms.len() != joint_nodes.len() {
        return Err(format!("{what}: the skin has {} joints but {} inverse bind matrices", joint_nodes.len(), ibms.len()));
    }
    let node_name = |i: usize| doc.nodes().nth(i).and_then(|n| n.name().map(str::to_string)).unwrap_or_else(|| format!("node{i}"));
    let joints: Vec<SkinJointData> = joint_nodes
        .iter()
        .enumerate()
        .map(|(ji, &node)| {
            let n = doc.nodes().nth(node).expect("a skin's joint is a node");
            let (t, r, s) = n.transform().decomposed();
            let parent_node = parent_of[node];
            let parent = parent_node.and_then(|p| joint_nodes.iter().position(|&j| j == p));
            let outer = match (parent, parent_node) {
                (None, Some(p)) => {
                    let w = world_of(p);
                    (w != glam::Mat4::IDENTITY).then(|| w.to_cols_array_2d())
                }
                _ => None,
            };
            SkinJointData { name: node_name(node), parent, translation: t, rotation: r, scale: s, inverse_bind: ibms[ji], outer }
        })
        .collect();

    let mut primitives = Vec::new();
    for node in doc.nodes() {
        if node.skin().map(|s| s.index()) != Some(skin.index()) {
            continue;
        }
        let Some(mesh) = node.mesh() else { continue };
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| Some(&buffers[b.index()].0));
            let positions: Vec<[f32; 3]> = reader.read_positions().ok_or_else(|| format!("{what}: a skinned primitive has no positions"))?.collect();
            let n = positions.len();
            let indices: Vec<u32> = match reader.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..n as u32).collect(),
            };
            let normals: Vec<[f32; 3]> = match reader.read_normals() {
                Some(it) => it.collect(),
                None => super::generate_flat_normals(&positions, &indices),
            };
            let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
                Some(it) => it.into_f32().collect(),
                None => super::generate_planar_uvs(&positions),
            };
            let joint_ids: Vec<[u16; 4]> = reader.read_joints(0).ok_or_else(|| format!("{what}: a skinned primitive has no JOINTS_0"))?.into_u16().collect();
            let weights: Vec<[f32; 4]> = reader.read_weights(0).ok_or_else(|| format!("{what}: a skinned primitive has no WEIGHTS_0"))?.into_f32().collect();
            if joint_ids.len() != n || weights.len() != n {
                return Err(format!("{what}: a primitive has {n} positions, {} JOINTS_0 and {} WEIGHTS_0", joint_ids.len(), weights.len()));
            }
            if let Some(bad) = joint_ids.iter().flatten().find(|&&j| j as usize >= joint_nodes.len()) {
                return Err(format!("{what}: a vertex names joint {bad} of a skin with {} joints", joint_nodes.len()));
            }
            let vertices = (0..n)
                .map(|i| SkinnedVertex {
                    position: positions[i],
                    normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
                    uv: uvs.get(i).copied().unwrap_or([0.0, 0.0]),
                    joints: joint_ids[i],
                    weights: unorm16_weights(weights[i]),
                })
                .collect();
            let material = prim.material().name().unwrap_or("default").to_string();
            primitives.push(SkinnedPrimitiveData { material, vertices, indices });
        }
    }

    let mut clips = Vec::new();
    for anim in doc.animations() {
        let mut channels = Vec::new();
        for ch in anim.channels() {
            let path = match ch.target().property() {
                gltf::animation::Property::Translation => ChannelPath::Translation,
                gltf::animation::Property::Rotation => ChannelPath::Rotation,
                gltf::animation::Property::Scale => ChannelPath::Scale,
                gltf::animation::Property::MorphTargetWeights => continue,
            };
            let interpolation = match ch.sampler().interpolation() {
                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                gltf::animation::Interpolation::Step => Interpolation::Step,
                gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
            };
            let reader = ch.reader(|b| Some(&buffers[b.index()].0));
            let times: Vec<f32> = reader.read_inputs().ok_or_else(|| format!("{what}: an animation channel has no key times"))?.collect();
            let values: Vec<f32> = match reader.read_outputs().ok_or_else(|| format!("{what}: an animation channel has no values"))? {
                gltf::animation::util::ReadOutputs::Translations(it) => it.flatten().collect(),
                gltf::animation::util::ReadOutputs::Scales(it) => it.flatten().collect(),
                gltf::animation::util::ReadOutputs::Rotations(r) => r.into_f32().flatten().collect(),
                gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => continue,
            };
            channels.push(AnimChannelData { node: node_name(ch.target().node().index()), path, interpolation, times, values });
        }
        clips.push(AnimClipData { name: anim.name().unwrap_or("animation").to_string(), channels });
    }
    Ok(SkinnedModelData { joints, primitives, clips })
}

/// Four weights as unorm16 summing to exactly 65535: negatives dropped, the rest scaled, and the
/// rounding left over given to the largest. A vertex with no weight at all is given wholly to its
/// first joint rather than to nothing (which would put it at the model's origin).
pub fn unorm16_weights(w: [f32; 4]) -> [u16; 4] {
    let w = w.map(|x| if x.is_finite() && x > 0.0 { x } else { 0.0 });
    let sum: f32 = w.iter().sum();
    if sum <= 1e-8 {
        return [u16::MAX, 0, 0, 0];
    }
    let mut out = w.map(|x| ((x / sum) * 65535.0).round() as i64);
    let total: i64 = out.iter().sum();
    let largest = (0..4).max_by(|&a, &b| w[a].total_cmp(&w[b])).unwrap_or(0);
    out[largest] += 65535 - total;
    out.map(|x| x.clamp(0, 65535) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(rel: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(rel)
    }

    /// The committed test figure reads with its whole skin: 24 joints in the skin's order (the twist
    /// joints included), all six materials as primitives, every vertex's joints inside the skin and
    /// its weights summing to exactly 65535, and its own reference walk.
    /// Seen red 2026-11-08 with the reader taking only the first primitive (one material, not six).
    #[test]
    fn the_test_figure_reads_with_every_primitive_joint_and_weight() {
        let m = read_skinned_gltf(&repo("assets/models/characters/hos_test_figure/hos_test_figure.gltf")).expect("the test figure reads");
        assert_eq!(m.joints.len(), 24);
        assert!(m.joints.iter().any(|j| j.name == "forearm_twist.L"));
        let roots: Vec<&str> = m.joints.iter().filter(|j| j.parent.is_none()).map(|j| j.name.as_str()).collect();
        assert_eq!(roots, vec!["pelvis"]);
        let mut mats: Vec<&str> = m.primitives.iter().map(|p| p.material.as_str()).collect();
        mats.sort();
        assert_eq!(mats, vec!["hair", "hands", "shirt", "shoes", "skin", "trousers"]);
        for p in &m.primitives {
            assert!(p.indices.iter().all(|&i| (i as usize) < p.vertices.len()));
            for v in &p.vertices {
                assert_eq!(v.weights.iter().map(|&w| w as u32).sum::<u32>(), 65535, "{v:?}");
                assert!(v.joints.iter().all(|&j| (j as usize) < m.joints.len()));
            }
        }
        assert!(m.clips.iter().any(|c| c.name == "ref_walk"));
    }

    /// The clip library's joints hang under a 0.01 "Armature" node (Mixamo's export): the root joint
    /// carries that node's matrix as `outer`, and all five clips read with rotation channels.
    /// Seen red 2026-11-08 with `outer` left None for a non-joint parent (the library's figure
    /// would stand 100 times too big).
    #[test]
    fn the_clip_librarys_armature_scale_and_clips_are_read() {
        let m = read_skinned_gltf(&repo("assets/models/characters/hos_test_clips/hos_test_clips.gltf")).expect("the clip library reads");
        let hips = m.joints.iter().find(|j| j.name == "mixamorig:Hips").expect("hips");
        let outer = glam::Mat4::from_cols_array_2d(&hips.outer.expect("the armature's matrix"));
        assert!((outer.x_axis.length() - 0.01).abs() < 1e-6, "{outer:?}");
        let mut names: Vec<&str> = m.clips.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["idle", "run", "turn_left", "turn_right", "walk"]);
        let walk = m.clips.iter().find(|c| c.name == "walk").unwrap();
        assert!(walk.channels.iter().filter(|c| c.path == ChannelPath::Rotation).count() >= 20);
    }

    /// Weights from any file sum to 65535 exactly, negatives and NaN dropped, an all-zero vertex
    /// given to its first joint.
    /// Seen red 2026-11-08 with the rounding remainder not handed to the largest weight
    /// (0.1/0.2/0.7 came to 65536).
    #[test]
    fn weights_always_sum_to_one_in_unorm16() {
        for w in [[0.1, 0.2, 0.7, 0.0], [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 0.0], [0.25; 4], [2.0, -1.0, f32::NAN, 2.0], [0.0; 4]] {
            let u = unorm16_weights(w);
            assert_eq!(u.iter().map(|&x| x as u32).sum::<u32>(), 65535, "{w:?} -> {u:?}");
        }
        assert_eq!(unorm16_weights([0.0; 4]), [65535, 0, 0, 0]);
        assert_eq!(unorm16_weights([2.0, -1.0, f32::NAN, 2.0])[1], 0);
    }
}
