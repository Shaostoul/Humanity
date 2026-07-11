//! Renderer — wgpu device/surface setup and render loop.
//!
//! Configuration loaded from `config/renderer.toml`.
//! Supports both native (winit window) and WASM (canvas) targets.

pub mod atmosphere;
pub mod bloom;
pub mod camera;
pub mod clouds;
pub mod floating_origin;
pub mod hologram;
pub mod light;
pub mod line;
pub mod mesh;
pub mod multi_scale;
pub mod particles;
pub mod pipeline;
pub mod shader_loader;
pub mod stars;

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
const MAX_OBJECTS: usize = 1024;
use wgpu::util::DeviceExt;

/// Describes one object to render in the scene.
#[derive(Clone)]
pub struct RenderObject {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub mesh: usize,     // index into Renderer::meshes
    pub material: usize, // index into Renderer::materials
}

/// Material properties for PBR-lite rendering.
pub struct Material {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: f32,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
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
}

impl Renderer {
    /// Create a new renderer attached to a native winit window.
    #[cfg(feature = "native")]
    pub async fn new_native(window: std::sync::Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

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

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let surface = instance.create_surface(window).expect("Failed to create surface");

        Self::init(instance, surface, width, height).await
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

        Self::init(instance, surface, width, height).await
    }

    /// Shared initialization: adapter, device, pipeline, depth buffer.
    async fn init(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
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
            bloom_intensity: 0.0, // Off by default; set > 0 to enable
            bloom_threshold: 0.8,
            // Defaults match camera.uniforms()'s former hardcoded sun/fill, so behaviour is unchanged
            // until lights are set (v0.571).
            cur_lights: Vec::new(),
            cur_sun: ([0.3, 1.0, 0.5], [1.0, 0.95, 0.9], 2.5),
            cur_fill: ([-0.5, 0.3, -0.3], [0.4, 0.5, 0.7], 0.6),
            supports_frame_capture,
        }
    }

    /// Handle window/canvas resize.
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
        });
        idx
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
        Ok(())
    }

    /// Render a frame with the given camera and objects.
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

            // Upload all object uniforms to the dynamic buffer BEFORE the render pass
            let uniform_align = 256_u64;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; } // MAX_OBJECTS
                let model = Mat4::from_scale_rotation_translation(
                    obj.scale,
                    obj.rotation,
                    obj.position,
                );
                let normal_matrix = model.inverse().transpose();
                let uniforms = ObjectUniforms {
                    model: model.to_cols_array_2d(),
                    normal_matrix: normal_matrix.to_cols_array_2d(),
                };
                self.queue.write_buffer(
                    &self.object_buffer,
                    uniform_align * i as u64,
                    bytemuck::bytes_of(&uniforms),
                );
            }

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
                render_pass.set_bind_group(2, &material.bind_group, &[]);
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

            // Upload all object uniforms to the dynamic buffer BEFORE the render pass
            let uniform_align = 256_u64;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let model = Mat4::from_scale_rotation_translation(
                    obj.scale,
                    obj.rotation,
                    obj.position,
                );
                let normal_matrix = model.inverse().transpose();
                let uniforms = ObjectUniforms {
                    model: model.to_cols_array_2d(),
                    normal_matrix: normal_matrix.to_cols_array_2d(),
                };
                self.queue.write_buffer(
                    &self.object_buffer,
                    uniform_align * i as u64,
                    bytemuck::bytes_of(&uniforms),
                );
            }

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
                render_pass.set_bind_group(2, &material.bind_group, &[]);
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

            let uniform_align = 256_u64;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let model = Mat4::from_scale_rotation_translation(obj.scale, obj.rotation, obj.position);
                let normal_matrix = model.inverse().transpose();
                let uniforms = ObjectUniforms {
                    model: model.to_cols_array_2d(),
                    normal_matrix: normal_matrix.to_cols_array_2d(),
                };
                self.queue.write_buffer(&self.object_buffer, uniform_align * i as u64, bytemuck::bytes_of(&uniforms));
            }

            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                render_pass.set_bind_group(2, &material.bind_group, &[]);
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
            let uniform_align = 256_u64;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let model = Mat4::from_scale_rotation_translation(obj.scale, obj.rotation, obj.position);
                let normal_matrix = model.inverse().transpose();
                let uniforms = ObjectUniforms {
                    model: model.to_cols_array_2d(),
                    normal_matrix: normal_matrix.to_cols_array_2d(),
                };
                self.queue.write_buffer(&self.object_buffer, uniform_align * i as u64, bytemuck::bytes_of(&uniforms));
            }
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                render_pass.set_bind_group(2, &material.bind_group, &[]);
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
    pub fn render_celestial_onto(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
        transparent: &[RenderObject],
        sun_dir: Vec3,
        time_s: f32,
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
            let uniform_align = 256_u64;
            for (i, obj) in objects.iter().chain(transparent.iter()).enumerate() {
                if i >= MAX_OBJECTS { break; }
                let model = Mat4::from_scale_rotation_translation(obj.scale, obj.rotation, obj.position);
                let normal_matrix = model.inverse().transpose();
                let uniforms = ObjectUniforms {
                    model: model.to_cols_array_2d(),
                    normal_matrix: normal_matrix.to_cols_array_2d(),
                };
                self.queue.write_buffer(&self.object_buffer, uniform_align * i as u64, bytemuck::bytes_of(&uniforms));
            }

            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                render_pass.set_bind_group(2, &material.bind_group, &[]);
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
                for (i, obj) in transparent.iter().enumerate() {
                    let slot = objects.len() + i;
                    if slot >= MAX_OBJECTS { break; }
                    let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                    let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                    let dynamic_offset = (uniform_align as u32) * (slot as u32);
                    render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
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

            for batch in batches {
                let mesh = match self.meshes.get(batch.mesh) {
                    Some(m) => m,
                    None => continue,
                };
                let material = match self.materials.get(batch.material) {
                    Some(m) => m,
                    None => continue,
                };

                render_pass.set_bind_group(2, &material.bind_group, &[]);
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
