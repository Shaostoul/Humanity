use core_firstperson_controller::{apply_look, apply_move, ControllerInput, MoveDir};
use core_offline_loop::{apply_command, Command, WorldSnapshot};
use std::collections::HashSet;
use std::time::Instant;
use wgpu::SurfaceError;
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    world: WorldSnapshot,
    pressed: HashSet<KeyCode>,
    last_update: Instant,
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

        Self {
            surface,
            device,
            queue,
            config,
            size,
            world: WorldSnapshot::new_default(),
            pressed: HashSet::new(),
            last_update: Instant::now(),
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
    }

    fn title_text(&self) -> String {
        format!(
            "Humanity Shell | pos=({},{}) stamina={:.0} water={:.0} contam={:.1} inv[w:{} f:{} s:{} k:{} food:{}] milestones[{}/{}/{}]",
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
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render-encoder"),
                });

        {
            let hydration = self.world.player.physiology.hydration / 100.0;
            let stamina = self.world.controller.stamina / 100.0;
            let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: (0.10 + 0.25 * hydration as f64).clamp(0.0, 1.0),
                            g: (0.12 + 0.30 * stamina as f64).clamp(0.0, 1.0),
                            b: 0.18,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("event loop");
    let window = WindowBuilder::new()
        .with_title("Humanity Engine Shell (wgpu scaffold)")
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
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    });
}
