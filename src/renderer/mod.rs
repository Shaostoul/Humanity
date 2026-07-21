//! Renderer — wgpu device/surface setup and render loop.
//!
//! Configuration loaded from `config/renderer.toml`.
//! Supports both native (winit window) and WASM (canvas) targets.

pub mod atmosphere;
pub mod bloom;
pub mod godrays;
pub mod ssao;
pub mod camera;
/// Non-blocking swapchain readback for live streaming (v0.853). The screenshot path
/// stalls the GPU on purpose; a stream must never do that. See stream_capture.rs.
///
/// NATIVE-GATED: it hands frames to `net::live`, which is native-only. `renderer` as a
/// whole is NOT gated, so an ungated submodule that reaches into `net` breaks the relay
/// build (and therefore CI's VPS deploy) while the native build stays green.
#[cfg(feature = "native")]
pub mod stream_capture;
pub mod cloud_noise;
pub mod clouds;
pub mod ground_textures;
pub mod floating_origin;
pub mod hologram;
pub mod light;
pub mod line;
pub mod mesh;
pub mod multi_scale;
pub mod plant_mesh;
pub mod particles;
pub mod pipeline;
pub mod shader_loader;
pub mod stars;
pub mod water;

use camera::{Camera, CameraUniforms};
use glam::{Mat4, Quat, Vec3};
use mesh::Mesh;
use pipeline::{MaterialUniforms, ObjectUniforms, Pipeline};

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

/// Material properties for PBR-lite rendering.
pub struct Material {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: f32,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Group-3 texture bind group for materials that carry real imagery
    /// (v0.811: per-pixel planet albedo). None = the renderer binds its 1x1
    /// white fallback instead, so every draw satisfies the shared pipeline
    /// layout. The bind group internally keeps its texture + view alive.
    albedo_bind_group: Option<wgpu::BindGroup>,
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

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pipeline: Pipeline,
    /// World-space thin-line pipeline (orbit paths). Shares the main
    /// camera bind group; reverse-Z depth-test, no depth-write.
    line_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    /// Uncapped scene-light list (v0.782): a storage buffer of 64-byte GpuLight
    /// entries; grows by doubling (bind group recreated) when the count exceeds
    /// capacity. The shader loops over `light_count` of these.
    lights_buffer: wgpu::Buffer,
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
    /// 1x1 white fallback bound at group 3 for every material without real
    /// imagery, so the shared pipeline layout is always satisfied and
    /// non-planet draws are unaffected (the shader only samples group 3 on
    /// material type 12 with the params.w flag set).
    default_texture_bind_group: wgpu::BindGroup,
    /// Shared tiling 3D cloud-noise volumes (clouds increment 3): the SHAPE
    /// (128^3 Perlin-Worley + Worley octaves) and DETAIL (64^3 Worley
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
    light_camera_bind_group: wgpu::BindGroup,
    shadow_comparison_sampler: wgpu::Sampler,
    ground_textures: ground_textures::GroundTextures,
    /// Sun shadows on/off (max-graphics default on; zero cost when the sun
    /// is absent - the pass and the shader lookup both self-gate).
    pub sun_shadows: bool,
    /// Screen-space ambient occlusion (v0.901): contact shading in the
    /// celestial slot. Strength 0 disables the pass entirely.
    ssao: ssao::SsaoPass,
    pub ssao_strength: f32,
    /// Detail-draw-distance factor (v0.905): scales every shader detail
    /// octave's anti-alias fade so fine structure survives further out.
    /// Synced from Settings each frame; poked into the view_pos.w pad.
    pub detail_distance: f32,
    /// Sea state 0..1 (v0.909): glassy -> ripples -> storm. Poked into the
    /// fill_color.w uniform pad each celestial pass.
    pub sea_state: f32,
    /// Tree-card hide radius in metres (v0.912): terrain silhouette cards
    /// within this range of the camera discard (the real 3D tree models
    /// stand there). Mirrors the Settings tree-model distance; 0 = off.
    pub tree_card_hide_m: f32,
    /// Tree-card FAR cutoff (v0.924 vegetation LOD): the silhouette stage's
    /// outer distance in metres (the Settings slider). Cards past it discard.
    pub tree_card_far_m: f32,
    /// Aerial perspective (v0.916): extinction per metre at the CAMERA's
    /// altitude (strength + height falloff folded in by lib.rs; 0 = off).
    pub aerial_sigma: f32,
    /// Aerial slant cap: haze-layer thickness in metres, bounding vertical
    /// sightlines so the sun/orbit stay clear.
    pub aerial_slant_cap: f32,
    /// Aerial in-scatter (sky) color, day/sunset tinted by lib.rs.
    pub aerial_sky: [f32; 3],
    /// Camera's radial up (world), for the slant path bound.
    pub aerial_up: [f32; 3],
}

impl Renderer {
    /// Create a new renderer attached to a native winit window.
    #[cfg(feature = "native")]
    pub async fn new_native(window: std::sync::Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        // Cloud-noise generation starts NOW on a background thread so the
        // 192^3 + 128^3 volume bake overlaps adapter/device/shader-compile
        // time; init() recv()s only the unfinished remainder (v0.872).
        let cloud_rx = {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let threads =
                    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
                let t0 = std::time::Instant::now();
                let shape = cloud_noise::generate_shape(threads);
                let detail = cloud_noise::generate_detail(threads);
                log::info!(
                    "Cloud noise volumes generated in background: {:.0} ms ({} threads)",
                    t0.elapsed().as_secs_f32() * 1000.0,
                    threads
                );
                let _ = tx.send((shape, detail));
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

        Self::init(instance, surface, width, height, cloud_rx).await
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

        Self::init(instance, surface, width, height, None).await
    }

    /// Shared initialization: adapter, device, pipeline, depth buffer.
    /// `cloud_rx`: pre-spawned cloud-noise generation (native path) so the
    /// volume bake overlaps device/shader init; None generates inline (wasm).
    async fn init(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        cloud_rx: Option<std::sync::mpsc::Receiver<(Vec<u8>, Vec<u8>)>>,
    ) -> Self {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("No suitable GPU adapter found");

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
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("HumanityOS Renderer"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to create device");

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
        let bloom_pass = bloom::BloomPass::new(&device, width, height, surface_format);
        let godray_pass = godrays::GodrayPass::new(&device, surface_format);
        let ssao_pass = ssao::SsaoPass::new(&device, surface_format);

        // Shader + pipeline
        let shader_loader = shader_loader::ShaderLoader::new();
        let shader = shader_loader.load_embedded_pbr(&device);
        let pipeline = Pipeline::new(&device, surface_format, &shader);
        // World-space thin-line pipeline — reuses the SAME camera BGL so
        // it can bind the existing camera_bind_group (full view-proj).
        let line_pipeline = line::build_line_pipeline(
            &device,
            surface_format,
            &pipeline.camera_bind_group_layout,
        );

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
        // 128/64 -> 192/128 in v0.872): generated procedurally, deterministic,
        // no repo assets, shared by every group-3 bind group at bindings 2..4.
        // Generation runs on a BACKGROUND thread spawned at the very top of
        // renderer creation, overlapping the DXC shader compiles, so the
        // bigger volumes cost boot nothing: this recv() only blocks for
        // whatever remainder has not finished by the time uploads start.
        let gen_start = std::time::Instant::now();
        let (shape_bytes, detail_bytes) = match cloud_rx {
            Some(rx) => rx.recv().expect("cloud noise generator thread died"),
            None => {
                // Fallback (wasm / callers without the pre-spawn): inline.
                let threads =
                    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
                (cloud_noise::generate_shape(threads), cloud_noise::generate_detail(threads))
            }
        };
        log::info!(
            "Cloud noise volumes ready: {s}^3 shape + {d}^3 detail (waited {:.0} ms at upload)",
            gen_start.elapsed().as_secs_f32() * 1000.0,
            s = cloud_noise::SHAPE_SIZE,
            d = cloud_noise::DETAIL_SIZE,
        );
        let make_volume = |label: &str, size: u32, bytes: &[u8]| -> wgpu::TextureView {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: size,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                // Linear (NOT sRGB): this is noise data, not color.
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * size),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: size,
                },
            );
            tex.create_view(&wgpu::TextureViewDescriptor::default())
        };
        let cloud_shape_view =
            make_volume("Cloud Shape Noise (128^3)", cloud_noise::SHAPE_SIZE, &shape_bytes);
        let cloud_detail_view =
            make_volume("Cloud Detail Noise (64^3)", cloud_noise::DETAIL_SIZE, &detail_bytes);
        let cloud_tile_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cloud Noise Tile Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest, // single mip
            ..Default::default()
        });

        let weather_map_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Live Weather Map"),
            size: wgpu::Extent3d {
                width: WEATHER_MAP_W,
                height: WEATHER_MAP_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
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
        // Ground PBR texture array (v0.907): loads the ambientCG sets from
        // assets/textures/ground/, or a neutral 1x1 fallback that renders
        // identically to the pre-texture look.
        let ground_textures = ground_textures::load(&device, &queue);
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
            ],
        });

        Self {
            device,
            queue,
            surface,
            config,
            depth_texture,
            depth_view,
            pipeline,
            line_pipeline,
            camera_buffer,
            camera_bind_group,
            lights_buffer,
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
            ssao_strength: 0.55,
            detail_distance: 1.0,
            sea_state: 0.35,
            tree_card_hide_m: 0.0,
            tree_card_far_m: 1500.0,
            aerial_sigma: 0.0,
            aerial_slant_cap: 25_000.0,
            aerial_sky: [0.0, 0.0, 0.0],
            aerial_up: [0.0, 1.0, 0.0],
            bloom_intensity: 0.0, // Off by default; set > 0 to enable
            bloom_threshold: 0.8,
            // Defaults match camera.uniforms()'s former hardcoded sun/fill, so behaviour is unchanged
            // until lights are set (v0.571).
            cur_lights: Vec::new(),
            cur_sun: ([0.3, 1.0, 0.5], [1.0, 0.95, 0.9], 2.5),
            cur_fill: ([-0.5, 0.3, -0.3], [0.4, 0.5, 0.7], 0.6),
            supports_frame_capture,
            albedo_sampler,
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
            ground_textures,
            shadow_comparison_sampler,
            sun_shadows: true,
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

    /// Register a material and return its handle (index).
    /// Uses material_type = 0.0 (default panel grid).
    pub fn add_material(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
    ) -> usize {
        self.add_material_typed(base_color, metallic, roughness, 0.0)
    }

    /// Register a material with an explicit material_type and return its handle (index).
    /// material_type: 0 = default panel grid, 1 = brushed metal, 2 = concrete, 3 = wood.
    /// emissive: 0.0 = no glow, 1.0+ = self-illuminating (sun, lava, neon lights).
    pub fn add_material_typed(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
    ) -> usize {
        self.add_material_full(base_color, metallic, roughness, material_type, 0.0)
    }

    /// Register a material with all parameters including emissive.
    pub fn add_material_full(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
        emissive: f32,
    ) -> usize {
        let uniforms = MaterialUniforms {
            base_color,
            params: [metallic, roughness, material_type, emissive],
        };
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Material Uniform Buffer"),
                contents: bytemuck::bytes_of(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &self.pipeline.material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let idx = self.materials.len();
        self.materials.push(Material {
            base_color,
            metallic,
            roughness,
            emissive,
            buffer,
            bind_group,
            albedo_bind_group: None,
        });
        idx
    }

    /// Build a group-3 bind group for an sRGB RGBA8 image (v0.811, per-pixel
    /// planet imagery). The Srgb format makes sampling return LINEAR values
    /// automatically -- the whole material pipeline is linear; the sRGB
    /// encode happens once, on store to the sRGB render target. The bind
    /// group keeps the texture + view alive internally.
    fn build_albedo_bind_group(&self, rgba: &[u8], width: u32, height: u32) -> wgpu::BindGroup {
        assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "albedo texture byte count must be width*height*4"
        );
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Material Albedo Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Albedo Bind Group"),
            layout: &self.pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.albedo_sampler),
                },
                // Shared cloud-noise volumes (clouds increment 3): every
                // group-3 bind group carries the same engine-global views.
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.cloud_shape_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.cloud_detail_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.cloud_tile_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&self.weather_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&self.shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&self.shadow_comparison_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.shadow_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&self.ground_textures.view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(&self.ground_textures.sampler),
                },
            ],
        })
    }

    /// Upload a fresh live-weather grid (RG8, WEATHER_W x WEATHER_H) into the
    /// persistent weather texture. No bind-group rebuild needed - every group
    /// already references this texture's view.
    pub fn update_weather_map(&self, queue: &wgpu::Queue, rg: &[u8]) {
        let (w, h) = (
            WEATHER_MAP_W,
            WEATHER_MAP_H,
        );
        if rg.len() != (w * h * 2) as usize {
            log::warn!("[Weather] bad grid size {} - ignored", rg.len());
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.weather_map_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rg,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 2),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Register a material that carries a real albedo texture at group 3
    /// (v0.811: per-pixel planet imagery; sRGB RGBA8 bytes, row-major,
    /// row 0 = top). Draws using it bind the texture instead of the white
    /// fallback; everything else about the material behaves like
    /// `add_material_full`.
    pub fn add_textured_material(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
        emissive: f32,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> usize {
        let albedo_bind_group = self.build_albedo_bind_group(rgba, width, height);
        let idx = self.add_material_full(base_color, metallic, roughness, material_type, emissive);
        self.materials[idx].albedo_bind_group = Some(albedo_bind_group);
        idx
    }

    /// Replace the albedo texture of an existing material IN PLACE (v0.811):
    /// hot-reloading a planet's RON re-bakes its imagery, and swapping the
    /// texture on the existing material index keeps VRAM bounded (the old
    /// texture is freed when its bind group drops) and every RenderObject's
    /// material index stable. No-op if idx is out of range.
    pub fn set_material_albedo_texture(
        &mut self,
        idx: usize,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) {
        if idx >= self.materials.len() {
            return;
        }
        let bg = self.build_albedo_bind_group(rgba, width, height);
        self.materials[idx].albedo_bind_group = Some(bg);
    }

    /// Replace the mesh at `idx` in place: drops the old mesh (wgpu frees its vertex/index buffers)
    /// and reuses the slot, so a per-frame editor rebuild (a room drag, a machine move) never leaks
    /// meshes. No-op if idx is out of range. (v0.531: the renderer is otherwise append-only.)
    pub fn replace_mesh(&mut self, idx: usize, mesh: Mesh) {
        if let Some(slot) = self.meshes.get_mut(idx) {
            *slot = mesh;
        }
    }

    /// Update the material at `idx` in place by rewriting its existing uniform buffer (reuses the
    /// buffer + bind group, zero allocation). No-op if idx is out of range. (v0.531)
    pub fn update_material_full(
        &mut self,
        idx: usize,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
        emissive: f32,
    ) {
        if let Some(mat) = self.materials.get(idx) {
            let uniforms = MaterialUniforms {
                base_color,
                params: [metallic, roughness, material_type, emissive],
            };
            self.queue
                .write_buffer(&mat.buffer, 0, bytemuck::bytes_of(&uniforms));
        }
    }

    /// Update the material at `idx` in place (typed convenience; emissive 0). (v0.531)
    pub fn update_material_typed(
        &mut self,
        idx: usize,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
    ) {
        self.update_material_full(idx, base_color, metallic, roughness, material_type, 0.0);
    }

    /// Set room lights for the next render call — UNCAPPED (v0.782). Lights go
    /// to a storage buffer (64 bytes each: pos+intensity, color+range, spot,
    /// cone), which doubles in capacity (recreating the camera bind group) when
    /// exceeded; only `light_count` in the camera uniform bounds the shader
    /// loop. Each light is a point light or a spot with a real cone (v0.639).
    /// There is deliberately no software cap: the practical ceiling is GPU
    /// fill cost, visible in the F2 overlay's live light count + FPS.
    pub fn set_point_lights(&mut self, lights: &[light::RoomLight]) {
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

    /// Whether the swapchain surface was configured with `COPY_SRC`, i.e. whether
    /// `capture_current_frame` can succeed on this backend. (v0.639)
    pub fn supports_frame_capture(&self) -> bool {
        self.supports_frame_capture
    }

    /// Capture `texture` (the swapchain texture of the frame just rendered, BEFORE
    /// `present()`) to a PNG at `path` (v0.639, the live in-game screenshot command). Reuses the
    /// copy-texture-to-buffer-to-PNG technique `ui_snapshots.rs::render_page_png` already uses
    /// for offscreen snapshots, adapted for the live swapchain: the surface format is not
    /// necessarily `Rgba8*` (Windows/DX12 commonly configures `Bgra8UnormSrgb`), so a BGRA
    /// surface has its R/B channels swapped back before the `image` crate (which expects RGBA)
    /// writes the file. Returns a plain error string (not a panic) if this backend's swapchain
    /// doesn't support `COPY_SRC` -- checked once at `init` via `supports_frame_capture`.
    pub fn capture_current_frame(&self, texture: &wgpu::Texture, path: &std::path::Path) -> Result<(), String> {
        if !self.supports_frame_capture {
            return Err("swapchain surface has no COPY_SRC usage on this backend -- frame capture unavailable".to_string());
        }
        let (w, h) = (self.config.width, self.config.height);
        if w == 0 || h == 0 {
            return Err("zero-sized surface -- nothing to capture".to_string());
        }
        self.read_texture_to_png(texture, w, h, path)
    }

    /// Largest texture edge this device supports (v0.810, hi-res screenshot capture).
    /// Queried live from the device limits so the capture path's size clamp never
    /// hardcodes a backend-specific number.
    pub fn max_texture_dimension_2d(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// Create an offscreen color target for a one-frame hi-res capture (v0.810).
    /// Uses the SWAPCHAIN's format so every existing scene pipeline (they were all
    /// built against `surface_format`) renders to it unchanged, plus COPY_SRC for
    /// the PNG readback. Caller renders the normal passes to the returned view,
    /// then hands the texture to `read_texture_to_png`.
    pub fn create_capture_target(&self, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HiRes Capture Target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Recreate the shared DEPTH buffer at an arbitrary size (v0.810). The hi-res
    /// offscreen capture re-runs the normal scene passes, which all bind
    /// `depth_view`, so the depth buffer must match the capture target's size for
    /// that one frame; the caller calls this again with the window size right
    /// after to restore. Deliberately does NOT reconfigure the swapchain (that
    /// belongs to the window) and does not touch scene_texture/bloom (they are
    /// not part of the live frame path).
    pub fn set_depth_target_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let (tex, view) = Self::create_depth_texture(&self.device, width, height);
        self.depth_texture = tex;
        self.depth_view = view;
    }

    /// Read a rendered texture (must have COPY_SRC and the swapchain's format)
    /// back to a PNG at `path` (v0.810; generalized from the v0.639 swapchain
    /// capture so the hi-res offscreen target uses the same proven path). After
    /// writing, the file's header is re-read and its dimensions must match
    /// `width` x `height` exactly, or this returns Err -- a capture that
    /// silently shipped a bad file must never report ok (project lesson).
    pub fn read_texture_to_png(
        &self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let (w, h) = (width, height);
        if w == 0 || h == 0 {
            return Err("zero-sized texture -- nothing to capture".to_string());
        }
        let bytes_per_row = ((w * 4 + 255) / 256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame_capture_readback"),
            size: (bytes_per_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame_capture_encoder"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
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
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * bytes_per_row) as usize;
            let row_bytes = &data[start..start + (w * 4) as usize];
            if bgra {
                for px in row_bytes.chunks_exact(4) {
                    pixels.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                }
            } else {
                pixels.extend_from_slice(row_bytes);
            }
        }
        drop(data);
        buffer.unmap();

        let img = image::RgbaImage::from_raw(w, h, pixels)
            .ok_or_else(|| "captured pixel buffer size mismatch".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        img.save(path).map_err(|e| e.to_string())?;
        // Self-verify the written file (v0.810): decode the PNG header off disk and
        // require the actual dimensions to match the capture request before
        // reporting success. A writer that silently ships nothing (or a truncated
        // file) must surface as an error, never an ok:true.
        let (dw, dh) = image::image_dimensions(path)
            .map_err(|e| format!("wrote {} but could not verify it: {e}", path.display()))?;
        if (dw, dh) != (w, h) {
            return Err(format!(
                "PNG verification failed: requested {w}x{h} but {} decodes as {dw}x{dh}",
                path.display()
            ));
        }
        Ok(())
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
                        material
                            .albedo_bind_group
                            .as_ref()
                            .unwrap_or(&self.default_texture_bind_group),
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
                        material
                            .albedo_bind_group
                            .as_ref()
                            .unwrap_or(&self.default_texture_bind_group),
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
                        material
                            .albedo_bind_group
                            .as_ref()
                            .unwrap_or(&self.default_texture_bind_group),
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
                        material
                            .albedo_bind_group
                            .as_ref()
                            .unwrap_or(&self.default_texture_bind_group),
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
        );
    }

    /// Screen-space ambient occlusion (v0.901): call right after
    /// render_godrays_onto, same celestial slot (depth still holds terrain +
    /// vegetation). Multiplies contact shade into the color target.
    pub fn render_ssao_onto(&self, camera: &Camera, view: &wgpu::TextureView) {
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
        let px_per_rad =
            self.config.height as f32 / camera.fov_degrees.to_radians().max(0.01);
        self.ssao.render(
            &self.device,
            &self.queue,
            &self.depth_view,
            view,
            m[2][2],
            m[3][2],
            px_per_rad,
            1.6,
            self.ssao_strength,
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
        // Camera planet-frame position mod 64 m (v0.902): the precision
        // anchor for sub-8 m micro detail. Poked into light0_cone_inner.yzw.
        ground_anchor: [f32; 3],
        view: &wgpu::TextureView,
    ) {
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
        // Micro-detail anchor in light0_cone_inner.yzw (offset 464 + 4).
        self.queue
            .write_buffer(&self.camera_buffer, 468, bytemuck::cast_slice(&ground_anchor));
        // Detail-distance factor in the view_pos.w pad (offset 64 + 12).
        self.queue
            .write_buffer(&self.camera_buffer, 76, bytemuck::bytes_of(&self.detail_distance));
        // Sea state 0..1 in the fill_color.w pad (offset 656 + 12; the fill
        // light's alpha is never read). 0 = glassy calm, 0.5 = ripples,
        // 1 = storm chop + breaking crests. Fed by the game weather's wind
        // at the player (lib.rs) or the showcase {"sea":x} dev override.
        self.queue
            .write_buffer(&self.camera_buffer, 668, bytemuck::bytes_of(&self.sea_state));
        // Aerial perspective params (v0.916) in the unused per-light cone
        // pads: [1].y sigma (484), [1].z slant cap (488), [2].yzw sky color
        // (500), [3].yzw camera radial up (516). The interior passes'
        // full uniform write zeroes these, so rooms never fog.
        self.queue
            .write_buffer(&self.camera_buffer, 484, bytemuck::bytes_of(&self.aerial_sigma));
        self.queue
            .write_buffer(&self.camera_buffer, 488, bytemuck::bytes_of(&self.aerial_slant_cap));
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
            let sd = [sun_dir.x, sun_dir.y, sun_dir.z, 2.5_f32];
            // w carries the cloud clock (written above at 636); this full
            // vec4 write would stomp it back to a constant otherwise.
            let sc = [1.0_f32, 0.97, 0.92, time_s];
            self.queue.write_buffer(&self.camera_buffer, 608, bytemuck::cast_slice(&sd));
            self.queue.write_buffer(&self.camera_buffer, 624, bytemuck::cast_slice(&sc));
        }

        // ── Sun shadow pass (v0.899) ── near-field ortho depth from the sun,
        // rendered before the main pass so every lit fragment this frame can
        // sample it. Texel-snapped so a drifting camera never swims the map.
        let shadow_on = self.sun_shadows && sun_dir != Vec3::ZERO;
        {
            const SHADOW_MAP_SIZE: f32 = 4096.0;
            let extent = 1500.0_f32;
            let sun = sun_dir.normalize();
            let center = camera.effective_position();
            let up = if sun.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
            let view_m = Mat4::look_at_rh(center + sun * 4000.0, center, up);
            let proj = Mat4::orthographic_rh(-extent, extent, -extent, extent, 0.1, 8000.0);
            let mut vp = proj * view_m;
            // Texel snap: shift so the world origin lands on a texel grid.
            let ndc_texel = 2.0 / SHADOW_MAP_SIZE;
            let origin = vp * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
            let snap = |v: f32| (v / ndc_texel).round() * ndc_texel - v;
            vp = Mat4::from_translation(Vec3::new(snap(origin.x), snap(origin.y), 0.0)) * vp;
            let mut light_u = <camera::CameraUniforms as bytemuck::Zeroable>::zeroed();
            light_u.view_proj = vp.to_cols_array_2d();
            self.queue
                .write_buffer(&self.light_camera_buffer, 0, bytemuck::bytes_of(&light_u));
            let mut su = [0.0_f32; 24];
            su[..16].copy_from_slice(&vp.to_cols_array());
            su[16] = if shadow_on { 1.0 } else { 0.0 };
            su[17] = 0.6; // shadow strength
            su[18] = 1.0 / SHADOW_MAP_SIZE;
            // params.w (v0.912): the tree-model radius - terrain tree CARDS
            // hide inside it so the real 3D conifers replace them cleanly.
            su[19] = self.tree_card_hide_m;
            // params2.x (v0.924): tree-card far cutoff (vegetation LOD slider).
            su[20] = self.tree_card_far_m.max(1.0);
            self.queue
                .write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::cast_slice(&su));
        }
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
                pass.set_bind_group(0, &self.light_camera_bind_group, &[]);
                // ONE group-3 for the whole pass: the dummy-depth variant
                // (the real shadow map is this pass's write target and must
                // not also be bound for sampling). vs_main samples nothing
                // from group 3, so the contents are irrelevant.
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
                    let dynamic_offset = (uniform_align as u32) * (i as u32);
                    pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                    if bound_material != obj.material {
                        bound_material = obj.material;
                        pass.set_bind_group(2, &material.bind_group, &[]);
                    }
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
            self.queue.submit(std::iter::once(senc.finish()));
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Celestial Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Celestial Pass"),
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
                        material
                            .albedo_bind_group
                            .as_ref()
                            .unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }

            // Atmosphere shells etc.: alpha-blended over the bodies, depth-TESTED
            // against them (no depth write), so the back hemisphere of a shell is
            // hidden by its own planet while the limb halo survives. Few and far
            // apart, so no depth sorting needed. (v0.763)
            if !transparent.is_empty() {
                render_pass.set_pipeline(&self.pipeline.transparent_pipeline);
                let mut bound_material = usize::MAX;
                for (i, obj) in transparent.iter().enumerate() {
                    let slot = objects.len() + i;
                    if slot >= MAX_OBJECTS { break; }
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
                        // loop.
                        render_pass.set_bind_group(
                            3,
                            material
                                .albedo_bind_group
                                .as_ref()
                                .unwrap_or(&self.default_texture_bind_group),
                            &[],
                        );
                    }
                    render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
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
                        material
                            .albedo_bind_group
                            .as_ref()
                            .unwrap_or(&self.default_texture_bind_group),
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
