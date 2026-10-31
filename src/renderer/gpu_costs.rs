//! The GPU's side of the frame costs (resource budgets, increment 1; see
//! `docs/design/resource-budgets.md`): the wgpu timestamp ring that times each render and
//! compute pass a frame late, the per-view pass ids (`SceneView`), a texture's bytes, and the
//! `Renderer` plumbing that times passes and walks its own buffers for the VRAM pie.
//!
//! Everything it measures is published into the process-wide store, `crate::frame_costs`,
//! which is the core's since the crate split's rung 0 (this file and that one were
//! `renderer::frame_costs` together): the store is std only and every crate records into it;
//! this half needs wgpu and stays in the renderer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// The store this half publishes into (record, publish, frame boundaries, the VRAM entries).
use crate::frame_costs::*;

// ────────────────────────────── GPU timestamps ──────────────────────────────

/// How many render/compute passes can be timed per frame. Each takes two
/// timestamp slots (begin + end). A world frame with the cloud caches uses
/// ~22 passes, the far rung's calibration frame 2 more and its mip burst 6
/// more; the in-world screens add one egui pass PER FRAMED SCREEN (up to six
/// in the console room) plus the camera screen's sky, scene and transparent
/// passes, and the particle sim and sky-view LUT passes are timed too since
/// the 2026-09-18 instrumentation pass. 64 leaves room for all of that at
/// once; the cost is 1 KB of query slots.
const MAX_TIMED_PASSES: usize = 64;

/// Which frame a scene-object pass (opaque or transparent) belongs to, so
/// its cost is published under its OWN key instead of being summed into the
/// live frame's. The camera wall screen re-renders the scene at 10 Hz
/// through the same `render_scene_onto` / `render_transparent_onto` the
/// live frame uses; before this enum both landed in `gpu.scene`, and a
/// 17 ms camera re-render was invisible as its own number (the 2026-09-18
/// measurement at the operator's settings).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SceneView {
    /// The live window frame (and the hi-res screenshot, which renders the
    /// same view once): `gpu.scene` / `gpu.transparent`.
    Main,
    /// An in-world camera screen's re-render: `gpu.screen_scene` /
    /// `gpu.screen_transparent`, with matching `cpu.*` stages so the
    /// no-timestamp fallback rule (`gpu.x` reads `cpu.x`) still holds.
    Screen,
    /// A Maps plan tile's bake (Maps increment 3, engine/map_tiles.rs): one
    /// 512 px render at most a frame while the plan is shown, its own number
    /// (`gpu.map_tile_scene`, ...) so its budget (2 ms) is read on its own.
    MapTile,
}

impl SceneView {
    /// (GPU pass id, CPU stage id) for the opaque scene pass.
    pub fn scene_ids(self) -> (&'static str, &'static str) {
        match self {
            SceneView::Main => ("gpu.scene", "cpu.scene"),
            SceneView::Screen => ("gpu.screen_scene", "cpu.screen_scene"),
            SceneView::MapTile => ("gpu.map_tile_scene", "cpu.map_tile_scene"),
        }
    }

    /// (GPU pass id, CPU stage id) for the transparent pass.
    pub fn transparent_ids(self) -> (&'static str, &'static str) {
        match self {
            SceneView::Main => ("gpu.transparent", "cpu.transparent"),
            SceneView::Screen => ("gpu.screen_transparent", "cpu.screen_transparent"),
            SceneView::MapTile => ("gpu.map_tile_transparent", "cpu.map_tile_transparent"),
        }
    }

    /// (GPU pass id, CPU stage id) for the sky (star) pass that opens a view:
    /// the live frame and the screenshot keep `gpu.stars` / `cpu.stars`; a
    /// camera screen's sky is its own number.
    pub fn sky_ids(self) -> (&'static str, &'static str) {
        match self {
            SceneView::Main => ("gpu.stars", "cpu.stars"),
            SceneView::Screen => ("gpu.screen_sky", "cpu.screen_sky"),
            SceneView::MapTile => ("gpu.map_tile_clear", "cpu.map_tile_clear"),
        }
    }

    /// (GPU pass id, CPU stage id) for the editor-gizmo overlay pass. A camera
    /// screen draws the overlay too (`render_view_onto`), and before these
    /// keys its cost was summed into the live frame's `gpu.overlay`.
    pub fn overlay_ids(self) -> (&'static str, &'static str) {
        match self {
            SceneView::Main => ("gpu.overlay", "cpu.overlay"),
            SceneView::Screen => ("gpu.screen_overlay", "cpu.screen_overlay"),
            SceneView::MapTile => ("gpu.map_tile_overlay", "cpu.map_tile_overlay"),
        }
    }

    /// (GPU pass id, CPU stage id) for the ring / guide line pass, same split.
    pub fn lines_ids(self) -> (&'static str, &'static str) {
        match self {
            SceneView::Main => ("gpu.lines", "cpu.lines"),
            SceneView::Screen => ("gpu.screen_lines", "cpu.screen_lines"),
            SceneView::MapTile => ("gpu.map_tile_lines", "cpu.map_tile_lines"),
        }
    }

    /// (GPU pass id, CPU stage id) for the present pass that copies the
    /// view's scene target into its display target (scene_target.rs,
    /// 2026-09-27): the live frame and the screenshot are `gpu.present`, a
    /// camera screen's copy is its own number like its other passes.
    pub fn present_ids(self) -> (&'static str, &'static str) {
        match self {
            SceneView::Main => ("gpu.present", "cpu.present"),
            SceneView::Screen => ("gpu.screen_present", "cpu.screen_present"),
            SceneView::MapTile => ("gpu.map_tile_present", "cpu.map_tile_present"),
        }
    }
}

/// wgpu timestamp-query ring. One query set is enough because we resolve the
/// PREVIOUS frame's writes at the start of the next frame and read the result
/// asynchronously: at no point does the CPU wait on the GPU.
pub struct GpuTimers {
    query_set: wgpu::QuerySet,
    /// Destination of `resolve_query_set` (GPU-side, not mappable).
    resolve_buf: wgpu::Buffer,
    /// Mappable copy the CPU reads from.
    read_buf: wgpu::Buffer,
    /// Nanoseconds per timestamp tick, from `Queue::get_timestamp_period`.
    period_ns: f32,
    state: Mutex<TimerState>,
    /// Set by the map callback when `read_buf` is ready to read.
    ready: Arc<AtomicBool>,
}

/// The ring's bookkeeping, kept apart from the wgpu objects so its rules
/// (which frame a batch belongs to, what happens when the slots run out) can
/// be tested without a GPU adapter.
#[derive(Default)]
struct TimerState {
    /// Passes timed during the frame in progress: (id, begin slot index).
    pending: Vec<(&'static str, u32)>,
    /// Whether the frame in progress (the one filling `pending`) is a WORLD
    /// frame. Set at each boundary from the caller's `world` flag; the value
    /// travels with the batch when it goes in flight, because the harvest
    /// two boundaries later must know whether to publish it.
    frame_world: bool,
    /// Passes whose timestamps are resolving on the GPU right now.
    in_flight: Vec<(&'static str, u32)>,
    /// The frame kind of the `in_flight` batch (see `frame_world`).
    in_flight_world: bool,
    /// Next free slot index in the query set.
    next: u32,
    /// Whether `read_buf` is currently mapped by us.
    mapped: bool,
    /// Slot-pair requests refused this frame because the ring was full. Each
    /// one is a pass that ran untimed and reads as "free" on the pie, so the
    /// count is published (world frames) instead of being swallowed.
    dropped: u32,
}

/// What a frame hands back at its boundary (`TimerState::close_frame`).
struct FrameEnd {
    /// Whether the frame that just ended was a world frame.
    world: bool,
    /// Its refused slot requests.
    untimed: u32,
    /// Its timed passes and how many slots they used, when the ring is free
    /// to resolve them; `None` when the previous readback is still pending
    /// (that frame's samples are dropped, never aliased onto the next).
    resolve: Option<(Vec<(&'static str, u32)>, u32)>,
}

/// Set by `Renderer::mark_view_frame`, taken at the next frame boundary.
static VIEW_FRAME: AtomicBool = AtomicBool::new(false);

/// A world view was rendered in the frame in progress.
pub(crate) fn note_view_frame() {
    VIEW_FRAME.store(true, Ordering::Relaxed);
}

/// Whether a world view was rendered since the last boundary (and clear it).
pub(crate) fn take_view_frame() -> bool {
    VIEW_FRAME.swap(false, Ordering::Relaxed)
}

/// At a frame boundary: is the frame that just ended published (a world frame,
/// or a page frame that rendered a world view) rather than discarded?
pub(crate) fn publish_cpu_frame(world: bool) -> bool {
    world || take_view_frame()
}

impl TimerState {
    /// The frame in progress drew a world view: publish its batch as a world
    /// frame's when it closes (`Renderer::mark_view_frame`).
    fn mark_world(&mut self) {
        self.frame_world = true;
    }

    /// Claim a begin/end slot pair for `id`, returning the begin index, or
    /// count the refusal when the ring is full.
    fn take_pair(&mut self, id: &'static str) -> Option<u32> {
        if self.next as usize + 2 > MAX_TIMED_PASSES * 2 {
            self.dropped += 1;
            return None;
        }
        let begin = self.next;
        self.next += 2;
        self.pending.push((id, begin));
        Some(begin)
    }

    /// Frame boundary: close the frame that just ended (its batch, its kind,
    /// its refusal count) and open the next one as `next_world`. The slots
    /// are always reclaimed, whether or not the batch can be resolved, so a
    /// UI-only frame neither leaks slots nor lets its passes alias onto the
    /// next frame's.
    fn close_frame(&mut self, next_world: bool) -> FrameEnd {
        let world = self.frame_world;
        let untimed = self.dropped;
        let pending = std::mem::take(&mut self.pending);
        let used = self.next;
        self.next = 0;
        self.dropped = 0;
        self.frame_world = next_world;
        let resolve = if self.in_flight.is_empty() && !self.mapped && !pending.is_empty() && used > 0 {
            Some((pending, used))
        } else {
            None
        };
        FrameEnd { world, untimed, resolve }
    }

    /// The batch whose resolve was submitted goes in flight, tagged with the
    /// kind of the frame that wrote it.
    fn mark_in_flight(&mut self, batch: Vec<(&'static str, u32)>, world: bool) {
        self.in_flight = batch;
        self.in_flight_world = world;
        self.mapped = true;
    }

    /// Hand the in-flight batch to the harvest and clear it: (passes, whether
    /// they were a world frame's).
    fn take_in_flight(&mut self) -> (Vec<(&'static str, u32)>, bool) {
        self.mapped = false;
        (std::mem::take(&mut self.in_flight), self.in_flight_world)
    }
}

impl GpuTimers {
    /// Build the query set, or `None` when the device was created without
    /// `TIMESTAMP_QUERY` (the caller then reports the CPU fallback).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let count = (MAX_TIMED_PASSES * 2) as u32;
        let bytes = count as u64 * 8;
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Frame Cost Timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count,
        });
        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Frame Cost Query Resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Frame Cost Query Readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Some(Self {
            query_set,
            resolve_buf,
            read_buf,
            period_ns: queue.get_timestamp_period(),
            state: Mutex::new(TimerState::default()),
            ready: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Timestamp writes to hand a render pass descriptor. Returns `None` once
    /// the frame's slots are exhausted (that pass is untimed, and the refusal
    /// is counted: see `TimerState::dropped`).
    pub fn writes(&self, id: &'static str) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let mut st = self.state.lock().ok()?;
        let begin = st.take_pair(id)?;
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        })
    }

    /// The compute-pass twin of `writes`: the same slot pair, handed to a
    /// `ComputePassDescriptor` (wgpu keeps the two descriptor types distinct
    /// even though they carry identical fields). Used by the GPU particle
    /// sim, the one compute pass the frame submits.
    pub fn compute_writes(
        &self,
        id: &'static str,
    ) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        let mut st = self.state.lock().ok()?;
        let begin = st.take_pair(id)?;
        Some(wgpu::ComputePassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        })
    }

    /// Frame boundary: harvest whatever finished, then kick off the resolve of
    /// the frame that just ended. Never blocks - `Maintain::Poll` only drains
    /// callbacks that are already complete.
    ///
    /// `world` says whether the frame that is STARTING renders the 3D world
    /// (`Renderer::acquire_surface`) or only clears the surface for a
    /// full-screen page (`acquire_surface_cleared`). The kind is remembered
    /// per batch and consulted when that batch's samples come back two
    /// boundaries later: a world frame's are published, a UI-only frame's are
    /// discarded so the column stays frozen (`publish_gpu_harvest`). The
    /// slots and the readback are handled identically for both kinds; only
    /// the publish differs.
    pub fn begin_frame(&self, device: &wgpu::Device, queue: &wgpu::Queue, world: bool) {
        let _ = device.poll(wgpu::Maintain::Poll);
        let mut st = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        // 1. Harvest a completed readback into the store.
        if self.ready.swap(false, Ordering::Acquire) {
            let (batch, batch_world) = st.take_in_flight();
            let mut samples: Vec<(&'static str, f32)> = Vec::new();
            {
                let view = self.read_buf.slice(..).get_mapped_range();
                // Read each 8-byte tick by hand rather than casting the slice:
                // a mapped range carries no alignment guarantee, and a
                // bytemuck cast would panic on an unaligned one.
                let tick = |i: usize| -> Option<u64> {
                    let o = i * 8;
                    view.get(o..o + 8)
                        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
                };
                for (id, begin) in batch.iter() {
                    let (b, e) = (*begin as usize, *begin as usize + 1);
                    if let (Some(t0), Some(t1)) = (tick(b), tick(e)) {
                        if t1 >= t0 {
                            let ns = (t1 - t0) as f64 * self.period_ns as f64;
                            samples.push((id, (ns / 1.0e6) as f32));
                        }
                    }
                }
            }
            self.read_buf.unmap();
            // Published as ONE frame so same-id passes sum and absent passes
            // decay, but ONLY for a world frame's batch; a UI-only frame's is
            // discarded (see `publish_gpu_harvest` for why).
            publish_gpu_harvest(&samples, batch_world);
        }

        // 2. Close the frame that just ended and open the new one. Its
        //    refused-slot count is published for world frames only, for the
        //    same reason as the samples: the page must show the last world
        //    frame's truth, not the UI-only frame's (which drops nothing).
        let end = st.close_frame(world);
        if end.world {
            set_gpu_untimed(end.untimed);
        }

        // 3. Resolve the batch, if the previous readback is done.
        if let Some((batch, used)) = end.resolve {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Cost Resolve"),
            });
            enc.resolve_query_set(&self.query_set, 0..used, &self.resolve_buf, 0);
            enc.copy_buffer_to_buffer(
                &self.resolve_buf,
                0,
                &self.read_buf,
                0,
                used as u64 * 8,
            );
            queue.submit(std::iter::once(enc.finish()));
            let flag = self.ready.clone();
            self.read_buf.slice(..).map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    flag.store(true, Ordering::Release);
                }
            });
            st.mark_in_flight(batch, end.world);
        }
    }
}

/// Bytes a texture occupies, mip levels and array layers included. Good enough
/// for an inventory: it is the logical size, not the driver's padded footprint,
/// which is exactly why the pie also shows the allocator's untracked remainder.
pub fn texture_bytes(tex: &wgpu::Texture) -> u64 {
    let size = tex.size();
    let block = tex.format().block_copy_size(None).unwrap_or(4) as u64;
    let (bw, bh) = tex.format().block_dimensions();
    let mut total = 0u64;
    for mip in 0..tex.mip_level_count() {
        let w = (size.width >> mip).max(1) as u64;
        let h = (size.height >> mip).max(1) as u64;
        let d = (size.depth_or_array_layers >> mip).max(1) as u64;
        let blocks_w = w.div_ceil(bw as u64);
        let blocks_h = h.div_ceil(bh as u64);
        total += blocks_w * blocks_h * d * block;
    }
    total
}

// ───────────────────────── Renderer-side plumbing ─────────────────────────
// A child module can see its parent's private items, so the inventory walk
// lives HERE (reading `Renderer`'s private buffers directly) instead of adding
// a hundred lines to `renderer/mod.rs`, which sits under a file-size ratchet.

use super::Renderer;

impl Renderer {
    /// This frame rendered a view of the WORLD although it is a UI-only frame
    /// (Maps increment 1: the Maps page's world layer, `engine/map_view.rs`).
    /// Without it the page frame's costs were thrown away like any menu's,
    /// so the map's render could never be measured. Marks both halves: the
    /// GPU passes timed so far this frame are published, and the CPU stages
    /// are folded in at the next frame boundary instead of discarded.
    pub(crate) fn mark_view_frame(&self) {
        note_view_frame();
        if let Some(t) = self.gpu_timers.as_ref() {
            if let Ok(mut st) = t.state.lock() {
                st.mark_world();
            }
        }
    }

    /// Timestamp writes for one render pass, or `None` when the adapter has no
    /// timestamp queries. Call sites read:
    ///     timestamp_writes: self.pass_timer("gpu.celestial"),
    pub(crate) fn pass_timer(
        &self,
        id: &'static str,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.gpu_timers.as_ref().and_then(|t| t.writes(id))
    }

    /// The compute-pass twin of `pass_timer`, for a `ComputePassDescriptor`.
    pub(crate) fn compute_pass_timer(
        &self,
        id: &'static str,
    ) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        self.gpu_timers.as_ref().and_then(|t| t.compute_writes(id))
    }

    /// A shared handle to the frame's timers, for a pass that is submitted
    /// OUTSIDE the renderer module and has no `&Renderer` in hand when it
    /// builds its descriptor: the in-world screens' egui-to-texture pass
    /// (`gui::screen_surface`) is created with a device and a queue only.
    /// `None` on an adapter without timestamp queries, same as `pass_timer`.
    pub(crate) fn gpu_timers(&self) -> Option<Arc<GpuTimers>> {
        self.gpu_timers.clone()
    }

    /// Frame boundary. Called from the two surface-acquire entry points, which
    /// every frame passes through exactly once: closes the CPU accumulation,
    /// resolves last frame's GPU timestamps, and (only while the Performance
    /// page is open, at most once a second) walks the memory inventory.
    ///
    /// `world` is false for the UI-only path (a page is open, nothing but a
    /// surface clear and the egui pass happens). Those frames are discarded
    /// rather than published on BOTH sides: the CPU accumulation is thrown
    /// away here (`discard_frame`), and the GPU timers still allocate and
    /// resolve their slots but drop the samples when they come back
    /// (`GpuTimers::begin_frame` with `world = false`, `publish_gpu_harvest`).
    /// That is what makes opening the Performance page NOT decay the numbers
    /// you opened it to read: until 2026-09-18 only the CPU half was guarded,
    /// and the GPU column EMA'd every world pass toward zero while the page
    /// (a UI-only page) was open.
    pub(crate) fn frame_costs_begin(&self, world: bool) {
        // A UI-only frame that rendered a world VIEW (the Maps page's world
        // layer, `mark_view_frame`) is a world frame for its costs.
        if publish_cpu_frame(world) {
            begin_frame();
        } else {
            discard_frame();
        }
        if let Some(t) = self.gpu_timers.as_ref() {
            t.begin_frame(&self.device, &self.queue, world);
        }
        // Dev rig hook: `HUMANITY_FRAME_COSTS=1` makes a headless probe run
        // behave as if the Performance page were open and drop the numbers to
        // `debug/frame_costs.json` once a second - the same machine-readable
        // JSON-drop convention as `debug/boot_timing.json`, so a scripted or AI
        // session can READ a real breakdown instead of eyeballing a screenshot.
        if dump_enabled() {
            watch();
        }
        if !watched() {
            return;
        }
        let due = match self.inventory_sampled.lock() {
            Ok(mut last) => {
                let due = last.map_or(true, |t: Instant| t.elapsed().as_secs_f32() > 1.0);
                if due {
                    *last = Some(Instant::now());
                }
                due
            }
            Err(_) => false,
        };
        if due {
            self.sample_memory_inventory();
            if dump_enabled() {
                dump_json();
            }
        }
    }

    /// Tracked GPU allocations, by bucket, plus the driver's own total so the
    /// untracked remainder is visible. Runs at most once a second and only
    /// while somebody is looking at the numbers.
    fn sample_memory_inventory(&self) {
        // Terrain patch arena: the mega-buffers are allocated at full size, so
        // report both what is RESERVED and what is actually in use.
        let (arena_used, arena_reserved) = match self.patch_arena.as_ref() {
            Some(a) => {
                let vstride = std::mem::size_of::<crate::vertex::Vertex>() as u64;
                let vused = (a.valloc.cap() - a.valloc.free_total()) as u64 * vstride;
                let iused = (a.ialloc.cap() - a.ialloc.free_total()) as u64 * 4;
                let reserved = a.vertex_buf.size()
                    + a.index_buf.size()
                    + a.instance_buf.size()
                    + a.indirect_buf.size()
                    + a.batch_buf.size();
                (vused + iused, reserved)
            }
            None => (0, 0),
        };
        set_vram("vram.patch_arena", arena_used);
        set_vram("vram.patch_arena_reserved", arena_reserved);

        // Classic meshes (homestead geometry, trees, props, unbatched patches).
        let mesh_bytes: u64 = self
            .meshes
            .iter()
            .map(|m| m.vertex_buffer.size() + m.index_buffer.size())
            .sum();
        set_vram("vram.meshes", mesh_bytes);

        // Engine-global textures and render targets.
        let targets = self.scene_target_bytes()
            + texture_bytes(&self.depth_texture)
            + texture_bytes(&self.tree_atlas_texture)
            + texture_bytes(&self.water_fft_texture)
            + texture_bytes(&self.weather_map_tex)
            + texture_bytes(&self.atmo_trans_tex)
            + texture_bytes(&self.atmo_ms_tex)
            // Room GI's probe atlas, its scratch and its buffers (2026-09-27).
            + self.room_gi.vram_bytes();
        set_vram("vram.render_targets", targets);

        // Particles: the two billboard vertex pools plus the GPU-simulated
        // pool's draw buffer (its private state buffer lands in the untracked
        // remainder, which is exactly what that slice is for).
        let particles = self.particle_vb_alpha.as_ref().map_or(0, |b| b.size())
            + self.particle_vb_additive.as_ref().map_or(0, |b| b.size())
            + self.gpu_particles.as_ref().map_or(0, |p| p.vertex_buf.size());
        set_vram("vram.particles", particles);

        // Grass strand instances (near-field vegetation).
        let grass = self.grass_instance_buf.as_ref().map_or(0, |b| b.size())
            + self.grass_mesh.as_ref().map_or(0, |m| {
                m.vertex_buffer.size() + m.index_buffer.size()
            });
        set_vram("vram.grass", grass);

        // Per-frame uniform and light buffers.
        let uniforms = self.camera_buffer.size()
            + self.object_buffer.size()
            + self.lights_buffer.size()
            + self.tile_counts_buffer.size()
            + self.tile_indices_buffer.size()
            + self.light_camera_buffer.size()
            + self.shadow_uniform_buffer.size()
            + self.particle_frame_buffer.size()
            + self.dummy_instance_buf.size();
        set_vram("vram.uniforms", uniforms);

        // What the driver-side allocator says it holds overall. Not every
        // backend reports one; when it does, the Performance page shows the
        // difference from our tracked buckets as "untracked / driver".
        if let Some(rep) = self.device.generate_allocator_report() {
            set_vram("vram.driver_reserved", rep.total_reserved_bytes);
            set_vram("vram.driver_allocated", rep.total_allocated_bytes);
        }

        sample_process_memory();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store is process-global (`crate::frame_costs`), so the tests that publish into it
    /// take its one lock, as the store's own tests do.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        crate::frame_costs::test_lock()
    }

    /// The ring remembers whose frame each batch was, all the way to the
    /// harvest two boundaries later: a batch written by a world frame is
    /// tagged world when it goes in flight and when it is taken back out; a
    /// UI-only frame's batch is tagged UI-only. Device-free: this is the
    /// bookkeeping `GpuTimers::begin_frame` runs, minus the wgpu calls.
    #[test]
    fn timer_ring_tags_each_batch_with_its_frame_kind() {
        let mut st = TimerState::default();
        // Boot: a world frame starts.
        let boot = st.close_frame(true);
        assert!(boot.resolve.is_none(), "nothing to resolve before any pass ran");
        // The world frame times two passes.
        assert_eq!(st.take_pair("gpu.scene"), Some(0));
        assert_eq!(st.take_pair("gpu.ui"), Some(2));
        // Boundary: the world frame ends, a UI-only frame (a page opened) starts.
        let end = st.close_frame(false);
        assert!(end.world, "the batch that just ended was a world frame");
        let (batch, used) = end.resolve.expect("a free ring resolves the batch");
        assert_eq!(used, 4);
        assert_eq!(batch, vec![("gpu.scene", 0), ("gpu.ui", 2)]);
        st.mark_in_flight(batch, end.world);
        // The UI-only frame times its clear; slots start from 0 again (no
        // leak, no alias onto the world frame's pair).
        assert_eq!(st.take_pair("gpu.clear"), Some(0));
        // The readback completes: the harvested batch is the world frame's.
        let (harvested, harvested_world) = st.take_in_flight();
        assert!(harvested_world, "the harvest must publish this batch");
        assert_eq!(harvested.len(), 2);
        // Boundary: the UI-only frame ends, another UI-only frame starts.
        let end = st.close_frame(false);
        assert!(!end.world, "the batch that just ended was UI-only");
        let (batch, _) = end.resolve.expect("the ring is free again");
        st.mark_in_flight(batch, end.world);
        let (_, harvested_world) = st.take_in_flight();
        assert!(!harvested_world, "a UI-only batch must be harvested as UI-only, so it is discarded");
    }

    /// A UI-only frame that rendered a world VIEW (the Maps page's world
    /// layer) publishes like a world frame: its GPU batch is tagged world
    /// when it closes, and its CPU stages fold in at the boundary rather
    /// than being discarded. One mark covers one frame only.
    ///
    /// Seen red 2026-10-21 with `mark_world` a no-op: "the map's render was
    /// thrown away with the page frame"; and with `publish_cpu_frame` reading
    /// only `world` (the CPU half, after the critic review found it untested):
    /// "the map's CPU stages were thrown away with the page frame".
    #[test]
    fn a_page_frame_that_drew_a_world_view_counts() {
        let mut st = TimerState::default();
        st.close_frame(false); // a page is open: UI-only frames
        assert_eq!(st.take_pair("gpu.celestial"), Some(0));
        st.mark_world(); // the Maps world layer rendered in this frame
        let end = st.close_frame(false);
        assert!(end.world, "the map's render was thrown away with the page frame");
        let (batch, _) = end.resolve.expect("a free ring resolves it");
        st.mark_in_flight(batch, end.world);
        let _ = st.take_in_flight();
        assert!(!st.close_frame(false).world, "the next page frame drew nothing and stays UI-only");
        // The CPU half, through the decision frame_costs_begin makes.
        assert!(!publish_cpu_frame(false), "a page frame that drew nothing is discarded");
        note_view_frame();
        assert!(publish_cpu_frame(false), "the map's CPU stages were thrown away with the page frame");
        assert!(!publish_cpu_frame(false), "and only that one frame");
        assert!(publish_cpu_frame(true), "a world frame is always published");
    }

    /// A pass that asks for a slot pair after the ring is full runs untimed.
    /// That used to be silent (the pass looked free on the pie); now each
    /// refusal is counted per frame, handed back at the boundary, and the
    /// count reaches the snapshot the Performance page reads.
    #[test]
    fn refused_slot_requests_are_counted_per_frame() {
        let _g = serial();
        let mut st = TimerState::default();
        st.close_frame(true);
        for _ in 0..MAX_TIMED_PASSES {
            assert!(st.take_pair("gpu.filler").is_some());
        }
        assert!(st.take_pair("gpu.late_1").is_none(), "the ring must be full");
        assert!(st.take_pair("gpu.late_2").is_none());
        assert!(st.take_pair("gpu.late_3").is_none());
        let end = st.close_frame(true);
        assert_eq!(end.untimed, 3, "three refusals in that frame");
        // The counter is per frame: the new frame starts clean.
        assert!(st.take_pair("gpu.next_frame").is_some());
        assert_eq!(st.close_frame(true).untimed, 0);

        // And the count is visible where the page looks.
        reset_for_test();
        assert_eq!(snapshot().gpu_untimed_passes, 0);
        set_gpu_untimed(end.untimed);
        assert_eq!(snapshot().gpu_untimed_passes, 3);
        reset_for_test();
    }

    /// REAL GPU verification of the timestamp path, headless (no window, no
    /// world): create a device with TIMESTAMP_QUERY, run four world frames of
    /// a trivial render pass through `writes()` AND the particle sim's
    /// compute pass through `compute_writes()` (via `GpuParticles::simulate`,
    /// the live caller), prove a duration comes back for each, then run
    /// UI-only frames and prove the world numbers hold (the 2026-09-18 major:
    /// the harvest used to publish UI-only frames and decay them).
    ///
    /// This is the part `cargo check` cannot cover - query-set creation, the
    /// `timestamp_writes` descriptors (render AND compute), `resolve_query_set`,
    /// the readback map and the tick-to-millisecond maths are all runtime wgpu
    /// validation, which is exactly the failure class that shipped ten
    /// unbootable releases in v0.1029-v0.1038. Ignored by default because it
    /// needs a GPU adapter (a brief device use, not a booted instance):
    ///   cargo test --features native --lib gpu_timestamps -- --ignored --nocapture
    ///
    /// Native-only: `pollster` is a native-feature dependency, and a headless
    /// relay has no business asking for a GPU adapter. Every OTHER test in this
    /// module is pure bookkeeping and still runs in the relay build.
    #[cfg(feature = "native")]
    #[test]
    #[ignore = "needs a GPU adapter; run explicitly (see the doc comment)"]
    fn gpu_timestamps_resolve_a_real_pass_duration() {
        let _g = serial();
        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Some(a) => a,
            None => {
                println!("no GPU adapter; skipping");
                return;
            }
        };
        if !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            println!("adapter has no TIMESTAMP_QUERY; the CPU fallback is the live path here");
            return;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("frame_costs timestamp test"),
                required_features: wgpu::Features::TIMESTAMP_QUERY,
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ))
        .expect("device with timestamp queries");
        let timers = GpuTimers::new(&device, &queue).expect("timers on a capable adapter");

        // A BIG clear, on purpose: an empty 256x256 pass finishes inside one
        // timestamp tick on this hardware and reads back as 0 ms, which proves
        // nothing. 4096x4096 is ~67 MB of writes, comfortably measurable.
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("timestamp test target"),
            size: wgpu::Extent3d { width: 4096, height: 4096, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        // The compute half: the live particle sim, 100k slots, timed through
        // `compute_writes` exactly as `Renderer::simulate_gpu_particles` does.
        let mut particles = super::super::particles_gpu::GpuParticles::new(&device, 100_000);
        let sim = super::super::particles_gpu::SimParams {
            origin_radius: [0.0, 50.0, 0.0, 15.0],
            dir_spread: [0.0, -1.0, 0.0, 0.05],
            gravity_dt: [0.0, -2.0, 0.0, 1.0 / 60.0],
            speed_life: [9.0, 14.0, 1.4, 2.0],
            size_shape: [0.02, 0.02, 0.0, 0.0],
            color_start: [0.6, 0.7, 0.9, 0.4],
            color_end: [0.6, 0.7, 0.9, 0.2],
            count_frame_streak: [0.0, 0.0, 0.0, 0.0],
        };

        // One frame's worth of passes: the big clear (render) + the sim (compute).
        let world_frame = |timers: &GpuTimers, particles: &mut super::super::particles_gpu::GpuParticles| {
            let mut enc = device.create_command_encoder(&Default::default());
            {
                let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("timestamp test pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: timers.writes("gpu.test_pass"),
                    occlusion_query_set: None,
                });
            }
            queue.submit(std::iter::once(enc.finish()));
            particles.simulate(&device, &queue, sim, 100_000, Some(timers));
        };

        reset_for_test();
        for _ in 0..4 {
            timers.begin_frame(&device, &queue, true);
            world_frame(&timers, &mut particles);
            let _ = device.poll(wgpu::Maintain::Wait);
        }
        // One more boundary so the last frame's readback is harvested.
        timers.begin_frame(&device, &queue, true);
        let _ = device.poll(wgpu::Maintain::Wait);
        timers.begin_frame(&device, &queue, true);

        let snap = snapshot();
        let recorded = snap.gpu.iter().any(|e| e.id == "gpu.test_pass");
        let v = snap.value("gpu.test_pass");
        println!("measured 4096x4096 clear pass: {v:.4} ms");
        // The load-bearing assertion is that the WHOLE path ran: query set,
        // pass timestamp_writes, resolve, readback map, tick maths.
        assert!(
            recorded,
            "no GPU duration was ever recorded - the timestamp path did not \
             resolve (query set, timestamp_writes, resolve_query_set, or the \
             readback map)"
        );
        assert!(
            v > 0.0 && v < 100.0,
            "expected a plausible GPU pass duration, got {v} ms"
        );
        // The compute path: a 100k-thread dispatch is comfortably more than
        // one timestamp tick, so a real duration must come back for it too.
        let sim_recorded = snap.gpu.iter().any(|e| e.id == "gpu.particles_sim");
        let sim_ms = snap.value("gpu.particles_sim");
        println!("measured 100k particle sim compute pass: {sim_ms:.4} ms");
        assert!(
            sim_recorded,
            "the compute pass was never recorded - ComputePassTimestampWrites \
             through compute_writes did not resolve"
        );
        assert!(
            sim_ms > 0.0 && sim_ms < 100.0,
            "expected a plausible compute pass duration, got {sim_ms} ms"
        );
        assert_eq!(snap.gpu_untimed_passes, 0, "two passes cannot exhaust a 64-pass ring");

        // The major: UI-only frames (a page open: one small clear, no world
        // pass) must NOT decay the world numbers. Six boundaries is enough
        // for every UI-only batch to be harvested; before the fix each one
        // published, and 6 UI-only frames take a value to 38 percent.
        for _ in 0..6 {
            timers.begin_frame(&device, &queue, false);
            let mut enc = device.create_command_encoder(&Default::default());
            {
                let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ui-only clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: timers.writes("gpu.clear"),
                    occlusion_query_set: None,
                });
            }
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::Maintain::Wait);
        }
        let after = snapshot();
        assert_eq!(
            after.value("gpu.test_pass"),
            v,
            "UI-only frames decayed a world pass on a real device"
        );
        assert_eq!(after.value("gpu.particles_sim"), sim_ms);
        assert!(
            !after.gpu.iter().any(|e| e.id == "gpu.clear"),
            "a UI-only frame's own pass must not be published either"
        );
        reset_for_test();
    }
}
