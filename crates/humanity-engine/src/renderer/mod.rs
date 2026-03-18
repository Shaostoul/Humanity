//! Renderer — wgpu device/surface setup and render loop.
//!
//! Configuration loaded from `config/renderer.toml`.

pub mod pipeline;
pub mod shader_loader;
pub mod camera;
pub mod multi_scale;

/// Core renderer state wrapping wgpu device, queue, and surface.
pub struct Renderer {
    // TODO: wgpu::Device, wgpu::Queue, wgpu::Surface
}

impl Renderer {
    /// Create a new renderer (stub — will initialize wgpu device and surface).
    pub fn new() -> Self {
        Self {}
    }
}
