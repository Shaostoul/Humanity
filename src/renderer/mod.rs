//! Renderer — wgpu device/surface setup and render loop.
//!
//! Configuration loaded from `config/renderer.toml`.
//! Supports both native (winit window) and WASM (canvas) targets.

pub mod atmosphere;
pub mod billboard_bake;
/// Atmosphere LUT generators (Hillaire stage 1: transmittance, CPU-side).
pub mod atmo_luts;
/// Sky-view LUT offscreen pass (sky arc stage 3b-2).
pub mod sky_view;
/// CPU twin + calibration of the megashader's indirect-light terms (v0.1104).
/// Pure math, no wgpu, so it compiles and tests under the relay feature too.
pub mod sky_ambient;
/// The engine's key lights (moonlight fill), extracted from lib.rs v0.1104.
pub mod key_lights;
/// Screen-tile light binning (clustering L1).
pub mod light_tiles;
pub mod bloom;
pub mod godrays;
pub mod ssao;
pub mod camera;
/// Frame + texture readback to PNG (screenshot command, hi-res capture, the
/// probe rig). Extracted from mod.rs in v0.1108 - see the file's header for
/// why this cluster and not another.
pub mod capture;
/// Non-blocking swapchain readback for live streaming (v0.853). The screenshot path
/// stalls the GPU on purpose; a stream must never do that. See stream_capture.rs.
///
/// NATIVE-GATED: it hands frames to `net::live`, which is native-only. `renderer` as a
/// whole is NOT gated, so an ungated submodule that reaches into `net` breaks the relay
/// build (and therefore CI's VPS deploy) while the native build stays green.
#[cfg(feature = "native")]
pub mod stream_capture;
pub mod cloud_noise;
pub mod cloud_primitives;
pub mod cloud_composite;
pub mod cloud_resolve;
pub mod cloud_reference;
pub mod cloud_temporal;
pub mod clouds;
/// Live per-pass / per-stage / per-allocation cost measurement (resource
/// budgets increment 1). Ungated: `renderer` compiles in the relay build too.
pub mod frame_costs;
pub mod ground_textures;
pub mod floating_origin;
pub mod hologram;
pub mod light;
pub mod line;
pub mod materials;
pub mod mesh;
pub mod multi_scale;
pub mod patch_arena;
pub mod plant_mesh;
pub mod tree_mesh;
pub mod particles;
pub mod particles_gpu;
pub mod pipeline;
pub mod shader_loader;
/// Which material types can DISCARD in the sun shadow pass, and the test that
/// keeps that answer equal to the shader's (v0.1108).
pub mod shadow_cutout;
pub mod stars;
pub mod water;

/// Sun shadow map resolution, texels per side. Module constants (v0.1104)
/// rather than locals inside the render loop, because the megashader's
/// normal-offset needs the WORLD size of one texel and pins it as a literal
/// (`SHADOW_TEXEL_M` in 00-bindings-vertex.wgsl). `sky_ambient`'s lockstep
/// test compares the two.
pub const SUN_SHADOW_MAP_SIZE: f32 = 4096.0;
/// Half-extent of the sun shadow map's ortho box, in metres.
pub const SUN_SHADOW_EXTENT_M: f32 = 1500.0;

use camera::{Camera, CameraUniforms};
use glam::{Mat4, Quat, Vec3};
use mesh::Mesh;
use pipeline::{ObjectUniforms, Pipeline};

/// Max opaque/transparent objects drawn per frame (dynamic uniform buffer capacity + the per-pass
/// draw cap). Bumped 256 -> 1024 in v0.528: a fully built home (the dense indoor garden alone is
/// ~100 machine meshes, plus pipes + markers + walls) exceeded 256, and objects past the cap were
/// silently truncated -- which made the home's machines vanish once they moved to their own render
/// list. 1024 entries x 256-byte alignment = 256 KB, allocated once. The cap is a ceiling, so the
/// per-frame cost stays proportional to the actual object count.
// 4096 since v0.887 (was 1024): max-graphics terrain wants 2000-3000
// patches at the 4 px split tier, and the whole scene shares this pool.
// 8192 since v0.892: the v0.891 submission batching made draw count ~4x
// cheaper on the CPU, so the patch-budget ceiling rose to 6144 for
// tomorrow's GPUs. Cost is one 2 MB dynamic uniform buffer - nothing.
const MAX_OBJECTS: usize = 16384;

/// One batched terrain-patch draw for this frame: arena ranges + the
/// per-instance data (anchor translation in render space + LOD fade, the
/// same fade encoding the classic path smuggles through model[0].w).
#[derive(Clone, Copy, Debug)]
pub struct PatchDraw {
    pub slot: patch_arena::PatchSlot,
    pub position: Vec3,
    pub fade: f32,
}
use wgpu::util::DeviceExt;

/// Describes one object to render in the scene.
#[derive(Clone)]
pub struct RenderObject {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub mesh: usize,     // index into Renderer::meshes
    pub material: usize, // index into Renderer::materials
    /// LOD crossfade (v0.920): 0.0 = drawn normally (the default everywhere).
    /// (0, 1) = fading IN - the fragment shader shows pixels where the 4x4
    /// Bayer threshold is BELOW this value. (-1, 0) = fading OUT with
    /// threshold |fade| - shows pixels where Bayer is AT/ABOVE it. A rising
    /// patch at t and its falling partner at -t therefore partition the
    /// screen per-pixel: no holes, no double-write, opaque depth intact.
    /// Rides row 3 of the model matrix (model[0].w - the vertex shader
    /// rebuilds the homogeneous w, so the slot is free metadata).
    pub fade: f32,
}

/// A textured material's group-3 bind groups: the SAME entry list built twice,
/// differing at binding 6 only (v0.1108).
///
/// The colour passes bind the real `shadow_map_view` there, because a lit
/// surface samples the sun map to shade itself. The SUN SHADOW pass cannot:
/// that texture is the pass's own depth attachment, and wgpu merges the two
/// uses into RESOURCE | DEPTH_STENCIL_WRITE, which is an exclusive-usage
/// conflict rejected at bind time. So the shadow pass needs a twin whose
/// binding 6 is the 1x1 dummy depth, and everything else identical.
///
/// WHY BOTH LIVE IN ONE STRUCT rather than two `Option` fields: through
/// v0.1107 the shadow pass had no per-material group at all, so `fs_shadow`'s
/// type-19 and type-21 discards sampled the pass-wide 1x1 WHITE fallback
/// (alpha 1) and never fired - near-tree foliage kept stamping solid quads
/// into the sun map while the shader read as if the job were done. A pair that
/// cannot be half-populated makes that failure unrepresentable: there is no
/// way to build the colour group without its shadow-safe twin, because
/// `materials::build_material_texture_bind_group` is the only constructor and
/// it returns both from one entry list.
pub struct AlbedoBindGroup {
    /// Colour passes. Binding 6 = the real shadow map.
    colour: wgpu::BindGroup,
    /// The sun shadow pass. Binding 6 = the 1x1 dummy depth.
    shadow: wgpu::BindGroup,
}

/// Material properties for PBR-lite rendering.
pub struct Material {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: f32,
    /// The shader's `material.params.z` - the type dispatch in
    /// 90-fragment-main.wgsl. Kept on the CPU side (v0.1108) so the shadow
    /// pass can pick the depth-only PSO for OPAQUE textured materials (bark,
    /// planet imagery) instead of paying a fragment stage that discards
    /// nothing. MUST be rewritten by every path that rewrites the uniform, or
    /// the selector below drifts from what the shader actually does; the two
    /// writers are `add_material_full` and `update_material_full`.
    material_type: f32,
    /// CPU copy of the shader's `material.params2` (clouds depth increment:
    /// per-planet slab bounds + radius ride here for the type-15 cloud
    /// shell; zero for everything else). Kept so `update_material_full` -
    /// which rewrites the WHOLE uniform buffer - preserves it instead of
    /// silently zeroing per-planet data every frame.
    params2: [f32; 4],
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Group-3 texture bind groups for materials that carry real imagery
    /// (v0.811: per-pixel planet albedo). None = the renderer binds its 1x1
    /// white fallback instead, so every draw satisfies the shared pipeline
    /// layout. The bind groups internally keep their texture + view alive.
    albedo_bind_group: Option<AlbedoBindGroup>,
}

impl Material {
    /// Group 3 for a COLOUR pass, or None to use the renderer's fallback.
    fn albedo_group(&self) -> Option<&wgpu::BindGroup> {
        self.albedo_bind_group.as_ref().map(|a| &a.colour)
    }

    /// Group 3 for the SUN SHADOW pass (dummy depth at binding 6), or None to
    /// use `shadow_pass_texture_bind_group`.
    fn shadow_albedo_group(&self) -> Option<&wgpu::BindGroup> {
        self.albedo_bind_group.as_ref().map(|a| &a.shadow)
    }

    /// True when `fs_shadow` can DISCARD for this material, i.e. when this
    /// caster must draw with the alpha-cutout shadow PSO instead of the
    /// depth-only one. See `shadow_cutout::type_casts_cutout_shadow` for the
    /// type bands and the test that keeps them equal to the shader's.
    fn casts_cutout_shadow(&self) -> bool {
        shadow_cutout::type_casts_cutout_shadow(self.material_type)
    }
}

/// Groups objects sharing the same mesh and material for instanced drawing.
pub struct InstanceBatch {
    /// Index into Renderer::meshes.
    pub mesh: usize,
    /// Index into Renderer::materials.
    pub material: usize,
    /// Model-space transforms for each instance.
    pub transforms: Vec<Mat4>,
}

/// Core renderer state wrapping wgpu device, queue, and surface.
/// Live weather map dimensions (v0.874). Defined HERE (not in
/// net::live_weather) because the renderer compiles in every feature set
/// while the fetcher is native-only; the fetcher aliases these.
pub const WEATHER_MAP_W: u32 = 1440;
pub const WEATHER_MAP_H: u32 = 720;
/// Weather-map mip levels (increment 11b): 720 halves to 1 in 9 steps + the
/// base = 10; wgpu floor-halves the non-square 1440x720 the same way.
pub const WEATHER_MAP_MIPS: u32 = 10;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pipeline: Pipeline,
    /// Megashader hot-reload state (v0.924): (path, last seen mtime) of the
    /// on-disk pbr_simple.wgsl; None in stripped installs (feature dormant).
    #[cfg(feature = "native")]
    shader_hot: Option<(std::path::PathBuf, std::time::SystemTime)>,
    /// Throttle for the mtime poll (one metadata read per second).
    #[cfg(feature = "native")]
    shader_hot_checked: std::time::Instant,
    /// World-space thin-line pipeline (orbit paths). Shares the main
    /// camera bind group; reverse-Z depth-test, no depth-write.
    line_pipeline: wgpu::RenderPipeline,
    /// Particle billboard pipelines (v0.966): alpha + additive blend pair,
    /// drawn as a post-pass (draw_particles_onto). The frame uniform holds
    /// the camera right/up axes for billboard expansion.
    particle_pipeline_alpha: wgpu::RenderPipeline,
    particle_pipeline_additive: wgpu::RenderPipeline,
    particle_frame_buffer: wgpu::Buffer,
    /// Persistent particle vertex buffers (v0.1067). These used to be created
    /// FRESH EVERY FRAME with create_buffer_init - a driver allocation, a
    /// mapped write and a deallocation per frame, per blend mode, whose cost
    /// scales with particle count exactly when you least want it to. They now
    /// grow to a high-water mark and are refilled with write_buffer.
    particle_vb_alpha: Option<wgpu::Buffer>,
    particle_vb_additive: Option<wgpu::Buffer>,
    /// GPU-simulated particle pool (v0.1068). None until first use; created on
    /// demand so a session that never sees weather never allocates it.
    pub gpu_particles: Option<particles_gpu::GpuParticles>,
    particle_vb_alpha_cap: usize,
    particle_vb_additive_cap: usize,
    particle_frame_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    /// Uncapped scene-light list (v0.782): a storage buffer of 64-byte GpuLight
    /// entries; grows by doubling (bind group recreated) when the count exceeds
    /// capacity. The shader loops over `light_count` of these.
    lights_buffer: wgpu::Buffer,
    tile_counts_buffer: wgpu::Buffer,
    tile_indices_buffer: wgpu::Buffer,
    /// Tile pixel sizes for the shadow-uniform poke (0 = tiling off).
    tile_px: (f32, f32),
    lights_capacity: usize,
    /// Pre-allocated object uniform buffer, reused each frame via write_buffer.
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
    // Registered meshes and materials
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    // ── Off-screen render target (for bloom, shadow maps, particles) ──
    /// Scene renders here first, then post-processing composites to swapchain.
    scene_texture: wgpu::Texture,
    scene_view: wgpu::TextureView,
    /// Bloom post-processing (reads scene_texture, composites result).
    pub bloom: Option<bloom::BloomPass>,
    /// Crepuscular god rays (v0.895): depth-marched light shafts drawn
    /// between the celestial and interior passes.
    godrays: godrays::GodrayPass,
    /// God-ray strength (0.0 disables the pass entirely).
    pub godray_intensity: f32,
    /// Bloom intensity (0.0 = off). Set > 0 to enable bloom post-process.
    pub bloom_intensity: f32,
    /// Brightness threshold for bloom extraction.
    pub bloom_threshold: f32,
    /// LIVE local-light state (v0.571). The `_onto` passes rewrite the WHOLE camera uniform at offset
    /// 0 from `camera.uniforms()` (which carries NO lights + a default sun), which used to CLOBBER the
    /// sub-range writes of `set_point_lights`/`set_sun_light`/`set_fill_light` -- so point lights never
    /// lit the interior and the GI toggle did nothing. We now STORE the light state here and inject it
    /// into each home pass via `lit_uniform`, so it survives the full-uniform write.
    cur_lights: Vec<light::RoomLight>,
    cur_sun: ([f32; 3], [f32; 3], f32), // (direction, color, intensity)
    cur_fill: ([f32; 3], [f32; 3], f32),
    /// Whether the swapchain surface was configured with `COPY_SRC` (v0.639, live screenshot
    /// command). Most backends support it; a backend that doesn't gets a clean
    /// `capture_current_frame` error instead of a validation panic.
    supports_frame_capture: bool,
    /// Shared sampler for all group-3 albedo textures (v0.811): bilinear,
    /// wrap in U (equirect longitude crosses the antimeridian), clamp in V
    /// (latitude holds at the pole rows) -- mirrors the CPU grid samplers'
    /// edge policy in terrain::planet_heightmap/planet_albedo.
    albedo_sampler: wgpu::Sampler,
    /// Sampler for TILING material textures - the baked bark (v0.1089).
    ///
    /// The shared `albedo_sampler` above cannot serve them: it CLAMPS V (right
    /// for equirect latitude, fatal for a texture that tiles up a trunk, which
    /// would smear one row of texels over the whole bole) and its mipmap
    /// filter is Nearest because planet imagery ships a single level. This one
    /// repeats on both axes and filters trilinearly with 8x anisotropy, which
    /// is what lets the type-22 bark branch drop the distance fade entirely.
    /// Binding 1 of the texture layout is a plain Sampler(Filtering) slot, so
    /// swapping which sampler a bind group carries is NOT a layout change and
    /// touches none of the three create_bind_group sites' entry counts.
    bark_sampler: wgpu::Sampler,
    /// Species id -> material index for baked bark (v0.1089). The bake is a
    /// pure function of the registry row, so it runs ONCE per species per
    /// session and every variant of that species shares the material. (BUG-059
    /// is the standing lesson: anything expensive reached from the per-frame
    /// near-tree block must be memoized at its own call site, not upstream.)
    bark_materials: std::collections::HashMap<String, usize>,
    /// 1x1 white fallback bound at group 3 for every material without real
    /// imagery, so the shared pipeline layout is always satisfied and
    /// non-planet draws are unaffected (the shader only samples group 3 on
    /// material type 12 with the params.w flag set).
    default_texture_bind_group: wgpu::BindGroup,
    /// Shared tiling 3D cloud-noise volumes (clouds increment 3): the SHAPE
    /// (384^3 Perlin-Worley + Worley octaves) and DETAIL (256^3 Worley
    /// octaves) textures every group-3 bind group references at bindings
    /// 2/3, plus the repeat-all-axes sampler at binding 4. Engine-global:
    /// generated once at startup by renderer::cloud_noise, identical for
    /// every material and planet (per-planet variety comes from the weather
    /// field's seed). Kept on the struct so build_albedo_bind_group can
    /// include them in every bind group it makes.
    cloud_shape_view: wgpu::TextureView,
    cloud_detail_view: wgpu::TextureView,
    cloud_tile_sampler: wgpu::Sampler,
    /// Live weather map (v0.874): RG8 equirect, R = NASA cloud fraction,
    /// G = validity. Zero = procedural sky; update_weather_map overwrites.
    weather_map_tex: wgpu::Texture,
    weather_map_view: wgpu::TextureView,
    /// Sun shadow map (v0.899): near-field ortho depth from the sun.
    shadow_map_view: wgpu::TextureView,
    shadow_uniform_buffer: wgpu::Buffer,
    /// Camera-layout uniform holding the LIGHT's view-proj for the shadow
    /// pass (vs_main renders with whatever camera is bound at group 0).
    light_camera_buffer: wgpu::Buffer,
    /// Group-3 bind for the SHADOW pass itself: identical to the fallback
    /// except binding 6 is a 1x1 dummy depth - the pass writes the real
    /// shadow map as its depth attachment, and wgpu forbids sampling a
    /// texture in the same pass that writes it (exclusive usage).
    shadow_pass_texture_bind_group: wgpu::BindGroup,
    /// The 1x1 Depth32Float view that stands in for the shadow map at binding
    /// 6 inside the shadow pass. Kept on the struct since v0.1108 because
    /// per-material group-3 bind groups are built LAZILY (whenever a textured
    /// material first loads, long after `new`), and each one needs its own
    /// shadow-safe twin - see `AlbedoBindGroup`.
    dummy_depth_view: wgpu::TextureView,
    light_camera_bind_group: wgpu::BindGroup,
    shadow_comparison_sampler: wgpu::Sampler,
    ground_textures: ground_textures::GroundTextures,
    /// Atmosphere LUT textures (sky arc stage 3a): transmittance +
    /// multiple-scattering, regenerated per frame-locked body. The views are
    /// bound at group-3 bindings 11/12; updates rewrite the SAME textures so
    /// no bind group ever rebuilds for a planet change.
    atmo_trans_tex: wgpu::Texture,
    pub atmo_trans_view: wgpu::TextureView,
    atmo_ms_tex: wgpu::Texture,
    pub atmo_ms_view: wgpu::TextureView,
    /// Params of the last LUT upload, so per-frame update calls no-op until
    /// the frame-locked body (or its atmosphere) actually changes.
    atmo_lut_params: Option<atmo_luts::TransLutParams>,
    /// The per-frame sky-view LUT pass (stage 3b-2). Encoded before the main
    /// passes whenever the camera is frame-locked near an atmosphere body;
    /// stage 3c samples its target for the near-surface sky.
    pub sky_view: sky_view::SkyViewPass,
    /// This frame's sky-view inputs, stashed by the lib.rs atmosphere hook.
    /// None = not near an atmosphere body = the pass is skipped.
    pub sky_view_uniform: Option<sky_view::SkyViewUniform>,
    /// Sun shadows on/off (max-graphics default on; zero cost when the sun
    /// is absent - the pass and the shader lookup both self-gate).
    pub sun_shadows: bool,
    /// How dark a fully occluded fragment gets, 0..1 (v0.1104). Consumed as
    /// `mix(1 - strength, 1, pcf)` in the megashader, so 1.0 means an occluded
    /// fragment keeps NO direct sun and is lit by sky irradiance alone.
    ///
    /// Was a hardcoded 0.6 from v0.899 to v0.1103, which left every shadow
    /// holding 40% of full sun IN THE SUN'S OWN WARM COLOUR: measured
    /// shadow/sunlit ratios of 0.412-0.432 against a physical clear-sky
    /// expectation of 0.10-0.20 and blue. That constant was doing the job of
    /// the indirect light the engine did not have; now that sky_ambient exists
    /// (measured: 12.8% of sunlit, sky-blue), the correct value is 1.0 and the
    /// shadows fill from the sky instead of from the sun.
    pub shadow_strength: f32,
    /// Screen-space ambient occlusion (v0.901): contact shading in the
    /// celestial slot. Strength 0 disables the pass entirely.
    ssao: ssao::SsaoPass,
    cloud_composite: cloud_composite::CloudCompositePass,
    cloud_resolve: cloud_resolve::CloudResolvePass,
    /// The temporal cloud map's basis anchor: the camera direction in the
    /// PLANET's local frame, re-anchored by lib.rs only when the camera
    /// drifts past a hysteresis threshold (Wave D fix 2: the first cut
    /// snapped STATELESSLY to a 0.03 rad grid in-shader, and a camera
    /// hovering near a cell boundary flip-flopped the whole map's basis
    /// frame to frame - the operator's "weird left/right flicking that
    /// gets worse the longer we stay"). Unit vector; pushed to the shader
    /// octahedrally through pads 496 + 556.
    pub cloud_map_anchor_local: [f32; 3],
    /// cos(theta_max) of the cloud map's extent (12c): -1 = full sphere
    /// (the pre-12c mapping), larger = the map concentrates its texels
    /// within theta_max of the anchor (orbit: just the planet disc).
    /// Frozen between re-anchors, LOCKSTEP with the anchor above; pushed
    /// through the light3_cone_inner.x pad (offset 512).
    pub cloud_map_cmax: f32,
    /// One-frame resample order (12c): Some((old_anchor_local, old_cmax))
    /// on the frame lib.rs re-anchored the map. The octa pass then looks
    /// history up through the OLD mapping so the re-anchor is invisible.
    /// Rides the legacy camera.light3 position vec4 (offset 128). A Cell
    /// consumed (take) at the pad write, because render_celestial_onto
    /// can run TWICE in one frame (hi-res capture re-render) and a second
    /// octa pass with the flag still up would warp the already-resampled
    /// history a second time (adversarial review finding 4).
    pub cloud_map_resample: std::cell::Cell<Option<([f32; 3], f32)>>,
    /// Per-frame camera translation baseline for the octa pass's history
    /// reprojection (slice B): the camera's PLANET-LOCAL displacement
    /// since last frame, rotated to current world axes, set by lib.rs at
    /// the cloud fill site. Planet-local is the frame the cloud content
    /// lives in - a world-frame baseline slides at orbital speed even
    /// parked (measured 1.3-2.1 km/frame) and smears the map at rest.
    /// Cell + take(): consumed once per celestial render, so the hi-res
    /// double render reprojects by zero on its second pass.
    pub cloud_reproj_delta: std::cell::Cell<Option<[f32; 3]>>,
    /// v0.1246: dispatch the octa pass even at near_mix == 1.0 (the CPU gate
    /// used to skip it entirely, freezing the map while the v0.1244
    /// per-pixel composite still displayed it in every horizon-band pixel -
    /// the operator's stale-daylight night band). Set by lib.rs when the
    /// camera is UNDER the deck (12c regime 3).
    pub cloud_octa_force: bool,
    /// Frames since the octa pass last dispatched (resume-drop bookkeeping).
    pub cloud_octa_idle: std::cell::Cell<u32>,
    /// EMA alpha-floor boost handed to the octa pass via light7_color.y:
    /// 1.0 on resume-after-idle (the map is stale - a fade would replay it),
    /// decaying over a few dispatched frames; also driven by the sun having
    /// moved since the map's content epoch (there was NO sun-delta
    /// invalidation at all - a 20-minute day guarantees stale lighting).
    pub cloud_octa_boost: std::cell::Cell<f32>,
    /// Previous frame's squared reprojection delta, for the EDGE-TRIGGERED
    /// teleport sentinel (v0.1246): the old LEVEL trigger fired every frame
    /// under the planet-spin content sweep (37 km/s at a 20-minute day),
    /// suspending cadence into a full-rate 16.7M-march death spiral that
    /// was itself the operator's 7 FPS. A teleport is a delta SPIKE; a
    /// sweep is a steady level the reprojection handles geometrically.
    pub cloud_prev_delta2: std::cell::Cell<f32>,
    /// Octa-pass march cadence counter (quarter-rate marching over the
    /// 4096 map): increments per celestial render, phase = counter % 4.
    pub cloud_octa_phase: std::cell::Cell<u32>,
    /// NEAR cloud regime flag (12d): true = the half-res screen pass
    /// replaces the octa map entirely (per-pixel march + screen
    /// reprojection; the whole direction-cache ghost family is
    /// structurally impossible there). Set per frame by lib.rs from the
    /// planet's on-screen size with hysteresis.
    pub cloud_mode_near: bool,
    /// 12g crossfade weight between the octa map (0) and the screen pass
    /// (1) at the composite; both passes run while 0 < mix < 1.
    pub cloud_near_mix: f32,
    /// The near-regime screen buffers (created by ensure_cloud_screen).
    pub(crate) cloud_screen: Option<cloud_temporal::CloudScreen>,
    /// Previous frame's camera basis (fwd/right/up) for the screen
    /// pass's reprojection pads. Consumed-and-replaced at the pad write.
    pub cloud_prev_basis: std::cell::Cell<Option<[[f32; 3]; 3]>>,
    /// Camera state for the 12e resolve pass, stashed at the pad-poke
    /// site each frame so march + octa + resolve all see one motion.
    cloud_resolve_frame: std::cell::Cell<cloud_resolve::CloudResolveFrame>,
    /// Cloud shell frame for the fullscreen depth-aware composite (Wave D
    /// slice 1b) - set by lib.rs at the cloud material fill site whenever
    /// the temporal map is armed; None disables the pass.
    pub cloud_composite_frame: Option<cloud_composite::CloudCompositeFrame>,
    pub ssao_strength: f32,
    /// Detail-draw-distance factor (v0.905): scales every shader detail
    /// octave's anti-alias fade so fine structure survives further out.
    /// Synced from Settings each frame; poked into the view_pos.w pad.
    pub detail_distance: f32,
    /// Sea state 0..1 (v0.909): glassy -> ripples -> storm. Poked into the
    /// fill_color.w uniform pad each celestial pass.
    pub sea_state: f32,
    /// Live sea CREST height in metres (v0.1051): ~3 sigma of the FFT sea, or
    /// the trains' fixed 3.1 m. The shader scales its shoal fade by this so
    /// storm waves never punch through the seabed, and the backstop shell's
    /// drop tracks it so a calm day keeps a tight backstop.
    pub sea_crest_m: f32,
    /// Ocean disaster event uniforms (ABYSSAL adoption rung 2, v0.1239):
    /// the 14-row block appended at CameraUniforms' tail (offset 672). All
    /// zeros = dead calm (row 11 w is the shader's active flag). Filled each
    /// frame by lib.rs from the live event pin/lifecycle; poked wholesale
    /// after the full uniform write, like every other pad.
    pub ocean_event_rows: [[f32; 4]; 14],
    /// Underwater extinction strength (v0.1054): 0 = unlimited visibility (the
    /// old behaviour), 1 = full physical seawater absorption. Driven by the
    /// Settings "Underwater clarity" slider, and zero unless the camera is
    /// actually submerged so surface views are untouched.
    pub underwater_ext: f32,
    /// Material ids of the WAVE water shell (v0.1057), so those patches cast
    /// into the sun shadow map and a 10 m crest shadows the trough behind it.
    /// Identified by MATERIAL rather than by an index range into the transparent
    /// list, because v0.1053 stable-sorts that list every frame whenever the
    /// camera is inside the atmosphere - a recorded index range would silently
    /// point at the atmosphere shell instead. The flat BACKSTOP is deliberately
    /// excluded: it is undisplaced and sits below the troughs, so it could only
    /// shadow the seabed.
    pub water_caster_mats: Vec<usize>,
    /// Temporal cloud accumulation state (clouds phase 4): the octa map
    /// pair + their group-3 bind groups. None until the first frame that
    /// activates the path (see cloud_temporal::set_cloud_temporal).
    pub(crate) cloud_temporal: Option<cloud_temporal::CloudTemporal>,
    /// The cloud MATERIAL index whose type-15 draw composites from the
    /// octa map this frame (None = direct march everywhere). Set per
    /// frame by lib.rs alongside the params2.w temporal flag.
    pub(crate) cloud_temporal_mat: Option<usize>,
    /// Draw the water shell on the DEPTH-WRITING pipeline (v0.1060). Set by
    /// lib.rs only when the camera is inside an atmosphere, which is exactly
    /// when v0.1053 also sorts water to the END of the transparent list - so
    /// nothing is submitted after the sea that its depth could wrongly occlude.
    /// From orbit this stays false and the approved space look is untouched.
    pub water_depth_write: bool,
    /// Sea sphere in RENDER space (v0.1061): xyz = planet centre, w = sea-level
    /// radius. Lets any fragment work out how deep it and the camera are, which
    /// is what turns underwater extinction from a whole-screen switch into a
    /// per-ray path integral - the over-under waterline.
    pub sea_sphere: [f32; 4],
    /// Foliage wind for the type-20 vertex branch (v0.1080): xyz = world wind
    /// direction (unit), w = speed m/s. lib.rs sets it each frame from the live
    /// weather; poked into BOTH camera buffers (colour + shadow) at offset 576,
    /// because the shadow pass runs the same vs_main off its own buffer and a
    /// one-buffer poke would cast shadows from a differently-posed tree.
    pub foliage_wind: [f32; 4],
    /// Camera-local day factor for the celestial pass's sun intensity
    /// (v0.1083, BUG-057 #1). 1.0 in space / by default; lib.rs writes the
    /// terminator value each frame when frame-locked to a body. Without it
    /// the celestial pass lit every tree and prop with a constant-2.5 sun
    /// all night (terrain has its own per-fragment gate; nothing else did).
    pub celestial_sun_day: f32,
    /// Fill-light intensity scale for the CELESTIAL pass (v0.998, operator:
    /// "trees were still being illuminated at night"): the default cool fill
    /// never dimmed after sunset, so night forests glowed. lib.rs sets this
    /// from the camera-local daylight while inside an atmosphere; 1.0 in
    /// space keeps the approved orbital look.
    pub fill_scale: f32,
    /// Terrain patch mega-buffer arena (draw-batching increment 1). Lazy:
    /// created on the first patch upload (a planet approach), so sessions
    /// that never activate chunked terrain pay zero VRAM for it.
    pub patch_arena: Option<patch_arena::PatchArena>,
    /// Whether MULTI_DRAW_INDIRECT + INDIRECT_FIRST_INSTANCE were granted
    /// (increment 2): true = the celestial batch is one indirect submit.
    pub patch_indirect: bool,
    /// One INSTANCE_STRIDE of zeros bound at vertex slot 1 for every CLASSIC
    /// draw (the pipelines declare the per-instance attributes; non-batched
    /// draws read element 0 = zeros, which the classic accessors ignore).
    dummy_instance_buf: wgpu::Buffer,
    /// ── Near-field grass strands (v0.1091) ──
    /// ONE shared unit-height tiller mesh, drawn once per visible tiller
    /// through a single instanced draw. `grass_n` is how many instances the
    /// buffer currently holds; zero means the layer draws nothing at all,
    /// which is the state everywhere except standing on vegetated ground.
    grass_mesh: Option<Mesh>,
    /// Detail rung the resident `grass_mesh` was built at, so a Settings
    /// change rebuilds it. See `terrain::grass::grass_detail_key`.
    grass_mesh_key: u32,
    grass_material: usize,
    grass_instance_buf: Option<wgpu::Buffer>,
    grass_instance_cap: usize,
    grass_n: u32,
    /// This frame's batched patch draws, set by the engine before the
    /// celestial render and consumed by it. Instance i in the storage
    /// buffer is draws[i]; the shadow pass reuses the SAME indices, so a
    /// culled shadow subset still addresses its instances correctly.
    pub patch_draws: Vec<PatchDraw>,
    /// Shared model rotation for every batched patch this frame (planet
    /// rotation; patches never scale) + the material they all share.
    pub patch_batch_rot: Mat4,
    pub patch_batch_material: usize,
    /// Tree-card hide radius in metres (v0.912): terrain silhouette cards
    /// within this range of the camera discard (the real 3D tree models
    /// stand there). Mirrors the Settings tree-model distance; 0 = off.
    pub tree_card_hide_m: f32,
    /// Tree-card FAR cutoff (v0.924 vegetation LOD): the silhouette stage's
    /// outer distance in metres (the Settings slider). Cards past it discard.
    pub tree_card_far_m: f32,
    /// Tree-card sprite atlas (v0.961, billboard bake increment 2): 3x2 grid
    /// of side-on baked conifer sprites, bound at group-3 binding 14. Created
    /// zeroed at init (bind groups never rebuild); bake_tree_atlas rewrites
    /// it in place and flips `tree_atlas_ready` (mirrored into the planet
    /// material's params.w bit 2 by lib.rs each frame).
    pub tree_atlas_texture: wgpu::Texture,
    pub tree_atlas_view: wgpu::TextureView,
    pub tree_atlas_ready: bool,
    /// FFT ocean displacement tile (v0.1029): rewritten in place by
    /// upload_water_fft each frame when FFT-ocean mode is on; bind groups
    /// reference the view forever, no rebuilds.
    pub water_fft_texture: wgpu::Texture,
    pub water_fft_view: wgpu::TextureView,
    /// Cloud wind-advection angle (radians, v0.1032): set per frame from
    /// the weather sim (lib.rs), poked into light1_cone_inner.x.
    pub cloud_advect: f32,
    /// Aerial perspective (v0.916): extinction per metre at the CAMERA's
    /// altitude (strength + height falloff folded in by lib.rs; 0 = off).
    pub aerial_sigma: f32,
    /// Aerial slant cap: haze-layer thickness in metres, bounding vertical
    /// sightlines so the sun/orbit stay clear.
    pub aerial_slant_cap: f32,
    /// Aerial in-scatter (sky) color, day/sunset tinted by lib.rs.
    pub aerial_sky: [f32; 3],
    // (cloud_ref_sun() and viewport_size() below expose cur_sun and the
    // surface config for the increment-10 reference-march scene dump.)
    /// WATER sky-mirror altitude gate (environment program W1): how much
    /// of the sky-view LUT the ocean may mirror this frame, 0..1. Set by
    /// lib.rs with the SAME law the atmosphere uses to retire its own
    /// LUT toward orbit ((1 - max(w_alt, w_far)), constants
    /// atmosphere::NEAR_R/FAR_R). Before this gate the water mirrored
    /// the LUT unconditionally at exposure 15 while the drawn sky gated
    /// the SAME table to zero from orbit - the cyan banding the operator
    /// reported at the horizon and around the orbital glint.
    pub water_lut_gate: f32,
    /// Camera's radial up (world), for the slant path bound.
    pub aerial_up: [f32; 3],
    /// GPU pass timing (resource budgets increment 1). `None` when the adapter
    /// has no TIMESTAMP_QUERY feature, in which case the Performance page shows
    /// CPU-side pass times and says so.
    gpu_timers: Option<frame_costs::GpuTimers>,
    /// Throttle for the VRAM/RAM inventory walk (at most once a second, and
    /// only while the Performance page is open).
    inventory_sampled: std::sync::Mutex<Option<std::time::Instant>>,
}

impl Renderer {
    /// Create a new renderer attached to a native winit window.
    #[cfg(feature = "native")]
    pub async fn new_native(window: std::sync::Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        // Cloud-noise generation starts NOW on a background thread so the
        // 384^3 + 256^3 volume bake overlaps adapter/device/shader-compile
        // time; init() recv()s only the unfinished remainder (v0.872).
        //
        // The MIP CHAINS are built here too as of v0.1188. They used to run
        // inline in init(), which was fine at 192^3 (~180 ms) but is ~1.5 s
        // of pure boot-path stall at 384^3 - and it is the same pure-CPU
        // work as the bake, so it belongs in the same overlapped thread.
        // The channel therefore carries finished chains, and the upload
        // side does nothing but write_texture.
        let cloud_rx = {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let threads =
                    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
                let t0 = std::time::Instant::now();
                let shape = cloud_noise::generate_shape(threads);
                let detail = cloud_noise::generate_detail(threads);
                let t_gen = t0.elapsed().as_secs_f32() * 1000.0;
                let shape = cloud_noise::mip_chain(shape, cloud_noise::SHAPE_SIZE);
                let detail = cloud_noise::mip_chain(detail, cloud_noise::DETAIL_SIZE);
                log::info!(
                    "Cloud noise volumes generated in background: {:.0} ms bake + {:.0} ms mips \
                     ({} threads, {}^3 + {}^3, {:.0} MiB)",
                    t_gen,
                    t0.elapsed().as_secs_f32() * 1000.0 - t_gen,
                    threads,
                    cloud_noise::SHAPE_SIZE,
                    cloud_noise::DETAIL_SIZE,
                    (shape.iter().map(|l| l.len()).sum::<usize>()
                        + detail.iter().map(|l| l.len()).sum::<usize>())
                        as f32
                        / (1024.0 * 1024.0),
                );
                let _ = tx.send((shape, detail));
            });
            Some(rx)
        };

        // Ground-texture CPU bake starts NOW too (same overlap trick,
        // v0.1133): the ~1 s of PNG decode + mip-chain building runs while
        // the adapter request and DXC shader compile (~5 s combined) hold
        // the boot path. init() recv()s the finished bake and does only the
        // fast GPU upload at the same point in the sequence as before -- no
        // bind-group or ordering change, just no more blocking bake.
        let ground_rx = {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(ground_textures::bake_all());
            });
            Some(rx)
        };

        // DX12-only on Windows. wgpu unconditionally compiles Vulkan support
        // (hardcoded in wgpu's Cargo.toml for wgpu-core). Even with Backends::DX12,
        // wgpu still loads vulkan-1.dll during instance creation and enumerates
        // Vulkan adapters. Steam/Epic overlay layers hook into this DLL load and
        // cause a segfault (STATUS_ACCESS_VIOLATION) before our code runs.
        //
        // Vulkan support is available for Linux/non-overlay systems via the
        // #[cfg(not(target_os = "windows"))] path below.
        #[cfg(target_os = "windows")]
        let backends = wgpu::Backends::DX12;
        #[cfg(not(target_os = "windows"))]
        let backends = wgpu::Backends::VULKAN | wgpu::Backends::METAL;

        // DXC instead of FXC for DX12 shader compilation (v0.865): FXC spent
        // ~17-21 s of every boot compiling the PBR megashader (profiled from
        // run.log gaps 2026-07-16). DXC compiles the same shaders in a
        // fraction of the time. We load it DYNAMICALLY when dxcompiler.dll +
        // dxil.dll sit beside the exe and fall back to FXC when they do not,
        // so a bare exe still boots (just slower). The static-dxc cargo
        // feature was tried first but its prebuilt lib needs MSVC ATL, which
        // plain Build Tools installs lack. DLL source: the Windows SDK bin
        // dir or a Microsoft DirectXShaderCompiler release (MIT licensed).
        #[cfg(target_os = "windows")]
        let backend_options = {
            let dlls = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| (d.join("dxcompiler.dll"), d.join("dxil.dll"))));
            match dlls {
                Some((dxc, dxil)) if dxc.exists() && dxil.exists() => {
                    log::info!("DX12 shader compiler: DXC ({})", dxc.display());
                    wgpu::BackendOptions {
                        dx12: wgpu::Dx12BackendOptions {
                            shader_compiler: wgpu::Dx12Compiler::DynamicDxc {
                                dxc_path: dxc.to_string_lossy().into_owned(),
                                dxil_path: dxil.to_string_lossy().into_owned(),
                            },
                        },
                        ..Default::default()
                    }
                }
                _ => {
                    log::info!(
                        "DX12 shader compiler: FXC (no dxcompiler.dll beside the exe; boot is slower)"
                    );
                    wgpu::BackendOptions::default()
                }
            }
        };
        #[cfg(not(target_os = "windows"))]
        let backend_options = wgpu::BackendOptions::default();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            backend_options,
            ..Default::default()
        });

        let surface = instance.create_surface(window).expect("Failed to create surface");

        Self::init(instance, surface, width, height, cloud_rx, ground_rx).await
    }

    /// Create a new renderer attached to a WASM canvas element.
    #[cfg(feature = "wasm")]
    pub async fn new_wasm(canvas: web_sys::HtmlCanvasElement) -> Self {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .expect("Failed to create surface from canvas");

        Self::init(instance, surface, width, height, None, None).await
    }

    /// Shared initialization: adapter, device, pipeline, depth buffer.
    /// `cloud_rx`: pre-spawned cloud-noise generation (native path) so the
    /// volume bake overlaps device/shader init; None generates inline (wasm).
    async fn init(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        cloud_rx: Option<std::sync::mpsc::Receiver<(Vec<Vec<u8>>, Vec<Vec<u8>>)>>,
        ground_rx: Option<std::sync::mpsc::Receiver<ground_textures::BakedGround>>,
    ) -> Self {
        // [BootPhase] sub-spans: renderer_init is the single largest boot
        // phase (6.7 s measured 2026-08-14); these marks attribute it so
        // optimization targets are data, not guesses. Grep run.log for
        // "[BootPhase]" after any boot.
        let t_phase = std::time::Instant::now();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("No suitable GPU adapter found");
        log::info!("[BootPhase] adapter_request: {:.0} ms", t_phase.elapsed().as_secs_f32() * 1000.0);
        let t_phase = std::time::Instant::now();

        // v0.784.2 BOOT FIX: the uncapped-lights storage buffer (v0.782) needs
        // fragment-stage storage buffers, but the old `downlevel_webgl2_defaults`
        // profile requests ZERO of them -- so creating the camera bind group
        // layout failed device validation and the app died before the first
        // frame (operator: "I get a flicker but the game never comes up").
        // Request wgpu's standard native limits instead (every Vulkan/DX12-era
        // GPU supports them; the WebGL2 profile only mattered for a wasm target
        // this renderer doesn't build for). Resolution limits still follow the
        // adapter so huge-texture support matches the hardware.
        // 2026-07-11 (ultra star catalog): the 25M-star tier packs into a
        // ~300 MB vertex buffer, which EXCEEDS wgpu's default 256 MiB
        // max_buffer_size limit -- with the default, creating that buffer
        // would fail device validation at world load, the same boot-killing
        // failure class as v0.782. Follow the adapter's real buffer capacity
        // instead (desktop GPUs allow gigabytes); requesting exactly what
        // the adapter reports is always grantable. Every other limit stays
        // at the safe standard defaults. StarRenderer::new additionally
        // trims the star list to whatever THIS device's limit turns out to
        // be, so a small-limit adapter degrades to a partial sky, never a
        // dead app.
        let adapter_limits = adapter.limits();
        let mut required_limits =
            wgpu::Limits::default().using_resolution(adapter_limits.clone());
        required_limits.max_buffer_size = adapter_limits.max_buffer_size;
        // Draw-batching increment 2: request the indirect-draw features IF
        // the adapter has them (intersection = grantable by construction,
        // never a boot risk). When granted, the 12k-patch terrain batch
        // submits as ONE multi_draw_indexed_indirect; when not, the
        // per-draw loop runs on the exact same buffers and shaders.
        let indirect_features = wgpu::Features::MULTI_DRAW_INDIRECT
            | wgpu::Features::INDIRECT_FIRST_INSTANCE;
        // Same intersection trick for TIMESTAMP_QUERY (resource budgets
        // increment 1): asking for exactly what the adapter reports can never
        // fail device creation, which is the v0.782 boot-killer class. Without
        // it the Performance page falls back to CPU-side pass timing.
        let granted_indirect = adapter.features()
            & (indirect_features | wgpu::Features::TIMESTAMP_QUERY);
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("HumanityOS Renderer"),
                    required_features: granted_indirect,
                    required_limits,
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to create device");
        log::info!("[BootPhase] device_request: {:.0} ms", t_phase.elapsed().as_secs_f32() * 1000.0);
        let t_phase = std::time::Instant::now();
        let patch_indirect = granted_indirect.contains(indirect_features);
        log::info!(
            "[PatchBatch] indirect multi-draw: {}",
            if patch_indirect { "SUPPORTED (one submit per batch)" } else { "unsupported (per-draw loop)" }
        );
        // GPU pass timers. `None` = no timestamp queries on this adapter.
        let gpu_timers = frame_costs::GpuTimers::new(&device, &queue);
        frame_costs::set_gpu_timing(gpu_timers.is_some());
        log::info!(
            "[FrameCosts] GPU pass timing: {}",
            if gpu_timers.is_some() { "timestamp queries" } else { "CPU fallback (adapter has no TIMESTAMP_QUERY)" }
        );

        // Surface configuration
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Live screenshot command (v0.639): request COPY_SRC on the swapchain surface so the
        // rendered frame can be read back to a PNG. Most backends support this alongside
        // RENDER_ATTACHMENT; check first rather than assuming, so a backend that doesn't just
        // gets a clean `capture_current_frame` error instead of a wgpu validation panic.
        let supports_frame_capture = surface_caps.usages.contains(wgpu::TextureUsages::COPY_SRC);
        let mut surface_usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if supports_frame_capture {
            surface_usage |= wgpu::TextureUsages::COPY_SRC;
        }
        let config = wgpu::SurfaceConfiguration {
            usage: surface_usage,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // Depth buffer
        let (depth_texture, depth_view) = Self::create_depth_texture(&device, width, height);

        // Off-screen scene texture (for post-processing: bloom, etc.)
        let (scene_tex, scene_tex_view) = Self::create_scene_texture(&device, width, height, surface_format);
        let t_unit = std::time::Instant::now();
        let bloom_pass = bloom::BloomPass::new(&device, width, height, surface_format);
        log::info!("[BootPhase]   bloom_pass: {:.0} ms", t_unit.elapsed().as_secs_f32() * 1000.0);
        let t_unit = std::time::Instant::now();
        let godray_pass = godrays::GodrayPass::new(&device, surface_format);
        log::info!("[BootPhase]   godray_pass: {:.0} ms", t_unit.elapsed().as_secs_f32() * 1000.0);
        let t_unit = std::time::Instant::now();
        let ssao_pass = ssao::SsaoPass::new(&device, surface_format);
        let cloud_composite_pass = cloud_composite::CloudCompositePass::new(&device, surface_format);
        let cloud_resolve_pass = cloud_resolve::CloudResolvePass::new(&device);
        log::info!("[BootPhase]   ssao_pass: {:.0} ms", t_unit.elapsed().as_secs_f32() * 1000.0);

        // Shader + pipeline. The megashader compiles from the EMBEDDED
        // source; when assets/shaders/pbr_simple.wgsl exists on disk (dev
        // checkout, portable rig) hot-reload arms (v0.924): saving the file
        // revalidates + rebuilds the PSOs in seconds instead of a full
        // rebuild-and-reboot. Detection is a once-per-second MTIME poll,
        // not a filesystem watcher - the notify backend silently delivered
        // ZERO events through the rig's NTFS junction (probe-proven, both
        // on the junction path and the canonicalized real path), and one
        // metadata read per second is free and works through every alias
        // and editor write strategy. See poll_shader_reload.
        let shader_loader = shader_loader::ShaderLoader::new();
        let t_unit = std::time::Instant::now();
        let shader = shader_loader.load_embedded_pbr(&device);
        log::info!("[BootPhase]   pbr_module: {:.0} ms", t_unit.elapsed().as_secs_f32() * 1000.0);
        // Terrain-batch variant module (draw-batching increment 1): same
        // assembled source with the OBJECT-SOURCE block swapped for the
        // storage-array version (see shader_loader::batched_variant_of).
        let t_unit = std::time::Instant::now();
        let batch_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pbr_simple (terrain-batch variant)"),
            source: wgpu::ShaderSource::Wgsl(
                shader_loader::boot_pbr_batch_source().into(),
            ),
        });
        log::info!("[BootPhase]   batch_module: {:.0} ms", t_unit.elapsed().as_secs_f32() * 1000.0);
        #[cfg(feature = "native")]
        let shader_hot = shader_loader::find_shaders_dir().and_then(|dir| {
            // v0.973 source split: the megashader is assembled from the
            // numbered parts under assets/shaders/pbr/; the poll tracks the
            // NEWEST part mtime so saving any part triggers a rebuild.
            let mtime = shader_loader::pbr_parts_mtime(&dir)?;
            log::info!("[HotReload] armed: polling part mtimes under {:?}", dir.join("pbr"));
            Some((dir, mtime))
        });
        let t_unit = std::time::Instant::now();
        let pipeline = Pipeline::new(&device, surface_format, &shader, &batch_shader);
        log::info!("[BootPhase]   pipeline_new: {:.0} ms", t_unit.elapsed().as_secs_f32() * 1000.0);
        // World-space thin-line pipeline — reuses the SAME camera BGL so
        // it can bind the existing camera_bind_group (full view-proj).
        let t_unit = std::time::Instant::now();
        let (particle_pipeline_alpha, particle_pipeline_additive, particle_frame_bgl) =
            particles::build_particle_pipelines(
                &device,
                config.format,
                &pipeline.camera_bind_group_layout,
            );
        let particle_frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Frame UB"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let particle_frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Frame BG"),
            layout: &particle_frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: particle_frame_buffer.as_entire_binding(),
            }],
        });
        let line_pipeline = line::build_line_pipeline(
            &device,
            surface_format,
            &pipeline.camera_bind_group_layout,
        );
        log::info!("[BootPhase]   particles_and_line: {:.0} ms", t_unit.elapsed().as_secs_f32() * 1000.0);

        // Camera uniform buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&CameraUniforms {
                view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                view_pos: [0.0; 4],
                light_positions: [[0.0; 4]; 8],
                light_colors: [[0.0; 4]; 8],
                light_spot: [[0.0, -1.0, 0.0, -1.0]; 8],
                light_cone_inner: [[0.0; 4]; 8],
                light_count: [0.0; 4],
                // Default directional lights (match former shader constants)
                sun_direction: [0.3, 1.0, 0.5, 2.5],
                sun_color: [1.0, 0.95, 0.9, 0.0],
                fill_direction: [-0.5, 0.3, -0.3, 0.6],
                fill_color: [0.4, 0.5, 0.7, 0.0],
                ocean_event: [[0.0; 4]; 14],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Uncapped light storage buffer (v0.782): starts with room for 1024
        // lights (64 KB) and doubles on demand (recreating the bind group).
        let lights_capacity = 1024_usize;
        let lights_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Scene Lights Storage Buffer"),
            size: (lights_capacity * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Light-tile lists (clustering L1b): fixed-size, rewritten per frame
        // by update_light_tiles when tiling is enabled.
        let tile_counts_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Tile Counts"),
            size: (light_tiles::TILE_COUNT * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tile_indices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Tile Indices"),
            size: (light_tiles::TILE_COUNT * light_tiles::TILE_CAP * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &pipeline.camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: tile_counts_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: tile_indices_buffer.as_entire_binding(),
                },
            ],
        });

        // Dynamic object uniform buffer — holds up to MAX_OBJECTS entries (module const).
        // Each entry is aligned to 256 bytes (wgpu minimum uniform buffer offset alignment).
        let uniform_align = 256_u64; // minimum uniform buffer offset alignment
        let object_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Object Uniform Buffer (Dynamic)"),
            size: uniform_align * MAX_OBJECTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Zero per-instance data for classic draws (vertex slot 1; see the
        // dummy_instance_buf field doc). Must be a whole INSTANCE_STRIDE or
        // the layout's location-6 attribute reads past the buffer.
        let dummy_instance_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Dummy Instance Data"),
            contents: &[0u8; mesh::INSTANCE_STRIDE as usize],
            usage: wgpu::BufferUsages::VERTEX,
        });

        let object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Object Bind Group"),
            layout: &pipeline.object_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &object_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ObjectUniforms>() as u64),
                }),
            }],
        });

        // Group-3 defaults (v0.811, per-pixel planet imagery): one shared
        // sampler + a 1x1 white fallback texture so EVERY draw can bind
        // group 3 (the shared pipeline layout requires it) while only
        // textured planet materials carry real imagery.
        let albedo_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Albedo Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat, // longitude wraps
            address_mode_v: wgpu::AddressMode::ClampToEdge, // latitude clamps
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest, // single mip; sampled at level 0
            ..Default::default()
        });
        // Tiling-material sampler (v0.1089, baked bark): repeat BOTH axes,
        // trilinear, 8x anisotropy. wgpu requires all three filters Linear
        // when anisotropy_clamp > 1, and a real mip chain to filter between -
        // both of which the bark bake provides, and neither of which planet
        // imagery does, hence the second sampler rather than a change to the
        // shared one.
        let bark_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Tiling Material Sampler (bark)"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Albedo Fallback Texture (1x1 white)"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let white_view = white_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Tiling 3D cloud-noise volumes (clouds increment 3; res raised
        // 128/64 -> 192/128 in v0.872, 192/128 -> 384/256 in v0.1188):
        // generated procedurally, deterministic, no repo assets, shared by
        // every group-3 bind group at bindings 2..4. Generation AND the mip
        // chains run on a BACKGROUND thread spawned at the very top of
        // renderer creation, overlapping the DXC shader compiles, so the
        // bigger volumes cost boot nothing: this recv() only blocks for
        // whatever remainder has not finished by the time uploads start.
        let gen_start = std::time::Instant::now();
        let (shape_chain, detail_chain) = match cloud_rx {
            Some(rx) => rx.recv().expect("cloud noise generator thread died"),
            None => {
                // Fallback (wasm / callers without the pre-spawn): inline.
                let threads =
                    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
                (
                    cloud_noise::mip_chain(
                        cloud_noise::generate_shape(threads),
                        cloud_noise::SHAPE_SIZE,
                    ),
                    cloud_noise::mip_chain(
                        cloud_noise::generate_detail(threads),
                        cloud_noise::DETAIL_SIZE,
                    ),
                )
            }
        };
        log::info!(
            "Cloud noise volumes ready: {s}^3 shape ({sl} mips) + {d}^3 detail ({dl} mips) \
             (waited {:.0} ms at upload)",
            gen_start.elapsed().as_secs_f32() * 1000.0,
            s = cloud_noise::SHAPE_SIZE,
            d = cloud_noise::DETAIL_SIZE,
            sl = shape_chain.len(),
            dl = detail_chain.len(),
        );
        // Each volume carries a FULL CPU-built mip chain (box-filtered by
        // cloud_noise::mip_chain): the raymarch samples with a distance +
        // step-length LOD so far clouds read band-limited (pre-averaged)
        // noise instead of aliasing full-frequency texels - the structural
        // fix for distant shimmer that no amount of temporal averaging can
        // supply (v0.1161, clouds phase 5).
        //
        // MIP COUNT IS DERIVED, never hardcoded: `chain.len()` is 9 at the
        // v0.1188 sizes (384 -> ... -> 3 -> 1) where it was 8, and the
        // WGSL's cloud_lod clamp + carve-width table must match it.
        let make_volume = |label: &str, size: u32, chain: Vec<Vec<u8>>| -> wgpu::TextureView {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: size,
                },
                mip_level_count: chain.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                // Linear (NOT sRGB): this is noise data, not color.
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let mut mip_size = size;
            for (level, data) in chain.iter().enumerate() {
                // Uploaded in z-slabs (v0.1188): the 384^3 base level is
                // 216 MiB, which would be one staging allocation sitting
                // 40 MiB under wgpu's default max_buffer_size. The slab
                // list is pure arithmetic from cloud_noise::upload_slabs
                // and is unit-tested there (a mis-sliced copy would only
                // show up as a scrambled volume in a rendered frame).
                for (z0, depth, b0, b1) in
                    cloud_noise::upload_slabs(mip_size, cloud_noise::UPLOAD_SLAB_BYTES)
                {
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: level as u32,
                            origin: wgpu::Origin3d { x: 0, y: 0, z: z0 },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &data[b0..b1],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * mip_size),
                            rows_per_image: Some(mip_size),
                        },
                        wgpu::Extent3d {
                            width: mip_size,
                            height: mip_size,
                            depth_or_array_layers: depth,
                        },
                    );
                }
                mip_size = (mip_size / 2).max(1);
            }
            tex.create_view(&wgpu::TextureViewDescriptor::default())
        };
        let cloud_shape_view =
            make_volume("Cloud Shape Noise", cloud_noise::SHAPE_SIZE, shape_chain);
        let cloud_detail_view =
            make_volume("Cloud Detail Noise", cloud_noise::DETAIL_SIZE, detail_chain);
        let cloud_tile_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cloud Noise Tile Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // Trilinear across the mip chain: the raymarch passes an
            // explicit LOD per sample (textureSampleLevel).
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let weather_map_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Live Weather Map"),
            size: wgpu::Extent3d {
                width: WEATHER_MAP_W,
                height: WEATHER_MAP_H,
                depth_or_array_layers: 1,
            },
            // Full mip chain (increment 11b): a 27.8 km texel point-sampled
            // through a steep smoothstep was per-texel keep/kill stipple
            // from orbit. Mips are box-filtered CPU-side on every weather
            // refresh (update_weather_map) so a wide-footprint sample reads
            // the area's MEAN cloud fraction, which the fractional-coverage
            // law (G2) then renders AS areal coverage.
            mip_level_count: WEATHER_MAP_MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let weather_map_view = weather_map_tex.create_view(&wgpu::TextureViewDescriptor::default());

        // ── Sun shadow map resources (v0.899) ──
        const SHADOW_MAP_SIZE: u32 = 4096;
        let shadow_map_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Sun Shadow Map"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_map_view = shadow_map_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_comparison_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Comparison Sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        log::info!("[BootPhase] shaders_and_pipelines: {:.0} ms", t_phase.elapsed().as_secs_f32() * 1000.0);
        let t_phase = std::time::Instant::now();
        // Ground PBR texture array (v0.907): the ambientCG sets from
        // assets/textures/ground/, or a neutral 1x1 fallback that renders
        // identically to the pre-texture look. The CPU bake ran on a
        // background thread since before the adapter request (v0.1133);
        // recv() here collects it (usually already finished) and only the
        // fast GPU upload happens on the boot path. The wasm/fallback path
        // still bakes inline via load().
        let ground_textures = match ground_rx.and_then(|rx| rx.recv().ok()) {
            Some(baked) => ground_textures::upload(&device, &queue, baked),
            None => ground_textures::load(&device, &queue),
        };
        log::info!("[BootPhase] ground_textures_upload: {:.0} ms", t_phase.elapsed().as_secs_f32() * 1000.0);
        let t_phase = std::time::Instant::now();

        // Atmosphere LUTs (sky arc stage 3a, v0.945): transmittance 256x64 +
        // multiple-scattering 32x32, CPU-generated (atmo_luts.rs) and uploaded
        // as Rgba16Float. Seeded with Earth-like params at boot; refreshed per
        // frame-locked body via update_atmo_luts (no-op when params repeat).
        let make_lut_tex = |label: &str, w: u32, h: u32| {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (tex, view)
        };
        let (atmo_trans_tex, atmo_trans_view) = make_lut_tex(
            "Atmo Transmittance LUT",
            atmo_luts::TRANS_LUT_W as u32,
            atmo_luts::TRANS_LUT_H as u32,
        );
        let (atmo_ms_tex, atmo_ms_view) = make_lut_tex(
            "Atmo Multiple-Scattering LUT",
            atmo_luts::MS_LUT_W as u32,
            atmo_luts::MS_LUT_H as u32,
        );
        let sky_view_pass = sky_view::SkyViewPass::new(&device, &atmo_trans_view, &atmo_ms_view);
        // Tree-card sprite atlas (v0.961): fixed-size, zero-filled (alpha 0 =
        // sprite branch discards until the bake lands), swapchain format so
        // bake targets copy_texture_to_texture straight in.
        let tree_atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Tree Card Sprite Atlas"),
            size: wgpu::Extent3d {
                width: billboard_bake::ATLAS_COLS * billboard_bake::ATLAS_TILE_PX,
                height: billboard_bake::ATLAS_ROWS * billboard_bake::ATLAS_TILE_PX,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let tree_atlas_view = tree_atlas_texture.create_view(&Default::default());
        // FFT ocean tile (v0.1029 increment 1; v0.1031 increment 2 packs
        // RGBA = height, slope_u, slope_v, foam): 128x128 Rgba32Float the
        // type-16 water VS (height) and FS (slopes + Jacobian whitecaps)
        // read via textureLoad. wgpu zero-initializes it, so until the
        // engine's first upload (or with the setting off) it is a flat sea.
        let water_fft_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FFT Ocean Displacement"),
            // v0.1040: 128x256 - cascade A (64 m tile) in rows [0,128),
            // cascade B (256 m tile) in rows [128,256).
            size: wgpu::Extent3d {
                width: crate::terrain::ocean_fft::FFT_N as u32,
                height: crate::terrain::ocean_fft::FFT_TEX_H as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let water_fft_view = water_fft_texture.create_view(&Default::default());
        {
            let (rp, h) = atmosphere::shell_packing(0.06, 8500.0, 6.371e6);
            let params = atmo_luts::TransLutParams {
                tint: [0.18, 0.42, 1.0],
                density_mul: 1.0,
                rp,
                h,
            };
            let write = |tex: &wgpu::Texture, texels: &[[f32; 4]], w: u32, h_px: u32| {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &atmo_luts::lut_to_f16_bytes(texels),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(8 * w),
                        rows_per_image: Some(h_px),
                    },
                    wgpu::Extent3d { width: w, height: h_px, depth_or_array_layers: 1 },
                );
            };
            write(
                &atmo_trans_tex,
                &atmo_luts::transmittance_lut(&params),
                atmo_luts::TRANS_LUT_W as u32,
                atmo_luts::TRANS_LUT_H as u32,
            );
            write(
                &atmo_ms_tex,
                &atmo_luts::multiple_scattering_lut(&params),
                atmo_luts::MS_LUT_W as u32,
                atmo_luts::MS_LUT_H as u32,
            );
        }
        let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Uniforms"),
            size: 96, // mat4 (64) + params vec4 (16) + params2 vec4 (16)
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Camera Buffer"),
            size: std::mem::size_of::<camera::CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Light Camera BG"),
            layout: &pipeline.camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: light_camera_buffer.as_entire_binding(),
                },
                // The camera layout also carries the v0.782 lights storage
                // buffer; the shadow pass never reads it, but the layout
                // requires SOMETHING bound - share the main buffer.
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: tile_counts_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: tile_indices_buffer.as_entire_binding(),
                },
            ],
        });

        // 1x1 dummy depth for the shadow pass's own group 3 (see field doc).
        let dummy_depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy Shadow Depth"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_depth_view = dummy_depth_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let default_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Albedo Fallback Bind Group"),
            layout: &pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&albedo_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&cloud_shape_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&cloud_detail_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&cloud_tile_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&weather_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&shadow_comparison_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: shadow_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&ground_textures.view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(&ground_textures.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&atmo_trans_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&atmo_ms_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(&sky_view_pass.target_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(&tree_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: wgpu::BindingResource::TextureView(&water_fft_view),
                },
            ],
        });
        let shadow_pass_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Pass Texture BG"),
            layout: &pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&albedo_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&cloud_shape_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&cloud_detail_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&cloud_tile_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&weather_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&dummy_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&shadow_comparison_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: shadow_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&ground_textures.view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(&ground_textures.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&atmo_trans_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&atmo_ms_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(&sky_view_pass.target_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(&tree_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: wgpu::BindingResource::TextureView(&water_fft_view),
                },
            ],
        });

        log::info!("[BootPhase] luts_buffers_bindgroups: {:.0} ms", t_phase.elapsed().as_secs_f32() * 1000.0);

        Self {
            device,
            queue,
            surface,
            config,
            depth_texture,
            depth_view,
            pipeline,
            #[cfg(feature = "native")]
            shader_hot,
            #[cfg(feature = "native")]
            shader_hot_checked: std::time::Instant::now(),
            line_pipeline,
            particle_pipeline_alpha,
            particle_pipeline_additive,
            particle_frame_buffer,
            particle_vb_alpha: None,
            particle_vb_additive: None,
            gpu_particles: None,
            particle_vb_alpha_cap: 0,
            particle_vb_additive_cap: 0,
            particle_frame_bind_group,
            camera_buffer,
            camera_bind_group,
            lights_buffer,
            tile_counts_buffer,
            tile_indices_buffer,
            tile_px: (0.0, 0.0),
            tree_atlas_texture,
            tree_atlas_view,
            tree_atlas_ready: false,
            water_fft_texture,
            water_fft_view,
            cloud_advect: 0.0,
            lights_capacity,
            object_buffer,
            object_bind_group,
            meshes: Vec::new(),
            materials: Vec::new(),
            scene_texture: scene_tex,
            scene_view: scene_tex_view,
            bloom: Some(bloom_pass),
            godrays: godray_pass,
            godray_intensity: 0.55,
            ssao: ssao_pass,
            cloud_composite: cloud_composite_pass,
            cloud_resolve: cloud_resolve_pass,
            cloud_composite_frame: None,
            cloud_map_anchor_local: [0.0, 1.0, 0.0],
            cloud_map_cmax: -1.0,
            cloud_map_resample: std::cell::Cell::new(None),
            cloud_reproj_delta: std::cell::Cell::new(None),
            cloud_octa_force: false,
            cloud_octa_idle: std::cell::Cell::new(0),
            cloud_octa_boost: std::cell::Cell::new(0.0),
            cloud_prev_delta2: std::cell::Cell::new(0.0),
            cloud_octa_phase: std::cell::Cell::new(0),
            cloud_mode_near: false,
            cloud_near_mix: 0.0,
            cloud_screen: None,
            cloud_prev_basis: std::cell::Cell::new(None),
            cloud_resolve_frame: std::cell::Cell::new(Default::default()),
            ssao_strength: 0.55,
            detail_distance: 1.0,
            sea_state: 0.35,
            sea_crest_m: crate::terrain::ocean_waves::MAX_WAVE_HEIGHT_M,
            ocean_event_rows: [[0.0; 4]; 14],
            underwater_ext: 0.0,
            water_caster_mats: Vec::new(),
            cloud_temporal: None,
            cloud_temporal_mat: None,
            water_depth_write: false,
            sea_sphere: [0.0, 0.0, 0.0, 0.0],
            // Matches the shader's own fallback direction; speed 0 means the
            // shader uses its 4 m/s default until lib.rs stamps live weather.
            foliage_wind: [0.86, 0.0, 0.32, 0.0],
            celestial_sun_day: 1.0,
            fill_scale: 1.0,
            patch_arena: None,
            patch_indirect,
            dummy_instance_buf,
            grass_mesh: None,
            grass_mesh_key: 0,
            grass_material: usize::MAX,
            grass_instance_buf: None,
            grass_instance_cap: 0,
            grass_n: 0,
            patch_draws: Vec::new(),
            patch_batch_rot: Mat4::IDENTITY,
            patch_batch_material: 0,
            tree_card_hide_m: 0.0,
            tree_card_far_m: 1500.0,
            aerial_sigma: 0.0,
            aerial_slant_cap: 25_000.0,
            aerial_sky: [0.0, 0.0, 0.0],
            water_lut_gate: 0.0,
            aerial_up: [0.0, 1.0, 0.0],
            gpu_timers,
            inventory_sampled: std::sync::Mutex::new(None),
            bloom_intensity: 0.0, // Off by default; set > 0 to enable
            bloom_threshold: 0.8,
            // Defaults match camera.uniforms()'s former hardcoded sun/fill, so behaviour is unchanged
            // until lights are set (v0.571).
            cur_lights: Vec::new(),
            cur_sun: ([0.3, 1.0, 0.5], [1.0, 0.95, 0.9], 2.5),
            cur_fill: ([-0.5, 0.3, -0.3], [0.4, 0.5, 0.7], 0.6),
            supports_frame_capture,
            albedo_sampler,
            bark_sampler,
            bark_materials: std::collections::HashMap::new(),
            default_texture_bind_group,
            cloud_shape_view,
            cloud_detail_view,
            cloud_tile_sampler,
            weather_map_tex,
            weather_map_view,
            shadow_map_view,
            shadow_uniform_buffer,
            light_camera_buffer,
            light_camera_bind_group,
            shadow_pass_texture_bind_group,
            dummy_depth_view,
            ground_textures,
            sky_view: sky_view_pass,
            sky_view_uniform: None,
            atmo_trans_tex,
            atmo_trans_view,
            atmo_ms_tex,
            atmo_ms_view,
            atmo_lut_params: None,
            shadow_comparison_sampler,
            sun_shadows: true,
            shadow_strength: 1.0,
        }
    }

    /// Handle window/canvas resize.
    /// Apply the Settings VSync toggle (v0.909 - the toggle used to save a
    /// value nothing read). AutoVsync caps at the monitor refresh;
    /// AutoNoVsync uncaps (mailbox/immediate as the platform allows).
    pub fn set_vsync(&mut self, on: bool) {
        let mode = if on {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        if self.config.present_mode != mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (tex, view) = Self::create_depth_texture(&self.device, width, height);
        self.depth_texture = tex;
        self.depth_view = view;
        // Resize scene texture + bloom
        let fmt = self.config.format;
        let (st, sv) = Self::create_scene_texture(&self.device, width, height, fmt);
        self.scene_texture = st;
        self.scene_view = sv;
        if let Some(ref mut bloom) = self.bloom {
            bloom.resize(&self.device, width, height);
        }
    }

    /// Current surface aspect ratio.
    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    /// Surface texture format (needed by egui-wgpu renderer).
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Megashader hot-reload (v0.924, dev-aid): when pbr_simple.wgsl
    /// changes on disk, VALIDATE the new source with naga first (a mid-edit
    /// save logs and keeps the old pipelines - never crashes), then rebuild
    /// the four PSOs in place. Bind group layouts are reused, so every live
    /// bind group stays valid and the running world is untouched. Turns the
    /// shader iteration loop from a 3+ minute rebuild-and-reboot into a
    /// few-second recompile with full world state intact. Call once per
    /// frame; try_recv makes the idle cost effectively zero.
    #[cfg(feature = "native")]
    pub fn poll_shader_reload(&mut self) {
        if self.shader_hot_checked.elapsed().as_secs_f32() < 1.0 {
            return;
        }
        self.shader_hot_checked = std::time::Instant::now();
        let Some((shaders_dir, last_mtime)) = self.shader_hot.as_mut() else {
            return;
        };
        // v0.973 source split: the change signal is the newest mtime across
        // the parts under shaders/pbr/; the reload reassembles them all.
        let Some(mtime) = shader_loader::pbr_parts_mtime(shaders_dir) else {
            return;
        };
        if mtime == *last_mtime {
            return;
        }
        *last_mtime = mtime;
        let shaders_dir = shaders_dir.clone();
        let Some(source) = shader_loader::assembled_pbr_source_from_dir(&shaders_dir) else {
            log::error!("[HotReload] failed to assemble shader parts under {shaders_dir:?}");
            return;
        };
        if let Err(e) = shader_loader::validate_wgsl(&source) {
            log::error!("[HotReload] megashader REJECTED (old pipelines kept): {e}");
            return;
        }
        // The terrain-batch variant derives from the SAME on-disk source,
        // so shader edits keep applying to both pipeline families. Validate
        // it separately: a marker rename breaks only the variant.
        let Some(batch_source) = shader_loader::batched_variant_of(&source) else {
            log::error!(
                "[HotReload] OBJECT-SOURCE markers missing (old pipelines kept) - \
                 did 00-bindings-vertex.wgsl lose its BEGIN/END OBJECT-SOURCE comments?"
            );
            return;
        };
        if let Err(e) = shader_loader::validate_wgsl(&batch_source) {
            log::error!("[HotReload] terrain-batch variant REJECTED (old pipelines kept): {e}");
            return;
        }
        let t0 = std::time::Instant::now();
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("pbr megashader (hot-reload)"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let batch_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("pbr megashader (terrain-batch, hot-reload)"),
                source: wgpu::ShaderSource::Wgsl(batch_source.into()),
            });
        let format = self.config.format;
        self.pipeline
            .recreate_pipelines(&self.device, format, &module, &batch_module);
        log::info!(
            "[HotReload] megashader reassembled + 6 PSOs rebuilt in {:.1}s",
            t0.elapsed().as_secs_f32()
        );
    }

    /// Current surface dimensions.
    pub fn surface_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Register a mesh and return its handle (index).
    pub fn add_mesh(&mut self, mesh: Mesh) -> usize {
        let idx = self.meshes.len();
        self.meshes.push(mesh);
        idx
    }

    /// Upload a terrain patch into the mega-buffer arena (creating the
    /// arena on first use). None = arena full; the caller falls back to a
    /// classic per-patch Mesh (graceful, logged inside the arena).
    pub fn patch_arena_upload(
        &mut self,
        vertices: &[mesh::Vertex],
        indices: &[u32],
    ) -> Option<patch_arena::PatchSlot> {
        let _cost = frame_costs::stage("cpu.patch_upload");
        if self.patch_arena.is_none() {
            self.patch_arena = Some(patch_arena::PatchArena::new(
                &self.device,
                &self.pipeline.patch_bind_group_layout,
            ));
        }
        self.patch_arena
            .as_mut()
            .expect("just created")
            .upload(&self.queue, vertices, indices)
    }

    /// Return a patch's arena ranges to the free lists (eviction).
    pub fn patch_arena_release(&mut self, slot: patch_arena::PatchSlot) {
        if let Some(arena) = self.patch_arena.as_mut() {
            arena.release(slot);
        }
    }

    // ── Near-field grass strands (v0.1091) ───────────────────────────────
    //
    // ONE mesh, ONE material, ONE draw, N instances. The engine hands over a
    // fresh instance list each frame (positions are render-space and the
    // floating origin moves every frame, so there is nothing to keep); this
    // grows the GPU buffer when needed and remembers the count.

    /// Upload the shared tiller mesh. Idempotent - the first call wins, and
    /// callers are expected to just call it whenever grass is wanted rather
    /// than track readiness themselves.
    pub fn ensure_grass_mesh(&mut self) {
        // NOT first-call-wins any more (v0.1105): the tiller mesh's blade and
        // segment counts now follow the Settings vegetation slider, so a cache
        // that never invalidated would make the slider LOOK like it worked -
        // the harvest responds instantly - while the OLD mesh stayed on screen
        // until the next restart. That is a bug shaped exactly like the fix,
        // which is the kind that survives longest.
        let key = crate::terrain::planet_chunks::grass_detail_key();
        if self.grass_mesh.is_some() && self.grass_mesh_key == key {
            return;
        }
        let first = self.grass_mesh.is_none();
        let (builder, stats) = crate::terrain::planet_chunks::grass_tiller_mesh();
        self.grass_mesh = Some(Mesh::from_vertices(
            &self.device,
            &builder.vertices,
            &builder.indices,
        ));
        self.grass_mesh_key = key;
        if first {
            // Type 23: the grass arm of the plant wind family. base_color is
            // unused (the per-instance packed colour rules); params.w must stay
            // 0 so the type-19 wind opt-in cannot be misread. Registered ONCE -
            // a rebuild must not allocate a second material slot.
            self.grass_material =
                self.add_material_typed([1.0, 1.0, 1.0, 1.0], 0.0, 0.92, 23.0);
        }
        log::info!(
            "[Grass] shared tiller mesh: {} blades x {} segments, {} triangles, {} verts",
            stats.blades,
            stats.segments,
            stats.triangles,
            builder.vertices.len()
        );
    }

    /// Replace this frame's grass instance set. Empty = the layer draws
    /// nothing (the normal state away from vegetated ground).
    pub fn set_grass_instances(&mut self, inst: &[mesh::GrassInstance]) {
        let _cost = frame_costs::stage("cpu.grass_upload");
        self.grass_n = inst.len() as u32;
        if inst.is_empty() {
            return;
        }
        self.ensure_grass_mesh();
        if self.grass_instance_cap < inst.len() {
            // Grow in generous steps: a walking player's tiller count breathes
            // by a few percent per frame, and reallocating a VERTEX buffer
            // mid-frame is the one thing worth avoiding here.
            let cap = (inst.len() * 3 / 2).max(8192);
            self.grass_instance_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Grass Instance Data"),
                size: (cap as u64) * mesh::INSTANCE_STRIDE,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.grass_instance_cap = cap;
        }
        if let Some(buf) = self.grass_instance_buf.as_ref() {
            self.queue.write_buffer(buf, 0, bytemuck::cast_slice(inst));
        }
    }

    /// How many grass instances were submitted for the last frame (diag).
    pub fn grass_instance_count(&self) -> u32 {
        self.grass_n
    }

    /// Upload both FFT-ocean cascade realizations (each FFT_N x FFT_N of
    /// packed [height, slope_u, slope_v, foam] texels) into the
    /// persistent 128x256 tile: A at row 0, B at row FFT_N. Called once
    /// per frame while FFT-ocean mode is on; ~512 KB, no rebuild.
    pub fn upload_water_fft(&self, a: &[[f32; 4]], b: &[[f32; 4]]) {
        let _cost = frame_costs::stage("cpu.water_upload");
        let n = crate::terrain::ocean_fft::FFT_N as u32;
        debug_assert_eq!(a.len(), (n * n) as usize);
        debug_assert_eq!(b.len(), (n * n) as usize);
        for (texels, row0) in [(a, 0u32), (b, n)] {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.water_fft_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: row0, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(texels),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(16 * n),
                    rows_per_image: Some(n),
                },
                wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 1 },
            );
        }
    }

    /// Upload a fresh live-weather grid (RG8, WEATHER_W x WEATHER_H) into the
    /// persistent weather texture. No bind-group rebuild needed - every group
    /// already references this texture's view.
    /// Regenerate + upload both atmosphere LUTs for the given planet params.
    /// Cheap to call per frame: no-ops unless the params changed since the
    /// last upload (a body switch or a live atmosphere edit). CPU generation
    /// is ~milliseconds; the textures are rewritten in place so bind groups
    /// never rebuild.
    pub fn update_atmo_luts(&mut self, params: atmo_luts::TransLutParams) {
        let _cost = frame_costs::stage("cpu.atmo_luts");
        let queue = &self.queue;
        if self.atmo_lut_params == Some(params) {
            return;
        }
        let write = |tex: &wgpu::Texture, texels: &[[f32; 4]], w: u32, h_px: u32| {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &atmo_luts::lut_to_f16_bytes(texels),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(8 * w),
                    rows_per_image: Some(h_px),
                },
                wgpu::Extent3d { width: w, height: h_px, depth_or_array_layers: 1 },
            );
        };
        write(
            &self.atmo_trans_tex,
            &atmo_luts::transmittance_lut(&params),
            atmo_luts::TRANS_LUT_W as u32,
            atmo_luts::TRANS_LUT_H as u32,
        );
        write(
            &self.atmo_ms_tex,
            &atmo_luts::multiple_scattering_lut(&params),
            atmo_luts::MS_LUT_W as u32,
            atmo_luts::MS_LUT_H as u32,
        );
        self.atmo_lut_params = Some(params);
        log::info!("Atmosphere LUTs regenerated (rp={:.4} h={:.6})", params.rp, params.h);
    }

    pub fn update_weather_map(&self, queue: &wgpu::Queue, rg: &[u8]) {
        let _cost = frame_costs::stage("cpu.weather_upload");
        let (w, h) = (
            WEATHER_MAP_W,
            WEATHER_MAP_H,
        );
        if rg.len() != (w * h * 2) as usize {
            log::warn!("[Weather] bad grid size {} - ignored", rg.len());
            return;
        }
        // Upload the base + a CPU box-filtered mip chain (increment 11b).
        // ~1.4 MB of filtering per refresh (every few minutes) - noise.
        let mut level: Vec<u8> = rg.to_vec();
        let (mut lw, mut lh) = (w, h);
        for mip in 0..WEATHER_MAP_MIPS {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.weather_map_tex,
                    mip_level: mip,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &level,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(lw * 2),
                    rows_per_image: Some(lh),
                },
                wgpu::Extent3d {
                    width: lw,
                    height: lh,
                    depth_or_array_layers: 1,
                },
            );
            if mip + 1 == WEATHER_MAP_MIPS {
                break;
            }
            let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
            let mut next = vec![0u8; (nw * nh * 2) as usize];
            for y in 0..nh {
                for x in 0..nw {
                    let (x0, y0) = ((x * 2).min(lw - 1), (y * 2).min(lh - 1));
                    let (x1, y1) = ((x * 2 + 1).min(lw - 1), (y * 2 + 1).min(lh - 1));
                    for c in 0..2u32 {
                        let s = level[((y0 * lw + x0) * 2 + c) as usize] as u32
                            + level[((y0 * lw + x1) * 2 + c) as usize] as u32
                            + level[((y1 * lw + x0) * 2 + c) as usize] as u32
                            + level[((y1 * lw + x1) * 2 + c) as usize] as u32;
                        next[((y * nw + x) * 2 + c) as usize] = ((s + 2) / 4) as u8;
                    }
                }
            }
            level = next;
            lw = nw;
            lh = nh;
        }
    }

    /// Replace the mesh at `idx` in place: drops the old mesh (wgpu frees its vertex/index buffers)
    /// and reuses the slot, so a per-frame editor rebuild (a room drag, a machine move) never leaks
    /// meshes. No-op if idx is out of range. (v0.531: the renderer is otherwise append-only.)
    pub fn replace_mesh(&mut self, idx: usize, mesh: Mesh) {
        if let Some(slot) = self.meshes.get_mut(idx) {
            *slot = mesh;
        }
    }

    /// Set room lights for the next render call — UNCAPPED (v0.782). Lights go
    /// to a storage buffer (64 bytes each: pos+intensity, color+range, spot,
    /// cone), which doubles in capacity (recreating the camera bind group) when
    /// exceeded; only `light_count` in the camera uniform bounds the shader
    /// loop. Each light is a point light or a spot with a real cone (v0.639).
    /// There is deliberately no software cap: the practical ceiling is GPU
    /// fill cost, visible in the F2 overlay's live light count + FPS.
    /// Rebuild + upload the per-tile light lists (clustering L1b). Call after
    /// `set_point_lights` with the SAME light slice (tile indices index into
    /// it). `enabled = false` zeroes the tile-pixel poke, which sends the
    /// shader down the classic full-loop path.
    pub fn update_light_tiles(
        &mut self,
        lights: &[light::RoomLight],
        view_proj: &glam::Mat4,
        cam_pos: glam::Vec3,
        screen: (u32, u32),
        enabled: bool,
    ) {
        let _cost = frame_costs::stage("cpu.light_tiles");
        if !enabled || lights.is_empty() {
            self.tile_px = (0.0, 0.0);
            return;
        }
        let bins: Vec<light_tiles::BinLight> = lights
            .iter()
            .map(|l| {
                if l.cos_outer <= -1.5 {
                    // LINE light (sentinel -2.0): the whole segment pos..dir
                    // emits. Bin the enclosing sphere: midpoint + half-length
                    // added to the range (conservative).
                    let mid = (l.pos + l.dir) * 0.5;
                    light_tiles::BinLight {
                        pos: mid,
                        range: l.range + (l.dir - l.pos).length() * 0.5,
                    }
                } else {
                    light_tiles::BinLight { pos: l.pos, range: l.range }
                }
            })
            .collect();
        let (counts, indices) = light_tiles::bin_lights(&bins, view_proj, cam_pos, screen);
        self.queue
            .write_buffer(&self.tile_counts_buffer, 0, bytemuck::cast_slice(&counts));
        self.queue
            .write_buffer(&self.tile_indices_buffer, 0, bytemuck::cast_slice(&indices));
        self.tile_px = (
            (screen.0 as f32 / light_tiles::TILE_COLS as f32).max(1.0),
            (screen.1 as f32 / light_tiles::TILE_ROWS as f32).max(1.0),
        );
    }

    pub fn set_point_lights(&mut self, lights: &[light::RoomLight]) {
        let _cost = frame_costs::stage("cpu.lights");
        // Grow the storage buffer by doubling if needed (bind groups are
        // immutable, so a grow recreates the camera bind group too).
        if lights.len() > self.lights_capacity {
            let mut cap = self.lights_capacity.max(1);
            while cap < lights.len() {
                cap *= 2;
            }
            self.lights_capacity = cap;
            self.lights_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Scene Lights Storage Buffer"),
                size: (cap * 64) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.camera_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Camera Bind Group"),
                layout: &self.pipeline.camera_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.tile_counts_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.tile_indices_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.lights_buffer.as_entire_binding(),
                    },
                ],
            });
        }
        // Pack ALL lights: [pos.xyz, intensity][color.rgb, range][spot dir.xyz,
        // cos_outer][cos_inner, 0, 0, 0] — matches the WGSL GpuLight struct.
        if !lights.is_empty() {
            let packed: Vec<[f32; 16]> = lights
                .iter()
                .map(|l| {
                    [
                        l.pos.x, l.pos.y, l.pos.z, l.intensity,
                        l.color[0], l.color[1], l.color[2], l.range,
                        l.dir.x, l.dir.y, l.dir.z, l.cos_outer,
                        l.cos_inner, 0.0, 0.0, 0.0,
                    ]
                })
                .collect();
            self.queue
                .write_buffer(&self.lights_buffer, 0, bytemuck::cast_slice(&packed));
        }
        // light_count still lives in the camera uniform: offset past view_proj
        // (64) + view_pos (16) + the four legacy [8] light arrays (4 * 128) =
        // 592 bytes. (The legacy arrays are no longer written — the shader
        // reads the storage buffer — but they stay allocated so no offset
        // after them shifts.)
        let light_count = [lights.len() as f32, 0.0_f32, 0.0, 0.0];
        self.queue.write_buffer(
            &self.camera_buffer,
            592,
            bytemuck::cast_slice(&light_count),
        );
        // Store for re-injection by the home passes (the count in the uniform
        // gets clobbered by the full camera-uniform write at offset 0; this is
        // the authoritative copy). (v0.571)
        self.cur_lights = lights.to_vec();
    }

    /// Inject the live local-light state (point/spot lights + sun + fill) into a base camera
    /// uniform (v0.571, spot cones added v0.639). The home `_onto` passes call this so the
    /// full-uniform write at offset 0 carries the real lights instead of `camera.uniforms()`'s
    /// empty/default set.
    fn lit_uniform(&self, mut u: camera::CameraUniforms) -> camera::CameraUniforms {
        // v0.782: lights live in the storage buffer now; the legacy [8] uniform
        // arrays are left zeroed (kept only so no byte offset shifts). The
        // COUNT is the full uncapped list — it bounds the shader's storage-
        // buffer loop.
        u.light_positions = [[0.0; 4]; 8];
        u.light_colors = [[0.0; 4]; 8];
        u.light_spot = [[0.0, -1.0, 0.0, -1.0]; 8];
        u.light_cone_inner = [[0.0; 4]; 8];
        u.light_count = [self.cur_lights.len() as f32, 0.0, 0.0, 0.0];
        let (sd, sc, si) = self.cur_sun;
        u.sun_direction = [sd[0], sd[1], sd[2], si];
        u.sun_color = [sc[0], sc[1], sc[2], 0.0];
        let (fd, fc, fi) = self.cur_fill;
        u.fill_direction = [fd[0], fd[1], fd[2], fi];
        u.fill_color = [fc[0], fc[1], fc[2], 0.0];
        u
    }

    /// Scene illumination scalar for effects that carry no lighting of
    /// their own (particle billboards): 1.0 in daylight, clamped to a small
    /// moon/ambient floor at night so unlit rain dims to near-invisible
    /// instead of glowing, but never vanishes entirely.
    fn scene_illum(&self) -> f32 {
        let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let (_, sc, si) = self.cur_sun;
        let (_, fc, fi) = self.cur_fill;
        (si * luma(sc) + fi * luma(fc)).clamp(0.04, 1.0)
    }

    /// How many scene lights are currently uploaded (v0.782): feeds the F2
    /// overlay so the operator can watch the uncapped count against FPS.
    pub fn light_count(&self) -> usize {
        self.cur_lights.len()
    }

    /// Set the directional sun light for the next render call.
    /// `direction` points toward the light source (will be normalized in the shader).
    /// `color` is the RGB color, `intensity` is the brightness multiplier.
    pub fn set_sun_light(&mut self, direction: Vec3, color: [f32; 3], intensity: f32) {
        // sun_direction sits at byte offset 608 (after light_cone_inner ends at 592, +light_count's 16)
        let sun_dir = [direction.x, direction.y, direction.z, intensity];
        let sun_col = [color[0], color[1], color[2], 0.0_f32];
        self.queue.write_buffer(
            &self.camera_buffer,
            608,
            bytemuck::cast_slice(&sun_dir),
        );
        self.queue.write_buffer(
            &self.camera_buffer,
            624,
            bytemuck::cast_slice(&sun_col),
        );
        self.cur_sun = ([direction.x, direction.y, direction.z], color, intensity); // v0.571
    }

    /// Set the fill light for the next render call.
    /// `direction` points toward the light source (will be normalized in the shader).
    /// `color` is the RGB color, `intensity` is the brightness multiplier.
    pub fn set_fill_light(&mut self, direction: Vec3, color: [f32; 3], intensity: f32) {
        // fill_direction sits at byte offset 640
        let fill_dir = [direction.x, direction.y, direction.z, intensity];
        let fill_col = [color[0], color[1], color[2], 0.0_f32];
        self.queue.write_buffer(
            &self.camera_buffer,
            640,
            bytemuck::cast_slice(&fill_dir),
        );
        self.queue.write_buffer(
            &self.camera_buffer,
            656,
            bytemuck::cast_slice(&fill_col),
        );
        self.cur_fill = ([direction.x, direction.y, direction.z], color, intensity); // v0.571
    }

    /// Render a frame with the given camera and objects.
    /// Batched object-uniform upload (v0.891): build every per-object uniform
    /// block in ONE staging vec and issue ONE queue.write_buffer, instead of a
    /// queue call per object. At 3000+ terrain patches the per-call overhead
    /// (per-call validation + copy scheduling) dominated CPU frame time.
    fn upload_object_uniforms<'a>(&self, objects: impl Iterator<Item = &'a RenderObject>) {
        const ALIGN: usize = 256;
        let mut staging: Vec<u8> = Vec::with_capacity(ALIGN * 1024);
        for (i, obj) in objects.enumerate() {
            if i >= MAX_OBJECTS {
                break;
            }
            let clean =
                Mat4::from_scale_rotation_translation(obj.scale, obj.rotation, obj.position);
            // Normal matrix from the CLEAN transform - the fade smuggled into
            // the w row below would corrupt the inverse.
            let normal_matrix = clean.inverse().transpose();
            // LOD crossfade (v0.920) rides model[0].w; the vertex shader
            // rebuilds the homogeneous w after transforming, so this slot is
            // free per-object metadata (see RenderObject::fade).
            let mut model = clean;
            model.x_axis.w = obj.fade;
            let uniforms = ObjectUniforms {
                model: model.to_cols_array_2d(),
                normal_matrix: normal_matrix.to_cols_array_2d(),
            };
            // Pad the previous slot out to the 256-byte dynamic-offset
            // alignment, then append this 128-byte block.
            staging.resize(i * ALIGN, 0);
            staging.extend_from_slice(bytemuck::bytes_of(&uniforms));
        }
        if !staging.is_empty() {
            self.queue.write_buffer(&self.object_buffer, 0, &staging);
        }
    }

    pub fn render(&self, camera: &Camera, objects: &[RenderObject]) -> Result<(), wgpu::SurfaceError> {
        let (output, _view) = self.render_scene(camera, objects)?;
        output.present();
        Ok(())
    }

    /// Acquire the surface texture and clear it with a solid color.
    /// Used when rendering UI-only frames (no 3D scene).
    pub fn acquire_surface_cleared(
        &self,
        clear_color: wgpu::Color,
    ) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        // UI-only frame: no world pass runs, so the budget numbers stay frozen
        // at the last rendered world frame (resource budgets increment 1).
        self.frame_costs_begin(false);
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Clear Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                timestamp_writes: self.pass_timer("gpu.clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        Ok((output, view))
    }

    /// Render the 3D scene and return the surface texture + view for further
    /// overlay rendering (e.g., egui). Caller must call `output.present()`
    /// after all overlay passes are complete.
    pub fn render_scene(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
    ) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        // Update camera uniforms
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                timestamp_writes: self.pass_timer("gpu.scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // reverse-Z: clear to 0 (farthest)
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline.render_pipeline);
            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // One batched object-uniform upload (v0.891).
            let uniform_align = 256_u64;
            self.upload_object_uniforms(objects.iter());

            let mut bound_material = usize::MAX;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) {
                    Some(m) => m,
                    None => continue,
                };
                let material = match self.materials.get(obj.material) {
                    Some(m) => m,
                    None => continue,
                };

                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): terrain patches share one material, so 3000+
                // redundant rebinds per frame collapse to one.
                if bound_material != obj.material {
                    bound_material = obj.material;
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok((output, view))
    }

    /// Render 3D objects onto an already-acquired surface texture.
    /// Uses LoadOp::Load to preserve existing content (e.g. stars rendered first).
    pub fn render_scene_onto(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.scene");
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Overlay Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Overlay Pass"),
                timestamp_writes: self.pass_timer("gpu.scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve star background
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // reverse-Z: clear to 0 (farthest)
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline.render_pipeline);
            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // One batched object-uniform upload (v0.891).
            let uniform_align = 256_u64;
            self.upload_object_uniforms(objects.iter());

            let mut bound_material = usize::MAX;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) {
                    Some(m) => m,
                    None => continue,
                };
                let material = match self.materials.get(obj.material) {
                    Some(m) => m,
                    None => continue,
                };

                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): terrain patches share one material, so 3000+
                // redundant rebinds per frame collapse to one.
                if bound_material != obj.material {
                    bound_material = obj.material;
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render TRANSPARENT objects (glass windows, the portal) over the already-drawn scene,
    /// alpha-blended (v0.456). Call AFTER `render_scene_onto`: it preserves the colour
    /// (LoadOp::Load) and LOADS the scene depth (so glass behind a wall is occluded) but does
    /// not WRITE depth (so you see through it). A material's `base_color.a` is its opacity.
    pub fn render_transparent_onto(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.transparent");
        if objects.is_empty() {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Transparent Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Transparent Pass"),
                timestamp_writes: self.pass_timer("gpu.transparent"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // blend over the scene
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // test against the opaque scene; no write
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline.transparent_pipeline);
            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // One batched object-uniform upload (v0.891).
            let uniform_align = 256_u64;
            self.upload_object_uniforms(objects.iter());

            let mut bound_material = usize::MAX;
            let mut bound_material = usize::MAX;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): terrain patches share one material, so 3000+
                // redundant rebinds per frame collapse to one.
                if bound_material != obj.material {
                    bound_material = obj.material;
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render editor GIZMOS on top of everything (v0.560): same as the transparent pass but with the
    /// depth-test-disabled `overlay_pipeline`, so corner orbs / the avatar / rings show THROUGH walls
    /// + floors. Call AFTER `render_transparent_onto`. Reuses the shared object buffer (the prior pass
    /// already drew), so the writes are safe.
    pub fn render_overlay_onto(&self, camera: &Camera, objects: &[RenderObject], view: &wgpu::TextureView) {
        let _cost = frame_costs::stage("cpu.overlay");
        if objects.is_empty() {
            return;
        }
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Overlay Encoder") });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay Pass"),
                timestamp_writes: self.pass_timer("gpu.overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    // CLEAR depth (reverse-Z far = 0.0) so gizmos ignore the world but still depth-sort
                    // among themselves; the colour is Loaded so they blend over the rendered scene.
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            render_pass.set_pipeline(&self.pipeline.overlay_pipeline);
            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            // One batched object-uniform upload (v0.891).
            let uniform_align = 256_u64;
            self.upload_object_uniforms(objects.iter());
            let mut bound_material = usize::MAX;
            let mut bound_material = usize::MAX;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): terrain patches share one material, so 3000+
                // redundant rebinds per frame collapse to one.
                if bound_material != obj.material {
                    bound_material = obj.material;
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render CELESTIAL bodies (planet + Sun + solar-system bodies) onto the frame with a
    /// HUGE far plane, so they are not clipped by the gameplay far (~500 m). Call BETWEEN the
    /// star pass and `render_scene_onto`: it preserves the stars (LoadOp::Load color) and
    /// clears its own depth so the bodies depth-sort among themselves; the interior scene then
    /// clears depth again and draws OVER the bodies' color where home geometry exists. (v0.450)
    /// Crepuscular god rays (v0.895): call BETWEEN the celestial pass and
    /// the scene pass, while the shared depth buffer still holds the
    /// terrain + bodies silhouettes (the scene pass clears it right after).
    /// `sun_dir` = world direction TOWARD the sun; the pass skips itself
    /// when the sun projects behind the camera or intensity is 0.
    pub fn render_godrays_onto(
        &self,
        camera: &Camera,
        sun_dir: Vec3,
        view: &wgpu::TextureView,
        weather_scale: f32,
    ) {
        let _cost = frame_costs::stage("cpu.godrays");
        // Settings slider at 0 = pass off entirely (v0.907).
        if self.godray_intensity <= 0.001 {
            return;
        }
        // The SAME projection the celestial pass rendered depth with
        // (reverse-Z, far plane at 1e13) — a mismatched matrix would park
        // the sun uv in the wrong place and bend every shaft.
        let proj = Mat4::perspective_rh(
            camera.fov_degrees.to_radians(),
            camera.aspect,
            1.0e13,
            1.0,
        );
        let view_proj = proj * camera.view_matrix();
        self.godrays.render(
            &self.device,
            &self.queue,
            &self.depth_view,
            view,
            view_proj,
            camera.effective_position(),
            sun_dir,
            camera.aspect,
            self.godray_intensity * weather_scale.clamp(0.0, 1.0),
            self.pass_timer("gpu.godrays"),
        );
    }

    /// Screen-space ambient occlusion (v0.901): call right after
    /// render_godrays_onto, same celestial slot (depth still holds terrain +
    /// vegetation). Multiplies contact shade into the color target.
    /// The current sun (direction, color, intensity) for the increment-10
    /// reference-march scene dump - cur_sun itself stays private.
    pub fn cloud_ref_sun(&self) -> ([f32; 3], [f32; 3], f32) {
        self.cur_sun
    }

    /// Swapchain size (w, h) for the same dump - the reference reconstructs
    /// pixel rays from fov + viewport rows.
    pub fn viewport_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn render_ssao_onto(&self, camera: &Camera, view: &wgpu::TextureView) {
        let _cost = frame_costs::stage("cpu.ssao");
        // Settings slider at 0 = pass off entirely (v0.907).
        if self.ssao_strength <= 0.001 {
            return;
        }
        // The SAME projection the celestial depth was rendered with; its
        // [2][2] / [3][2] elements linearize reverse-Z depth in the shader.
        let proj = Mat4::perspective_rh(
            camera.fov_degrees.to_radians(),
            camera.aspect,
            1.0e13,
            1.0,
        );
        let m = proj.to_cols_array_2d();
        // True focal length in pixels — the shader reconstructs view-space
        // positions from it, so the small-angle px-per-radian approximation
        // is no longer good enough (v0.1100 estimator rebuild, BUG-062).
        let focal_px = self.config.height as f32 * 0.5
            / (camera.fov_degrees.to_radians() * 0.5).tan().max(1.0e-4);
        self.ssao.render(
            &self.device,
            &self.queue,
            &self.depth_view,
            view,
            m[2][2],
            m[3][2],
            focal_px,
            // Contact-AO neighborhood. 0.4 m, not the old 1.6 m: contact
            // shading is a decimetre-scale effect; 1.6 m let every trunk
            // shade ground far behind it (the BUG-062 aura).
            0.4,
            self.ssao_strength,
            self.pass_timer("gpu.ssao"),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_celestial_onto(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
        transparent: &[RenderObject],
        sun_dir: Vec3,
        time_s: f32,
        // Cloud ground shadows (v0.898): (cloud seed, deck coverage, enable).
        // Poked into the light_count.yzw pads after the full uniform write,
        // so the type-12 terrain branch can sample the sky's coverage field.
        cloud_shadow: (f32, f32, bool),
        // [FFT-ocean flag, then camera planet-frame position mod 64 m
        // (v0.902)]: the precision anchor for sub-8 m micro detail plus the
        // v0.1029 water-mode toggle. Poked into light0_cone_inner.xyzw.
        ground_anchor: [f32; 4],
        // FFT cascade-B anchor + water-weld LOD scale (v0.1040/41):
        // xyz = camera planet-frame position mod 256 m, w = the
        // selection's px_per_rad/split_px. Poked into
        // light4_cone_inner.xyzw.
        ocean_anchor256: [f32; 4],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.celestial");
        if objects.is_empty() && transparent.is_empty() {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&camera.celestial_uniforms()),
        );
        // Cloud clock (shader type 15): app-start-relative seconds, parked in
        // sun_color.w -- a documented-unused pad in CameraUniforms, so the
        // animated cloud deck needed NO uniform-layout change (the same
        // no-layout-churn rule as the type-14 material packing). Offset 636 =
        // sun_color (624) + 12 bytes to its w component. Written before the
        // sun poke below so both land in this pass's uniform snapshot.
        self.queue.write_buffer(&self.camera_buffer, 636, bytemuck::bytes_of(&time_s));
        // Cloud-ground-shadow params in the light_count yzw pads (offsets
        // 592 + 4/8/12; documented unused in CameraUniforms).
        let cs = [
            cloud_shadow.0,
            cloud_shadow.1,
            if cloud_shadow.2 { 1.0_f32 } else { 0.0 },
        ];
        self.queue.write_buffer(&self.camera_buffer, 596, bytemuck::cast_slice(&cs));
        // Point lights reach the CELESTIAL pass too (v0.976, the v0.953/954
        // dark-grid mystery): `Camera::uniforms()` hardcodes light_count.x = 0
        // and `celestial_uniforms()` inherits it, so every fragment drawn in
        // this pass - the planet TERRAIN, and everything riding it - looped
        // over zero lights while the storage buffer sat full. The ship
        // interior lit because the scene pass rewrites the uniform with the
        // real count. Poke the live count (light_count.x, offset 592) exactly
        // like the yzw pads above.
        let lc = self.cur_lights.len() as f32;
        self.queue.write_buffer(&self.camera_buffer, 592, bytemuck::bytes_of(&lc));
        // FFT-ocean flag + micro-detail anchor in light0_cone_inner.xyzw
        // (offset 464: .x = water mode, .yzw = the v0.902 anchor).
        self.queue
            .write_buffer(&self.camera_buffer, 464, bytemuck::cast_slice(&ground_anchor));
        // Cloud wind-advection angle in light1_cone_inner.x (offset 480,
        // beside the aerial params in .y/.z; v0.1032): the zonal rotation
        // the weather-map lookups apply so cloud masses drift with the
        // live wind between MODIS refreshes.
        self.queue
            .write_buffer(&self.camera_buffer, 480, bytemuck::bytes_of(&self.cloud_advect));
        // FFT cascade-B anchor + weld K in light4_cone_inner.xyzw
        // (offset 528; the light4 pads are unused - aerial data stops at
        // light3, v0.1040/41).
        self.queue
            .write_buffer(&self.camera_buffer, 528, bytemuck::cast_slice(&ocean_anchor256));
        // Detail-distance factor in the view_pos.w pad (offset 64 + 12).
        self.queue
            .write_buffer(&self.camera_buffer, 76, bytemuck::bytes_of(&self.detail_distance));
        // Sea state 0..1 in the fill_color.w pad (offset 656 + 12; the fill
        // light's alpha is never read). 0 = glassy calm, 0.5 = ripples,
        // 1 = storm chop + breaking crests. Fed by the game weather's wind
        // at the player (lib.rs) or the showcase {"sea":x} dev override.
        self.queue
            .write_buffer(&self.camera_buffer, 668, bytemuck::bytes_of(&self.sea_state));
        // Live sea crest (m) in light5_cone_inner.x (offset 544; the light5 pads
        // are unused - aerial data stops at light3 and the ocean anchor owns
        // light4). The shader's shoal fade reads it, v0.1051.
        self.queue
            .write_buffer(&self.camera_buffer, 544, bytemuck::bytes_of(&self.sea_crest_m));
        // Ocean disaster event block at the CameraUniforms TAIL (offset 672,
        // pinned by camera.rs::ocean_event_block_sits_at_the_struct_tail).
        // Written after the wholesale uniform write like every pad poke; all
        // zeros when no event is live, and the shader's row-11 flag branch
        // skips the field entirely in that case.
        self.queue.write_buffer(
            &self.camera_buffer,
            672,
            bytemuck::cast_slice(&self.ocean_event_rows),
        );
        // TRUE screen pixel angle in light5_cone_inner.z (offset 552) -
        // Wave B, environment program increment 9. The cloud march's
        // ray-cone footprint used a hardcoded ~1 mrad guess
        // (CLOUD_PIX_ANG_SCREEN); the real value is 2*tan(fov/2)/rows,
        // which at 90 deg fov over 1387 rows is ~1.44 mrad - the guess
        // under-read the footprint by ~40% and every mip pick with it.
        let pix_ang = 2.0 * (camera.fov_degrees.to_radians() * 0.5).tan()
            / (self.config.height.max(1) as f32);
        self.queue
            .write_buffer(&self.camera_buffer, 552, bytemuck::bytes_of(&pix_ang));
        // Cloud-map basis anchor, octahedrally encoded into two spare pads
        // (496 = light2_cone_inner.x, 556 = light5_cone_inner.w). The
        // shell/octa shaders decode it in cloud_map_axis_world; the
        // composite pass receives the raw vector through its own uniform.
        // LOCKSTEP: the decode in 40-clouds.wgsl must mirror this encode.
        fn octa_encode(a: [f32; 3]) -> (f32, f32) {
            let denom = a[0].abs() + a[1].abs() + a[2].abs();
            let (mut ox, mut oz) = (a[0] / denom.max(1e-9), a[2] / denom.max(1e-9));
            if a[1] < 0.0 {
                let (fx, fz) = (ox, oz);
                ox = (1.0 - fz.abs()) * if fx >= 0.0 { 1.0 } else { -1.0 };
                oz = (1.0 - fx.abs()) * if fz >= 0.0 { 1.0 } else { -1.0 };
            }
            (ox, oz)
        }
        let (ox, oz) = octa_encode(self.cloud_map_anchor_local);
        self.queue
            .write_buffer(&self.camera_buffer, 496, bytemuck::bytes_of(&ox));
        self.queue
            .write_buffer(&self.camera_buffer, 556, bytemuck::bytes_of(&oz));
        // 12c extent: the frozen cos(theta_max) in light3_cone_inner.x
        // (offset 512), and the OLD params + resample flag in the legacy
        // (unused since the storage-buffer light list) camera.light3
        // position vec4 at offset 128: x/y = old anchor octa pair, z = old
        // cos(theta_max), w = 1 on the single frame after a re-anchor. The
        // octa pass reprojects its history through the old mapping on that
        // frame, which is what makes a re-anchor invisible.
        self.queue
            .write_buffer(&self.camera_buffer, 512, bytemuck::bytes_of(&self.cloud_map_cmax));
        // take() consumes the order: a second render_celestial_onto call in
        // the same frame (hi-res capture) must run its octa pass with the
        // flag DOWN - the history is already in the new mapping by then.
        let old_pads: [f32; 4] = match self.cloud_map_resample.take() {
            Some((a, cm)) => {
                let (oox, ooz) = octa_encode(a);
                [oox, ooz, cm, 1.0]
            }
            None => [0.0, 0.0, -1.0, 0.0],
        };
        self.queue
            .write_buffer(&self.camera_buffer, 128, bytemuck::cast_slice(&old_pads));
        // Slice B translation reprojection: this frame's PLANET-LOCAL
        // camera displacement (from lib.rs, rotated to world axes) + flag
        // in the legacy camera.light4 position vec4 (offset 144). take():
        // one octa reprojection per delivered delta - the hi-res double
        // render reprojects by zero on its second pass. Parked cameras
        // deliver ~0 by construction (planet-local frame), so statics
        // stay converged; only real content-relative motion reprojects.
        // w encodes flag + march cadence phase: 0 = reprojection off (no
        // baseline), 1..4.9 = on with quarter-cadence phase (w - 1) - the
        // octa pass marches only the 2x2 cell matching the phase each
        // frame (4096-map brute force at the old 2048 per-frame cost).
        let phase = {
            let p = self.cloud_octa_phase.get();
            self.cloud_octa_phase.set(p.wrapping_add(1));
            (p % 4) as f32
        };
        // True-TELEPORT test for the 12e resolve (adversarial review of the
        // march/resolve split, finding 1): the resolve's screen history is
        // translation-exact via per-pixel first-hit distances, so sustained
        // fast flight does NOT invalidate it - only a jump large enough
        // that the reprojection itself is meaningless (~15 degrees of
        // parallax at the slab distance, mirroring the octa pass's own
        // per-texel teleport guard). Coupling snap to the CADENCE sentinel
        // below (threshold ~2 screen pixels of parallax, ~8 m/frame near
        // the deck) would drop the history on EVERY frame of an ordinary
        // approach flight and hand the operator raw unconverged march
        // static - the exact regression 12e exists to cure.
        let mut resolve_teleport = false;
        let delta_pads: [f32; 4] = match self.cloud_reproj_delta.take() {
            Some(d) if self.cloud_temporal_mat.is_some() => {
                let d2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                if let Some(f) = self.cloud_composite_frame.as_ref() {
                    let eye = camera.effective_position();
                    let dc = ((eye.x - f.center[0]).powi(2)
                        + (eye.y - f.center[1]).powi(2)
                        + (eye.z - f.center[2]).powi(2))
                    .sqrt()
                        - f.rt * f.planet_r;
                    let d_slab = dc.max(3.0e3);
                    let tele = 0.25 * d_slab;
                    resolve_teleport = d2 > tele * tele;
                }
                // Cadence-suspension threshold, ANGULAR not absolute
                // (ghost-echo round 6): a cadence-skipped block re-warps
                // its own already-warped content for up to 3 frames, and
                // iterated bilinear warping at more than ~2 map texels of
                // shift per frame smears block-wise ghost terraces (the
                // operator's fading "old mirrors" during sustained
                // sub-teleport flight - the old flat 2 km threshold let
                // 10-60-texel shifts through). Suspend cadence (sentinel
                // w = 9: march everything) whenever the frame delta
                // exceeds ~2 texels of parallax at the cloud slab's
                // distance; correctness costs frames only while moving
                // that fast.
                // SCREEN-RELATIVE threshold + small-disc veto (the
                // deep-space lag report): ghosting only matters at the
                // scale a SCREEN pixel can show, so the shift budget is
                // 2x the larger of the map texel and the screen pixel
                // angle - at orbit that is ~9x more headroom than the
                // map-texel form, which had the sentinel firing on every
                // frame of ordinary space cruising and full-rate
                // marching 16.7M texels for a few-hundred-px disc. And
                // when the planet is small on screen (sentinel_ok false,
                // px < 700), the sentinel never fires at all: cadence
                // ghosting on a small disc is sub-pixel, while the
                // march-all cost is the lag.
                let sentinel = self
                    .cloud_composite_frame
                    .as_ref()
                    .map(|f| {
                        if !f.sentinel_ok {
                            return false;
                        }
                        let eye = camera.effective_position();
                        let dc = ((eye.x - f.center[0]).powi(2)
                            + (eye.y - f.center[1]).powi(2)
                            + (eye.z - f.center[2]).powi(2))
                        .sqrt()
                            - f.rt * f.planet_r;
                        let d_slab = dc.max(3.0e3);
                        let k = (1.0 - f.cmax).clamp(1.0e-3, 2.0);
                        let texel_ang = (2.0 * k).sqrt() / 4096.0;
                        let screen_ang = 2.0
                            * (camera.fov_degrees.to_radians() * 0.5).tan()
                            / (self.config.height.max(1) as f32);
                        let thresh = 2.0 * texel_ang.max(screen_ang) * d_slab;
                        // EDGE-TRIGGERED (v0.1246): require the level AND a
                        // spike vs last frame. Under the sustained planet-
                        // spin content sweep (km-scale delta EVERY frame on
                        // the 20-minute day) the old level trigger fired
                        // continuously: cadence suspended, 16.7M marches per
                        // frame, FPS pinned ~7 - and the low FPS made each
                        // per-frame delta bigger, locking the spiral. A
                        // sweep's reprojection is geometrically valid (a
                        // coherent fetch N texels away); only a genuine JUMP
                        // (teleport/re-park) invalidates history, and a jump
                        // is a spike: delta far above the running level.
                        let prev2 = self.cloud_prev_delta2.replace(d2);
                        d2 > thresh * thresh && d2 > prev2 * 16.0 + 1.0
                    })
                    .unwrap_or(false);
                if sentinel {
                    [d[0], d[1], d[2], 9.0]
                } else {
                    [d[0], d[1], d[2], 1.0 + phase]
                }
            }
            _ => [0.0, 0.0, 0.0, 0.0],
        };
        // Reprojection diagnostics (slice B bring-up): a parked camera must
        // read ~0 here. Throttled ~2 s; drop after the operator confirms
        // the smear is dead.
        {
            let d2 = delta_pads[0] * delta_pads[0]
                + delta_pads[1] * delta_pads[1]
                + delta_pads[2] * delta_pads[2];
            static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let now_s = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now_s != LAST.swap(now_s, std::sync::atomic::Ordering::Relaxed) {
                log::info!("[CloudReproj] frame delta {:.3} m", d2.sqrt());
            }
        }
        self.queue
            .write_buffer(&self.camera_buffer, 144, bytemuck::cast_slice(&delta_pads));
        // 12d near-regime screen reprojection: the PREVIOUS frame's camera
        // basis in the legacy light5/6/7 position vec4s (offsets 160/176/
        // 192, unused since the storage-buffer light list). light5.xyz =
        // prev forward, light6.xyz = prev right, light7.xyz = prev up;
        // light5.w = tan(fov/2), light6.w = aspect (both current - the
        // projection does not change frame to frame). The basis is taken
        // from the view MATRIX rows, so surface-mode and world-Y cameras
        // both reproject through exactly what the GPU rendered with. On
        // the first frame (no stored basis) the current basis is written,
        // which makes reprojection the identity - correct for a fresh
        // history.
        {
            // The SAME basis convention the fullscreen composite ray-casts
            // with (proven by its slab-geometry discards landing exactly
            // right): forward()/right() and up = right x fwd. The first
            // cut extracted rows from the view matrix and produced rays
            // pointing INTO the planet - every under-deck sky pixel died
            // on the march's ground-occlusion gate (the magenta-sentinel
            // forensics, 2026-08-23).
            // ROLL-AWARE basis (v0.1243, origin audit #19): forward()/right()
            // ignore flight ROLL and camera-mode transition interpolation,
            // both of which the rendered frame's view_matrix() applies
            // (rolled_up). Every cloud ray built from the unrolled basis was
            // rotated about the view axis relative to the scene it registers
            // against - a misregistration radiating from the view centre
            // whenever the camera rolls (the fly band the operator lives in).
            // The view matrix's rotation rows ARE the camera axes: row0 =
            // right, row1 = up, row2 = -forward. Identical to the old basis
            // at zero roll outside transitions.
            let vm = camera.view_matrix();
            let fwd = -glam::Vec3::new(vm.row(2).x, vm.row(2).y, vm.row(2).z);
            let right = glam::Vec3::new(vm.row(0).x, vm.row(0).y, vm.row(0).z);
            let up = glam::Vec3::new(vm.row(1).x, vm.row(1).y, vm.row(1).z);
            let cur: [[f32; 3]; 3] = [
                [fwd.x, fwd.y, fwd.z],
                [right.x, right.y, right.z],
                [up.x, up.y, up.z],
            ];
            let prev = self.cloud_prev_basis.replace(Some(cur)).unwrap_or(cur);
            let tanf = (camera.fov_degrees.to_radians() * 0.5).tan();
            let aspect = self.config.width.max(1) as f32 / self.config.height.max(1) as f32;
            // CURRENT basis in light0/1/2 (offsets 80/96/112, also unused):
            // the screen pass builds its pixel rays analytically from
            // these instead of shell mesh fragments (chord-sag hazard).
            let p0: [f32; 4] = [cur[0][0], cur[0][1], cur[0][2], 0.0];
            let p1: [f32; 4] = [cur[1][0], cur[1][1], cur[1][2], 0.0];
            let p2: [f32; 4] = [cur[2][0], cur[2][1], cur[2][2], 0.0];
            self.queue
                .write_buffer(&self.camera_buffer, 80, bytemuck::cast_slice(&p0));
            self.queue
                .write_buffer(&self.camera_buffer, 96, bytemuck::cast_slice(&p1));
            self.queue
                .write_buffer(&self.camera_buffer, 112, bytemuck::cast_slice(&p2));
            let p5: [f32; 4] = [prev[0][0], prev[0][1], prev[0][2], tanf];
            let p6: [f32; 4] = [prev[1][0], prev[1][1], prev[1][2], aspect];
            // light7.w = frame counter for the march's subpixel-jitter
            // sequence (12e). Wrapped so the f32 stays exact.
            let fidx = (self.cloud_octa_phase.get() % 2048) as f32;
            let p7: [f32; 4] = [prev[2][0], prev[2][1], prev[2][2], fidx];
            self.queue
                .write_buffer(&self.camera_buffer, 160, bytemuck::cast_slice(&p5));
            self.queue
                .write_buffer(&self.camera_buffer, 176, bytemuck::cast_slice(&p6));
            self.queue
                .write_buffer(&self.camera_buffer, 192, bytemuck::cast_slice(&p7));
            // Stash the resolve pass's camera state (12e): the SAME motion
            // delta the octa pads carry + the prev basis, consumed by
            // run_cloud_screen_passes this frame. Snap on the teleport
            // sentinel (w = 9: no history is valid).
            self.cloud_resolve_frame.set(cloud_resolve::CloudResolveFrame {
                prev_dpos: [delta_pads[0], delta_pads[1], delta_pads[2]],
                prev_basis: prev,
                snap: resolve_teleport,
            });
        }
        // Near-regime arming mix in light7_color.x (offset 320; the whole
        // light*_color block is legacy-unread, v0.1245): the octa pass
        // gates its full-rate cadence on the map actually being the
        // visible renderer - inside/under the deck the near arm owns the
        // sky and full-rate marching the occluded map was most of the
        // in-layer frame cost.
        self.queue
            .write_buffer(&self.camera_buffer, 320, bytemuck::bytes_of(&self.cloud_near_mix));
        // Underwater extinction in light5_cone_inner.y (offset 548), v0.1054.
        self.queue
            .write_buffer(&self.camera_buffer, 548, bytemuck::bytes_of(&self.underwater_ext));
        // Sea sphere in light6_cone_inner.xyzw (offset 560), v0.1061.
        self.queue
            .write_buffer(&self.camera_buffer, 560, bytemuck::cast_slice(&self.sea_sphere));
        // Foliage wind in light7_cone_inner.xyzw (offset 576), v0.1080:
        // xyz = world wind direction (unit), w = speed m/s. Must land AFTER
        // the celestial_uniforms() stamp (same trap as the fill light above).
        self.queue
            .write_buffer(&self.camera_buffer, 576, bytemuck::cast_slice(&self.foliage_wind));
        // Fill DIRECTION, COLOUR and intensity (v0.998 intensity, v0.1052 the
        // rest). This pass stamps camera.celestial_uniforms() over the whole
        // buffer first, which carries the DEFAULT fill - so the fill that
        // lib.rs sets each frame never reached anything the celestial pass
        // draws, and planet terrain is drawn in this pass. That is the same
        // shape of bug as the hardcoded sun below: v0.1052 aims the fill at the
        // real Moon after sunset to give night a key light, and until now that
        // aim and colour were silently discarded here while a literal 0.6 was
        // used for the strength. Re-poke all of it.
        //
        // fill_direction xyz+w at offset 640, fill_color rgb at 656.
        let (fdir, fcol, fint) = self.cur_fill;
        let fill_dw = [fdir[0], fdir[1], fdir[2], fint * self.fill_scale.clamp(0.0, 1.0)];
        self.queue
            .write_buffer(&self.camera_buffer, 640, bytemuck::cast_slice(&fill_dw));
        self.queue
            .write_buffer(&self.camera_buffer, 656, bytemuck::cast_slice(&fcol));
        // Aerial perspective params (v0.916) in the unused per-light cone
        // pads: [1].y sigma (484), [1].z slant cap (488), [2].yzw sky color
        // (500), [3].yzw camera radial up (516). The interior passes'
        // full uniform write zeroes these, so rooms never fog.
        self.queue
            .write_buffer(&self.camera_buffer, 484, bytemuck::bytes_of(&self.aerial_sigma));
        self.queue
            .write_buffer(&self.camera_buffer, 488, bytemuck::bytes_of(&self.aerial_slant_cap));
        // W1: the water's sky-mirror altitude gate rides the last free pad
        // of light1_cone_inner (.w, offset 492 - beside its aerial
        // siblings; verified unread before this).
        self.queue
            .write_buffer(&self.camera_buffer, 492, bytemuck::bytes_of(&self.water_lut_gate));
        self.queue
            .write_buffer(&self.camera_buffer, 500, bytemuck::cast_slice(&self.aerial_sky));
        self.queue
            .write_buffer(&self.camera_buffer, 516, bytemuck::cast_slice(&self.aerial_up));
        // Light the bodies by the REAL Sun (v0.451): the full-uniform write above
        // stamps the default fake sun [0.3,1,0.5] at offset 608 (v0.639: shifted from 352 by
        // the +256-byte light_spot/light_cone_inner insertion), so re-poke it with the true
        // Earth->Sun direction. Now the planets' lit hemisphere faces the visible Sun disc
        // instead of a fixed up-and-right fake light. (The Sun body itself is emissive, so its
        // own shading is unaffected.)
        if sun_dir != Vec3::ZERO {
            // Intensity scaled by the camera-local day factor (BUG-057 #1):
            // this used to be a bare 2.5 day and night, so everything in the
            // celestial pass without its own terminator gate (trees, props -
            // all types except terrain's 12) was sunlit at midnight. The
            // shaders read w as 2.5 * day and normalize with * 0.4 where they
            // need the plain day factor.
            let sd = [sun_dir.x, sun_dir.y, sun_dir.z, 2.5_f32 * self.celestial_sun_day];
            // w carries the cloud clock (written above at 636); this full
            // vec4 write would stomp it back to a constant otherwise.
            // RGB is the TRANSMITTANCE-TINTED sun (cur_sun.1, fed by
            // lib.rs's atmosphere::sun_transmittance since v0.915): a
            // hardcoded [1.0, 0.97, 0.92] sat here from the v0.639 poke
            // onward, so sunset DIMMED the celestial pass (clouds,
            // terrain, water) but never reddened it - the same
            // stale-literal bug shape as the fake sun direction (v0.451)
            // and the fill light (v0.1052). Golden hour now reaches
            // everything this pass draws.
            let scol = self.cur_sun.1;
            let sc = [scol[0], scol[1], scol[2], time_s];
            self.queue.write_buffer(&self.camera_buffer, 608, bytemuck::cast_slice(&sd));
            self.queue.write_buffer(&self.camera_buffer, 624, bytemuck::cast_slice(&sc));
        }

        // ── Sun shadow pass (v0.899) ── near-field ortho depth from the sun,
        // rendered before the main pass so every lit fragment this frame can
        // sample it. Texel-snapped so a drifting camera never swims the map.
        let shadow_on = self.sun_shadows && sun_dir != Vec3::ZERO;
        {
            const SHADOW_MAP_SIZE: f32 = SUN_SHADOW_MAP_SIZE;
            let extent = SUN_SHADOW_EXTENT_M;
            let sun = sun_dir.normalize();
            let center = camera.effective_position();
            let up = if sun.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
            let view_m = Mat4::look_at_rh(center + sun * 4000.0, center, up);
            // DEPTH RANGE, tightened v0.1058. The light sits at center + sun*4000
            // and everything that can cast into a +/-1500 m box lies within
            // about 1500 m of that centre plane, but the projection mapped
            // 0.1..8000 m onto 0..1 of depth - so 3.2x of the precision was
            // spent on empty space in front of and behind the scene.
            //
            // That matters because the shader's shadow bias is expressed in NDC
            // (0.0006 flat, 0.0025 at grazing), so its WORLD size is the bias
            // times the depth range: 4.8 m flat and 20 m grazing. A conifer is
            // 20-30 m tall, so most of a tree's shadow - and all of a trunk's -
            // fell inside the bias and was erased. Same for any modest terrain
            // relief and for wave crests, which are 10 m at most.
            //
            // Fitting the range to the box takes it to 2400..5600 m = 3200 m,
            // so the same NDC bias is 1.9 m flat and 8 m grazing: 2.5x tighter
            // shadows everywhere, for one line and no perf cost. A margin of
            // 1.4x extent covers casters standing above the box (a 30 m tree on
            // a ridge) and the texel snap.
            let z_margin = extent * 1.4;
            let z_near = (4000.0 - extent - z_margin).max(1.0);
            let z_far = 4000.0 + extent + z_margin;
            let proj = Mat4::orthographic_rh(-extent, extent, -extent, extent, z_near, z_far);
            let mut vp = proj * view_m;
            // Texel snap: shift so the world origin lands on a texel grid.
            let ndc_texel = 2.0 / SHADOW_MAP_SIZE;
            let origin = vp * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
            let snap = |v: f32| (v / ndc_texel).round() * ndc_texel - v;
            vp = Mat4::from_translation(Vec3::new(snap(origin.x), snap(origin.y), 0.0)) * vp;
            // v0.1057: build the light camera from the REAL celestial uniforms
            // and overwrite only view_proj, instead of starting from zeroed.
            // Same class of bug as the hardcoded sun and the discarded fill
            // above: a zeroed uniform means the type-16 water vertex branch sees
            // FFT-mode 0, both cascade anchors 0, weld K 0, the wave clock 0 and
            // view_pos 0 - so if water ever casts into this map it rasterises a
            // DIFFERENT sea than the colour pass draws, and the shadows land on
            // the wrong water. Everything the water VS reads has to be re-poked
            // at the same offsets the colour pass uses on camera_buffer. If a
            // refactor re-zeroes this, wave shadows silently go wrong rather
            // than absent, which is much harder to spot.
            let mut light_u = camera.celestial_uniforms();
            light_u.view_proj = vp.to_cols_array_2d();
            self.queue
                .write_buffer(&self.light_camera_buffer, 0, bytemuck::bytes_of(&light_u));
            // 464 = ground_anchor (FFT flag + the 64 m cascade-A anchor).
            self.queue.write_buffer(
                &self.light_camera_buffer,
                464,
                bytemuck::cast_slice(&ground_anchor),
            );
            // 528 = cascade-B anchor + the water weld K.
            self.queue.write_buffer(
                &self.light_camera_buffer,
                528,
                bytemuck::cast_slice(&ocean_anchor256),
            );
            // 544 = live sea crest, which the VS shoal fade reads.
            self.queue.write_buffer(
                &self.light_camera_buffer,
                544,
                bytemuck::bytes_of(&self.sea_crest_m),
            );
            // 636 = the wave clock, parked in sun_color.w.
            self.queue.write_buffer(
                &self.light_camera_buffer,
                636,
                bytemuck::bytes_of(&time_s),
            );
            // 576 = foliage wind (v0.1080). The shadow pass runs the SAME
            // vs_main with the SAME type-20 wind branch off THIS buffer; omit
            // this and the shadow map records a near-upright fallback-wind
            // tree while the colour pass draws one leaning metres downwind
            // (up to ~10 texels of detachment at storm speeds).
            self.queue.write_buffer(
                &self.light_camera_buffer,
                576,
                bytemuck::cast_slice(&self.foliage_wind),
            );
            let mut su = [0.0_f32; 24];
            su[..16].copy_from_slice(&vp.to_cols_array());
            su[16] = if shadow_on { 1.0 } else { 0.0 };
            su[17] = self.shadow_strength.clamp(0.0, 1.0);
            su[18] = 1.0 / SHADOW_MAP_SIZE;
            // params.w (v0.912): the tree-model radius - terrain tree CARDS
            // hide inside it so the real 3D conifers replace them cleanly.
            su[19] = self.tree_card_hide_m;
            // params2.x (v0.924): tree-card far cutoff (vegetation LOD slider).
            su[20] = self.tree_card_far_m.max(1.0);
            // params2.y = sky-view LUT valid this frame (stage 3c gate): the
            // table only re-renders near an atmosphere body; elsewhere it is
            // stale and the megashader must not blend it in.
            su[21] = if self.sky_view_uniform.is_some() { 1.0 } else { 0.0 };
            // params2.zw = light-tile pixel sizes (clustering L1b); zero = the
            // classic full light loop.
            su[22] = self.tile_px.0;
            su[23] = self.tile_px.1;
            self.queue
                .write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::cast_slice(&su));
        }
        // Batched patch instances (draw-batching increment 1): uploaded ONCE
        // ahead of both encoders -- the shadow pass and the celestial pass
        // index the same storage buffer, so a culled shadow subset still
        // addresses its instances by their full-list position.
        let patch_batch_n = if let Some(arena) = self.patch_arena.as_ref() {
            let n = self.patch_draws.len().min(patch_arena::MAX_PATCH_DRAWS);
            if n > 0 {
                let inst: Vec<patch_arena::PatchInstance> = self.patch_draws[..n]
                    .iter()
                    .map(|d| {
                        patch_arena::PatchInstance::new([
                            d.position.x,
                            d.position.y,
                            d.position.z,
                            d.fade,
                        ])
                    })
                    .collect();
                self.queue
                    .write_buffer(&arena.instance_buf, 0, bytemuck::cast_slice(&inst));
                self.queue.write_buffer(
                    &arena.batch_buf,
                    0,
                    bytemuck::bytes_of(&self.patch_batch_rot.to_cols_array_2d()),
                );
                // Increment 2: indirect args for the one-submit path. Each
                // entry's first_instance selects the instance-attribute
                // element (honored in hardware; the builtin would not be).
                if self.patch_indirect {
                    let args: Vec<patch_arena::IndirectArgs> = self.patch_draws[..n]
                        .iter()
                        .enumerate()
                        .map(|(i, d)| patch_arena::IndirectArgs {
                            index_count: d.slot.icount,
                            instance_count: 1,
                            first_index: d.slot.istart,
                            base_vertex: d.slot.vstart as i32,
                            first_instance: i as u32,
                        })
                        .collect();
                    self.queue
                        .write_buffer(&arena.indirect_buf, 0, bytemuck::cast_slice(&args));
                }
            }
            n
        } else {
            0
        };
        if shadow_on {
            // Object uniforms uploaded HERE cover both the shadow pass and
            // the main pass below (same list, same offsets).
            self.upload_object_uniforms(objects.iter().chain(transparent.iter()));
            let mut senc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Shadow Encoder"),
                });
            {
                let mut pass = senc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Sun Shadow Pass"),
                    timestamp_writes: self.pass_timer("gpu.shadow"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow_map_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });
                pass.set_pipeline(&self.pipeline.shadow_pipeline);
                // Slot 1: zero per-instance data for classic draws (increment 2).
                pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                pass.set_bind_group(0, &self.light_camera_bind_group, &[]);
                // Group 3 for casters with NO texture of their own: the
                // dummy-depth variant (the real shadow map is this pass's
                // write target; wgpu rejects sampling it here). fs_shadow
                // reads binding 14, the tree atlas, from it.
                //
                // v0.1108: a TEXTURED caster now rebinds its own shadow-safe
                // group below. Until it did, fs_shadow's type-19 and type-21
                // branches sampled the 1x1 WHITE fallback at binding 0 - alpha
                // 1 everywhere, so neither discard could ever fire and every
                // near-tree cluster card still stamped a solid quad into the
                // sun map. The branches existed; the texture did not.
                pass.set_bind_group(3, &self.shadow_pass_texture_bind_group, &[]);
                let uniform_align = 256_u64;
                let mut bound_material = usize::MAX;
                // Near-field caster cull (v0.899; tightened v0.911, perf
                // audit #2): the ortho box covers 1.5 km around the camera,
                // so a caster can only matter if its anchor sits within the
                // box plus the largest patch's own reach. 6 km covers the
                // coarsest horizon patch that could still poke a triangle
                // into the box; the old 65 km bound re-rasterized thousands
                // of far patches into the 4096 map every frame for nothing.
                let cast_center = camera.effective_position();
                for (i, obj) in objects.iter().enumerate() {
                    if i >= MAX_OBJECTS {
                        break;
                    }
                    if (obj.position - cast_center).length_squared() > 6_000.0_f32 * 6_000.0 {
                        continue;
                    }
                    let mesh = match self.meshes.get(obj.mesh) {
                        Some(m) => m,
                        None => continue,
                    };
                    let material = match self.materials.get(obj.material) {
                        Some(m) => m,
                        None => continue,
                    };
                    // v0.1106 (why + cost: Pipeline::shadow_for): a crossfading LOD
                    // dithers and a cutout caster may alpha-discard, so those two
                    // take fs_shadow; the rest keep the depth-only fast path.
                    //
                    // v0.1108 narrowed the second half from "has an albedo
                    // texture" to "fs_shadow actually has a discard for this
                    // material TYPE". The old test was wrong in both
                    // directions: baked bark (22) and textured planet meshes
                    // are opaque and paid a fragment stage that can never
                    // discard, while an untextured terrain-patch material (12)
                    // took the depth-only path even though its sprite tree
                    // cards discard on the atlas alpha.
                    pass.set_pipeline(
                        self.pipeline
                            .shadow_for(obj.fade != 0.0 || material.casts_cutout_shadow()),
                    );
                    let dynamic_offset = (uniform_align as u32) * (i as u32);
                    pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                    if bound_material != obj.material {
                        bound_material = obj.material;
                        pass.set_bind_group(2, &material.bind_group, &[]);
                        // Group 3 follows the material, so it rides the same
                        // change check: the material's SHADOW-SAFE albedo when
                        // it has one, the pass-wide fallback otherwise. This is
                        // the bind that makes the type-19 / type-21 discards in
                        // fs_shadow read real texels for the first time.
                        pass.set_bind_group(
                            3,
                            material
                                .shadow_albedo_group()
                                .unwrap_or(&self.shadow_pass_texture_bind_group),
                            &[],
                        );
                    }
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
                // WATER CASTERS (v0.1057). The wave shell lives in the
                // TRANSPARENT list, which this pass never walked - so a 10 m
                // crest cast nothing and the trough behind it stayed fully lit.
                // That absent self-shadowing is a large part of why a storm sea
                // read as a flat pattern rather than relief. Object uniforms for
                // the transparent list were already uploaded by the chain above,
                // at slot objects.len() + i, so the dynamic offset is the only
                // thing that changes. Group 3 for this pass binds the REAL FFT
                // tile at binding 15, so the vertex displacement matches the
                // colour pass exactly (and v0.1057 also stopped this pass from
                // zeroing the anchors it needs to do that).
                //
                // Tighter cull than the 6 km above: the ortho box is 1.5 km
                // across at 0.73 m/texel, and only crests inside it land at a
                // useful resolution.
                if !self.water_caster_mats.is_empty() {
                    pass.set_pipeline(&self.pipeline.shadow_pipeline);
                    pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                    for (i, obj) in transparent.iter().enumerate() {
                        let slot = objects.len() + i;
                        if slot >= MAX_OBJECTS {
                            break;
                        }
                        if !self.water_caster_mats.contains(&obj.material) {
                            continue;
                        }
                        if (obj.position - cast_center).length_squared()
                            > 2_500.0_f32 * 2_500.0
                        {
                            continue;
                        }
                        let mesh = match self.meshes.get(obj.mesh) {
                            Some(m) => m,
                            None => continue,
                        };
                        let material = match self.materials.get(obj.material) {
                            Some(m) => m,
                            None => continue,
                        };
                        let dynamic_offset = (uniform_align as u32) * (slot as u32);
                        pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                        if bound_material != obj.material {
                            bound_material = obj.material;
                            pass.set_bind_group(2, &material.bind_group, &[]);
                            // Group 3 explicitly (v0.1108): the classic loop
                            // above may have left a cluster card's group bound,
                            // and water must read binding 15 - the FFT tile -
                            // from a group it chose, not one it inherited.
                            pass.set_bind_group(
                                3,
                                material
                                    .shadow_albedo_group()
                                    .unwrap_or(&self.shadow_pass_texture_bind_group),
                                &[],
                            );
                        }
                        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    }
                }
                // Batched patch casters: same 6 km cull, one bind, one
                // draw per near patch. Instance index = full-list position
                // (the storage buffer holds ALL of this frame's draws).
                if patch_batch_n > 0 {
                    let arena = self.patch_arena.as_ref().expect("patch_batch_n > 0");
                    pass.set_pipeline(&self.pipeline.patch_shadow_pipeline);
                    pass.set_bind_group(1, &arena.bind_group, &[]);
                    // Groups 2 and 3 explicitly: the classic caster loop above
                    // may not have bound any material (zero classic casters),
                    // and if it did, the group 3 left bound belongs to whatever
                    // drew last. The patch PSO always runs fs_shadow, so its
                    // group 3 matters - binding 0 for a textured planet's
                    // type-12 cutout, binding 14 for the sprite tree cards.
                    if let Some(material) = self.materials.get(self.patch_batch_material) {
                        pass.set_bind_group(2, &material.bind_group, &[]);
                        pass.set_bind_group(
                            3,
                            material
                                .shadow_albedo_group()
                                .unwrap_or(&self.shadow_pass_texture_bind_group),
                            &[],
                        );
                    }
                    pass.set_vertex_buffer(0, arena.vertex_buf.slice(..));
                    pass.set_vertex_buffer(1, arena.instance_buf.slice(..));
                    pass.set_index_buffer(arena.index_buf.slice(..), wgpu::IndexFormat::Uint32);
                    // Per-draw loop stays here even with indirect support:
                    // the 6 km caster cull keeps this to a few dozen draws,
                    // and a separate culled args buffer isn't worth it.
                    for (i, d) in self.patch_draws[..patch_batch_n].iter().enumerate() {
                        if (d.position - cast_center).length_squared() > 6_000.0_f32 * 6_000.0 {
                            continue;
                        }
                        pass.draw_indexed(
                            d.slot.istart..d.slot.istart + d.slot.icount,
                            d.slot.vstart as i32,
                            (i as u32)..(i as u32 + 1),
                        );
                    }
                }
            }
            self.queue.submit(std::iter::once(senc.finish()));
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Celestial Encoder"),
            });

        // Sky-view LUT (stage 3b-2): refresh the distant-sky table first so
        // everything later in the frame could sample this frame's sky. Only
        // runs frame-locked near an atmosphere body (the uniform is stashed
        // by the lib.rs atmosphere hook; None elsewhere = zero cost).
        if let Some(u) = self.sky_view_uniform {
            self.sky_view.encode(&self.queue, &mut encoder, &u);
        }

        // ── Temporal cloud octa pass (clouds phase 4) ── re-march + EMA the
        // direction-indexed cloud map BEFORE the main pass so this frame's
        // composite samples this frame's accumulation. The object uniforms
        // were staged above (shells continue the index range after the
        // opaque list); the pass binds the cloud SHELL's slot so obj_model()
        // gives the march its planet frame, and the group-3 with the
        // ping-pong PARTNER in the albedo slot supplies the history.
        // FAR side (12d/12g): runs whenever the crossfade still gives the
        // octa map any weight (mix < 1) - fully near, the half-res screen
        // pass replaces it entirely (marching 16.7M map texels for a
        // full-screen planet was the near-planet lag, and the direction
        // cache is the ghost family).
        // v0.1246: ALSO dispatch when under the deck (cloud_octa_force,
        // 12c regime 3) - since the v0.1244 per-pixel split the composite
        // gives the map full weight in the whole horizon band even at
        // near_mix 1.0, and a skipped pass froze that band at whatever
        // daylight it last held (the operator's bright-white night band and
        // the wall of static). Inside the slab (regime 2) the skip stays:
        // near ownership genuinely covers the sky there.
        let octa_runs = self.cloud_near_mix < 1.0 || self.cloud_octa_force;
        if octa_runs {
            let idle = self.cloud_octa_idle.replace(0);
            if idle >= 30 {
                // Resume after a real freeze: the whole map is stale (wrong
                // sun, wrong weather). A fade-in would REPLAY the stale
                // content for seconds - boost the EMA floor to ~1 instead
                // and decay over a few dispatched frames.
                self.cloud_octa_boost.set(1.0);
            }
        } else {
            self.cloud_octa_idle.set(self.cloud_octa_idle.get().saturating_add(1));
        }
        let boost = self.cloud_octa_boost.get();
        if boost > 0.0 {
            self.cloud_octa_boost.set((boost - 0.18).max(0.0));
        }
        // light7_color.y (offset 324; legacy-unread block): the octa pass
        // applies this as an alpha floor for marching texels.
        self.queue
            .write_buffer(&self.camera_buffer, 324, bytemuck::bytes_of(&boost));
        if let (Some(ct), Some(mat_idx), true) = (
            self.cloud_temporal.as_ref(),
            self.cloud_temporal_mat,
            octa_runs,
        ) {
            if let (Some(i), Some(material)) = (
                transparent.iter().position(|o| o.material == mat_idx),
                self.materials.get(mat_idx),
            ) {
                let slot = objects.len() + i;
                if slot < MAX_OBJECTS {
                    // The main pass's shared upload runs later; stage the
                    // object uniforms now so the octa pass sees them.
                    self.upload_object_uniforms(objects.iter().chain(transparent.iter()));
                    let read = ct.cur.get();
                    let write = 1 - read;
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Cloud Octa Temporal Pass"),
                        timestamp_writes: self.pass_timer("gpu.cloud_octa"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &ct.views[write],
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        ..Default::default()
                    });
                    pass.set_pipeline(&self.pipeline.cloud_octa_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    let uniform_align = 256_u64;
                    pass.set_bind_group(
                        1,
                        &self.object_bind_group,
                        &[(uniform_align as u32) * (slot as u32)],
                    );
                    pass.set_bind_group(2, &material.bind_group, &[]);
                    pass.set_bind_group(3, &ct.groups[read].colour, &[]);
                    pass.draw(0..3, 0..1);
                    drop(pass);
                    ct.cur.set(write);
                }
            }
        }

        // ── 12e NEAR march + resolve ── two passes replace 12d's single
        // cadence+history hybrid (whose one blend constant could not both
        // converge the jittered march AND kill stale history - the
        // operator's "static" + residual ghosting on the first flight):
        //  1. MARCH: every pixel of the quarter-res pair, every frame,
        //     subpixel-jittered analytic rays (no cadence, no history) -
        //     MRT premultiplied result + first-hit distance in km.
        //  2. RESOLVE: deep accumulation into the half-res ping-pong with
        //     VARIANCE-CLIPPED reprojected history - ghosts snap to the
        //     current neighbourhood in one frame while corroborated
        //     content converges ~8 frames deep.
        // The composite then samples the accumulation at each fragment's
        // own screen uv, unchanged. No direction cache exists anywhere in
        // this regime and there is no arming altitude.
        // DIAG4 (12d bring-up): once-per-second trace of the near-regime
        // chain - drop after the under-deck vanish is verified fixed.
        {
            static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let now_s = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now_s != LAST.swap(now_s, std::sync::atomic::Ordering::Relaxed) {
                log::info!(
                    "[CloudScreen] near={} cs={} mat={:?} shell_found={:?}",
                    self.cloud_mode_near,
                    self.cloud_screen.is_some(),
                    self.cloud_temporal_mat,
                    self.cloud_temporal_mat.and_then(|m| {
                        transparent.iter().position(|o| o.material == m)
                    }),
                );
            }
        }
        if self.cloud_mode_near {
            if let (Some(cs), Some(mat_idx)) =
                (self.cloud_screen.as_ref(), self.cloud_temporal_mat)
            {
                if let (Some(i), Some(material)) = (
                    transparent.iter().position(|o| o.material == mat_idx),
                    self.materials.get(mat_idx),
                ) {
                    let slot = objects.len() + i;
                    if slot < MAX_OBJECTS {
                        self.upload_object_uniforms(
                            objects.iter().chain(transparent.iter()),
                        );
                        let mut pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Cloud March Pass"),
                                timestamp_writes: self.pass_timer("gpu.cloud_screen"),
                                color_attachments: &[
                                    Some(wgpu::RenderPassColorAttachment {
                                        view: &cs.march_view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(
                                                wgpu::Color::TRANSPARENT,
                                            ),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    }),
                                    Some(wgpu::RenderPassColorAttachment {
                                        view: &cs.dist_view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(
                                                wgpu::Color::TRANSPARENT,
                                            ),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    }),
                                ],
                                depth_stencil_attachment: None,
                                ..Default::default()
                            });
                        pass.set_pipeline(&self.pipeline.cloud_screen_pipeline);
                        pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        let uniform_align = 256_u64;
                        pass.set_bind_group(
                            1,
                            &self.object_bind_group,
                            &[(uniform_align as u32) * (slot as u32)],
                        );
                        pass.set_bind_group(2, &material.bind_group, &[]);
                        // Group 3 unused by the march (no history read) -
                        // the shared layout still requires a binding.
                        pass.set_bind_group(3, &self.default_texture_bind_group, &[]);
                        pass.draw(0..3, 0..1);
                        drop(pass);

                        let read = cs.cur.get();
                        let write = 1 - read;
                        let mut frame = self.cloud_resolve_frame.get();
                        // Regime entry / buffer recreation: the history is
                        // zeroed - drop it outright instead of fading the
                        // deck in from black over ~1/alpha frames.
                        if cs.fresh.replace(false) {
                            frame.snap = true;
                        }
                        // Roll-aware basis (v0.1243, audit #19) - same
                        // extraction as the march pads above; the three
                        // consumers must agree or the resolve reprojects
                        // against a twisted frame.
                        let vm = camera.view_matrix();
                        let fwd = -glam::Vec3::new(vm.row(2).x, vm.row(2).y, vm.row(2).z);
                        let right = glam::Vec3::new(vm.row(0).x, vm.row(0).y, vm.row(0).z);
                        let up = glam::Vec3::new(vm.row(1).x, vm.row(1).y, vm.row(1).z);
                        let eye = camera.effective_position();
                        self.cloud_resolve.render(
                            &self.device,
                            &self.queue,
                            &mut encoder,
                            &cs.march_view,
                            &cs.dist_view,
                            &cs.views[read],
                            &cs.views[write],
                            &frame,
                            [eye.x, eye.y, eye.z],
                            [fwd.x, fwd.y, fwd.z],
                            [right.x, right.y, right.z],
                            [up.x, up.y, up.z],
                            (camera.fov_degrees.to_radians() * 0.5).tan(),
                            camera.aspect,
                            self.pass_timer("gpu.cloud_resolve"),
                        );
                        cs.cur.set(write);
                    }
                }
            }
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Celestial Pass"),
                timestamp_writes: self.pass_timer("gpu.celestial"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve the star background
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // reverse-Z: clear to 0 (farthest)
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline.render_pipeline);
            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // Opaque bodies + transparent shells (atmospheres) share one
            // object-uniform buffer: shells continue the index range after the
            // opaque list. Both lists together must stay under MAX_OBJECTS
            // (a couple dozen sky bodies in practice).
            // One batched object-uniform upload (v0.891): opaque bodies +
            // transparent shells share the buffer, shells continue the range.
            // KEEP THIS UNCONDITIONAL. The v0.911 perf audit suggested
            // skipping this upload when the shadow pass already staged the
            // identical bytes at 2072 - probe-bisected result: with the
            // skip, the atmosphere DOME vanished at ground level (black
            // starfield at noon, only the horizon limb left) on DX12. The
            // two writes are byte-identical in source, so the failure is a
            // queue-write/submission-ordering subtlety, not logic; the
            // ~1-2 ms is not worth a broken sky. Do not re-attempt without
            // a boot+ground-level-sky probe check.
            let uniform_align = 256_u64;
            self.upload_object_uniforms(objects.iter().chain(transparent.iter()));

            let mut bound_material = usize::MAX;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): terrain patches share one material, so 3000+
                // redundant rebinds per frame collapse to one.
                if bound_material != obj.material {
                    bound_material = obj.material;
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }

            // ── Batched terrain patches (draw-batching increment 1) ──
            // The 12k-patch working set that used to be 12k RenderObjects
            // (each with a dynamic-offset bind + two buffer binds) is now:
            // bind everything ONCE, then one draw_indexed per patch with
            // instance range i..i+1 -- the instance index routes the shader
            // to that patch's translation + fade in the storage array.
            // Opaque + depth-written, so ordering against the classic
            // opaque loop above is irrelevant; runs BEFORE the transparent
            // shells below, which is the order transparency needs.
            if patch_batch_n > 0 {
                let arena = self.patch_arena.as_ref().expect("patch_batch_n > 0");
                render_pass.set_pipeline(&self.pipeline.patch_render_pipeline);
                render_pass.set_bind_group(1, &arena.bind_group, &[]);
                if let Some(material) = self.materials.get(self.patch_batch_material) {
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, arena.vertex_buf.slice(..));
                render_pass.set_vertex_buffer(1, arena.instance_buf.slice(..));
                render_pass
                    .set_index_buffer(arena.index_buf.slice(..), wgpu::IndexFormat::Uint32);
                if self.patch_indirect {
                    // Increment 2: the whole batch in ONE command. This is
                    // what removes the ~1.5 us x N draw-encoding cost that
                    // still dominated after increment 1.
                    render_pass.multi_draw_indexed_indirect(
                        &arena.indirect_buf,
                        0,
                        patch_batch_n as u32,
                    );
                } else {
                    for (i, d) in self.patch_draws[..patch_batch_n].iter().enumerate() {
                        render_pass.draw_indexed(
                            d.slot.istart..d.slot.istart + d.slot.icount,
                            d.slot.vstart as i32,
                            (i as u32)..(i as u32 + 1),
                        );
                    }
                }
            }

            // ── Near-field grass strands (v0.1091) ──
            // ONE draw for the whole sward: the shared unit-height tiller
            // mesh, instanced once per visible tiller. The instance record
            // (mesh::GrassInstance) carries the entire transform, so this
            // needs no per-tiller object uniform and no per-tiller bind.
            //
            // AFTER the terrain batch and on the OPAQUE pipeline, both
            // deliberate: opaque + depth-write means blades resolve against
            // each other and against the ground by depth, in any order, and
            // drawing after the ground means most buried fragments are
            // already z-rejected.
            if self.grass_n > 0 {
                if let (Some(gm), Some(gbuf)) =
                    (self.grass_mesh.as_ref(), self.grass_instance_buf.as_ref())
                {
                    if let Some(material) = self.materials.get(self.grass_material) {
                        render_pass.set_pipeline(&self.pipeline.render_pipeline);
                        // Object uniform slot 0 is the identity model matrix
                        // staged by upload_object_uniforms for every frame
                        // that draws anything; grass ignores it (the vertex
                        // stage builds its own matrix from the instance) but
                        // the shared pipeline layout requires group 1 bound.
                        render_pass.set_bind_group(1, &self.object_bind_group, &[0]);
                        render_pass.set_bind_group(2, &material.bind_group, &[]);
                        render_pass.set_bind_group(3, &self.default_texture_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, gm.vertex_buffer.slice(..));
                        render_pass.set_vertex_buffer(1, gbuf.slice(..));
                        render_pass.set_index_buffer(
                            gm.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.draw_indexed(0..gm.index_count, 0, 0..self.grass_n);
                    }
                }
            }

        }

        // 12c ORDER FIX (adversarial review finding 3): when the camera is
        // OUTSIDE the atmosphere the deck must sit UNDER the limb haze -
        // the v0.997 rule the shell path expressed by transparent-list
        // order (clouds first, dome after). The fullscreen composite
        // therefore runs HERE - after the opaque pass wrote terrain
        // depth, before the transparent pass blends the atmosphere dome
        // over it. Inside the atmosphere the dome is the sky BEHIND the
        // deck, so the composite stays after the transparent pass below.
        if self
            .cloud_composite_frame
            .as_ref()
            .map(|f| f.atmo_over)
            .unwrap_or(false)
        {
            self.run_cloud_composite(&mut encoder, view, camera);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Celestial Transparent Pass"),
                timestamp_writes: self.pass_timer("gpu.celestial_t"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            let uniform_align = 256_u64;

            // Atmosphere shells etc.: alpha-blended over the bodies, depth-TESTED
            // against them (no depth write), so the back hemisphere of a shell is
            // hidden by its own planet while the limb halo survives. Few and far
            // apart, so no depth sorting needed. (v0.763)
            if !transparent.is_empty() {
                render_pass.set_pipeline(&self.pipeline.transparent_pipeline);
                // Slot 1: zero per-instance data for classic draws (increment 2).
                render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                let mut bound_material = usize::MAX;
                // ── WATER DEPTH WRITE (v0.1060) ──
                // Operator: "I can essentially see waves behind waves very
                // easily, almost transparent like glass. Almost like the wave
                // behind renders in front of the close waves."
                //
                // Exactly right, and it is not a shading problem: the water
                // shell is thousands of separate patches drawn with alpha
                // blending and NO depth write, in whatever order the LOD
                // selection emitted them - which is heap order by screen error,
                // uncorrelated with distance. So a far wave patch submitted
                // later paints straight over a near one. At 10 m storm crests
                // the sea folds over itself on screen constantly, which is why
                // it only became obvious once the waves got big.
                //
                // The fix is depth, not sorting: the sea's alpha is 0.93-1.0
                // almost everywhere, so it is opaque enough that the NEAREST
                // fragment is simply the right answer. overlay_pipeline is
                // already alpha-blend + cull None + depth_write TRUE - the exact
                // state - so this costs no new pipeline compile.
                //
                // This is only safe because v0.1053 moved water to the END of
                // the transparent list whenever the camera is inside an
                // atmosphere: nothing is drawn after the sea, so its depth
                // cannot wrongly occlude the atmosphere or cloud shells. From
                // orbit the flag stays false and the old behaviour is kept.
                let water_dw = self.water_depth_write && !self.water_caster_mats.is_empty();
                let mut on_water_pipe = false;
                for (i, obj) in transparent.iter().enumerate() {
                    let slot = objects.len() + i;
                    if slot >= MAX_OBJECTS { break; }
                    if water_dw {
                        let is_water = self.water_caster_mats.contains(&obj.material);
                        if is_water != on_water_pipe {
                            on_water_pipe = is_water;
                            render_pass.set_pipeline(if is_water {
                                &self.pipeline.overlay_pipeline
                            } else {
                                &self.pipeline.transparent_pipeline
                            });
                            render_pass
                                .set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                            bound_material = usize::MAX;
                        }
                    }
                    let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                    let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                    let dynamic_offset = (uniform_align as u32) * (slot as u32);
                    render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                    // Material bind groups (2 + 3) skipped when unchanged
                    // (v0.891); also drops a duplicate group-3 rebind that a
                    // copy-paste had left here.
                    if bound_material != obj.material {
                        bound_material = obj.material;
                        render_pass.set_bind_group(2, &material.bind_group, &[]);
                        // Group 3 fallback/texture -- same rule as the opaque
                        // loop. Temporal-cloud override (phase 4): the cloud
                        // material's composite samples the freshly written
                        // octa map through the albedo slot.
                        let g3 = if Some(obj.material) == self.cloud_temporal_mat {
                            self.cloud_temporal
                                .as_ref()
                                .map(|ct| &ct.groups[ct.cur.get()].colour)
                        } else {
                            material.albedo_group()
                        };
                        render_pass.set_bind_group(
                            3,
                            g3.unwrap_or(&self.default_texture_bind_group),
                            &[],
                        );
                    }
                    render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
        }

        // ── Fullscreen depth-aware cloud composite (Wave D slice 1b) ──
        // The INSIDE-the-atmosphere position: after the transparent pass,
        // so the deck draws over the sky dome behind it. The outside
        // position ran earlier (before the transparent pass) - see the
        // 12c order-fix comment there. One compositor either way: the
        // shell's own temporal branch discards while the map is armed.
        if self
            .cloud_composite_frame
            .as_ref()
            .map(|f| !f.atmo_over)
            .unwrap_or(false)
        {
            self.run_cloud_composite(&mut encoder, view, camera);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// The fullscreen depth-aware cloud composite draw (Wave D slice 1b).
    /// Requires the scene depth for this frame's opaques to be complete;
    /// call position relative to the transparent celestial pass is chosen
    /// by CloudCompositeFrame::atmo_over (12c order fix). This is what
    /// lets a deck below the camera survive: the shell's fragments for
    /// downward rays lie beyond the planet and the hardware depth test
    /// killed them; here occlusion is per-pixel against the REAL scene
    /// depth, mountains included.
    fn run_cloud_composite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        camera: &Camera,
    ) {
        if let (Some(ct), Some(frame), true) = (
            self.cloud_temporal.as_ref(),
            self.cloud_composite_frame.as_ref(),
            self.cloud_temporal_mat.is_some(),
        ) {
            let proj = Mat4::perspective_rh(
                camera.fov_degrees.to_radians(),
                camera.aspect,
                1.0e13,
                1.0,
            );
            let m = proj.to_cols_array_2d();
            // Roll-aware basis (v0.1243, audit #19) - third of the three
            // agreeing consumers (march pads, resolve, composite).
            let vm = camera.view_matrix();
            let fwd = -glam::Vec3::new(vm.row(2).x, vm.row(2).y, vm.row(2).z);
            let right = glam::Vec3::new(vm.row(0).x, vm.row(0).y, vm.row(0).z);
            let up = glam::Vec3::new(vm.row(1).x, vm.row(1).y, vm.row(1).z);
            let eye = camera.effective_position();
            // 12g: the composite crossfades the octa map with the half-res
            // SCREEN accumulation by cloud_near_mix. When the screen pair
            // does not exist yet (first near frame races
            // ensure_cloud_screen) the octa map stands in at weight 0.
            // v0.1244: the dist view rides along for the per-pixel regime
            // key. When the screen pair does not exist the map view stands
            // in as a dummy (near_mix 0 means the shader never reads it).
            let (screen_view, dist_view, near_mix) =
                match (self.cloud_mode_near, self.cloud_screen.as_ref()) {
                    (true, Some(cs)) => (
                        &cs.views[cs.cur.get()],
                        &cs.dist_view,
                        self.cloud_near_mix,
                    ),
                    _ => (&ct.views[ct.cur.get()], &ct.views[ct.cur.get()], 0.0),
                };
            self.cloud_composite.render(
                &self.device,
                &self.queue,
                encoder,
                &self.depth_view,
                &ct.views[ct.cur.get()],
                view,
                frame,
                screen_view,
                dist_view,
                near_mix,
                [eye.x, eye.y, eye.z],
                [fwd.x, fwd.y, fwd.z],
                [right.x, right.y, right.z],
                [up.x, up.y, up.z],
                (camera.fov_degrees.to_radians() * 0.5).tan(),
                camera.aspect,
                m[2][2],
                m[3][2],
                self.pass_timer("gpu.cloud_composite"),
            );
        }
    }

    /// Draw world-space thin lines (orbit paths) onto an already-rendered
    /// frame. Call AFTER `render_scene_onto` so the depth buffer holds
    /// the planets — the reverse-Z depth-test (no depth-write) then
    /// occludes any segment passing behind a planet. Same camera as the
    /// scene (full view-proj + floating origin), so lines sit exactly on
    /// the bodies. Transient per-frame vertex buffer (a few thousand
    /// verts — trivial).
    pub fn draw_lines_onto(
        &self,
        camera: &Camera,
        verts: &[line::LineVertex],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.lines");
        if verts.len() < 2 {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("World Line VB"),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("World Line Encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("World Line Pass"),
                timestamp_writes: self.pass_timer("gpu.lines"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve stars + scene
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // test against the planets
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_pipeline(&self.line_pipeline);
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_vertex_buffer(0, vbuf.slice(..));
            rp.draw(0..verts.len() as u32, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Draw particle billboards onto an already-rendered frame (v0.966).
    /// Same post-pass shape as draw_lines_onto: transient instance buffer,
    /// reverse-Z depth TEST against the scene, no depth write. Billboard
    /// axes come from the camera basis, uploaded to the frame uniform.
    pub fn draw_particles_onto(
        &mut self,
        camera: &Camera,
        alpha: &[particles::ParticleVertexData],
        additive: &[particles::ParticleVertexData],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.particles");
        if alpha.is_empty() && additive.is_empty() {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );
        let right = camera.right();
        let up = right.cross(camera.forward()).normalize_or_zero() * -1.0;
        // right.w carries the scene illumination scalar (v0.1154): particle
        // billboards have no lighting of their own, and an authored-tint
        // raindrop rendered at full brightness at midnight (the operator's
        // "rain glows at night" report). Daylight = 1, night = the floor.
        let frame: [f32; 8] =
            [right.x, right.y, right.z, self.scene_illum(), up.x, up.y, up.z, 0.0];
        self.queue
            .write_buffer(&self.particle_frame_buffer, 0, bytemuck::cast_slice(&frame));
        // Grow-to-high-water-mark, then refill. Reallocating only when the
        // count exceeds the previous peak means a steady downpour allocates
        // once and then never again.
        let mut ensure = |buf: &mut Option<wgpu::Buffer>,
                          cap: &mut usize,
                          data: &[particles::ParticleVertexData],
                          label: &str| {
            if data.is_empty() {
                return;
            }
            if buf.is_none() || *cap < data.len() {
                // Round up so a slowly-growing storm does not reallocate every
                // frame on the way up.
                let want = (data.len() * 3 / 2).max(4096);
                *buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: (want * std::mem::size_of::<particles::ParticleVertexData>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                *cap = want;
            }
            if let Some(b) = buf.as_ref() {
                self.queue.write_buffer(b, 0, bytemuck::cast_slice(data));
            }
        };
        let mut vb_a = self.particle_vb_alpha.take();
        let mut vb_b = self.particle_vb_additive.take();
        let mut cap_a = self.particle_vb_alpha_cap;
        let mut cap_b = self.particle_vb_additive_cap;
        ensure(&mut vb_a, &mut cap_a, alpha, "Particle VB (alpha)");
        ensure(&mut vb_b, &mut cap_b, additive, "Particle VB (additive)");
        let vb_alpha = (!alpha.is_empty()).then(|| vb_a.as_ref().expect("ensured")).cloned();
        let vb_add = (!additive.is_empty()).then(|| vb_b.as_ref().expect("ensured")).cloned();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Particle Encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Pass"),
                timestamp_writes: self.pass_timer("gpu.particles"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_bind_group(1, &self.particle_frame_bind_group, &[]);
            if let Some(vb) = &vb_alpha {
                rp.set_pipeline(&self.particle_pipeline_alpha);
                rp.set_vertex_buffer(0, vb.slice(..));
                rp.draw(0..4, 0..alpha.len() as u32);
            }
            if let Some(vb) = &vb_add {
                rp.set_pipeline(&self.particle_pipeline_additive);
                rp.set_vertex_buffer(0, vb.slice(..));
                rp.draw(0..4, 0..additive.len() as u32);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.particle_vb_alpha = vb_a;
        self.particle_vb_additive = vb_b;
        self.particle_vb_alpha_cap = cap_a;
        self.particle_vb_additive_cap = cap_b;
    }

    /// Advance the GPU particle pool, creating it on first use (v0.1068).
    /// `live` is how many slots to simulate this frame; the pool itself only
    /// grows, so dialling density up and down costs nothing after the peak.
    /// Stop drawing the GPU precipitation pool. Called every frame the GPU
    /// path is inactive (Clear condition, altitude gate closed, setting off):
    /// the sim dispatch stopping does NOT stop the draw, so without zeroing
    /// `live` the last-written verts render forever as rain frozen mid-air
    /// (operator field report 2026-07-31).
    pub fn deactivate_gpu_particles(&mut self) {
        if let Some(g) = self.gpu_particles.as_mut() {
            g.live = 0;
        }
    }

    pub fn simulate_gpu_particles(
        &mut self,
        params: particles_gpu::SimParams,
        live: u32,
        capacity_hint: u32,
    ) {
        let _cost = frame_costs::stage("cpu.gpu_particle_sim");
        if self.gpu_particles.is_none()
            || self
                .gpu_particles
                .as_ref()
                .is_some_and(|g| g.capacity() < capacity_hint)
        {
            self.gpu_particles = Some(particles_gpu::GpuParticles::new(
                &self.device,
                capacity_hint.max(live),
            ));
        }
        if let Some(g) = self.gpu_particles.as_mut() {
            g.simulate(&self.device, &self.queue, params, live);
        }
    }

    /// Draw the GPU-simulated pool. Deliberately the SAME pipeline and the same
    /// instanced quad the CPU path uses - the compute shader wrote the identical
    /// vertex layout, so nothing here knows or cares where the data came from.
    pub fn draw_gpu_particles_onto(&self, camera: &Camera, view: &wgpu::TextureView) {
        let _cost = frame_costs::stage("cpu.gpu_particles");
        let Some(g) = self.gpu_particles.as_ref() else {
            return;
        };
        if g.live == 0 {
            return;
        }
        // Same billboard basis the CPU path uses - see draw_particles_onto.
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );
        let right = camera.right();
        let up = right.cross(camera.forward()).normalize_or_zero() * -1.0;
        // right.w carries the scene illumination scalar (v0.1154): particle
        // billboards have no lighting of their own, and an authored-tint
        // raindrop rendered at full brightness at midnight (the operator's
        // "rain glows at night" report). Daylight = 1, night = the floor.
        let frame: [f32; 8] =
            [right.x, right.y, right.z, self.scene_illum(), up.x, up.y, up.z, 0.0];
        self.queue
            .write_buffer(&self.particle_frame_buffer, 0, bytemuck::cast_slice(&frame));
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GPU Particle Encoder"),
            });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GPU Particle Pass"),
                timestamp_writes: self.pass_timer("gpu.gpu_particles"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_bind_group(1, &self.particle_frame_bind_group, &[]);
            rp.set_pipeline(&self.particle_pipeline_alpha);
            rp.set_vertex_buffer(0, g.vertex_buf.slice(..));
            rp.draw(0..4, 0..g.live);
        }
        self.queue.submit(std::iter::once(enc.finish()));
    }

    /// Orbit paths drawn with the CELESTIAL far plane (v0.451) so the AU-scale rings
    /// are not clipped by the gameplay far (~500 m) the way `draw_lines_onto` clips them.
    /// Call BETWEEN `render_celestial_onto` and `render_scene_onto`: it loads the
    /// celestial depth (so a ring passing behind a planet is occluded by that body) and
    /// the interior scene then clears depth + draws OVER the rings where home geometry
    /// exists (walls occlude the sky-rings). Same transient-VB approach as `draw_lines_onto`.
    pub fn draw_celestial_lines_onto(
        &self,
        camera: &Camera,
        verts: &[line::LineVertex],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.celestial_lines");
        if verts.len() < 2 {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&camera.celestial_uniforms()),
        );
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Celestial Line VB"),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Celestial Line Encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Celestial Line Pass"),
                timestamp_writes: self.pass_timer("gpu.celestial_lines"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve stars + bodies
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // test against the celestial bodies
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_pipeline(&self.line_pipeline);
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_vertex_buffer(0, vbuf.slice(..));
            rp.draw(0..verts.len() as u32, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Acquire surface and clear to black, returning the texture for star + scene rendering.
    pub fn acquire_surface(&self) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        self.frame_costs_begin(true);
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok((output, view))
    }

    /// Render instanced batches — objects sharing the same mesh/material are
    /// drawn with a single draw call each. More efficient than `render()` when
    /// many objects share geometry (trees, rocks, buildings).
    pub fn render_instanced(
        &self,
        camera: &Camera,
        batches: &[InstanceBatch],
    ) -> Result<(), wgpu::SurfaceError> {
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Instanced Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Instanced Render Pass"),
                timestamp_writes: self.pass_timer("gpu.instanced"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // reverse-Z: clear to 0 (farthest)
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline.render_pipeline);
            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            let mut bound_material = usize::MAX;
            for batch in batches {
                let mesh = match self.meshes.get(batch.mesh) {
                    Some(m) => m,
                    None => continue,
                };
                let material = match self.materials.get(batch.material) {
                    Some(m) => m,
                    None => continue,
                };

                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): consecutive batches can share a material.
                if bound_material != batch.material {
                    bound_material = batch.material;
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(
                    mesh.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );

                // Draw each instance with its own transform via the shared object buffer.
                // Uses the same uniform-per-draw approach as render() but avoids
                // per-frame buffer allocation. For truly GPU-instanced rendering
                // (single draw call per batch), a storage buffer or instance vertex
                // buffer with shader changes would be needed.
                for transform in &batch.transforms {
                    let normal_matrix = transform.inverse().transpose();
                    let object_uniforms = ObjectUniforms {
                        model: transform.to_cols_array_2d(),
                        normal_matrix: normal_matrix.to_cols_array_2d(),
                    };
                    self.queue.write_buffer(
                        &self.object_buffer,
                        0,
                        bytemuck::bytes_of(&object_uniforms),
                    );
                    render_pass.set_bind_group(1, &self.object_bind_group, &[]);
                    render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    /// Create the off-screen scene texture (same format as surface, with TEXTURE_BINDING).
    fn create_scene_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}
