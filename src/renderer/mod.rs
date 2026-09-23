//! Renderer — the `Renderer` struct, its construction, and its registries.
//!
//! Configuration loaded from `config/renderer.toml`.
//! Supports both native (winit window) and WASM (canvas) targets.
//!
//! THE PASSES NO LONGER LIVE HERE (v0.1319, the file-size ratchet: this file
//! had reached 5,652 lines against a 3,883 budget). What is left is the state
//! and the things that fill it: the struct itself, `init` (which builds every
//! GPU resource it holds), and the registries and per-frame setters - meshes,
//! the terrain patch arena, grass instances, lights, the live weather map,
//! the atmosphere LUTs. Each pass moved to a sibling file whose own header
//! says what it does and why it is a cluster:
//!
//!   * `celestial.rs`    - the far world: planets, terrain patches, the sun
//!                         shadow map, the shells, the cloud march.
//!   * `scene_draw.rs`   - the near world: the opaque, transparent and
//!                         overlay draw loops, god rays, SSAO.
//!   * `overlay_draw.rs` - post-passes over a finished frame: orbit lines
//!                         and particle billboards, CPU and GPU.
//!   * `surface.rs`      - the swapchain and render-target lifecycle.
//!   * `materials.rs`    - the material registry (v0.1093).
//!   * `capture.rs`      - pixels back off the GPU and onto disk (v0.1108).
//!
//! They are all `impl Renderer` blocks in child modules, so every call site
//! in the crate is unchanged: an inherent method resolves by receiver type,
//! not by module path.

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
/// Positioned, sized environmental effects (storms, fog banks, fires) that
/// every consumer reads the same way. See docs/design/environment-fields.md.
pub mod env_regions;
/// Light that lives ABOVE the cloud deck, drawn after the cloud composite.
pub mod emission_pass;
pub mod bloom;
pub mod godrays;
pub mod ssao;
pub mod camera;
/// The far-world pass: planets, terrain patches, the sun shadow map, the
/// shells and the cloud march. Extracted from mod.rs in v0.1319 - see the
/// file's header for why this cluster and what its byte-offset pokes need.
pub mod celestial;
pub mod celestial_order;
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
pub mod material_bind_groups;
pub mod materials;
pub mod mesh;
pub mod multi_scale;
/// Post-passes drawn over a finished frame: orbit lines and particle
/// billboards, CPU and GPU. Extracted from mod.rs in v0.1319 - see the file's
/// header for why this cluster.
pub mod overlay_draw;
pub mod patch_arena;
pub mod plant_mesh;
pub mod tree_mesh;
pub mod particles;
pub mod particles_gpu;
pub mod pipeline;
/// The near-world draw loops: opaque, transparent, overlay, god rays, SSAO,
/// and the whole-frame wrappers around them. Extracted from mod.rs in
/// v0.1319 - see the file's header for why this cluster.
pub mod scene_draw;
pub mod shader_loader;
/// Which material types can DISCARD in the sun shadow pass, and the test that
/// keeps that answer equal to the shader's (v0.1108).
pub mod shadow_cutout;
pub mod stars;
/// Swapchain + render-target lifecycle: configure, resize, acquire, and the
/// two target constructors everything else shares. Extracted from mod.rs in
/// v0.1319 - see the file's header for why this cluster.
pub mod surface;
/// Which depth texture is current, the window's or an off-screen view's.
/// Extracted from capture.rs on 2026-09-19 - see the file's header for why.
pub mod view_depth;
pub mod water;

/// Sun shadow map resolution, texels per side. Module constants (v0.1104)
/// rather than locals inside the render loop, because the megashader's
/// normal-offset needs the WORLD size of one texel and pins it as a literal
/// (`SHADOW_TEXEL_M` in 00-bindings-vertex.wgsl). `sky_ambient`'s lockstep
/// test compares the two.
pub const SUN_SHADOW_MAP_SIZE: f32 = 4096.0;
/// Half-extent of the sun shadow map's ortho box, in metres.
pub const SUN_SHADOW_EXTENT_M: f32 = 1500.0;

/// The VSync switch's two pure helpers moved to `surface.rs` with the rest of
/// the swapchain lifecycle (v0.1319). Re-exported here so the old
/// `renderer::vsync_present_mode` path still resolves.
pub use surface::{pending_mode_change, vsync_present_mode};

use camera::{Camera, CameraUniforms};
use glam::{Mat4, Quat, Vec3};
use mesh::Mesh;
use pipeline::{ObjectUniforms, Pipeline, ShaderClass};

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
    /// The albedo texture this material OWNS, with its pixel size, when it
    /// has one (in-world screens, rung 1). `add_textured_material` and
    /// `set_material_albedo_texture` fill it; `update_material_albedo_pixels`
    /// writes new pixels into it in place when the size matches, which is
    /// what a 30 fps live feed needs (the old reallocate-every-call path was
    /// wrong for video). `None` for materials whose texture lives elsewhere
    /// and is only bound by view (a screen surface's texture, the temporal
    /// cloud map): the bind group keeps that view alive, not this field.
    albedo_texture: Option<(wgpu::Texture, u32, u32)>,
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
    /// A present-mode change waiting for the start of the next frame (see
    /// `set_vsync` and BUG-077: the surface must not be reconfigured while
    /// a frame's swapchain view is alive). `None` almost always.
    pending_present_mode: Option<wgpu::PresentMode>,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    /// The spare depth buffer for off-screen views (camera screens, the
    /// hi-res screenshot), swapped in around `render_view_onto` so the
    /// window's depth buffer is never recreated per view. See
    /// `view_depth::ViewDepth` and `begin_view_depth` / `end_view_depth`.
    view_depth: view_depth::ViewDepth,
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
    /// Environment regions (v0.1329): positioned, sized environmental effects
    /// at binding 4, the same uncapped-storage shape v0.782 gave lights. Grows
    /// by doubling; a grow recreates the camera bind group, so EVERY site that
    /// builds one has to bind this or world entry fails validation (the
    /// v0.1029 lesson). See renderer/env_regions.rs.
    env_regions_buffer: wgpu::Buffer,
    env_regions_capacity: usize,
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
    /// field's seed). Kept on the struct so build_albedo_group_from_view
    /// can include them in every bind group it makes.
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
    /// Rosette-bisect channel (0 = off; showcase map_diag) - poked into
    /// light7_color.z for the octa pass's EMA-bypassed diagnostic render.
    pub cloud_map_diag: f32,
    /// Freeze the cloud advection clock at this many seconds (negative =
    /// live). Dev/measurement only.
    ///
    /// The clock is app-start-relative, so every probe-rig boot dropped the
    /// cloud field somewhere new: the SAME build and SAME vantage captured
    /// twice differed in 20% of pixels by more than 40 levels (measured
    /// 2026-09-01). Every cross-run A/B in the rosette arc was therefore
    /// comparing two different cloud fields rather than two builds, which
    /// is why fixes kept "measuring" as improvements and then failed in
    /// flight. Pinning this makes a capture a function of the build alone.
    pub cloud_clock_pin: f32,
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
    /// Exact spin-aware motion split for the resolve (v0.1251), set by
    /// lib.rs each armed frame (f64 chain); taken at staging time.
    pub cloud_resolve_motion: std::cell::Cell<Option<cloud_resolve::CloudResolveMotion>>,
    /// Dev bisect (showcase {"cloud_temporal":"0"}): force the resolve's
    /// snap path every frame - raw march, no temporal accumulation.
    pub cloud_temporal_off: bool,
    /// Operator taste toggle (showcase {"cloud_dither":"0"}): disable the
    /// frozen spatial dither - smooth interiors, agate arcs on sheets.
    /// Written to pad light7_color.w (offset 332).
    pub cloud_dither_off: bool,
    /// Cloud march resolution divisor (v0.1255, operator: "it seems like
    /// the cloud layer is lower resolution than the surface layer, would
    /// upping the resolution help?"). It IS lower: 4 = the historical
    /// quarter-res march - one cloud sample per 4x4 screen pixels, which
    /// is the "solid pixels" look and the binary opaque/clear edges.
    /// 2 = half, 1 = full. The accumulation pair follows at half this.
    pub cloud_res_div: u32,
    /// F10 A/B: disable the per-cloud shape frame (squash + wind stretch),
    /// rendering the old isotropic ball cluster. Pad light7_color.w bit 1.
    pub cloud_shape_off: bool,
    /// Restore the pre-v0.1268 detail scale: freeze the body scale once per
    /// ray from the slab CHORD (a function of viewing angle) instead of
    /// tracking each sample own footprint. Comparison switch only - it turns
    /// the sun-channel rosette back on, which is the point of having it.
    pub cloud_chord_foot: bool,
    /// Evaluate the SHAPE-defining cloud fields (domain warp, displacement
    /// octaves) at a fixed WORLD level of detail instead of one chosen from
    /// camera distance. The invariant is already written above
    /// g_v2_disp_lod in 40-clouds.wgsl and the code violates it: a cloud
    /// changes shape as you fly toward it, and lines of equal
    /// distance-to-camera are circles centred on the nadir point.
    pub cloud_world_shape_lod: bool,
    /// Turn OFF the per-pixel lod dither (the 2026-08-24 "mip ring cure").
    /// DEFAULT TRUE - the dither is off - because measurement says it never
    /// helps and sometimes hurts. Radial energy at three altitudes, depth
    /// jitter on, clock pinned, 0.7 noise floor:
    ///   3.4 km  23.65 with it, 21.05 without   (WORSE with it)
    ///   9.2 km  23.94 with it, 23.74 without   (neutral)
    ///   60 km   14.71 with it, 14.71 without   (neutral)
    /// A per-pixel random mip through the non-linear carve averages to a
    /// biased result, and the bias itself varies with radius, so the cure
    /// carried its own radial signature. The DEPTH jitter is the term that
    /// actually suppresses the artifact (~3.6) and stays on its own switch.
    pub cloud_ring_cure_off: bool,
    /// Experiment (dev pad bit 5): march step depends on camera distance only,
    /// never on the angle to the local vertical.
    pub cloud_uniform_step: bool,
    /// Experiment (dev pad bit 6): density edge widened to the radiative
    /// smoothing scale (~300 m) on both cloud paths.
    pub cloud_wide_edge: bool,
    /// Runtime edge-width multiplier on the carve hinge (0 = shader constant).
    pub cloud_edge_mul: f32,
    /// Runtime wide rind in metres for the constructed bodies (0 = constant).
    pub cloud_rind_wide_m: f32,
    /// Fixed march step in metres for the estimator test (0 = off).
    pub cloud_step_m: f32,
    /// Uniform eastward lean, metres per metre of height above the deck
    /// base (light6_color.w). 0 = off. The prism-wall discriminator.
    pub cloud_shear: f32,
    /// Height-varying warp amplitude in km (light5_color.x); 0 = shader constant.
    pub cloud_hv_km: f32,
    /// Extinction multiplier (light5_color.y); 0 = off. The transparency test.
    pub cloud_sigma_mul: f32,
    /// Dev pad bit 7: the sample-anchored march (v0.1272).
    pub cloud_est: bool,
    /// Dev pad bit 8: warp band-limited to its own tile + rind/4 refine.
    pub cloud_warp_bl: bool,
    /// Dev pad bit 10: carve normaliser floor (design item 2A, v0.1273).
    pub cloud_norm_floor: bool,
    /// Dev pad bit 9: isotropic near step + bounded far angle term (v0.1274).
    pub cloud_iso_step: bool,
    /// Dev pad bit 11: thin-deck experiment (band height x0.3).
    pub cloud_thin_deck: bool,
    /// Dev pad bit 12: height-varying domain warp on the noise body (design 2c).
    pub cloud_hv_warp: bool,
    /// Component bisect (v0.1279), dev pad bits 13-17: one density term off.
    pub cloud_no_detail: bool,
    pub cloud_no_puff: bool,
    pub cloud_no_cell: bool,
    pub cloud_no_fray: bool,
    pub cloud_no_bdrop: bool,
    /// Dev pad bit 18: sharp cloud base (0.5% of the band instead of 3%).
    pub cloud_sharp_base: bool,
    /// Dev pad bit 19: interior relief fade (relief weighted by eye transmittance).
    pub cloud_relief_fade: bool,
    /// Dev pad bit 20: coarse sun ladder for deep view samples.
    pub cloud_deep_rung: bool,
    /// Dev pad bit 21: synthetic checker density (the projection test).
    pub cloud_checker: bool,
    /// Dev pad bit 22: increment A, the in-cloud light (Eddington source,
    /// depth-split sun ladder, rind-only relief and hue).
    pub cloud_ms: bool,
    /// Dev pad bit 23 (the last exact f32 bit): increment B 2.1, the
    /// three-octave domain warp on the noise path.
    pub cloud_field: bool,
    /// Dev pad bit 17 (perf increment 3, v0.1287): the per-ray body cluster
    /// cache in cloud_v2_body (cell weather + built lobes kept per ray).
    pub cloud_body_cache: bool,
    /// Perf increment 2 (v0.1288): step-economy strength 0..1 in
    /// light7_color.y (footprint step floors + deep relaxation).
    pub cloud_step_eco: f32,
    /// Dev pad bit 16 (performance plan increment 1, v0.1286): the
    /// sun-shadow CACHE. On = the march reads rungs 2..11 of the sun
    /// ladder from the planet-fixed slice atlas baked by the Cloud Light
    /// Bake Pass; off = the full 12-rung ladder per pixel (the A/B twin).
    /// Mirrored from GuiState::cloud_dev_light by lib.rs.
    pub cloud_light: bool,
    /// The atlas + planning state, created on first use by
    /// `ensure_cloud_light` (cloud_temporal.rs).
    pub(crate) cloud_light_cache: Option<cloud_temporal::CloudLightCache>,
    /// This frame's ground point / sun / planet constants from the cloud
    /// fill block (None = no cloud body armed this frame, the pass skips).
    pub(crate) cloud_light_frame: Option<cloud_temporal::CloudLightFrame>,
    /// The cloud PROFILE knob (perf increment 4, the far rung; pad
    /// light2_color.z): 0 = off (the point-sampled field, bit-identical,
    /// the A/B twin), 1 = on (automatic level by footprint), 2..7 = level
    /// 0..5 forced, 8 = hard switch (the prove-red), 9 = the reference
    /// bake. Mirrored from GuiState::cloud_dev_profile_knob by lib.rs.
    pub cloud_profile_knob: i32,
    /// D3 dev bit (2026-09-06; flags-pad bit 12 of light2_color.w, see
    /// `cloud_temporal::CLOUD_FR_FLAG_TOP_BOUND`): the BUILT-BODY TOP
    /// BOUND. On = the body publishes a from-above SDF lower bound so the
    /// march's stride finds thin built clouds under the 928 m comb, and
    /// the step economy's in-cloud floor is capped at a quarter of the
    /// found cloud's height. Off (default) = today's comb, the A/B twin.
    /// Independent of the profile knob and of the atlas: the pad carries
    /// it every frame, atlas or not. Mirrored from
    /// GuiState::cloud_dev_top_bound by lib.rs; showcase key
    /// `cloud_top_bound`.
    pub cloud_top_bound: bool,
    /// The profile atlas, its views / groups and the planning state,
    /// created on first use by `ensure_cloud_profile` (cloud_temporal.rs).
    pub(crate) cloud_profile_cache: Option<cloud_temporal::CloudProfileCache>,
    /// This frame's ground cell / altitude / clock / coverage inputs from
    /// the cloud fill block on EVERY tier (None = no cloud shell this frame).
    pub(crate) cloud_profile_frame: Option<cloud_temporal::CloudProfileFrame>,
    /// The cloud SHELL material this frame on every tier (set by lib.rs
    /// unconditionally beside the temporal arming, cleared at the top of
    /// every frame): the far-rung passes find the shell's object slot by
    /// it, and the transparent shell draw binds the profile group by it.
    pub(crate) cloud_shell_mat: Option<usize>,
    /// The MARCH texture's pixel angle `2 tan(fov/2) / (height / div)`,
    /// stored beside the screen pix_ang write each celestial pass and read
    /// by lib.rs for the profile's active-level range (the temporal march
    /// derives its footprint from its own rasterizer, not the screen's).
    pub cloud_pix_ang_march: std::cell::Cell<f32>,
    /// Bumped at every weather-map upload (`update_weather_map`): the
    /// profile's global map re-references on it.
    pub weather_map_gen: std::cell::Cell<u32>,
    /// The 1x1 white fallback albedo view (group-3 binding 0 of every
    /// untextured draw); kept as a field so the profile's bind groups can
    /// put it at binding 0 while the atlas rides binding 14.
    pub(crate) white_view: wgpu::TextureView,
    /// Gain on the in-scattered source (light5_color.z); 0 = 1.0.
    pub cloud_ms_gain: f32,
    /// Increment C (v0.1282): interior density saturation of the constructed
    /// bodies, 0..1, light5_color.w (offset 300). 0 = the v0.1231 profile.
    pub cloud_int_sat: f32,
    /// F10 bisect: paint WHY each pixel's cloud was discarded by the
    /// composite instead of discarding it (v0.1262).
    pub cloud_discard_diag: bool,
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
    /// SHADOW-ONLY celestial objects (frame-cost arc V1, 2026-09-18): the
    /// index range of this frame's celestial object list that the COLOUR
    /// pass skips and the sun shadow pass still draws. The near-tree loop
    /// appends its frustum-culled models here: a photoscan standing behind
    /// the camera rasterises nothing in colour, but at a low sun it still
    /// shades the ground in front of the player, so it keeps casting. Set
    /// per frame by lib.rs, reset to empty with `tree_card_hide_m`.
    pub celestial_colour_skip: std::ops::Range<usize>,
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
    /// Shared (`Arc`) so a pass submitted outside this module - the in-world
    /// screens' egui pass in `gui::screen_surface` - can open a timestamp
    /// scope through the same query ring; see `Renderer::gpu_timers`.
    gpu_timers: Option<std::sync::Arc<frame_costs::GpuTimers>>,
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

        // Cloud-noise generation runs on a background thread so the 384^3 +
        // 256^3 volume bake overlaps device/shader-compile time; init()
        // recv()s only the unfinished remainder (v0.872). The MIP CHAINS are
        // built on the same thread as of v0.1188 (they used to run inline in
        // init(), fine at 192^3 but ~1.5 s of boot-path stall at 384^3, and
        // it is the same pure-CPU work), so the channel carries finished
        // chains and the upload side does nothing but write_texture.
        // Ground-texture CPU bake rides along with it (v0.1133): ~1 s of PNG
        // decode + mip-chain building, the same pure-CPU shape.
        //
        // BOTH now start INSIDE init(), the moment the adapter request
        // returns, rather than here (v0.1322). See the note at that spawn
        // site: the adapter request is the one phase of the boot these
        // threads must not be running during.

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

        Self::init(instance, surface, width, height, true).await
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

        Self::init(instance, surface, width, height, false).await
    }

    /// Shared initialization: adapter, device, pipeline, depth buffer.
    /// `background_bakes`: true on native, where the cloud-noise volumes and
    /// the ground textures are baked on background threads that overlap the
    /// device request and the shader compiles. False on wasm, which has no
    /// threads and generates both inline where they are needed.
    async fn init(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        background_bakes: bool,
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

        // ── The background bakes start HERE, and not one line earlier ──
        // They used to spawn in `new_native`, before the instance existed,
        // on the reasoning that more overlap is better. It is not: the next
        // thing the boot thread does is ask the driver for an adapter, which
        // is LATENCY-bound work (DXGI enumeration, a D3D12 device per adapter
        // to query capabilities) that a saturated CPU stretches badly.
        // Measured 2026-09-19 on the boot rig by delaying the two bakes past
        // the adapter and changing nothing else: adapter_request 2173 ms ->
        // 345 ms. Nearly two seconds of "the driver is slow" was this
        // process's own twelve noise-bake threads. Everything AFTER the
        // adapter is throughput-bound and shares the machine happily (device
        // + shader modules + 19 PSO compiles run about 5 s, the bake needs
        // about 4, and the upload still reports "waited 0 ms"). If you move
        // these again, re-read `[BootPhase] adapter_request`: that is the
        // number that says whether you were right.
        let (cloud_rx, ground_rx) = if background_bakes {
            let (tx, cloud_rx) = std::sync::mpsc::channel();
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
            let (tx, ground_rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(ground_textures::bake_all());
            });
            (Some(cloud_rx), Some(ground_rx))
        } else {
            (None, None)
        };

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
        // Can this backend hand us a compiled-pipeline blob to keep between
        // boots? That is the standard answer to a slow PSO build, and it is
        // the first thing anyone looking at the `[Pipelines]` line will
        // reach for, so the run log says outright whether it is even on
        // offer. In wgpu 24 only the VULKAN backend advertises
        // `PIPELINE_CACHE`; the DX12 backend's `create_pipeline_cache` is a
        // stub that stores nothing and returns no data, and Windows runs
        // DX12 here. Logged rather than silently skipped so this stops being
        // re-investigated, and so the day it flips to true somebody sees it.
        log::info!(
            "[Pipelines] adapter offers a persistent pipeline cache: {}",
            adapter.features().contains(wgpu::Features::PIPELINE_CACHE)
        );
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
        let gpu_timers = frame_costs::GpuTimers::new(&device, &queue).map(std::sync::Arc::new);
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
        // Environment regions (v0.1329). Starts at 16 rows (768 bytes) because
        // a planet has a handful of weather systems, not thousands; it doubles
        // like the lights buffer if that ever stops being true. Never zero-sized:
        // wgpu rejects a zero-length storage binding, and the shader walks
        // arrayLength() over the whole thing with empty rows contributing nothing.
        let env_regions_capacity = 16_usize;
        let env_regions_buffer = env_regions::storage_buffer(&device, env_regions_capacity);
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

        let camera_bind_group = pipeline::camera_bind_group(
            &device,
            &pipeline.camera_bind_group_layout,
            "Camera Bind Group",
            &camera_buffer,
            &lights_buffer,
            &tile_counts_buffer,
            &tile_indices_buffer,
            &env_regions_buffer,
        );

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
        let light_camera_bind_group = pipeline::camera_bind_group(
            &device,
            &pipeline.camera_bind_group_layout,
            "Light Camera BG",
            &light_camera_buffer,
            &lights_buffer,
            &tile_counts_buffer,
            &tile_indices_buffer,
            &env_regions_buffer,
        );

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
            view_depth: view_depth::ViewDepth::None,
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
            env_regions_buffer,
            env_regions_capacity,
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
            cloud_map_diag: 0.0,
            cloud_clock_pin: -1.0,
            cloud_octa_idle: std::cell::Cell::new(0),
            cloud_octa_boost: std::cell::Cell::new(0.0),
            cloud_prev_delta2: std::cell::Cell::new(0.0),
            cloud_octa_phase: std::cell::Cell::new(0),
            cloud_mode_near: false,
            cloud_near_mix: 0.0,
            cloud_screen: None,
            cloud_prev_basis: std::cell::Cell::new(None),
            cloud_resolve_frame: std::cell::Cell::new(Default::default()),
            cloud_resolve_motion: std::cell::Cell::new(None),
            cloud_temporal_off: false,
            cloud_dither_off: false,
            cloud_res_div: 4,
            cloud_shape_off: false,
            cloud_chord_foot: false,
            cloud_world_shape_lod: false,
            cloud_ring_cure_off: true,
            cloud_uniform_step: false,
            cloud_wide_edge: false,
            cloud_edge_mul: 0.0,
            cloud_rind_wide_m: 0.0,
            cloud_step_m: 0.0,
            cloud_shear: 0.0,
            cloud_hv_km: 0.0,
            cloud_sigma_mul: 0.0,
            cloud_est: true,
            cloud_warp_bl: true,
            cloud_norm_floor: false,
            cloud_iso_step: false,
            cloud_thin_deck: false,
            cloud_hv_warp: false,
            cloud_no_detail: false,
            cloud_no_puff: false,
            cloud_no_cell: false,
            cloud_no_fray: false,
            cloud_no_bdrop: false,
            cloud_sharp_base: false,
            cloud_relief_fade: false,
            cloud_deep_rung: false,
            cloud_checker: false,
            cloud_ms: true,
            cloud_field: false,
            cloud_body_cache: true,
            cloud_step_eco: 1.0,
            cloud_light: true,
            cloud_light_cache: None,
            cloud_light_frame: None,
            // Default 0 (off) until gates G0..G6 of the far-rung contract
            // pass; the orchestrator flips it to 1.
            cloud_profile_knob: 0,
            // D3 dev bit, default OFF until its gate (prof-vert-250/60-r1
            // fix vs ref) passes; the orchestrator flips it.
            cloud_top_bound: false,
            cloud_profile_cache: None,
            cloud_profile_frame: None,
            cloud_shell_mat: None,
            cloud_pix_ang_march: std::cell::Cell::new(0.0),
            weather_map_gen: std::cell::Cell::new(0),
            white_view,
            cloud_ms_gain: 1.0,
            cloud_int_sat: 0.0,
            cloud_discard_diag: false,
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
            celestial_colour_skip: 0..0,
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
            pending_present_mode: None,
        }
    }

    /// Megashader hot-reload (v0.924, dev-aid): when a part under
    /// assets/shaders/pbr/ changes on disk, VALIDATE the reassembled source
    /// with naga first (a mid-edit save logs and keeps the old pipelines,
    /// never crashes; so does a source from before the shader permutation,
    /// no switch declared or used, which would otherwise compile fine and
    /// silently hand the terrain pass the cloud march again), then rebuild
    /// every PSO compiled from the module
    /// in place. Bind group layouts are reused, so every live
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
        // The counts come back from the rebuild itself (its slot tables),
        // so this line reports what was installed, not a number typed here.
        let rebuilt = self
            .pipeline
            .recreate_pipelines(&self.device, format, &module, &batch_module);
        log::info!(
            "[HotReload] megashader reassembled + {} PSOs rebuilt ({} megashader + {} cloud) in {:.1}s",
            rebuilt.total(),
            rebuilt.megashader,
            rebuilt.cloud,
            t0.elapsed().as_secs_f32()
        );
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
        // The cloud profile's global map is baked from this map: count the
        // upload so the next plan re-references it (a fast 2 s pass).
        self.weather_map_gen.set(self.weather_map_gen.get().wrapping_add(1));
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
            self.camera_bind_group = pipeline::camera_bind_group(
                &self.device,
                &self.pipeline.camera_bind_group_layout,
                "Camera Bind Group",
                &self.camera_buffer,
                &self.lights_buffer,
                &self.tile_counts_buffer,
                &self.tile_indices_buffer,
                &self.env_regions_buffer,
            );
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

    /// Upload the frame's environment regions (v0.1329).
    ///
    /// Deliberately has NO count uniform. The shader walks arrayLength() over
    /// the whole buffer and an empty row contributes nothing, so the tail is
    /// zero-filled instead. A count would have needed a free lane in the camera
    /// uniform, and running out of those is the problem this whole mechanism
    /// exists to stop (BUG-080, docs/design/environment-fields.md).
    pub fn set_env_regions(&mut self, regions: &[env_regions::EnvRegion]) {
        // Grow by doubling, like the lights buffer. A grow invalidates the
        // camera bind group (bind groups are immutable), so it is rebuilt here
        // with EVERY binding - missing one is the v0.1029 failure where world
        // entry panics but a menu-only boot stays green.
        if regions.len() > self.env_regions_capacity {
            let mut cap = self.env_regions_capacity.max(1);
            while cap < regions.len() {
                cap *= 2;
            }
            self.env_regions_capacity = cap;
            self.env_regions_buffer = env_regions::storage_buffer(&self.device, cap);
            self.camera_bind_group = pipeline::camera_bind_group(
                &self.device,
                &self.pipeline.camera_bind_group_layout,
                "Camera Bind Group",
                &self.camera_buffer,
                &self.lights_buffer,
                &self.tile_counts_buffer,
                &self.tile_indices_buffer,
                &self.env_regions_buffer,
            );
        }
        let packed = env_regions::pack_all(regions, self.env_regions_capacity);
        self.queue.write_buffer(
            &self.env_regions_buffer,
            0,
            bytemuck::cast_slice(&packed),
        );
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

            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            let mut bound_material = usize::MAX;
            // The opaque PSO by the batch material's class (P3), switched
            // only when the class changes: batches are grouped by material
            // already, so this binds once per run of a class.
            let mut bound_class: Option<ShaderClass> = None;
            for batch in batches {
                let mesh = match self.meshes.get(batch.mesh) {
                    Some(m) => m,
                    None => continue,
                };
                let material = match self.materials.get(batch.material) {
                    Some(m) => m,
                    None => continue,
                };
                let class = pipeline::shader_class(material.material_type);
                if bound_class != Some(class) {
                    bound_class = Some(class);
                    // `opaque_for` debug-asserts on a shell, cloud or water
                    // batch: no such batch exists, and one would be a bug.
                    render_pass.set_pipeline(self.pipeline.opaque_for(class));
                    render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                    bound_material = usize::MAX;
                }

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
}
