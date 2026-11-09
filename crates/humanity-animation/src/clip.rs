//! Clips: an imported animation sampled from its keys (`SourceClip`, on the rig it was made for)
//! and a clip baked onto a body (`Clip`, uniform frames on the body's skeleton, what plays).
//!
//! WHY BAKE: a clip made on another rig is retargeted by role once, at load, into frames at a fixed
//! rate on the body's own joints (`retarget`). Playing it is then two frame lookups and a blend per
//! joint, the "microseconds on the CPU" of the plan's section 7, whatever the source file's keys
//! looked like. The frames are made in memory from the files and never written anywhere (plan 8.7).

use std::collections::HashMap;

use glam::{Quat, Vec3};
use humanity_core::skin_data::{AnimClipData, ChannelPath, Interpolation};

use crate::pose::{nlerp, Pose};
use crate::Skeleton;

/// One channel's keys, read as glTF says (Linear, Step or CubicSpline).
#[derive(Debug, Clone, PartialEq)]
struct Curve {
    interpolation: Interpolation,
    times: Vec<f32>,
    values: Vec<f32>,
    /// Floats per value: 3 or 4.
    width: usize,
}

impl Curve {
    fn value(&self, key: usize, part: usize) -> &[f32] {
        // CubicSpline stores (in-tangent, value, out-tangent) per key.
        let per_key = if self.interpolation == Interpolation::CubicSpline { 3 } else { 1 };
        let at = (key * per_key + part) * self.width;
        &self.values[at..at + self.width]
    }

    fn point(&self, key: usize) -> &[f32] {
        self.value(key, if self.interpolation == Interpolation::CubicSpline { 1 } else { 0 })
    }

    /// The value at time `t` (clamped to the keys), written into `out` (width floats).
    fn sample(&self, t: f32, out: &mut [f32; 4], rotation: bool) {
        let n = self.times.len();
        let copy = |src: &[f32], out: &mut [f32; 4]| out[..src.len()].copy_from_slice(src);
        if n == 1 || t <= self.times[0] {
            copy(self.point(0), out);
            return;
        }
        if t >= self.times[n - 1] {
            copy(self.point(n - 1), out);
            return;
        }
        let k = match self.times.binary_search_by(|x| x.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Less)) {
            Ok(i) => i.min(n - 2),
            Err(i) => i - 1,
        };
        let (t0, t1) = (self.times[k], self.times[k + 1]);
        let dt = (t1 - t0).max(1e-9);
        let u = ((t - t0) / dt).clamp(0.0, 1.0);
        match self.interpolation {
            Interpolation::Step => copy(self.point(k), out),
            Interpolation::Linear => {
                let (a, b) = (self.point(k), self.point(k + 1));
                if rotation {
                    let q = nlerp(Quat::from_slice(a), Quat::from_slice(b), u);
                    out.copy_from_slice(&q.to_array());
                } else {
                    for i in 0..self.width {
                        out[i] = a[i] + (b[i] - a[i]) * u;
                    }
                }
            }
            Interpolation::CubicSpline => {
                // glTF 2.0, Appendix C: p(u) = (2u^3 - 3u^2 + 1) p0 + (u^3 - 2u^2 + u) m0
                //                            + (-2u^3 + 3u^2) p1 + (u^3 - u^2) m1, with the
                // tangents scaled by the key interval.
                let (p0, m0, p1, m1) = (self.point(k), self.value(k, 2), self.point(k + 1), self.value(k + 1, 0));
                let (u2, u3) = (u * u, u * u * u);
                let (h00, h10, h01, h11) = (2.0 * u3 - 3.0 * u2 + 1.0, u3 - 2.0 * u2 + u, -2.0 * u3 + 3.0 * u2, u3 - u2);
                for i in 0..self.width {
                    out[i] = h00 * p0[i] + h10 * m0[i] * dt + h01 * p1[i] + h11 * m1[i] * dt;
                }
                if rotation {
                    let q = Quat::from_slice(&out[..4]).normalize();
                    out.copy_from_slice(&q.to_array());
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct SourceTrack {
    translation: Option<Curve>,
    rotation: Option<Curve>,
    scale: Option<Curve>,
}

/// An imported clip on the rig it was made for: its channels bound to that rig's joints by name.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceClip {
    pub name: String,
    pub duration: f32,
    tracks: Vec<SourceTrack>,
}

impl SourceClip {
    /// Bind a file's animation to a skeleton. Channels on nodes that are not joints of the skin
    /// are left out (a rig's helper nodes); a channel whose keys and values disagree in count is
    /// an error.
    pub fn from_data(data: &AnimClipData, skel: &Skeleton) -> Result<Self, String> {
        let mut tracks = vec![SourceTrack::default(); skel.joint_count()];
        let mut duration: f32 = 0.0;
        for ch in &data.channels {
            let Some(j) = skel.names.iter().position(|n| *n == ch.node) else { continue };
            let width = if ch.path == ChannelPath::Rotation { 4 } else { 3 };
            let per_key = if ch.interpolation == Interpolation::CubicSpline { 3 } else { 1 };
            if ch.times.is_empty() || ch.values.len() != ch.times.len() * width * per_key {
                return Err(format!(
                    "clip {}: the {:?} channel of {} has {} keys and {} values",
                    data.name,
                    ch.path,
                    ch.node,
                    ch.times.len(),
                    ch.values.len()
                ));
            }
            duration = duration.max(*ch.times.last().expect("not empty"));
            let curve = Curve { interpolation: ch.interpolation, times: ch.times.clone(), values: ch.values.clone(), width };
            match ch.path {
                ChannelPath::Translation => tracks[j].translation = Some(curve),
                ChannelPath::Rotation => tracks[j].rotation = Some(curve),
                ChannelPath::Scale => tracks[j].scale = Some(curve),
            }
        }
        Ok(SourceClip { name: data.name.clone(), duration, tracks })
    }

    /// The pose at time `t` (clamped to the clip): each joint's channels where it has them, its
    /// rest transform where it has none.
    pub fn sample(&self, skel: &Skeleton, t: f32, out: &mut Pose) {
        out.joints.clear();
        out.joints.extend_from_slice(&skel.rest);
        let mut v = [0.0_f32; 4];
        for (j, track) in self.tracks.iter().enumerate() {
            let trs = &mut out.joints[j];
            if let Some(c) = &track.translation {
                c.sample(t, &mut v, false);
                trs.t = Vec3::new(v[0], v[1], v[2]);
            }
            if let Some(c) = &track.rotation {
                c.sample(t, &mut v, true);
                trs.r = Quat::from_array(v).normalize();
            }
            if let Some(c) = &track.scale {
                c.sample(t, &mut v, false);
                trs.s = Vec3::new(v[0], v[1], v[2]);
            }
        }
    }

    /// Does the joint have any channel at all?
    pub fn animates(&self, joint: usize) -> bool {
        self.tracks.get(joint).is_some_and(|t| t.translation.is_some() || t.rotation.is_some() || t.scale.is_some())
    }
}

/// A clip baked onto a body's skeleton: frames at a fixed rate, a rotation per joint per frame
/// (or none: the joint keeps its rest rotation) and a translation track for the joints whose
/// position moves (the hips' bob and sway), as an OFFSET from the joint's rest translation, so the
/// clip plays on the same body reshaped by its morph targets (`Skeleton::reshaped`), whose hips
/// stand elsewhere (2026-11-09).
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frames: usize,
    pub looped: bool,
    /// Metres a second the clip's gait covers at its own speed (0 for clips that stand still):
    /// the blend space plays it faster or slower to match the body's real speed.
    pub speed_mps: f32,
    pub rotations: Vec<Option<Vec<Quat>>>,
    pub translations: Vec<Option<Vec<Vec3>>>,
}

impl Clip {
    /// The pose at time `t`: looped clips wrap, others hold their last frame.
    pub fn sample(&self, skel: &Skeleton, t: f32, out: &mut Pose) {
        out.joints.clear();
        out.joints.extend_from_slice(&skel.rest);
        if self.frames == 0 {
            return;
        }
        let t = if self.looped && self.duration > 0.0 { t.rem_euclid(self.duration) } else { t.clamp(0.0, self.duration) };
        let f = t * self.fps;
        let last = self.frames - 1;
        let f0 = (f.floor() as usize).min(last);
        let f1 = if f0 == last { if self.looped { 0 } else { last } } else { f0 + 1 };
        let u = (f - f0 as f32).clamp(0.0, 1.0);
        for j in 0..out.joints.len().min(self.rotations.len()) {
            if let Some(r) = &self.rotations[j] {
                out.joints[j].r = nlerp(r[f0], r[f1], u);
            }
            if let Some(t) = self.translations.get(j).and_then(|t| t.as_ref()) {
                out.joints[j].t = skel.rest[j].t + t[f0].lerp(t[f1], u);
            }
        }
    }

    /// The pose at a fraction 0 to 1 of the clip (phase-synchronised blend spaces).
    pub fn sample_phase(&self, skel: &Skeleton, phase: f32, out: &mut Pose) {
        self.sample(skel, phase.rem_euclid(1.0) * self.duration, out)
    }
}

/// Every clip a body plays, by id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipSet {
    pub clips: Vec<Clip>,
    pub by_id: HashMap<String, usize>,
}

impl ClipSet {
    pub fn insert(&mut self, id: &str, clip: Clip) {
        self.by_id.insert(id.to_string(), self.clips.len());
        self.clips.push(clip);
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    pub fn get(&self, i: usize) -> &Clip {
        &self.clips[i]
    }
}
