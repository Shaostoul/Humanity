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
//! Activation is per frame from lib.rs (`set_cloud_temporal`): armed at
//! EVERY altitude since 12c (the extent-parametrized map concentrates
//! its texels on whatever the camera can see, so orbit gets a sharper-
//! than-screen map instead of per-pixel march static). The params2.w
//! (+4.0) flag tells the shell fragment to get out of the way; Low
//! quality keeps the direct march.

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
// 4096 in the brute-force wave (operator, v0.1187: "prep for brute force
// improvements... can you do everything in tandem?"): another 4x texels
// (~0.055 deg/texel, sub-screen-pixel at every altitude) WITHOUT 4x
// march cost - the octa pass marches a quarter of the texels per frame
// (2x2 cadence, see 45-cloud-temporal.wgsl), so per-frame march count
// equals the old full-rate 2048 map. RGBA16F ping-pong = 2 x 134 MB,
// budgeted for the 12 GB tier. The march FOOTPRINT deliberately stays at
// the 2048-map angular size (see cloud_pix_ang_map in 40-clouds.wgsl):
// the extra texels spatially supersample the same band-limited field,
// so per-ray march cost does not rise with the resolution.
pub const CLOUD_OCTA_SIZE: u32 = 4096;

pub struct CloudTemporal {
    // (dev forensics accessor below needs the textures; fields stay private)
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

impl CloudTemporal {
    /// The most recently written octa map texture (dev forensics: the
    /// cloudmap_request dump reads it back to a PNG so the accumulated map
    /// CONTENT can be inspected directly - the discriminator between
    /// "the starburst is baked into the map" and "it appears at sampling").
    pub fn cur_texture(&self) -> &wgpu::Texture {
        &self._textures[self.cur.get()]
    }
}

/// The NEAR-regime screen buffers (12e march/resolve split): a
/// QUARTER-res march pair (per-frame premultiplied march + first-hit
/// distance in km) plus a HALF-res RGBA16F accumulation ping-pong the
/// resolve pass deep-blends into. Recreated whenever the swapchain size
/// changes. The accumulation pair still carries group-3 albedo groups
/// (unused by the resolve, but kept so any future megashader consumer
/// can ride the standard slot).
pub struct CloudScreen {
    _textures: [wgpu::Texture; 2],
    pub views: [wgpu::TextureView; 2],
    pub groups: [AlbedoBindGroup; 2],
    pub cur: std::cell::Cell<usize>,
    pub size: (u32, u32),
    _march_tex: wgpu::Texture,
    pub march_view: wgpu::TextureView,
    _dist_tex: wgpu::Texture,
    pub dist_view: wgpu::TextureView,
    /// True on the frame the buffers were (re)created: the resolve must
    /// drop the (zeroed) history outright instead of fading in from
    /// black over ~1/alpha frames.
    pub fresh: std::cell::Cell<bool>,
    /// The march divisor these buffers were built for (v0.1255).
    pub div: u32,
}

impl Renderer {
    /// Ensure the near-regime screen buffers exist at the current
    /// resolution (12e: quarter-res march pair + half-res accumulation
    /// pair). Called by lib.rs when the near cloud mode is active.
    pub fn ensure_cloud_screen(&mut self) {
        // v0.1255: the march resolution is a live setting (4 = the
        // historical quarter res, 2 = half, 1 = full). The accumulation
        // pair runs at half the march divisor so the resolve keeps its
        // supersampling headroom at every setting. div is part of the
        // identity test because divisors 2 and 1 SHARE an accumulation
        // size - comparing size alone would silently keep a stale march
        // texture at the old resolution.
        let div = self.cloud_res_div.clamp(1, 4);
        let adiv = (div / 2).max(1);
        let want = (
            (self.config.width / adiv).max(8),
            (self.config.height / adiv).max(8),
        );
        if self
            .cloud_screen
            .as_ref()
            .map(|s| s.size == want && s.div == div)
            .unwrap_or(false)
        {
            return;
        }
        let mk = |dev: &wgpu::Device,
                  label: &str,
                  w: u32,
                  h: u32,
                  format: wgpu::TextureFormat| {
            let tex = dev.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                // COPY_SRC (v0.1247): the cloudmap/screen dev dumps read
                // these back with copy_texture_to_buffer - without the flag
                // the first live dump PANICKED the operator's session (wgpu
                // usage validation). Never ship an instrument untested.
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (tex, view)
        };
        let (t0, v0) = mk(
            &self.device, "Cloud Screen A", want.0, want.1,
            wgpu::TextureFormat::Rgba16Float,
        );
        let (t1, v1) = mk(
            &self.device, "Cloud Screen B", want.0, want.1,
            wgpu::TextureFormat::Rgba16Float,
        );
        let (qw, qh) = (
            (self.config.width / div).max(8),
            (self.config.height / div).max(8),
        );
        let (mt, mv) = mk(
            &self.device, "Cloud March Color", qw, qh,
            wgpu::TextureFormat::Rgba16Float,
        );
        let (dt, dv) = mk(
            &self.device, "Cloud March Dist", qw, qh,
            wgpu::TextureFormat::R16Float,
        );
        let g0 = self.build_albedo_group_from_view(&v0, &self.albedo_sampler);
        let g1 = self.build_albedo_group_from_view(&v1, &self.albedo_sampler);
        self.cloud_screen = Some(CloudScreen {
            _textures: [t0, t1],
            views: [v0, v1],
            groups: [g0, g1],
            cur: std::cell::Cell::new(0),
            size: want,
            div,
            _march_tex: mt,
            march_view: mv,
            _dist_tex: dt,
            dist_view: dv,
            fresh: std::cell::Cell::new(true),
        });
        log::info!(
            "Cloud screen pass ON: {}x{} march -> {}x{} accumulation",
            qw, qh, want.0, want.1
        );
    }

    /// Turn the temporal cloud path on (Some(cloud material index)) or off
    /// for this frame, creating the maps on first use. Called by lib.rs
    /// right after the cloud material update each frame.
    /// Record which material carries the cloud shell this frame.
    ///
    /// ── THE OCTA MAP IS FULLY DELETED (v0.1261) ──
    /// This used to allocate a 4096^2 RGBA16F PING-PONG PAIR (256 MB) for
    /// the direction-indexed cloud map, plus its ping-pong bind groups.
    /// The map stopped dispatching in v0.1250 (ONE RENDERER: the per-pixel
    /// screen march owns the whole sky), and v0.1260 stopped compositing
    /// its texture - which the operator had diagnosed from the outside as
    /// "another texture affecting the clouds that is not supposed to be".
    /// It still saw the effect afterwards, so the whole subsystem goes:
    /// no allocation, no pass, no binding. A never-written render target
    /// is not a guaranteed-zero source on every backend, and the only way
    /// to be certain it contributes nothing is for it not to exist.
    pub fn set_cloud_temporal(&mut self, mat: Option<usize>) {
        self.cloud_temporal_mat = mat;
    }
}
