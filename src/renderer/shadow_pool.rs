//! THE MIDDLE TIER'S SHADOW POOL (2026-11-14, rung 4 of the room light shadows;
//! docs/design/local-light-shadows.md section 10, "Built: the film pyramid as one compute pass, the
//! swaying leaves and the middle tier on the GPU").
//!
//! The operator, 2026-11-13: "The CPU cost? Shouldn't shadows and all other renderer stuff be on the
//! GPU?" After rung 3 the crowd's shadows still cost about 540 draws a frame on the processor: the
//! MIDDLE-TIER bodies (and the first-person body's unfolded twin) are skinned in the vertex stage by
//! the main view, so there were no skinned vertices for the GPU-driven caster path to cast from, and
//! the atlas drew each of their regions once per shadow face through the skinned shadow PSOs
//! (`Renderer::draw_skinned_shadow`), skinning the body again every time.
//!
//! HOW. Each middle body that casts into some shadow view this frame (within a picked lamp's reach, or
//! in the near sun tile) is skinned ONCE by the compute skinning shader (assets/shaders/skinning.wgsl,
//! the near bodies' own) into this pool: one vertex buffer in the ordinary mesh layout, each person a
//! range of it (`plan`), the copy in the body's own space with no motion target (`skin_params`'s
//! `no_prev`). Its regions then join the GPU-driven caster path as casters like any other: the source
//! is the pool and the body's index buffer (a body's regions are ranges of one buffer since this rung),
//! the model matrix the palette's, the bounds the generous body sphere, plain regions in a plain set
//! and card hair in a cutout set, culled and drawn per face by the GPU.
//!
//! THE RED TWIN: showcase `shadow_gpu_crowd` "0" (`LocalShadows::crowd_off`) leaves every middle body
//! on the per-draw loop, as merged before this rung.
//!
//! NOTHING KEPT ACROSS FRAMES as worked-out data: the plan, the jobs and the skinned copies are made
//! afresh each frame, the bind groups made every frame. The pool's buffers are resources (grown to the
//! largest crowd met, a power of two, up to the device's storage binding limit; a person past it
//! stays on the per-draw loop).

use super::*;

/// Bytes of one pooled vertex (the ordinary `Vertex`: what the plain and cutout sets read).
const POOL_VERTEX_BYTES: u64 = 32;
/// One job's numbers in the params buffer: a uniform binding's dynamic offset must be a multiple of
/// the device's alignment, which is at most 256.
const PARAMS_SLOT: u64 = 256;
/// Workgroup size of `cs_skin`.
const WORKGROUP: u32 = 64;

/// One person skinned into the pool this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolJob {
    pub body: usize,
    pub model_index: u32,
    pub vertex_count: u32,
    pub vertex_base: u32,
    /// Where its copy starts in the pool.
    pub out_base: u32,
}

/// THE PLAN: each person's range of the pool, in the order met, packed end to end. A person whose
/// range would pass `max_vertices` gets None (it stays on the per-draw loop); the total is the
/// vertices the pool must hold. Pure, so it is tested without a GPU.
pub fn plan(vertex_counts: &[u32], max_vertices: u32) -> (Vec<Option<u32>>, u32) {
    let mut at = 0u32;
    let bases = vertex_counts
        .iter()
        .map(|&n| match at.checked_add(n) {
            Some(end) if end <= max_vertices => {
                let base = at;
                at = end;
                Some(base)
            }
            _ => None,
        })
        .collect();
    (bases, at)
}

/// The pool's GPU side, kept on `LocalShadows`.
pub struct ShadowPool {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    /// The skinned copies (`POOL_VERTEX_BYTES` each), grown as needed.
    out: Option<wgpu::Buffer>,
    /// Every job's numbers, one `PARAMS_SLOT` each, written in one upload a frame.
    params: Option<wgpu::Buffer>,
    /// What binding 6 (the last-pose copy) holds for a shadow job, which never writes it.
    no_prev: wgpu::Buffer,
    /// This frame's jobs (`plan_jobs`), recorded by `encode`.
    jobs: Vec<PoolJob>,
    /// The most vertices the pool may hold (the device's storage binding limit).
    max_vertices: u32,
}

impl ShadowPool {
    pub fn new(device: &wgpu::Device) -> Self {
        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        // The near places' layout (renderer/skinning.rs) with the numbers at a dynamic offset, so one
        // params buffer serves every job.
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Pool Skinning Layout"),
            entries: &[
                storage(0, true),
                storage(1, true),
                storage(2, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(skinning::SKIN_PARAMS_BYTES) },
                    count: None,
                },
                storage(4, true),
                storage(5, true),
                storage(6, false),
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Shadow Pool Skinning Shader"), source: wgpu::ShaderSource::Wgsl(Renderer::skin_compute_source().into()) });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("Shadow Pool Skinning Pipeline Layout"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("Shadow Pool Skinning Pipeline"), layout: Some(&pl), module: &module, entry_point: Some("cs_skin"), compilation_options: Default::default(), cache: None });
        let no_prev = device.create_buffer(&wgpu::BufferDescriptor { label: Some("Shadow Pool No Motion Target"), size: 256, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let limits = device.limits();
        let max_bytes = (limits.max_storage_buffer_binding_size as u64).min(limits.max_buffer_size);
        ShadowPool { layout, pipeline, out: None, params: None, no_prev, jobs: Vec::new(), max_vertices: (max_bytes / POOL_VERTEX_BYTES).min(u32::MAX as u64) as u32 }
    }

    /// The skinned copies' buffer (None before the first crowd).
    pub fn out(&self) -> Option<&wgpu::Buffer> {
        self.out.as_ref()
    }

    /// Video memory: the copies and the numbers.
    pub fn vram_bytes(&self) -> u64 {
        self.out.as_ref().map_or(0, |b| b.size()) + self.params.as_ref().map_or(0, |b| b.size())
    }

    /// This frame's people skinned into the pool.
    pub fn jobs(&self) -> &[PoolJob] {
        &self.jobs
    }

    /// No pool this frame (the path is off).
    pub fn forget_jobs(&mut self) {
        self.jobs.clear();
    }

    /// Plan this frame's pool for `people` ((body, model index, vertex count, vertex base), each person
    /// once), grow the buffers to hold it and write every job's numbers in one upload. Returns each
    /// person's range start (None: past the pool's reach, left on the per-draw loop).
    pub fn plan_jobs(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, people: &[(usize, u32, u32, u32)]) -> Vec<Option<u32>> {
        self.jobs.clear();
        let counts: Vec<u32> = people.iter().map(|p| p.2).collect();
        let (bases, total) = plan(&counts, self.max_vertices);
        if total == 0 {
            return bases;
        }
        let want = (total as u64 * POOL_VERTEX_BYTES).next_power_of_two().min(self.max_vertices as u64 * POOL_VERTEX_BYTES);
        if self.out.as_ref().is_none_or(|b| b.size() < total as u64 * POOL_VERTEX_BYTES) {
            self.out = Some(device.create_buffer(&wgpu::BufferDescriptor { label: Some("Shadow Pool Vertices"), size: want, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false }));
            log::info!("[Shadows] the shadow pool grew to {} vertices ({:.1} MB) for {} middle-tier bodies", want / POOL_VERTEX_BYTES, want as f64 / 1_048_576.0, people.len());
        }
        for (&(body, model_index, vertex_count, vertex_base), base) in people.iter().zip(&bases) {
            if let Some(out_base) = *base {
                self.jobs.push(PoolJob { body, model_index, vertex_count, vertex_base, out_base });
            }
        }
        let bytes = self.jobs.len() as u64 * PARAMS_SLOT;
        if self.params.as_ref().is_none_or(|b| b.size() < bytes) {
            self.params = Some(device.create_buffer(&wgpu::BufferDescriptor { label: Some("Shadow Pool Skinning Params"), size: bytes.next_power_of_two().max(PARAMS_SLOT), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }));
        }
        let mut staging = vec![0u8; bytes as usize];
        for (k, j) in self.jobs.iter().enumerate() {
            let p = skinning::skin_params(j.vertex_count, j.model_index, j.vertex_base, j.out_base, true);
            let at = k * PARAMS_SLOT as usize;
            staging[at..at + skinning::SKIN_PARAMS_BYTES as usize].copy_from_slice(bytemuck::cast_slice(&p));
        }
        if let Some(params) = self.params.as_ref() {
            queue.write_buffer(params, 0, &staging);
        }
        bases
    }
}

impl Renderer {
    /// THE MIDDLE TIER ON THE GPU PATH (`render_shadow_atlas`, beside the scene lists' casters): every
    /// middle-tier and shadow-only draw whose body sphere `reaches` a shadow view becomes a GPU-driven
    /// caster drawn from the pool, its person queued to be skinned there once; the draws taken are
    /// marked so the per-draw loop leaves them out. A region whose material needs a test the GPU sets
    /// do not have, or whose set would pass `shadow_gpu::MAX_SETS`, stays on the per-draw loop.
    /// Returns how many casters it added.
    pub(super) fn gather_pool_casters(&mut self, casters: &mut Vec<Caster>, gpu_sets: &mut Vec<shadow_gpu::DrawSet>, gpu_sources: &mut Vec<(wgpu::Buffer, wgpu::Buffer)>, reaches: impl Fn(Vec3, f32) -> bool) -> u32 {
        let draws = self.skin_shadow_draws();
        let total = draws.last().map_or(0, |d| d.draw + 1);
        let mut on_gpu = vec![false; total];
        // Each draw's set kind, or None (the per-draw loop keeps it): a plain region is depth only,
        // as the skinned shadow PSO draws it; a cut one only when the cutout sets have its test.
        let kinds: Vec<Option<shadow_gpu::SetKind>> = draws
            .iter()
            .map(|d| {
                if !reaches(d.centre, d.radius) {
                    return None;
                }
                let mat = self.materials.get(d.material)?;
                if !mat.casts_cutout_shadow() {
                    Some(shadow_gpu::SetKind::Plain)
                } else if shadow_gpu::gpu_cutout_type(mat.material_type) && takes_fast_path(mat.material_type, false) {
                    Some(shadow_gpu::SetKind::Cutout(d.material))
                } else {
                    None
                }
            })
            .collect();
        // Each person once, in the order met.
        let mut people: Vec<(usize, u32, u32, u32)> = Vec::new();
        let mut person_of: Vec<usize> = vec![usize::MAX; draws.len()];
        for (i, d) in draws.iter().enumerate() {
            if kinds[i].is_none() {
                continue;
            }
            person_of[i] = match people.iter().position(|p| p.0 == d.body && p.1 == d.model_index) {
                Some(k) => k,
                None => {
                    people.push((d.body, d.model_index, d.vertex_count, d.vertex_base));
                    people.len() - 1
                }
            };
        }
        let bases = self.local_shadows.pool.plan_jobs(&self.device, &self.queue, &people);
        let Some(out) = self.local_shadows.pool.out().cloned() else {
            self.skin_set_shadow_gpu(on_gpu);
            return 0;
        };
        let mut added = 0u32;
        for (i, d) in draws.iter().enumerate() {
            let (Some(kind), Some(Some(out_base))) = (kinds[i], person_of.get(i).and_then(|&k| bases.get(k))) else { continue };
            let pair = (&out, &d.indices);
            let source = match gpu_sources.iter().position(|(v, ix)| (v, ix) == pair) {
                Some(k) => k as u32 + 1,
                None => {
                    gpu_sources.push((out.clone(), d.indices.clone()));
                    gpu_sources.len() as u32
                }
            };
            let ds = shadow_gpu::DrawSet { kind, source };
            let set = match gpu_sets.iter().position(|s| *s == ds) {
                Some(k) => k as u32,
                None if gpu_sets.len() < shadow_gpu::MAX_SETS => {
                    gpu_sets.push(ds);
                    gpu_sets.len() as u32 - 1
                }
                None => continue,
            };
            let Some(mat) = self.materials.get(d.material) else { continue };
            let cutout = matches!(kind, shadow_gpu::SetKind::Cutout(_));
            let extra = if cutout { [mat.material_type, mat.params2[2], 0.0, 0.0] } else { [0.0; 4] };
            let obj = RenderObject { position: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE, mesh: usize::MAX, material: d.material, fade: 0.0 };
            let batch = shadow_gpu::CullBatch { index_count: d.index_count, first_index: d.first_index, base_vertex: *out_base as i32, ..Default::default() };
            casters.push(Caster { obj, centre: d.centre, radius: d.radius, cutout, fast: false, sun_only: false, model: d.model.to_cols_array(), gpu: Some(GpuPick { set, batch: u32::MAX, extra }), partial: None, pool: Some(batch) });
            on_gpu[d.draw] = true;
            added += 1;
            if casters.len() >= MAX_OBJECTS {
                break;
            }
        }
        self.skin_set_shadow_gpu(on_gpu);
        added
    }

    /// Record this frame's pool skinning into `encoder` (the shadow atlas's, after the near bodies'
    /// skinning and before the cull and the draws that read the pool).
    pub(super) fn encode_shadow_pool(&self, encoder: &mut wgpu::CommandEncoder) {
        self.local_shadows.pool.encode_jobs(&self.device, encoder, |body| self.skin_job_buffers(body), self.compute_pass_timer("gpu.skinning"));
    }
}

impl ShadowPool {
    /// Record the planned jobs into `encoder` as one compute pass: a dispatch per person, its numbers
    /// at its slot's dynamic offset. `buffers_of(body)` is what a body's job binds (this frame's
    /// palette and last frame's, its source vertices, its morph targets).
    pub fn encode_jobs(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, buffers_of: impl Fn(usize) -> Option<[wgpu::Buffer; 4]>, timer: Option<wgpu::ComputePassTimestampWrites<'_>>) {
        let pool = self;
        let (Some(out), Some(params)) = (pool.out.as_ref(), pool.params.as_ref()) else { return };
        if pool.jobs.is_empty() {
            return;
        }
        // One group per body met this frame (its source vertices and morph targets; the palettes,
        // the pool and the numbers are every job's), made every frame.
        let mut groups: Vec<(usize, wgpu::BindGroup)> = Vec::new();
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Shadow Pool Skinning"), timestamp_writes: timer });
        pass.set_pipeline(&pool.pipeline);
        for (k, j) in pool.jobs.iter().enumerate() {
            let g = match groups.iter().position(|(b, _)| *b == j.body) {
                Some(g) => g,
                None => {
                    let Some([palette, prev_palette, vertices, morphs]) = buffers_of(j.body) else { continue };
                    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Shadow Pool Skinning BG"),
                        layout: &pool.layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: palette.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: vertices.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 2, resource: out.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: params, offset: 0, size: wgpu::BufferSize::new(skinning::SKIN_PARAMS_BYTES) }) },
                            wgpu::BindGroupEntry { binding: 4, resource: morphs.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 5, resource: prev_palette.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 6, resource: pool.no_prev.as_entire_binding() },
                        ],
                    });
                    groups.push((j.body, group));
                    groups.len() - 1
                }
            };
            pass.set_bind_group(0, &groups[g].1, &[(k as u64 * PARAMS_SLOT) as u32]);
            pass.dispatch_workgroups(j.vertex_count.div_ceil(WORKGROUP), 1, 1);
        }
    }
}

#[cfg(test)]
#[path = "shadow_pool_tests.rs"]
mod tests;
