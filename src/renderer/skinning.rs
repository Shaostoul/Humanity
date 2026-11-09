//! SKINNED BODIES ON THE GPU (character animation rung 1, 2026-11-08;
//! docs/design/character-animation.md section 2.2).
//!
//! Two ways to bend a body, by distance (the tiers are chosen in the animation crate):
//!
//! - NEAR: a compute pass skins the body once a frame (`assets/shaders/skinning.wgsl`) into a
//!   vertex buffer in the ordinary mesh layout. Each of the body's regions (skin, hair, shirt, ...)
//!   is registered once as an ordinary `Mesh` over that buffer, so the frame draws a near body as a
//!   handful of plain `RenderObject`s: the sun shadow pass, the cascades and the colour pass all
//!   read the same skinned vertices, and no pass had to learn anything.
//! - MIDDLE: the vertex shader skins it (`assets/shaders/skinning_vs.wgsl`, appended to the PBR
//!   megashader for one PSO of its own, the surface class's fragment entry with the surface PSO's
//!   permutation), which costs no memory. Drawn after the opaque list in the scene passes (the live
//!   frame and the camera screens). Rung 1 gives middle bodies no sun shadow; rung 4's instanced
//!   crowd skinning adds a skinned shadow PSO beside this one.
//!
//! THE POSE PALETTE is one storage buffer, rewritten every frame: per body, its model matrix and
//! then each joint's skinning matrix, three `vec4` rows per matrix (48 bytes;
//! `humanity_animation::pose`). A near body's compute dispatch is told where its joints start; a
//! middle body's draw passes where its model matrix is as its first instance.
//!
//! NOTHING HERE IS REUSED ACROSS FRAMES as worked-out data: the palette and every near body's
//! skinned vertices are written afresh each frame. What persists is GPU allocation (a body's source
//! vertices, a near place's output buffer, the PSOs), which is a resource, not a result that can go
//! stale. The VS PSO builds on a background thread so it never adds to the boot; until it is ready
//! (and after a scene-format A/B switch, until it is rebuilt) middle bodies are skinned by compute
//! like near ones, so a body is never missing.

use super::*;
use super::deferred_build::{Deferred, Polled};
use humanity_core::skin_data::SkinnedModelData;
use humanity_core::vertex::SkinnedVertex;
use wgpu::util::DeviceExt;

/// Bytes per palette row (one `vec4<f32>`).
const ROW_BYTES: u64 = 16;
/// Bytes per skinned output vertex (the ordinary `Vertex`: 8 floats).
const OUT_VERTEX_BYTES: u64 = 32;
/// The compute shader's workgroup width (`@workgroup_size(64)`).
const WORKGROUP: u32 = 64;
/// The skinned vertex buffer layout of the VS path, matching `SkinnedVertexInput`.
const SKINNED_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Uint16x4, 4 => Unorm16x4
];

/// One body uploaded once: its skinned vertices (read as a storage buffer by the compute pass and
/// as a vertex buffer by the VS path) and an index buffer per region, offset into the one vertex
/// list.
pub struct SkinnedBody {
    vertices: wgpu::Buffer,
    vertex_count: u32,
    joint_count: u32,
    /// (index buffer, index count) per region, in the model's primitive order.
    regions: Vec<(wgpu::Buffer, u32)>,
}

/// A near place: one body's compute-skinned vertices for this frame, and its regions registered as
/// ordinary meshes over them.
struct NearSlot {
    body: usize,
    params: wgpu::Buffer,
    bind: wgpu::BindGroup,
    out: wgpu::Buffer,
    meshes: Vec<usize>,
}

/// A middle body's region to draw this frame.
#[derive(Clone, Copy)]
struct MiddleDraw {
    body: usize,
    region: usize,
    material: usize,
    model_index: u32,
}

/// The VS PSO and what it was built for.
struct SkinnedPso {
    pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
}

/// Everything the skinned path keeps on the renderer.
pub struct Skinning {
    bodies: Vec<SkinnedBody>,
    compute: wgpu::ComputePipeline,
    compute_layout: wgpu::BindGroupLayout,
    palette: wgpu::Buffer,
    palette_rows: u64,
    staging: Vec<[f32; 4]>,
    slots: Vec<NearSlot>,
    /// (slot, joint base) of each near body skinned this frame.
    jobs: Vec<(usize, u32)>,
    /// This frame's dispatches are queued and not yet recorded (`encode_skinning`).
    dispatch_pending: std::cell::Cell<bool>,
    middle: Vec<MiddleDraw>,
    /// Group 1 of the VS PSO: the identity object uniform and the palette.
    vs_layout: wgpu::BindGroupLayout,
    vs_object: wgpu::Buffer,
    vs_bind: wgpu::BindGroup,
    vs_pso: Deferred<SkinnedPso>,
}

impl Renderer {
    /// Upload a body (its mesh as read from the glTF) and return its id. The first call builds the
    /// compute pipeline and starts the VS PSO's background build.
    pub fn skin_register_body(&mut self, model: &SkinnedModelData) -> usize {
        if self.skinning.is_none() {
            self.skinning = Some(Box::new(Skinning::new(self)));
        }
        let mut vertices: Vec<SkinnedVertex> = Vec::with_capacity(model.vertex_count());
        let mut regions = Vec::with_capacity(model.primitives.len());
        for prim in &model.primitives {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&prim.vertices);
            let indices: Vec<u32> = prim.indices.iter().map(|i| i + base).collect();
            let ib = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Skinned Body Region Indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            regions.push((ib, indices.len() as u32));
        }
        // A storage binding may not be empty.
        if vertices.is_empty() {
            vertices.push(bytemuck::Zeroable::zeroed());
        }
        let vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skinned Body Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        });
        let sk = self.skinning.as_mut().expect("made above");
        sk.bodies.push(SkinnedBody { vertices: vb, vertex_count: vertices.len() as u32, joint_count: model.joints.len() as u32, regions });
        sk.bodies.len() - 1
    }

    /// Forget every body and near place (a reload of the character data). The meshes the near
    /// places registered stay in the mesh registry, unused, as every replaced mesh does.
    pub fn skin_clear_bodies(&mut self) {
        if let Some(sk) = self.skinning.as_mut() {
            sk.bodies.clear();
            sk.slots.clear();
            sk.jobs.clear();
            sk.middle.clear();
        }
    }

    /// Start a frame's skinning: nothing queued, the palette empty.
    pub fn skin_begin_frame(&mut self) {
        if let Some(sk) = self.skinning.as_mut() {
            sk.staging.clear();
            sk.jobs.clear();
            sk.middle.clear();
            sk.dispatch_pending.set(false);
        }
    }

    /// Append one body's matrices to this frame's palette: its model matrix's rows, then its joints'
    /// rows (three per joint). Returns the model matrix's index; its joints start one after.
    pub fn skin_push_palette(&mut self, model_rows: [[f32; 4]; 3], joint_rows: &[[f32; 4]]) -> u32 {
        let Some(sk) = self.skinning.as_mut() else { return 0 };
        let index = (sk.staging.len() / 3) as u32;
        sk.staging.extend_from_slice(&model_rows);
        sk.staging.extend_from_slice(joint_rows);
        index
    }

    /// Skin a body by compute this frame in near place `slot` (made on first use, for this body),
    /// with its matrices at `model_index` in the palette. Returns its regions' mesh ids, to draw as
    /// ordinary objects placed by the body's transform.
    pub fn skin_near(&mut self, body: usize, slot: usize, model_index: u32) -> Vec<usize> {
        let fresh = match self.skinning.as_ref() {
            None => return Vec::new(),
            Some(sk) => {
                if body >= sk.bodies.len() {
                    return Vec::new();
                }
                sk.slots.get(slot).map_or(true, |s| s.body != body)
            }
        };
        // Places are numbered 0, 1, 2 ... by the engine each frame; a place keeps its buffers while
        // it skins the same body and is remade when the body changes (a reload).
        let mut slot = slot;
        if fresh {
            let made = self.make_near_slot(body);
            let sk = self.skinning.as_mut().expect("checked above");
            if slot < sk.slots.len() {
                sk.slots[slot] = made;
            } else {
                sk.slots.push(made);
                slot = sk.slots.len() - 1;
            }
        }
        let sk = self.skinning.as_mut().expect("checked above");
        sk.jobs.push((slot, model_index + 1));
        sk.slots[slot].meshes.clone()
    }

    /// Draw a middle body's region this frame with the VS PSO, or, while that PSO is not ready,
    /// say so (`false`) so the caller skins it by compute instead.
    pub fn skin_middle_ready(&mut self) -> bool {
        let format = self.scene_format();
        let Some(sk) = self.skinning.as_mut() else { return false };
        if let Polled::Lost = sk.vs_pso.poll() {
            log::warn!("Skinning: the vertex-shader skinning PSO build was lost; middle bodies stay on compute skinning");
        }
        // Built for another scene format (the scene-format A/B switch): build it again for this one,
        // compute skinning standing in meanwhile.
        let stale = matches!(sk.vs_pso.ready(), Some(p) if p.format != format) && !sk.vs_pso.is_pending();
        if stale {
            let layout = sk.vs_layout.clone();
            let rebuilt = Skinning::spawn_vs_pso(self, &layout);
            if let Some(sk) = self.skinning.as_mut() {
                sk.vs_pso = rebuilt;
            }
            return false;
        }
        matches!(sk.vs_pso.ready(), Some(p) if p.format == format)
    }

    /// Queue one region of a middle body (call `skin_middle_ready` first).
    pub fn skin_middle(&mut self, body: usize, region: usize, material: usize, model_index: u32) {
        if let Some(sk) = self.skinning.as_mut() {
            sk.middle.push(MiddleDraw { body, region, material, model_index });
        }
    }

    /// Upload this frame's palette and queue the near bodies' compute skinning (recorded by the
    /// next scene pass, `encode_skinning`). Call once a frame after queueing and before any scene
    /// pass draws.
    pub fn skin_submit(&mut self) {
        let _cost = frame_costs::stage("cpu.skinning");
        let Some(sk) = self.skinning.as_mut() else { return };
        if sk.staging.is_empty() {
            return;
        }
        let rows = sk.staging.len() as u64;
        if rows > sk.palette_rows {
            let grown = rows.next_power_of_two().max(256);
            sk.palette = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Pose Palette"),
                size: grown * ROW_BYTES,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            sk.palette_rows = grown;
            // Every bind group naming the old palette is rebuilt over the new one.
            sk.vs_bind = Skinning::vs_bind_group(&self.device, &sk.vs_layout, &sk.vs_object, &sk.palette);
            for i in 0..sk.slots.len() {
                let body = &sk.bodies[sk.slots[i].body];
                let slot = &sk.slots[i];
                let bind = Skinning::compute_bind_group(&self.device, &sk.compute_layout, &sk.palette, &body.vertices, &slot.out, &slot.params);
                sk.slots[i].bind = bind;
            }
        }
        self.queue.write_buffer(&sk.palette, 0, bytemuck::cast_slice(&sk.staging));
        for &(slot, joint_base) in &sk.jobs {
            let s = &sk.slots[slot];
            let count = sk.bodies[s.body].vertex_count;
            self.queue.write_buffer(&s.params, 0, bytemuck::cast_slice(&[count, joint_base, 0, 0]));
        }
        // The dispatches themselves ride the scene pass's own encoder (`encode_skinning`), ahead of
        // its render pass: a submit of their own cost 0.46 ms of CPU a frame for one body (measured
        // 2026-11-08, low impact beside another game), where the upload alone costs microseconds.
        sk.dispatch_pending.set(!sk.jobs.is_empty());
    }

    /// Record this frame's compute skinning into `encoder`, once a frame: the first scene pass
    /// after `skin_submit` (the live view's, or a camera screen's when it comes first) carries it.
    /// No pass before the scene pass draws a near body (the celestial pass and its sun shadow draw
    /// their own list), so the vertices are skinned before anything reads them.
    pub(super) fn encode_skinning(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(sk) = self.skinning.as_ref() else { return };
        if !sk.dispatch_pending.replace(false) {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Skinning"),
            timestamp_writes: self.gpu_timers.as_ref().and_then(|t| t.compute_writes("gpu.skinning")),
        });
        pass.set_pipeline(&sk.compute);
        for &(slot, _) in &sk.jobs {
            let s = &sk.slots[slot];
            let count = sk.bodies[s.body].vertex_count;
            pass.set_bind_group(0, &s.bind, &[]);
            pass.dispatch_workgroups(count.div_ceil(WORKGROUP), 1, 1);
        }
    }

    /// Draw the queued middle bodies into a scene pass whose group 0 is already bound. Called by
    /// `render_scene_onto` after the opaque list.
    pub(super) fn draw_skinned_middle(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(sk) = self.skinning.as_ref() else { return };
        if sk.middle.is_empty() {
            return;
        }
        let Some(pso) = sk.vs_pso.ready() else { return };
        if pso.format != self.scene_format() {
            return;
        }
        pass.set_pipeline(&pso.pipeline);
        pass.set_bind_group(1, &sk.vs_bind, &[]);
        let mut bound_material = usize::MAX;
        let mut bound_body = usize::MAX;
        for d in &sk.middle {
            let Some(body) = sk.bodies.get(d.body) else { continue };
            let Some((ib, count)) = body.regions.get(d.region) else { continue };
            let Some(material) = self.materials.get(d.material) else { continue };
            if bound_material != d.material {
                bound_material = d.material;
                pass.set_bind_group(2, &material.bind_group, &[]);
                pass.set_bind_group(3, material.albedo_group().unwrap_or(&self.default_texture_bind_group), &[]);
            }
            if bound_body != d.body {
                bound_body = d.body;
                pass.set_vertex_buffer(0, body.vertices.slice(..));
            }
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*count, 0, d.model_index..d.model_index + 1);
        }
    }

    /// How many joints a registered body has (the palette rows it needs are three per joint).
    pub fn skin_joint_count(&self, body: usize) -> u32 {
        self.skinning.as_ref().and_then(|s| s.bodies.get(body)).map_or(0, |b| b.joint_count)
    }

    fn make_near_slot(&mut self, body: usize) -> NearSlot {
        let sk = self.skinning.as_ref().expect("a body was registered");
        let b = &sk.bodies[body];
        let out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Skinned Body Output"),
            size: (b.vertex_count as u64).max(1) * OUT_VERTEX_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Skinning Params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = Skinning::compute_bind_group(&self.device, &sk.compute_layout, &sk.palette, &b.vertices, &out, &params);
        // Buffer handles are shared clones (wgpu 24), so the region meshes and this place draw the
        // same GPU memory; collected before the registry is borrowed to add them.
        let regions: Vec<(wgpu::Buffer, u32)> = b.regions.iter().map(|(ib, n)| (ib.clone(), *n)).collect();
        let meshes = regions
            .into_iter()
            .map(|(index_buffer, index_count)| self.add_mesh(Mesh { vertex_buffer: out.clone(), index_buffer, index_count }))
            .collect();
        NearSlot { body, params, bind, out, meshes }
    }
}

impl Skinning {
    fn new(r: &Renderer) -> Self {
        let device = &r.device;
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Skinning Layout"),
            entries: &[
                storage_entry(0, wgpu::ShaderStages::COMPUTE, true),
                storage_entry(1, wgpu::ShaderStages::COMPUTE, true),
                storage_entry(2, wgpu::ShaderStages::COMPUTE, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: wgpu::BufferSize::new(16) },
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Skinning Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(COMPUTE_WGSL)),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Skinning Pipeline Layout"),
            bind_group_layouts: &[&compute_layout],
            push_constant_ranges: &[],
        });
        let compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Skinning Pipeline"),
            layout: Some(&layout),
            module: &module,
            entry_point: Some("cs_skin"),
            compilation_options: Default::default(),
            cache: None,
        });
        let palette_rows = 256;
        let palette = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Pose Palette"),
            size: palette_rows * ROW_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vs_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skinned Object Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<ObjectUniforms>() as u64),
                    },
                    count: None,
                },
                storage_entry(1, wgpu::ShaderStages::VERTEX, true),
            ],
        });
        let identity = ObjectUniforms { model: Mat4::IDENTITY.to_cols_array_2d(), normal_matrix: Mat4::IDENTITY.to_cols_array_2d() };
        let vs_object = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skinned Object Identity"),
            contents: bytemuck::bytes_of(&identity),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let vs_bind = Self::vs_bind_group(device, &vs_layout, &vs_object, &palette);
        let vs_pso = Self::spawn_vs_pso(r, &vs_layout);
        Skinning {
            bodies: Vec::new(),
            compute,
            compute_layout,
            palette,
            palette_rows,
            staging: Vec::new(),
            slots: Vec::new(),
            jobs: Vec::new(),
            dispatch_pending: std::cell::Cell::new(false),
            middle: Vec::new(),
            vs_layout,
            vs_object,
            vs_bind,
            vs_pso,
        }
    }

    fn compute_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        palette: &wgpu::Buffer,
        source: &wgpu::Buffer,
        out: &wgpu::Buffer,
        params: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Skinning Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: palette.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: source.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: out.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: params.as_entire_binding() },
            ],
        })
    }

    fn vs_bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, object: &wgpu::Buffer, palette: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skinned Object Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: object.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: palette.as_entire_binding() },
            ],
        })
    }

    /// Build the VS PSO on a thread of its own: the megashader with `vs_skinned` appended, the
    /// surface class's fragment entry and permutation (its own Surface-class row in PSO_REGISTRY), the opaque
    /// state, and the class PSOs' groups 0, 2 and 3 around this path's own group 1.
    fn spawn_vs_pso(r: &Renderer, vs_layout: &wgpu::BindGroupLayout) -> Deferred<SkinnedPso> {
        let device = r.device.clone();
        let format = r.scene_format();
        let camera = r.pipeline.camera_bind_group_layout.clone();
        let material = r.pipeline.material_bind_group_layout.clone();
        let texture = r.pipeline.texture_bind_group_layout.clone();
        let object = vs_layout.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("skinned-pso".into())
            .stack_size(pipeline::COMPILE_STACK_BYTES)
            .spawn(move || {
                let started = std::time::Instant::now();
                let source = format!("{}\n{}", super::shader_loader::boot_pbr_source(), SKINNED_VS_WGSL);
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("PBR-lite Skinned Shader"),
                    source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(source)),
                });
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("PBR-lite Skinned Pipeline Layout"),
                    bind_group_layouts: &[&camera, &object, &material, &texture],
                    push_constant_ranges: &[],
                });
                let pipeline = pipeline::Pipeline::build_skinned_surface_pso(&device, format, &module, &layout, &SKINNED_ATTRS, SKINNED_STRIDE);
                log::info!("[Pipelines] vertex-shader skinning PSO built in {:.1}s (background)", started.elapsed().as_secs_f32());
                let _ = tx.send(SkinnedPso { pipeline, format });
            });
        match spawned {
            Ok(_) => Deferred::waiting(rx),
            Err(e) => {
                log::warn!("Skinning: could not start the skinned PSO build ({e}); middle bodies use compute skinning");
                Deferred::empty()
            }
        }
    }
}

/// The SkinnedVertex stride the VS path reads.
const SKINNED_STRIDE: u64 = SkinnedVertex::BYTES as u64;

fn storage_entry(binding: u32, visibility: wgpu::ShaderStages, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

const COMPUTE_WGSL: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/skinning.wgsl"));
const SKINNED_VS_WGSL: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/skinning_vs.wgsl"));

#[cfg(test)]
mod tests {
    use super::*;

    /// Both skinning shaders parse and validate as wgpu will compile them: the compute shader on
    /// its own, the VS entry appended to the shipped megashader (it must name the megashader's
    /// `VertexOutput`, `camera` and group-1 numbering exactly).
    /// Seen red 2026-11-08 with `vs_skinned` writing `out.pack` as a vec3 (a type error naga
    /// rejects; wgpu would have refused the PSO at boot).
    #[test]
    fn both_skinning_shaders_validate() {
        let parse = |src: &str| -> Result<wgpu::naga::Module, String> {
            let module = wgpu::naga::front::wgsl::parse_str(src).map_err(|e| format!("{e}"))?;
            wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all())
                .validate(&module)
                .map_err(|e| format!("{e:?}"))?;
            Ok(module)
        };
        let c = parse(COMPUTE_WGSL).expect("the compute skinning shader validates");
        assert!(c.entry_points.iter().any(|e| e.name == "cs_skin"));
        let full = format!("{}\n{}", super::super::shader_loader::assembled_pbr_source(), SKINNED_VS_WGSL);
        let v = parse(&full).expect("the megashader with vs_skinned validates");
        assert!(v.entry_points.iter().any(|e| e.name == "vs_skinned"));
    }

    /// The VS path's vertex layout reads a SkinnedVertex where its fields are: joints at byte 32 as
    /// four u16, weights at 40 as four unorm16, a 48-byte stride.
    /// Seen red 2026-11-08 with the weights declared Float32x4 (the layout would have run past
    /// the vertex's 48 bytes).
    #[test]
    fn the_skinned_vertex_layout_matches_the_skinned_vertex() {
        let end = |a: &wgpu::VertexAttribute| a.offset + a.format.size();
        assert_eq!(SKINNED_ATTRS[3].offset, std::mem::offset_of!(SkinnedVertex, joints) as u64);
        assert_eq!(SKINNED_ATTRS[3].format, wgpu::VertexFormat::Uint16x4);
        assert_eq!(SKINNED_ATTRS[4].offset, std::mem::offset_of!(SkinnedVertex, weights) as u64);
        assert_eq!(SKINNED_ATTRS[4].format, wgpu::VertexFormat::Unorm16x4);
        assert_eq!(SKINNED_ATTRS.iter().map(end).max(), Some(SKINNED_STRIDE));
    }
}
