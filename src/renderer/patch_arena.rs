//! Terrain patch mega-buffer arena (draw-batching increment 1, v0.1001).
//!
//! WHY: at high patch budgets the celestial pass was submitting ~12k terrain
//! patches as 12k individual draws, each with its own bind-group set (dynamic
//! offset), vertex-buffer bind, and index-buffer bind -- ~50k render-pass
//! commands per frame, plus two 3 MB object-uniform uploads and ~25k CPU
//! matrix inversions staging them. The operator's 12,288-budget config ran at
//! 13 FPS in an empty desert on that submission cost alone.
//!
//! THE FIX (increment 1): all batched patches live in ONE shared vertex
//! buffer + ONE shared index buffer (range-allocated), per-patch data
//! (anchor translation + LOD fade) rides an instance-rate VERTEX ATTRIBUTE
//! (slot 1), and the draw loop becomes: bind everything once, then one
//! `draw_indexed(irange, base_vertex, i..i+1)` per patch.
//!
//! Increment 2: measurement showed wgpu's ~1.5 us per draw_indexed encoding
//! still dominated (2,049 draws = 15.8 ms vs 8,718 draws = 25.8 ms, same
//! scene), so when the device has MULTI_DRAW_INDIRECT +
//! INDIRECT_FIRST_INSTANCE the whole batch collapses into ONE
//! multi_draw_indexed_indirect over `indirect_buf`. The per-instance data
//! deliberately rides an ATTRIBUTE and not the instance_index builtin:
//! this project's primary DX12 adapter is missing the
//! VERTEX_AND_INSTANCE_INDEX_RESPECTS_RESPECTIVE_FIRST_VALUE_IN_INDIRECT_DRAW
//! downlevel flag (boot log, 2026-07-27), so the BUILTIN would not see
//! first_instance from indirect args there -- while instance-attribute
//! fetch honors first_instance in hardware everywhere. Devices without the
//! features keep the per-draw loop (same shaders, same buffers).
//!
//! Patch geometry VARIES in size (the base grid is a fixed 1056 vertices, but
//! vegetation cards are appended into the same mesh), so slots are variable
//! ranges from a first-fit free-list allocator, not fixed-size bins. When the
//! arena is full, the caller falls back to the classic per-mesh path for that
//! patch (graceful degradation, logged -- never a crash).

use super::mesh::Vertex;

/// One allocated patch: element ranges in the shared arenas. `base_vertex`
/// for the draw is `vstart`; indices are stored patch-local (0-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PatchSlot {
    pub vstart: u32,
    pub vcount: u32,
    pub istart: u32,
    pub icount: u32,
}

/// First-fit range allocator over `0..cap` with free-list merging on free.
/// Units are ELEMENTS (vertices or indices), not bytes.
#[derive(Debug)]
pub struct RangeAlloc {
    cap: u32,
    /// Sorted, non-overlapping, non-adjacent free ranges (start, len).
    free: Vec<(u32, u32)>,
}

impl RangeAlloc {
    pub fn new(cap: u32) -> Self {
        Self { cap, free: vec![(0, cap)] }
    }

    pub fn cap(&self) -> u32 {
        self.cap
    }

    /// Total free elements (fragmentation may still fail a large alloc).
    pub fn free_total(&self) -> u32 {
        self.free.iter().map(|(_, l)| *l).sum()
    }

    /// Allocate `len` contiguous elements; None when no free range fits.
    pub fn alloc(&mut self, len: u32) -> Option<u32> {
        if len == 0 {
            return None;
        }
        let i = self.free.iter().position(|(_, l)| *l >= len)?;
        let (start, flen) = self.free[i];
        if flen == len {
            self.free.remove(i);
        } else {
            self.free[i] = (start + len, flen - len);
        }
        Some(start)
    }

    /// Return a range to the free list, merging with neighbors. Freeing a
    /// range that was never allocated is a caller bug; debug builds assert.
    pub fn free(&mut self, start: u32, len: u32) {
        if len == 0 {
            return;
        }
        debug_assert!(start + len <= self.cap, "free out of range");
        let pos = self.free.partition_point(|(s, _)| *s < start);
        debug_assert!(
            (pos == 0 || {
                let (ps, pl) = self.free[pos - 1];
                ps + pl <= start
            }) && (pos == self.free.len() || start + len <= self.free[pos].0),
            "double free / overlap in RangeAlloc"
        );
        self.free.insert(pos, (start, len));
        // Merge with next, then previous.
        if pos + 1 < self.free.len() {
            let (ns, nl) = self.free[pos + 1];
            if start + len == ns {
                self.free[pos].1 += nl;
                self.free.remove(pos + 1);
            }
        }
        if pos > 0 {
            let (ps, pl) = self.free[pos - 1];
            if ps + pl == start {
                self.free[pos - 1].1 += self.free[pos].1;
                self.free.remove(pos);
            }
        }
    }
}

/// One per-patch instance record as the shader sees it (the inst_pos_fade
/// vertex attribute, slot 1): xyz = anchor translation, w = LOD fade.
///
/// `_unused` covers shader locations 5 and 6, which slot 1 has carried since
/// v0.1091 (grass strands need a full per-instance transform). Terrain reads
/// neither, but the STRIDE is part of the shared vertex layout, so this
/// struct has to match `renderer::mesh::INSTANCE_STRIDE` exactly or every
/// patch after the first fetches its translation from the wrong offset.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PatchInstance {
    pub pos_fade: [f32; 4],
    pub _unused: [f32; 8],
}

impl PatchInstance {
    pub fn new(pos_fade: [f32; 4]) -> Self {
        Self { pos_fade, _unused: [0.0; 8] }
    }
}

/// One multi_draw_indexed_indirect entry (wgpu arg order: index_count,
/// instance_count, first_index, base_vertex, first_instance).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct IndirectArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

/// Max batched patch draws per frame (instance buffer capacity). Matches
/// the renderer's MAX_OBJECTS scale; patches beyond it draw classically.
pub const MAX_PATCH_DRAWS: usize = 16384;

/// The GPU-side arena: shared geometry buffers + the per-frame instance
/// storage + batch uniform the terrain-batch pipelines bind at group 1.
pub struct PatchArena {
    pub vertex_buf: wgpu::Buffer,
    pub index_buf: wgpu::Buffer,
    pub instance_buf: wgpu::Buffer,
    pub indirect_buf: wgpu::Buffer,
    pub batch_buf: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub valloc: RangeAlloc,
    pub ialloc: RangeAlloc,
    /// One-shot log flag: the first alloc failure after a success logs
    /// (no silent caps), then stays quiet until pressure clears.
    overflow_logged: bool,
}

impl PatchArena {
    /// Element capacities honoring the device's real buffer-size limit.
    ///
    /// SPLIT REBALANCED v0.1062. The old split was 1280 MB of vertices to
    /// 192 MB of indices, chosen to sit in the same ballpark as the patch
    /// cache's byte cap - but it never checked the ratio a patch actually
    /// CONSUMES. A PATCH_TESS = 16 triangular patch is 153 grid vertices and
    /// 768 grid indices, so demand runs about 5 indices per vertex, while the
    /// old arena provisioned about 1.2. Indices therefore exhausted while a
    /// fifth of the vertex arena sat stranded and unusable:
    ///
    ///   [PatchArena] index arena full (9,222,583 of 41,943,040 vertex elems
    ///   free, 937,440 of 50,331,648 index elems free)
    ///
    /// measured live - 22% of the vertex arena free, 1.9% of the index arena.
    /// Every patch past that point falls to the CLASSIC per-draw path, and
    /// [PatchBatch] shows the cost: 4,442 of 12,294 patches per frame paying a
    /// full set_bind_group + 2 buffer binds + draw_indexed each, on one thread.
    ///
    /// MEASURED from the live overflow logs rather than derived from the grid,
    /// because vertex sharing means a patch does NOT cost the naive 153:768.
    /// Two independent exhaustion snapshots give 1.51 and 1.26 indices per
    /// vertex, i.e. about 713 vertices and 1076 indices per patch. (A first
    /// attempt at this fix reasoned from the raw grid, assumed 5:1, and simply
    /// moved the exhaustion to the VERTEX pool - worth recording so nobody
    /// re-derives it from the tessellation again.)
    ///
    /// RE-SPLIT v0.1084 for INDEXED CARDS (the third rebalance, and the doc
    /// above explains why each one chased a moving geometry mix). Indexing
    /// cut card vertices 3x (12 -> 4 per card) with index count unchanged, so
    /// the aggregate demand ratio flipped from 1.2-1.3 i/v to 3.0-3.4 i/v and
    /// the INDEX pool became the binding one: the first deep-LOD session after
    /// indexing hit "index arena full, 560 of 71M free" with 22% of the vertex
    /// pool idle. 1050 MiB verts (34.4M elems) + 420 MiB indices (110.1M)
    /// provisions ~3.2 i/v, matching the measured post-indexing mix, roughly
    /// 2.2x the measured working set on BOTH pools at once. The patch CACHE
    /// ceiling (planet_chunks::PATCH_CACHE_MAX_BYTES) sits BELOW this total so
    /// graceful LRU eviction, not arena overflow, is the limiter.
    fn capacities(device: &wgpu::Device) -> (u32, u32) {
        let max_buf = device.limits().max_buffer_size;
        let vert_bytes = (1050u64 * 1024 * 1024).min(max_buf);
        let idx_bytes = (420u64 * 1024 * 1024).min(max_buf);
        let vcap = (vert_bytes / std::mem::size_of::<Vertex>() as u64) as u32;
        let icap = (idx_bytes / 4) as u32;
        (vcap, icap)
    }

    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let (vcap, icap) = Self::capacities(device);
        log::info!(
            "[PatchArena] created: {} MB vertices ({}M) + {} MB indices ({}M), instance cap {}",
            vcap as u64 * std::mem::size_of::<Vertex>() as u64 / (1024 * 1024),
            vcap / 1_000_000,
            icap as u64 * 4 / (1024 * 1024),
            icap / 1_000_000,
            MAX_PATCH_DRAWS,
        );
        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Patch Arena Vertices"),
            size: vcap as u64 * std::mem::size_of::<Vertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Patch Arena Indices"),
            size: icap as u64 * 4,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Instance data is a VERTEX buffer (slot 1, step_mode Instance) --
        // attribute fetch respects first_instance in both direct and
        // indirect draws on every backend, unlike the instance_index
        // builtin (broken for indirect on this DX12 adapter).
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Patch Instance Data"),
            size: (MAX_PATCH_DRAWS * std::mem::size_of::<PatchInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Indirect draw args (increment 2): 20 bytes per patch draw, one
        // multi_draw_indexed_indirect submits them all when the device
        // supports it (the per-draw loop is the fallback).
        let indirect_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Patch Indirect Args"),
            size: (MAX_PATCH_DRAWS * 20) as u64,
            usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let batch_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Patch Batch Uniforms"),
            size: 64, // one mat4x4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Patch Batch Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: batch_buf.as_entire_binding(),
            }],
        });
        Self {
            vertex_buf,
            index_buf,
            instance_buf,
            indirect_buf,
            batch_buf,
            bind_group,
            valloc: RangeAlloc::new(vcap),
            ialloc: RangeAlloc::new(icap),
            overflow_logged: false,
        }
    }

    /// Allocate arena ranges and upload the geometry. None = arena full
    /// (or geometry empty); the caller falls back to a classic mesh.
    pub fn upload(
        &mut self,
        queue: &wgpu::Queue,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Option<PatchSlot> {
        if vertices.is_empty() || indices.is_empty() {
            return None;
        }
        let vstart = match self.valloc.alloc(vertices.len() as u32) {
            Some(s) => s,
            None => {
                self.log_overflow("vertex");
                return None;
            }
        };
        let istart = match self.ialloc.alloc(indices.len() as u32) {
            Some(s) => s,
            None => {
                // Roll back the vertex claim so the arenas never skew.
                self.valloc.free(vstart, vertices.len() as u32);
                self.log_overflow("index");
                return None;
            }
        };
        self.overflow_logged = false;
        queue.write_buffer(
            &self.vertex_buf,
            vstart as u64 * std::mem::size_of::<Vertex>() as u64,
            bytemuck::cast_slice(vertices),
        );
        queue.write_buffer(&self.index_buf, istart as u64 * 4, bytemuck::cast_slice(indices));
        Some(PatchSlot {
            vstart,
            vcount: vertices.len() as u32,
            istart,
            icount: indices.len() as u32,
        })
    }

    pub fn release(&mut self, slot: PatchSlot) {
        self.valloc.free(slot.vstart, slot.vcount);
        self.ialloc.free(slot.istart, slot.icount);
    }

    fn log_overflow(&mut self, which: &str) {
        if !self.overflow_logged {
            self.overflow_logged = true;
            log::warn!(
                "[PatchArena] {} arena full ({} of {} vertex elems free, {} of {} index elems free) - overflow patches draw on the classic path until eviction frees ranges",
                which,
                self.valloc.free_total(),
                self.valloc.cap(),
                self.ialloc.free_total(),
                self.ialloc.cap(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_free_roundtrip_and_merge() {
        let mut a = RangeAlloc::new(100);
        let x = a.alloc(30).unwrap();
        let y = a.alloc(30).unwrap();
        let z = a.alloc(30).unwrap();
        assert_eq!((x, y, z), (0, 30, 60));
        assert_eq!(a.free_total(), 10);
        // Free the middle, then the first: they must merge into one 60-run.
        a.free(y, 30);
        a.free(x, 30);
        assert_eq!(a.alloc(60), Some(0));
        // Free everything; the whole range must be one run again.
        a.free(0, 60);
        a.free(z, 30);
        assert_eq!(a.free_total(), 100);
        assert_eq!(a.alloc(100), Some(0));
    }

    #[test]
    fn alloc_fails_when_fragmented_but_not_lost() {
        let mut a = RangeAlloc::new(100);
        let x = a.alloc(40).unwrap();
        let _y = a.alloc(40).unwrap();
        a.free(x, 40);
        // 60 free total, but the largest run is 40.
        assert_eq!(a.free_total(), 60);
        assert!(a.alloc(50).is_none());
        assert_eq!(a.alloc(40), Some(0));
    }

    #[test]
    fn zero_len_alloc_rejected() {
        let mut a = RangeAlloc::new(10);
        assert!(a.alloc(0).is_none());
        assert_eq!(a.free_total(), 10);
    }

    #[test]
    fn exhaustion_returns_none() {
        let mut a = RangeAlloc::new(10);
        assert_eq!(a.alloc(10), Some(0));
        assert!(a.alloc(1).is_none());
        a.free(0, 10);
        assert_eq!(a.alloc(1), Some(0));
    }

    /// Churn like real patch streaming: interleaved allocs/frees of varied
    /// sizes must never leak elements or corrupt the free list.
    #[test]
    fn churn_never_leaks() {
        let mut a = RangeAlloc::new(10_000);
        let mut live: Vec<(u32, u32)> = Vec::new();
        let mut seed = 0x51F0_A11Cu64;
        let mut rng = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for step in 0..2000 {
            if step % 3 != 2 || live.is_empty() {
                let len = (rng() % 97 + 1) as u32;
                if let Some(s) = a.alloc(len) {
                    // No overlap with any live range.
                    for &(ls, ll) in &live {
                        assert!(s + len <= ls || ls + ll <= s, "overlap at step {step}");
                    }
                    live.push((s, len));
                }
            } else {
                let i = (rng() as usize) % live.len();
                let (s, l) = live.swap_remove(i);
                a.free(s, l);
            }
            let live_total: u32 = live.iter().map(|(_, l)| *l).sum();
            assert_eq!(a.free_total() + live_total, 10_000, "leak at step {step}");
        }
        for (s, l) in live.drain(..) {
            a.free(s, l);
        }
        assert_eq!(a.alloc(10_000), Some(0));
    }
}
