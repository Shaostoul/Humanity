//! Temporal cloud accumulation (clouds phase 4).
//!
//! The operator's report after the physical-medium rewrite (v0.1158) was
//! "they look like static instead of clouds" - the per-pixel march at
//! physical extinction is heavy spatial noise, and no single frame can
//! afford the samples to resolve it. This module owns the CPU side of the
//! production answer (temporal accumulation, Horizon/Nubis-class), shaped
//! for this engine:
//!
//! - Two 1024x1024 RGBA16F OCTAHEDRAL maps (ping/pong), indexed by world
//!   DIRECTION. Rotation needs no reprojection; translation against
//!   km-distant clouds is sub-texel per frame.
//! - Each frame `run_octa_pass` re-marches every texel with the animated
//!   accumulation jitter and EMA-blends into the write map (the shader
//!   entries live in assets/shaders/pbr/45-cloud-temporal.wgsl).
//! - The composite is free: the freshly written map rides the cloud
//!   material's ALBEDO slot in the transparent loop (see the group-3
//!   override in render_celestial_onto), and the type-15 fragment samples
//!   it by direction instead of marching. No bind-group-layout changes
//!   anywhere - the v0.1029 every-create-site hazard never applies.
//!
//! Activation is per frame from lib.rs (`set_cloud_temporal`): the near-
//! slab camera branch (inside/under the deck, where grain is visible)
//! turns it on and flags the shader through params2.w (+4.0); orbit keeps
//! the direct march, whose grain is sub-pixel at that distance.

use super::{AlbedoBindGroup, Renderer};

/// Map resolution: 1024^2 covers the sky half of the octahedron at
/// ~0.18 deg/texel - a touch softer than the screen's ~0.035 deg/px,
/// which reads as gentle edge softness rather than blur, at ~30% of the
/// march cost of a full-res sky.
// 2048 since phase 8 (the edge-sharpness ceiling): at 1024 a Lambert
// texel subtended 0.22 deg = 3.4 screen pixels at the operator's FOV -
// a hard cap no tuning could lift, and the visible "grain" autocorrelated
// at exactly 1-2 map texels. The ground-occlusion cull (v0.1161) cut the
// pass to 1.9 ms, buying the 4x texel budget for ~0.11 deg/texel.
pub const CLOUD_OCTA_SIZE: u32 = 2048;

pub struct CloudTemporal {
    _textures: [wgpu::Texture; 2],
    pub views: [wgpu::TextureView; 2],
    /// Full group-3 bind groups (colour + shadow variants) with map[i]'s
    /// view in the albedo slot. groups[read] feeds the octa pass as
    /// history; groups[written] is the composite's albedo override.
    pub groups: [AlbedoBindGroup; 2],
    /// Index of the most recently WRITTEN map (interior mutability: the
    /// flip happens inside the &self render path).
    pub cur: std::cell::Cell<usize>,
}

impl Renderer {
    /// Turn the temporal cloud path on (Some(cloud material index)) or off
    /// for this frame, creating the maps on first use. Called by lib.rs
    /// right after the cloud material update each frame.
    pub fn set_cloud_temporal(&mut self, mat: Option<usize>) {
        self.cloud_temporal_mat = mat;
        if mat.is_some() && self.cloud_temporal.is_none() {
            let mk = |label: &str| {
                let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: CLOUD_OCTA_SIZE,
                        height: CLOUD_OCTA_SIZE,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                (tex, view)
            };
            let (t0, v0) = mk("Cloud Octa Map A");
            let (t1, v1) = mk("Cloud Octa Map B");
            let g0 = self.build_albedo_group_from_view(&v0, &self.albedo_sampler);
            let g1 = self.build_albedo_group_from_view(&v1, &self.albedo_sampler);
            self.cloud_temporal = Some(CloudTemporal {
                _textures: [t0, t1],
                views: [v0, v1],
                groups: [g0, g1],
                cur: std::cell::Cell::new(0),
            });
            log::info!(
                "Cloud temporal accumulation ON: {0}x{0} octa map pair",
                CLOUD_OCTA_SIZE
            );
        }
    }
}
