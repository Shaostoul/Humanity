use bytemuck::{Pod, Zeroable};
use core_firstperson_controller::{apply_look, apply_move, ControllerInput, MoveDir};
use core_offline_loop::{apply_command, Command, WorldSnapshot};
use std::collections::HashSet;
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
                            KeyCode::KeyE => self.run_game_command(Command::Gather("wood".to_string())),
                            KeyCode::KeyQ => self.run_game_command(Command::Gather("fiber".to_string())),
                            KeyCode::KeyZ => self.run_game_command(Command::Gather("scrap".to_string())),
                            KeyCode::KeyR => self.run_game_command(Command::CraftFilter),
                            KeyCode::KeyT => self.run_game_command(Command::TreatWater),
                            KeyCode::KeyF => self.run_game_command(Command::FarmTick),
                            KeyCode::KeyC => self.run_game_command(Command::Eat),
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
        let dt = (now - self.last_update).as_secs_f32().clamp(0.0, 0.1);
        self.last_update = now;

        if !self.menu_open {
            let sprint = self.key_down(KeyCode::ShiftLeft);
            if self.key_down(KeyCode::KeyW) || self.key_down(KeyCode::ArrowUp) {
                apply_move(
                    &mut self.world.controller,
                    ControllerInput {
                        dir: MoveDir::Forward,
                        dt_seconds: dt,
                        sprint,
                    },
                );
            }
            if self.key_down(KeyCode::KeyS) || self.key_down(KeyCode::ArrowDown) {
                apply_move(
                    &mut self.world.controller,
                    ControllerInput {
                        dir: MoveDir::Backward,
                        dt_seconds: dt,
                        sprint,
                    },
                );
            }
            if self.key_down(KeyCode::KeyA) || self.key_down(KeyCode::ArrowLeft) {
                apply_move(
                    &mut self.world.controller,
                    ControllerInput {
                        dir: MoveDir::Left,
                        dt_seconds: dt,
                        sprint,
                    },
                );
            }
            if self.key_down(KeyCode::KeyD) || self.key_down(KeyCode::ArrowRight) {
                apply_move(
                    &mut self.world.controller,
                    ControllerInput {
                        dir: MoveDir::Right,
                        dt_seconds: dt,
                        sprint,
                    },
                );
            }
        }

        self.world.player_pos.x = self.world.controller.position.x.round() as i32;
        self.world.player_pos.y = self.world.controller.position.z.round() as i32;

        let vertices = build_scene_vertices(
            &self.world,
            self.start_time.elapsed().as_secs_f32(),
            self.size,
            self.menu_open,
        );
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
        let msg = if Instant::now() <= self.status_until {
            self.status_msg.as_str()
        } else if self.menu_open {
            "MENU OPEN (I inventory, O objective, ESC close)"
        } else {
            "Running"
        };

        format!(
            "Humanity Shell | EASY | pos=({},{}) yaw={:.0} stamina={:.0} water={:.0} contam={:.1} inv[w:{} f:{} s:{} k:{} food:{}] obj[{}/{}/{}] | {}",
            self.world.player_pos.x,
            self.world.player_pos.y,
            self.world.controller.yaw_deg,
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
            msg,
        )
    }

    fn render(&mut self) -> Result<(), SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
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

fn build_scene_vertices(
    world: &WorldSnapshot,
    t: f32,
    size: PhysicalSize<u32>,
    menu_open: bool,
) -> Vec<Vertex> {
    let mut v = Vec::new();

    let cam_x = world.controller.position.x;
    let cam_z = world.controller.position.z;
    let yaw_rad = world.controller.yaw_deg.to_radians();

    let day_phase = (t / 120.0) % 1.0; // slower day/night
    let sun_angle = day_phase * std::f32::consts::TAU;
    let day_light = ((sun_angle.sin() + 1.0) * 0.5).clamp(0.0, 1.0);
    let night = 1.0 - day_light;

    let sky_top = [0.02 + 0.18 * day_light, 0.04 + 0.32 * day_light, 0.08 + 0.70 * day_light];
    let sky_horizon = [0.06 + 0.45 * day_light, 0.05 + 0.30 * day_light, 0.10 + 0.20 * day_light];
    let horizon_y = -0.18;

    add_rect(&mut v, -1.0, horizon_y, 1.0, 1.0, sky_horizon, sky_top);

    // Sun / moon
    let sx = (sun_angle.cos() * 0.55) - (yaw_rad.sin() * 0.08);
    let sy = sun_angle.sin() * 0.30 + 0.35;
    if day_light > 0.2 {
        add_diamond(&mut v, sx, sy, 0.03, 0.03, [1.0, 0.84, 0.35]);
    } else {
        add_diamond(&mut v, -sx * 0.9, sy * 0.9, 0.025, 0.025, [0.84, 0.90, 1.0]);
    }

    // stars / constellations
    if night > 0.35 {
        let stars = [
            (-0.76, 0.76), (-0.70, 0.81), (-0.64, 0.77),
            (-0.12, 0.85), (-0.02, 0.88), (0.08, 0.84),
            (0.48, 0.72), (0.58, 0.77), (0.68, 0.73),
        ];
        for (x, y) in stars {
            let tw = 0.004 + 0.003 * (t * 2.2 + x * 13.0).sin().abs();
            add_diamond(&mut v, x - yaw_rad.sin() * 0.03, y, tw, tw, [0.8 + 0.2 * night, 0.9, 1.0]);
        }
    }

    // distant mountain silhouette with yaw parallax
    let mshift = -yaw_rad.sin() * 0.20;
    let mcol = [0.08 + day_light * 0.10, 0.10 + day_light * 0.12, 0.16 + day_light * 0.16];
    add_triangle(&mut v, [-1.1 + mshift, horizon_y], [-0.6 + mshift, 0.20], [-0.1 + mshift, horizon_y], mcol);
    add_triangle(&mut v, [-0.35 + mshift, horizon_y], [0.05 + mshift, 0.08], [0.45 + mshift, horizon_y], [mcol[0]*0.9,mcol[1]*0.9,mcol[2]*0.9]);
    add_triangle(&mut v, [0.15 + mshift, horizon_y], [0.70 + mshift, 0.24], [1.15 + mshift, horizon_y], [mcol[0]*0.8,mcol[1]*0.8,mcol[2]*0.8]);

    // Perspective ground strips (first-person vibe)
    for i in 0..70 {
        let near = i as f32 / 70.0;
        let far = (i + 1) as f32 / 70.0;

        let y_near = -1.0 + near * (horizon_y + 1.0);
        let y_far = -1.0 + far * (horizon_y + 1.0);

        let half_near = 1.25 * (1.0 - near) + 0.03;
        let half_far = 1.25 * (1.0 - far) + 0.03;

        let center_near = ((cam_x * 0.03) + yaw_rad.sin() * 0.18) * near;
        let center_far = ((cam_x * 0.03) + yaw_rad.sin() * 0.18) * far;

        let g = if i % 2 == 0 { 0.24 } else { 0.20 };
        let base = [0.10 + g * day_light, 0.18 + g, 0.10 + 0.12 * day_light];
        add_rect(
            &mut v,
            center_near - half_near,
            y_near,
            center_near + half_near,
            y_far,
            base,
            [base[0] * 0.85, base[1] * 0.85, base[2] * 0.85],
        );

        // central path hint
        let p_half_near = half_near * 0.17;
        let p_half_far = half_far * 0.17;
        add_rect(
            &mut v,
            center_near - p_half_near,
            y_near,
            center_far + p_half_far,
            y_far,
            [0.28, 0.25, 0.22],
            [0.22, 0.20, 0.18],
        );
    }

    // Blossoming orchard billboards
    for i in 0..36 {
        let wx = -28.0 + i as f32 * 1.8;
        let wz = 10.0 + (i as f32 * 3.4) % 90.0;
        let rel_x = wx - cam_x;
        let rel_z = wz - cam_z;
        if rel_z <= 2.0 || rel_z > 95.0 {
            continue;
        }

        let depth = (1.0 - rel_z / 95.0).clamp(0.0, 1.0);
        let sx = (rel_x / rel_z * 0.7) - yaw_rad.sin() * 0.2;
        let y = -1.0 + depth * (horizon_y + 1.0);
        let scale = 0.004 + depth * 0.03;
        let sway = (t * 2.2 + i as f32 * 0.3).sin() * scale * 0.6;

        // stem
        add_rect(
            &mut v,
            sx - scale * 0.12,
            y,
            sx + scale * 0.12,
            y + scale * 2.0,
            [0.20, 0.35, 0.16],
            [0.25, 0.45, 0.20],
        );
        // blossom
        add_diamond(
            &mut v,
            sx + sway,
            y + scale * 2.2,
            scale * 0.55,
            scale * 0.55,
            [0.85, 0.62, 0.78],
        );
    }

    // Menu overlay
    if menu_open {
        add_rect(
            &mut v,
            -0.75,
            -0.55,
            0.75,
            0.55,
            [0.05, 0.08, 0.10],
            [0.09, 0.12, 0.14],
        );
        add_rect(
            &mut v,
            -0.73,
            -0.53,
            0.73,
            -0.48,
            [0.14, 0.20, 0.24],
            [0.14, 0.20, 0.24],
        );
    }

    // 8px green diamond crosshair
    let sx = (8.0 / size.width.max(1) as f32).clamp(0.002, 0.03);
    let sy = (8.0 / size.height.max(1) as f32).clamp(0.002, 0.03);
    add_diamond(&mut v, 0.0, 0.0, sx, sy, [0.10, 1.0, 0.2]);

    v
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("event loop");
    let window = WindowBuilder::new()
        .with_title("Humanity Shell")
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
                let sensitivity = 0.08;
                apply_look(
                    &mut state.world.controller,
                    delta.0 as f32 * sensitivity,
                    -delta.1 as f32 * sensitivity,
                );
            }
        }
        Event::AboutToWait => window.request_redraw(),
        _ => {}
    });
}
