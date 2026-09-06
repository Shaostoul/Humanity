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
        // Binding 14 of the accumulation pair: the profile atlas when it
        // exists (the far rung, A8: every cloud-side group carries the
        // atlas), else the tree atlas. The pair is recreated on every
        // resolution / cloud_res_div change, so this is the ONLY place the
        // rule can hold across a resize; ensure_cloud_profile does the same
        // for a pair that already exists when the atlas is first created.
        let (g0, g1) = match self.cloud_profile_cache.as_ref() {
            Some(pc) => (
                self.build_albedo_group_from_view_b14(&v0, &self.albedo_sampler, &pc.view_all),
                self.build_albedo_group_from_view_b14(&v1, &self.albedo_sampler, &pc.view_all),
            ),
            None => (
                self.build_albedo_group_from_view(&v0, &self.albedo_sampler),
                self.build_albedo_group_from_view(&v1, &self.albedo_sampler),
            ),
        };
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
        // The sun-shadow cache follows the same per-frame lifecycle: the
        // cloud fill block re-feeds it every frame it is armed, so a
        // frame with no cloud body leaves it unfed and the pass skips.
        // An unfed frame also marks the atlas stale: when the body comes
        // back the first bake is a full one, not a resumed eighth-cycle
        // over slices from an older cloud clock.
        if mat.is_none() {
            if self.cloud_light_frame.take().is_none() {
                if let Some(lc) = self.cloud_light_cache.as_ref() {
                    lc.state.mark_stale();
                }
            }
            // The far rung (increment 4) follows the same lifecycle on
            // EVERY tier: the shell material and the profile feed are
            // cleared here at the top of the frame; an unfed frame marks
            // the atlas stale so the next fed frame refills.
            self.cloud_shell_mat = None;
            if self.cloud_profile_frame.take().is_none() {
                if let Some(pc) = self.cloud_profile_cache.as_mut() {
                    pc.state.mark_stale();
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// THE SUN-SHADOW CACHE (performance plan increment 1, v0.1286)
// ═══════════════════════════════════════════════════════════════════════
//
// What it is for. Every cloud march step used to pay one eye density plus
// up to 12 sun-ladder densities (the "rung ladder" in cloud_sun_tau), and
// at Ultra each of those rebuilds the constructed cluster, so the ladder
// was 80-88% of all density work in every situation (the day-0 table in
// docs/PRIORITIES.md). The sun optical depth of a point does NOT depend
// on where the camera is, only on where the point is and where the sun
// is - so it can be computed once per planet-fixed lattice point, stored,
// and read back with one texture tap by every march sample near it.
//
// Shape. Two nested axis-aligned boxes ("windows") in a local frame at
// the camera's ground point (east e, up u, north n at the anchor): a FINE
// window (256 x 256 columns of 190 m, 48 levels of 240 m, 48.6 km square)
// and a COARSE window (128 x 128 of 760 m, 24 levels of 480 m, 97 km
// square), both 11.5 km tall starting 400 m above the sphere. Both are
// packed side by side into ONE 2D R16F atlas, one slice per level, so the
// texture rides the EXISTING group-3 albedo binding (the way the retired
// octa map did): no bind-group-layout change, none of the v0.1029
// every-create-site hazard.
//
// Who owns what. Rust (this file) owns the numbers: the atlas, the
// anchors (f64, the v0.1238 lesson: never subtract planet-scale
// quantities in f32), the re-anchor hysteresis, the sun re-reference,
// the bake phase, and the pad values written into the camera uniform.
// WGSL (45-cloud-temporal.wgsl `fs_cloud_light_bake`) owns the fragment
// that fills a lattice point, and 40-clouds.wgsl `light_cache_tau` owns
// the read. Every constant both sides share is a CLOUD_LC_* below with a
// unit test that reads the shader text and refuses to let them drift.
//
// Dev pad bit 16 (65536.0 in light7_color.w) = cache ON; off = the full
// ladder per pixel, the A/B twin (renderer field `cloud_light`).
//
// Frame agreement, one silent dependency worth knowing: the cloud shell
// render object is rotated by `Quat::from_rotation_y(spin as f32)` (lib.rs,
// the cloud fill block) while the feed below uses the f64 twin
// `DQuat::from_rotation_y(spin)`. They agree because `current_planet_spin`
// is kept wrapped to [0, 2 pi) (frame_lock.rs): the f32 ulp of a wrapped
// angle is ~5e-7 rad, ~3 m at the surface, under 2% of a fine cell. An
// UNWRAPPED spin accumulator (thousands of radians) would widen that gap
// to whole cells and quietly shift the window against the bake; wrap it
// or feed the f32 angle here too.

use glam::DVec3;

/// Fine window: columns per side (nx = ny).
pub const CLOUD_LC_FINE_NX: u32 = 256;
/// Fine window: levels (nz).
pub const CLOUD_LC_FINE_NZ: u32 = 48;
/// Fine window: horizontal cell size in metres.
pub const CLOUD_LC_FINE_CELL_H_M: f32 = 190.0;
/// Fine window: vertical cell size in metres.
pub const CLOUD_LC_FINE_CELL_V_M: f32 = 240.0;
/// Coarse window: columns per side.
pub const CLOUD_LC_COARSE_NX: u32 = 128;
/// Coarse window: levels.
pub const CLOUD_LC_COARSE_NZ: u32 = 24;
/// Coarse window: horizontal cell size in metres.
pub const CLOUD_LC_COARSE_CELL_H_M: f32 = 760.0;
/// Coarse window: vertical cell size in metres.
pub const CLOUD_LC_COARSE_CELL_V_M: f32 = 480.0;
// There is deliberately NO z0 constant. The slab base height above the
// anchor is `g_cloud_rb - length(anchor)` in the shader (light_cache_point
// and light_cache_tap, 40-clouds.wgsl), i.e. the planet's own cloud base
// `(slab_rb - 1) * radius`, which is 400 m on Earth but not on a modded
// world. An Earth number here would pass a sync test while proving
// nothing (the check-that-cannot-fail class); `cloud_lc_z0_m` below is the
// metre twin of the shader's rule instead.
/// Atlas width in texels: 48 fine slices of 256 then 24 coarse slices of
/// 128, side by side along x.
pub const CLOUD_LC_ATLAS_W: u32 = 15360;
/// Atlas height in texels (the fine slice size; coarse slices use the
/// bottom half of each column band).
pub const CLOUD_LC_ATLAS_H: u32 = 256;
/// x offset of the first coarse slice (48 * 256).
pub const CLOUD_LC_COARSE_X0: u32 = 12288;
/// Stored optical depth is clamped to this before the f16 write.
pub const CLOUD_LC_TAU_MAX: f32 = 64.0;
// There is deliberately NO "start 87 m sunward" constant either: the bake
// runs `cloud_sun_tau_far` from the lattice point itself with rungs 0-1 at
// depth 0, and the ladder's own rung recurrence places rung 2 where the
// per-pixel ladder would (the 87 m is the sum of rungs 0 and 1, which stay
// per pixel as `g_sun_tau01`). Applying an offset on the Rust side too
// would double it.
/// Frames per full refresh of each window when nothing forced a full bake
/// (one eighth of the slices per frame).
pub const CLOUD_LC_PHASES: u32 = 8;
/// Sun re-reference threshold in degrees: when the sun has moved this far
/// from the direction the cache was last referenced with, the reference
/// is moved and the event is COUNTED for the log line. It does NOT order a
/// full bake: the rolling refresh rebakes every slice within
/// CLOUD_LC_PHASES frames with the then-current sun, so the atlas already
/// trails the sun by well under a tenth of a degree, and a forced full
/// bake (8x a partial frame) every 2 degrees of sun travel would be a
/// permanent spike for nothing (Rust-only, the shader never needs it).
pub const CLOUD_LC_SUN_REREF_DEG: f64 = 2.0;
/// Re-anchor hysteresis in fine cells: the ground point must leave the
/// inner half of the fine window by this many cells before the windows
/// move (Rust-only).
pub const CLOUD_LC_REANCHOR_HYST_CELLS: f64 = 8.0;

/// The per-frame inputs lib.rs feeds the cache from the cloud fill block
/// (the same values the cloud shell render object is built from, so the
/// cache and the shader agree on the frame by construction).
#[derive(Clone, Copy, Debug)]
pub struct CloudLightFrame {
    /// The camera's ground point in PLANET-LOCAL metres: planet-centred,
    /// axes = the planet basis (the frame the cloud shell's object space
    /// uses, before its scale). Always exactly on the sphere of radius
    /// `radius_m`.
    pub ground_local_m: DVec3,
    /// The sun direction (toward the sun) in the same planet-local frame.
    pub sun_local: DVec3,
    /// Planet radius in metres.
    pub radius_m: f64,
    /// The drawn cloud shell radius as a multiple of the planet radius
    /// (lib.rs `shell_ratio` = slab_rt + 0.0006). One p-unit in the march
    /// is `radius_m * shell_ratio` metres.
    pub shell_ratio: f64,
    /// The cloud slab BASE as a multiple of the planet radius (lib.rs
    /// `slab_rb` from `cloud_slab_scales`, forwarded to the shader as
    /// material params2.x). The lattice's z0 is derived from it exactly
    /// as the shader does: `slab_rb * radius - |anchor|`.
    pub slab_rb: f64,
}

/// The slab base height above the anchor in metres: the exact metre twin
/// of the shader's `z0 = g_cloud_rb - length(anchor)` (40-clouds.wgsl
/// light_cache_point / light_cache_tap), where `g_cloud_rb = slab_rb *
/// inv_drawn` in p-units. Derived per planet, never an Earth constant.
pub fn cloud_lc_z0_m(anchor_m: DVec3, radius_m: f64, slab_rb: f64) -> f64 {
    slab_rb * radius_m - anchor_m.length()
}

/// Local east/up/north frame at a planet-local anchor (contract: u =
/// normalize(anchor), e = normalize(cross(Y, u)) with Y the planet spin
/// axis (0,1,0), n = cross(u, e)). At the poles cross(Y, u) vanishes; the
/// X axis stands in so the frame stays orthonormal instead of NaN.
pub fn cloud_lc_frame(anchor: DVec3) -> (DVec3, DVec3, DVec3) {
    let u = anchor.normalize();
    let mut e = DVec3::Y.cross(u);
    if e.length_squared() < 1.0e-12 {
        e = DVec3::X.cross(u);
    }
    let e = e.normalize();
    let n = u.cross(e);
    (e, u, n)
}

/// Position of lattice point (i, j, k) of a window in planet-local metres:
/// `anchor + e * ((i + 0.5) * cell_h - half_w) + n * ((j + 0.5) * cell_h - half_w) + u * (k * cell_v + z0)`
/// with `half_w = n_side * cell_h / 2` and `z0_m` from `cloud_lc_z0_m`
/// (the caller supplies it so this helper carries no planet assumption).
/// This is the formula the bake fragment evaluates; the unit tests below
/// hold the two sides together, including a text check that the shader's
/// `light_cache_point` still spells the same four terms.
pub fn cloud_lc_lattice_point(
    anchor: DVec3,
    n_side: u32,
    cell_h_m: f64,
    cell_v_m: f64,
    z0_m: f64,
    i: u32,
    j: u32,
    k: u32,
) -> DVec3 {
    let (e, u, n) = cloud_lc_frame(anchor);
    let half_w = n_side as f64 * cell_h_m * 0.5;
    anchor
        + e * ((i as f64 + 0.5) * cell_h_m - half_w)
        + n * ((j as f64 + 0.5) * cell_h_m - half_w)
        + u * (k as f64 * cell_v_m + z0_m)
}

/// Convert planet-local metres to the march's p-units (the cloud shell's
/// object space: origin at the planet centre, 1 unit = the drawn shell
/// radius = `radius_m * shell_ratio` metres). Both the anchors and the
/// cell sizes go through this so the pads carry exactly what the shader's
/// `g_cloud_upkm` ladder implies.
pub fn cloud_lc_metres_to_p(radius_m: f64, shell_ratio: f64) -> f64 {
    1.0 / (radius_m * shell_ratio)
}

/// What `plan` decided this frame, for the log line and the tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloudLightPlan {
    /// The windows moved to the current ground point (full bake ordered).
    pub reanchored: bool,
    /// The sun moved past the threshold (full bake ordered).
    pub sun_reref: bool,
}

/// The GPU-free planning state of the cache: anchors, sun reference,
/// bake phase, counters. Split from the atlas so the unit tests can drive
/// the exact code the renderer runs without a device.
#[derive(Default)]
pub struct CloudLightState {
    /// Fine window anchor, planet-local metres, on the sphere. `None`
    /// until the first plan.
    pub anchor_fine_m: Option<DVec3>,
    /// Coarse window anchor. Shares the fine anchor (the coarse window is
    /// centred on the fine one, so the fine window and its re-anchor
    /// hysteresis are always inside it), kept as its own field because
    /// the pads carry two anchors and a later cut may let them drift.
    pub anchor_coarse_m: Option<DVec3>,
    /// The planet-local sun direction the atlas was last fully baked
    /// with; the 2 degree re-reference compares against it.
    pub sun_ref: Option<DVec3>,
    /// Which eighth of the slices the next partial bake refreshes.
    pub phase: std::cell::Cell<u32>,
    /// Set by `plan` on creation, re-anchor and sun re-reference: the next
    /// bake pass covers every slice in one frame instead of an eighth.
    pub full_pending: std::cell::Cell<bool>,
    /// Bookkeeping for the once-a-second log line.
    pub reanchors: u32,
    pub sun_rerefs: u32,
    pub full_bakes: std::cell::Cell<u32>,
    pub partial_bakes: std::cell::Cell<u32>,
    /// The frame constants the pads were last computed with.
    pub frame: Option<CloudLightFrame>,
    /// The last plan result (log line).
    pub last_plan: CloudLightPlan,
}

impl CloudLightState {
    /// Decide this frame's anchors from the camera's ground point and the
    /// sun. Returns what changed. Pure planning: no GPU work here.
    ///
    /// Re-anchor rule (contract): the windows move when the ground point
    /// leaves the inner HALF of the fine window, with an 8-cell hysteresis
    /// margin - the point must be more than `half_w / 2 + 8 * cell_h`
    /// along east or north from the anchor. After a move the ground point
    /// sits at the centre, so a camera parked near a boundary cannot
    /// chatter the windows.
    pub fn plan(&mut self, frame: CloudLightFrame) -> CloudLightPlan {
        let mut out = CloudLightPlan::default();
        let ground = frame.ground_local_m;
        let need_anchor = match self.anchor_fine_m {
            None => true,
            Some(a) => {
                let (e, _u, n) = cloud_lc_frame(a);
                let d = ground - a;
                let cell = CLOUD_LC_FINE_CELL_H_M as f64;
                let half_w = CLOUD_LC_FINE_NX as f64 * cell * 0.5;
                let limit = half_w * 0.5 + CLOUD_LC_REANCHOR_HYST_CELLS * cell;
                d.dot(e).abs() > limit || d.dot(n).abs() > limit
            }
        };
        if need_anchor {
            // The anchor is the ground point itself (on the sphere): the
            // cheapest placement, and the one that puts the camera at the
            // best-resolved centre of both windows.
            let a = ground.normalize() * frame.radius_m;
            self.anchor_fine_m = Some(a);
            self.anchor_coarse_m = Some(a);
            if self.frame.is_some() {
                self.reanchors += 1;
                out.reanchored = true;
            }
            self.full_pending.set(true);
        }
        // Sun re-reference: tracked and counted, but it orders NO full
        // bake (see CLOUD_LC_SUN_REREF_DEG): the rolling eighth-per-frame
        // refresh already rebakes every slice with the current sun within
        // CLOUD_LC_PHASES frames. The first reference (None) is part of
        // the creation full bake ordered by the anchor branch above.
        let sun = frame.sun_local.normalize_or_zero();
        let need_sun = match self.sun_ref {
            None => true,
            Some(s) => s.dot(sun) < CLOUD_LC_SUN_REREF_DEG.to_radians().cos(),
        };
        if need_sun {
            self.sun_ref = Some(sun);
            if self.frame.is_some() && !need_anchor {
                self.sun_rerefs += 1;
                out.sun_reref = true;
            }
        }
        self.frame = Some(frame);
        self.last_plan = out;
        out
    }

    /// Mark the atlas contents STALE: the next bake pass covers every
    /// slice in one frame. Called for every frame the cache exists but is
    /// not planned (toggle off, or no near cloud body), so that turning
    /// it back on never lets the march read slices baked at an older
    /// cloud clock; the A/B methodology ("cache on vs off in ONE boot")
    /// depends on the second on-capture being current.
    pub fn mark_stale(&self) {
        if self.anchor_fine_m.is_some() {
            self.full_pending.set(true);
        }
    }

    /// The two camera-uniform pads (light3_color at byte 256, light4_color
    /// at 272): `(anchor_x, anchor_y, anchor_z, cell_h)` per window, in
    /// the march's p-units, f32 only at this final narrowing.
    pub fn pads(&self) -> ([f32; 4], [f32; 4]) {
        let (Some(f), Some(af), Some(ac)) =
            (self.frame, self.anchor_fine_m, self.anchor_coarse_m)
        else {
            return ([0.0; 4], [0.0; 4]);
        };
        let s = cloud_lc_metres_to_p(f.radius_m, f.shell_ratio);
        let af = af * s;
        let ac = ac * s;
        (
            [
                af.x as f32,
                af.y as f32,
                af.z as f32,
                (CLOUD_LC_FINE_CELL_H_M as f64 * s) as f32,
            ],
            [
                ac.x as f32,
                ac.y as f32,
                ac.z as f32,
                (CLOUD_LC_COARSE_CELL_H_M as f64 * s) as f32,
            ],
        )
    }

    /// The cache has an anchor and a sun reference: the shader may read
    /// it (the pad bit is only raised when this is true).
    pub fn ready(&self) -> bool {
        self.anchor_fine_m.is_some() && self.sun_ref.is_some() && self.frame.is_some()
    }

    /// Scissor rectangles `(x, y, w, h)` for one bake frame: every slice
    /// of both windows when a full bake is pending (two rects, each the
    /// exact height of its window: the coarse rows 128..255 are never
    /// sampled, so they are never drawn either), else one eighth of each
    /// window's slices (a contiguous run of whole slices, so a partial
    /// bake never splits a slice). Advances the phase on a partial bake.
    /// Called by the bake pass on `&self`.
    pub fn take_bake_rects(&self) -> Vec<(u32, u32, u32, u32)> {
        if self.full_pending.replace(false) {
            self.full_bakes.set(self.full_bakes.get().wrapping_add(1));
            return vec![
                (0, 0, CLOUD_LC_COARSE_X0, CLOUD_LC_FINE_NX),
                (
                    CLOUD_LC_COARSE_X0,
                    0,
                    CLOUD_LC_ATLAS_W - CLOUD_LC_COARSE_X0,
                    CLOUD_LC_COARSE_NX,
                ),
            ];
        }
        let phase = self.phase.get();
        self.phase.set((phase + 1) % CLOUD_LC_PHASES);
        self.partial_bakes.set(self.partial_bakes.get().wrapping_add(1));
        let fine_per = CLOUD_LC_FINE_NZ / CLOUD_LC_PHASES;
        let coarse_per = CLOUD_LC_COARSE_NZ / CLOUD_LC_PHASES;
        vec![
            (
                phase * fine_per * CLOUD_LC_FINE_NX,
                0,
                fine_per * CLOUD_LC_FINE_NX,
                CLOUD_LC_FINE_NX,
            ),
            (
                CLOUD_LC_COARSE_X0 + phase * coarse_per * CLOUD_LC_COARSE_NX,
                0,
                coarse_per * CLOUD_LC_COARSE_NX,
                CLOUD_LC_COARSE_NX,
            ),
        ]
    }
}

/// The atlas plus its planning state. Created by `ensure_cloud_light` on
/// the first frame the cache is wanted; planned every frame by
/// `cloud_light_plan` (called from lib.rs) and consumed by the bake pass
/// in mod.rs (interior mutability, the pass runs on `&self`).
pub struct CloudLightCache {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    /// Group-3 bind groups with the atlas in the albedo slot: the MARCH
    /// binds `.colour` at group 3 while the cache is on.
    pub group: AlbedoBindGroup,
    pub state: CloudLightState,
}

impl CloudLightCache {
    /// The most recently written atlas (dev forensics: a future
    /// cloudmap-style dump reads it back to a PNG).
    pub fn texture(&self) -> &wgpu::Texture {
        &self._texture
    }
}

impl Renderer {
    /// Create the atlas on first use. Refuses (logs once, stays `None`)
    /// when the device cannot hold a 15360-wide texture: the limits are
    /// requested `using_resolution(adapter)` so a desktop GPU grants
    /// 16384+, but a small-limit adapter must degrade to the ladder, not
    /// die at create_texture (the v0.782 boot-killer class).
    pub fn ensure_cloud_light(&mut self) {
        if self.cloud_light_cache.is_some() {
            return;
        }
        let max_dim = self.device.limits().max_texture_dimension_2d;
        if max_dim < CLOUD_LC_ATLAS_W {
            static WARNED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                log::warn!(
                    "[CloudLight] atlas {}x{} exceeds max_texture_dimension_2d {}: cache stays off",
                    CLOUD_LC_ATLAS_W, CLOUD_LC_ATLAS_H, max_dim
                );
            }
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Cloud Light Cache Atlas"),
            size: wgpu::Extent3d {
                width: CLOUD_LC_ATLAS_W,
                height: CLOUD_LC_ATLAS_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            // COPY_SRC for the same reason as the screen buffers: a dev
            // dump must never be the first time the flag is missed.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        // The group's own albedo sampler is linear (u repeats, v clamps):
        // the shader does its own trilinear with texel-centre clamping per
        // slice, so the u wrap is never reached. The per-slice isolation
        // is exact in y (256 is a power of two, so a clamped 127.5/256
        // reconstructs to the texel centre with zero weight on row 128)
        // but only approximate in x: 15360 is not a power of two, so the
        // sampler's fixed-point (x + 0.5) / 15360 can carry up to ~1/256
        // of filter weight into the neighbouring column, which at the
        // fine/coarse seam (x = 12287.5) belongs to the OTHER window. That
        // is 0.4% of the two windows' tau difference, under the f16 noise
        // floor; do not let a future atlas re-pack make it worse.
        let group = self.build_albedo_group_from_view(&view, &self.albedo_sampler);
        self.cloud_light_cache = Some(CloudLightCache {
            _texture: tex,
            view,
            group,
            state: CloudLightState::default(),
        });
        // The profile's `group_sun` (b0 = THIS atlas, b14 = the profile
        // atlas) is rebuilt whenever either atlas is (re)created.
        self.rebuild_cloud_profile_sun_group();
        log::info!(
            "[CloudLight] atlas ON: {}x{} R16F ({:.1} MB), fine {}^2x{} @ {} m, coarse {}^2x{} @ {} m",
            CLOUD_LC_ATLAS_W,
            CLOUD_LC_ATLAS_H,
            (CLOUD_LC_ATLAS_W * CLOUD_LC_ATLAS_H * 2) as f64 / 1.0e6,
            CLOUD_LC_FINE_NX,
            CLOUD_LC_FINE_NZ,
            CLOUD_LC_FINE_CELL_H_M,
            CLOUD_LC_COARSE_NX,
            CLOUD_LC_COARSE_NZ,
            CLOUD_LC_COARSE_CELL_H_M,
        );
    }

    /// Per-frame feed from lib.rs (the cloud fill block): create the atlas
    /// if wanted, plan the anchors. When the toggle is off this records
    /// the frame and marks the atlas stale, so flipping the F10 box on
    /// takes effect the next frame with a FULL bake (never slices from an
    /// older cloud clock). A second call in the same frame (two near cloud
    /// bodies at once, which the near==temporal arming cannot produce
    /// today) is ignored so two ground points can never fight over the
    /// anchors and re-anchor every frame; `set_cloud_temporal(None)` at
    /// the top of every frame clears the feed for the next one.
    pub fn cloud_light_plan(&mut self, frame: CloudLightFrame) {
        if self.cloud_light_frame.is_some() {
            return;
        }
        self.cloud_light_frame = Some(frame);
        if !self.cloud_light {
            if let Some(lc) = self.cloud_light_cache.as_ref() {
                lc.state.mark_stale();
            }
            return;
        }
        self.ensure_cloud_light();
        if let Some(lc) = self.cloud_light_cache.as_mut() {
            lc.state.plan(frame);
        }
    }

    /// The cache the march may READ this frame: the toggle is on, the
    /// near regime is armed AND the temporal material is set (the pass
    /// that binds the atlas at group 3 is gated on the material, so bit
    /// 16 must be too, or a near-without-temporal frame would send the
    /// shell fragment reading a 1x1 fallback texel as tau), the frame was
    /// fed, the atlas exists and has been planned.
    pub(crate) fn cloud_light_active(&self) -> Option<&CloudLightCache> {
        if !self.cloud_light
            || !self.cloud_mode_near
            || self.cloud_temporal_mat.is_none()
            || self.cloud_light_frame.is_none()
        {
            return None;
        }
        self.cloud_light_cache.as_ref().filter(|lc| lc.state.ready())
    }
}

/// Read `const NAME: <type> = <value>;` out of a shader text whatever the
/// scalar type (the cloud constants are a mix of u32, i32 and f32). Shared by
/// the sun-cache and the profile sync tests.
#[cfg(test)]
fn wgsl_const(src: &str, name: &str) -> f64 {
    for line in src.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix(&format!("const {name}:")) else {
            continue;
        };
        let Some((_ty, val)) = rest.split_once('=') else {
            continue;
        };
        let v = val.trim().trim_end_matches(';').trim().trim_end_matches('u');
        return v
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("parse {name} = {v}"));
    }
    panic!(
        "const {name} not found in the shader - the WGSL side must declare \
         every shared constant with exactly this name and value"
    );
}

#[cfg(test)]
mod cloud_light_tests {
    use super::*;

    /// Earth numbers from data/solar_system (radius 6371 km) and the
    /// shipped slab (top 12 km): shell_ratio = slab_rt + 0.0006.
    const R_M: f64 = 6_371_000.0;
    const SHELL_RATIO: f64 = 12.0 / 6371.0 + 1.0 + 0.0006;
    /// The slab base ratio exactly as `Planet::cloud_slab_scales` derives
    /// it for a body with no override (base = r_km * 0.4 / 6371): the
    /// same value the shader receives in params2.x.
    const SLAB_RB: f64 = 1.0 + (6371.0 * (0.4 / 6371.0)) / 6371.0;

    fn frame_at(ground: DVec3, sun: DVec3) -> CloudLightFrame {
        CloudLightFrame {
            ground_local_m: ground.normalize() * R_M,
            sun_local: sun.normalize(),
            radius_m: R_M,
            shell_ratio: SHELL_RATIO,
            slab_rb: SLAB_RB,
        }
    }

    /// z0 for an anchor on the sphere, by the shader's rule.
    fn z0_at(anchor: DVec3) -> f64 {
        cloud_lc_z0_m(anchor, R_M, SLAB_RB)
    }

    /// The contract's sync test: every CLOUD_LC_* the shader reads must
    /// equal the Rust constant, read from the shader TEXT so the two can
    /// never drift silently (the pattern of
    /// wgsl_reference_constants_stay_in_sync in cloud_reference.rs).
    #[test]
    fn wgsl_cloud_light_constants_stay_in_sync() {
        let root = env!("CARGO_MANIFEST_DIR");
        let src = std::fs::read_to_string(format!("{root}/assets/shaders/pbr/40-clouds.wgsl"))
            .expect("read 40-clouds.wgsl");
        let pairs: [(&str, f64); 12] = [
            ("CLOUD_LC_FINE_NX", CLOUD_LC_FINE_NX as f64),
            ("CLOUD_LC_FINE_NZ", CLOUD_LC_FINE_NZ as f64),
            ("CLOUD_LC_FINE_CELL_H_M", CLOUD_LC_FINE_CELL_H_M as f64),
            ("CLOUD_LC_FINE_CELL_V_M", CLOUD_LC_FINE_CELL_V_M as f64),
            ("CLOUD_LC_COARSE_NX", CLOUD_LC_COARSE_NX as f64),
            ("CLOUD_LC_COARSE_NZ", CLOUD_LC_COARSE_NZ as f64),
            ("CLOUD_LC_COARSE_CELL_H_M", CLOUD_LC_COARSE_CELL_H_M as f64),
            ("CLOUD_LC_COARSE_CELL_V_M", CLOUD_LC_COARSE_CELL_V_M as f64),
            ("CLOUD_LC_ATLAS_W", CLOUD_LC_ATLAS_W as f64),
            ("CLOUD_LC_ATLAS_H", CLOUD_LC_ATLAS_H as f64),
            ("CLOUD_LC_COARSE_X0", CLOUD_LC_COARSE_X0 as f64),
            ("CLOUD_LC_TAU_MAX", CLOUD_LC_TAU_MAX as f64),
        ];
        for (name, rust_v) in pairs {
            let wgsl_v = wgsl_const(&src, name);
            assert!(
                (wgsl_v - rust_v).abs() < 1.0e-6,
                "{name}: WGSL {wgsl_v} != Rust {rust_v} - the bake and the read no longer agree on the lattice"
            );
        }
    }

    /// The lattice FORMULA, not just its constants: the shader's
    /// `light_cache_point` must still spell the four terms
    /// `cloud_lc_lattice_point` evaluates (and derive z0 by the rule
    /// `cloud_lc_z0_m` mirrors). A text check is weak, but it is the only
    /// thing here that goes red when the shader's sign, half-width or
    /// axis order drifts; the geometry tests below validate the Rust
    /// helper against itself and cannot see the shader at all.
    #[test]
    fn wgsl_light_cache_point_spells_the_same_lattice_formula() {
        let root = env!("CARGO_MANIFEST_DIR");
        let src = std::fs::read_to_string(format!("{root}/assets/shaders/pbr/40-clouds.wgsl"))
            .expect("read 40-clouds.wgsl");
        let start = src
            .find("fn light_cache_point(")
            .expect("40-clouds.wgsl must define fn light_cache_point (the bake's lattice)");
        let body = &src[start..];
        let end = body.find("\n}").expect("light_cache_point body end");
        let body = &body[..end];
        for term in [
            "let half_w = nx * cell_h * 0.5;",
            "let z0 = g_cloud_rb - length(anchor);",
            "e * ((i + 0.5) * cell_h - half_w)",
            "n * ((j + 0.5) * cell_h - half_w)",
            "u * (k * cell_v + z0)",
        ] {
            assert!(
                body.contains(term),
                "light_cache_point no longer contains `{term}`: the bake lattice drifted from cloud_lc_lattice_point"
            );
        }
        // The frame convention too: u from the anchor, n = cross(u, e),
        // matching cloud_lc_frame (e comes from light_cache_east, whose
        // spin-axis cross is checked by the frame tests' east.y == 0).
        assert!(body.contains("let u = normalize(anchor);"));
        assert!(body.contains("let n = cross(u, e);"));
    }

    /// The packing arithmetic the constants encode must stay coherent on
    /// the Rust side on its own (no shader needed).
    #[test]
    fn atlas_packing_constants_are_coherent() {
        assert_eq!(CLOUD_LC_COARSE_X0, CLOUD_LC_FINE_NZ * CLOUD_LC_FINE_NX);
        assert_eq!(
            CLOUD_LC_ATLAS_W,
            CLOUD_LC_COARSE_X0 + CLOUD_LC_COARSE_NZ * CLOUD_LC_COARSE_NX
        );
        assert_eq!(CLOUD_LC_ATLAS_H, CLOUD_LC_FINE_NX);
        assert_eq!(CLOUD_LC_FINE_NZ % CLOUD_LC_PHASES, 0);
        assert_eq!(CLOUD_LC_COARSE_NZ % CLOUD_LC_PHASES, 0);
        // Both windows span the same 11.5 km of slab.
        assert!(
            ((CLOUD_LC_FINE_NZ as f32 * CLOUD_LC_FINE_CELL_V_M)
                - (CLOUD_LC_COARSE_NZ as f32 * CLOUD_LC_COARSE_CELL_V_M))
                .abs()
                < 1.0
        );
    }

    /// The contract's Units test: a point 1 km east of the anchor at the
    /// equator maps to `anchor_p + e * (1 km in p-units)` where "1 km in
    /// p-units" is the shader's own `g_cloud_upkm`, ported here from the
    /// two lines in cloud_set_slab_bounds:
    ///     inv_drawn = material.params.w  (lib.rs passes 1 / shell_ratio)
    ///     g_cloud_upkm = inv_drawn / material.params2.z  (planet radius km)
    #[test]
    fn anchor_to_p_units_matches_the_shader_ladder() {
        let inv_drawn = 1.0 / SHELL_RATIO;
        let radius_km = R_M / 1000.0;
        let upkm_shader = inv_drawn / radius_km;
        let s = cloud_lc_metres_to_p(R_M, SHELL_RATIO);
        let upkm_rust = 1000.0 * s;
        assert!(
            (upkm_shader - upkm_rust).abs() < 1.0e-9,
            "units per km: shader {upkm_shader} vs rust {upkm_rust}"
        );
        // Anchor at the equator, longitude 0 in planet-local space.
        let anchor = DVec3::new(R_M, 0.0, 0.0);
        let (e, _u, _n) = cloud_lc_frame(anchor);
        let east_1km_m = anchor + e * 1000.0;
        let got = east_1km_m * s;
        let want = anchor * s + e * upkm_shader;
        assert!(
            (got - want).length() < 1.0e-6,
            "1 km east in p-units: got {got:?} want {want:?}"
        );
        // And the pads carry that same scale: cell_h in p-units equals
        // cell_h_m * 0.001 * upkm.
        let mut st = CloudLightState::default();
        st.plan(frame_at(anchor, DVec3::new(0.3, 1.0, 0.5)));
        let (fine, coarse) = st.pads();
        let want_fine = (CLOUD_LC_FINE_CELL_H_M as f64 * 0.001 * upkm_shader) as f32;
        let want_coarse = (CLOUD_LC_COARSE_CELL_H_M as f64 * 0.001 * upkm_shader) as f32;
        assert!(
            (fine[3] - want_fine).abs() < 1.0e-9,
            "fine cell pad {} vs {}",
            fine[3],
            want_fine
        );
        assert!(
            (coarse[3] - want_coarse).abs() < 1.0e-9,
            "coarse cell pad {} vs {}",
            coarse[3],
            want_coarse
        );
        // The anchor pad sits on the planet sphere at radius 1/shell_ratio.
        let ap = DVec3::new(fine[0] as f64, fine[1] as f64, fine[2] as f64);
        assert!((ap.length() - 1.0 / SHELL_RATIO).abs() < 1.0e-6);
        // The unplanned state writes zero pads (the shader never reads
        // them while the bit is down, but zeros are still the safe value).
        assert_eq!(CloudLightState::default().pads(), ([0.0; 4], [0.0; 4]));
    }

    /// Lattice geometry: the frame is orthonormal, east is horizontal,
    /// north points toward the +Y pole, the centre column's altitude is
    /// z0 + k * cell_v (the anchor sits on the sphere), and the extreme
    /// corner of the fine window is exactly half_w - cell_h/2 along both
    /// axes. Also the inverse: dotting a lattice point's offset with
    /// (e, n, u) recovers (i, j, k), which is what the shader's
    /// `light_cache_tau` does to find its taps.
    fn lattice_checks(anchor: DVec3) {
        let (e, u, n) = cloud_lc_frame(anchor);
        for (a, b) in [(e, u), (e, n), (u, n)] {
            assert!(a.dot(b).abs() < 1.0e-9, "frame axes not orthogonal");
        }
        for v in [e, u, n] {
            assert!((v.length() - 1.0).abs() < 1.0e-9, "frame axis not unit");
        }
        assert!(e.y.abs() < 1.0e-9, "east must be horizontal (no spin-axis component)");
        assert!(n.y > 0.0, "north must point toward the +Y pole");
        assert!((u - anchor.normalize()).length() < 1.0e-9);
        let cell_h = CLOUD_LC_FINE_CELL_H_M as f64;
        let cell_v = CLOUD_LC_FINE_CELL_V_M as f64;
        let half_w = CLOUD_LC_FINE_NX as f64 * cell_h * 0.5;
        // Centre column: i = j = N/2 - 1 sits half a cell south-west of
        // the anchor; its altitude above the sphere is z0 + k * cell_v
        // plus the curvature sag of a 95 m offset (millimetres).
        let z0 = z0_at(anchor);
        // The shader's rule gives Earth's 0.4 km base for a sphere anchor.
        assert!((z0 - 400.0).abs() < 1.0e-3, "Earth z0 {z0} m");
        let c = CLOUD_LC_FINE_NX / 2 - 1;
        for k in [0u32, 1, 47] {
            let p =
                cloud_lc_lattice_point(anchor, CLOUD_LC_FINE_NX, cell_h, cell_v, z0, c, c, k);
            let alt = p.length() - R_M;
            let want = z0 + k as f64 * cell_v;
            assert!(
                (alt - want).abs() < 0.01,
                "centre column k={k}: altitude {alt} vs {want}"
            );
            // Inverse lookup recovers (i, j, k).
            let d = p - anchor;
            let fi = (d.dot(e) + half_w) / cell_h - 0.5;
            let fj = (d.dot(n) + half_w) / cell_h - 0.5;
            let fk = (d.dot(u) - z0) / cell_v;
            assert!((fi - c as f64).abs() < 1.0e-6, "i recovered {fi}");
            assert!((fj - c as f64).abs() < 1.0e-6, "j recovered {fj}");
            assert!((fk - k as f64).abs() < 1.0e-6, "k recovered {fk}");
        }
        // Extreme corner (0, 0, 0): half_w - cell_h/2 south-west of the
        // anchor at the slab base. The lattice is a FLAT box, not a shell:
        // at 24.2 km the sphere has sagged 46 m below it. The bake and the
        // read use the same flat box, so no correction belongs here; the
        // sag is asserted only so the number is on record.
        let p0 = cloud_lc_lattice_point(anchor, CLOUD_LC_FINE_NX, cell_h, cell_v, z0, 0, 0, 0);
        let d0 = p0 - anchor;
        assert!((d0.dot(e) + (half_w - cell_h * 0.5)).abs() < 1.0e-6);
        assert!((d0.dot(n) + (half_w - cell_h * 0.5)).abs() < 1.0e-6);
        assert!((d0.dot(u) - z0).abs() < 1.0e-6);
        let horiz2 = d0.dot(e).powi(2) + d0.dot(n).powi(2);
        let sag = horiz2 / (2.0 * R_M);
        assert!(sag > 80.0 && sag < 100.0, "corner sag {sag} m (sanity)");
    }

    #[test]
    fn lattice_at_the_equator() {
        lattice_checks(DVec3::new(R_M, 0.0, 0.0));
        // A second longitude so the east axis is exercised off the X axis.
        lattice_checks(DVec3::new(0.0, 0.0, -R_M));
    }

    #[test]
    fn lattice_at_latitude_60() {
        let lat = 60.0f64.to_radians();
        lattice_checks(DVec3::new(lat.cos() * R_M, lat.sin() * R_M, 0.0));
        let lon = 37.0f64.to_radians();
        lattice_checks(DVec3::new(
            lat.cos() * lon.cos() * R_M,
            lat.sin() * R_M,
            -lat.cos() * lon.sin() * R_M,
        ));
    }

    #[test]
    fn frame_survives_the_poles() {
        let (e, u, n) = cloud_lc_frame(DVec3::new(0.0, R_M, 0.0));
        for v in [e, u, n] {
            assert!(v.is_finite() && (v.length() - 1.0).abs() < 1.0e-9);
        }
    }

    /// Re-anchor hysteresis and the sun re-reference, on the GPU-free
    /// planning state.
    #[test]
    fn plan_reanchors_past_the_inner_half_and_rerefs_the_sun_at_2_degrees() {
        let anchor = DVec3::new(R_M, 0.0, 0.0);
        let (e, _u, n) = cloud_lc_frame(anchor);
        let sun = DVec3::new(0.3, 1.0, 0.5).normalize();
        let mut st = CloudLightState::default();
        // First plan: anchors placed, full bake pending, not counted as a
        // re-anchor.
        let p0 = st.plan(frame_at(anchor, sun));
        assert_eq!(p0, CloudLightPlan::default());
        assert!(st.full_pending.get());
        assert!(st.ready());
        st.full_pending.set(false);
        let cell = CLOUD_LC_FINE_CELL_H_M as f64;
        let half_w = CLOUD_LC_FINE_NX as f64 * cell * 0.5;
        let limit = half_w * 0.5 + CLOUD_LC_REANCHOR_HYST_CELLS * cell;
        // Inside the inner half plus the margin: no move.
        let p1 = st.plan(frame_at(anchor + e * (limit - 100.0), sun));
        assert!(!p1.reanchored && !st.full_pending.get());
        assert_eq!(st.anchor_fine_m.unwrap(), anchor);
        // Just past it along north: move, full bake ordered.
        let far = anchor + n * (limit + 100.0);
        let p2 = st.plan(frame_at(far, sun));
        assert!(p2.reanchored && st.full_pending.get());
        let a2 = st.anchor_fine_m.unwrap();
        assert!((a2.length() - R_M).abs() < 1.0e-3, "anchor must sit on the sphere");
        assert!((a2 - far.normalize() * R_M).length() < 1.0e-3);
        assert_eq!(st.anchor_coarse_m.unwrap(), a2);
        assert_eq!(st.reanchors, 1);
        st.full_pending.set(false);
        // Sun 1.9 degrees away: nothing. 2.1 degrees: re-reference,
        // counted but NOT a full bake (the rolling refresh tracks the sun;
        // a full bake here would be an 8x frame every ~7 s of day).
        let tilt = |deg: f64| {
            let axis = sun.cross(DVec3::Y).normalize();
            glam::DQuat::from_axis_angle(axis, deg.to_radians()) * sun
        };
        let p3 = st.plan(frame_at(far, tilt(1.9)));
        assert!(!p3.sun_reref && !st.full_pending.get());
        let p4 = st.plan(frame_at(far, tilt(2.1)));
        assert!(p4.sun_reref && !st.full_pending.get() && !p4.reanchored);
        assert_eq!(st.sun_rerefs, 1);
        // The reference is now the 2.1 degree sun: 1 more degree does not
        // trip it again.
        let p5 = st.plan(frame_at(far, tilt(3.0)));
        assert!(!p5.sun_reref && !st.full_pending.get());
    }

    /// Stale marking: a planned cache that misses frames (toggle off, or
    /// no near body) must come back with a FULL bake, and an unplanned
    /// state stays untouched (nothing to invalidate).
    #[test]
    fn stale_marking_orders_a_full_bake_only_for_a_planned_cache() {
        let st = CloudLightState::default();
        st.mark_stale();
        assert!(!st.full_pending.get(), "nothing planned, nothing stale");
        let mut st = st;
        st.plan(frame_at(DVec3::new(R_M, 0.0, 0.0), DVec3::new(0.3, 1.0, 0.5)));
        // Drain the creation bake, then one partial.
        st.take_bake_rects();
        assert_eq!(st.take_bake_rects().len(), 2);
        assert!(!st.full_pending.get());
        st.mark_stale();
        assert!(st.full_pending.get(), "a skipped frame makes the atlas stale");
        // And the next bake is the full one.
        let rects = st.take_bake_rects();
        assert_eq!(rects[0].2, CLOUD_LC_COARSE_X0, "full fine rect");
        assert!(!st.full_pending.get());
    }

    /// The bake schedule: a full bake covers the whole atlas once, then
    /// eight partial frames each cover 6 fine and 3 coarse whole slices,
    /// tiling both windows exactly once with no overlap.
    #[test]
    fn bake_rects_tile_the_atlas_in_eight_frames() {
        let st = CloudLightState::default();
        st.full_pending.set(true);
        // The full bake: two rects that together span every column, each
        // exactly its window's slice height (fine 256 rows, coarse 128;
        // the coarse rows 128..255 are never sampled so never drawn).
        let full = st.take_bake_rects();
        assert_eq!(full.len(), 2);
        assert_eq!(full[0], (0, 0, CLOUD_LC_COARSE_X0, CLOUD_LC_FINE_NX));
        assert_eq!(
            full[1],
            (
                CLOUD_LC_COARSE_X0,
                0,
                CLOUD_LC_ATLAS_W - CLOUD_LC_COARSE_X0,
                CLOUD_LC_COARSE_NX
            )
        );
        assert_eq!(full[0].2 + full[1].2, CLOUD_LC_ATLAS_W);
        let mut covered = vec![0u8; CLOUD_LC_ATLAS_W as usize];
        for _ in 0..CLOUD_LC_PHASES {
            let rects = st.take_bake_rects();
            assert_eq!(rects.len(), 2);
            for (x, y, w, h) in rects {
                assert_eq!(y, 0);
                // Whole slices only, aligned to slice boundaries.
                let (base, slice) = if x < CLOUD_LC_COARSE_X0 {
                    (0, CLOUD_LC_FINE_NX)
                } else {
                    (CLOUD_LC_COARSE_X0, CLOUD_LC_COARSE_NX)
                };
                assert_eq!(h, slice);
                assert_eq!((x - base) % slice, 0);
                assert_eq!(w % slice, 0);
                for c in x..x + w {
                    covered[c as usize] += 1;
                }
            }
        }
        assert!(
            covered.iter().all(|&c| c == 1),
            "every atlas column baked exactly once per cycle"
        );
        assert_eq!(st.phase.get(), 0);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// THE CLOUD PROFILE, THE FAR RUNG (performance plan increment 4)
// ═══════════════════════════════════════════════════════════════════════
//
// Contract: docs/design/cloud-far-rung.md (v2). Read that first; this block
// implements its "Rust" section and nothing else, and every constant below
// is named and valued exactly as the contract's table says.
//
// What it is for. From orbit the constructed cloud bodies (the Ultra
// cumulus field) are POINT-SAMPLED at footprints larger than the clouds
// themselves: each march sample is a coin flip (inside a lobe or clear)
// printed as one full-white texel, which is the "speckle" the operator sees
// at 873 km. The far rung gives the built field a PREFILTERED
// representation: a planet-fixed PROFILE per lattice cell and height bin
// (cloud fraction f, mean density G and the column C above the bin), baked
// into an RGBA8 atlas over six nested toroidal (clipmap) windows around the
// camera's ground cell plus one global equirect map with a real mip
// pyramid. The march reads it with 4-tap loads chosen by the same lodb that
// picks every noise mip, and integrates the profile share in transmittance
// with a clumped-medium law (a property of the medium, not of the step).
//
// Who owns what. Rust (this block) owns the NUMBERS and the SCHEDULE: the
// lattice (absolute equal-angle equirect, no anchor), the ground cell in
// f64, the active level range from the footprint law, the toroidal scroll
// rects when the ground cell moves, the time-based fill / refresh / global
// cadence, the calibration re-run rule, the pads (`light2_color`), the
// atlas and its views / bind groups, the four pipelines and the passes.
// WGSL (40-clouds.wgsl, 45-cloud-temporal.wgsl) owns the bake fragments,
// the calibration fragments, the mip fragment and the read side.
//
// Nothing view-dependent enters the bake: every texel is decoded from its
// own storage position and the ground cell, at the CELL'S OWN direction
// (BUG-074 stays dead by construction).

/// Finest window cell, km of arc (level L cell = CELL0 * 2^L: 0.25 .. 8 km).
pub const CLOUD_FR_CELL0_KM: f32 = 0.25;
/// Window levels (0.25 to 8 km cells).
pub const CLOUD_FR_LEVELS: u32 = 6;
/// lodb of level 0. WGSL-owned; the sync test asserts it equals log2(CELL0).
pub const CLOUD_FR_LOD0: f32 = -2.0;
/// Window cells across (and down): each level's window is NX x NX cells.
pub const CLOUD_FR_NX: u32 = 512;
/// Slab height bins.
pub const CLOUD_FR_NZ: u32 = 12;
/// Pair slices per level: (f_k, G_k, f_k+1, G_k+1) for k = 2p.
pub const CLOUD_FR_PAIRS: u32 = 6;
/// Column slices per level: (C_4q .. C_4q+3), the last carrying T in .w.
pub const CLOUD_FR_CSLICES: u32 = 3;
/// Pair + column slices per level.
pub const CLOUD_FR_SLICES_PER_LEVEL: u32 = 9;
/// Slices per atlas row (12 x 512 = 6144).
pub const CLOUD_FR_SLICE_COLS: u32 = 12;
/// Column encoding scale: enc(C) = sqrt(C / 12), dec(v) = v * v * 12.
pub const CLOUD_FR_COL_SCALE: f32 = 12.0;
/// Atlas width at mip 0.
pub const CLOUD_FR_ATLAS_W: u32 = 6144;
/// Atlas height at mip 0 (window band 2560 + global 1024).
pub const CLOUD_FR_ATLAS_H: u32 = 3584;
/// Global equirect map width (one slice).
pub const CLOUD_FR_GLOBAL_W: u32 = 2048;
/// Global equirect map height.
pub const CLOUD_FR_GLOBAL_H: u32 = 1024;
/// Atlas row where the global region starts (5 * 2^9: mip-aligned).
pub const CLOUD_FR_GLOBAL_Y0: u32 = 2560;
/// Pooled height bins of the global (three slab bins each).
pub const CLOUD_FR_GLOBAL_NZ: u32 = 4;
/// Atlas mip count; mips 1..6 hold the global region only.
pub const CLOUD_FR_GLOBAL_MIPS: u32 = 7;
/// Final calibration table origin, MIP-1 texel coordinates (32 x 4).
pub const CLOUD_FR_CALIB_X0: u32 = 1536;
pub const CLOUD_FR_CALIB_Y0: u32 = 1024;
/// Calibration table height rows (one per cloud-relative height band) and
/// stratified seeds per archetype: "both" constants, read by the two
/// calibration fragments and pinned against the shader text by the sync
/// test, so the Rust scissors below can never shrink under the shader.
pub const CLOUD_FR_CALIB_ROWS: u32 = 32;
pub const CLOUD_FR_CALIB_SEEDS: u32 = 8;
/// Archetypes in the calibration table (humilis, congestus, stratocumulus,
/// cumulonimbus). The WGSL calibration fragments bound their archetype
/// index with a literal 4 today; if a fifth archetype is ever added the
/// shader AND this constant change together (the scissors derive from it).
pub const CLOUD_FR_CALIB_ARCHETYPES: u32 = 4;
/// Calibration table size in mip-1 texels: one column per height row, one
/// row per archetype. DERIVED, never retyped: the scissor of the reduce
/// pass is exactly what the reduce fragment bounds-checks.
pub const CLOUD_FR_CALIB_W: u32 = CLOUD_FR_CALIB_ROWS;
pub const CLOUD_FR_CALIB_H: u32 = CLOUD_FR_CALIB_ARCHETYPES;
/// Per-seed staging origin, MIP-2 texel coordinates (32 x 32).
pub const CLOUD_FR_CALIB_STAGE_X0: u32 = 768;
pub const CLOUD_FR_CALIB_STAGE_Y0: u32 = 512;
/// Staging area size in mip-2 texels: one column per height row, one row
/// per (archetype, seed) pair. DERIVED from the same three constants.
pub const CLOUD_FR_CALIB_STAGE_W: u32 = CLOUD_FR_CALIB_ROWS;
pub const CLOUD_FR_CALIB_STAGE_H: u32 = CLOUD_FR_CALIB_ARCHETYPES * CLOUD_FR_CALIB_SEEDS;
/// Wall seconds per full rolling refresh of one active level; also the
/// duration of the FAST (first or re-referenced) global pass. Rust-only.
pub const CLOUD_FR_REFRESH_S: f64 = 2.0;
/// Wall seconds per rolling refresh of the global map. Rust-only.
pub const CLOUD_FR_GLOBAL_REFRESH_S: f64 = 60.0;
/// Storage rows per frame per level during a FILL (1/8 of the level).
pub const CLOUD_FR_FILL_ROWS: u32 = 64;
/// D5 (the bake cadence at low frame rates, 2026-09-06): the MOST rows one
/// level's rolling REFRESH may bake in one frame, CLOUD_FR_FILL_ROWS / 4.
/// The refresh is time based (ceil(512 * dt / CLOUD_FR_REFRESH_S) rows), so
/// at a low frame rate one frame asked for many rows: at the rig's 4 fps
/// (dt 0.25 s) that was 64 rows per level per frame, the whole per-second
/// bake cost landing on every frame (gpu.cloud_profile 17.8 ms per frame at
/// bm12). With this cap the per-frame cost is bounded by the FRAME, not by
/// the second: at 60 fps (5 rows per frame) the 2 s refresh is unchanged;
/// at 4 fps a level's refresh stretches to 512 / 16 = 32 frames, about 8 s,
/// which is harmless (clouds evolve slowly; the old map serves meanwhile).
/// The FILL keeps its own 64-row cap (a fill is a one-off, and the level is
/// invisible until it completes). Rust-only.
pub const CLOUD_FR_REFRESH_ROWS_MAX: u32 = CLOUD_FR_FILL_ROWS / 4;
/// D5: the most rows the GLOBAL pass may bake in one frame (the fast pass
/// and the rolling pass alike; REF mode keeps its own 2). At 60 fps the fast
/// 2 s pass asked for ceil(1024 / 120) = 9 rows per frame and now takes 8
/// (128 frames, 2.13 s); at 4 fps it asked for 128 rows per frame (a full
/// 6144-wide, 128-row bake on every frame) and now takes 8 (32 s for the
/// fast pass at that frame rate, bounded per frame). Rust-only.
pub const CLOUD_FR_GLOBAL_ROWS_MAX: u32 = 8;
/// D5, the fast pass exception: the FIRST global pass (and every
/// re-referenced one) has to COMPLETE inside a capture window or the map
/// never becomes valid, the Low sheet falls back to the old weather path and
/// every global-fed gate reads a picture the profile never drew. The rig runs
/// at 2 to 4 fps and the profile fixtures settle 12 to 14 s, so 8 rows a frame
/// (128 frames, 32 to 64 s) is too slow: the fast pass gets 32 rows (at most
/// 32 frames, about 10 s at 3 fps, still a quarter of the uncapped 128-row
/// spike). The rolling 60 s pass keeps the tight bound. Rust-only.
pub const CLOUD_FR_GLOBAL_FAST_ROWS_MAX: u32 = 32;
/// Storage rows per frame in REF mode for the windows (the global uses 2).
pub const CLOUD_FR_REF_ROWS: u32 = 4;
/// Storage rows per frame in REF mode for the global.
pub const CLOUD_FR_REF_GLOBAL_ROWS: u32 = 2;
/// Cloud-clock JUMP (seconds) that re-references the global: the rolling
/// pass restarts t_ref every 60 s, so only a scrub or a pin change trips it.
pub const CLOUD_FR_GLOBAL_REREF_S: f64 = 120.0;
/// Coverage change that re-references the global.
pub const CLOUD_FR_COVERAGE_REREF: f32 = 0.02;
// Knob codes (pad light2_color.z), both languages.
/// Off: today's field, bit-identical (the A/B twin).
pub const CLOUD_FR_KNOB_OFF: i32 = 0;
/// Automatic level by lodb, blended across levels and edges.
pub const CLOUD_FR_KNOB_ON: i32 = 1;
/// Level 0..5 forced at w = 1 on every sample (Rust keeps it active).
pub const CLOUD_FR_KNOB_FORCE0: i32 = 2;
pub const CLOUD_FR_KNOB_FORCE1: i32 = 3;
pub const CLOUD_FR_KNOB_FORCE2: i32 = 4;
pub const CLOUD_FR_KNOB_FORCE3: i32 = 5;
pub const CLOUD_FR_KNOB_FORCE4: i32 = 6;
pub const CLOUD_FR_KNOB_FORCE5: i32 = 7;
/// Automatic level, hard switch, no blend anywhere (the prove-red).
pub const CLOUD_FR_KNOB_HARD: i32 = 8;
/// The reference bake (dev only, slow: 128 frames per level).
pub const CLOUD_FR_KNOB_REF: i32 = 9;

/// D3 (the marched field empties from above, 2026-09-06): flags-pad bit 12
/// (4096) of `light2_color.w` = the BUILT-BODY TOP BOUND dev bit. When it is
/// on, `cloud_v2_body`'s vertical reject publishes the gap to the admitted
/// density region as a from-above SDF lower bound (so the march's stride
/// finds a thin humilis under a 928 m comb instead of a coin flip), and the
/// step economy's in-cloud floor is capped at a quarter of the found cloud's
/// height (so a found cloud is integrated, not skimmed). Off = today's comb,
/// the A/B twin. Read in WGSL as `cloud_profile_flag(12)` =
/// `cloud_top_bound_on()`. It is INDEPENDENT of the profile knob and of the
/// atlas: the pad's flags lane carries it even when no atlas exists (the
/// bits 0..8 above are then all zero), which is why the gate's cells run it
/// at `cloud_profile` 0. Bits 0..8 are the validity bits; bits 13..15 are
/// the component bisect on a different pad; 12 was free.
pub const CLOUD_FR_FLAG_TOP_BOUND: u32 = 1 << 12;

/// OR the D3 top-bound dev bit into a flags value (`cloud_fr_flags` or the
/// zero pad of a frame without an atlas). Exact in f32: the largest value is
/// 4096 + 511, far below 2^24.
pub fn cloud_fr_flags_with_top_bound(flags: u32, top_bound: bool) -> u32 {
    if top_bound {
        flags | CLOUD_FR_FLAG_TOP_BOUND
    } else {
        flags
    }
}

/// The per-frame inputs lib.rs feeds the profile from the cloud fill block,
/// on EVERY tier the cloud shell exists (the Low sheet needs the global).
/// All the planet-scale values are f64 (the v0.1238 lesson: never subtract
/// planet-scale quantities in f32); the pad narrows to exact integers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudProfileFrame {
    /// The camera's ground longitude / latitude in the planet-local frame,
    /// radians, from `p_l`: `lat = asin(y / |p|)`, `lon = atan2(-z, x)`
    /// (the same lines `cloud_v2_body` uses for its cells).
    pub ground_lon_rad: f64,
    pub ground_lat_rad: f64,
    /// Camera altitude above the sphere, km (from `cam_r_ratio`).
    pub alt_km: f64,
    /// Planet radius, km (`d.radius / 1000`; the shader's params2.z).
    pub radius_km: f64,
    /// Slab base / top as multiples of the planet radius (params2.x / .y).
    pub slab_rb: f64,
    pub slab_rt: f64,
    /// The MARCH texture's pixel angle: `2 tan(fov/2) / rows` with rows =
    /// `config.height / cloud_res_div` (the temporal march derives its
    /// footprint from its own rasterizer, so the active-level range must
    /// use the same value, not the screen's).
    pub pix_ang_march: f64,
    /// The cloud clock as written to sun_color.w (pinned or live).
    pub cloud_t: f32,
    /// Effective coverage (`cov_eff`, base_color.a).
    pub coverage: f32,
    /// The placement / type pin (`pin` before the temporal +4).
    pub type_pin: f32,
    /// Counter bumped at every weather-map upload.
    pub weather_gen: u32,
    /// Cloud quality tier (params.y): 0 low, 1 medium, 2 high, 3 ultra.
    pub tier: f32,
    /// The knob code (CLOUD_FR_KNOB_*).
    pub knob: i32,
    /// Hash of (tier, interior saturation, wide-edge bit, bisect index):
    /// a change re-runs the calibration and refills every level.
    pub calib_key: u32,
}

impl CloudProfileFrame {
    /// The march pixel angle for a given screen height, divisor and fov
    /// (the contract's `pix_ang_march = 2 tan(fov/2) / max(height / div, 8)`).
    pub fn pix_ang_march(fov_degrees: f64, height_px: u32, res_div: u32) -> f64 {
        let rows = (height_px / res_div.clamp(1, 4)).max(8) as f64;
        2.0 * (fov_degrees.to_radians() * 0.5).tan() / rows
    }
}

// ── The lattice (contract: "The data: lattice, bins, texels, atlas") ──

/// Cell size of level L in km of arc.
pub fn cloud_fr_cell_km(level: u32) -> f64 {
    CLOUD_FR_CELL0_KM as f64 * (1u32 << level) as f64
}

/// Angular cell size of level L in radians: `c_L / planet_km`.
pub fn cloud_fr_cell_rad(level: u32, planet_km: f64) -> f64 {
    cloud_fr_cell_km(level) / planet_km
}

/// Cells around the planet at level L: `floor(2 pi planet_km / c_L)`,
/// computed in f32 exactly as the shader does (`floor(TAU * planet_km /
/// c_km)` with `c_km = CELL0 * exp2(L)`) so both sides are bit-identical.
pub fn cloud_fr_n_i(level: u32, planet_km: f32) -> u32 {
    let c_km = CLOUD_FR_CELL0_KM * (level as f32).exp2();
    (std::f32::consts::TAU * planet_km / c_km).floor().max(1.0) as u32
}

/// Rows pole to pole at level L: `floor(pi planet_km / c_L)`, f32 like the shader.
pub fn cloud_fr_n_j(level: u32, planet_km: f32) -> u32 {
    let c_km = CLOUD_FR_CELL0_KM * (level as f32).exp2();
    (std::f32::consts::PI * planet_km / c_km).floor().max(1.0) as u32
}

/// The ground CELL of level 0 from the ground direction: RAW indices in
/// `[0, N_I(0))` x `[0, N_J(0))`, never unwrapped (a date-line crossing is
/// handled by a full refill, see `CloudProfileState::plan`). f64 in, exact
/// integers out.
pub fn cloud_fr_ground_cell(lon_rad: f64, lat_rad: f64, planet_km: f64) -> (i64, i64) {
    let cell = cloud_fr_cell_rad(0, planet_km);
    let ni = cloud_fr_n_i(0, planet_km as f32) as i64;
    let nj = cloud_fr_n_j(0, planet_km as f32) as i64;
    let i = ((lon_rad + std::f64::consts::PI) / cell).floor() as i64;
    let j = ((lat_rad + std::f64::consts::FRAC_PI_2) / cell).floor() as i64;
    (i.clamp(0, ni - 1), j.clamp(0, nj - 1))
}

/// The window origin `(I0_L, J0_L)` of level L from the level-0 ground
/// cell: `ground_I_L = floor(ground_I_0 / 2^L)`, `I0_L = ground_I_L - NX/2`
/// (the same arithmetic the shader does with `floor(pad.x / exp2(L))`).
pub fn cloud_fr_window_origin(ground_0: (i64, i64), level: u32) -> (i64, i64) {
    let half = (CLOUD_FR_NX / 2) as i64;
    let sh = level as i64;
    // Non-negative raw indices, so >> is the floor division.
    ((ground_0.0 >> sh) - half, (ground_0.1 >> sh) - half)
}

/// Positive modulus (the toroidal storage rule): `((a mod n) + n) mod n`.
pub fn cloud_fr_pmod(a: i64, n: i64) -> i64 {
    ((a % n) + n) % n
}

/// Atlas origin `(x0, y0)` of slice `s = L * 9 + p` (mip 0 texels):
/// 12 slices per atlas row, five rows.
pub fn cloud_fr_slice_origin(level: u32, slice: u32) -> (u32, u32) {
    let s = level * CLOUD_FR_SLICES_PER_LEVEL + slice;
    ((s % CLOUD_FR_SLICE_COLS) * CLOUD_FR_NX, (s / CLOUD_FR_SLICE_COLS) * CLOUD_FR_NX)
}

/// Split a run of `n` consecutive window-frame indices starting at
/// `start` into storage runs `(offset, len)` inside a 512-wide toroidal
/// slice: one run, or two when the run wraps the slice edge. `n <= NX`.
pub fn cloud_fr_storage_runs(start: i64, n: u32) -> Vec<(u32, u32)> {
    let nx = CLOUD_FR_NX;
    let n = n.min(nx);
    if n == 0 {
        return Vec::new();
    }
    let a = cloud_fr_pmod(start, nx as i64) as u32;
    if a + n <= nx {
        vec![(a, n)]
    } else {
        vec![(a, nx - a), (0, a + n - nx)]
    }
}

/// The active level range from the footprint law (contract: `plan`).
/// `foot_min = max(alt - slab_top, 0) * pix_ang`, `foot_max = (horizon of
/// the camera + horizon of the slab top) * pix_ang`, both in km; level L is
/// active iff `[L - 1, L + 1)` meets `[log2(foot_min) - LOD0, log2(foot_max)
/// - LOD0]`. The forced level (knob 2..7) is always active.
pub fn cloud_fr_active_levels(frame: &CloudProfileFrame) -> [bool; 6] {
    let r = frame.radius_km;
    let top_km = (frame.slab_rt - 1.0) * r;
    let alt = frame.alt_km.max(0.0);
    let foot_min = (alt - top_km).max(0.0) * frame.pix_ang_march;
    let horiz_cam = ((r + alt) * (r + alt) - r * r).max(0.0).sqrt();
    let horiz_top = ((r + top_km) * (r + top_km) - r * r).max(0.0).sqrt();
    let foot_max = (horiz_cam + horiz_top) * frame.pix_ang_march;
    // log2(0) = -inf: a camera inside the slab has no lower bound.
    let lo = if foot_min > 0.0 { foot_min.log2() - CLOUD_FR_LOD0 as f64 } else { f64::NEG_INFINITY };
    let hi = if foot_max > 0.0 { foot_max.log2() - CLOUD_FR_LOD0 as f64 } else { f64::NEG_INFINITY };
    let forced = cloud_fr_forced_level(frame.knob);
    let mut out = [false; 6];
    for (l, slot) in out.iter_mut().enumerate() {
        let lf = l as f64;
        // [L - 1, L + 1) meets [lo, hi] iff lo < L + 1 and hi >= L - 1.
        *slot = (lo < lf + 1.0 && hi >= lf - 1.0) || forced == Some(l as u32);
    }
    out
}

/// The forced level of a FORCE knob, else None.
pub fn cloud_fr_forced_level(knob: i32) -> Option<u32> {
    if (CLOUD_FR_KNOB_FORCE0..=CLOUD_FR_KNOB_FORCE5).contains(&knob) {
        Some((knob - CLOUD_FR_KNOB_FORCE0) as u32)
    } else {
        None
    }
}

/// The flags pad: bit 0 = some window level valid, bit 1 = global valid,
/// bits 2..7 = level L = b - 2 valid, bit 8 = calibration valid. An exact
/// integer in f32 (the shader isolates bits with a scaled fract).
pub fn cloud_fr_flags(levels_valid: [bool; 6], global_valid: bool, calib_valid: bool) -> u32 {
    let mut f = 0u32;
    if levels_valid.iter().any(|&v| v) {
        f |= 1;
    }
    if global_valid {
        f |= 2;
    }
    for (l, &v) in levels_valid.iter().enumerate() {
        if v {
            f |= 1 << (2 + l);
        }
    }
    if calib_valid {
        f |= 1 << 8;
    }
    f
}

/// Worst-case cv2 candidate count of a texel whose cell is `c_lat` km
/// north-south at latitude `lat` (the contract's dev counter formula with
/// the 1.1 km humilis grid): `(ceil(c_lat / 1.1) + 2) * (ceil(c_lat cos(lat)
/// / 1.1) + 2)`.
fn cloud_fr_worst_candidates(c_lat_km: f64, lat_rad: f64) -> f64 {
    let g = 1.1;
    ((c_lat_km / g).ceil() + 2.0) * ((c_lat_km * lat_rad.cos().max(0.0) / g).ceil() + 2.0)
}

// ── Planning state (GPU-free, unit-tested; interior mutability because the
// bake pass consumes it on `&self`, like CloudLightState) ──

use std::cell::Cell;

/// One window level's schedule.
#[derive(Default, Debug)]
pub struct CloudProfileLevel {
    /// Window origin `(I0_L, J0_L)` in level-L cells; None = no window (the
    /// level is inactive, or has never been planned).
    pub origin: Cell<Option<(i64, i64)>>,
    /// In the active range this frame (or forced).
    pub active: Cell<bool>,
    /// The first full fill completed: the shader may read this level.
    pub valid: Cell<bool>,
    /// Rows baked so far in the current FILL (None = not filling).
    pub fill_cursor: Cell<Option<u32>>,
    /// Rolling refresh cursor, storage row (fractional by the contract's
    /// type; advanced by whole rows here).
    pub refresh_cursor: Cell<f64>,
    /// The scroll delta `(dI, dJ)` pending for this frame's scroll rects
    /// (set by `plan`, consumed by `take_bake_rects`).
    pub scroll: Cell<(i64, i64)>,
    /// Bookkeeping for the 1 Hz line: columns + rows scrolled, fills started.
    pub scrolled: Cell<u32>,
    pub fills: Cell<u32>,
}

/// The whole schedule: six levels, the global, the calibration.
#[derive(Default, Debug)]
pub struct CloudProfileState {
    pub levels: [CloudProfileLevel; 6],
    /// The global's first pass has been ordered.
    pub global_started: bool,
    /// Storage row the global's rolling / fast pass is at.
    pub global_pass_cursor: Cell<f64>,
    /// The current pass is a FAST one (2 s), else rolling (60 s).
    pub global_pass_fast: Cell<bool>,
    /// The global has completed a pass (plus its mips): readable.
    pub global_valid: Cell<bool>,
    /// The cloud clock the global was last (re)referenced or completed at.
    pub global_t_ref: Cell<f32>,
    /// The references a re-reference compares against.
    pub coverage_ref: f32,
    pub pin_ref: f32,
    pub weather_ref: u32,
    pub tier_ref: f32,
    /// Knob CLASS (1 = analytic, 9 = the reference bake).
    pub knob_class_ref: i32,
    /// A knob-off frame (or a stale mark) orders the next plan to fast-pass
    /// the global even though nothing else changed.
    pub global_reref_pending: bool,
    /// Set when a global pass completes: the six mip passes run this frame.
    pub mips_pending: Cell<bool>,
    /// The calibration table is baked and its flag may be raised.
    pub calib_valid: Cell<bool>,
    /// The calibration must run (first use, or its key changed).
    pub calib_pending: Cell<bool>,
    /// The calibration ran THIS frame: the bake waits one frame so the
    /// pad's bit 8 lands before any texel is baked from the table.
    pub calib_ran: Cell<bool>,
    pub calib_key_ref: Option<u32>,
    /// The frame the pads were last computed with.
    pub frame: Option<CloudProfileFrame>,
    /// Level-0 ground cell of the last plan.
    pub ground_0: (i64, i64),
    /// Counters for the 1 Hz line.
    pub global_passes: Cell<u32>,
    pub calib_runs: Cell<u32>,
    /// Wall clock of the last bake, for dt.
    pub last_bake: Cell<Option<std::time::Instant>>,
}

impl CloudProfileState {
    /// Decide this frame's windows and passes. Pure planning: no GPU work.
    ///
    /// Per level (contract): a level that was inactive, or whose origin
    /// moved by 512 cells or more (a date-line crossing produces this), or
    /// a calibration / tier / knob-class change, starts a FILL (`valid`
    /// false until it completes); otherwise the move becomes scroll rects.
    /// The global: first use, a cloud-clock JUMP, a weather-map upload, a
    /// coverage change over 0.02, a type-pin, tier or knob-class change each
    /// start a FAST pass; `valid` stays as it was during a re-reference (the
    /// old map serves until the new pass completes), except on first use.
    pub fn plan(&mut self, frame: CloudProfileFrame) {
        let planet_km = frame.radius_km;
        let ground_0 = cloud_fr_ground_cell(frame.ground_lon_rad, frame.ground_lat_rad, planet_km);
        self.ground_0 = ground_0;
        // The calibration: run once, re-run on a key change; a re-run refills
        // every active level and fast-passes the global.
        let calib_change = self.calib_key_ref != Some(frame.calib_key);
        if calib_change {
            self.calib_key_ref = Some(frame.calib_key);
            self.calib_pending.set(true);
            self.calib_valid.set(false);
            self.calib_runs.set(self.calib_runs.get().wrapping_add(1));
        }
        let class = if frame.knob == CLOUD_FR_KNOB_REF { CLOUD_FR_KNOB_REF } else { CLOUD_FR_KNOB_ON };
        let first = self.frame.is_none();
        let tier_change = !first && self.tier_ref != frame.tier;
        let class_change = !first && self.knob_class_ref != class;
        let refill_all = calib_change || tier_change || class_change;
        let active = cloud_fr_active_levels(&frame);
        for (l, lv) in self.levels.iter_mut().enumerate() {
            lv.active.set(active[l]);
            if !active[l] {
                // An inactive level drops its window: the shader must never
                // read a window that is not being maintained, and the level
                // refills when it comes back.
                lv.origin.set(None);
                lv.valid.set(false);
                lv.fill_cursor.set(None);
                lv.scroll.set((0, 0));
                continue;
            }
            let new_origin = cloud_fr_window_origin(ground_0, l as u32);
            let start_fill = match lv.origin.get() {
                None => true,
                Some(prev) => {
                    // The scroll delta ACCUMULATES until the bake pass
                    // consumes it (take_bake_rects), never overwrites: a
                    // frame that plans but does not bake (the calibration
                    // frame, or any future skipped pass) would otherwise
                    // lose the columns it exposed while the level stayed
                    // valid, and a cell 512 cells stale would read as
                    // data. The accumulated delta is measured from the
                    // origin at the LAST BAKE, so the fill rule applies to
                    // the sum: 512 or more cells since the last bake means
                    // nothing of the old window survives.
                    let (p_i, p_j) = lv.scroll.get();
                    let d_i = p_i + (new_origin.0 - prev.0);
                    let d_j = p_j + (new_origin.1 - prev.1);
                    if d_i.abs() >= CLOUD_FR_NX as i64 || d_j.abs() >= CLOUD_FR_NX as i64 || refill_all {
                        true
                    } else {
                        if (d_i, d_j) != (p_i, p_j) {
                            lv.scrolled.set(lv.scrolled.get().wrapping_add(
                                ((d_i - p_i).abs() + (d_j - p_j).abs()) as u32,
                            ));
                        }
                        lv.scroll.set((d_i, d_j));
                        false
                    }
                }
            };
            if start_fill {
                // A fill rewrites the whole window: any pending scroll is
                // covered by it.
                lv.scroll.set((0, 0));
                lv.valid.set(false);
                lv.fill_cursor.set(Some(0));
                lv.refresh_cursor.set(0.0);
                lv.fills.set(lv.fills.get().wrapping_add(1));
            }
            lv.origin.set(Some(new_origin));
        }
        // The global.
        let clock_jump = (frame.cloud_t - self.global_t_ref.get()).abs() as f64 > CLOUD_FR_GLOBAL_REREF_S;
        let need_fast = !self.global_started
            || self.global_reref_pending
            || clock_jump
            || frame.weather_gen != self.weather_ref
            || (frame.coverage - self.coverage_ref).abs() > CLOUD_FR_COVERAGE_REREF
            || frame.type_pin != self.pin_ref
            || tier_change
            || class_change
            || calib_change;
        if need_fast {
            if !self.global_started {
                self.global_valid.set(false);
            }
            self.global_started = true;
            self.global_reref_pending = false;
            self.global_pass_fast.set(true);
            self.global_pass_cursor.set(0.0);
            self.global_t_ref.set(frame.cloud_t);
            self.coverage_ref = frame.coverage;
            self.pin_ref = frame.type_pin;
            self.weather_ref = frame.weather_gen;
        }
        self.tier_ref = frame.tier;
        self.knob_class_ref = class;
        self.frame = Some(frame);
    }

    /// Mark everything STALE: called for every frame the atlas exists but
    /// the knob is off (or no cloud body is armed), so that turning the
    /// profile back on refills every window and fast-passes the global
    /// instead of serving slices from an older cloud clock. The global's
    /// `valid` is kept (the old map serves for the 2 s of the fast pass).
    pub fn mark_stale(&mut self) {
        for lv in self.levels.iter() {
            lv.origin.set(None);
            lv.valid.set(false);
            lv.fill_cursor.set(None);
            lv.scroll.set((0, 0));
        }
        if self.global_started {
            self.global_reref_pending = true;
        }
        self.last_bake.set(None);
    }

    /// Whether the shader may read anything: some level or the global valid.
    pub fn any_valid(&self) -> bool {
        self.levels.iter().any(|l| l.valid.get()) || self.global_valid.get()
    }

    pub fn levels_valid(&self) -> [bool; 6] {
        let mut v = [false; 6];
        for (i, l) in self.levels.iter().enumerate() {
            v[i] = l.valid.get();
        }
        v
    }

    /// The `light2_color` pad: `(ground_I_0, ground_J_0, knob, flags)`, all
    /// exact integers in f32. Zeros when nothing was planned.
    pub fn pads(&self) -> [f32; 4] {
        let Some(f) = self.frame else {
            return [0.0; 4];
        };
        let flags = cloud_fr_flags(self.levels_valid(), self.global_valid.get(), self.calib_valid.get());
        [self.ground_0.0 as f32, self.ground_0.1 as f32, f.knob as f32, flags as f32]
    }

    /// The calibration passes run this frame? Consumes the pending order;
    /// after this frame the table is valid (bit 8 rises on the next pad
    /// write) and the bake waits one frame (see `take_bake_rects`).
    pub fn take_calib(&self) -> bool {
        if self.calib_pending.replace(false) {
            self.calib_valid.set(true);
            self.calib_ran.set(true);
            true
        } else {
            false
        }
    }

    /// Scissor rects `(x, y, w, h)` for this frame's bake pass, in the
    /// contract's order: (1) scroll rects for every active level with a
    /// window, (2) fill rects, (3) refresh rects for every valid active
    /// level, (4) the global's pass rows. `dt_s` = wall seconds since the
    /// last bake (the cadence is TIME based, gated on its MAX). Called on
    /// `&self` by the bake pass.
    pub fn take_bake_rects(&self, dt_s: f64) -> Vec<(u32, u32, u32, u32)> {
        let mut out = Vec::new();
        // The frame the calibration ran: pads carried bit 8 = 0, so no texel
        // is baked from the table until the flag has landed (one frame).
        if self.calib_ran.replace(false) {
            return out;
        }
        let Some(frame) = self.frame else {
            return out;
        };
        let ref_mode = frame.knob == CLOUD_FR_KNOB_REF;
        let nx = CLOUD_FR_NX;
        let dt = dt_s.clamp(0.0, 1.0);
        for (l, lv) in self.levels.iter().enumerate() {
            if !lv.active.get() {
                continue;
            }
            let Some(origin) = lv.origin.get() else {
                continue;
            };
            let level = l as u32;
            // (1) Scroll rects: the columns / rows the window exposed when
            // the origin moved, as toroidal storage runs, for all 9 slices.
            let (d_i, d_j) = lv.scroll.replace((0, 0));
            if d_i != 0 {
                let n = d_i.unsigned_abs() as u32;
                // dI > 0: I_abs in [I0_new + NX - dI, I0_new + NX); dI < 0: [I0_new, I0_new - dI).
                let start = if d_i > 0 { origin.0 + nx as i64 - d_i } else { origin.0 };
                for (off, len) in cloud_fr_storage_runs(start, n) {
                    for p in 0..CLOUD_FR_SLICES_PER_LEVEL {
                        let (x0, y0) = cloud_fr_slice_origin(level, p);
                        out.push((x0 + off, y0, len, nx));
                    }
                }
            }
            if d_j != 0 {
                let n = d_j.unsigned_abs() as u32;
                let start = if d_j > 0 { origin.1 + nx as i64 - d_j } else { origin.1 };
                for (off, len) in cloud_fr_storage_runs(start, n) {
                    for p in 0..CLOUD_FR_SLICES_PER_LEVEL {
                        let (x0, y0) = cloud_fr_slice_origin(level, p);
                        out.push((x0, y0 + off, nx, len));
                    }
                }
            }
            // (2) Fill rects: FILL_ROWS (REF: REF_ROWS) storage rows at the
            // fill cursor; the level becomes valid when the cursor reaches NX.
            if let Some(c) = lv.fill_cursor.get() {
                let rows = if ref_mode { CLOUD_FR_REF_ROWS } else { CLOUD_FR_FILL_ROWS };
                let n = rows.min(nx - c);
                for p in 0..CLOUD_FR_SLICES_PER_LEVEL {
                    let (x0, y0) = cloud_fr_slice_origin(level, p);
                    out.push((x0, y0 + c, nx, n));
                }
                if c + n >= nx {
                    lv.fill_cursor.set(None);
                    lv.valid.set(true);
                    lv.refresh_cursor.set(0.0);
                } else {
                    lv.fill_cursor.set(Some(c + n));
                }
                continue;
            }
            // (3) Refresh rects: ceil(NX * dt / REFRESH_S) rows (at most
            // REFRESH_ROWS_MAX; REF: REF_ROWS) at the refresh cursor, wrapping.
            // D5: the cap was FILL_ROWS (64), which a 0.25 s frame reached
            // outright, so the per-frame bake cost scaled with 1 / fps; now
            // the cap is 16 rows and the cost is bounded per FRAME (the
            // refresh takes longer at a low frame rate instead of costing
            // more per frame; see CLOUD_FR_REFRESH_ROWS_MAX).
            if lv.valid.get() {
                let n = if ref_mode {
                    CLOUD_FR_REF_ROWS
                } else {
                    ((nx as f64 * dt / CLOUD_FR_REFRESH_S).ceil() as u32).clamp(1, CLOUD_FR_REFRESH_ROWS_MAX)
                };
                let start = lv.refresh_cursor.get().floor() as i64;
                for (off, len) in cloud_fr_storage_runs(start, n) {
                    for p in 0..CLOUD_FR_SLICES_PER_LEVEL {
                        let (x0, y0) = cloud_fr_slice_origin(level, p);
                        out.push((x0, y0 + off, nx, len));
                    }
                }
                lv.refresh_cursor.set(((start + n as i64) % nx as i64) as f64);
            }
        }
        // (4) The global: n rows at the pass cursor across all three slices
        // (width 6144); a completed pass sets mips_pending and valid.
        // D5: the row count is capped per frame (the fast pass at
        // CLOUD_FR_GLOBAL_FAST_ROWS_MAX = 32 so it still completes inside a
        // capture window, the rolling pass at CLOUD_FR_GLOBAL_ROWS_MAX = 8) per
        // frame (it was clamped only to the whole 1024-row map, so a 0.25 s
        // frame baked 128 rows of 6144 texels in one go); the pass takes
        // more frames at a low frame rate instead of costing more per frame.
        if self.global_started {
            let fast = self.global_pass_fast.get();
            let n = if ref_mode {
                CLOUD_FR_REF_GLOBAL_ROWS
            } else {
                let span = if fast { CLOUD_FR_REFRESH_S } else { CLOUD_FR_GLOBAL_REFRESH_S };
                let cap = if fast { CLOUD_FR_GLOBAL_FAST_ROWS_MAX } else { CLOUD_FR_GLOBAL_ROWS_MAX };
                ((CLOUD_FR_GLOBAL_H as f64 * dt / span).ceil() as u32).clamp(1, cap)
            };
            let c = self.global_pass_cursor.get().floor() as u32;
            let n = n.min(CLOUD_FR_GLOBAL_H - c);
            out.push((0, CLOUD_FR_GLOBAL_Y0 + c, CLOUD_FR_ATLAS_W, n));
            if c + n >= CLOUD_FR_GLOBAL_H {
                self.global_pass_cursor.set(0.0);
                self.mips_pending.set(true);
                self.global_valid.set(true);
                self.global_pass_fast.set(false);
                self.global_t_ref.set(frame.cloud_t);
                self.global_passes.set(self.global_passes.get().wrapping_add(1));
            } else {
                self.global_pass_cursor.set((c + n) as f64);
            }
        }
        out
    }

    /// Analytic dev counter: the number of global-row and window-row texels
    /// whose worst-case cv2 candidate count exceeds CLOUD_FR_MAX_CV2 (512),
    /// i.e. where the bake's stride subsampling engages. 0 on Earth; a
    /// larger planet is where it engages. Printed at 1 Hz.
    pub fn truncated_texels_estimate(&self) -> u64 {
        let Some(f) = self.frame else {
            return 0;
        };
        let planet_km = f.radius_km;
        let cap = 512.0;
        let mut n = 0u64;
        // The global: 1024 rows of 2048 texels, cell = 2 pi R / 2048.
        let global_km = std::f64::consts::TAU * planet_km / CLOUD_FR_GLOBAL_W as f64;
        for j in 0..CLOUD_FR_GLOBAL_H {
            let lat = std::f64::consts::FRAC_PI_2 - (j as f64 + 0.5) / CLOUD_FR_GLOBAL_H as f64 * std::f64::consts::PI;
            if cloud_fr_worst_candidates(global_km, lat) > cap {
                n += CLOUD_FR_GLOBAL_W as u64;
            }
        }
        // The windows: each active level's 512 rows at their own latitudes.
        for (l, lv) in self.levels.iter().enumerate() {
            let Some(origin) = lv.origin.get() else {
                continue;
            };
            let level = l as u32;
            let c_km = cloud_fr_cell_km(level);
            let cell_rad = cloud_fr_cell_rad(level, planet_km);
            let nj = cloud_fr_n_j(level, planet_km as f32) as i64;
            for r in 0..CLOUD_FR_NX as i64 {
                let j_abs = origin.1 + r;
                if j_abs < 0 || j_abs >= nj {
                    continue;
                }
                let lat = (j_abs as f64 + 0.5) * cell_rad - std::f64::consts::FRAC_PI_2;
                if cloud_fr_worst_candidates(c_km, lat) > cap {
                    n += CLOUD_FR_NX as u64;
                }
            }
        }
        n
    }
}

/// The atlas, its views and bind groups, plus the planning state. Created
/// by `ensure_cloud_profile` on the first frame the knob is on; planned
/// every frame by `cloud_profile_plan` (from lib.rs) and consumed by the
/// passes in mod.rs (interior mutability, the passes run on `&self`).
pub struct CloudProfileCache {
    _texture: wgpu::Texture,
    /// All seven mips: the march, the shell and the Low sheet read this.
    pub view_all: wgpu::TextureView,
    /// One mip each: attachments, and the mip passes' read sources.
    pub view_mip: [wgpu::TextureView; 7],
    /// b0 = the 1x1 white fallback, b14 = the whole atlas: the march without
    /// the sun cache, the transparent shell draw, the Low sheet.
    pub group_plain: AlbedoBindGroup,
    /// b0 = the sun-shadow atlas, b14 = the whole atlas: the march when both
    /// caches are active. Rebuilt whenever either atlas is (re)created.
    pub group_sun: Option<AlbedoBindGroup>,
    /// b0 = white, b14 = mip m (m = 0..5): the mip passes (writing m + 1),
    /// the calibration reduce (`[2]`) and the bake (`[1]`).
    pub group_mip_src: [AlbedoBindGroup; 6],
    pub state: CloudProfileState,
}

impl CloudProfileCache {
    pub fn texture(&self) -> &wgpu::Texture {
        &self._texture
    }
}

impl Renderer {
    /// Create the profile atlas on first use. Refuses (logs once, stays
    /// `None`) when the device cannot hold a 6144-wide texture: the limits
    /// are requested `using_resolution(adapter)` so a desktop GPU grants
    /// 8192+, but a small-limit adapter must degrade to the point-sampled
    /// field, not die at create_texture (the v0.782 boot-killer class).
    pub fn ensure_cloud_profile(&mut self) {
        if self.cloud_profile_cache.is_some() {
            return;
        }
        let max_dim = self.device.limits().max_texture_dimension_2d;
        if max_dim < CLOUD_FR_ATLAS_W {
            static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                log::warn!(
                    "[CloudProfile] atlas {}x{} exceeds max_texture_dimension_2d {}: profile stays off",
                    CLOUD_FR_ATLAS_W, CLOUD_FR_ATLAS_H, max_dim
                );
            }
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Cloud Profile Atlas"),
            size: wgpu::Extent3d {
                width: CLOUD_FR_ATLAS_W,
                height: CLOUD_FR_ATLAS_H,
                depth_or_array_layers: 1,
            },
            // The full-extent chain is allocated (117 MB); mips 1..6 are
            // written only over the global region plus the two calibration
            // areas, the rest is never touched.
            mip_level_count: CLOUD_FR_GLOBAL_MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            // COPY_SRC for the dev dump (never let a dump be the first time
            // the flag is missed).
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view_all = tex.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Cloud Profile Atlas (all mips)"),
            ..Default::default()
        });
        let mip_view = |m: u32| {
            tex.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Cloud Profile Atlas (one mip)"),
                base_mip_level: m,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let view_mip = [
            mip_view(0), mip_view(1), mip_view(2), mip_view(3), mip_view(4), mip_view(5), mip_view(6),
        ];
        // Every cloud-side group rebuilt with the atlas at binding 14 (A8):
        // no layout change, 16 entries at every site.
        let group_plain = self.build_albedo_group_from_view_b14(&self.white_view, &self.albedo_sampler, &view_all);
        let group_sun = self
            .cloud_light_cache
            .as_ref()
            .map(|lc| self.build_albedo_group_from_view_b14(&lc.view, &self.albedo_sampler, &view_all));
        let mip_src = |m: usize| self.build_albedo_group_from_view_b14(&self.white_view, &self.albedo_sampler, &view_mip[m]);
        let group_mip_src = [mip_src(0), mip_src(1), mip_src(2), mip_src(3), mip_src(4), mip_src(5)];
        // The CloudScreen accumulation pair (unused by any pass today) also
        // carries b14 = the atlas, for consistency with every cloud group;
        // ensure_cloud_screen applies the same rule when the pair is
        // recreated later (a resize or a cloud_res change).
        let screen_groups = self.cloud_screen.as_ref().map(|cs| {
            [
                self.build_albedo_group_from_view_b14(&cs.views[0], &self.albedo_sampler, &view_all),
                self.build_albedo_group_from_view_b14(&cs.views[1], &self.albedo_sampler, &view_all),
            ]
        });
        if let (Some(g), Some(cs)) = (screen_groups, self.cloud_screen.as_mut()) {
            cs.groups = g;
        }
        self.cloud_profile_cache = Some(CloudProfileCache {
            _texture: tex,
            view_all,
            view_mip,
            group_plain,
            group_sun,
            group_mip_src,
            state: CloudProfileState::default(),
        });
        log::info!(
            "[CloudProfile] atlas ON: {}x{} RGBA8, {} mips ({:.1} MB mip 0), {} levels x {} slices of {}^2, global {}x{}",
            CLOUD_FR_ATLAS_W,
            CLOUD_FR_ATLAS_H,
            CLOUD_FR_GLOBAL_MIPS,
            (CLOUD_FR_ATLAS_W as f64 * CLOUD_FR_ATLAS_H as f64 * 4.0) / 1.0e6,
            CLOUD_FR_LEVELS,
            CLOUD_FR_SLICES_PER_LEVEL,
            CLOUD_FR_NX,
            CLOUD_FR_GLOBAL_W,
            CLOUD_FR_GLOBAL_H,
        );
    }

    /// Rebuild the profile's `group_sun` (b0 = the sun atlas, b14 = the
    /// profile atlas). Called whenever EITHER atlas is (re)created: from
    /// `ensure_cloud_profile` above and from `ensure_cloud_light`.
    pub(crate) fn rebuild_cloud_profile_sun_group(&mut self) {
        let g = match (self.cloud_light_cache.as_ref(), self.cloud_profile_cache.as_ref()) {
            (Some(lc), Some(pc)) => {
                Some(self.build_albedo_group_from_view_b14(&lc.view, &self.albedo_sampler, &pc.view_all))
            }
            _ => None,
        };
        if let (Some(g), Some(pc)) = (g, self.cloud_profile_cache.as_mut()) {
            pc.group_sun = Some(g);
        }
    }

    /// Per-frame feed from lib.rs (the cloud fill block, EVERY tier): create
    /// the atlas if the knob is on, plan the windows. With the knob off the
    /// frame is recorded (the pads carry knob 0, the shader's bit-identical
    /// branch) and the atlas is marked stale, so flipping the knob on
    /// refills. A second call in the same frame is ignored (two cloud
    /// bodies can never fight over the ground cell); `set_cloud_temporal
    /// (None)` at the top of every frame clears the feed.
    ///
    /// Returns whether THIS call's frame was accepted (true for the first
    /// call of the frame, false for a later body). lib.rs sets
    /// `cloud_shell_mat` only on acceptance, so the material the passes
    /// bind (planet radius, slab, params) is always the body whose ground
    /// cell was planned: a last-wins material beside a first-wins plan
    /// would bake body A's lattice with body B's parameters into a window
    /// that persists across frames.
    pub fn cloud_profile_plan(&mut self, frame: CloudProfileFrame) -> bool {
        if self.cloud_profile_frame.is_some() {
            return false;
        }
        self.cloud_profile_frame = Some(frame);
        if frame.knob == CLOUD_FR_KNOB_OFF {
            if let Some(pc) = self.cloud_profile_cache.as_mut() {
                pc.state.mark_stale();
                pc.state.frame = Some(frame);
            }
            return true;
        }
        self.ensure_cloud_profile();
        if let Some(pc) = self.cloud_profile_cache.as_mut() {
            pc.state.plan(frame);
        }
        true
    }

    /// The profile the march may READ this frame: the knob is on, the frame
    /// was fed, the atlas exists and some level or the global is valid.
    pub(crate) fn cloud_profile_active(&self) -> Option<&CloudProfileCache> {
        if self.cloud_profile_knob == CLOUD_FR_KNOB_OFF || self.cloud_profile_frame.is_none() {
            return None;
        }
        self.cloud_profile_cache.as_ref().filter(|pc| pc.state.any_valid())
    }

    /// The calibration key: a hash of (tier, interior saturation, the
    /// wide-edge bit, the component bisect index), the inputs the
    /// calibration table depends on. A change re-runs it.
    pub fn cloud_profile_calib_key(&self, tier: f32) -> u32 {
        let bisect: u32 = if self.cloud_no_detail {
            1
        } else if self.cloud_no_puff {
            2
        } else if self.cloud_no_cell {
            3
        } else if self.cloud_no_fray {
            4
        } else if self.cloud_no_bdrop {
            5
        } else {
            0
        };
        // FNV-1a over the four inputs' bit patterns: deterministic, no
        // allocation, and any single-bit change moves the key.
        let mut h: u32 = 0x811c_9dc5;
        for w in [tier.to_bits(), self.cloud_int_sat.clamp(0.0, 1.0).to_bits(), self.cloud_wide_edge as u32, bisect] {
            for b in w.to_le_bytes() {
                h ^= b as u32;
                h = h.wrapping_mul(0x0100_0193);
            }
        }
        h
    }

    /// Dev dump (`debug/cloud_profile_dump_request.json`): every window
    /// slice (`cloud_profile_L<level>_s<slice>.png`, 54), the three global
    /// slices (`cloud_profile_global_<0|1|c>.png`) and the calibration table
    /// (`cloud_profile_calib.png`, 32 x 4 from mip 1), raw RGBA8, into
    /// `dir`. Returns the number of files written. The A17 proof reads the
    /// synthetic pattern back through these; `scripts/cloud-profile-compare.js`
    /// diffs two dumps (the reference bake against the analytic one).
    pub fn dump_cloud_profile_pngs(&self, dir: &std::path::Path) -> Result<usize, String> {
        let pc = self.cloud_profile_cache.as_ref().ok_or_else(|| "cloud profile atlas not created".to_string())?;
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        // One readback of mip 0 (88 MB) plus the 32 x 4 calibration area of mip 1.
        let read = |mip: u32, x: u32, y: u32, w: u32, h: u32| -> Result<Vec<u8>, String> {
            let bytes_per_row = ((w * 4 + 255) / 256) * 256;
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cloud_profile_readback"),
                size: (bytes_per_row * h) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cloud_profile_dump_encoder") });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: pc.texture(),
                    mip_level: mip,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(h),
                    },
                },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
            self.queue.submit([encoder.finish()]);
            let slice = buffer.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            let _ = self.device.poll(wgpu::Maintain::Wait);
            let data = slice.get_mapped_range();
            let mut out = vec![0u8; (w * h * 4) as usize];
            for row in 0..h {
                let s = (row * bytes_per_row) as usize;
                let d = (row * w * 4) as usize;
                out[d..d + (w * 4) as usize].copy_from_slice(&data[s..s + (w * 4) as usize]);
            }
            drop(data);
            buffer.unmap();
            Ok(out)
        };
        let atlas = read(0, 0, 0, CLOUD_FR_ATLAS_W, CLOUD_FR_ATLAS_H)?;
        let crop = |x0: u32, y0: u32, w: u32, h: u32| -> Vec<u8> {
            let mut px = vec![0u8; (w * h * 4) as usize];
            for row in 0..h {
                let s = (((y0 + row) * CLOUD_FR_ATLAS_W + x0) * 4) as usize;
                let d = (row * w * 4) as usize;
                px[d..d + (w * 4) as usize].copy_from_slice(&atlas[s..s + (w * 4) as usize]);
            }
            px
        };
        let save = |name: String, w: u32, h: u32, px: Vec<u8>| -> Result<(), String> {
            let img = image::RgbaImage::from_raw(w, h, px).ok_or_else(|| format!("{name}: pixel buffer size mismatch"))?;
            img.save(dir.join(name)).map_err(|e| e.to_string())
        };
        let mut files = 0usize;
        for level in 0..CLOUD_FR_LEVELS {
            for slice in 0..CLOUD_FR_SLICES_PER_LEVEL {
                let (x0, y0) = cloud_fr_slice_origin(level, slice);
                save(format!("cloud_profile_L{level}_s{slice}.png"), CLOUD_FR_NX, CLOUD_FR_NX, crop(x0, y0, CLOUD_FR_NX, CLOUD_FR_NX))?;
                files += 1;
            }
        }
        for (i, tag) in ["0", "1", "c"].iter().enumerate() {
            save(
                format!("cloud_profile_global_{tag}.png"),
                CLOUD_FR_GLOBAL_W,
                CLOUD_FR_GLOBAL_H,
                crop(i as u32 * CLOUD_FR_GLOBAL_W, CLOUD_FR_GLOBAL_Y0, CLOUD_FR_GLOBAL_W, CLOUD_FR_GLOBAL_H),
            )?;
            files += 1;
        }
        let calib = read(1, CLOUD_FR_CALIB_X0, CLOUD_FR_CALIB_Y0, CLOUD_FR_CALIB_W, CLOUD_FR_CALIB_H)?;
        save("cloud_profile_calib.png".to_string(), CLOUD_FR_CALIB_W, CLOUD_FR_CALIB_H, calib)?;
        files += 1;
        Ok(files)
    }
}

#[cfg(test)]
mod cloud_profile_tests {
    use super::*;

    /// Earth (radius 6371 km, slab 0.4 .. 12 km), the rig's 2560 x 1387
    /// captures at fov 90.05.
    const R_KM: f64 = 6371.0;
    const SLAB_RB: f64 = 1.0 + 0.4 / 6371.0;
    const SLAB_RT: f64 = 1.0 + 12.0 / 6371.0;
    const FOV: f64 = 90.05;
    const ROWS: u32 = 1387;

    fn frame_at(alt_km: f64, res_div: u32, knob: i32) -> CloudProfileFrame {
        CloudProfileFrame {
            ground_lon_rad: 0.0,
            ground_lat_rad: 0.0,
            alt_km,
            radius_km: R_KM,
            slab_rb: SLAB_RB,
            slab_rt: SLAB_RT,
            pix_ang_march: CloudProfileFrame::pix_ang_march(FOV, ROWS, res_div),
            cloud_t: 120.0,
            coverage: 0.5,
            type_pin: 2.34,
            weather_gen: 0,
            tier: 3.0,
            knob,
            calib_key: 7,
        }
    }

    /// Every "both"-owned constant of the contract's table must equal the
    /// value declared in 40-clouds.wgsl, read from the shader TEXT so the
    /// two can never drift silently.
    #[test]
    fn wgsl_cloud_profile_constants_stay_in_sync() {
        let root = env!("CARGO_MANIFEST_DIR");
        let src = std::fs::read_to_string(format!("{root}/assets/shaders/pbr/40-clouds.wgsl")).expect("read 40-clouds.wgsl");
        let pairs: [(&str, f64); 34] = [
            ("CLOUD_FR_CALIB_ROWS", CLOUD_FR_CALIB_ROWS as f64),
            ("CLOUD_FR_CALIB_SEEDS", CLOUD_FR_CALIB_SEEDS as f64),
            ("CLOUD_FR_CALIB_ARCH", CLOUD_FR_CALIB_ARCHETYPES as f64),
            ("CLOUD_FR_CELL0_KM", CLOUD_FR_CELL0_KM as f64),
            ("CLOUD_FR_LEVELS", CLOUD_FR_LEVELS as f64),
            ("CLOUD_FR_NX", CLOUD_FR_NX as f64),
            ("CLOUD_FR_NZ", CLOUD_FR_NZ as f64),
            ("CLOUD_FR_PAIRS", CLOUD_FR_PAIRS as f64),
            ("CLOUD_FR_CSLICES", CLOUD_FR_CSLICES as f64),
            ("CLOUD_FR_SLICES_PER_LEVEL", CLOUD_FR_SLICES_PER_LEVEL as f64),
            ("CLOUD_FR_SLICE_COLS", CLOUD_FR_SLICE_COLS as f64),
            ("CLOUD_FR_COL_SCALE", CLOUD_FR_COL_SCALE as f64),
            ("CLOUD_FR_ATLAS_W", CLOUD_FR_ATLAS_W as f64),
            ("CLOUD_FR_ATLAS_H", CLOUD_FR_ATLAS_H as f64),
            ("CLOUD_FR_GLOBAL_W", CLOUD_FR_GLOBAL_W as f64),
            ("CLOUD_FR_GLOBAL_H", CLOUD_FR_GLOBAL_H as f64),
            ("CLOUD_FR_GLOBAL_Y0", CLOUD_FR_GLOBAL_Y0 as f64),
            ("CLOUD_FR_GLOBAL_NZ", CLOUD_FR_GLOBAL_NZ as f64),
            ("CLOUD_FR_GLOBAL_MIPS", CLOUD_FR_GLOBAL_MIPS as f64),
            ("CLOUD_FR_CALIB_X0", CLOUD_FR_CALIB_X0 as f64),
            ("CLOUD_FR_CALIB_Y0", CLOUD_FR_CALIB_Y0 as f64),
            ("CLOUD_FR_CALIB_STAGE_X0", CLOUD_FR_CALIB_STAGE_X0 as f64),
            ("CLOUD_FR_CALIB_STAGE_Y0", CLOUD_FR_CALIB_STAGE_Y0 as f64),
            ("CLOUD_FR_KNOB_OFF", CLOUD_FR_KNOB_OFF as f64),
            ("CLOUD_FR_KNOB_ON", CLOUD_FR_KNOB_ON as f64),
            ("CLOUD_FR_KNOB_FORCE0", CLOUD_FR_KNOB_FORCE0 as f64),
            ("CLOUD_FR_KNOB_FORCE1", CLOUD_FR_KNOB_FORCE1 as f64),
            ("CLOUD_FR_KNOB_FORCE2", CLOUD_FR_KNOB_FORCE2 as f64),
            ("CLOUD_FR_KNOB_FORCE3", CLOUD_FR_KNOB_FORCE3 as f64),
            ("CLOUD_FR_KNOB_FORCE4", CLOUD_FR_KNOB_FORCE4 as f64),
            ("CLOUD_FR_KNOB_FORCE5", CLOUD_FR_KNOB_FORCE5 as f64),
            ("CLOUD_FR_KNOB_HARD", CLOUD_FR_KNOB_HARD as f64),
            ("CLOUD_FR_KNOB_REF", CLOUD_FR_KNOB_REF as f64),
            // WGSL-owned, but its value is a function of CELL0: the test
            // pins the relation (log2(0.25) = -2).
            ("CLOUD_FR_LOD0", (CLOUD_FR_CELL0_KM as f64).log2()),
        ];
        for (name, rust_v) in pairs {
            let wgsl_v = wgsl_const(&src, name);
            assert!(
                (wgsl_v - rust_v).abs() < 1.0e-6,
                "{name}: WGSL {wgsl_v} != Rust {rust_v} - the bake and the read no longer agree on the lattice"
            );
        }
        assert!((CLOUD_FR_LOD0 as f64 - (CLOUD_FR_CELL0_KM as f64).log2()).abs() < 1.0e-9);
        // D3: the top-bound dev bit must be read by the shader on the SAME
        // bit Rust writes it on. `CLOUD_FR_FLAG_TOP_BOUND` and the shader's
        // `cloud_top_bound_on()` were written independently; without this
        // tie a shader reading bit 11 would present as "the fix arm equals
        // the prod arm", a failed fix, not the wiring bug it is. The bit
        // rides `light2_color.w`, read through `cloud_profile_flag(b)`.
        let want = format!("cloud_profile_flag({})", CLOUD_FR_FLAG_TOP_BOUND.trailing_zeros());
        assert!(
            src.contains("fn cloud_top_bound_on") && src.contains(&want),
            "40-clouds.wgsl must define cloud_top_bound_on() and read the top-bound bit as {want}"
        );
        // Also the calibration areas must be spelled in the temporal shader's
        // stubs by the same names (the passes scissor to them by these values).
        let tmp = std::fs::read_to_string(format!("{root}/assets/shaders/pbr/45-cloud-temporal.wgsl")).expect("read 45-cloud-temporal.wgsl");
        for entry in ["fn fs_cloud_profile_bake", "fn fs_cloud_profile_mip", "fn fs_cloud_profile_calib(", "fn fs_cloud_profile_calib_reduce"] {
            assert!(tmp.contains(entry), "45-cloud-temporal.wgsl must define {entry} (the pipelines bind it)");
        }
    }

    /// `N_I` / `N_J` computed in f32 (the shader's arithmetic) must agree
    /// with the f64 floors for Earth at every level, and match the
    /// contract's numbers.
    #[test]
    fn n_i_and_n_j_agree_between_f32_and_f64_for_earth() {
        let want_ni = [160120u32, 80060, 40030, 20015, 10007, 5003];
        for l in 0..6 {
            let c = cloud_fr_cell_km(l);
            let ni64 = (std::f64::consts::TAU * R_KM / c).floor() as u32;
            let nj64 = (std::f64::consts::PI * R_KM / c).floor() as u32;
            assert_eq!(cloud_fr_n_i(l, R_KM as f32), ni64, "N_I level {l}");
            assert_eq!(cloud_fr_n_j(l, R_KM as f32), nj64, "N_J level {l}");
            assert_eq!(ni64, want_ni[l as usize], "N_I level {l} vs the contract");
        }
        // Earth level 0 cell: 3.9240e-5 rad.
        assert!((cloud_fr_cell_rad(0, R_KM) - 3.9240e-5).abs() < 1.0e-8);
    }

    /// The lattice formula: a point 1 km east of the equator / prime
    /// meridian cell centre lands at `I = I_c + 4` of level 0 within 1e-6
    /// rad (four 250 m cells), and the same latitude row.
    #[test]
    fn one_km_east_is_four_level0_cells() {
        let cell = cloud_fr_cell_rad(0, R_KM);
        let (ic, jc) = cloud_fr_ground_cell(0.0, 0.0, R_KM);
        // The centre of that cell, then 1 km east along the equator.
        let lon_c = (ic as f64 + 0.5) * cell - std::f64::consts::PI;
        let lat_c = (jc as f64 + 0.5) * cell - std::f64::consts::FRAC_PI_2;
        assert!(lon_c.abs() < cell && lat_c.abs() < cell, "lon0/lat0 sit in their own cell");
        let lon_e = lon_c + 1.0 / R_KM;
        let (ie, je) = cloud_fr_ground_cell(lon_e, lat_c, R_KM);
        assert_eq!(ie, ic + 4, "1 km east = 4 cells of 250 m");
        assert_eq!(je, jc);
        // And the residual against the exact 4-cell offset is under 1e-6 rad.
        let lon_back = (ie as f64 + 0.5) * cell - std::f64::consts::PI;
        assert!((lon_back - lon_e).abs() < 1.0e-6);
        // Raw indices never unwrap: the date line clamps to the last cell.
        let ni = cloud_fr_n_i(0, R_KM as f32) as i64;
        let (iw, _) = cloud_fr_ground_cell(std::f64::consts::PI - 1.0e-9, 0.0, R_KM);
        assert_eq!(iw, ni - 1);
        // The window origin halves per level with the shared grid origin.
        let g = (1000, 2000);
        assert_eq!(cloud_fr_window_origin(g, 0), (1000 - 256, 2000 - 256));
        assert_eq!(cloud_fr_window_origin(g, 3), (125 - 256, 250 - 256));
        assert_eq!(cloud_fr_window_origin((1001, 2001), 3), (125 - 256, 250 - 256));
    }

    /// Packing coherence: 54 slices fit 12 x 5, the window band is 5 x 512,
    /// both atlas dimensions are multiples of 64 (every mip edge stays
    /// texel-aligned), and the calibration areas lie OUTSIDE the global's
    /// mip regions.
    #[test]
    fn atlas_packing_is_coherent() {
        assert!(CLOUD_FR_LEVELS * CLOUD_FR_SLICES_PER_LEVEL <= CLOUD_FR_SLICE_COLS * 5);
        assert_eq!(CLOUD_FR_GLOBAL_Y0, 5 * CLOUD_FR_NX);
        assert_eq!(CLOUD_FR_GLOBAL_Y0 % 64, 0);
        assert_eq!(CLOUD_FR_ATLAS_H % 64, 0);
        assert_eq!(CLOUD_FR_ATLAS_W, CLOUD_FR_SLICE_COLS * CLOUD_FR_NX);
        assert_eq!(CLOUD_FR_ATLAS_W, 3 * CLOUD_FR_GLOBAL_W);
        assert_eq!(CLOUD_FR_ATLAS_H, CLOUD_FR_GLOBAL_Y0 + CLOUD_FR_GLOBAL_H);
        assert_eq!(CLOUD_FR_PAIRS + CLOUD_FR_CSLICES, CLOUD_FR_SLICES_PER_LEVEL);
        assert_eq!(2 * CLOUD_FR_PAIRS, CLOUD_FR_NZ);
        assert_eq!(3 * CLOUD_FR_GLOBAL_NZ, CLOUD_FR_NZ);
        // The last slice sits in row 4, column 5; columns 6..11 are spare.
        assert_eq!(cloud_fr_slice_origin(5, 8), (5 * CLOUD_FR_NX, 4 * CLOUD_FR_NX));
        assert_eq!(cloud_fr_slice_origin(0, 0), (0, 0));
        assert_eq!(cloud_fr_slice_origin(1, 3), (0, CLOUD_FR_NX));
        // The calibration areas against the global's mip regions: mip 1's
        // region starts at row 1280, mip 2's at 640; the table (mip 1) sits
        // at rows 1024..1028 and the staging (mip 2) at rows 512..544.
        let mip1_y0 = CLOUD_FR_GLOBAL_Y0 >> 1;
        let mip2_y0 = CLOUD_FR_GLOBAL_Y0 >> 2;
        assert!(CLOUD_FR_CALIB_Y0 + CLOUD_FR_CALIB_H <= mip1_y0, "calibration table overlaps the global's mip 1");
        assert!(CLOUD_FR_CALIB_STAGE_Y0 + CLOUD_FR_CALIB_STAGE_H <= mip2_y0, "staging overlaps the global's mip 2");
        assert!(CLOUD_FR_CALIB_X0 + CLOUD_FR_CALIB_W <= CLOUD_FR_ATLAS_W >> 1);
        assert!(CLOUD_FR_CALIB_STAGE_X0 + CLOUD_FR_CALIB_STAGE_W <= CLOUD_FR_ATLAS_W >> 2);
        // The calibration scissors are what the two calibration fragments
        // bounds-check: the reduce pass covers one column per height row
        // and one row per archetype; the staging pass one row per
        // (archetype, seed). Pinned as relations so neither scissor can be
        // retyped narrower than the table the shader writes.
        assert_eq!(CLOUD_FR_CALIB_W, CLOUD_FR_CALIB_ROWS);
        assert_eq!(CLOUD_FR_CALIB_H, CLOUD_FR_CALIB_ARCHETYPES);
        assert_eq!(CLOUD_FR_CALIB_STAGE_W, CLOUD_FR_CALIB_ROWS);
        assert_eq!(CLOUD_FR_CALIB_STAGE_H, CLOUD_FR_CALIB_ARCHETYPES * CLOUD_FR_CALIB_SEEDS);
        assert_eq!((CLOUD_FR_CALIB_STAGE_W, CLOUD_FR_CALIB_STAGE_H), (32, 32), "the contract's 32 x 32 staging area");
        assert_eq!((CLOUD_FR_CALIB_W, CLOUD_FR_CALIB_H), (32, 4), "the contract's 32 x 4 table");
        // Every mip region edge is exact: 2560 = 5 * 2^9, 3584 = 7 * 2^9.
        for m in 1..CLOUD_FR_GLOBAL_MIPS {
            assert_eq!((CLOUD_FR_GLOBAL_Y0 >> m) << m, CLOUD_FR_GLOBAL_Y0);
            assert_eq!((CLOUD_FR_ATLAS_H >> m) << m, CLOUD_FR_ATLAS_H);
            assert_eq!((CLOUD_FR_GLOBAL_W >> m) << m, CLOUD_FR_GLOBAL_W);
        }
    }

    /// Storage runs: no wrap, wrap, and the full-slice case.
    #[test]
    fn storage_runs_wrap_the_slice() {
        assert_eq!(cloud_fr_storage_runs(10, 3), vec![(10, 3)]);
        assert_eq!(cloud_fr_storage_runs(510, 3), vec![(510, 2), (0, 1)]);
        assert_eq!(cloud_fr_storage_runs(-1, 1), vec![(511, 1)]);
        assert_eq!(cloud_fr_storage_runs(-3, 3), vec![(509, 3)]);
        assert_eq!(cloud_fr_storage_runs(0, 512), vec![(0, 512)]);
        assert_eq!(cloud_fr_storage_runs(1024 + 5, 2), vec![(5, 2)]);
        assert_eq!(cloud_fr_pmod(-1, 512), 511);
        assert_eq!(cloud_fr_pmod(512, 512), 0);
    }

    /// Drive one level through its fill, then a scroll of `d_i` cells,
    /// returning the scroll rects (the level's nine slices, columns only).
    fn scroll_rects(d_i: i64, d_j: i64, start_cell: i64) -> Vec<(u32, u32, u32, u32)> {
        let mut st = CloudProfileState::default();
        // Level 0 forced at 12000 km, where the footprint law activates
        // nothing on its own, so exactly one level is active.
        let cell = cloud_fr_cell_rad(0, R_KM);
        let mut f = frame_at(12000.0, 1, CLOUD_FR_KNOB_FORCE0);
        // Place the ground at cell (start_cell, mid-row) exactly (cell centre).
        let nj = cloud_fr_n_j(0, R_KM as f32) as i64;
        let j0 = nj / 2;
        f.ground_lon_rad = (start_cell as f64 + 0.5) * cell - std::f64::consts::PI;
        f.ground_lat_rad = (j0 as f64 + 0.5) * cell - std::f64::consts::FRAC_PI_2;
        st.plan(f);
        st.take_calib();
        assert!(st.take_bake_rects(1.0 / 60.0).is_empty(), "the calibration frame bakes nothing");
        // Drain the fill: 8 frames of 64 rows.
        for _ in 0..8 {
            let r = st.take_bake_rects(1.0 / 60.0);
            assert!(r.iter().filter(|r| r.0 < CLOUD_FR_NX * 9).count() >= 9);
        }
        assert!(st.levels[0].valid.get(), "level 0 valid after 8 fill frames");
        assert!(st.levels[1..].iter().all(|l| !l.active.get()), "only the forced level is active at 12000 km");
        // Move the ground by (d_i, d_j) cells.
        f.ground_lon_rad += d_i as f64 * cell;
        f.ground_lat_rad += d_j as f64 * cell;
        st.plan(f);
        assert_eq!(st.levels[0].scroll.get(), (d_i, d_j));
        // Zero dt: the refresh still bakes one row (ceil), the global one row.
        let rects = st.take_bake_rects(0.0);
        // Keep only the level-0 rects that are NOT the refresh row (full
        // width) and not the global: i.e. column runs (height 512) for the
        // horizontal scroll and row runs for the vertical one.
        rects.into_iter().filter(|r| r.1 < CLOUD_FR_GLOBAL_Y0).collect()
    }

    /// Two planned moves with NO bake between them (+1 then +2 cells) must
    /// bake the same three columns a single +3 move bakes: the pending
    /// scroll accumulates until the bake consumes it, never overwrites
    /// (a frame that plans but skips the bake would otherwise leave the
    /// first move's column stale while the level stayed valid). And two
    /// moves whose SUM reaches 512 become a fill even though each alone is
    /// a scroll.
    #[test]
    fn scroll_deltas_accumulate_across_unbaked_frames() {
        let cell = cloud_fr_cell_rad(0, R_KM);
        let nj = cloud_fr_n_j(0, R_KM as f32) as i64;
        let mk = |start_cell: i64| {
            let mut st = CloudProfileState::default();
            let mut f = frame_at(12000.0, 1, CLOUD_FR_KNOB_FORCE0);
            f.ground_lon_rad = (start_cell as f64 + 0.5) * cell - std::f64::consts::PI;
            f.ground_lat_rad = ((nj / 2) as f64 + 0.5) * cell - std::f64::consts::FRAC_PI_2;
            st.plan(f);
            st.take_calib();
            st.take_bake_rects(1.0 / 60.0);
            for _ in 0..8 {
                st.take_bake_rects(1.0 / 60.0);
            }
            assert!(st.levels[0].valid.get());
            (st, f)
        };
        // +1 planned, not baked; then +2 planned: the pending delta is +3.
        let (mut st, mut f) = mk(1000);
        f.ground_lon_rad += cell;
        st.plan(f);
        assert_eq!(st.levels[0].scroll.get(), (1, 0));
        f.ground_lon_rad += 2.0 * cell;
        st.plan(f);
        assert_eq!(st.levels[0].scroll.get(), (3, 0), "the second move adds to the first, it does not replace it");
        assert!(st.levels[0].valid.get(), "still a scroll, not a fill");
        let got: Vec<_> = st.take_bake_rects(0.0).into_iter().filter(|r| r.1 < CLOUD_FR_GLOBAL_Y0).collect();
        let want = scroll_rects(3, 0, 1000);
        let cols = |v: &Vec<(u32, u32, u32, u32)>| {
            let mut c: Vec<_> = v.iter().filter(|r| r.3 == CLOUD_FR_NX).map(|r| (r.0, r.2)).collect();
            c.sort();
            c
        };
        assert_eq!(cols(&got), cols(&want), "two unbaked moves bake exactly the columns one +3 move bakes");
        assert_eq!(st.levels[0].scroll.get(), (0, 0), "the bake consumed the delta");
        // 300 + 300 without a bake between: the sum is a refill.
        let (mut st, mut f) = mk(1000);
        f.ground_lon_rad += 300.0 * cell;
        st.plan(f);
        assert!(st.levels[0].valid.get());
        assert_eq!(st.levels[0].scroll.get(), (300, 0));
        f.ground_lon_rad += 300.0 * cell;
        st.plan(f);
        assert!(!st.levels[0].valid.get(), "600 cells since the last bake: nothing of the old window survives");
        assert_eq!(st.levels[0].fill_cursor.get(), Some(0));
        assert_eq!(st.levels[0].scroll.get(), (0, 0));
    }

    /// Scroll rects for dI = +1, -1, +3 with and without wrapping 512, and a
    /// dI = 600 move that becomes a full fill.
    #[test]
    fn scroll_rects_expose_the_right_columns() {
        // Ground at cell 1000: I0 = 744, new I0 = 745; +1 exposes I_abs =
        // I0_prev + 512 = 1256 -> x = 1256 mod 512 = 232.
        let r = scroll_rects(1, 0, 1000);
        let cols: Vec<_> = r.iter().filter(|r| r.3 == 512 && r.2 < 512).collect();
        assert_eq!(cols.len(), 9, "one column run per slice");
        for (p, c) in cols.iter().enumerate() {
            let (x0, y0) = cloud_fr_slice_origin(0, p as u32);
            assert_eq!(**c, (x0 + 232, y0, 1, 512));
        }
        // -1 from 1001 (I0 = 745, new 744): exposes I_abs = 744 -> x = 232.
        let r = scroll_rects(-1, 0, 1001);
        let cols: Vec<_> = r.iter().filter(|r| r.3 == 512 && r.2 < 512).collect();
        assert_eq!(cols.len(), 9);
        assert_eq!(cols[0].0, 232);
        assert_eq!(cols[0].2, 1);
        // +3 from 1000: I_abs 1256..1258 -> x 232..234, one run.
        let r = scroll_rects(3, 0, 1000);
        let cols: Vec<_> = r.iter().filter(|r| r.3 == 512 && r.2 < 512).collect();
        assert_eq!(cols.len(), 9);
        assert_eq!((cols[0].0, cols[0].2), (232, 3));
        // +3 with a WRAP: ground 1278 -> I0 = 1022; new I0 = 1025; exposed
        // I_abs = 1534..1536 -> x = 510, 511, 0: two runs per slice.
        let r = scroll_rects(3, 0, 1278);
        let cols: Vec<_> = r.iter().filter(|r| r.3 == 512 && r.2 < 512).collect();
        assert_eq!(cols.len(), 18, "two runs per slice when the exposed columns wrap");
        // Rects are emitted run-major (the first run for all nine slices,
        // then the second), so slice 0's two runs sit at 0 and 9.
        assert_eq!((cols[0].0, cols[0].2), (510, 2));
        assert_eq!((cols[9].0, cols[9].2), (0, 1));
        assert_eq!(cols[1].0, cloud_fr_slice_origin(0, 1).0 + 510);
        // -1 with a wrap: ground 1280 -> I0 = 1024; new I0 = 1023; exposed
        // I_abs = 1023 -> x = 511.
        let r = scroll_rects(-1, 0, 1280);
        let cols: Vec<_> = r.iter().filter(|r| r.3 == 512 && r.2 < 512).collect();
        assert_eq!(cols.len(), 9);
        assert_eq!((cols[0].0, cols[0].2), (511, 1));
        // A row scroll (+1 north): exposes J_abs = J0_new + 511, a full-width run.
        let r = scroll_rects(0, 1, 1000);
        let rows: Vec<_> = r.iter().filter(|r| r.2 == 512 && r.3 == 1).collect();
        // The refresh row is also a 512 x 1 run at row 0; the scroll row is
        // at pmod(J0 + 511, 512). Both present.
        assert!(rows.len() >= 9);
        // dI = 600: no scroll, a FILL (valid drops, the cursor restarts).
        let mut st = CloudProfileState::default();
        let cell = cloud_fr_cell_rad(0, R_KM);
        let mut f = frame_at(12000.0, 1, CLOUD_FR_KNOB_FORCE0);
        f.ground_lon_rad = 1000.5 * cell - std::f64::consts::PI;
        st.plan(f);
        st.take_calib();
        st.take_bake_rects(0.0);
        for _ in 0..8 {
            st.take_bake_rects(1.0 / 60.0);
        }
        assert!(st.levels[0].valid.get());
        f.ground_lon_rad += 600.0 * cell;
        st.plan(f);
        assert!(!st.levels[0].valid.get(), "a 600-cell move is a refill");
        assert_eq!(st.levels[0].fill_cursor.get(), Some(0));
        assert_eq!(st.levels[0].scroll.get(), (0, 0));
        let r = st.take_bake_rects(1.0 / 60.0);
        let fills: Vec<_> = r.iter().filter(|r| r.2 == 512 && r.3 == 64).collect();
        assert_eq!(fills.len(), 9, "first fill frame: 64 rows per slice");
        assert_eq!(fills[0].1, 0);
    }

    /// Refresh row counts at dt = 1/60 and 1/30: ceil(512 * dt / 2 s) =
    /// 5 and 9 rows; the global rolling pass 1 row, its fast pass 9 rows at
    /// 1/60; REF mode 4 window rows and 2 global rows.
    #[test]
    fn refresh_row_counts_follow_the_time_cadence() {
        let drive = |knob: i32| {
            let mut st = CloudProfileState::default();
            // 12000 km: only the forced level 0 (or, in REF mode, level 0
            // forced through the FORCE0 knob is not available, so REF at
            // 12000 km activates nothing and the test below forces it).
            let mut f = frame_at(12000.0, 1, knob);
            if knob == CLOUD_FR_KNOB_REF {
                // REF at 60 km quarter res: levels 0..5 active; level 0 is
                // the one the assertions read.
                f = frame_at(60.0, 4, knob);
            }
            st.plan(f);
            st.take_calib();
            st.take_bake_rects(0.0);
            let fill_frames = if knob == CLOUD_FR_KNOB_REF { 128 } else { 8 };
            for _ in 0..fill_frames {
                st.take_bake_rects(1.0 / 60.0);
            }
            assert!(st.levels[0].valid.get());
            st
        };
        let st = drive(CLOUD_FR_KNOB_FORCE0);
        // The fast global pass is still running (ceil(1024 / 120) = 9 rows
        // per 1/60 frame, under the D5 fast cap of 32 ->
        // 128 frames); its rect is the one at y >= 2560.
        let r = st.take_bake_rects(1.0 / 60.0);
        let win: Vec<_> = r.iter().filter(|r| r.1 < CLOUD_FR_GLOBAL_Y0).collect();
        let glob: Vec<_> = r.iter().filter(|r| r.1 >= CLOUD_FR_GLOBAL_Y0).collect();
        assert_eq!(win.len(), 9);
        assert_eq!(win[0].3, 5, "1/60 s at 2 s per 512 rows = ceil(4.27) = 5 rows");
        assert_eq!(glob.len(), 1);
        assert_eq!(glob[0].3, 9, "fast global at 60 fps: ceil(1024 / 120) = 9 rows, under the fast cap of 32");
        assert_eq!(glob[0].2, CLOUD_FR_ATLAS_W);
        let r = st.take_bake_rects(1.0 / 30.0);
        let win: Vec<_> = r.iter().filter(|r| r.1 < CLOUD_FR_GLOBAL_Y0).collect();
        assert_eq!(win[0].3, 9, "1/30 s = ceil(8.53) = 9 rows");
        assert_eq!(win[0].1, 5, "the cursor advanced by the previous 5 rows");
        // Drain the fast pass; then the rolling pass takes 1 row per frame.
        for _ in 0..200 {
            st.take_bake_rects(1.0 / 60.0);
        }
        assert!(st.global_valid.get());
        assert!(!st.global_pass_fast.get());
        assert!(st.mips_pending.get(), "mips pending after the pass completed");
        st.mips_pending.set(false);
        let r = st.take_bake_rects(1.0 / 60.0);
        let glob: Vec<_> = r.iter().filter(|r| r.1 >= CLOUD_FR_GLOBAL_Y0).collect();
        assert_eq!(glob[0].3, 1, "rolling global: ceil(1024 / 3600) = 1 row");
        // The refresh cursor wraps: at 6 rows per frame (dt 1/50) a run
        // straddles the slice edge and is split into two rects, the second
        // starting at storage row 0.
        let mut wrapped = false;
        for _ in 0..110 {
            let r = st.take_bake_rects(1.0 / 50.0);
            if r.iter().any(|r| r.1 < CLOUD_FR_GLOBAL_Y0 && r.1 % 512 == 0 && r.2 == 512 && r.3 < 6) {
                wrapped = true;
            }
        }
        assert!(wrapped, "a refresh run split at the slice edge");
        assert!(st.levels[0].refresh_cursor.get() < 512.0);
        // REF mode: 4 window rows, 2 global rows, whatever dt.
        let st = drive(CLOUD_FR_KNOB_REF);
        let r = st.take_bake_rects(1.0 / 30.0);
        let win: Vec<_> = r.iter().filter(|r| r.1 < CLOUD_FR_GLOBAL_Y0).collect();
        let glob: Vec<_> = r.iter().filter(|r| r.1 >= CLOUD_FR_GLOBAL_Y0).collect();
        assert_eq!(win[0].3, CLOUD_FR_REF_ROWS);
        assert_eq!(glob[0].3, CLOUD_FR_REF_GLOBAL_ROWS);
    }

    /// D5 (the bake cadence at low frame rates): at dt 0.3 s (about 3 fps,
    /// the rig's slow cells) a level's refresh never bakes more than
    /// CLOUD_FR_REFRESH_ROWS_MAX rows per frame and the global never more
    /// than CLOUD_FR_GLOBAL_FAST_ROWS_MAX on its fast pass (the rolling one
    /// keeps CLOUD_FR_GLOBAL_ROWS_MAX), so the per-frame cost is bounded by
    /// the frame and not by the second (uncapped, 0.3 s asked for 77 window
    /// rows per level and 154 global rows in ONE frame); a level still
    /// completes a full refresh (512 rows in exactly 32 frames); and the fast
    /// global pass still finishes inside a capture window (1024 rows at 32 a
    /// frame = 32 frames = 9.6 s at 3 fps, under every fixture s 12 to 14 s
    /// settle: the reason the fast cap is not 8).
    #[test]
    fn low_frame_rate_caps_rows_per_frame_and_still_completes_a_refresh() {
        let mut st = CloudProfileState::default();
        st.plan(frame_at(12000.0, 1, CLOUD_FR_KNOB_FORCE0));
        st.take_calib();
        st.take_bake_rects(0.0);
        // The FILL keeps its 64-row cap (8 frames whatever the dt).
        for _ in 0..8 {
            let r = st.take_bake_rects(0.3);
            let fill_rows: u32 = r.iter().filter(|r| r.0 == 0 && r.1 < 512 && r.2 == 512).map(|r| r.3).sum();
            assert_eq!(fill_rows, CLOUD_FR_FILL_ROWS, "a fill frame bakes 64 rows of slice 0");
        }
        assert!(st.levels[0].valid.get());
        let start = st.levels[0].refresh_cursor.get();
        let mut refreshed = 0u32;
        let mut global_done_at = None;
        for frame in 1..=128u32 {
            let r = st.take_bake_rects(0.3);
            // Level 0's refresh rows this frame: the rects of its slice 0
            // (origin (0, 0), full width), one or two runs when wrapping.
            let win_rows: u32 = r.iter().filter(|r| r.0 == 0 && r.1 < 512 && r.2 == 512).map(|r| r.3).sum();
            assert!(win_rows >= 1 && win_rows <= CLOUD_FR_REFRESH_ROWS_MAX, "frame {frame}: {win_rows} refresh rows");
            let glob: Vec<_> = r.iter().filter(|r| r.1 >= CLOUD_FR_GLOBAL_Y0).collect();
            for g in &glob {
                assert!(g.3 >= 1 && g.3 <= CLOUD_FR_GLOBAL_FAST_ROWS_MAX, "frame {frame}: {} global rows", g.3);
                assert_eq!(g.2, CLOUD_FR_ATLAS_W);
            }
            if frame <= 32 {
                refreshed += win_rows;
            }
            if frame == 32 {
                // 512 rows at 16 per frame: the cursor is back where it
                // started, one whole refresh done in 32 frames (9.6 s at
                // 0.3 s per frame; 2 s at 60 fps, unchanged there).
                assert_eq!(refreshed, CLOUD_FR_NX, "a full refresh in 32 frames");
                assert_eq!(st.levels[0].refresh_cursor.get(), start, "the refresh cursor wrapped to its start");
            }
            if global_done_at.is_none() && st.global_valid.get() {
                global_done_at = Some(frame);
            }
        }
        // The fast global pass at 32 rows per frame. The bar that matters is
        // 32 frames (9.6 s at 3 fps, inside a 12 s settle): before the split
        // it took 120 frames = 36 s and no capture window ever saw a valid
        // global, so the Low sheet fell back to the old weather path and its
        // gate could not fail.
        let done = global_done_at.expect("the fast global pass completes");
        assert!(done <= 32, "the fast global pass completes inside a capture window, took {done} frames");
    }

    /// The active level range at 3, 60, 873 and 12000 km for cloud_res 1
    /// (1387 rows) and 4 (346 rows), from the footprint law.
    #[test]
    fn active_levels_follow_the_footprint_law() {
        let levels = |alt: f64, div: u32| -> Vec<u32> {
            let a = cloud_fr_active_levels(&frame_at(alt, div, CLOUD_FR_KNOB_ON));
            (0..6u32).filter(|&l| a[l as usize]).collect()
        };
        assert_eq!(levels(3.0, 1), vec![0, 1, 2]);
        assert_eq!(levels(3.0, 4), vec![0, 1, 2, 3, 4]);
        assert_eq!(levels(60.0, 1), vec![0, 1, 2, 3]);
        assert_eq!(levels(60.0, 4), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(levels(873.0, 1), vec![2, 3, 4, 5]);
        assert_eq!(levels(873.0, 4), vec![4, 5]);
        assert_eq!(levels(12000.0, 1), Vec::<u32>::new());
        assert_eq!(levels(12000.0, 4), Vec::<u32>::new());
        // The forced level is active whatever the footprint.
        let a = cloud_fr_active_levels(&frame_at(12000.0, 1, CLOUD_FR_KNOB_FORCE3));
        assert_eq!(a, [false, false, false, true, false, false]);
        assert_eq!(cloud_fr_forced_level(CLOUD_FR_KNOB_HARD), None);
        assert_eq!(cloud_fr_forced_level(CLOUD_FR_KNOB_FORCE5), Some(5));
        // The pixel angle itself: 1.443e-3 rad at full res, 4x at quarter
        // (integer rows: 346).
        let full = CloudProfileFrame::pix_ang_march(FOV, ROWS, 1);
        assert!((full - 1.4432e-3).abs() < 2.0e-6, "full-res pix_ang {full}");
        let quarter = CloudProfileFrame::pix_ang_march(FOV, ROWS, 4);
        assert!((quarter - 2.0 * (FOV.to_radians() * 0.5).tan() / 346.0).abs() < 1.0e-12);
    }

    /// Flags encoding: bit 0 = any window, bit 1 = global, bits 2..7 per
    /// level, bit 8 = calibration; exact integers in f32.
    #[test]
    fn flags_encode_the_valid_bits() {
        assert_eq!(cloud_fr_flags([false; 6], false, false), 0);
        assert_eq!(cloud_fr_flags([false; 6], true, false), 2);
        assert_eq!(cloud_fr_flags([true, false, false, false, false, false], false, false), 1 | 4);
        assert_eq!(cloud_fr_flags([false, false, false, false, false, true], false, true), 1 | 128 | 256);
        let all = cloud_fr_flags([true; 6], true, true);
        assert_eq!(all, 0b1_1111_1111);
        // The shader isolates bit b as fract(flags * 2^-(b+1)) >= 0.5: check
        // the arithmetic on the f32 the pad carries.
        let f = all as f32;
        for b in 0..9 {
            let v = (f * (-(b as f32 + 1.0)).exp2()).fract();
            assert!(v >= 0.5, "bit {b} of {all} decodes set");
        }
        let f = cloud_fr_flags([false; 6], true, false) as f32;
        assert!((f * 0.5).fract() < 0.5, "bit 0 clear");
        assert!((f * 0.25).fract() >= 0.5, "bit 1 set");
        // pads() from a planned state carries the ground cell and the knob.
        let mut st = CloudProfileState::default();
        assert_eq!(st.pads(), [0.0; 4]);
        st.plan(frame_at(873.0, 1, CLOUD_FR_KNOB_HARD));
        let p = st.pads();
        let (gi, gj) = cloud_fr_ground_cell(0.0, 0.0, R_KM);
        assert_eq!(p[0], gi as f32);
        assert_eq!(p[1], gj as f32);
        assert_eq!(p[2], CLOUD_FR_KNOB_HARD as f32);
        assert_eq!(p[3], 0.0, "nothing valid yet");
        // Earth's ground index fits an exact f32 integer (< 2^24).
        assert!(cloud_fr_n_i(0, R_KM as f32) < (1 << 24));
    }

    /// D3: the built-body top-bound dev bit rides bit 12 of the flags pad.
    /// The shader isolates it as `fract(flags * exp2(-13)) >= 0.5`; the bit
    /// must decode set on the f32 the pad carries, leave bits 0..8 exactly
    /// as they were, and survive on the zero pad of a frame without an
    /// atlas (the gate runs it at knob 0, where no atlas exists).
    #[test]
    fn top_bound_bit_decodes_as_bit_12_and_leaves_the_valid_bits_alone() {
        let decode = |f: f32, b: u32| (f * (-(b as f32 + 1.0)).exp2()).fract() >= 0.5;
        // Every validity pattern: the bit is added, nothing else moves.
        let patterns = [
            cloud_fr_flags([false; 6], false, false),
            cloud_fr_flags([false; 6], true, false),
            cloud_fr_flags([true, false, true, false, true, false], true, true),
            cloud_fr_flags([true; 6], true, true),
        ];
        for &base in &patterns {
            let off = cloud_fr_flags_with_top_bound(base, false);
            let on = cloud_fr_flags_with_top_bound(base, true);
            assert_eq!(off, base, "off leaves the flags untouched");
            assert_eq!(on, base | 4096, "on ORs exactly 4096");
            assert_eq!(on & 0x1FF, base & 0x1FF, "bits 0..8 unchanged under the bit");
            let f_on = on as f32;
            let f_off = off as f32;
            assert_eq!(f_on as u32, on, "exact in f32");
            assert!(decode(f_on, 12), "bit 12 decodes set on {on}");
            assert!(!decode(f_off, 12), "bit 12 decodes clear on {off}");
            // The shader's rule reads every validity bit the same way with
            // the top bound on as off.
            for b in 0..9 {
                assert_eq!(decode(f_on, b), decode(f_off, b), "bit {b} of {base} moves under the top bound");
                assert_eq!(decode(f_off, b), (base >> b) & 1 == 1, "bit {b} of {base} decodes");
            }
            // Bits 9..11 and 13..15 stay clear: the bit lands on 12 only.
            for b in [9u32, 10, 11, 13, 14, 15] {
                assert!(!decode(f_on, b), "bit {b} spuriously set by the top bound");
            }
        }
        // The no-atlas pad: zeros everywhere, the flags lane = the bit alone.
        assert_eq!(cloud_fr_flags_with_top_bound(0, true), 4096);
        assert_eq!(cloud_fr_flags_with_top_bound(0, true) as f32, 4096.0);
    }

    /// The global's re-reference rules and the stale marking.
    #[test]
    fn global_rereferences_on_jumps_weather_coverage_pin_and_class() {
        let mut st = CloudProfileState::default();
        let mut f = frame_at(873.0, 1, CLOUD_FR_KNOB_ON);
        st.plan(f);
        assert!(st.global_started && st.global_pass_fast.get() && !st.global_valid.get());
        st.take_calib();
        st.take_bake_rects(0.0);
        for _ in 0..130 {
            st.take_bake_rects(1.0 / 60.0);
        }
        assert!(st.global_valid.get() && !st.global_pass_fast.get());
        // Nothing changed: still rolling.
        st.plan(f);
        assert!(!st.global_pass_fast.get());
        // A 200 s clock jump: fast pass, valid kept.
        f.cloud_t += 200.0;
        st.plan(f);
        assert!(st.global_pass_fast.get() && st.global_valid.get());
        st.global_pass_fast.set(false);
        // Coverage +0.01: nothing; +0.03: fast.
        f.coverage += 0.01;
        st.plan(f);
        assert!(!st.global_pass_fast.get());
        f.coverage += 0.02;
        st.plan(f);
        assert!(st.global_pass_fast.get());
        st.global_pass_fast.set(false);
        f.weather_gen += 1;
        st.plan(f);
        assert!(st.global_pass_fast.get());
        st.global_pass_fast.set(false);
        f.type_pin = 1.0;
        st.plan(f);
        assert!(st.global_pass_fast.get());
        st.global_pass_fast.set(false);
        // Knob class 1 -> 9 (REF): fast pass AND every window refills.
        for _ in 0..8 {
            st.take_bake_rects(1.0 / 60.0);
        }
        let valid_before = st.levels_valid();
        assert!(valid_before.iter().any(|&v| v));
        f.knob = CLOUD_FR_KNOB_REF;
        st.plan(f);
        assert!(st.global_pass_fast.get());
        assert!(st.levels.iter().filter(|l| l.active.get()).all(|l| !l.valid.get() && l.fill_cursor.get() == Some(0)));
        // Calibration key change: the calibration re-runs, everything refills.
        f.knob = CLOUD_FR_KNOB_ON;
        st.plan(f);
        for _ in 0..8 {
            st.take_bake_rects(1.0 / 60.0);
        }
        f.calib_key += 1;
        st.plan(f);
        assert!(st.calib_pending.get() && !st.calib_valid.get());
        assert!(st.levels.iter().filter(|l| l.active.get()).all(|l| l.fill_cursor.get() == Some(0)));
        assert!(st.take_calib());
        assert!(!st.take_calib(), "consumed");
        assert!(st.calib_valid.get());
        assert!(st.take_bake_rects(1.0 / 60.0).is_empty(), "the bake waits one frame after the calibration");
        assert!(!st.take_bake_rects(1.0 / 60.0).is_empty());
        // Stale marking drops every window and orders a fast global pass on
        // the next plan.
        st.mark_stale();
        assert!(st.levels.iter().all(|l| !l.valid.get() && l.origin.get().is_none()));
        st.global_pass_fast.set(false);
        st.plan(f);
        assert!(st.global_pass_fast.get());
        assert!(st.levels.iter().filter(|l| l.active.get()).all(|l| l.fill_cursor.get() == Some(0)));
    }

    /// The truncation counter is 0 on Earth and engages on a 4x planet.
    #[test]
    fn truncated_texels_estimate_is_zero_on_earth() {
        let mut st = CloudProfileState::default();
        st.plan(frame_at(873.0, 4, CLOUD_FR_KNOB_ON));
        assert_eq!(st.truncated_texels_estimate(), 0);
        let mut f = frame_at(873.0, 4, CLOUD_FR_KNOB_ON);
        f.radius_km = 4.0 * R_KM;
        let mut st = CloudProfileState::default();
        st.plan(f);
        assert!(st.truncated_texels_estimate() > 0, "a 78 km global cell exceeds 512 candidates");
    }
}
