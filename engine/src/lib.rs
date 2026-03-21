//! HumanityOS Game Engine
//!
//! Custom engine built on wgpu, hecs, rapier3d, and kira.
//! Designed for multi-scale space simulation with hot-reloadable data files.
//!
//! Compiles to both native (desktop via Tauri/winit) and WASM (browser via WebGPU).
//! Feature flags: `native` (default) or `wasm`.

pub mod renderer;
pub mod ecs;
pub mod physics;
pub mod audio;
pub mod input;
pub mod assets;
pub mod platform;

#[cfg(feature = "native")]
pub mod hot_reload;

#[cfg(feature = "wasm")]
pub mod wasm_entry;

#[cfg(feature = "native")]
mod native_app {
    use glam::{Quat, Vec3};
    use crate::renderer::camera::{Camera, CameraController};
    use crate::renderer::mesh::Mesh;
    use crate::renderer::{RenderObject, Renderer};
    use std::sync::Arc;
    use std::time::Instant;
    use winit::application::ApplicationHandler;
    use winit::event::{DeviceEvent, DeviceId, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::keyboard::PhysicalKey;
    use winit::window::{Window, WindowId};

    /// Run the engine standalone — opens a window, renders a spinning cube on a ground plane.
    pub fn run() {
        env_logger::init();
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        let mut app = App::new();
        event_loop.run_app(&mut app).expect("Event loop error");
    }

    struct App {
        state: Option<EngineState>,
    }

    impl App {
        fn new() -> Self {
            Self { state: None }
        }
    }

    struct EngineState {
        window: Arc<Window>,
        renderer: Renderer,
        camera: Camera,
        controller: CameraController,
        cube_mesh: usize,
        plane_mesh: usize,
        cube_material: usize,
        plane_material: usize,
        start_time: Instant,
        last_frame: Instant,
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.state.is_some() {
                return;
            }

            let window_attrs = Window::default_attributes()
                .with_title("HumanityOS Engine")
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

            let window = Arc::new(
                event_loop
                    .create_window(window_attrs)
                    .expect("Failed to create window"),
            );

            // Initialize renderer (block on async)
            let mut renderer = pollster::block_on(Renderer::new_native(window.clone()));

            // Create meshes
            let cube_mesh = renderer.add_mesh(Mesh::cube(&renderer.device));
            let plane_mesh = renderer.add_mesh(Mesh::plane(&renderer.device));

            // Create materials
            let cube_material =
                renderer.add_material([0.8, 0.3, 0.2, 1.0], 0.0, 0.5); // red-orange, non-metallic
            let plane_material =
                renderer.add_material([0.3, 0.5, 0.3, 1.0], 0.0, 0.8); // green-ish ground

            let mut camera = Camera::new();
            camera.aspect = renderer.aspect_ratio();

            let controller = CameraController::new(5.0, 3.0);

            self.state = Some(EngineState {
                window,
                renderer,
                camera,
                controller,
                cube_mesh,
                plane_mesh,
                cube_material,
                plane_material,
                start_time: Instant::now(),
                last_frame: Instant::now(),
            });
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            let state = match self.state.as_mut() {
                Some(s) => s,
                None => return,
            };

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(size) => {
                    state.renderer.resize(size.width, size.height);
                    state.camera.aspect = state.renderer.aspect_ratio();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(key) = event.physical_key {
                        if key == winit::keyboard::KeyCode::Escape {
                            event_loop.exit();
                            return;
                        }
                        state.controller.process_keyboard(key, event.state);
                    }
                }
                WindowEvent::MouseInput { button, state: btn_state, .. } => {
                    state.controller.process_mouse_button(button, btn_state);
                }
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    let dt = (now - state.last_frame).as_secs_f32();
                    state.last_frame = now;

                    // Update camera
                    state.controller.update_camera(&mut state.camera, dt);

                    // Spinning cube
                    let elapsed = (now - state.start_time).as_secs_f32();
                    let cube_rotation =
                        Quat::from_euler(glam::EulerRot::YXZ, elapsed * 0.7, elapsed * 0.5, 0.0);

                    let objects = [
                        // Cube floating above the ground
                        RenderObject {
                            position: Vec3::new(0.0, 1.0, 0.0),
                            rotation: cube_rotation,
                            scale: Vec3::ONE,
                            mesh: state.cube_mesh,
                            material: state.cube_material,
                        },
                        // Ground plane
                        RenderObject {
                            position: Vec3::ZERO,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                            mesh: state.plane_mesh,
                            material: state.plane_material,
                        },
                    ];

                    match state.renderer.render(&state.camera, &objects) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            let size = state.window.inner_size();
                            state.renderer.resize(size.width, size.height);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::error!("Out of GPU memory");
                            event_loop.exit();
                        }
                        Err(e) => {
                            log::warn!("Render error: {:?}", e);
                        }
                    }
                }
                _ => {}
            }
        }

        fn device_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _device_id: DeviceId,
            event: DeviceEvent,
        ) {
            let state = match self.state.as_mut() {
                Some(s) => s,
                None => return,
            };

            if let DeviceEvent::MouseMotion { delta } = event {
                state.controller.process_mouse_motion(delta.0, delta.1);
            }
        }

        fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
            if let Some(state) = &self.state {
                state.window.request_redraw();
            }
        }
    }
}

/// Run the native engine (only available with the `native` feature).
#[cfg(feature = "native")]
pub fn run() {
    native_app::run();
}
