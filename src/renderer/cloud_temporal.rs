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

    /// Read `const NAME: <type> = <value>;` out of 40-clouds.wgsl whatever
    /// the scalar type (the cache constants are a mix of u32 and f32).
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
            "const {name} not found in 40-clouds.wgsl - the WGSL side must declare \
             every CLOUD_LC_* constant with exactly this name and value"
        );
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
