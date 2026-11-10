//! THE OWN-MESH POOL (2026-11-10, the look's follow-up; docs/design/character-animation.md section
//! 16): each near person's own shaped and dressed mesh lives in ranges of a few GPU buffers allocated
//! ONCE, on the first frame that draws bodies, and handed from person to person as people come and go.
//!
//! WHY. A person's mesh went to the GPU in buffers made for it on the frame it arrived
//! (`skin_register_body`): most took about a millisecond, but about one in 38 took 9.9 to 11.5 ms, in
//! three of five crowd runs (the driver making room for a new allocation; the garment textures'
//! measured lesson: "A GPU allocation's cost isn't proportional to its size... Allocate at world
//! entry, never when content arrives"). Against an 8.3 ms frame at 120 frames a second that is a
//! visible stutter each time a crowd walks in, which the operator asked us to avoid (2026-10-30).
//!
//! WHY RANGES AND NOT FIXED SLOTS. Every garment texture of one size has one shape, so the texture
//! pool hands out fixed slots. Meshes do not: a person is the body (15,479 vertices) plus whatever
//! they wear, from about 21,000 vertices dressed simply to 40,389 in the largest outfit, shoes, hat
//! and the longest hair (two-sided cards: their vertices twice). Fixed slots sized for the largest
//! would hold about twice the memory the people near you need, so the pool is the terrain patch
//! arena's technique (`patch_arena`): one buffer each for the source vertices, their skinned copy and
//! the indices, and a first-fit range allocator (`RangeAlloc`) handing each person exactly their
//! size. It is planned for the near tier's worst case, as the texture pools are: its cap and the
//! player all wearing the largest of everything at once (`MeshPool::new`); typical people fill about
//! half of that, and the rest keeps the meshes of people who just left the near tier, so walking the
//! near line does not build them again.
//!
//! WHEN IT IS FULL. A newcomer takes the room of the person longest out of the near tier (their mesh
//! is built again if they come back); when only people near hold room and the free room is in pieces,
//! one person a frame is PACKED down into the gap below them (their mesh written again from its CPU
//! copy, the frame's one upload), so the free room becomes one run at the top and the newcomer fits:
//! the near tier's worst case always fits within a few frames. Meanwhile the newcomer is drawn with
//! the shared body, never a stall and never an allocation on the frame. A mesh bigger than the
//! largest dressed person the pool was planned for (a garment added since, worn with others in a way
//! the plan did not count) is refused (`Refusal::TooBig`), says so in the log and is drawn with the
//! shared body until the pool is planned again (the library read again).
//!
//! This is a reuse of memory, not of data: a range holds exactly the mesh last written into it,
//! written whole when the person takes it, so nothing in it can be stale (CLAUDE.md's no-caches rule
//! is about data kept across runs or past its source; this is neither), and with the reuse switch off
//! every mesh is built and written again every frame, and the pool is planned again whenever the
//! library is read again.
//!
//! `MeshPool` is the bookkeeping (who holds which ranges), plain data tested without a GPU; the
//! renderer holds the buffers (`renderer::skinning`, `skin_create_own_pool`, `skin_own_place`).

use super::patch_arena::RangeAlloc;

/// GPU bytes a pooled vertex takes: its source (`SkinnedVertex`, 48 bytes) and its skinned copy (the
/// ordinary `Vertex`, 32).
pub const VERTEX_BYTES: u64 = 48 + 32;
/// GPU bytes a pooled index takes.
pub const INDEX_BYTES: u64 = 4;

/// The largest own mesh the pool is planned for: the largest dressed person (the library's largest
/// body and the largest garment of every kind, `CharacterLibrary::largest_own_mesh`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshClass {
    pub vertices: u32,
    pub indices: u32,
}

impl MeshClass {
    /// Does a mesh of this many vertices and indices fit the plan?
    pub fn fits(&self, vertices: u32, indices: u32) -> bool {
        vertices <= self.vertices && indices <= self.indices
    }

    /// The GPU bytes of one person this size.
    pub fn bytes(&self) -> u64 {
        self.vertices as u64 * VERTEX_BYTES + self.indices as u64 * INDEX_BYTES
    }
}

/// Where a person's mesh lives in the pool: its vertices (and their skinned copy) and its indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    pub vstart: u32,
    pub vlen: u32,
    pub istart: u32,
    pub ilen: u32,
}

impl Held {
    pub fn bytes(&self) -> u64 {
        self.vlen as u64 * VERTEX_BYTES + self.ilen as u64 * INDEX_BYTES
    }
}

/// Why a mesh was not put in the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Bigger than the largest dressed person the pool was planned for.
    TooBig,
    /// Every handle (a person's place in the pool's books) is taken.
    NoHandle,
    /// No free run is long enough for its vertices, its indices, or both.
    NoRoom { vertices: bool, indices: bool },
}

/// The pool's two kinds of range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Vertices,
    Indices,
}

/// What to do next to put a finished mesh in the pool (`MeshPool::next_step`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// It fits: place it with this handle.
    Place(usize),
    /// Let the mesh of the person holding this handle go first (they are out of the near tier, the
    /// longest of those kept), then ask again.
    Evict(usize),
    /// Pack this side down by one person (this frame's upload), and try again next frame.
    Pack(Side),
    /// Nothing can be done this frame: draw them with the shared body and try again next frame.
    Wait,
    /// Bigger than the pool was planned for: drawn with the shared body (logged).
    TooBig,
}

/// Who holds which ranges of one pool. No GPU here: the renderer's buffers are sized from it.
#[derive(Debug, Clone)]
pub struct MeshPool {
    pub class: MeshClass,
    /// People the pool holds at the largest size at once (the near tier: its cap and the player).
    pub people: u32,
    verts: RangeAlloc,
    inds: RangeAlloc,
    /// Each handle's ranges (None: free). A handle is a person's place in the books, and the
    /// renderer's body id for them; there are as many as the engine may keep meshes for.
    held: Vec<Option<Held>>,
    /// How many times the pool asked for its buffers to be allocated: once for its whole life.
    pub allocations: u32,
    /// Its buffers are allocated (`needs_buffers` was answered).
    allocated: bool,
}

impl MeshPool {
    /// A pool for `people` of the largest dressed person `class` at once, with `handles` places in
    /// its books (the most meshes the engine keeps: the near tier and those just out of it).
    pub fn new(class: MeshClass, people: u32, handles: u32) -> Self {
        let people = people.max(1);
        MeshPool {
            class,
            people,
            verts: RangeAlloc::new(class.vertices.max(1).saturating_mul(people)),
            inds: RangeAlloc::new(class.indices.max(1).saturating_mul(people)),
            held: vec![None; handles.max(people) as usize],
            allocations: 0,
            allocated: false,
        }
    }

    pub fn capacity_vertices(&self) -> u32 {
        self.verts.cap()
    }

    pub fn capacity_indices(&self) -> u32 {
        self.inds.cap()
    }

    pub fn handles(&self) -> usize {
        self.held.len()
    }

    /// The GPU bytes the pool's buffers take.
    pub fn bytes(&self) -> u64 {
        self.verts.cap() as u64 * VERTEX_BYTES + self.inds.cap() as u64 * INDEX_BYTES
    }

    /// The GPU bytes the people in it take now.
    pub fn used_bytes(&self) -> u64 {
        self.held.iter().flatten().map(Held::bytes).sum()
    }

    /// How many people hold a mesh in it.
    pub fn holders(&self) -> usize {
        self.held.iter().filter(|h| h.is_some()).count()
    }

    /// The ranges handle `h` holds.
    pub fn held(&self, h: usize) -> Option<Held> {
        self.held.get(h).copied().flatten()
    }

    /// A handle nobody holds.
    pub fn free_handle(&self) -> Option<usize> {
        self.held.iter().position(Option::is_none)
    }

    /// True until the buffers are allocated; the call that allocates them counts it.
    pub fn needs_buffers(&mut self) -> bool {
        if self.allocated {
            return false;
        }
        self.allocated = true;
        self.allocations += 1;
        true
    }

    /// Put a mesh of `v` vertices and `i` indices in the pool for handle `h` (a free handle, or one
    /// holding the person's last mesh, which it replaces). Its new ranges are taken while the old
    /// are still held when there is room (the last mesh stays whole until the new one is written),
    /// else from the room the old ones leave. On a refusal nothing changes.
    pub fn place(&mut self, h: usize, v: u32, i: u32) -> Result<Held, Refusal> {
        if !self.class.fits(v, i) {
            return Err(Refusal::TooBig);
        }
        let Some(&old) = self.held.get(h) else { return Err(Refusal::NoHandle) };
        let (v, i) = (v.max(1), i.max(1));
        let got = match Self::take(&mut self.verts, &mut self.inds, v, i) {
            Ok(got) => {
                if let Some(o) = old {
                    self.verts.free(o.vstart, o.vlen);
                    self.inds.free(o.istart, o.ilen);
                }
                got
            }
            Err(refusal) => {
                let Some(o) = old else { return Err(refusal) };
                self.verts.free(o.vstart, o.vlen);
                self.inds.free(o.istart, o.ilen);
                match Self::take(&mut self.verts, &mut self.inds, v, i) {
                    Ok(got) => got,
                    Err(refusal) => {
                        // Put the old ranges back exactly: they were free a moment ago.
                        let back = self.verts.alloc_at(o.vstart, o.vlen) && self.inds.alloc_at(o.istart, o.ilen);
                        debug_assert!(back, "the ranges just freed are taken back");
                        return Err(refusal);
                    }
                }
            }
        };
        let held = Held { vstart: got.0, vlen: v, istart: got.1, ilen: i };
        self.held[h] = Some(held);
        Ok(held)
    }

    /// Both ranges, or neither.
    fn take(verts: &mut RangeAlloc, inds: &mut RangeAlloc, v: u32, i: u32) -> Result<(u32, u32), Refusal> {
        let vs = verts.alloc(v);
        let is = inds.alloc(i);
        match (vs, is) {
            (Some(a), Some(b)) => Ok((a, b)),
            (a, b) => {
                if let Some(a) = a {
                    verts.free(a, v);
                }
                if let Some(b) = b {
                    inds.free(b, i);
                }
                Err(Refusal::NoRoom { vertices: a.is_none(), indices: b.is_none() })
            }
        }
    }

    /// Give handle `h`'s ranges back (the person left, or their mesh was let go). Returns them.
    pub fn release(&mut self, h: usize) -> Option<Held> {
        let held = self.held.get_mut(h)?.take()?;
        self.verts.free(held.vstart, held.vlen);
        self.inds.free(held.istart, held.ilen);
        Some(held)
    }

    /// The person packing `side` would move next and where to: the lowest holder with a gap below
    /// them, to the gap's start. None when that side is one run of holders from its start.
    pub fn pack_candidate(&self, side: Side) -> Option<(usize, u32)> {
        let mut order: Vec<(u32, u32, usize)> = self
            .held
            .iter()
            .enumerate()
            .filter_map(|(h, x)| x.map(|x| match side {
                Side::Vertices => (x.vstart, x.vlen, h),
                Side::Indices => (x.istart, x.ilen, h),
            }))
            .collect();
        order.sort_unstable();
        let mut end = 0u32;
        for (start, len, h) in order {
            if start > end {
                return Some((h, end));
            }
            end = start + len;
        }
        None
    }

    /// Move the person `pack_candidate` names down into the gap below them (their new range may
    /// overlap their old one: the caller writes their mesh again from its CPU copy). Returns the
    /// handle, its new ranges and its old ones.
    pub fn pack_step(&mut self, side: Side) -> Option<(usize, Held, Held)> {
        let (h, to) = self.pack_candidate(side)?;
        let old = self.held(h)?;
        let mut new = old;
        let moved = match side {
            Side::Vertices => {
                self.verts.free(old.vstart, old.vlen);
                new.vstart = to;
                self.verts.alloc_at(to, old.vlen)
            }
            Side::Indices => {
                self.inds.free(old.istart, old.ilen);
                new.istart = to;
                self.inds.alloc_at(to, old.ilen)
            }
        };
        debug_assert!(moved, "the gap below a holder and its own range are one free run");
        self.held[h] = Some(new);
        Some((h, new, old))
    }

    /// What to do next to put a finished mesh of `v` vertices and `i` indices in the pool for a
    /// person holding handle `have` (None: they hold none yet). `kept`: (seconds out of the near
    /// tier, handle) of the people whose meshes are kept though they are not near; the longest out
    /// gives way first. Asks without changing anything.
    pub fn next_step(&self, have: Option<usize>, v: u32, i: u32, kept: &[(f32, usize)]) -> Step {
        if !self.class.fits(v, i) {
            return Step::TooBig;
        }
        // The kept meshes that may give way, longest out first.
        let mut order: Vec<(f32, usize)> = kept.iter().copied().filter(|&(_, h)| Some(h) != have && self.held(h).is_some()).collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        let longest = order.first().map(|&(_, h)| h);
        let Some(h) = have.or_else(|| self.free_handle()) else {
            return longest.map_or(Step::Wait, Step::Evict);
        };
        let mut trial = self.clone();
        match trial.place(h, v, i) {
            Ok(_) => Step::Place(h),
            Err(Refusal::TooBig) => Step::TooBig,
            Err(Refusal::NoHandle) => longest.map_or(Step::Wait, Step::Evict),
            Err(Refusal::NoRoom { vertices, indices }) => {
                // The longest out whose room alone lets the mesh fit (one mesh let go, not several in
                // turn while the free room stays in pieces), else the longest out.
                for &(_, k) in &order {
                    let mut t = self.clone();
                    t.release(k);
                    if t.place(h, v, i).is_ok() {
                        return Step::Evict(k);
                    }
                }
                if let Some(k) = longest {
                    return Step::Evict(k);
                }
                // Packing helps a side only when all its free room, the person's own old range
                // with it, would hold them as one run.
                let own = have.and_then(|h| self.held(h));
                let fits_packed = |side: Side| match side {
                    Side::Vertices => self.verts.free_total() as u64 + own.map_or(0, |o| o.vlen as u64) >= v.max(1) as u64,
                    Side::Indices => self.inds.free_total() as u64 + own.map_or(0, |o| o.ilen as u64) >= i.max(1) as u64,
                };
                for (short, side) in [(vertices, Side::Vertices), (indices, Side::Indices)] {
                    if short && fits_packed(side) && self.pack_candidate(side).is_some() {
                        return Step::Pack(side);
                    }
                }
                Step::Wait
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The largest dressed person the shipped library plans for (`largest_own_mesh`, measured
    /// 2026-11-10) and a typical one (the look's measured person: a casual suit, shoes, short hair,
    /// brows and lashes).
    const LARGEST: MeshClass = MeshClass { vertices: 40_389, indices: 203_916 };
    const TYPICAL: (u32, u32) = (21_184, 96_066);

    /// THE POOL HANDS ITS MEMORY FROM PERSON TO PERSON AND ITS BUFFERS ARE ALLOCATED ONCE: 41 people
    /// (the crowd's count of arrivals in the look's sweep) come near and leave through a pool planned
    /// for 8 at the largest size, each a typical size, each giving their room back when they go; the
    /// buffers are allocated once, and a person arriving after one left takes the room they freed.
    /// Seen red 2026-11-10 with `needs_buffers` answering true for every person (a fresh allocation
    /// per person, as main's `skin_register_body` did): "41 people made 41 allocations".
    #[test]
    fn the_pool_hands_room_on_and_allocates_once() {
        let mut pool = MeshPool::new(LARGEST, 8, 24);
        let mut near: Vec<usize> = Vec::new();
        for person in 0..41usize {
            if pool.needs_buffers() {
                // The renderer allocates the buffers here, once.
            }
            let freed = if near.len() == 12 {
                let gone = near.remove(0);
                Some(pool.release(gone).expect("a person near holds room"))
            } else {
                None
            };
            let h = pool.free_handle().expect("a free handle");
            let held = match pool.next_step(None, TYPICAL.0, TYPICAL.1, &[]) {
                Step::Place(at) => {
                    assert_eq!(at, h);
                    pool.place(h, TYPICAL.0, TYPICAL.1).expect("it fits")
                }
                other => panic!("person {person}: {other:?} with {} near", near.len()),
            };
            if let Some(f) = freed {
                assert_eq!((held.vstart, held.istart), (f.vstart, f.istart), "person {person} takes the room the one who left freed");
            }
            near.push(h);
        }
        assert_eq!(pool.allocations, 1, "41 people made {} allocations", pool.allocations);
        assert_eq!(pool.holders(), 12, "twelve typical people fit a pool planned for eight of the largest");
        assert!(pool.used_bytes() <= pool.bytes());
    }

    /// A MESH BIGGER THAN THE PLAN IS REFUSED AND TAKES NOTHING: one vertex or one index over the
    /// largest dressed person is `TooBig` (the engine draws that person with the shared body and says
    /// so in the log), and the pool's books are as they were. The largest itself fits.
    /// Seen red 2026-11-10 with `place` and `next_step` comparing against the pool's whole capacity
    /// instead of the plan (`next_step` answered `Place(0)` for a mesh one vertex over the largest
    /// dressed person, which would have pushed a near person out of the plan's worst case).
    #[test]
    fn an_oversized_mesh_is_refused_and_takes_nothing() {
        let mut pool = MeshPool::new(LARGEST, 4, 8);
        let before = (pool.free_handle(), pool.used_bytes(), pool.holders());
        assert_eq!(pool.next_step(None, LARGEST.vertices + 1, 1000, &[]), Step::TooBig);
        assert_eq!(pool.next_step(None, 1000, LARGEST.indices + 1, &[]), Step::TooBig);
        assert_eq!(pool.place(0, LARGEST.vertices + 1, 1000), Err(Refusal::TooBig));
        assert_eq!((pool.free_handle(), pool.used_bytes(), pool.holders()), before, "a refusal changes nothing");
        assert_eq!(pool.next_step(None, LARGEST.vertices, LARGEST.indices, &[]), Step::Place(0));
    }

    /// THE NEAR TIER'S WORST CASE ALWAYS FITS: a pool planned for four of the largest, filled with a
    /// mix and some let go, leaves its free room in pieces none of which holds the largest; with
    /// people kept from the near tier, one gives way: the one whose room alone makes the newcomer fit,
    /// else the longest out; with nobody kept, the people near are packed down one a frame until the
    /// free room is one run, and the largest fits.
    /// Seen red 2026-11-10 with `next_step` never answering `Pack` (the newcomer waited on the shared
    /// body for as long as the people near stood still): "after 0 frames: Wait". The one-that-fits
    /// choice came from the after runs (2026-11-10: four people of the crowd built twice, kept meshes
    /// let go in turn while the room stayed in pieces); seen red with the longest-out choice:
    /// `Evict(hs[5])` where letting hs[1] go alone made room.
    #[test]
    fn a_full_pool_evicts_the_longest_kept_then_packs_until_the_largest_fits() {
        let mut pool = MeshPool::new(LARGEST, 4, 8);
        let half = (LARGEST.vertices / 2, LARGEST.indices / 2);
        let mut hs = Vec::new();
        for _ in 0..7 {
            let h = pool.free_handle().unwrap();
            pool.place(h, half.0, half.1).unwrap();
            hs.push(h);
        }
        // Three let go, every other one: three half-size gaps and the half-size tail.
        for &h in &[hs[0], hs[2], hs[4]] {
            pool.release(h);
        }
        // Letting hs[1] go joins its room to the gaps either side (room for the largest); letting
        // hs[5] go joins it to one gap (a vertex short), so hs[1] gives way though out for less time.
        let kept = [(3.0, hs[1]), (9.0, hs[5])];
        assert_eq!(pool.next_step(None, LARGEST.vertices, LARGEST.indices, &kept), Step::Evict(hs[1]), "the one whose room alone makes it fit gives way");
        assert_eq!(pool.next_step(None, LARGEST.vertices, LARGEST.indices, &kept[1..]), Step::Evict(hs[5]), "when none does alone, the longest out gives way first");
        // Nobody kept: the people near are packed until the largest fits.
        let mut frames = 0;
        loop {
            match pool.next_step(None, LARGEST.vertices, LARGEST.indices, &[]) {
                Step::Place(h) => {
                    pool.place(h, LARGEST.vertices, LARGEST.indices).expect("it fits once packed");
                    break;
                }
                Step::Pack(side) => {
                    let (_, new, old) = pool.pack_step(side).expect("a holder moves");
                    match side {
                        Side::Vertices => assert!(new.vstart < old.vstart, "packing moves a person down"),
                        Side::Indices => assert!(new.istart < old.istart, "packing moves a person down"),
                    }
                }
                other => panic!("after {frames} frames: {other:?}"),
            }
            frames += 1;
            assert!(frames <= 16, "packing converges");
        }
        assert!(frames >= 2, "the free room really was in pieces ({frames} packing frames)");
        // Every holder's ranges are inside the pool and none overlap.
        let mut spans: Vec<(u32, u32)> = (0..pool.handles()).filter_map(|h| pool.held(h)).map(|x| (x.vstart, x.vstart + x.vlen)).collect();
        spans.sort();
        assert!(spans.windows(2).all(|w| w[0].1 <= w[1].0), "no two people's vertices overlap: {spans:?}");
        assert!(spans.last().unwrap().1 <= pool.capacity_vertices());
    }

    /// A REBUILD REPLACES IN PLACE OR NEARBY, AND A REFUSED ONE KEEPS THE LAST MESH: a person
    /// rebuilt at a new size takes new ranges and gives the old back; when even the old room plus the
    /// free room cannot hold the new mesh, nothing changes and their last mesh stays where it was.
    #[test]
    fn a_rebuild_replaces_its_ranges_or_keeps_the_last() {
        // Planned for two of 100 vertices: 200 vertices and 600 indices in all.
        let mut pool = MeshPool::new(MeshClass { vertices: 100, indices: 300 }, 2, 4);
        let one = pool.place(1, 100, 300).unwrap();
        assert_eq!(pool.place(0, 60, 200).map(|h| h.vstart), Ok(100));
        // Rebuilt at 80: the free 40 above it cannot hold that, its own 60 and the 40 together can.
        let new = pool.place(0, 80, 250).unwrap();
        assert_eq!((new.vstart, new.vlen), (100, 80), "the old 60 and the free 40 beside it hold 80");
        assert_eq!(pool.used_bytes(), one.bytes() + new.bytes(), "the old ranges were given back");
        // A third person fills the last 20; rebuilt at 100, person 0 has only their own 80 and no
        // free room: refused, and their last mesh stays where it was.
        pool.place(2, 20, 50).unwrap();
        assert_eq!(pool.place(0, 100, 250), Err(Refusal::NoRoom { vertices: true, indices: false }));
        assert_eq!(pool.held(0), Some(new), "a refused rebuild keeps the last mesh where it was");
        assert_eq!(pool.place(0, 101, 10), Err(Refusal::TooBig));
        assert_eq!(pool.held(0), Some(new));
    }
}
