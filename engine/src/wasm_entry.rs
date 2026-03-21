//! WASM entry point — initializes the engine in the browser.
//!
//! Compiled only with the `wasm` feature flag. Creates a wgpu surface from a
//! canvas element, sets up the render pipeline, and drives the render loop
//! via requestAnimationFrame.

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use std::cell::RefCell;
use std::rc::Rc;

use glam::{Quat, Vec3};
use crate::renderer::camera::Camera;
use crate::renderer::mesh::Mesh;
use crate::renderer::{RenderObject, Renderer};

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

    // Create meshes
    let cube_mesh = renderer.add_mesh(Mesh::cube(&renderer.device));
    let plane_mesh = renderer.add_mesh(Mesh::plane(&renderer.device));

    // Create materials
    let cube_material = renderer.add_material([0.8, 0.3, 0.2, 1.0], 0.0, 0.5);
    let plane_material = renderer.add_material([0.3, 0.5, 0.3, 1.0], 0.0, 0.8);

    // Set up camera at a 3/4 angle to see the cube
    let mut camera = Camera::new();
    camera.position = Vec3::new(3.0, 3.0, 5.0);
    camera.yaw = -0.5;    // rotate slightly left to face the cube
    camera.pitch = -0.4;  // look down at the cube
    camera.aspect = renderer.aspect_ratio();

    log::info!("Scene ready: spinning cube + ground plane. Starting render loop.");

    // Bundle state for the render loop (single-threaded WASM, no Send/Sync needed)
    let state = Rc::new(RefCell::new(WasmEngineState {
        renderer,
        camera,
        cube_mesh,
        plane_mesh,
        cube_material,
        plane_material,
        canvas,
    }));

    // Start the requestAnimationFrame render loop
    start_render_loop(state);
}

/// All mutable state needed by the WASM render loop.
struct WasmEngineState {
    renderer: Renderer,
    camera: Camera,
    cube_mesh: usize,
    plane_mesh: usize,
    cube_material: usize,
    plane_material: usize,
    canvas: HtmlCanvasElement,
}

/// Drive the render loop via requestAnimationFrame.
/// The closure captures shared state and re-registers itself each frame.
fn start_render_loop(state: Rc<RefCell<WasmEngineState>>) {
    // Shared closure reference for self-registration
    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let start_time = web_sys::window()
        .unwrap()
        .performance()
        .unwrap()
        .now();

    *g.borrow_mut() = Some(Closure::new(move |timestamp_ms: f64| {
        // elapsed time in seconds since start
        let elapsed = (timestamp_ms - start_time) / 1000.0;

        let mut s = state.borrow_mut();

        // Handle canvas resize (CSS may change size at any time)
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

        // Spinning cube rotation based on elapsed time
        let cube_rotation = Quat::from_euler(
            glam::EulerRot::YXZ,
            elapsed as f32 * 0.7,
            elapsed as f32 * 0.5,
            0.0,
        );

        let objects = [
            // Cube floating above the ground
            RenderObject {
                position: Vec3::new(0.0, 1.0, 0.0),
                rotation: cube_rotation,
                scale: Vec3::ONE,
                mesh: s.cube_mesh,
                material: s.cube_material,
            },
            // Ground plane
            RenderObject {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: s.plane_mesh,
                material: s.plane_material,
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
                return; // Stop the render loop
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
