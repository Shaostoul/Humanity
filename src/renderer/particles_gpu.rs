//! GPU particle simulation (v0.1068).
//!
//! WHY THIS EXISTS, measured rather than assumed. v0.1066 added a probe and
//! found the CPU particle loop costs ~17-20 ns per particle per frame. v0.1067
//! then tried the two obvious fixes and learned something more useful than the
//! speedup: rayon across six cores bought only 1.25x, which is the signature of
//! a MEMORY-BANDWIDTH limit rather than a compute one. Shrinking `Particle`
//! from 80 to 32 bytes (48 of which were emitter constants copied into every
//! particle) got it to 1.5x. That is the end of that road - the pool has to be
//! streamed through cache twice a frame no matter how fast you make the maths.
//!
//! So: stop moving particles across the bus. The pool lives in GPU memory, the
//! CPU submits one dispatch per frame and never touches a particle.
//!
//! THE DESIGN CHOICE THAT MAKES THIS SAFE. The compute shader writes the SAME
//! `ParticleVertexData` layout the CPU used to upload, into a buffer flagged
//! `VERTEX | STORAGE`. The draw path is therefore completely unchanged - same
//! pipeline, same instanced quad, same fragment shader with its procedural
//! snowflake. It also avoids the exact failure that shipped three unbootable
//! releases in v0.782: a vertex shader reading a storage buffer needs
//! vertex-stage storage support, and this design never asks for it. Compute
//! and storage under `wgpu::Limits::default()` (what the renderer requests)
//! allow a 128 MiB binding = 4.19M particles, and 65535 x 256 invocations per
//! dispatch, both far past anything wanted here.
//!
//! RECYCLING, NOT SPAWN/FREE. A dead slot respawns in place inside the emitter
//! volume instead of being compacted out. For weather that is not a compromise,
//! it is the correct model - steady rain is a pool in equilibrium - and it buys
//! no free list, no compaction, no atomic counter, and a dispatch whose size is
//! identical every frame.

use crate::renderer::particles::ParticleVertexData;

/// Per-particle state on the GPU. 32 bytes, matching `ParticleState` in
/// assets/shaders/particle_sim.wgsl. Interleaved so each thread touches one
/// cache line's worth of contiguous data.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuParticleState {
    position: [f32; 3],
    age: f32,
    velocity: [f32; 3],
    lifetime: f32,
}

/// Simulation parameters, one uniform per dispatch. Mirrors `SimParams` in the
/// shader; keep the two in lockstep.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimParams {
    /// xyz = emitter origin (world), w = spawn radius
    pub origin_radius: [f32; 4],
    /// xyz = base fall direction (unit), w = spread (radians)
    pub dir_spread: [f32; 4],
    /// xyz = gravity, w = dt
    pub gravity_dt: [f32; 4],
    /// speed_min, speed_max, life_min, life_max
    pub speed_life: [f32; 4],
    /// size_start, size_end, emissive, shape
    pub size_shape: [f32; 4],
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
    /// live count, frame counter (RNG salt), streak seconds, unused
    pub count_frame_streak: [f32; 4],
}

pub struct GpuParticles {
    state_buf: wgpu::Buffer,
    /// The draw's vertex buffer. `VERTEX | STORAGE`: written by compute, read
    /// by the unchanged particle draw.
    pub vertex_buf: wgpu::Buffer,
    params_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    /// Slots allocated. The pool only ever grows.
    capacity: u32,
    /// Slots simulated this frame.
    pub live: u32,
    frame: u32,
}

impl GpuParticles {
    /// Threads per workgroup; must match `@workgroup_size` in the shader.
    const WG: u32 = 256;

    pub fn new(device: &wgpu::Device, capacity: u32) -> Self {
        let capacity = capacity.max(Self::WG);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Sim"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/particle_sim.wgsl").into(),
            ),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particle Sim BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Sim PL"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Particle Sim Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let state_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle State"),
            size: capacity as u64 * std::mem::size_of::<GpuParticleState>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Verts (GPU sim)"),
            size: capacity as u64 * std::mem::size_of::<ParticleVertexData>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Sim Params"),
            size: std::mem::size_of::<SimParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Sim BG"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: state_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vertex_buf.as_entire_binding(),
                },
            ],
        });
        log::info!(
            "[GpuParticles] pool {} slots ({:.1} MB state + {:.1} MB verts)",
            capacity,
            capacity as f64 * 32.0 / 1.048576e6,
            capacity as f64 * std::mem::size_of::<ParticleVertexData>() as f64 / 1.048576e6,
        );
        Self {
            state_buf,
            vertex_buf,
            params_buf,
            bind_group,
            pipeline,
            capacity,
            live: 0,
            frame: 0,
        }
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Advance the pool by one frame. `live` is clamped to the pool size, so a
    /// density slider can be dragged past capacity without any risk - it simply
    /// stops getting denser.
    pub fn simulate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut params: SimParams,
        live: u32,
    ) {
        self.live = live.min(self.capacity);
        if self.live == 0 {
            return;
        }
        self.frame = self.frame.wrapping_add(1);
        params.count_frame_streak[0] = self.live as f32;
        params.count_frame_streak[1] = self.frame as f32;
        queue.write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&params));
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Particle Sim Encoder"),
        });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Particle Sim Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(self.live.div_ceil(Self::WG), 1, 1);
        }
        queue.submit(std::iter::once(enc.finish()));
    }

    /// Zero the state so every slot re-seeds on the next dispatch. Used when
    /// the emitter's character changes enough that carrying old particles would
    /// look wrong (a condition switch, a teleport).
    pub fn reset(&self, queue: &wgpu::Queue) {
        let zeros = vec![0u8; self.capacity as usize * std::mem::size_of::<GpuParticleState>()];
        queue.write_buffer(&self.state_buf, 0, &zeros);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The GPU state struct must stay 32 bytes: it is the whole reason this
    /// path exists (the CPU one was bandwidth-bound at 80, then 32).
    #[test]
    fn gpu_particle_state_is_32_bytes() {
        assert_eq!(std::mem::size_of::<GpuParticleState>(), 32);
    }

    /// SimParams must match the shader's uniform block exactly - 8 vec4s.
    #[test]
    fn sim_params_matches_shader_block() {
        assert_eq!(std::mem::size_of::<SimParams>(), 8 * 16);
        let src = include_str!("../../assets/shaders/particle_sim.wgsl");
        for field in [
            "origin_radius",
            "dir_spread",
            "gravity_dt",
            "speed_life",
            "size_shape",
            "color_start",
            "color_end",
            "count_frame_streak",
        ] {
            assert!(src.contains(field), "shader missing SimParams field {field}");
        }
        // The dispatch arithmetic in simulate() assumes this workgroup size.
        assert!(
            src.contains("@workgroup_size(256)"),
            "workgroup size must stay in lockstep with GpuParticles::WG"
        );
    }

    /// The compute shader writes the draw's vertex layout directly, which is
    /// what keeps the draw path unchanged. If ParticleVertexData ever grows,
    /// the shader's ParticleVertex must grow with it.
    #[test]
    fn vertex_layout_is_shared_with_the_draw() {
        assert_eq!(std::mem::size_of::<ParticleVertexData>(), 52);
        let src = include_str!("../../assets/shaders/particle_sim.wgsl");
        assert!(src.contains("struct ParticleVertex"));
        for f in ["position", "color", "size_emissive", "stretch"] {
            assert!(src.contains(f), "shader vertex missing {f}");
        }
    }
}

#[cfg(test)]
mod device_tests {
    use super::*;

    /// THE verification that matters for this module: the engine had no compute
    /// shader before v0.1068, so nothing had ever proven the device accepts one.
    /// wgpu validates WGSL and the pipeline layout at creation, and a rejection
    /// there is exactly the failure class that shipped three unbootable releases
    /// in v0.782 - caught only when a human launched the game.
    ///
    /// Ignored because it needs a real adapter (CI has none). Run it deliberately:
    ///   cargo test --features native --lib device_tests -- --ignored --nocapture
    #[test]
    #[ignore]
    fn compute_pipeline_builds_on_a_real_device() {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("no adapter");
        let limits = wgpu::Limits::default().using_resolution(adapter.limits());
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("particle sim test"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                ..Default::default()
            },
            None,
        ))
        .expect("no device");
        device.on_uncaptured_error(Box::new(|e| panic!("wgpu validation error: {e}")));

        let mut gp = GpuParticles::new(&device, 100_000);
        assert_eq!(gp.capacity(), 100_000);
        let params = SimParams {
            origin_radius: [0.0, 50.0, 0.0, 15.0],
            dir_spread: [0.0, -1.0, 0.0, 0.05],
            gravity_dt: [0.0, -2.0, 0.0, 1.0 / 60.0],
            speed_life: [9.0, 14.0, 1.4, 2.0],
            size_shape: [0.02, 0.02, 0.0, 0.0],
            color_start: [0.6, 0.7, 0.9, 0.4],
            color_end: [0.6, 0.7, 0.9, 0.2],
            count_frame_streak: [0.0, 0.0, 0.0, 0.0],
        };
        // Several frames: the first seeds the pool from zeroed state, later ones
        // exercise the advance and the recycle-on-death branch.
        for _ in 0..8 {
            gp.simulate(&device, &queue, params, 100_000);
        }
        device.poll(wgpu::Maintain::Wait);
        assert_eq!(gp.live, 100_000);
        println!("[GpuParticles] dispatch OK: 100,000 slots, 8 frames, no validation error");
    }
}
