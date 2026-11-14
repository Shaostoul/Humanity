//! SKINNED BODIES ON THE GPU (character animation rung 1, 2026-11-08;
//! docs/design/character-animation.md section 2.2).
//!
//! Two ways to bend a body, by distance (the tiers are chosen in the animation crate):
//!
//! - NEAR: a compute pass skins the body once a frame (`assets/shaders/skinning.wgsl`) into a
//!   vertex buffer in the ordinary mesh layout. Each of the body's regions (skin, hair, shirt, ...)
//!   is registered once as an ordinary `Mesh` over that buffer, so the frame draws a near body as a
//!   handful of plain `RenderObject`s in the scene list: the shadow atlas (renderer/local_shadows.rs,
//!   since 2026-11-10; the far sun map never draws the scene list) and the colour pass read the same
//!   skinned vertices, and no pass had to learn anything.
//! - MIDDLE: the vertex shader skins it (`assets/shaders/skinning_vs.wgsl`, appended to the PBR
//!   megashader for one PSO of its own, the surface class's fragment entry with the surface PSO's
//!   permutation), which costs no memory. Drawn after the opaque list in the scene passes (the live
//!   frame and the camera screens). Since 2026-11-10 two skinned SHADOW PSOs beside it draw the middle
//!   bodies, and the first-person body's unfolded twin (`skin_shadow_only`), into the shadow atlas.
//!
//! THE POSE PALETTE is one storage buffer, rewritten every frame: per body, its model matrix, its
//! morph block (`MORPH_SLOTS` matrices saying which of its shape targets are on and how much,
//! 2026-11-09) and then each joint's skinning matrix, three `vec4` rows per matrix (48 bytes;
//! `humanity_animation::pose`). A near body's compute dispatch is told where its morph block and
//! joints start; a middle body's draw passes where its model matrix is as its first instance.
//!
//! SHAPE TARGETS (2026-11-09, the MakeHuman bodies): each body's morph targets live in a storage
//! buffer of their own (`morph_deltas`), read by both shaders, which add the active ones to a vertex
//! before skinning it. So one body on the GPU draws every person's shape, and a person's shape costs
//! the palette six rows, not a mesh of their own.
//!
//! NOTHING HERE IS REUSED ACROSS FRAMES as worked-out data: the palette and every near body's
//! skinned vertices are written afresh each frame. What persists is GPU allocation (a body's source
//! vertices, a near place's output buffer, the PSOs), which is a resource, not a result that can go
//! stale. The VS PSO builds on a background thread so it never adds to the boot; until it is ready
//! (and after a scene-format A/B switch, until it is rebuilt) middle bodies are skinned by compute
//! like near ones, so a body is never missing.
//!
//! EACH NEAR PERSON'S OWN MESH (the look, 2026-11-09) lives in THE OWN-MESH POOL (2026-11-10,
//! `renderer::mesh_pool`): three buffers allocated once, on the first frame that draws bodies
//! (`skin_create_own_pool`), the source vertices, their skinned copy and the indices, of which each
//! person holds a range (`skin_own_place`). Each of the pool's handles is a body here (ids after the
//! shared bodies) with one near place made with the pool, so a person arriving is two queue writes
//! and no allocation; their body draws with `base_vertex` and a first index into the pool's buffers
//! (`Mesh::base_vertex`, `Mesh::first_index`), and the compute pass skins from and into their range
//! (the parameters' fourth word). The same first frame also makes the shared bodies' near places for
//! a full near tier and sizes the palette for a crowd, so nothing a crowd arriving needs is
//! allocated on the frame (measured: one own-mesh upload in about 38 took 9.9 to 11.5 ms when each
//! had buffers of its own).

use super::*;
use super::deferred_build::{Deferred, Polled};
use super::mesh_pool::{MeshClass, MeshPool, Refusal, Side, Step};
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
/// as a vertex buffer by the VS path), an index buffer per region, offset into the one vertex
/// list, and its morph targets' deltas (2026-11-09). A person's own body in the own-mesh pool
/// (2026-11-10) is a range of the pool's buffers: `base_vertex` is where its vertices start, and
/// its regions are ranges of the pool's one index buffer.
pub struct SkinnedBody {
    vertices: wgpu::Buffer,
    /// Where this body's vertices start in `vertices` and in its near place's output: 0 for a body
    /// with buffers of its own, the person's range in the own-mesh pool.
    base_vertex: u32,
    vertex_count: u32,
    joint_count: u32,
    /// (index buffer, first index, index count) per region, in the model's primitive order.
    regions: Vec<(wgpu::Buffer, u32, u32)>,
    /// Every morph target's deltas: for target k and vertex i, two `vec4` at (k * vertex_count + i)
    /// * 2: the position delta then the normal delta (w unused). One zero `vec4` pair for a body
    /// without targets (a storage binding may not be empty). Which targets a body draws with, and
    /// how much, rides in the palette's morph block (`humanity_animation::pose::MORPH_SLOTS`).
    morphs: wgpu::Buffer,
    /// The VS path's group 1 for this body: the identity object uniform, the palette, its morphs.
    vs_bind: wgpu::BindGroup,
}

/// A near place: one body's compute-skinned vertices for this frame, and its regions registered as
/// ordinary meshes over them. `meshes` may hold more ids than the body has regions (a person's own
/// body rebuilt with fewer garments keeps its spare mesh ids for the next rebuild); the first
/// `regions` are this body's.
struct NearSlot {
    params: wgpu::Buffer,
    bind: wgpu::BindGroup,
    out: wgpu::Buffer,
    /// The same vertices skinned by LAST frame's pose (the previous palette), in the same layout:
    /// vertex slot 2 of the region meshes, for the motion target (renderer/taa.rs).
    prev: wgpu::Buffer,
    meshes: Vec<usize>,
    regions: usize,
}

/// A middle body's region to draw this frame.
#[derive(Clone, Copy)]
struct MiddleDraw {
    body: usize,
    region: usize,
    material: usize,
    model_index: u32,
}

/// The VS PSO and what it was built for, with its two shadow twins (2026-11-10, the shadow atlas:
/// renderer/local_shadows.rs): `vs_skinned` into a depth-only target, and the same with `fs_shadow`
/// for the card hair's cut. The colour PSO's scene format does not touch them.
struct SkinnedPso {
    pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
    /// Whether the colour PSO has the shadow atlas lookups compiled in (shadows rung 1b: out while
    /// Room light shadows is Off, `pipeline::ATLAS_TWINS`); it follows `Pipeline::shadow_atlas_wanted`.
    atlas: bool,
    shadow: wgpu::RenderPipeline,
    shadow_alpha: wgpu::RenderPipeline,
}

/// A skinned near mesh's bounding sphere in the body's own space (`Mesh::bounds`): its vertices are
/// written each frame by the skinning pass, so the sphere is the generous body one
/// (`local_shadows::BODY_RADIUS_M` about `BODY_CENTRE`), which holds a body lying down or reaching up.
const BODY_BOUNDS: [f32; 4] = [
    super::local_shadows::BODY_CENTRE[0],
    super::local_shadows::BODY_CENTRE[1],
    super::local_shadows::BODY_CENTRE[2],
    super::local_shadows::BODY_RADIUS_M,
];

/// Everything the skinned path keeps on the renderer.
pub struct Skinning {
    bodies: Vec<SkinnedBody>,
    compute: wgpu::ComputePipeline,
    compute_layout: wgpu::BindGroupLayout,
    palette: wgpu::Buffer,
    palette_rows: u64,
    staging: Vec<[f32; 4]>,
    /// THE PREVIOUS PALETTE (temporal anti-aliasing, renderer/taa.rs): this frame's layout filled
    /// with each person's rows from the last frame (found by their key), the model rows carried into
    /// this frame's render space, so both skinning paths can place a vertex where it was. A person
    /// new this frame (or the history discarded) gets this frame's rows: no motion of their own.
    prev_palette: wgpu::Buffer,
    /// (key, first row, rows) of each body pushed this frame (`skin_push_palette`).
    keys: Vec<(u64, u32, u32)>,
    /// The last frame's staging and where each key's rows were in it (reuse data: dropped whenever
    /// the history is, `skin_upload_prev`).
    /// reuse: site: the bodies' previous palette for the motion target: with the switch off
    /// `skin_upload_prev` uploads this frame's rows instead, so nobody moves on their own.
    last_staging: Vec<[f32; 4]>,
    /// reuse: site: where each body's rows sat in `last_staging`, forgotten with it when the switch
    /// is off (`skin_upload_prev`).
    last_keys: std::collections::HashMap<u64, (u32, u32)>,
    /// Near places per body (a place keeps the body it was made for).
    slots: Vec<Vec<NearSlot>>,
    /// (body, slot, model matrix index) of each near body skinned this frame.
    jobs: Vec<(usize, usize, u32)>,
    /// This frame's dispatches are queued and not yet recorded (`encode_skinning`).
    dispatch_pending: std::cell::Cell<bool>,
    middle: Vec<MiddleDraw>,
    /// Bodies drawn into the shadow maps only (2026-11-10): the first-person body's unfolded twin,
    /// whose head the eye camera's copy folds away but whose shadow keeps it
    /// (`skin_shadow_only`; docs/design/character-animation.md section 17).
    shadow_only: Vec<MiddleDraw>,
    /// Group 1 of the VS PSO: the identity object uniform, the palette and a body's morphs.
    vs_layout: wgpu::BindGroupLayout,
    vs_object: wgpu::Buffer,
    vs_pso: Deferred<SkinnedPso>,
    /// The own-mesh pool, once made (`skin_create_own_pool`): its handles are the bodies from
    /// `OwnPool::first_body` on.
    own: Option<OwnPool>,
    /// A few bytes for a mesh id that draws nothing (a region a rebuilt body no longer has, a
    /// dropped pool's places), so it holds on to no body's memory.
    empty: wgpu::Buffer,
}

/// The own-mesh pool's GPU side (`renderer::mesh_pool` holds the books).
struct OwnPool {
    book: MeshPool,
    /// Each pooled vertex as read (`SkinnedVertex`): storage for the compute pass, vertex buffer for
    /// the VS path.
    sources: wgpu::Buffer,
    /// Each pooled vertex skinned this frame (the ordinary `Vertex`): the near places' output.
    outputs: wgpu::Buffer,
    /// The same skinned by last frame's pose (`NearSlot::prev`).
    prev_outputs: wgpu::Buffer,
    /// Every pooled person's indices, relative to their own first vertex (drawn with `base_vertex`).
    indices: wgpu::Buffer,
    /// The morph binding of a pooled body (an own mesh has no shape targets: its shape is in its
    /// vertices); one zero pair, shared.
    zero_morphs: wgpu::Buffer,
    /// The body id of handle 0 (handle `h` is body `first_body + h`).
    first_body: usize,
    /// The shared bodies' near places made with the pool, and their bytes (the memory figure).
    shared_place_bytes: u64,
}

impl OwnPool {
    fn handle_of(&self, body: usize) -> Option<usize> {
        body.checked_sub(self.first_body).filter(|&h| h < self.book.handles())
    }
}

/// What the own-mesh pool holds, for the log, the Dev page and Settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnPoolStats {
    /// The pool's buffers and the shared bodies' near places made with it, bytes.
    pub reserved_bytes: u64,
    /// The people in the pool now, bytes.
    pub used_bytes: u64,
    pub holders: usize,
    pub people: u32,
    pub class: MeshClass,
}

/// The palette's morph block, in matrix slots: `humanity_animation::pose::MORPH_SLOTS` (this module
/// compiles in the relay build, which has no animation crate, so it is written here and held equal by
/// `engine::animation`'s tests).
pub const MORPH_SLOTS: u32 = 2;
/// Matrices from a body's model matrix to its morph block, and to its first joint.
const MORPH_BLOCK_AT: u32 = 1;
const JOINTS_AT: u32 = 1 + MORPH_SLOTS;

impl Renderer {
    /// Upload a shared body (a library body, its mesh as read from the glTF) and return its id. The
    /// first call builds the compute pipeline and starts the VS PSO's background build. A person's
    /// own mesh goes to the own-mesh pool instead (`skin_own_place`). The pool's handles come after
    /// the shared bodies, so registering one lets the pool go (the engine plans it again on the
    /// next frame that draws bodies; a body is registered only when the library is read).
    pub fn skin_register_body(&mut self, model: &SkinnedModelData) -> usize {
        if self.skinning.is_none() {
            self.skinning = Some(Box::new(Skinning::new(self)));
        }
        if self.skinning.as_ref().is_some_and(|sk| sk.own.is_some()) {
            log::info!("Characters: a body registered after the own-mesh pool; the pool is let go and planned again");
            self.skin_drop_own_pool();
        }
        let body = self.make_skinned_body(model);
        let sk = self.skinning.as_mut().expect("made above");
        sk.bodies.push(body);
        sk.slots.push(Vec::new());
        sk.bodies.len() - 1
    }

    /// Release a person's own body (they left, or stayed out of the near tier long enough, or gave
    /// way to a newcomer): its range of the own-mesh pool goes back to the pool, its body draws
    /// nothing until the handle is taken again. Nothing is freed on the GPU: the pool lives on.
    pub fn skin_release_body(&mut self, id: usize) {
        let Some(sk) = self.skinning.as_mut() else { return };
        let Some(own) = sk.own.as_mut() else { return };
        let Some(h) = own.handle_of(id) else { return };
        if own.book.release(h).is_none() {
            return;
        }
        let empty = sk.empty.clone();
        let b = &mut sk.bodies[id];
        b.vertex_count = 0;
        b.joint_count = 0;
        b.regions.clear();
        let ids = sk.slots[id].first().map(|s| s.meshes.clone()).unwrap_or_default();
        if let Some(s) = sk.slots[id].first_mut() {
            s.regions = 0;
        }
        for mid in ids {
            self.replace_mesh(mid, Mesh::nothing(&empty));
        }
    }

    /// THE OWN-MESH POOL (2026-11-10, `renderer::mesh_pool`): allocate its three buffers for
    /// `people` of the largest dressed person `class` at once, with `handles` places in its books
    /// (each a body here, with its near place), on the first frame that draws bodies. The same
    /// call makes each shared body's near places up to `shared_places` (a full near tier waiting
    /// for their own builds is drawn with them) and sizes the palette for `palette_bodies`, so a
    /// crowd arriving allocates nothing on the frame. Returns the milliseconds the allocations took.
    pub fn skin_create_own_pool(&mut self, class: MeshClass, people: u32, handles: u32, shared_places: usize, palette_bodies: u32) -> Option<f32> {
        let t0 = std::time::Instant::now();
        self.skin_drop_own_pool();
        let sk = self.skinning.as_ref()?;
        let mut book = MeshPool::new(class, people, handles);
        if !book.needs_buffers() {
            return None;
        }
        let vcap = book.capacity_vertices() as u64;
        let icap = book.capacity_indices() as u64;
        let buffer = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            self.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: size.max(16), usage, mapped_at_creation: false })
        };
        let sources = buffer("Own Mesh Pool Vertices", vcap * SkinnedVertex::BYTES as u64, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let outputs = buffer("Own Mesh Pool Skinned", vcap * OUT_VERTEX_BYTES, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE);
        let prev_outputs = buffer("Own Mesh Pool Skinned Last Frame", vcap * OUT_VERTEX_BYTES, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE);
        let indices = buffer("Own Mesh Pool Indices", icap * 4, wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST);
        let zero_morphs = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Own Mesh Pool Morphs"),
            contents: bytemuck::cast_slice(&[[0.0f32; 4]; 2]),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let first_body = sk.bodies.len();
        let shared = first_body;
        // The palette, sized for a crowd now rather than grown (an allocation and every bind group
        // made again) the frame one walks in.
        self.skin_reserve_palette(palette_bodies);
        let sk = self.skinning.as_mut().expect("checked above");
        for _ in 0..book.handles() {
            let vs_bind = Skinning::vs_bind_group(&self.device, &sk.vs_layout, &sk.vs_object, &sk.palette, &sk.prev_palette, &zero_morphs);
            let params = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Own Mesh Skinning Params"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = Skinning::compute_bind_group(&self.device, &sk.compute_layout, (&sk.palette, &sk.prev_palette), &sources, (&outputs, &prev_outputs), &params, &zero_morphs);
            sk.bodies.push(SkinnedBody { vertices: sources.clone(), base_vertex: 0, vertex_count: 0, joint_count: 0, regions: Vec::new(), morphs: zero_morphs.clone(), vs_bind });
            sk.slots.push(vec![NearSlot { params, bind, out: outputs.clone(), prev: prev_outputs.clone(), meshes: Vec::new(), regions: 0 }]);
        }
        // The shared bodies' near places for a full near tier.
        let mut shared_place_bytes = 0u64;
        for b in 0..shared {
            while self.skinning.as_ref().is_some_and(|sk| sk.slots[b].len() < shared_places) {
                let made = self.make_near_slot(b);
                let sk = self.skinning.as_mut().expect("checked above");
                sk.slots[b].push(made);
            }
            if let Some(sk) = self.skinning.as_ref() {
                shared_place_bytes += sk.slots[b].iter().map(|s| s.out.size() + s.params.size()).sum::<u64>();
            }
        }
        let sk = self.skinning.as_mut().expect("checked above");
        sk.own = Some(OwnPool { book, sources, outputs, prev_outputs, indices, zero_morphs, first_body, shared_place_bytes });
        Some(t0.elapsed().as_secs_f32() * 1000.0)
    }

    /// Let the own-mesh pool go (the library is read again; the engine plans it again). Its bodies
    /// leave the body list and its places' mesh ids draw nothing, so nothing holds its memory.
    pub fn skin_drop_own_pool(&mut self) {
        let Some(sk) = self.skinning.as_mut() else { return };
        let Some(own) = sk.own.take() else { return };
        let empty = sk.empty.clone();
        let ids: Vec<usize> = sk.slots[own.first_body..].iter().flatten().flat_map(|s| s.meshes.iter().copied()).collect();
        sk.bodies.truncate(own.first_body);
        sk.slots.truncate(own.first_body);
        sk.jobs.clear();
        sk.middle.clear();
        for mid in ids {
            self.replace_mesh(mid, Mesh::nothing(&empty));
        }
    }

    /// The palette holds at least `bodies` bodies of 64 joints without growing.
    fn skin_reserve_palette(&mut self, bodies: u32) {
        let rows = (bodies as u64 * (JOINTS_AT as u64 + 64) * 3).next_power_of_two().max(256);
        let Some(sk) = self.skinning.as_mut() else { return };
        if rows > sk.palette_rows {
            sk.palette = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Pose Palette"),
                size: rows * ROW_BYTES,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            sk.prev_palette = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Pose Palette Last Frame"),
                size: rows * ROW_BYTES,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            sk.palette_rows = rows;
            sk.rebind_palette(&self.device);
        }
    }

    /// What to do next to put a person's finished own mesh (`vertices`, `indices`) in the pool
    /// (`MeshPool::next_step`), in BODY ids: `have` is the body they hold (None: none yet), `kept`
    /// the (seconds out of the near tier, body) of the meshes kept for people not near, and the ids
    /// in `Step::Place` and `Step::Evict` are bodies. None while there is no pool.
    pub fn skin_own_next_step(&self, have: Option<usize>, vertices: u32, indices: u32, kept: &[(f32, usize)]) -> Option<Step> {
        let own = self.skinning.as_ref()?.own.as_ref()?;
        // A body that is not one of the pool's (none should be) counts as none held.
        let have = have.and_then(|b| own.handle_of(b)).filter(|&h| own.book.held(h).is_some());
        let kept: Vec<(f32, usize)> = kept.iter().filter_map(|&(s, b)| own.handle_of(b).map(|h| (s, h))).collect();
        Some(match own.book.next_step(have, vertices, indices, &kept) {
            Step::Place(h) => Step::Place(own.first_body + h),
            Step::Evict(h) => Step::Evict(own.first_body + h),
            other => other,
        })
    }

    /// Put a person's own mesh in the pool as body `body` (a handle `skin_own_next_step` named):
    /// its ranges taken (its last mesh's given back), its vertices and indices written there (two
    /// queue writes: no allocation), its near place's region meshes pointed at them.
    pub fn skin_own_place(&mut self, body: usize, model: &SkinnedModelData) -> Result<(), Refusal> {
        let (v, i) = (model.vertex_count() as u32, model.primitives.iter().map(|p| p.indices.len()).sum::<usize>() as u32);
        let sk = self.skinning.as_mut().ok_or(Refusal::NoHandle)?;
        let own = sk.own.as_mut().ok_or(Refusal::NoHandle)?;
        let h = own.handle_of(body).ok_or(Refusal::NoHandle)?;
        own.book.place(h, v, i)?;
        self.skin_own_write(body, model);
        Ok(())
    }

    /// The body packing `side` would move next (to write its mesh again with `skin_own_pack`).
    pub fn skin_own_pack_candidate(&self, side: Side) -> Option<usize> {
        let own = self.skinning.as_ref()?.own.as_ref()?;
        own.book.pack_candidate(side).map(|(h, _)| own.first_body + h)
    }

    /// Pack `side` of the pool down by one person, body `body` (`skin_own_pack_candidate`'s), whose
    /// mesh `model` is written again at its new ranges. False when that body was not the one to move.
    pub fn skin_own_pack(&mut self, side: Side, body: usize, model: &SkinnedModelData) -> bool {
        let Some(own) = self.skinning.as_mut().and_then(|sk| sk.own.as_mut()) else { return false };
        if own.book.pack_candidate(side).map(|(h, _)| own.first_body + h) != Some(body) {
            return false;
        }
        if own.book.pack_step(side).is_none() {
            return false;
        }
        self.skin_own_write(body, model);
        true
    }

    /// Write a pooled body's mesh at the ranges its handle holds, and point its body and near place
    /// at them.
    fn skin_own_write(&mut self, body: usize, model: &SkinnedModelData) {
        let Some(sk) = self.skinning.as_mut() else { return };
        let Some(own) = sk.own.as_ref() else { return };
        let Some(held) = own.handle_of(body).and_then(|h| own.book.held(h)) else { return };
        let mut vertices: Vec<SkinnedVertex> = Vec::with_capacity(model.vertex_count());
        let mut indices: Vec<u32> = Vec::with_capacity(held.ilen as usize);
        let mut regions = Vec::with_capacity(model.primitives.len());
        for prim in &model.primitives {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&prim.vertices);
            let first = indices.len() as u32;
            indices.extend(prim.indices.iter().map(|i| i + base));
            regions.push((own.indices.clone(), held.istart + first, prim.indices.len() as u32));
        }
        if !vertices.is_empty() {
            self.queue.write_buffer(&own.sources, held.vstart as u64 * SkinnedVertex::BYTES as u64, bytemuck::cast_slice(&vertices));
        }
        if !indices.is_empty() {
            self.queue.write_buffer(&own.indices, held.istart as u64 * 4, bytemuck::cast_slice(&indices));
        }
        let outputs = own.outputs.clone();
        let prev_outputs = own.prev_outputs.clone();
        let b = &mut sk.bodies[body];
        b.base_vertex = held.vstart;
        b.vertex_count = vertices.len() as u32;
        b.joint_count = model.joints.len() as u32;
        b.regions = regions;
        self.fill_near_slot_over(body, 0, &outputs, &prev_outputs);
    }

    /// What the own-mesh pool holds (None before it is made).
    pub fn skin_own_pool_stats(&self) -> Option<OwnPoolStats> {
        let own = self.skinning.as_ref()?.own.as_ref()?;
        Some(OwnPoolStats {
            reserved_bytes: own.book.bytes() + own.book.handles() as u64 * 16 + own.shared_place_bytes,
            used_bytes: own.book.used_bytes(),
            holders: own.book.holders(),
            people: own.book.people,
            class: own.book.class,
        })
    }

    /// Every GPU byte the skinned bodies hold: the shared bodies' buffers, their near places, the
    /// own-mesh pool and its places (`vram.characters`).
    pub fn skin_vram_bytes(&self) -> u64 {
        let Some(sk) = self.skinning.as_ref() else { return 0 };
        let first_pooled = sk.own.as_ref().map_or(sk.bodies.len(), |o| o.first_body);
        let shared: u64 = sk.bodies[..first_pooled].iter().map(|b| b.vertices.size() + b.morphs.size() + b.regions.iter().map(|(ib, _, _)| ib.size()).sum::<u64>()).sum();
        let places: u64 = sk.slots[..first_pooled].iter().flatten().map(|s| s.out.size() + s.prev.size() + s.params.size()).sum();
        let pooled = sk.own.as_ref().map_or(0, |o| o.sources.size() + o.outputs.size() + o.prev_outputs.size() + o.indices.size() + o.zero_morphs.size() + (sk.slots.len() - o.first_body) as u64 * 16);
        // The previous palette beside the palette (temporal anti-aliasing).
        shared + places + pooled + sk.prev_palette.size()
    }

    /// The buffers the skinned bodies' near places draw from (their region meshes are in the mesh
    /// registry, but their memory is `vram.characters`'s, so `vram.meshes` leaves them out).
    pub fn skin_drawn_buffers(&self) -> std::collections::HashSet<wgpu::Buffer> {
        let mut out = std::collections::HashSet::new();
        if let Some(sk) = self.skinning.as_ref() {
            for s in sk.slots.iter().flatten() {
                out.insert(s.out.clone());
            }
            out.insert(sk.empty.clone());
        }
        out
    }

    /// A body's GPU buffers from its mesh.
    fn make_skinned_body(&mut self, model: &SkinnedModelData) -> SkinnedBody {
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
            regions.push((ib, 0, indices.len() as u32));
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
        let deltas = morph_deltas(model);
        let morphs = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skinned Body Morph Targets"),
            contents: bytemuck::cast_slice(&deltas),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let sk = self.skinning.as_ref().expect("made by the caller");
        let vs_bind = Skinning::vs_bind_group(&self.device, &sk.vs_layout, &sk.vs_object, &sk.palette, &sk.prev_palette, &morphs);
        SkinnedBody { vertices: vb, base_vertex: 0, vertex_count: vertices.len() as u32, joint_count: model.joints.len() as u32, regions, morphs, vs_bind }
    }

    /// Forget every body, near place and the own-mesh pool (a reload of the character data). The
    /// mesh ids the near places registered stay in the mesh registry drawing nothing, so they hold
    /// on to no body's memory.
    pub fn skin_clear_bodies(&mut self) {
        let Some(sk) = self.skinning.as_mut() else { return };
        let empty = sk.empty.clone();
        let ids: Vec<usize> = sk.slots.iter().flatten().flat_map(|s| s.meshes.iter().copied()).collect();
        sk.bodies.clear();
        sk.slots.clear();
        sk.jobs.clear();
        sk.middle.clear();
        sk.own = None;
        for mid in ids {
            self.replace_mesh(mid, Mesh::nothing(&empty));
        }
    }

    /// GPU bytes a registered body holds (its vertices, indices and morph targets; for a person's
    /// own body, the range of the own-mesh pool it holds): the Dev page's and the rigs' figure.
    pub fn skin_body_bytes(&self, body: usize) -> u64 {
        let Some(sk) = self.skinning.as_ref() else { return 0 };
        if let Some(own) = sk.own.as_ref() {
            if let Some(h) = own.handle_of(body) {
                return own.book.held(h).map_or(0, |held| held.bytes());
            }
        }
        sk.bodies.get(body).map_or(0, |b| b.vertices.size() + b.morphs.size() + b.regions.iter().map(|(ib, _, _)| ib.size()).sum::<u64>())
    }

    /// Start a frame's skinning: nothing queued, the palette empty.
    pub fn skin_begin_frame(&mut self) {
        if let Some(sk) = self.skinning.as_mut() {
            sk.staging.clear();
            sk.keys.clear();
            sk.jobs.clear();
            sk.middle.clear();
            sk.shadow_only.clear();
            sk.dispatch_pending.set(false);
        }
    }

    /// Append one body's matrices to this frame's palette: its model matrix's rows, its morph block
    /// (`MORPH_SLOTS` matrices: which shape targets and how much), then its joints' rows (three per
    /// joint). Returns the model matrix's index; the morph block starts one after, the joints
    /// `1 + MORPH_SLOTS` after.
    /// `key` names who this is from frame to frame (the engine's figure key), so the previous palette
    /// can carry their rows from the last frame (temporal anti-aliasing's motion target).
    pub fn skin_push_palette(&mut self, key: u64, model_rows: [[f32; 4]; 3], morph_rows: &[[f32; 4]; MORPH_SLOTS as usize * 3], joint_rows: &[[f32; 4]]) -> u32 {
        let Some(sk) = self.skinning.as_mut() else { return 0 };
        let start = sk.staging.len();
        let index = (start / 3) as u32;
        sk.staging.extend_from_slice(&model_rows);
        sk.staging.extend_from_slice(morph_rows);
        sk.staging.extend_from_slice(joint_rows);
        sk.keys.push((key, start as u32, (sk.staging.len() - start) as u32));
        index
    }

    /// THE PREVIOUS PALETTE (temporal anti-aliasing, renderer/taa.rs), uploaded at the frame's start
    /// once the floating origin is known (`taa_begin_frame`): this frame's staging with each person's
    /// rows replaced by theirs from the last frame when they were drawn then with the same row count,
    /// the model rows carried into this frame's render space by `new_from_old`. `keep` false (no TAA,
    /// a cut, the reuse switch off) uploads this frame's rows: nobody moves on their own.
    pub(super) fn skin_upload_prev(&mut self, new_from_old: Mat4, keep: bool) {
        let Some(sk) = self.skinning.as_mut() else { return };
        if sk.staging.is_empty() {
            sk.last_staging.clear();
            sk.last_keys.clear();
            return;
        }
        let prev = prev_palette_rows(&sk.staging, &sk.keys, &sk.last_staging, &sk.last_keys, new_from_old, keep);
        if prev.len() as u64 <= sk.palette_rows {
            self.queue.write_buffer(&sk.prev_palette, 0, bytemuck::cast_slice(&prev));
        }
        sk.last_staging.clone_from(&sk.staging);
        sk.last_keys = sk.keys.iter().map(|&(k, s, n)| (k, (s, n))).collect();
    }

    /// Skin a body by compute this frame in its near place `slot` (made on first use; places are
    /// numbered per body by the engine each frame, so a place always skins the body it was made
    /// for), with its matrices at `model_index` in the palette. Returns its regions' mesh ids, to draw
    /// as ordinary objects placed by the body's transform.
    pub fn skin_near(&mut self, body: usize, slot: usize, model_index: u32) -> Vec<usize> {
        let fresh = match self.skinning.as_ref() {
            None => return Vec::new(),
            Some(sk) => {
                if body >= sk.bodies.len() {
                    return Vec::new();
                }
                slot >= sk.slots[body].len()
            }
        };
        let mut slot = slot;
        if fresh {
            let made = self.make_near_slot(body);
            let sk = self.skinning.as_mut().expect("checked above");
            sk.slots[body].push(made);
            slot = sk.slots[body].len() - 1;
        }
        let sk = self.skinning.as_mut().expect("checked above");
        sk.jobs.push((body, slot, model_index));
        let s = &sk.slots[body][slot];
        s.meshes[..s.regions].to_vec()
    }

    /// Draw a middle body's region this frame with the VS PSO, or, while that PSO is not ready,
    /// say so (`false`) so the caller skins it by compute instead.
    pub fn skin_middle_ready(&mut self) -> bool {
        let format = self.scene_format();
        let Some(sk) = self.skinning.as_mut() else { return false };
        if let Polled::Lost = sk.vs_pso.poll() {
            log::warn!("Skinning: the vertex-shader skinning PSO build was lost; middle bodies stay on compute skinning");
        }
        // Built for another scene format (the scene-format A/B switch), or with the shadow atlas
        // lookups in while Room light shadows went Off or out while it came back (shadows rung 1b):
        // build it again for this one, compute skinning standing in meanwhile.
        let atlas = self.pipeline.shadow_atlas_wanted();
        let stale = matches!(sk.vs_pso.ready(), Some(p) if p.format != format || p.atlas != atlas) && !sk.vs_pso.is_pending();
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

    /// Queue one region of a body for the SHADOW MAPS ONLY (2026-11-10), skinned in the vertex shader
    /// from the palette at `model_index`: the first-person body's unfolded twin, so your shadow keeps
    /// the head the eye camera's copy folds away. Never drawn in a colour pass. Call only when
    /// `shadow_casters_wanted()` (the palette rows cost a body's worth otherwise).
    pub fn skin_shadow_only(&mut self, body: usize, region: usize, material: usize, model_index: u32) {
        if let Some(sk) = self.skinning.as_mut() {
            sk.shadow_only.push(MiddleDraw { body, region, material, model_index });
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
            // Sized for a crowd when the own-mesh pool is made (`skin_reserve_palette`), so this
            // grows only past that.
            let grown = rows.next_power_of_two().max(256);
            sk.palette = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Pose Palette"),
                size: grown * ROW_BYTES,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            sk.prev_palette = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Pose Palette Last Frame"),
                size: grown * ROW_BYTES,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            sk.palette_rows = grown;
            log::info!("Skinning: the pose palette grew to {grown} rows ({:.2} MB) on the frame", (grown * ROW_BYTES) as f64 / 1_048_576.0);
            sk.rebind_palette(&self.device);
        }
        self.queue.write_buffer(&sk.palette, 0, bytemuck::cast_slice(&sk.staging));
        for &(body, slot, model_index) in &sk.jobs {
            let s = &sk.slots[body][slot];
            let b = &sk.bodies[body];
            // The fourth word: where the body's vertices start in its buffers (its range of the
            // own-mesh pool; 0 for a body with buffers of its own).
            self.queue.write_buffer(&s.params, 0, bytemuck::cast_slice(&[b.vertex_count, model_index + JOINTS_AT, model_index + MORPH_BLOCK_AT, b.base_vertex]));
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
        for &(body, slot, _) in &sk.jobs {
            let s = &sk.slots[body][slot];
            let count = sk.bodies[body].vertex_count;
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
        let mut bound_material = usize::MAX;
        let mut bound_body = usize::MAX;
        for d in &sk.middle {
            let Some(body) = sk.bodies.get(d.body) else { continue };
            let Some((ib, first, count)) = body.regions.get(d.region) else { continue };
            let Some(material) = self.materials.get(d.material) else { continue };
            if bound_material != d.material {
                bound_material = d.material;
                pass.set_bind_group(2, &material.bind_group, &[]);
                pass.set_bind_group(3, material.albedo_group().unwrap_or(&self.default_texture_bind_group), &[]);
            }
            if bound_body != d.body {
                bound_body = d.body;
                pass.set_bind_group(1, &body.vs_bind, &[]);
                pass.set_vertex_buffer(0, body.vertices.slice(..));
            }
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            // A pooled body's vertices start at `base_vertex` (its range of the own-mesh pool). The
            // VS path reads its shape targets by vertex index, which `base_vertex` shifts; a pooled
            // body has none on (an own mesh's shape is in its vertices: its morph block is empty).
            pass.draw_indexed(*first..*first + *count, body.base_vertex as i32, d.model_index..d.model_index + 1);
        }
    }

    /// Draw the skinned bodies the scene list does not hold into a shadow view whose groups 0 and 1
    /// the caller set (2026-11-10, renderer/local_shadows.rs): the middle tier and the shadow-only
    /// twins, each kept when `keep(centre, radius)` says its bounding sphere touches the view (the
    /// centre read from its model matrix in this frame's palette). Depth only, the card hair through
    /// `fs_shadow`. Returns how many draws it made. Nothing while the skinned PSOs are building.
    pub(super) fn draw_skinned_shadow(&self, pass: &mut wgpu::RenderPass<'_>, keep: impl Fn(Vec3, f32) -> bool) -> u32 {
        let Some(sk) = self.skinning.as_ref() else { return 0 };
        if sk.middle.is_empty() && sk.shadow_only.is_empty() {
            return 0;
        }
        let Some(pso) = sk.vs_pso.ready() else { return 0 };
        let mut draws = 0u32;
        for cutout in [false, true] {
            let mut bound_pso = false;
            let mut bound_material = usize::MAX;
            let mut bound_body = usize::MAX;
            for d in sk.middle.iter().chain(sk.shadow_only.iter()) {
                let Some(material) = self.materials.get(d.material) else { continue };
                if material.casts_cutout_shadow() != cutout {
                    continue;
                }
                let Some(body) = sk.bodies.get(d.body) else { continue };
                let Some((ib, first, count)) = body.regions.get(d.region) else { continue };
                let Some((centre, radius)) = shadow_sphere(&sk.staging, d.model_index) else { continue };
                if !keep(centre, radius) {
                    continue;
                }
                if !bound_pso {
                    bound_pso = true;
                    pass.set_pipeline(if cutout { &pso.shadow_alpha } else { &pso.shadow });
                }
                if bound_material != d.material {
                    bound_material = d.material;
                    pass.set_bind_group(2, &material.bind_group, &[]);
                    pass.set_bind_group(3, material.shadow_albedo_group().unwrap_or(&self.shadow_pass_texture_bind_group), &[]);
                }
                if bound_body != d.body {
                    bound_body = d.body;
                    pass.set_bind_group(1, &body.vs_bind, &[]);
                    pass.set_vertex_buffer(0, body.vertices.slice(..));
                }
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(*first..*first + *count, body.base_vertex as i32, d.model_index..d.model_index + 1);
                draws += 1;
            }
        }
        draws
    }

    /// The bounding spheres `draw_skinned_shadow` tests, one per draw it could make (the middle tier
    /// and the shadow-only twins), so the atlas knows which views have a body to draw before it binds
    /// anything for them (renderer/local_shadows.rs).
    pub(super) fn skinned_shadow_spheres(&self) -> Vec<(Vec3, f32)> {
        let Some(sk) = self.skinning.as_ref() else { return Vec::new() };
        if sk.vs_pso.ready().is_none() {
            return Vec::new();
        }
        sk.middle.iter().chain(sk.shadow_only.iter()).filter_map(|d| shadow_sphere(&sk.staging, d.model_index)).collect()
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
        let prev = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Skinned Body Output Last Frame"),
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
        let bind = Skinning::compute_bind_group(&self.device, &sk.compute_layout, (&sk.palette, &sk.prev_palette), &b.vertices, (&out, &prev), &params, &b.morphs);
        // Buffer handles are shared clones (wgpu 24), so the region meshes and this place draw the
        // same GPU memory; collected before the registry is borrowed to add them.
        let regions: Vec<(wgpu::Buffer, u32, u32)> = b.regions.clone();
        let base_vertex = b.base_vertex as i32;
        let n = regions.len();
        let meshes = regions
            .into_iter()
            .map(|(index_buffer, first_index, index_count)| {
                self.add_mesh(Mesh { vertex_buffer: out.clone(), prev_vertex_buffer: Some(prev.clone()), index_buffer, index_count, first_index, base_vertex, bounds: BODY_BOUNDS })
            })
            .collect();
        NearSlot { params, bind, out, prev, meshes, regions: n }
    }

    /// Point near place `slot` of `body` at the body's current regions over the output buffer
    /// `out` it already has (a pooled body written at new ranges): its region meshes replaced in
    /// place, a mesh id added only when the body has more regions than the place has ever had (a
    /// registry entry, no GPU memory), spare ids drawing nothing. No allocation.
    fn fill_near_slot_over(&mut self, body: usize, slot: usize, out: &wgpu::Buffer, prev: &wgpu::Buffer) {
        let Some(sk) = self.skinning.as_ref() else { return };
        let b = &sk.bodies[body];
        let Some(s) = sk.slots[body].get(slot) else { return };
        let regions: Vec<(wgpu::Buffer, u32, u32)> = b.regions.clone();
        let base_vertex = b.base_vertex as i32;
        let empty = sk.empty.clone();
        let mut ids = s.meshes.clone();
        let n = regions.len();
        for (k, (index_buffer, first_index, index_count)) in regions.into_iter().enumerate() {
            let mesh = Mesh { vertex_buffer: out.clone(), prev_vertex_buffer: Some(prev.clone()), index_buffer, index_count, first_index, base_vertex, bounds: BODY_BOUNDS };
            if k < ids.len() {
                self.replace_mesh(ids[k], mesh);
            } else {
                ids.push(self.add_mesh(mesh));
            }
        }
        for &id in ids.iter().skip(n) {
            self.replace_mesh(id, Mesh::nothing(&empty));
        }
        let sk = self.skinning.as_mut().expect("checked above");
        let s = &mut sk.slots[body][slot];
        s.meshes = ids;
        s.regions = n;
    }
}

/// THE PREVIOUS PALETTE'S ROWS (`Renderer::skin_upload_prev`), pure so it is tested without a GPU:
/// this frame's `staging` with each key's rows replaced by its rows in `last` when it was there with
/// the same count, the first three (its model matrix) carried by `new_from_old`. `keep` false gives
/// this frame's rows back unchanged.
fn prev_palette_rows(
    staging: &[[f32; 4]],
    keys: &[(u64, u32, u32)],
    last: &[[f32; 4]],
    last_keys: &std::collections::HashMap<u64, (u32, u32)>,
    new_from_old: Mat4,
    keep: bool,
) -> Vec<[f32; 4]> {
    let mut prev = staging.to_vec();
    if !keep {
        return prev;
    }
    for &(key, start, rows) in keys {
        let Some(&(ls, lr)) = last_keys.get(&key) else { continue };
        if lr != rows || rows < 3 {
            continue;
        }
        let (s, l, n) = (start as usize, ls as usize, rows as usize);
        if l + n > last.len() || s + n > prev.len() {
            continue;
        }
        prev[s..s + n].copy_from_slice(&last[l..l + n]);
        let carried = carry_model_rows(new_from_old, [prev[s], prev[s + 1], prev[s + 2]]);
        prev[s..s + 3].copy_from_slice(&carried);
    }
    prev
}

/// Three palette rows of a model matrix (an affine 3 x 4, row by row, as `skin_apply` dots them)
/// carried by `m` (last frame's render space to this frame's, renderer/taa.rs).
fn carry_model_rows(m: Mat4, rows: [[f32; 4]; 3]) -> [[f32; 4]; 3] {
    let model = Mat4::from_cols_array_2d(&[rows[0], rows[1], rows[2], [0.0, 0.0, 0.0, 1.0]]).transpose();
    let t = (m * model).transpose().to_cols_array_2d();
    [t[0], t[1], t[2]]
}

/// A body's morph targets as the shaders read them: for target k and vertex i (the primitives'
/// vertices one after another, as the vertex buffer holds them), the position delta and the normal
/// delta as two `vec4` at (k * vertices + i) * 2. One zero pair for a body without targets.
pub fn morph_deltas(model: &SkinnedModelData) -> Vec<[f32; 4]> {
    let targets = model.primitives.first().map_or(0, |p| p.morphs.len());
    let n = model.vertex_count();
    if targets == 0 || n == 0 {
        return vec![[0.0; 4]; 2];
    }
    let mut out = Vec::with_capacity(targets * n * 2);
    for k in 0..targets {
        for prim in &model.primitives {
            match prim.morphs.get(k) {
                Some(m) => {
                    for i in 0..prim.vertices.len() {
                        let (p, q) = (m.positions[i], m.normals[i]);
                        out.push([p[0], p[1], p[2], 0.0]);
                        out.push([q[0], q[1], q[2], 0.0]);
                    }
                }
                None => out.extend(std::iter::repeat([0.0; 4]).take(prim.vertices.len() * 2)),
            }
        }
    }
    out
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
                // The body's morph targets.
                storage_entry(4, wgpu::ShaderStages::COMPUTE, true),
                // The previous palette and the vertices skinned by it (temporal anti-aliasing).
                storage_entry(5, wgpu::ShaderStages::COMPUTE, true),
                storage_entry(6, wgpu::ShaderStages::COMPUTE, false),
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
        let prev_palette = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Pose Palette Last Frame"),
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
                // The body's morph targets.
                storage_entry(2, wgpu::ShaderStages::VERTEX, true),
                // The previous palette (temporal anti-aliasing's motion target).
                storage_entry(3, wgpu::ShaderStages::VERTEX, true),
            ],
        });
        let identity = ObjectUniforms {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            normal_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            prev_model: Mat4::IDENTITY.to_cols_array_2d(),
        };
        let vs_object = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skinned Object Identity"),
            contents: bytemuck::bytes_of(&identity),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let vs_pso = Self::spawn_vs_pso(r, &vs_layout);
        let empty = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Skinned Nothing"),
            size: 16,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
            mapped_at_creation: false,
        });
        Skinning {
            bodies: Vec::new(),
            compute,
            compute_layout,
            palette,
            palette_rows,
            staging: Vec::new(),
            prev_palette,
            keys: Vec::new(),
            last_staging: Vec::new(),
            last_keys: std::collections::HashMap::new(),
            slots: Vec::new(),
            jobs: Vec::new(),
            dispatch_pending: std::cell::Cell::new(false),
            middle: Vec::new(),
            shadow_only: Vec::new(),
            vs_layout,
            vs_object,
            vs_pso,
            own: None,
            empty,
        }
    }

    /// Every bind group naming the palette made again over the current one (it was made larger).
    fn rebind_palette(&mut self, device: &wgpu::Device) {
        for b in 0..self.bodies.len() {
            let body = &self.bodies[b];
            let vs_bind = Skinning::vs_bind_group(device, &self.vs_layout, &self.vs_object, &self.palette, &self.prev_palette, &body.morphs);
            let binds: Vec<wgpu::BindGroup> = self.slots[b]
                .iter()
                .map(|slot| Skinning::compute_bind_group(device, &self.compute_layout, (&self.palette, &self.prev_palette), &body.vertices, (&slot.out, &slot.prev), &slot.params, &body.morphs))
                .collect();
            self.bodies[b].vs_bind = vs_bind;
            for (slot, bind) in self.slots[b].iter_mut().zip(binds) {
                slot.bind = bind;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// `palettes` is (this frame's, last frame's), `outs` the vertices skinned by each.
    #[allow(clippy::too_many_arguments)]
    fn compute_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        palettes: (&wgpu::Buffer, &wgpu::Buffer),
        source: &wgpu::Buffer,
        outs: (&wgpu::Buffer, &wgpu::Buffer),
        params: &wgpu::Buffer,
        morphs: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Skinning Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: palettes.0.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: source.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: outs.0.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: morphs.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: palettes.1.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: outs.1.as_entire_binding() },
            ],
        })
    }

    fn vs_bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, object: &wgpu::Buffer, palette: &wgpu::Buffer, prev_palette: &wgpu::Buffer, morphs: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skinned Object Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: object.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: palette.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: morphs.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: prev_palette.as_entire_binding() },
            ],
        })
    }

    /// Build the VS PSO on a thread of its own: the megashader with `vs_skinned` appended, the
    /// surface class's fragment entry and permutation (its own Surface-class row in PSO_REGISTRY), the opaque
    /// state, and the class PSOs' groups 0, 2 and 3 around this path's own group 1.
    fn spawn_vs_pso(r: &Renderer, vs_layout: &wgpu::BindGroupLayout) -> Deferred<SkinnedPso> {
        let device = r.device.clone();
        let format = r.scene_format();
        let atlas = r.pipeline.shadow_atlas_wanted();
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
                let pipeline = pipeline::Pipeline::build_skinned_surface_pso(&device, format, &module, &layout, &SKINNED_ATTRS, SKINNED_STRIDE, atlas);
                // Its two shadow twins (2026-11-10, renderer/local_shadows.rs), from the same module.
                let shadow = pipeline::Pipeline::build_skinned_shadow_pso(&device, &module, &layout, &SKINNED_ATTRS, SKINNED_STRIDE, false);
                let shadow_alpha = pipeline::Pipeline::build_skinned_shadow_pso(&device, &module, &layout, &SKINNED_ATTRS, SKINNED_STRIDE, true);
                log::info!("[Pipelines] vertex-shader skinning PSOs (colour and two shadow) built in {:.1}s (background)", started.elapsed().as_secs_f32());
                let _ = tx.send(SkinnedPso { pipeline, format, atlas, shadow, shadow_alpha });
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

/// A skinned body's bounding sphere for the shadow views, from its model matrix (the palette's three
/// rows at `model_index`): a `BODY_RADIUS_M` sphere about `BODY_CENTRE` lifted along its up axis,
/// scaled with the body (renderer/local_shadows.rs).
fn shadow_sphere(staging: &[[f32; 4]], model_index: u32) -> Option<(Vec3, f32)> {
    let r = model_index as usize * 3;
    let rows = staging.get(r..r + 3)?;
    let centre = Vec3::new(rows[0][3], rows[1][3], rows[2][3]);
    let scale = Vec3::new(rows[0][0], rows[1][0], rows[2][0]).length();
    let lift = Vec3::new(rows[0][1], rows[1][1], rows[2][1]) * super::local_shadows::BODY_CENTRE[1];
    Some((centre + lift, super::local_shadows::BODY_RADIUS_M * scale.max(0.1)))
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
    /// The previous palette (temporal anti-aliasing): a person found by key gets LAST frame's rows at
    /// THIS frame's place in the palette, their placement carried into this frame's render space;
    /// someone new keeps this frame's rows; `keep` false (a cut, the reuse switch off) changes nothing.
    /// Seen red 2026-11-13 with the copy reading this frame's place in `last` (`l` taken from `start`):
    /// the two people swapped places in the palette and each was given the other's last pose.
    #[test]
    fn the_previous_palette_follows_each_person_by_key() {
        let row = |v: f32| [v, v, v, v];
        let ident = |t: f32| [[1.0, 0.0, 0.0, t], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
        // Last frame: A at rows 0..4 (model at x = 1, one joint row 10), B at rows 4..8 (x = 2, 20).
        let mut last = Vec::new();
        last.extend_from_slice(&ident(1.0));
        last.push(row(10.0));
        last.extend_from_slice(&ident(2.0));
        last.push(row(20.0));
        let last_keys: std::collections::HashMap<u64, (u32, u32)> = [(7u64, (0u32, 4u32)), (9, (4, 4))].into_iter().collect();
        // This frame: B first, then A, then C (new).
        let mut now = Vec::new();
        now.extend_from_slice(&ident(2.5));
        now.push(row(21.0));
        now.extend_from_slice(&ident(1.5));
        now.push(row(11.0));
        now.extend_from_slice(&ident(3.0));
        now.push(row(30.0));
        let keys = [(9u64, 0u32, 4u32), (7, 4, 4), (5, 8, 4)];
        let shift = Mat4::from_translation(Vec3::new(-0.25, 0.0, 0.0));
        let prev = super::prev_palette_rows(&now, &keys, &last, &last_keys, shift, true);
        assert_eq!(prev[3], row(20.0), "B's joint row is B's from last frame");
        assert!((prev[0][3] - 1.75).abs() < 1e-6, "B's placement last frame (x = 2), carried by -0.25: {:?}", prev[0]);
        assert_eq!(prev[7], row(10.0), "A's joint row is A's");
        assert!((prev[4][3] - 0.75).abs() < 1e-6, "A's placement: {:?}", prev[4]);
        assert_eq!(&prev[8..12], &now[8..12], "someone new moves with nobody's past");
        assert_eq!(super::prev_palette_rows(&now, &keys, &last, &last_keys, shift, false), now, "keep false: this frame's rows");
    }

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
