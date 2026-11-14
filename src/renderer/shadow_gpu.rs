//! GPU-DRIVEN SHADOW CASTERS (2026-11-13; docs/design/local-light-shadows.md section 10, "Built:
//! GPU-driven casters"). The graphics card builds its own draw list for every face of the shadow
//! atlas, so the processor sends one command per face instead of one draw per run of casters.
//!
//! WHY. The operator, 2026-11-13, told that room light shadows still cost 1.4 to 2.9 ms of the
//! processor at Full against a 1 ms target: "The CPU cost? Shouldn't shadows and all other renderer
//! stuff be on the GPU?" Rung 1b measured where the time goes: about 0.7 microseconds of D3D12
//! calls a draw (the vertex and index buffer views, the root constants, the draw itself), 900 to
//! 1,300 draws in the busiest views, and neither merging draws nor replaying them from a render
//! bundle removed those calls. Only fewer commands do, and GPU-driven rendering is the standard way
//! to get them: Haar and Aaltonen, "GPU-Driven Rendering Pipelines" (SIGGRAPH 2015) and Wihlidal,
//! "Optimizing the Graphics Pipeline with Compute" (GDC 2016), the technique virtual shadow maps
//! build on.
//!
//! THE PIECES.
//! - **The arena** (`ArenaBook`, the GPU buffers in `GpuCasters`): every plain caster's mesh copied
//!   once, on the graphics card, into ONE vertex buffer and ONE index buffer (ranges from the terrain
//!   arena's allocator, `patch_arena::RangeAlloc`), so a face's draws need no buffer change between
//!   them. A mesh is keyed by the serial the renderer gives it on `add_mesh` and `replace_mesh`
//!   (`Renderer::mesh_serial`): a new serial is copied again into a new range and the old range
//!   freed. With `humanity_core::reuse::enabled()` false the book forgets every range at the start of
//!   each frame and every mesh drawn is copied again.
//! - **The casters** (`CullCaster`): each plain caster's model matrix, bounding sphere and batch
//!   (one batch per arena mesh), uploaded once a frame; **the views** (`CullView`): each drawn face's
//!   planes, light, shadow reach and texel size, and the near sun tile's.
//! - **The cull** (`assets/shaders/shadow_cull.wgsl`): one compute pass of three dispatches tests
//!   every caster against every drawn view by the processor's own rule (`local_shadows::face_keeps`
//!   and `sun_keeps`; `gpu_keeps` is the shader's test written again here for the tests), counts
//!   each (view, mesh), gives each counted pair a run of the instance buffer and writes its
//!   `DrawIndexedIndirect` arguments, then writes the instances.
//! - **The draw**: per face its viewport and scissor and ONE `multi_draw_indexed_indirect_count`
//!   over the face's section of the argument buffer, through the fast path's own pipeline
//!   (`shadow_atlas_instanced.wgsl`: the instance's matrix is a vertex attribute, which the hardware
//!   offsets by the first instance in an indirect draw on every backend; the instance_index builtin
//!   would not see it on this project's DX12 adapter, `patch_arena`'s header).
//!
//! WHAT STAYS ON THE OLD LOOP. Everything the fast path did not take (cutouts, the swaying classes:
//! their shapes come from the megashader), and the compute-skinned near bodies: their vertices are
//! written again every frame by the skinning into the own-mesh pool (`mesh_pool`), so a copy in the
//! arena would be stale a frame later. The pool already holds every near body in one set of
//! buffers, so it can be its own arena in a later rung; until then a body is one draw per face it
//! falls in, a handful a frame. A mesh whose buffers cannot be copied (made without `COPY_SRC`)
//! stays on the old loop too, and the log line counts all of them.
//!
//! THE FALLBACK. The path needs three device features (`required_features`): multi-draw indirect,
//! its count variant and a first instance in indirect draws. A device without them (Metal in wgpu
//! 24, some older cards) draws every caster through the per-draw loop exactly as before
//! (`DrawPath::CpuNoFeature`), and Settings > Graphics says so in words under Room light shadows
//! (the hardware-notice rule). Showcase `shadow_gpu` "0" forces the old loop on any device: the red
//! twin, whose picture must equal this path's.

use super::patch_arena::RangeAlloc;
use glam::{Vec3, Vec4};

/// The shader (its own module, not the megashader: no `PSO_REGISTRY` row).
pub const CULL_WGSL: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/shadow_cull.wgsl"));

/// Bytes of one arena vertex: the mesh's own `Vertex`, copied whole so the fast path's pipeline
/// reads the arena exactly as it reads a mesh.
pub const VERTEX_BYTES: u64 = std::mem::size_of::<humanity_core::vertex::Vertex>() as u64;
/// Bytes of one `DrawIndexedIndirect` argument set.
pub const DRAW_ARGS_BYTES: u64 = 20;
/// Bytes of one caster instance (its model matrix, the view's index in column 0's w).
pub const INSTANCE_BYTES: u64 = 64;
/// The compute shader's workgroup width.
const WORKGROUP: u32 = 64;

/// The device features the GPU-driven path needs.
pub fn required_features() -> wgpu::Features {
    wgpu::Features::MULTI_DRAW_INDIRECT | wgpu::Features::MULTI_DRAW_INDIRECT_COUNT | wgpu::Features::INDIRECT_FIRST_INSTANCE
}

/// Which way the shadow atlas's plain casters are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawPath {
    /// The graphics card culls them and builds each face's draws (one command a face).
    #[default]
    Gpu,
    /// The device lacks a feature the path needs: the processor sends each draw (the old loop).
    CpuNoFeature,
    /// Showcase `shadow_gpu` "0": the old loop on a device that could run the path (the red twin).
    CpuPinned,
}

/// The path a device with `granted` features draws by, with the showcase pin.
pub fn draw_path(granted: wgpu::Features, pinned_off: bool) -> DrawPath {
    if !granted.contains(required_features()) {
        DrawPath::CpuNoFeature
    } else if pinned_off {
        DrawPath::CpuPinned
    } else {
        DrawPath::Gpu
    }
}

/// The three features, each with its id: the log names it, and the Settings notice reads its words
/// at `graphics.local_shadows_feature.<id>`.
pub const FEATURES: [(wgpu::Features, &str); 3] = [
    (wgpu::Features::MULTI_DRAW_INDIRECT, "multi_draw_indirect"),
    (wgpu::Features::MULTI_DRAW_INDIRECT_COUNT, "indirect_count"),
    (wgpu::Features::INDIRECT_FIRST_INSTANCE, "first_instance"),
];

/// The features a device lacks for the path, one bit each in `FEATURES`' order (0 = it has them all).
pub fn missing_mask(granted: wgpu::Features) -> u8 {
    FEATURES.iter().enumerate().filter(|(_, (f, _))| !granted.contains(*f)).fold(0u8, |m, (i, _)| m | 1 << i)
}

/// The ids of the features in `mask` (`missing_mask`).
pub fn mask_ids(mask: u8) -> Vec<&'static str> {
    FEATURES.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, (_, id))| *id).collect()
}

/// The ids of the features a device lacks for the path, for the log (empty when it has them all).
pub fn missing_features(granted: wgpu::Features) -> Vec<&'static str> {
    mask_ids(missing_mask(granted))
}

// ── What the shader reads (LOCKSTEP with shadow_cull.wgsl's structs) ────────────────────────────

/// The frame's constants: the camera's planes, the counts, the pinned cull and the filter margin.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullParams {
    pub cam: [[f32; 4]; 6],
    pub n_casters: u32,
    pub n_views: u32,
    pub n_batches: u32,
    pub cull_off: u32,
    pub margin: f32,
    pub instance_cap: u32,
    pub pad: [u32; 2],
}

/// One drawn view: a light's face or the near sun tile.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullView {
    pub planes: [[f32; 4]; 6],
    /// xyz the light (the eye for the sun tile), w the texel size per metre of depth (flat for the sun).
    pub origin: [f32; 4],
    /// x the light's shadow reach, y 1 for the orthographic sun tile.
    pub info: [f32; 4],
    /// x the view's index in the atlas's view array.
    pub index: [u32; 4],
}

impl CullView {
    /// A light's face: its planes, the light, its shadow reach and texel size per metre.
    pub fn face(planes: &[Vec4; 6], light: Vec3, reach: f32, texel_m: f32, index: u32) -> Self {
        CullView { planes: planes.map(|p| p.to_array()), origin: [light.x, light.y, light.z, texel_m], info: [reach, 0.0, 0.0, 0.0], index: [index, 0, 0, 0] }
    }

    /// The near sun tile: its planes and its flat texel size.
    pub fn sun(planes: &[Vec4; 6], eye: Vec3, texel_m: f32, index: u32) -> Self {
        CullView { planes: planes.map(|p| p.to_array()), origin: [eye.x, eye.y, eye.z, texel_m], info: [0.0, 1.0, 0.0, 0.0], index: [index, 0, 0, 0] }
    }
}

/// One plain caster: its model matrix, bounding sphere and batch.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullCaster {
    pub model: [f32; 16],
    /// xyz the sphere's centre in render space, w its radius (below 0 = unbounded).
    pub sphere: [f32; 4],
    /// x the batch, y 1 for a celestial piece that casts into the near sun tile only.
    pub tag: [u32; 4],
}

impl CullCaster {
    /// A caster from the processor's form (an infinite radius is unbounded).
    pub fn new(model: [f32; 16], centre: Vec3, radius: f32, batch: u32, sun_only: bool) -> Self {
        let r = if radius.is_finite() { radius } else { -1.0 };
        CullCaster { model, sphere: [centre.x, centre.y, centre.z, r], tag: [batch, u32::from(sun_only), 0, 0] }
    }
}

/// One batch: an arena mesh's index range and base vertex.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullBatch {
    pub index_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub pad: u32,
}

/// One `DrawIndexedIndirect` argument set, as the shader writes it and the GPU reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

/// THE SHADER'S TEST, written again in Rust line for line (`keeps` in shadow_cull.wgsl): whether
/// caster `c` is drawn into view `v`. The tests hold it equal to the processor's rule
/// (`local_shadows::face_keeps`, `sun_keeps`) on one scene, and the GPU's own result to it.
pub fn gpu_keeps(c: &CullCaster, v: &CullView, p: &CullParams) -> bool {
    let centre = Vec3::new(c.sphere[0], c.sphere[1], c.sphere[2]);
    let r = c.sphere[3];
    let o = Vec3::new(v.origin[0], v.origin[1], v.origin[2]);
    let texel_m = v.origin[3];
    let view_sphere_in = || r < 0.0 || v.planes.iter().all(|q| Vec3::new(q[0], q[1], q[2]).dot(centre) + q[3] >= -r);
    let big_enough = |ortho: bool| {
        if r < 0.0 {
            return true;
        }
        let texel = if ortho { texel_m } else { texel_m * (centre.distance(o) - r).max(0.05) };
        2.0 * r >= texel
    };
    if v.info[1] > 0.5 {
        return view_sphere_in() && big_enough(true);
    }
    if c.tag[1] != 0 {
        return false;
    }
    let reach = v.info[0];
    if r >= 0.0 && !(centre.distance(o) < reach + r) {
        return false;
    }
    if !(view_sphere_in() && big_enough(false)) {
        return false;
    }
    if p.cull_off != 0 || r < 0.0 {
        return true;
    }
    // shadow_reaches_view against the camera's planes.
    let to = centre - o;
    let d = to.length();
    if d <= r + 1e-4 {
        return true;
    }
    let pad = texel_m * p.margin;
    let far_d = reach.max(d);
    let far = o + to * (far_d / d);
    let (near_r, far_r) = (r + pad, r * far_d / d + pad);
    !p.cam.iter().any(|q| {
        let n = Vec3::new(q[0], q[1], q[2]);
        n.dot(centre) + q[3] < -near_r && n.dot(far) + q[3] < -far_r
    })
}

// ── The arena's bookkeeping (plain data, tested without a GPU) ──────────────────────────────────────

/// Each mesh slot's serial (2026-11-13): a new one on every `Renderer::add_mesh` and `replace_mesh`,
/// what the arena keys its copy of a mesh by, so a replaced mesh is copied again. Reuse: the version a
/// reuse is tagged with, not a reuse itself.
#[derive(Debug, Default)]
pub struct MeshSerials {
    serials: Vec<u64>,
    next: u64,
}

impl MeshSerials {
    /// A mesh was added at the next id.
    pub fn added(&mut self) {
        self.next += 1;
        self.serials.push(self.next);
    }

    /// The mesh at `idx` was replaced.
    pub fn replaced(&mut self, idx: usize) {
        self.next += 1;
        if let Some(s) = self.serials.get_mut(idx) {
            *s = self.next;
        }
    }

    /// The serial of the mesh at `idx` (0 for none).
    pub fn get(&self, idx: usize) -> u64 {
        self.serials.get(idx).copied().unwrap_or(0)
    }
}

// ── Where each mesh lives in the arena ────────────────────────────────────────────────────────────

/// Where one mesh lives in the arena: its vertices and indices, and the serial it was copied at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaSlot {
    pub serial: u64,
    pub vbase: u32,
    pub vcount: u32,
    pub ibase: u32,
    pub icount: u32,
}

impl ArenaSlot {
    /// The batch a draw of this mesh reads.
    pub fn batch(&self) -> CullBatch {
        CullBatch { index_count: self.icount, first_index: self.ibase, base_vertex: self.vbase as i32, pad: 0 }
    }
}

/// What `ArenaBook::place` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Already there at this serial: nothing to copy.
    Kept(ArenaSlot),
    /// Given a new range this frame: the caller copies the mesh in.
    New(ArenaSlot),
    /// No room: the caller grows the arena (`grow_for`) and asks again.
    Full,
}

/// The arena's bookkeeping: which mesh holds which ranges, by the mesh's id (its index in the
/// renderer's registry) and serial. The vertex and index caps grow by doubling.
#[derive(Debug, Clone)]
pub struct ArenaBook {
    valloc: RangeAlloc,
    ialloc: RangeAlloc,
    /// By mesh id (dense, as the registry's ids are).
    slots: Vec<Option<ArenaSlot>>,
    /// Meshes copied in since the last `take_new`.
    fresh: Vec<(usize, ArenaSlot)>,
}

impl ArenaBook {
    pub fn new(vcap: u32, icap: u32) -> Self {
        ArenaBook { valloc: RangeAlloc::new(vcap), ialloc: RangeAlloc::new(icap), slots: Vec::new(), fresh: Vec::new() }
    }

    pub fn caps(&self) -> (u32, u32) {
        (self.valloc.cap(), self.ialloc.cap())
    }

    /// Meshes in the arena now.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Start a frame. THE REUSE SWITCH: with reuse off every range is forgotten, so every mesh a
    /// caster draws this frame is placed and copied again.
    pub fn begin_frame(&mut self, reuse: bool) {
        if !reuse {
            let (v, i) = self.caps();
            self.valloc = RangeAlloc::new(v);
            self.ialloc = RangeAlloc::new(i);
            self.slots.iter_mut().for_each(|s| *s = None);
            self.fresh.clear();
        }
    }

    /// The slot for mesh `id` at `serial` with `vcount` vertices and `icount` indices: kept when it
    /// is there at the same serial (and reuse is on: `begin_frame` emptied the book otherwise), else
    /// its old ranges are freed and new ones taken.
    pub fn place(&mut self, id: usize, serial: u64, vcount: u32, icount: u32) -> Place {
        if let Some(Some(s)) = self.slots.get(id) {
            if s.serial == serial {
                return Place::Kept(*s);
            }
        }
        if let Some(old) = self.slots.get_mut(id).and_then(Option::take) {
            self.valloc.free(old.vbase, old.vcount);
            self.ialloc.free(old.ibase, old.icount);
        }
        let Some(vbase) = self.valloc.alloc(vcount) else { return Place::Full };
        let Some(ibase) = self.ialloc.alloc(icount) else {
            self.valloc.free(vbase, vcount);
            return Place::Full;
        };
        let slot = ArenaSlot { serial, vbase, vcount, ibase, icount };
        if self.slots.len() <= id {
            self.slots.resize(id + 1, None);
        }
        self.slots[id] = Some(slot);
        self.fresh.push((id, slot));
        Place::New(slot)
    }

    /// Grow the caps (doubling) until `vneed` vertices and `icount` indices more fit in one run each,
    /// keeping every range where it is (the GPU copies the old buffers into the new ones). Returns
    /// the new caps, or None past `max` elements of either (the caller leaves that mesh on the old
    /// loop).
    pub fn grow_for(&mut self, vneed: u32, ineed: u32, max_v: u32, max_i: u32) -> Option<(u32, u32)> {
        // Everything past the old caps is one free run, so doubling until that run alone holds the
        // mesh makes it fit however the room below is broken up.
        let (old_v, old_i) = self.caps();
        let (mut v, mut i) = (old_v.checked_mul(2)?, old_i.checked_mul(2)?);
        while v - old_v < vneed {
            v = v.checked_mul(2)?;
        }
        while i - old_i < ineed {
            i = i.checked_mul(2)?;
        }
        if v > max_v || i > max_i {
            return None;
        }
        // The new allocators: every held range taken again where it is, the rest (past the old caps)
        // free; the gaps below the old caps stay free.
        let mut va = RangeAlloc::new(v);
        let mut ia = RangeAlloc::new(i);
        for s in self.slots.iter().flatten() {
            va.alloc_at(s.vbase, s.vcount);
            ia.alloc_at(s.ibase, s.icount);
        }
        debug_assert_eq!(va.free_total(), v - (old_v - self.valloc.free_total()));
        debug_assert_eq!(ia.free_total(), i - (old_i - self.ialloc.free_total()));
        self.valloc = va;
        self.ialloc = ia;
        Some((v, i))
    }

    /// The meshes placed since the last call: what the GPU must copy in.
    pub fn take_new(&mut self) -> Vec<(usize, ArenaSlot)> {
        std::mem::take(&mut self.fresh)
    }

    /// The slot mesh `id` holds now.
    pub fn slot(&self, id: usize) -> Option<ArenaSlot> {
        self.slots.get(id).copied().flatten()
    }
}

/// Whether a mesh can live in the arena: its vertices stand still (a compute-skinned body's are
/// written again every frame, `prev_vertex_buffer`), its buffers are its own (not a range of a
/// shared pool) and the GPU may copy out of them.
pub fn arena_eligible(m: &super::mesh::Mesh) -> bool {
    m.prev_vertex_buffer.is_none()
        && m.first_index == 0
        && m.base_vertex == 0
        && m.index_count > 0
        && m.vertex_buffer.usage().contains(wgpu::BufferUsages::COPY_SRC)
        && m.index_buffer.usage().contains(wgpu::BufferUsages::COPY_SRC)
        && m.vertex_buffer.size() >= VERTEX_BYTES
}

// ── The GPU side ────────────────────────────────────────────────────────────────────────────────

/// The arena's starting caps: 1 M vertices (32 MB) and 4 M indices (16 MB), grown by doubling.
const ARENA_V0: u32 = 1 << 20;
const ARENA_I0: u32 = 1 << 22;

/// The GPU-driven casters' buffers and pipelines.
pub struct GpuCasters {
    /// The device has the features (`required_features`).
    pub available: bool,
    pub missing: Vec<&'static str>,
    pub missing_mask: u8,
    pipes: Option<CullPipes>,
    pub book: ArenaBook,
    arena_v: Option<wgpu::Buffer>,
    arena_i: Option<wgpu::Buffer>,
    /// The arena's buffers before a grow this frame, copied into the new ones first.
    grown_from: Option<(wgpu::Buffer, wgpu::Buffer)>,
    max_v: u32,
    max_i: u32,
    params: Option<wgpu::Buffer>,
    views: Option<wgpu::Buffer>,
    casters: Option<wgpu::Buffer>,
    batches: Option<wgpu::Buffer>,
    counters: Option<wgpu::Buffer>,
    draws: Option<wgpu::Buffer>,
    draw_counts: Option<wgpu::Buffer>,
    instances: Option<wgpu::Buffer>,
    group: Option<wgpu::BindGroup>,
    /// This frame's counts (0 views or 0 batches = nothing to cull or draw).
    pub n_views: u32,
    pub n_batches: u32,
    pub n_casters: u32,
}

struct CullPipes {
    layout: wgpu::BindGroupLayout,
    count: wgpu::ComputePipeline,
    emit: wgpu::ComputePipeline,
    write: wgpu::ComputePipeline,
}

impl CullPipes {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Shadow Cull Shader"), source: wgpu::ShaderSource::Wgsl(CULL_WGSL.into()) });
        let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let rw = wgpu::BufferBindingType::Storage { read_only: false };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Cull Layout"),
            entries: &[entry(0, wgpu::BufferBindingType::Uniform), entry(1, ro), entry(2, ro), entry(3, ro), entry(4, rw), entry(5, rw), entry(6, rw), entry(7, rw)],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("Shadow Cull Pipeline Layout"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipe = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        CullPipes { count: pipe("cull_count"), emit: pipe("emit_draws"), write: pipe("write_instances"), layout }
    }
}

/// Make a buffer of at least `bytes` (rounded to a power of two, never under 256), or keep `have`
/// when it is big enough. True when it was made again (the bind group must be too).
fn ensure(device: &wgpu::Device, have: &mut Option<wgpu::Buffer>, bytes: u64, label: &str, usage: wgpu::BufferUsages) -> bool {
    if have.as_ref().is_some_and(|b| b.size() >= bytes) {
        return false;
    }
    let size = bytes.max(256).next_power_of_two();
    *have = Some(device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false }));
    true
}

impl GpuCasters {
    pub fn new(device: &wgpu::Device) -> Self {
        let granted = device.features();
        let available = granted.contains(required_features());
        let max = device.limits().max_buffer_size;
        GpuCasters {
            available,
            missing: missing_features(granted),
            missing_mask: missing_mask(granted),
            pipes: available.then(|| CullPipes::new(device)),
            book: ArenaBook::new(ARENA_V0, ARENA_I0),
            arena_v: None,
            arena_i: None,
            grown_from: None,
            max_v: (max / VERTEX_BYTES).min(u32::MAX as u64) as u32,
            max_i: (max / 4).min(u32::MAX as u64) as u32,
            params: None,
            views: None,
            casters: None,
            batches: None,
            counters: None,
            draws: None,
            draw_counts: None,
            instances: None,
            group: None,
            n_views: 0,
            n_batches: 0,
            n_casters: 0,
        }
    }

    /// Put mesh `id` (at `serial`) in the arena, growing it when full; None when it cannot be (past
    /// the device's largest buffer): that caster stays on the old loop.
    pub fn place(&mut self, device: &wgpu::Device, id: usize, serial: u64, mesh: &super::mesh::Mesh) -> Option<ArenaSlot> {
        let vcount = (mesh.vertex_buffer.size() / VERTEX_BYTES) as u32;
        let icount = mesh.index_count;
        self.ensure_arena(device);
        match self.book.place(id, serial, vcount, icount) {
            Place::Kept(s) | Place::New(s) => Some(s),
            Place::Full => {
                let (v, i) = self.book.grow_for(vcount, icount, self.max_v, self.max_i)?;
                self.grow_buffers(device, v, i);
                match self.book.place(id, serial, vcount, icount) {
                    Place::Kept(s) | Place::New(s) => Some(s),
                    Place::Full => None,
                }
            }
        }
    }

    fn arena_buffer(device: &wgpu::Device, label: &str, bytes: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: bytes, usage: usage | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false })
    }

    fn ensure_arena(&mut self, device: &wgpu::Device) {
        if self.arena_v.is_none() {
            let (v, i) = self.book.caps();
            self.arena_v = Some(Self::arena_buffer(device, "Shadow Arena Vertices", v as u64 * VERTEX_BYTES, wgpu::BufferUsages::VERTEX));
            self.arena_i = Some(Self::arena_buffer(device, "Shadow Arena Indices", i as u64 * 4, wgpu::BufferUsages::INDEX));
            log::info!("[Shadows] GPU-driven casters: the arena made for {v} vertices and {i} indices ({:.0} MB)", (v as u64 * VERTEX_BYTES + i as u64 * 4) as f64 / 1_048_576.0);
        }
    }

    fn grow_buffers(&mut self, device: &wgpu::Device, v: u32, i: u32) {
        let new_v = Self::arena_buffer(device, "Shadow Arena Vertices", v as u64 * VERTEX_BYTES, wgpu::BufferUsages::VERTEX);
        let new_i = Self::arena_buffer(device, "Shadow Arena Indices", i as u64 * 4, wgpu::BufferUsages::INDEX);
        let old_v = self.arena_v.replace(new_v);
        let old_i = self.arena_i.replace(new_i);
        // Only the first grow of a frame keeps the old buffers: a second grow's "old" buffers are this
        // frame's new ones, still empty, whose ranges the first old pair holds.
        if self.grown_from.is_none() {
            if let (Some(ov), Some(oi)) = (old_v, old_i) {
                self.grown_from = Some((ov, oi));
            }
        }
        log::info!("[Shadows] GPU-driven casters: the arena grew to {v} vertices and {i} indices ({:.0} MB)", (v as u64 * VERTEX_BYTES + i as u64 * 4) as f64 / 1_048_576.0);
    }

    /// Upload this frame's casters, views and batches, sizing every buffer (the instance buffer for
    /// `instance_bound` pairs, the processor's count of every candidate pair, so it never overruns).
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mut params: CullParams, views: &[CullView], casters: &[CullCaster], batches: &[CullBatch], instance_bound: u64) {
        self.n_views = views.len() as u32;
        self.n_batches = batches.len() as u32;
        self.n_casters = casters.len() as u32;
        if self.pipes.is_none() || views.is_empty() || batches.is_empty() || casters.is_empty() {
            self.n_views = 0;
            return;
        }
        let vb = views.len() as u64 * batches.len() as u64;
        use wgpu::BufferUsages as U;
        let mut remade = false;
        remade |= ensure(device, &mut self.params, std::mem::size_of::<CullParams>() as u64, "Shadow Cull Params", U::UNIFORM | U::COPY_DST);
        remade |= ensure(device, &mut self.views, std::mem::size_of_val(views) as u64, "Shadow Cull Views", U::STORAGE | U::COPY_DST);
        remade |= ensure(device, &mut self.casters, std::mem::size_of_val(casters) as u64, "Shadow Cull Casters", U::STORAGE | U::COPY_DST);
        remade |= ensure(device, &mut self.batches, std::mem::size_of_val(batches) as u64, "Shadow Cull Batches", U::STORAGE | U::COPY_DST);
        remade |= ensure(device, &mut self.counters, (3 * vb + 1) * 4, "Shadow Cull Counters", U::STORAGE | U::COPY_DST | U::COPY_SRC);
        remade |= ensure(device, &mut self.draws, vb * DRAW_ARGS_BYTES, "Shadow Cull Draws", U::STORAGE | U::INDIRECT | U::COPY_SRC);
        remade |= ensure(device, &mut self.draw_counts, views.len() as u64 * 4, "Shadow Cull Draw Counts", U::STORAGE | U::INDIRECT | U::COPY_DST | U::COPY_SRC);
        remade |= ensure(device, &mut self.instances, instance_bound.max(1) * INSTANCE_BYTES, "Shadow Cull Instances", U::STORAGE | U::VERTEX | U::COPY_SRC);
        if remade || self.group.is_none() {
            let pipes = self.pipes.as_ref().expect("checked above");
            fn b(buf: &Option<wgpu::Buffer>) -> wgpu::BindingResource<'_> {
                buf.as_ref().expect("made above").as_entire_binding()
            }
            self.group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Shadow Cull BG"),
                layout: &pipes.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: b(&self.params) },
                    wgpu::BindGroupEntry { binding: 1, resource: b(&self.views) },
                    wgpu::BindGroupEntry { binding: 2, resource: b(&self.casters) },
                    wgpu::BindGroupEntry { binding: 3, resource: b(&self.batches) },
                    wgpu::BindGroupEntry { binding: 4, resource: b(&self.counters) },
                    wgpu::BindGroupEntry { binding: 5, resource: b(&self.draws) },
                    wgpu::BindGroupEntry { binding: 6, resource: b(&self.draw_counts) },
                    wgpu::BindGroupEntry { binding: 7, resource: b(&self.instances) },
                ],
            }));
        }
        params.n_casters = self.n_casters;
        params.n_views = self.n_views;
        params.n_batches = self.n_batches;
        params.instance_cap = (self.instances.as_ref().map_or(0, |b| b.size()) / INSTANCE_BYTES).min(u32::MAX as u64) as u32;
        let w = |buf: &Option<wgpu::Buffer>, bytes: &[u8]| queue.write_buffer(buf.as_ref().expect("made above"), 0, bytes);
        w(&self.params, bytemuck::bytes_of(&params));
        w(&self.views, bytemuck::cast_slice(views));
        w(&self.casters, bytemuck::cast_slice(casters));
        w(&self.batches, bytemuck::cast_slice(batches));
    }

    /// Record the arena's copies (a grow's old contents first, then this frame's new meshes from
    /// their own buffers), then the cull: the counters cleared, the three dispatches. Returns how many
    /// meshes were copied in (the log line's count: none on a still frame with reuse on).
    pub fn encode(&mut self, encoder: &mut wgpu::CommandEncoder, meshes: &[super::mesh::Mesh], timer: Option<wgpu::ComputePassTimestampWrites<'_>>) -> u32 {
        if let (Some((ov, oi)), Some(nv), Some(ni)) = (self.grown_from.take(), self.arena_v.as_ref(), self.arena_i.as_ref()) {
            encoder.copy_buffer_to_buffer(&ov, 0, nv, 0, ov.size());
            encoder.copy_buffer_to_buffer(&oi, 0, ni, 0, oi.size());
        }
        let fresh = self.book.take_new();
        let mut copied = 0u32;
        if let (Some(av), Some(ai)) = (self.arena_v.as_ref(), self.arena_i.as_ref()) {
            for (id, s) in fresh {
                // A slot freed again this frame (its mesh replaced twice) would copy into a range
                // someone else may hold: only the slot the book holds now is copied.
                if self.book.slot(id) != Some(s) {
                    continue;
                }
                let Some(m) = meshes.get(id) else { continue };
                encoder.copy_buffer_to_buffer(&m.vertex_buffer, 0, av, s.vbase as u64 * VERTEX_BYTES, s.vcount as u64 * VERTEX_BYTES);
                encoder.copy_buffer_to_buffer(&m.index_buffer, 0, ai, s.ibase as u64 * 4, s.icount as u64 * 4);
                copied += 1;
            }
        }
        if self.n_views == 0 {
            return copied;
        }
        let (Some(pipes), Some(group), Some(counters), Some(draw_counts)) = (self.pipes.as_ref(), self.group.as_ref(), self.counters.as_ref(), self.draw_counts.as_ref()) else { return copied };
        let vb = self.n_views as u64 * self.n_batches as u64;
        encoder.clear_buffer(counters, 0, Some((3 * vb + 1) * 4));
        encoder.clear_buffer(draw_counts, 0, Some(self.n_views as u64 * 4));
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Shadow Cull Pass"), timestamp_writes: timer });
        pass.set_bind_group(0, group, &[]);
        let caster_groups = self.n_casters.div_ceil(WORKGROUP);
        pass.set_pipeline(&pipes.count);
        pass.dispatch_workgroups(caster_groups, self.n_views, 1);
        pass.set_pipeline(&pipes.emit);
        pass.dispatch_workgroups(self.n_batches.div_ceil(WORKGROUP), self.n_views, 1);
        pass.set_pipeline(&pipes.write);
        pass.dispatch_workgroups(caster_groups, self.n_views, 1);
        copied
    }

    /// Draw view `g` (its index in this frame's `CullView`s): ONE indirect multi-draw over its
    /// section of the argument buffer, its count from the GPU. The caller has bound the fast path's
    /// pipeline and view group; this binds the arena and the instances (once a pass: `bound`).
    pub fn draw_view<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, g: u32, bound: &mut bool) -> bool {
        let (Some(av), Some(ai), Some(inst), Some(draws), Some(counts)) = (self.arena_v.as_ref(), self.arena_i.as_ref(), self.instances.as_ref(), self.draws.as_ref(), self.draw_counts.as_ref()) else { return false };
        if g >= self.n_views || self.n_batches == 0 {
            return false;
        }
        if !*bound {
            pass.set_vertex_buffer(0, av.slice(..));
            pass.set_vertex_buffer(1, inst.slice(..));
            pass.set_index_buffer(ai.slice(..), wgpu::IndexFormat::Uint32);
            *bound = true;
        }
        let stride = self.n_batches as u64 * DRAW_ARGS_BYTES;
        pass.multi_draw_indexed_indirect_count(draws, g as u64 * stride, counts, g as u64 * 4, self.n_batches);
        true
    }

    /// Video memory: the arena and the cull's buffers.
    pub fn vram_bytes(&self) -> u64 {
        [&self.arena_v, &self.arena_i, &self.params, &self.views, &self.casters, &self.batches, &self.counters, &self.draws, &self.draw_counts, &self.instances]
            .iter()
            .map(|b| b.as_ref().map_or(0, |b| b.size()))
            .sum()
    }

    /// For the tests: the buffers the cull writes.
    #[cfg(all(test, feature = "native"))]
    pub(crate) fn outputs(&self) -> (&wgpu::Buffer, &wgpu::Buffer, &wgpu::Buffer) {
        (self.draws.as_ref().unwrap(), self.draw_counts.as_ref().unwrap(), self.instances.as_ref().unwrap())
    }

    /// For the tests: the arena's vertex and index buffers.
    #[cfg(all(test, feature = "native"))]
    pub(crate) fn arena_buffers(&self) -> (&wgpu::Buffer, &wgpu::Buffer) {
        (self.arena_v.as_ref().unwrap(), self.arena_i.as_ref().unwrap())
    }
}

#[cfg(test)]
#[path = "shadow_gpu_tests.rs"]
mod tests;
