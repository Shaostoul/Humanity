use bytemuck::{Pod, Zeroable};
use core_offline_loop::{apply_command, Command, WorldSnapshot};
use glam::{Mat4, Vec3};
use std::collections::HashSet;
use std::f32::consts::PI;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;
use wgpu::SurfaceError;
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,

    pipeline: wgpu::RenderPipeline,
    depth_view: wgpu::TextureView,

    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    terrain: Mesh,
    capsule_base_positions: Vec<Vec3>,
    capsule_colors: Vec<[f32; 3]>,
    capsule_indices: Vec<u32>,
    capsule_vertex_buffer: wgpu::Buffer,
    capsule_index_buffer: wgpu::Buffer,

    world: WorldSnapshot,
    pressed: HashSet<KeyCode>,
    last_update: Instant,
    start_time: Instant,

    yaw: f32,
    pitch: f32,
    menu_open: bool,
    status_msg: String,
    status_until: Instant,
}

impl<'a> State<'a> {
    async fn new(window: &'a winit::window::Window) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).expect("create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("request adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("engine-shell-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("request device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_view = create_depth_view(&device, config.width, config.height);

        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("basic-3d-shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct Camera {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: Camera;

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) color: vec3<f32>) -> VSOut {
    var out: VSOut;
    out.pos = camera.view_proj * vec4<f32>(position, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(@location(0) color: vec3<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(color, 1.0);
}
"#
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline-layout"),
            bind_group_layouts: &[&camera_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let (terrain_vertices, terrain_indices) = build_terrain_mesh(160, 180.0);
        let terrain = Mesh {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terrain-vb"),
                contents: bytemuck::cast_slice(&terrain_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terrain-ib"),
                contents: bytemuck::cast_slice(&terrain_indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: terrain_indices.len() as u32,
        };

        let (capsule_pos, capsule_colors, capsule_indices) = build_capsule_mesh(0.35, 0.95, 20, 12);
        let placeholder_vertices = vec![
            Vertex {
                pos: [0.0, 0.0, 0.0],
                color: [0.8, 0.8, 0.9],
            };
            capsule_pos.len()
        ];

        let capsule_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("capsule-vb"),
            contents: bytemuck::cast_slice(&placeholder_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let capsule_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("capsule-ib"),
            contents: bytemuck::cast_slice(&capsule_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut world = WorldSnapshot::new_default();
        world.controller.position.x = -60.0;
        world.controller.position.y = 0.0;
        world.controller.position.z = -50.0;

        Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            depth_view,
            camera_buffer,
            camera_bind_group,
            terrain,
            capsule_base_positions: capsule_pos,
            capsule_colors,
            capsule_indices,
            capsule_vertex_buffer,
            capsule_index_buffer,
            world,
            pressed: HashSet::new(),
            last_update: Instant::now(),
            start_time: Instant::now(),
            yaw: -0.6,
            pitch: -0.25,
            menu_open: false,
            status_msg: "ESC menu: H help, I inventory, O objective".to_string(),
            status_until: Instant::now() + Duration::from_secs(5),
        }
    }

    fn set_status(&mut self, text: impl Into<String>) {
        self.status_msg = text.into();
        self.status_until = Instant::now() + Duration::from_secs(4);
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = create_depth_view(&self.device, self.config.width, self.config.height);
        }
    }

    fn run_game_command(&mut self, cmd: Command) {
        match apply_command(&mut self.world, cmd) {
            Ok(msg) => self.set_status(msg),
            Err(err) => self.set_status(format!("error: {err}")),
        }
    }

    fn key_down(&self, code: KeyCode) -> bool {
        self.pressed.contains(&code)
    }

    fn handle_input(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if let PhysicalKey::Code(code) = event.physical_key {
                match event.state {
                    ElementState::Pressed => {
                        self.pressed.insert(code);

                        if code == KeyCode::Escape {
                            self.menu_open = !self.menu_open;
                            self.set_status(if self.menu_open {
                                "MENU OPEN: H help, I inventory, O objective, ESC close"
                            } else {
                                "MENU CLOSED"
                            });
                            return true;
                        }

                        if self.menu_open {
                            match code {
                                KeyCode::KeyI => self.run_game_command(Command::Inventory),
                                KeyCode::KeyO => self.run_game_command(Command::Objective),
                                KeyCode::KeyH => self.set_status("Menu: I inventory, O objective, ESC close"),
                                _ => {}
                            }
                            return true;
                        }

                        match code {
                            KeyCode::Digit1 => self.run_game_command(Command::Gather("wood".to_string())),
                            KeyCode::Digit2 => self.run_game_command(Command::Gather("fiber".to_string())),
                            KeyCode::Digit3 => self.run_game_command(Command::Gather("scrap".to_string())),
                            KeyCode::Digit4 => self.run_game_command(Command::CraftFilter),
                            KeyCode::Digit5 => self.run_game_command(Command::TreatWater),
                            KeyCode::Digit6 => self.run_game_command(Command::FarmTick),
                            KeyCode::Digit7 => self.run_game_command(Command::Eat),
                            KeyCode::KeyI => self.run_game_command(Command::Inventory),
                            KeyCode::KeyO => self.run_game_command(Command::Objective),
                            _ => {}
                        }
                    }
                    ElementState::Released => {
                        self.pressed.remove(&code);
                    }
                }
                return true;
            }
        }
        false
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_update).as_secs_f32().clamp(0.0, 0.05);
        self.last_update = now;

        if !self.menu_open {
            let sprint = self.key_down(KeyCode::ShiftLeft);
            let speed = if sprint { 14.0 } else { 8.0 };
            let forward = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos()).normalize_or_zero();
            let right = Vec3::new(forward.z, 0.0, -forward.x).normalize_or_zero();

            let mut delta = Vec3::ZERO;
            if self.key_down(KeyCode::KeyW) || self.key_down(KeyCode::ArrowUp) {
                delta += forward;
            }
            if self.key_down(KeyCode::KeyS) || self.key_down(KeyCode::ArrowDown) {
                delta -= forward;
            }
            if self.key_down(KeyCode::KeyA) || self.key_down(KeyCode::ArrowLeft) {
                delta -= right;
            }
            if self.key_down(KeyCode::KeyD) || self.key_down(KeyCode::ArrowRight) {
                delta += right;
            }
            if delta.length_squared() > 0.0 {
                delta = delta.normalize() * speed * dt;
                let mut p = vec3_from_world(&self.world);
                p += delta;
                self.world.controller.position.x = p.x;
                self.world.controller.position.z = p.z;
                self.world.player_pos.x = p.x.round() as i32;
                self.world.player_pos.y = p.z.round() as i32;
                if sprint {
                    self.world.controller.stamina = (self.world.controller.stamina - 12.0 * dt).clamp(0.0, 100.0);
                } else {
                    self.world.controller.stamina = (self.world.controller.stamina + 5.0 * dt).clamp(0.0, 100.0);
                }
            }
        }

        // stick player to terrain
        let p = vec3_from_world(&self.world);
        let ground = terrain_height(p.x, p.z);
        self.world.controller.position.y = ground + 1.25;

        let cam_target = vec3_from_world(&self.world) + Vec3::new(0.0, 0.7, 0.0);
        let forward = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        )
        .normalize_or_zero();
        let cam_pos = cam_target - forward * 5.0 + Vec3::new(0.0, 2.0, 0.0);

        let view = Mat4::look_at_rh(cam_pos, cam_target, Vec3::Y);
        let mut proj = Mat4::perspective_rh_gl(
            60.0f32.to_radians(),
            (self.config.width as f32 / self.config.height.max(1) as f32).max(0.01),
            0.1,
            1000.0,
        );
        proj.y_axis.y *= -1.0;
        let view_proj = proj * view;

        let uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));

        let player = vec3_from_world(&self.world);
        let transformed_capsule = self
            .capsule_base_positions
            .iter()
            .zip(self.capsule_colors.iter())
            .map(|(bp, c)| Vertex {
                pos: [bp.x + player.x, bp.y + player.y, bp.z + player.z],
                color: *c,
            })
            .collect::<Vec<_>>();
        self.queue.write_buffer(
            &self.capsule_vertex_buffer,
            0,
            bytemuck::cast_slice(&transformed_capsule),
        );
    }

    fn title_text(&self) -> String {
        let msg = if Instant::now() <= self.status_until {
            self.status_msg.as_str()
        } else if self.menu_open {
            "MENU OPEN (I inventory, O objective, ESC close)"
        } else {
            "1 wood,2 fiber,3 scrap,4 craft,5 purify,6 farm,7 eat"
        };

        format!(
            "Humanity Shell 3D | EASY | pos=({:.1},{:.1},{:.1}) yaw={:.1} pitch={:.1} stamina={:.0} inv[w:{} f:{} s:{} k:{} food:{}] obj[{}/{}/{}] | {}",
            self.world.controller.position.x,
            self.world.controller.position.y,
            self.world.controller.position.z,
            self.yaw,
            self.pitch,
            self.world.controller.stamina,
            self.world.inventory.wood,
            self.world.inventory.fiber,
            self.world.inventory.scrap,
            self.world.inventory.filter_kits,
            self.world.inventory.food_rations,
            self.world.milestones.crafted_filter,
            self.world.milestones.purified_water,
            self.world.milestones.planted_cycle,
            msg
        )
    }

    fn render(&mut self) -> Result<(), SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let day_phase = (self.start_time.elapsed().as_secs_f32() / 120.0) % 1.0;
        let d = ((day_phase * PI * 2.0).sin() + 1.0) * 0.5;
        let clear = wgpu::Color {
            r: (0.02 + 0.18 * d as f64).clamp(0.0, 1.0),
            g: (0.03 + 0.22 * d as f64).clamp(0.0, 1.0),
            b: (0.07 + 0.45 * d as f64).clamp(0.0, 1.0),
            a: 1.0,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("render-encoder") });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
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
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);

            rpass.set_vertex_buffer(0, self.terrain.vertex_buffer.slice(..));
            rpass.set_index_buffer(self.terrain.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..self.terrain.index_count, 0, 0..1);

            rpass.set_vertex_buffer(0, self.capsule_vertex_buffer.slice(..));
            rpass.set_index_buffer(self.capsule_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..self.capsule_indices.len() as u32, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

fn vec3_from_world(world: &WorldSnapshot) -> Vec3 {
    Vec3::new(
        world.controller.position.x,
        world.controller.position.y,
        world.controller.position.z,
    )
}

fn terrain_height(x: f32, z: f32) -> f32 {
    // procedural "Rainier-like" volcano + ridges
    let cx = 35.0;
    let cz = 45.0;
    let dx = x - cx;
    let dz = z - cz;
    let r = (dx * dx + dz * dz).sqrt();

    let peak = 58.0 * (-(r * r) / (2.0 * 28.0 * 28.0)).exp();
    let shoulder = 12.0 * (-(r * r) / (2.0 * 65.0 * 65.0)).exp();
    let ridges = ((x * 0.08).sin() * 2.0 + (z * 0.06).cos() * 1.6) * 0.8;
    let valley = -0.015 * (z + 80.0).max(0.0);

    peak + shoulder + ridges + valley
}

fn terrain_color(h: f32) -> [f32; 3] {
    if h > 42.0 {
        [0.92, 0.94, 0.96] // snow
    } else if h > 26.0 {
        [0.42, 0.43, 0.45] // rock
    } else if h > 12.0 {
        [0.29, 0.40, 0.25] // alpine
    } else {
        [0.22, 0.48, 0.24] // meadow
    }
}

fn build_terrain_mesh(resolution: u32, span: f32) -> (Vec<Vertex>, Vec<u32>) {
    let n = resolution.max(8);
    let step = span / (n - 1) as f32;
    let half = span * 0.5;

    let mut vertices = Vec::with_capacity((n * n) as usize);
    for z in 0..n {
        for x in 0..n {
            let wx = -half + x as f32 * step;
            let wz = -half + z as f32 * step;
            let h = terrain_height(wx, wz);
            vertices.push(Vertex {
                pos: [wx, h, wz],
                color: terrain_color(h),
            });
        }
    }

    let mut indices = Vec::with_capacity(((n - 1) * (n - 1) * 6) as usize);
    for z in 0..(n - 1) {
        for x in 0..(n - 1) {
            let i0 = z * n + x;
            let i1 = i0 + 1;
            let i2 = i0 + n;
            let i3 = i2 + 1;

            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    (vertices, indices)
}

fn build_capsule_mesh(radius: f32, half_height: f32, segments: u32, rings: u32) -> (Vec<Vec3>, Vec<[f32; 3]>, Vec<u32>) {
    let seg = segments.max(8);
    let ring = rings.max(6);

    // start from sphere-ish rings, then stretch Y to pill/capsule look
    let mut pos = Vec::new();
    let mut col = Vec::new();
    let mut indices = Vec::new();

    for y in 0..=ring {
        let v = y as f32 / ring as f32;
        let phi = v * PI;
        let py = phi.cos() * radius;
        let pr = phi.sin() * radius;

        for x in 0..=seg {
            let u = x as f32 / seg as f32;
            let th = u * PI * 2.0;
            let px = th.cos() * pr;
            let pz = th.sin() * pr;

            let stretched_y = py * 1.8 + if py >= 0.0 { half_height } else { -half_height };
            pos.push(Vec3::new(px, stretched_y, pz));
            col.push([0.90, 0.86, 0.78]);
        }
    }

    let row = seg + 1;
    for y in 0..ring {
        for x in 0..seg {
            let i0 = y * row + x;
            let i1 = i0 + 1;
            let i2 = i0 + row;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    (pos, col, indices)
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth-tex"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    depth_tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("event loop");
    let window = WindowBuilder::new()
        .with_title("Humanity Shell 3D")
        .build(&event_loop)
        .expect("window");

    let mut state = pollster::block_on(State::new(&window));

    let _ = event_loop.run(|event, target| match event {
        Event::WindowEvent { event, window_id } if window_id == window.id() => {
            if !state.handle_input(&event) {
                match event {
                    WindowEvent::CloseRequested => target.exit(),
                    WindowEvent::Resized(size) => state.resize(size),
                    WindowEvent::RedrawRequested => {
                        state.update();
                        window.set_title(&state.title_text());
                        match state.render() {
                            Ok(_) => {}
                            Err(SurfaceError::Lost) => state.resize(state.size),
                            Err(SurfaceError::OutOfMemory) => target.exit(),
                            Err(_) => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion { delta },
            ..
        } => {
            if !state.menu_open {
                let sensitivity = 0.0025;
                state.yaw -= delta.0 as f32 * sensitivity;
                state.pitch = (state.pitch - delta.1 as f32 * sensitivity).clamp(-1.25, 1.25);
            }
        }
        Event::AboutToWait => window.request_redraw(),
        _ => {}
    });
}
