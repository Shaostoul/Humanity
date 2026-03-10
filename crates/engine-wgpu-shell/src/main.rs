use bytemuck::{Pod, Zeroable};
use core_firstperson_controller::{apply_look, apply_move, ControllerInput, MoveDir};
use core_offline_loop::{apply_command, Command, WorldSnapshot};
use std::collections::HashSet;
use std::time::Instant;
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
    pos: [f32; 2],
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
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    world: WorldSnapshot,
    pressed: HashSet<KeyCode>,
    last_update: Instant,
    start_time: Instant,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
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
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);

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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("basic-shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) color: vec3<f32>) -> VSOut {
    var out: VSOut;
    out.pos = vec4<f32>(position, 0.0, 1.0);
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
            bind_group_layouts: &[],
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
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex-buffer"),
            contents: &[0u8; 4],
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            world: WorldSnapshot::new_default(),
            pressed: HashSet::new(),
            last_update: Instant::now(),
            start_time: Instant::now(),
            pipeline,
            vertex_buffer,
            num_vertices: 0,
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn handle_input(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if let PhysicalKey::Code(code) = event.physical_key {
                match event.state {
                    ElementState::Pressed => {
                        self.pressed.insert(code);
                        match code {
                            KeyCode::KeyE => {
                                let _ = apply_command(&mut self.world, Command::Gather("wood".to_string()));
                            }
                            KeyCode::KeyQ => {
                                let _ = apply_command(&mut self.world, Command::Gather("fiber".to_string()));
                            }
                            KeyCode::KeyZ => {
                                let _ = apply_command(&mut self.world, Command::Gather("scrap".to_string()));
                            }
                            KeyCode::KeyR => {
                                let _ = apply_command(&mut self.world, Command::CraftFilter);
                            }
                            KeyCode::KeyT => {
                                let _ = apply_command(&mut self.world, Command::TreatWater);
                            }
                            KeyCode::KeyF => {
                                let _ = apply_command(&mut self.world, Command::FarmTick);
                            }
                            KeyCode::KeyC => {
                                let _ = apply_command(&mut self.world, Command::Eat);
                            }
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
        let dt = (now - self.last_update).as_secs_f32().clamp(0.0, 0.1);
        self.last_update = now;

        if self.pressed.contains(&KeyCode::KeyW) {
            apply_move(
                &mut self.world.controller,
                ControllerInput {
                    dir: MoveDir::Forward,
                    dt_seconds: dt,
                    sprint: self.pressed.contains(&KeyCode::ShiftLeft),
                },
            );
        }
        if self.pressed.contains(&KeyCode::KeyS) {
            apply_move(
                &mut self.world.controller,
                ControllerInput {
                    dir: MoveDir::Backward,
                    dt_seconds: dt,
                    sprint: self.pressed.contains(&KeyCode::ShiftLeft),
                },
            );
        }
        if self.pressed.contains(&KeyCode::KeyA) {
            apply_move(
                &mut self.world.controller,
                ControllerInput {
                    dir: MoveDir::Left,
                    dt_seconds: dt,
                    sprint: self.pressed.contains(&KeyCode::ShiftLeft),
                },
            );
        }
        if self.pressed.contains(&KeyCode::KeyD) {
            apply_move(
                &mut self.world.controller,
                ControllerInput {
                    dir: MoveDir::Right,
                    dt_seconds: dt,
                    sprint: self.pressed.contains(&KeyCode::ShiftLeft),
                },
            );
        }

        self.world.player_pos.x = self.world.controller.position.x.round() as i32;
        self.world.player_pos.y = self.world.controller.position.z.round() as i32;

        let vertices = build_scene_vertices(&self.world, self.start_time.elapsed().as_secs_f32(), self.size);
        self.num_vertices = vertices.len() as u32;
        let bytes = bytemuck::cast_slice(&vertices);

        if self.vertex_buffer.size() < bytes.len() as u64 {
            self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vertex-buffer-grow"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue.write_buffer(&self.vertex_buffer, 0, bytes);
    }

    fn title_text(&self) -> String {
        format!(
            "Humanity Shell | EASY | pos=({},{}) stamina={:.0} water={:.0} contam={:.1} inv[w:{} f:{} s:{} k:{} food:{}] milestones[{}/{}/{}]",
            self.world.player_pos.x,
            self.world.player_pos.y,
            self.world.controller.stamina,
            self.world.water.liters,
            self.world.water.quality.contamination_index,
            self.world.inventory.wood,
            self.world.inventory.fiber,
            self.world.inventory.scrap,
            self.world.inventory.filter_kits,
            self.world.inventory.food_rations,
            self.world.milestones.crafted_filter,
            self.world.milestones.purified_water,
            self.world.milestones.planted_cycle,
        )
    }

    fn render(&mut self) -> Result<(), SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.draw(0..self.num_vertices, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

fn add_rect(v: &mut Vec<Vertex>, x0: f32, y0: f32, x1: f32, y1: f32, c0: [f32; 3], c1: [f32; 3]) {
    v.push(Vertex { pos: [x0, y0], color: c0 });
    v.push(Vertex { pos: [x1, y0], color: c0 });
    v.push(Vertex { pos: [x1, y1], color: c1 });

    v.push(Vertex { pos: [x0, y0], color: c0 });
    v.push(Vertex { pos: [x1, y1], color: c1 });
    v.push(Vertex { pos: [x0, y1], color: c1 });
}

fn add_triangle(v: &mut Vec<Vertex>, a: [f32; 2], b: [f32; 2], c: [f32; 2], col: [f32; 3]) {
    v.push(Vertex { pos: a, color: col });
    v.push(Vertex { pos: b, color: col });
    v.push(Vertex { pos: c, color: col });
}

fn add_diamond(v: &mut Vec<Vertex>, cx: f32, cy: f32, sx: f32, sy: f32, col: [f32; 3]) {
    add_triangle(v, [cx, cy + sy], [cx + sx, cy], [cx, cy - sy], col);
    add_triangle(v, [cx, cy + sy], [cx, cy - sy], [cx - sx, cy], col);
}

fn build_scene_vertices(world: &WorldSnapshot, t: f32, size: PhysicalSize<u32>) -> Vec<Vertex> {
    let mut v = Vec::new();

    let day_phase = (t / 90.0) % 1.0; // 90s day/night loop
    let sun_angle = day_phase * std::f32::consts::TAU;
    let day_light = ((sun_angle.sin() + 1.0) * 0.5).clamp(0.0, 1.0);
    let night = 1.0 - day_light;

    let sky_top = [
        (0.03 + 0.35 * day_light) as f32,
        (0.05 + 0.45 * day_light) as f32,
        (0.10 + 0.60 * day_light) as f32,
    ];
    let sky_horizon = [
        (0.10 + 0.50 * day_light) as f32,
        (0.08 + 0.35 * day_light) as f32,
        (0.12 + 0.25 * day_light) as f32,
    ];

    add_rect(&mut v, -1.0, -1.0, 1.0, 1.0, sky_horizon, sky_top);

    // Ground (mountain meadow)
    let ground_col = [0.06 + day_light * 0.20, 0.14 + day_light * 0.35, 0.07 + day_light * 0.18];
    add_rect(&mut v, -1.0, -1.0, 1.0, -0.25, ground_col, [ground_col[0] * 0.7, ground_col[1] * 0.7, ground_col[2] * 0.7]);

    // Mountain silhouette
    let mcol = [0.10 + 0.08 * day_light, 0.12 + 0.10 * day_light, 0.16 + 0.10 * day_light];
    add_triangle(&mut v, [-0.95, -0.25], [-0.50, 0.35], [-0.05, -0.25], mcol);
    add_triangle(&mut v, [-0.35, -0.25], [0.05, 0.15], [0.45, -0.25], [mcol[0]*0.9,mcol[1]*0.9,mcol[2]*0.9]);
    add_triangle(&mut v, [0.20, -0.25], [0.70, 0.28], [1.00, -0.25], [mcol[0]*0.8,mcol[1]*0.8,mcol[2]*0.8]);

    // Sun / moon
    let sx = sun_angle.cos() * 0.55;
    let sy = sun_angle.sin() * 0.35 + 0.2;
    if day_light > 0.2 {
        add_diamond(&mut v, sx, sy, 0.04, 0.04, [1.0, 0.82, 0.35]);
    } else {
        add_diamond(&mut v, -sx, -sy + 0.2, 0.03, 0.03, [0.85, 0.90, 1.0]);
    }

    // Sparse clouds
    for i in 0..4 {
        let px = -0.8 + i as f32 * 0.55 + (t * 0.01 + i as f32).sin() * 0.04;
        let py = 0.55 + (i as f32 * 0.13).sin() * 0.06;
        let c = [0.75 + day_light * 0.2, 0.76 + day_light * 0.2, 0.80 + day_light * 0.2];
        add_diamond(&mut v, px, py, 0.07, 0.03, c);
    }

    // Constellation-like stars at night
    if night > 0.35 {
        let star_col = [0.7 + 0.3 * night, 0.75 + 0.25 * night, 0.95];
        let stars = [
            (-0.72, 0.62), (-0.66, 0.68), (-0.61, 0.64),
            (0.50, 0.71), (0.58, 0.76), (0.66, 0.70),
            (-0.08, 0.82), (0.02, 0.86), (0.11, 0.81),
        ];
        for (x, y) in stars {
            let tw = 0.005 + 0.003 * (t * 2.0 + x * 7.0).sin().abs();
            add_diamond(&mut v, x, y, tw, tw, star_col);
        }
    }

    // Blossoming orchard dots with breeze sway
    for i in 0..24 {
        let bx = -0.95 + i as f32 * 0.08;
        let sway = (t * 1.8 + i as f32 * 0.5).sin() * 0.01;
        let by = -0.35 + (i as f32 * 0.31).cos() * 0.02 + sway;
        add_diamond(&mut v, bx, by, 0.01, 0.01, [0.85, 0.55, 0.78]);
    }

    // Player position marker on horizon for reference
    let px = ((world.player_pos.x as f32) * 0.03).clamp(-0.95, 0.95);
    add_diamond(&mut v, px, -0.20, 0.015, 0.02, [0.95, 0.95, 0.2]);

    // 8px green diamond crosshair in center
    let sx = (8.0 / size.width.max(1) as f32).clamp(0.002, 0.03);
    let sy = (8.0 / size.height.max(1) as f32).clamp(0.002, 0.03);
    add_diamond(&mut v, 0.0, 0.0, sx, sy, [0.1, 1.0, 0.2]);

    v
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("event loop");
    let window = WindowBuilder::new()
        .with_title("Humanity Engine Shell")
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
            let sensitivity = 0.08;
            apply_look(
                &mut state.world.controller,
                delta.0 as f32 * sensitivity,
                -delta.1 as f32 * sensitivity,
            );
        }
        Event::AboutToWait => window.request_redraw(),
        _ => {}
    });
}
