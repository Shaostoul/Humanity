//! WASM entry point — initializes the engine in the browser.
//!
//! Compiled only with the `wasm` feature flag. Creates a wgpu surface from a
//! canvas element, sets up the render pipeline, and drives the render loop
//! via requestAnimationFrame. Wires keyboard/mouse input to the camera controller.

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use std::cell::RefCell;
use std::rc::Rc;

use glam::{Quat, Vec3};
use crate::ecs::GameWorld;
use crate::ecs::components::{Controllable, Health, Name, Transform, Velocity};
use crate::ecs::systems::SystemRunner;
use crate::hot_reload::data_store::DataStore;
use crate::input::InputState;
use crate::renderer::camera::{Camera, CameraController, CameraMode};
use crate::renderer::mesh::Mesh;
use crate::renderer::{RenderObject, Renderer};
use crate::systems::crafting::CraftingSystem;
use crate::systems::farming::FarmingSystem;
use crate::systems::interaction::InteractionSystem;
use crate::systems::inventory::InventorySystem;
use crate::systems::player::PlayerControllerSystem;
use crate::systems::time::TimeSystem;

/// WASM entry point — called automatically when the module loads.
#[wasm_bindgen(start)]
pub async fn main() {
    // Set up panic hook for better error messages in the browser console
    console_error_panic_hook::set_once();

    // Initialize logging to browser console
    console_log::init_with_level(log::Level::Info)
        .expect("Failed to initialize console logger");

    log::info!("HumanityOS Engine starting (WASM)...");

    // Get the canvas element
    let window = web_sys::window().expect("No global window");
    let document = window.document().expect("No document");
    let canvas = document
        .get_element_by_id("game-canvas")
        .expect("No element with id 'game-canvas'")
        .dyn_into::<HtmlCanvasElement>()
        .expect("Element 'game-canvas' is not a canvas");

    // Match canvas size to its CSS display size
    let device_pixel_ratio = window.device_pixel_ratio();
    let client_width = canvas.client_width() as f64 * device_pixel_ratio;
    let client_height = canvas.client_height() as f64 * device_pixel_ratio;
    canvas.set_width(client_width as u32);
    canvas.set_height(client_height as u32);

    log::info!(
        "Canvas: {}x{} (dpr: {:.1})",
        canvas.width(),
        canvas.height(),
        device_pixel_ratio,
    );

    // Initialize renderer with the canvas
    log::info!("Requesting GPU adapter...");
    let mut renderer = Renderer::new_wasm(canvas.clone()).await;
    log::info!("GPU adapter acquired, renderer initialized.");

    // Create meshes — a scene with multiple objects to navigate around
    let cube_mesh = renderer.add_mesh(Mesh::cube(&renderer.device));
    let plane_mesh = renderer.add_mesh(Mesh::plane(&renderer.device));

    // Create materials
    let cube_material = renderer.add_material([0.8, 0.3, 0.2, 1.0], 0.0, 0.5);
    let green_material = renderer.add_material([0.3, 0.5, 0.3, 1.0], 0.0, 0.8);
    let blue_material = renderer.add_material([0.2, 0.4, 0.8, 1.0], 0.3, 0.4);
    let yellow_material = renderer.add_material([0.9, 0.8, 0.2, 1.0], 0.0, 0.6);

    // Set up camera
    let mut camera = Camera::new();
    camera.position = Vec3::new(0.0, 2.0, 5.0);
    camera.yaw = 0.0;
    camera.pitch = -0.2;
    camera.aspect = renderer.aspect_ratio();

    let controller = CameraController::new(5.0, 3.0);

    log::info!("Scene ready. Controls: WASD=move, Mouse=look, Tab=cycle mode, F=FP/TP, M=orbit, O=ortho, Scroll=zoom");

    // Initialize ECS + game systems
    let mut game_world = GameWorld::new();
    let mut system_runner = SystemRunner::new();
    let mut data_store = DataStore::new();

    // Seed DataStore with initial shared state
    data_store.insert("input_state", InputState::default());
    data_store.insert("camera_position", Vec3::new(0.0, 2.0, 5.0));
    data_store.insert("camera_forward", Vec3::NEG_Z);
    data_store.insert("camera_yaw", 0.0_f32);

    // Register all game systems (tick order matters)
    system_runner.register(TimeSystem::new());
    system_runner.register(PlayerControllerSystem);
    data_store.insert("interaction_prompt", std::sync::Mutex::new(String::new()));
    system_runner.register(InteractionSystem::new());
    system_runner.register(FarmingSystem::new());
    system_runner.register(InventorySystem::new());
    system_runner.register(CraftingSystem::new());

    // Spawn a player entity
    game_world.world.spawn((
        Transform::default(),
        Velocity::default(),
        Controllable,
        Health::default(),
        Name("Player".to_string()),
    ));

    log::info!("ECS initialized: {} systems registered", system_runner.count());

    // Bundle state for the render loop
    let state = Rc::new(RefCell::new(WasmEngineState {
        renderer,
        camera,
        controller,
        game_world,
        system_runner,
        data_store,
        cube_mesh,
        plane_mesh,
        cube_material,
        green_material,
        blue_material,
        yellow_material,
        canvas: canvas.clone(),
        last_timestamp: 0.0,
    }));

    // Wire up input events
    setup_input_handlers(&canvas, &state);

    // Start the render loop
    start_render_loop(state);
}

/// All mutable state needed by the WASM render loop.
struct WasmEngineState {
    renderer: Renderer,
    camera: Camera,
    controller: CameraController,
    game_world: GameWorld,
    system_runner: SystemRunner,
    data_store: DataStore,
    cube_mesh: usize,
    plane_mesh: usize,
    cube_material: usize,
    green_material: usize,
    blue_material: usize,
    yellow_material: usize,
    canvas: HtmlCanvasElement,
    last_timestamp: f64,
}

/// Attach keyboard, mouse, and wheel event listeners to drive the camera.
fn setup_input_handlers(
    canvas: &HtmlCanvasElement,
    state: &Rc<RefCell<WasmEngineState>>,
) {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    // Keyboard down
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
            // Prevent default for game keys to avoid scrolling
            let code = event.code();
            match code.as_str() {
                "KeyW" | "KeyA" | "KeyS" | "KeyD" | "Space" | "Tab" | "KeyE" => {
                    event.prevent_default();
                }
                _ => {}
            }
            let mut s = state.borrow_mut();
            s.controller.process_key(&code, true);
            update_input_state(&mut s.data_store, &code, true);
        });
        document
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Keyboard up
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
            let code = event.code();
            let mut s = state.borrow_mut();
            s.controller.process_key(&code, false);
            update_input_state(&mut s.data_store, &code, false);
        });
        document
            .add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Mouse down on canvas
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            event.prevent_default();
            state
                .borrow_mut()
                .controller
                .set_mouse_button(event.button() as i32, true);
        });
        canvas
            .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Mouse up (on document, to catch releases outside canvas)
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            state
                .borrow_mut()
                .controller
                .set_mouse_button(event.button() as i32, false);
        });
        document
            .add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Mouse move
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let dx = event.movement_x() as f64;
            let dy = event.movement_y() as f64;
            state.borrow_mut().controller.process_mouse_motion(dx, dy);
        });
        canvas
            .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Scroll wheel
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
            event.prevent_default();
            // Normalize: positive delta_y = scroll down = zoom out → negative scroll value
            let delta = -event.delta_y() as f32 / 100.0;
            state.borrow_mut().controller.process_scroll(delta);
        });
        canvas
            .add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Prevent context menu on right-click
    {
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            event.prevent_default();
        });
        canvas
            .add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
}

/// Update InputState in DataStore from a key code string.
fn update_input_state(data_store: &mut DataStore, code: &str, pressed: bool) {
    let mut input = data_store
        .get::<InputState>("input_state")
        .cloned()
        .unwrap_or_default();
    match code {
        "KeyW" => input.forward = pressed,
        "KeyS" => input.backward = pressed,
        "KeyA" => input.left = pressed,
        "KeyD" => input.right = pressed,
        "Space" => input.jump = pressed,
        "KeyE" => input.interact = pressed,
        _ => return, // no change needed
    }
    data_store.insert("input_state", input);
}

/// Drive the render loop via requestAnimationFrame.
fn start_render_loop(state: Rc<RefCell<WasmEngineState>>) {
    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move |timestamp_ms: f64| {
        let mut s = state.borrow_mut();

        // Calculate dt
        let dt = if s.last_timestamp > 0.0 {
            ((timestamp_ms - s.last_timestamp) / 1000.0) as f32
        } else {
            1.0 / 60.0
        };
        s.last_timestamp = timestamp_ms;

        // Clamp dt to avoid huge jumps on tab-switch
        let dt = dt.min(0.1);

        // Handle canvas resize
        let window = web_sys::window().unwrap();
        let dpr = window.device_pixel_ratio();
        let new_width = (s.canvas.client_width() as f64 * dpr) as u32;
        let new_height = (s.canvas.client_height() as f64 * dpr) as u32;
        if new_width != s.canvas.width() || new_height != s.canvas.height() {
            s.canvas.set_width(new_width);
            s.canvas.set_height(new_height);
            s.renderer.resize(new_width, new_height);
            s.camera.aspect = s.renderer.aspect_ratio();
        }

        // Update camera from input
        // Destructure to get separate mutable references (satisfies borrow checker)
        let WasmEngineState {
            ref mut controller,
            ref mut camera,
            ref mut data_store,
            ref mut system_runner,
            ref mut game_world,
            ..
        } = *s;
        controller.update_camera(camera, dt);

        // Sync camera state into DataStore for game systems
        data_store.insert("camera_position", camera.position);
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let forward = Vec3::new(-yaw_sin, 0.0, -yaw_cos).normalize();
        data_store.insert("camera_forward", forward);
        data_store.insert("camera_yaw", camera.yaw);

        // Tick all ECS systems
        system_runner.tick(&mut game_world.world, dt, data_store);

        // Spinning cube rotation
        let elapsed = (timestamp_ms / 1000.0) as f32;
        let cube_rotation = Quat::from_euler(
            glam::EulerRot::YXZ,
            elapsed * 0.7,
            elapsed * 0.5,
            0.0,
        );

        // Scene objects — several cubes to give spatial reference
        let objects = [
            // Center cube (spinning, red)
            RenderObject {
                position: Vec3::new(0.0, 1.0, 0.0),
                rotation: cube_rotation,
                scale: Vec3::ONE,
                mesh: s.cube_mesh,
                material: s.cube_material,
            },
            // Blue cube at +X
            RenderObject {
                position: Vec3::new(4.0, 0.5, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: s.cube_mesh,
                material: s.blue_material,
            },
            // Yellow cube at -Z
            RenderObject {
                position: Vec3::new(0.0, 0.5, -4.0),
                rotation: Quat::from_rotation_y(0.5),
                scale: Vec3::splat(0.7),
                mesh: s.cube_mesh,
                material: s.yellow_material,
            },
            // Ground plane
            RenderObject {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: s.plane_mesh,
                material: s.green_material,
            },
        ];

        // Render
        match s.renderer.render(&s.camera, &objects) {
            Ok(_) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let w = s.canvas.width();
                let h = s.canvas.height();
                s.renderer.resize(w, h);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Out of GPU memory");
                return;
            }
            Err(e) => {
                log::warn!("Render error: {:?}", e);
            }
        }

        // Request next frame
        web_sys::window()
            .unwrap()
            .request_animation_frame(
                f.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
            )
            .expect("requestAnimationFrame failed");
    }));

    // Kick off the first frame
    web_sys::window()
        .unwrap()
        .request_animation_frame(
            g.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
        )
        .expect("requestAnimationFrame failed");
}
