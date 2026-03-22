//! Renderer — wgpu device/surface setup and render loop.
//!
//! Configuration loaded from `config/renderer.toml`.
//! Supports both native (winit window) and WASM (canvas) targets.

pub mod camera;
pub mod mesh;
pub mod multi_scale;
pub mod pipeline;
pub mod shader_loader;

use camera::{Camera, CameraUniforms};
use glam::{Mat4, Quat, Vec3};
use mesh::Mesh;
use pipeline::{MaterialUniforms, ObjectUniforms, Pipeline};
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
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
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
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    // Registered meshes and materials
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

impl Renderer {
    /// Create a new renderer attached to a native winit window.
    #[cfg(feature = "native")]
    pub async fn new_native(window: std::sync::Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
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

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("HumanityOS Renderer"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
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

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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

        // Shader + pipeline
        let shader_loader = shader_loader::ShaderLoader::new();
        let shader = shader_loader.load_embedded_pbr(&device);
        let pipeline = Pipeline::new(&device, surface_format, &shader);

        // Camera uniform buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&CameraUniforms {
                view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                view_pos: [0.0; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &pipeline.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
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
            camera_buffer,
            camera_bind_group,
            meshes: Vec::new(),
            materials: Vec::new(),
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
    }

    /// Current surface aspect ratio.
    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    /// Register a mesh and return its handle (index).
    pub fn add_mesh(&mut self, mesh: Mesh) -> usize {
        let idx = self.meshes.len();
        self.meshes.push(mesh);
        idx
    }

    /// Register a material and return its handle (index).
    pub fn add_material(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
    ) -> usize {
        let uniforms = MaterialUniforms {
            base_color,
            params: [metallic, roughness, 0.0, 0.0],
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
            buffer,
            bind_group,
        });
        idx
    }

    /// Render a frame with the given camera and objects.
    pub fn render(&self, camera: &Camera, objects: &[RenderObject]) -> Result<(), wgpu::SurfaceError> {
        // Update camera uniforms
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&camera.uniforms()),
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
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            for obj in objects {
                let mesh = match self.meshes.get(obj.mesh) {
                    Some(m) => m,
                    None => continue,
                };
                let material = match self.materials.get(obj.material) {
                    Some(m) => m,
                    None => continue,
                };

                // Build model matrix
                let model = Mat4::from_scale_rotation_translation(
                    obj.scale,
                    obj.rotation,
                    obj.position,
                );
                // Normal matrix = transpose(inverse(model)) — for uniform scaling
                // we can use the model matrix directly, but let's be correct
                let normal_matrix = model.inverse().transpose();

                let object_uniforms = ObjectUniforms {
                    model: model.to_cols_array_2d(),
                    normal_matrix: normal_matrix.to_cols_array_2d(),
                };

                // Create per-object bind group (temporary — fine for small scenes)
                let object_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Object Uniform Buffer"),
                            contents: bytemuck::bytes_of(&object_uniforms),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });
                let object_bind_group =
                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Object Bind Group"),
                        layout: &self.pipeline.object_bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: object_buffer.as_entire_binding(),
                        }],
                    });

                render_pass.set_bind_group(1, &object_bind_group, &[]);
                render_pass.set_bind_group(2, &material.bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
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
