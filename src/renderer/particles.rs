//! GPU particle system with data-driven emitter definitions.
//!
//! Particles are simulated on the CPU and rendered as billboarded point sprites.
//! Emitter types are loaded from `data/particles.ron` and are hot-reloadable.
//!
//! Usage:
//!   let mut ps = ParticleSystem::new();
//!   ps.spawn("fire", position);           // start emitting
//!   ps.tick(dt);                          // advance simulation
//!   ps.render(&renderer, &camera);        // draw all particles

use glam::Vec3;

/// A single live particle.
#[derive(Clone)]
struct Particle {
    position: Vec3,
    velocity: Vec3,
    age: f32,
    lifetime: f32,
    size_start: f32,
    size_end: f32,
    color_start: [f32; 4],
    color_end: [f32; 4],
    emissive: f32,
    /// Sprite shape, copied from the emitter def (v0.1064).
    shape: f32,
}

/// Serializable emitter def for RON loading.
#[derive(Clone, Debug, serde::Deserialize)]
struct EmitterDefRon {
    #[serde(default = "default_100")] max_particles: usize,
    #[serde(default = "default_20f")] spawn_rate: f32,
    #[serde(default = "default_half")] lifetime_min: f32,
    #[serde(default = "default_1_5")] lifetime_max: f32,
    #[serde(default = "default_1f")] speed_min: f32,
    #[serde(default = "default_2f")] speed_max: f32,
    #[serde(default = "default_up")] direction: (f32, f32, f32),
    #[serde(default = "default_30f")] spread_angle_deg: f32,
    #[serde(default = "default_zero3")] gravity: (f32, f32, f32),
    #[serde(default = "default_005")] size_start: f32,
    #[serde(default = "default_001")] size_end: f32,
    #[serde(default = "default_white4")] color_start: (f32, f32, f32, f32),
    #[serde(default = "default_clear4")] color_end: (f32, f32, f32, f32),
    #[serde(default)] emissive: f32,
    #[serde(default = "default_alpha")] blend_mode: String,
    /// Spawn positions jitter inside this radius around the emitter (v0.966:
    /// area effects like drifting leaves and space dust need a volume, not a
    /// point source). 0 = the classic point emitter.
    #[serde(default)] spawn_radius: f32,
    /// Motion-blur streak length in SECONDS of travel (v0.1036): each
    /// particle's quad stretches along its velocity by v * this. 0 =
    /// classic round sprite; rain uses ~0.035 s so 12 m/s drops draw as
    /// ~40 cm streaks while snow stays a drifting flake.
    #[serde(default)] streak_stretch: f32,
    #[serde(default)] shape: f32,
}
fn default_100() -> usize { 100 }
fn default_20f() -> f32 { 20.0 }
fn default_half() -> f32 { 0.5 }
fn default_1_5() -> f32 { 1.5 }
fn default_1f() -> f32 { 1.0 }
fn default_2f() -> f32 { 2.0 }
fn default_up() -> (f32, f32, f32) { (0.0, 1.0, 0.0) }
fn default_30f() -> f32 { 30.0 }
fn default_zero3() -> (f32, f32, f32) { (0.0, 0.0, 0.0) }
fn default_005() -> f32 { 0.05 }
fn default_001() -> f32 { 0.01 }
fn default_white4() -> (f32, f32, f32, f32) { (1.0, 1.0, 1.0, 1.0) }
fn default_clear4() -> (f32, f32, f32, f32) { (1.0, 1.0, 1.0, 0.0) }
fn default_alpha() -> String { "alpha".into() }

impl EmitterDefRon {
    fn to_def(&self) -> EmitterDef {
        EmitterDef {
            max_particles: self.max_particles,
            spawn_rate: self.spawn_rate,
            lifetime_min: self.lifetime_min,
            lifetime_max: self.lifetime_max,
            speed_min: self.speed_min,
            speed_max: self.speed_max,
            direction: Vec3::new(self.direction.0, self.direction.1, self.direction.2),
            spread_angle_deg: self.spread_angle_deg,
            gravity: Vec3::new(self.gravity.0, self.gravity.1, self.gravity.2),
            size_start: self.size_start,
            size_end: self.size_end,
            color_start: [self.color_start.0, self.color_start.1, self.color_start.2, self.color_start.3],
            color_end: [self.color_end.0, self.color_end.1, self.color_end.2, self.color_end.3],
            emissive: self.emissive,
            blend_additive: self.blend_mode == "additive",
            spawn_radius: self.spawn_radius,
            streak_stretch: self.streak_stretch,
            shape: self.shape,
        }
    }
}

/// Definition of a particle emitter type (loaded from RON).
#[derive(Clone, Debug)]
pub struct EmitterDef {
    pub max_particles: usize,
    pub spawn_rate: f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub direction: Vec3,
    pub spread_angle_deg: f32,
    pub gravity: Vec3,
    pub size_start: f32,
    pub size_end: f32,
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
    pub emissive: f32,
    pub blend_additive: bool,
    pub spawn_radius: f32,
    pub streak_stretch: f32,
    /// Sprite SHAPE (v0.1064): 0 = the classic soft round dot, 1 = a procedural
    /// six-fold snowflake. Operator: "Can we also do procedural snowflake
    /// designs instead of just dots? While it won't matter much for distant
    /// snow the stuff that falls real close to the screen will look beautiful."
    /// A data field rather than a magic sentinel packed into emissive, so new
    /// shapes (hail, ash, blossom) are a RON edit and a shader arm.
    pub shape: f32,
}

impl Default for EmitterDef {
    fn default() -> Self {
        Self {
            max_particles: 100,
            spawn_rate: 20.0,
            lifetime_min: 0.5,
            lifetime_max: 1.5,
            speed_min: 1.0,
            speed_max: 2.0,
            direction: Vec3::Y,
            spread_angle_deg: 30.0,
            gravity: Vec3::ZERO,
            size_start: 0.05,
            size_end: 0.01,
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 1.0, 1.0, 0.0],
            emissive: 0.0,
            blend_additive: false,
            spawn_radius: 0.0,
            streak_stretch: 0.0,
            shape: 0.0,
        }
    }
}

/// An active emitter instance in the world.
pub struct Emitter {
    pub emitter_type: String,
    pub position: Vec3,
    pub active: bool,
    /// Per-instance emission direction override (v0.1033, precipitation):
    /// the shared def says "straight down", but live wind TILTS rain -
    /// the weather-driven emitter updates this every frame. None = def.
    pub dir_override: Option<Vec3>,
    /// Per-instance spawn-rate multiplier (v0.1033): weather intensity
    /// scales drizzle -> downpour without forking the def.
    pub rate_scale: f32,
    /// Multiplies the emitter def's max_particles at runtime (v0.1060).
    /// Operator: "We could make the rain/snow/hail/whatever even denser, maybe
    /// a slider that goes fairly extreme so we can test limits?" - and the
    /// limit they were hitting is exactly this: rate_scale only ever changed
    /// the spawn RATE, while the population ceiling stayed pinned at the RON's
    /// max_particles (1600 for rain). Past that cap a higher rate buys nothing
    /// at all, which is why heavy rain looked like light rain. Scaling the cap
    /// is what actually makes a downpour.
    pub cap_scale: f32,
    particles: Vec<Particle>,
    spawn_accumulator: f32,
    rng_state: u32,
}

impl Emitter {
    fn new(emitter_type: String, position: Vec3) -> Self {
        Self {
            emitter_type,
            position,
            active: true,
            dir_override: None,
            rate_scale: 1.0,
            cap_scale: 1.0,
            particles: Vec::new(),
            spawn_accumulator: 0.0,
            rng_state: (position.x.to_bits() ^ position.y.to_bits() ^ position.z.to_bits())
                .wrapping_add(42),
        }
    }

    /// Simple fast pseudo-random [0, 1)
    fn rand(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng_state >> 9) as f32 / (1u32 << 23) as f32
    }

    fn rand_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.rand() * (max - min)
    }

    fn tick(&mut self, dt: f32, def: &EmitterDef) {
        // Advance existing particles
        self.particles.retain_mut(|p| {
            p.age += dt;
            if p.age >= p.lifetime {
                return false;
            }
            p.velocity += def.gravity * dt;
            p.position += p.velocity * dt;
            true
        });

        // Spawn new particles
        if self.active {
            let cap = ((def.max_particles as f32) * self.cap_scale.max(0.0)) as usize;
            // ── STEADY-STATE RATE CLAMP (v0.1064) ──
            // Operator: "the rain kinda pulses from heavy to light to heavy. It
            // should be consistent."
            //
            // A pool of N particles each living L seconds can only sustain N/L
            // spawns per second - that is what steady state MEANS. Asking for
            // more does not make denser rain; it fills the pool in a burst, the
            // cap then blocks all spawning, the whole cohort dies together
            // because they were born together, and the cycle repeats. That is
            // the pulse, and raising precip_density made it worse by widening
            // the gap between the requested rate and the sustainable one.
            //
            // Clamping the rate to the sustainable value keeps the pool full
            // AND lets ages spread out, so deaths become uniform and the fall
            // looks continuous. Density still rises with the cap, which is the
            // knob that actually adds rain.
            let life_avg = ((def.lifetime_min + def.lifetime_max) * 0.5).max(0.05);
            let sustainable = cap as f32 / life_avg;
            let want_rate = def.spawn_rate * self.rate_scale.max(0.0);
            self.spawn_accumulator += want_rate.min(sustainable) * dt;
            while self.spawn_accumulator >= 1.0 && self.particles.len() < cap {
                self.spawn_accumulator -= 1.0;
                self.spawn_particle(def);
            }
        }
    }

    fn spawn_particle(&mut self, def: &EmitterDef) {
        let lifetime = self.rand_range(def.lifetime_min, def.lifetime_max);
        let speed = self.rand_range(def.speed_min, def.speed_max);

        // Random direction within cone
        let spread_rad = def.spread_angle_deg.to_radians();
        let theta = self.rand() * std::f32::consts::TAU;
        let phi = self.rand() * spread_rad;
        let sin_phi = phi.sin();
        let local_dir = Vec3::new(sin_phi * theta.cos(), phi.cos(), sin_phi * theta.sin());

        // Rotate local direction to align with emitter direction
        // (instance override wins - wind-tilted precipitation).
        let up = self.dir_override.unwrap_or(def.direction).normalize_or_zero();
        let velocity = if up.length_squared() < 0.001 {
            // Omnidirectional
            let rx = self.rand() * 2.0 - 1.0;
            let ry = self.rand() * 2.0 - 1.0;
            let rz = self.rand() * 2.0 - 1.0;
            Vec3::new(rx, ry, rz).normalize_or_zero() * speed
        } else {
            // Build rotation from Y-axis to emitter direction
            let right = up.cross(Vec3::Z).normalize_or_zero();
            let right = if right.length_squared() < 0.001 { up.cross(Vec3::X).normalize() } else { right };
            let forward = right.cross(up).normalize();
            (right * local_dir.x + up * local_dir.y + forward * local_dir.z).normalize() * speed
        };

        // Volume jitter (v0.966): random point in a cube of spawn_radius
        // (slight corner bias is fine for ambience).
        let jitter = if def.spawn_radius > 0.0 {
            Vec3::new(
                self.rand() * 2.0 - 1.0,
                self.rand() * 2.0 - 1.0,
                self.rand() * 2.0 - 1.0,
            ) * def.spawn_radius
        } else {
            Vec3::ZERO
        };
        self.particles.push(Particle {
            position: self.position + jitter,
            velocity,
            age: 0.0,
            lifetime,
            size_start: def.size_start,
            size_end: def.size_end,
            color_start: def.color_start,
            color_end: def.color_end,
            emissive: def.emissive,
            shape: def.shape,
        });
    }

    /// Collect vertex data for GPU upload. `streak_s` is the emitter
    /// def's motion-blur seconds (0 = round sprites).
    fn collect_vertices(&self, streak_s: f32) -> Vec<ParticleVertexData> {
        self.particles.iter().map(|p| {
            let t = (p.age / p.lifetime).clamp(0.0, 1.0);
            let size = p.size_start + (p.size_end - p.size_start) * t;
            let color = [
                p.color_start[0] + (p.color_end[0] - p.color_start[0]) * t,
                p.color_start[1] + (p.color_end[1] - p.color_start[1]) * t,
                p.color_start[2] + (p.color_end[2] - p.color_start[2]) * t,
                p.color_start[3] + (p.color_end[3] - p.color_start[3]) * t,
            ];
            let s = p.velocity * streak_s;
            ParticleVertexData {
                position: [p.position.x, p.position.y, p.position.z],
                color,
                size_emissive: [size, p.emissive, p.shape],
                stretch: [s.x, s.y, s.z],
            }
        }).collect()
    }
}

/// GPU vertex data for a single particle (matches shader ParticleVertex).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleVertexData {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub size_emissive: [f32; 3],
    /// World-space motion vector (velocity * streak seconds, v0.1036):
    /// the shader stretches the billboard along its screen projection.
    /// Zero = classic round sprite.
    pub stretch: [f32; 3],
}

/// The main particle system managing all emitters.
pub struct ParticleSystem {
    pub emitter_defs: std::collections::HashMap<String, EmitterDef>,
    pub emitters: Vec<Emitter>,
}

impl ParticleSystem {
    pub fn new() -> Self {
        Self {
            emitter_defs: std::collections::HashMap::new(),
            emitters: Vec::new(),
        }
    }

    /// Load emitter definitions from the data directory.
    pub fn load_defs(&mut self, data_dir: &std::path::Path) {
        let path = data_dir.join("particles.ron");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Could not load particles.ron: {}", e);
                return;
            }
        };
        let parsed: Result<std::collections::HashMap<String, EmitterDefRon>, _> = ron::from_str(&text);
        match parsed {
            Ok(map) => {
                self.emitter_defs.clear();
                for (id, raw) in map {
                    self.emitter_defs.insert(id, raw.to_def());
                }
                log::info!("Loaded {} particle emitter definitions", self.emitter_defs.len());
            }
            Err(e) => {
                log::warn!("Failed to parse particles.ron: {}", e);
            }
        }
    }

    /// Spawn a new emitter at a position. Returns the emitter index.
    pub fn spawn(&mut self, emitter_type: &str, position: Vec3) -> usize {
        let idx = self.emitters.len();
        self.emitters.push(Emitter::new(emitter_type.to_string(), position));
        idx
    }

    /// Stop an emitter (it will fade out as existing particles die).
    pub fn stop(&mut self, index: usize) {
        if let Some(e) = self.emitters.get_mut(index) {
            e.active = false;
        }
    }

    /// Advance all particles by dt seconds.
    pub fn tick(&mut self, dt: f32) {
        let defs = &self.emitter_defs;
        self.emitters.retain_mut(|emitter| {
            if let Some(def) = defs.get(&emitter.emitter_type) {
                emitter.tick(dt, def);
                // Remove emitters that are inactive with no live particles
                emitter.active || !emitter.particles.is_empty()
            } else {
                false
            }
        });
    }

    /// Total live particle count across all emitters.
    pub fn particle_count(&self) -> usize {
        self.emitters.iter().map(|e| e.particles.len()).sum()
    }

    /// First live emitter of a type (the ambient leaf/dust emitters are
    /// singletons managed by type, because emitter INDICES are unstable
    /// across tick()'s retain).
    pub fn emitter_by_type_mut(&mut self, t: &str) -> Option<&mut Emitter> {
        self.emitters.iter_mut().find(|e| e.emitter_type == t)
    }

    /// Shift every particle + emitter by a render-space delta (v0.966): the
    /// floating origin rebases render space around the camera, so
    /// world-anchored ambience must move opposite the ship each frame or it
    /// teleports with you (the camera-pinned test-light lesson).
    pub fn shift_all(&mut self, delta: Vec3) {
        for e in &mut self.emitters {
            e.position += delta;
            for p in &mut e.particles {
                p.position += delta;
            }
        }
    }

    /// Drop everything (an FTL jump moved render space too far to shift).
    pub fn clear(&mut self) {
        self.emitters.clear();
    }

    /// Collect vertices split by blend mode: (alpha, additive).
    pub fn collect_split(&self) -> (Vec<ParticleVertexData>, Vec<ParticleVertexData>) {
        let mut alpha = Vec::new();
        let mut additive = Vec::new();
        for e in &self.emitters {
            let def = self.emitter_defs.get(&e.emitter_type);
            let add = def.map(|d| d.blend_additive).unwrap_or(false);
            let streak = def.map(|d| d.streak_stretch).unwrap_or(0.0);
            let out = if add { &mut additive } else { &mut alpha };
            out.extend(e.collect_vertices(streak));
        }
        (alpha, additive)
    }

    /// Collect all particle vertices for GPU upload.
    pub fn collect_all_vertices(&self) -> Vec<ParticleVertexData> {
        let mut verts = Vec::with_capacity(self.particle_count());
        for emitter in &self.emitters {
            let streak = self
                .emitter_defs
                .get(&emitter.emitter_type)
                .map(|d| d.streak_stretch)
                .unwrap_or(0.0);
            verts.extend(emitter.collect_vertices(streak));
        }
        verts
    }
}


impl ParticleVertexData {
    /// Per-INSTANCE vertex layout for the billboard pipeline (v0.966): one
    /// instance per particle; the quad corners come from vertex_index.
    pub fn instance_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleVertexData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 28,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Build the particle billboard pipeline pair (alpha, additive) + the
/// frame-uniform bind group layout. Mirrors build_line_pipeline's shape:
/// reverse-Z depth test, no depth write, drawn as a post-pass onto the
/// rendered frame.
pub fn build_particle_pipelines(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    camera_bgl: &wgpu::BindGroupLayout,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Particle Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/particles.wgsl").into()),
    });
    let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Particle Frame BGL"),
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
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Particle Pipeline Layout"),
        bind_group_layouts: &[camera_bgl, &frame_bgl],
        push_constant_ranges: &[],
    });
    let build = |label: &str, blend: wgpu::BlendState| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[ParticleVertexData::instance_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            // Reverse-Z like the line pipeline: test against the scene,
            // never write (particles must not occlude each other).
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    };
    let alpha = build(
        "Particle Pipeline (alpha)",
        wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        },
    );
    let additive = build(
        "Particle Pipeline (additive)",
        wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        },
    );
    (alpha, additive, frame_bgl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> EmitterDef {
        EmitterDef { spawn_rate: 100.0, max_particles: 1000, ..Default::default() }
    }

    /// v0.1033 precipitation controls: the per-instance rate multiplier
    /// scales emission, and the direction override retargets the cone.
    #[test]
    fn rate_scale_and_dir_override_drive_the_instance() {
        let d = def();
        let mut full = Emitter::new("t".into(), Vec3::ZERO);
        let mut half = Emitter::new("t".into(), Vec3::ZERO);
        half.rate_scale = 0.5;
        full.tick(1.0, &d);
        half.tick(1.0, &d);
        assert_eq!(full.particles.len(), 100);
        assert_eq!(half.particles.len(), 50, "rate_scale must halve emission");
        // Override: emit straight -X; every velocity should lean -X.
        let mut tilted = Emitter::new("t".into(), Vec3::ZERO);
        tilted.dir_override = Some(Vec3::new(-1.0, 0.0, 0.0));
        tilted.tick(1.0, &d);
        for p in &tilted.particles {
            assert!(p.velocity.x < 0.0, "override ignored: {:?}", p.velocity);
        }
        // Zero scale = no emission at all (indoors-later posture).
        let mut off = Emitter::new("t".into(), Vec3::ZERO);
        off.rate_scale = 0.0;
        off.tick(1.0, &d);
        assert!(off.particles.is_empty());
    }

    /// v0.1036 streaks: the shipped defs parse with the new field (rain
    /// streaked, snow round via the serde default), and collected
    /// vertices carry velocity * streak seconds.
    #[test]
    fn shipped_defs_parse_streak_and_vertices_carry_it() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/particles.ron"),
        )
        .expect("particles.ron readable");
        let map: std::collections::HashMap<String, EmitterDefRon> =
            ron::from_str(&text).expect("particles.ron parses");
        assert!(map.get("rain").unwrap().streak_stretch > 0.0, "rain streaks");
        assert_eq!(map.get("snow").unwrap().streak_stretch, 0.0, "snow stays round");
        // Vertex packing: stretch = velocity * streak seconds.
        let mut d = def();
        d.speed_min = 10.0;
        d.speed_max = 10.0;
        d.direction = Vec3::new(0.0, -1.0, 0.0);
        d.spread_angle_deg = 0.0;
        let mut e = Emitter::new("t".into(), Vec3::ZERO);
        e.tick(0.1, &d);
        let verts = e.collect_vertices(0.05);
        assert!(!verts.is_empty());
        for v in &verts {
            let mag = (v.stretch[0].powi(2) + v.stretch[1].powi(2) + v.stretch[2].powi(2)).sqrt();
            assert!((mag - 0.5).abs() < 0.05, "10 m/s * 0.05 s should be ~0.5 m, got {mag}");
        }
        assert!(e.collect_vertices(0.0).iter().all(|v| v.stretch == [0.0; 3]));
    }
}

#[cfg(test)]
mod perf_probe {
    use super::*;

    /// MEASURED particle-budget probe (v0.1066). Ignored by default - it is a
    /// timing measurement, not an assertion, and timings do not belong in a
    /// gate. Run it deliberately:
    ///   cargo test --features native --lib perf_probe -- --ignored --nocapture
    ///
    /// It exists because "what is our particle ceiling" is a question the
    /// operator asks and the honest answer needs a number, not an estimate:
    /// draw submission is O(1) here (one instanced draw for all alpha
    /// particles), bandwidth is trivial, so the CPU update+collect loop is the
    /// real serial cost and this is what measures it.
    #[test]
    #[ignore]
    fn particle_update_cost_per_frame() {
        let def = EmitterDef {
            max_particles: 1_000_000,
            spawn_rate: 1.0e9,
            lifetime_min: 100.0,
            lifetime_max: 100.0,
            speed_min: 1.0,
            speed_max: 2.0,
            ..Default::default()
        };
        for n in [10_000usize, 100_000, 1_000_000] {
            let mut e = Emitter::new("probe".to_string(), Vec3::ZERO);
            e.cap_scale = 1.0;
            // Fill the pool DIRECTLY. Going through tick() cannot get there:
            // the v0.1065 steady-state clamp deliberately limits spawning to
            // cap/lifetime per second, which is the point of it.
            for _ in 0..n {
                e.spawn_particle(&def);
            }
            assert_eq!(e.particles.len(), n, "pool did not fill");
            let t0 = std::time::Instant::now();
            let frames = 30;
            let mut sink = 0usize;
            for _ in 0..frames {
                e.tick(1.0 / 60.0, &def);
                sink += e.collect_vertices(0.0).len();
            }
            let ms = t0.elapsed().as_secs_f64() * 1000.0 / frames as f64;
            println!(
                "[ParticleProbe] {:>9} particles -> {:.3} ms/frame update+collect ({} sink)",
                n, ms, sink
            );
        }
    }
}
