//! THE TREE CARD CULL (2026-11-14): the far tree cards are drawn only where they can show.
//!
//! WHY. BUG-301 made the far tree cards (the patch-baked sprite cards, type 12's card branch in
//! `fs_terrain`) draw for the first time in any release, and they cost 7 to 10 ms of the world
//! surface pass (`gpu.celestial`) plus 1.4 to 2.2 ms of the sun shadow pass at the default 1,500 m
//! on an RTX 4070 at 2560 x 1440: a whole 120 fps frame. Every card was baked INTO its terrain
//! patch's mesh, so it was drawn with the ground whether or not it could show: about half the
//! carried cards sat outside the window the fragment stage keeps (inside the hide radius where the
//! 3D models stand, or past the far cutoff), and every one of their pixels was still rasterized
//! and discarded one by one; cards behind the camera were drawn too; and the sun pass, which draws
//! without face culling, drew every card's two windings over the same texels twice.
//!
//! THE TECHNIQUE: GPU-driven culling into indirect draws, the way the plain shadow casters went in
//! `shadow_gpu.rs` (Haar and Aaltonen, "GPU-Driven Rendering Pipelines", SIGGRAPH 2015; Wihlidal,
//! "Optimizing the Graphics Pipeline with Compute", GDC 2016: a compute pass tests each small piece
//! of geometry against the view and compacts the survivors' indices into an indirect draw).
//! - The cards are the TAIL of each patch mesh (the patch builder emits them after the skirt):
//!   [`card_tail_quads`] finds them at upload, checking every index, and [`CardRecords`] keeps one
//!   cull record per card quad (its centre in the patch frame and its bounding radius).
//! - Each frame [`CardCull::dispatch`] runs one compute workgroup per card-bearing drawn patch
//!   (`assets/shaders/tree_card_cull.wgsl`). The main view keeps a quad when its bounding sphere
//!   meets the kept window (read from the same shadow uniform `fs_terrain` reads) and the camera's
//!   frustum (the same camera uniform the draw reads); the sun view keeps it when the sphere meets
//!   the shadow map's box (the light's own matrix). Kept indices go into each patch's region of a
//!   compact index buffer, their count into that patch's indirect draw.
//! - THE MAIN VIEW KEEPS THE OLD DRAW ORDER. Its compact indices live in a range reserved inside the
//!   patch arena's own index buffer ([`CardCull::region`]), so ONE multi-draw carries, patch by
//!   patch in the front-to-back order the frame loop sorted, the patch's ground and then its kept
//!   cards, exactly the order the whole-patch draw had. The first cut drew every patch's ground and
//!   only then every card, and lost more than the cull saved at `fuji-forest-ground` (`gpu.celestial`
//!   23.0 ms against 21.1 unculled, same boot): near cards drawn early had been hiding the ground
//!   behind them from the depth test, and drawn last they hid nothing.
//! - The sun view draws its kept cards from a buffer of its own after the ground, ONE winding a
//!   quad (the pass draws without face culling, so the second winding covered the same texels).
//!
//! THE PICTURE does not change: every test is a bounding sphere, so it keeps at least every quad
//! the per-pixel tests in `fs_terrain` and `fs_shadow` let any pixel of through, and those still
//! decide every edge, the hashed dissolve included. Showcase `card_cull` "0" is the red twin (the
//! cards drawn in their patches as before), and the same-boot cost A/B's other arm. A device
//! without multi-draw indirect keeps the per-patch draws, cards included, as before.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use glam::{Mat4, Vec3, Vec4};
use humanity_core::vertex::Vertex;

use super::patch_arena::{IndirectArgs, RangeAlloc};
use super::PatchDraw;

/// The cull's compute shader.
pub const CULL_WGSL: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/tree_card_cull.wgsl"));

/// The shader's jitter widening of the main view's side planes (radians of view times distance).
pub const JITTER_MARGIN: f32 = 0.004;

/// The index pattern of one card quad (`planet_chunks` `emit_card` and `emit_sprite_card`): the
/// quad, then the quad wound backwards so it survives back-face culling from either side.
pub const QUAD_INDICES: [u32; 12] = [0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2];

/// The main view's first reserved index range: four million indices (16 MB of the arena's index
/// buffer, a third of a million card quads in view). Grown at upload when a frame wanted more.
pub const REGION_START: u32 = 1 << 22;

/// The showcase `card_cull` pin: true while it is "0" (the red twin).
static PINNED_OFF: AtomicBool = AtomicBool::new(false);

/// The last cull's card patches and quads (0 and 0 with the cull off), for the tree card counter
/// (engine/tree_card_stage.rs: the `[TreeCards]` line and every screenshot's done file).
static LAST_JOBS: AtomicU32 = AtomicU32::new(0);
static LAST_QUADS: AtomicU32 = AtomicU32::new(0);

/// The last cull's (card patches, card quads) it tested.
pub fn last_culled() -> (u32, u32) {
    (LAST_JOBS.load(Ordering::Relaxed), LAST_QUADS.load(Ordering::Relaxed))
}

/// Set the showcase `card_cull` pin (`on` false = the cards drawn in their patches, as before).
pub fn set_pinned_on(on: bool) {
    PINNED_OFF.store(!on, Ordering::Relaxed);
}

/// Is the showcase pin leaving the cull on?
pub fn pinned_on() -> bool {
    !PINNED_OFF.load(Ordering::Relaxed)
}

/// Is this vertex a tree card's (a sprite card's negative uv, or a coloured card's flag bit)?
fn is_card_vertex(v: &Vertex) -> bool {
    v.uv[0] < -0.5 || humanity_terrain::planet_surface::unpack_uv_tree_card(v.uv)
}

/// How many tree card quads sit at the tail of a patch mesh: the trailing card vertices, four a
/// quad, whose indices are the last twelve a quad in [`QUAD_INDICES`] order. 0 when the tail is not
/// exactly that (the patch then draws whole, cards included, as before the cull).
pub fn card_tail_quads(vertices: &[Vertex], indices: &[u32]) -> u32 {
    let n = vertices.iter().rev().take_while(|v| is_card_vertex(v)).count();
    if n == 0 || n % 4 != 0 || indices.len() < n * 3 {
        return 0;
    }
    let quads = n / 4;
    let v0 = (vertices.len() - n) as u32;
    let tail = &indices[indices.len() - quads * 12..];
    let ok = tail.chunks_exact(12).enumerate().all(|(q, c)| {
        let base = v0 + 4 * q as u32;
        c.iter().zip(QUAD_INDICES).all(|(&i, p)| i == base + p)
    });
    // Nothing before the tail may reach into it (a card vertex drawn by the ground's range would
    // vanish with the cull).
    if !ok || indices[..indices.len() - quads * 12].iter().any(|&i| i >= v0) {
        return 0;
    }
    quads as u32
}

/// One quad's cull record: its centre in the patch frame and the radius that bounds its corners.
pub fn quad_record(corners: [[f32; 3]; 4]) -> [f32; 4] {
    let p = corners.map(Vec3::from);
    let c = (p[0] + p[1] + p[2] + p[3]) * 0.25;
    let r = p.iter().map(|q| q.distance(c)).fold(0.0f32, f32::max);
    [c.x, c.y, c.z, r]
}

/// The rule the shader applies, on the processor (its twin: change one, change both). `c` is the
/// quad's centre in render space, `r` its radius; `cam_vp`/`cam_pos` the main view, `hide`/`far`
/// the kept window, `light_vp` the sun's matrix. Returns (main view keeps it, sun view keeps it).
pub fn cpu_keeps(c: Vec3, r: f32, cam_vp: Mat4, cam_pos: Vec3, hide: f32, far: f32, light_vp: Mat4) -> (bool, bool) {
    let row = |m: Mat4, i: usize| m.row(i);
    let dist = |p: Vec4| (p.truncate().dot(c) + p.w) / p.truncate().length().max(1.0e-20);
    let d = c.distance(cam_pos);
    let rm = -(r + d * JITTER_MARGIN);
    let (r0, r1, r3) = (row(cam_vp, 0), row(cam_vp, 1), row(cam_vp, 3));
    let main = d + r >= hide && d - r <= far && [r3 + r0, r3 - r0, r3 + r1, r3 - r1].iter().all(|&p| dist(p) >= rm);
    let (l0, l1, l2, l3) = (row(light_vp, 0), row(light_vp, 1), row(light_vp, 2), row(light_vp, 3));
    let sun = [l3 + l0, l3 - l0, l3 + l1, l3 - l1, l2, l3 - l2].iter().all(|&p| dist(p) >= -r);
    (main, sun)
}

/// Where a patch's cards are: how many quads, and their first record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardTail {
    pub quads: u32,
    pub rec: u32,
}

/// The cull records of every card quad in the patch arena, one `[f32; 4]` each, in a storage buffer
/// that grows by doubling (its old contents copied across at once, so later writes land after).
pub struct CardRecords {
    buf: wgpu::Buffer,
    alloc: RangeAlloc,
    max: u32,
    /// The arena's card patches by their first vertex (unique while the slot is held).
    tails: HashMap<u32, CardTail>,
    /// Bumped on every grow: the cull's bind group is made again.
    generation: u64,
    /// The cards' share of the patch arena: their vertices and their indices.
    pub card_bytes: u64,
    full_logged: bool,
}

const RECORD_BYTES: u64 = 16;
/// The first record buffer: a million quads (16 MB).
const RECORDS_START: u32 = 1 << 20;

impl CardRecords {
    fn new(device: &wgpu::Device) -> Self {
        let limits = device.limits();
        let max = (limits.max_storage_buffer_binding_size as u64).min(limits.max_buffer_size) / RECORD_BYTES;
        let max = max.min(u32::MAX as u64) as u32;
        let cap = RECORDS_START.min(max);
        Self { buf: Self::buffer(device, cap), alloc: RangeAlloc::new(cap), max, tails: HashMap::new(), generation: 0, card_bytes: 0, full_logged: false }
    }

    fn buffer(device: &wgpu::Device, cap: u32) -> wgpu::Buffer {
        use wgpu::BufferUsages as U;
        device.create_buffer(&wgpu::BufferDescriptor { label: Some("Tree Card Records"), size: cap as u64 * RECORD_BYTES, usage: U::STORAGE | U::COPY_DST | U::COPY_SRC, mapped_at_creation: false })
    }

    /// Take `n` records, growing the buffer when no free range fits.
    fn take(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, n: u32) -> Option<u32> {
        loop {
            if let Some(at) = self.alloc.alloc(n) {
                return Some(at);
            }
            let cap = self.alloc.cap();
            if cap >= self.max {
                if !self.full_logged {
                    self.full_logged = true;
                    log::warn!("[TreeCards] the card record buffer is at its limit ({cap} quads): new card patches draw whole, unculled, until eviction frees room");
                }
                return None;
            }
            let grown = cap.saturating_mul(2).min(self.max);
            let buf = Self::buffer(device, grown);
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Tree Card Records Grow") });
            enc.copy_buffer_to_buffer(&self.buf, 0, &buf, 0, cap as u64 * RECORD_BYTES);
            queue.submit(Some(enc.finish()));
            self.buf = buf;
            self.alloc.grow_to(grown);
            self.generation += 1;
            log::info!("[TreeCards] the card record buffer grew to {grown} quads ({:.0} MB)", grown as f64 * RECORD_BYTES as f64 / 1_048_576.0);
        }
    }

    /// A patch was uploaded at `vstart`: find its card tail and write its records.
    pub fn add(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vstart: u32, vertices: &[Vertex], indices: &[u32]) {
        let quads = card_tail_quads(vertices, indices);
        if quads == 0 {
            return;
        }
        let Some(rec) = self.take(device, queue, quads) else { return };
        self.full_logged = false;
        let v0 = vertices.len() - quads as usize * 4;
        let recs: Vec<[f32; 4]> = vertices[v0..].chunks_exact(4).map(|q| quad_record([q[0].position, q[1].position, q[2].position, q[3].position])).collect();
        queue.write_buffer(&self.buf, rec as u64 * RECORD_BYTES, bytemuck::cast_slice(&recs));
        self.tails.insert(vstart, CardTail { quads, rec });
        self.card_bytes += quads as u64 * Self::quad_bytes();
    }

    /// A patch's slot was released: free its records.
    pub fn remove(&mut self, vstart: u32) {
        if let Some(t) = self.tails.remove(&vstart) {
            self.alloc.free(t.rec, t.quads);
            self.card_bytes -= t.quads as u64 * Self::quad_bytes();
        }
    }

    /// Patch arena memory one card quad takes: four vertices and twelve indices.
    fn quad_bytes() -> u64 {
        4 * std::mem::size_of::<Vertex>() as u64 + 12 * 4
    }

    pub fn tail(&self, vstart: u32) -> Option<CardTail> {
        self.tails.get(&vstart).copied()
    }
}

/// One card-bearing drawn patch, as the shader reads it (`Job`): its translation, first record,
/// quads, first card vertex (absolute), its offsets into the main window and the sun buffer (12 and
/// 6 indices a quad), and `arg`, its card draw's place in the main argument list.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullJob {
    pub pos: [f32; 4],
    pub rec: u32,
    pub quads: u32,
    pub v0: u32,
    pub dst: u32,
    pub sdst: u32,
    pub arg: u32,
    pub pad: [u32; 2],
}

/// One frame's plan: the jobs; the main view's argument list (per patch in draw order its ground,
/// then, for a card patch, its cards in the reserved window, the count zeroed for the shader to
/// fill); the sun's (one per job); the ground's index count for every patch draw; the indices each
/// compact buffer needs.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub jobs: Vec<CullJob>,
    pub main_args: Vec<IndirectArgs>,
    pub sun_args: Vec<IndirectArgs>,
    pub ground: Vec<u32>,
    pub main_indices: u32,
    pub sun_indices: u32,
}

/// Plan a frame: every drawn patch with a card tail becomes a job (when `on`, and while its cards
/// fit the `region` (first index, capacity) reserved in the arena's index buffer), and its ground
/// draw stops short of its cards. `tail` answers a patch's first vertex.
pub fn plan(draws: &[PatchDraw], on: bool, region: (u32, u32), tail: impl Fn(u32) -> Option<CardTail>) -> Plan {
    let mut p = Plan::default();
    for (i, d) in draws.iter().enumerate() {
        let ground = |count| IndirectArgs { index_count: count, instance_count: 1, first_index: d.slot.istart, base_vertex: d.slot.vstart as i32, first_instance: i as u32 };
        let fits = |t: &CardTail| on && t.quads > 0 && t.quads * 12 <= d.slot.icount && t.quads * 4 <= d.slot.vcount && p.main_indices + 12 * t.quads <= region.1;
        match tail(d.slot.vstart).filter(fits) {
            Some(t) => {
                p.ground.push(d.slot.icount - 12 * t.quads);
                p.main_args.push(ground(d.slot.icount - 12 * t.quads));
                let v0 = d.slot.vstart + d.slot.vcount - 4 * t.quads;
                p.jobs.push(CullJob { pos: [d.position.x, d.position.y, d.position.z, 0.0], rec: t.rec, quads: t.quads, v0, dst: p.main_indices, sdst: p.sun_indices, arg: p.main_args.len() as u32, pad: [0; 2] });
                let cards = |first_index| IndirectArgs { index_count: 0, instance_count: 1, first_index, base_vertex: 0, first_instance: i as u32 };
                p.main_args.push(cards(region.0 + p.main_indices));
                p.sun_args.push(cards(p.sun_indices));
                p.main_indices += 12 * t.quads;
                p.sun_indices += 6 * t.quads;
            }
            None => {
                p.ground.push(d.slot.icount);
                p.main_args.push(ground(d.slot.icount));
            }
        }
    }
    p
}

/// The buffers the cull reads and writes that are not its own: the uniform buffers the draws read,
/// and the patch arena's index buffer with the range reserved in it for the main view's cards.
pub struct Inputs<'a> {
    /// The celestial pass's camera (`view_proj` and `view_pos` first, unjittered).
    pub camera: &'a wgpu::Buffer,
    /// The sun shadow pass's camera (its `view_proj` is the light's matrix).
    pub light: &'a wgpu::Buffer,
    /// The celestial shadow uniform (shadows on, the card hide radius, the card far cutoff).
    pub shadow: &'a wgpu::Buffer,
    /// The patch batch's rotation (`PatchArena::batch_buf`).
    pub rot: &'a wgpu::Buffer,
    /// The patch arena's index buffer (`STORAGE`), and its reserved (first index, capacity).
    pub index: &'a wgpu::Buffer,
    pub region: (u32, u32),
}

#[derive(Default)]
struct Frame {
    jobs: Option<wgpu::Buffer>,
    main_args: Option<wgpu::Buffer>,
    sun_args: Option<wgpu::Buffer>,
    sun_idx: Option<wgpu::Buffer>,
    group: Option<wgpu::BindGroup>,
    /// What the bind group was made for: the records' generation and the reserved region.
    group_key: (u64, u32, u32),
    n_jobs: u32,
    n_main: u32,
    ground: Vec<u32>,
}

/// The cull: its pipeline, the records, the main view's reserved range, and this frame's buffers
/// (written from the `&self` render path, hence the cells).
pub struct CardCull {
    pipe: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    pub records: CardRecords,
    /// The main view's (first index, capacity) inside the patch arena's index buffer (`PatchArena`
    /// reserves and grows it).
    pub region: (u32, u32),
    /// The most indices a frame's main view wanted (the arena grows the region to it at upload).
    pub wanted: Cell<u32>,
    frame: RefCell<Frame>,
}

/// Make a buffer of at least `bytes` (a power of two, never under 256), or keep `have`. True when made.
fn ensure(device: &wgpu::Device, have: &mut Option<wgpu::Buffer>, bytes: u64, label: &str, usage: wgpu::BufferUsages) -> bool {
    if have.as_ref().is_some_and(|b| b.size() >= bytes) {
        return false;
    }
    let size = bytes.max(256).next_power_of_two();
    *have = Some(device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false }));
    true
}

impl CardCull {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Tree Card Cull Shader"), source: wgpu::ShaderSource::Wgsl(CULL_WGSL.into()) });
        let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let (u, ro, rw) = (wgpu::BufferBindingType::Uniform, wgpu::BufferBindingType::Storage { read_only: true }, wgpu::BufferBindingType::Storage { read_only: false });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Tree Card Cull Layout"),
            entries: &[entry(0, u), entry(1, u), entry(2, u), entry(3, u), entry(4, ro), entry(5, ro), entry(6, rw), entry(7, rw), entry(8, rw), entry(9, rw)],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("Tree Card Cull Pipeline Layout"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("cull_cards"), layout: Some(&pl), module: &shader, entry_point: Some("cull_cards"), compilation_options: Default::default(), cache: None });
        Self { pipe, layout, records: CardRecords::new(device), region: (0, 0), wanted: Cell::new(0), frame: RefCell::new(Frame::default()) }
    }

    /// The largest main-view range a storage binding may cover on this device, in indices.
    pub fn max_region(device: &wgpu::Device) -> u32 {
        (device.limits().max_storage_buffer_binding_size / 4).min(u32::MAX / 4)
    }

    /// Plan this frame, upload it, and record and submit the cull (before the sun pass, after every
    /// uniform the cull reads was written: the queue applies those writes first). `on` false (the
    /// showcase pin, or a device without multi-draw indirect) leaves every patch drawing whole.
    /// `timer` is asked only when the pass is recorded (a timer pair taken and never written reads
    /// as an unrecorded pass).
    pub fn dispatch<'t>(&self, device: &wgpu::Device, queue: &wgpu::Queue, inputs: &Inputs<'_>, draws: &[PatchDraw], on: bool, timer: impl FnOnce() -> Option<wgpu::ComputePassTimestampWrites<'t>>) {
        let p = plan(draws, on, inputs.region, |v| self.records.tail(v));
        if on {
            // What the whole frame would want, for the arena to grow the region to.
            let want: u32 = draws.iter().filter_map(|d| self.records.tail(d.slot.vstart)).map(|t| 12 * t.quads).sum();
            self.wanted.set(self.wanted.get().max(want));
        }
        let mut f = self.frame.borrow_mut();
        f.ground = p.ground;
        f.n_jobs = p.jobs.len() as u32;
        f.n_main = p.main_args.len() as u32;
        LAST_JOBS.store(f.n_jobs, Ordering::Relaxed);
        LAST_QUADS.store(p.jobs.iter().map(|j| j.quads).sum(), Ordering::Relaxed);
        if p.jobs.is_empty() {
            return;
        }
        use wgpu::BufferUsages as U;
        let args = std::mem::size_of::<IndirectArgs>() as u64;
        let key = (self.records.generation, inputs.region.0, inputs.region.1);
        let mut remade = f.group_key != key;
        remade |= ensure(device, &mut f.jobs, (p.jobs.len() * std::mem::size_of::<CullJob>()) as u64, "Tree Card Cull Jobs", U::STORAGE | U::COPY_DST);
        remade |= ensure(device, &mut f.main_args, p.main_args.len() as u64 * args, "Tree Card Main Draws", U::STORAGE | U::INDIRECT | U::COPY_DST | U::COPY_SRC);
        remade |= ensure(device, &mut f.sun_args, p.sun_args.len() as u64 * args, "Tree Card Sun Draws", U::STORAGE | U::INDIRECT | U::COPY_DST | U::COPY_SRC);
        remade |= ensure(device, &mut f.sun_idx, p.sun_indices as u64 * 4, "Tree Card Sun Indices", U::STORAGE | U::INDEX | U::COPY_SRC);
        if remade || f.group.is_none() {
            fn b(buf: &Option<wgpu::Buffer>) -> wgpu::BindingResource<'_> {
                buf.as_ref().expect("made above").as_entire_binding()
            }
            let window = wgpu::BufferBinding { buffer: inputs.index, offset: inputs.region.0 as u64 * 4, size: wgpu::BufferSize::new(inputs.region.1 as u64 * 4) };
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Tree Card Cull BG"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: inputs.camera.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: inputs.light.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: inputs.shadow.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: inputs.rot.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: self.records.buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 5, resource: b(&f.jobs) },
                    wgpu::BindGroupEntry { binding: 6, resource: b(&f.main_args) },
                    wgpu::BindGroupEntry { binding: 7, resource: b(&f.sun_args) },
                    wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::Buffer(window) },
                    wgpu::BindGroupEntry { binding: 9, resource: b(&f.sun_idx) },
                ],
            });
            f.group = Some(group);
            f.group_key = key;
        }
        let w = |buf: &Option<wgpu::Buffer>, bytes: &[u8]| queue.write_buffer(buf.as_ref().expect("made above"), 0, bytes);
        w(&f.jobs, bytemuck::cast_slice(&p.jobs));
        w(&f.main_args, bytemuck::cast_slice(&p.main_args));
        w(&f.sun_args, bytemuck::cast_slice(&p.sun_args));
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Tree Card Cull Encoder") });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Tree Card Cull Pass"), timestamp_writes: timer() });
            pass.set_pipeline(&self.pipe);
            pass.set_bind_group(0, f.group.as_ref(), &[]);
            pass.dispatch_workgroups(f.n_jobs, 1, 1);
        }
        queue.submit(Some(enc.finish()));
    }

    /// Patch draw `i`'s ground index count this frame: its ground only when its cards are culled,
    /// its whole range otherwise.
    pub fn ground_icount(&self, i: usize, d: &PatchDraw) -> u32 {
        self.frame.borrow().ground.get(i).copied().unwrap_or(d.slot.icount).min(d.slot.icount)
    }

    /// The main view: draw this frame's argument list (every patch's ground, each card patch's kept
    /// cards after it) in one multi-draw. False when the cull did not run (draw the patches whole).
    /// The caller has bound the patch pipeline, the arena's buffers and the bind groups.
    pub fn draw_main(&self, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let f = self.frame.borrow();
        let Some(args) = f.main_args.as_ref().filter(|_| f.n_jobs > 0) else { return false };
        pass.multi_draw_indexed_indirect(args, 0, f.n_main);
        true
    }

    /// The sun view: draw this frame's kept cards, one winding each (after the ground). Binds its
    /// own index buffer.
    pub fn draw_sun(&self, pass: &mut wgpu::RenderPass<'_>) {
        let f = self.frame.borrow();
        let (Some(idx), Some(args)) = (f.sun_idx.as_ref(), f.sun_args.as_ref()) else { return };
        if f.n_jobs == 0 {
            return;
        }
        pass.set_index_buffer(idx.slice(..), wgpu::IndexFormat::Uint32);
        pass.multi_draw_indexed_indirect(args, 0, f.n_jobs);
    }

    /// Video memory: the cards' share of the patch arena (their geometry and the reserved range)
    /// plus the cull's own buffers.
    pub fn vram_bytes(&self) -> u64 {
        let f = self.frame.borrow();
        let own: u64 = [&f.jobs, &f.main_args, &f.sun_args, &f.sun_idx].iter().map(|b| b.as_ref().map_or(0, |b| b.size())).sum();
        self.records.card_bytes + self.region.1 as u64 * 4 + self.records.buf.size() + own
    }

    /// For the tests: the buffers the cull writes (main arguments, sun arguments, sun indices).
    #[cfg(all(test, feature = "native"))]
    pub(crate) fn outputs(&self) -> [wgpu::Buffer; 3] {
        let f = self.frame.borrow();
        [&f.main_args, &f.sun_args, &f.sun_idx].map(|b| b.as_ref().expect("dispatched").clone())
    }
}

use super::Renderer;

impl Renderer {
    /// Run this frame's card cull over the first `n` patch draws (celestial.rs, after the patch
    /// instances and every uniform the cull reads are written, before the sun pass).
    pub(super) fn card_cull_dispatch(&self, n: usize) {
        let Some(arena) = self.patch_arena.as_ref() else { return };
        let inputs = Inputs { camera: &self.camera_buffer, light: &self.light_camera_buffer, shadow: &self.shadow_uniform_buffer, rot: &arena.batch_buf, index: &arena.index_buf, region: arena.cards.region };
        let on = self.patch_indirect && pinned_on() && arena.cards.region.1 > 0;
        arena.cards.dispatch(&self.device, &self.queue, &inputs, &self.patch_draws[..n], on, || self.compute_pass_timer("gpu.card_cull"));
    }

    /// Patch draw `i`'s index count this frame in the sun pass (the ground only when its cards are
    /// culled).
    pub(super) fn patch_icount(&self, i: usize, d: &PatchDraw) -> u32 {
        self.patch_arena.as_ref().map_or(d.slot.icount, |a| a.cards.ground_icount(i, d))
    }

    /// The main view's patch batch in one multi-draw: the cull's list when it ran, else every patch
    /// whole from the arena's own arguments.
    pub(super) fn draw_patches_main(&self, pass: &mut wgpu::RenderPass<'_>, n: usize) {
        let Some(a) = self.patch_arena.as_ref() else { return };
        if !a.cards.draw_main(pass) {
            pass.multi_draw_indexed_indirect(&a.indirect_buf, 0, n as u32);
        }
    }

    /// The sun pass's kept cards, after the ground.
    pub(super) fn draw_patch_cards_sun(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(a) = self.patch_arena.as_ref() {
            a.cards.draw_sun(pass);
        }
    }
}

#[cfg(test)]
#[path = "tree_card_cull_tests.rs"]
mod tests;
