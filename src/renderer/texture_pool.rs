//! THE GARMENT TEXTURE POOL (2026-11-10, the look's textures; docs/design/character-animation.md
//! section 16): garment textures live in SLOTS of a few same-size GPU textures, allocated once per
//! size when the look first needs them and handed from garment to garment as people come and go.
//!
//! WHY. Each garment's texture was its own GPU allocation, made on the frame it arrived. Most took 1
//! to 3 ms, but 2 of 41 took 9.8 and 14.8 ms (the driver making room for a new allocation), and
//! those were the crowd's worst frames (cpu.look_build 16.7 ms against an 8.3 ms frame at 120 frames
//! a second). Every garment texture of one size is the same shape (`SlotClass`), so a pool of them
//! allocated once removes the allocation from the frame: a garment arriving is one write into a
//! slot it takes. This is a reuse of memory, not of data: a slot holds exactly the garment it was
//! last written for, written in full when that garment takes it, so nothing in it can be stale
//! (CLAUDE.md's no-caches rule is about data kept across runs or past its source; this is neither).
//!
//! HOW. A pool is ONE array texture of `capacity` layers; each slot is a 2D view of one layer, so a
//! material binds it at group 3's binding 0 exactly as any texture (no layout change, the v0.1029
//! rule). `SlotPool` is the bookkeeping (which garment holds which layer), plain data tested without
//! a GPU; the renderer holds the textures (`create_texture_pool`, `texture_pool_write`,
//! `set_material_pool_slot`). The engine sizes each pool from the near tier's cap (and never past
//! the garments the library has of that size), and when a pool is full of garments someone near is
//! wearing, the newcomer waits, drawn in its mean colour (the middle tier's tint), until a slot
//! frees: never a stall and never an allocation on the frame.
//!
//! FORMAT. BC7 (sRGB) where the GPU offers block compression (wgpu's `TEXTURE_COMPRESSION_BC`: every
//! desktop GPU on DX12, nearly every one on Vulkan), a quarter of RGBA8's memory; RGBA8 decoded from
//! the same files elsewhere (`renderer::bc7`), which Settings > Graphics > Garment textures says in
//! words (the hardware-notice rule).

use super::Renderer;

/// The shape every texture in a pool has: level 0's size, its levels, and whether it is BC7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotClass {
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub compressed: bool,
}

/// The smallest a half gets in a garment's mip chain (one BC7 block), as the converter writes it.
pub const MIP_MIN_PX: u32 = 4;

impl SlotClass {
    /// The shape of a garment texture whose halves are `half` texels: twice as wide as high, its
    /// levels down to one block a half (`MIP_MIN_PX`). One rule for the pools' plan
    /// (`gui::garment_textures::pool_shapes`) and for a file read into a slot
    /// (`engine::look_textures`), so a garment always finds the pool planned for it.
    pub fn for_half(half: u32, compressed: bool) -> SlotClass {
        let half = half.max(MIP_MIN_PX);
        SlotClass { width: half * 2, height: half, levels: (half / MIP_MIN_PX).ilog2() + 1, compressed }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        if self.compressed {
            wgpu::TextureFormat::Bc7RgbaUnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8UnormSrgb
        }
    }

    /// The bytes level `l` takes (BC7: whole 4 by 4 blocks of 16 bytes; RGBA8: 4 a texel).
    pub fn level_bytes(&self, l: u32) -> u64 {
        let (w, h) = ((self.width >> l).max(1), (self.height >> l).max(1));
        if self.compressed {
            super::bc7::level_bytes(w, h) as u64
        } else {
            w as u64 * h as u64 * 4
        }
    }

    /// The bytes one slot takes on the GPU, every level.
    pub fn slot_bytes(&self) -> u64 {
        (0..self.levels).map(|l| self.level_bytes(l)).sum()
    }
}

/// What a garment asking for a slot gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Take {
    /// This layer is the garment's now; `evicted` is the garment that held it (idle: nobody near
    /// wore it lately), whose materials must let the slot go first.
    Slot { layer: u32, evicted: Option<usize> },
    /// Every slot holds a garment someone near is wearing: wait (drawn in its mean colour).
    Full,
}

/// Which garment holds which slot of one pool. No GPU here: the renderer's texture is `gpu`.
#[derive(Debug, Clone)]
pub struct SlotPool {
    pub class: SlotClass,
    pub capacity: u32,
    /// The garment in each layer.
    holders: Vec<Option<usize>>,
    /// The renderer's array texture, once allocated (`create_texture_pool`).
    pub gpu: Option<usize>,
    /// How many times this pool asked for its texture to be allocated: once for its whole life.
    pub allocations: u32,
}

impl SlotPool {
    pub fn new(class: SlotClass, capacity: u32) -> Self {
        let capacity = capacity.max(1);
        SlotPool { class, capacity, holders: vec![None; capacity as usize], gpu: None, allocations: 0 }
    }

    /// The layer `garment` holds, if any.
    pub fn slot_of(&self, garment: usize) -> Option<u32> {
        self.holders.iter().position(|h| *h == Some(garment)).map(|l| l as u32)
    }

    /// How many slots hold a garment.
    pub fn held(&self) -> usize {
        self.holders.iter().filter(|h| h.is_some()).count()
    }

    /// A slot for `garment`: the one it holds already, else a free one, else the slot of the holder
    /// idle longest (`idle(g)`: Some(seconds since anyone near wore it) when it may be taken, None
    /// while someone near wears it), else `Full`. The pool's texture is allocated by the caller when
    /// `needs_texture` says so, before the first write.
    pub fn take(&mut self, garment: usize, idle: impl Fn(usize) -> Option<f32>) -> Take {
        if let Some(layer) = self.slot_of(garment) {
            return Take::Slot { layer, evicted: None };
        }
        if let Some(layer) = self.holders.iter().position(Option::is_none) {
            self.holders[layer] = Some(garment);
            return Take::Slot { layer: layer as u32, evicted: None };
        }
        let mut best: Option<(usize, f32)> = None;
        for (layer, h) in self.holders.iter().enumerate() {
            if let Some(s) = h.and_then(&idle) {
                if best.is_none_or(|(_, b)| s > b) {
                    best = Some((layer, s));
                }
            }
        }
        match best {
            Some((layer, _)) => {
                let evicted = self.holders[layer].replace(garment);
                Take::Slot { layer: layer as u32, evicted }
            }
            None => Take::Full,
        }
    }

    /// Give `garment`'s slot back (it was let go). Returns the layer it held.
    pub fn give_back(&mut self, garment: usize) -> Option<u32> {
        let layer = self.slot_of(garment)?;
        self.holders[layer as usize] = None;
        Some(layer)
    }

    /// True until the pool's texture is allocated; the call that allocates it counts it.
    pub fn needs_texture(&mut self) -> bool {
        if self.gpu.is_some() {
            return false;
        }
        self.allocations += 1;
        true
    }
}

/// A pool's GPU side: the array texture and a 2D view of each layer.
pub(crate) struct GpuPool {
    texture: wgpu::Texture,
    views: Vec<wgpu::TextureView>,
    class: SlotClass,
}

/// Whether this GPU offers block compression (set once at device creation), for Settings' notice.
static BLOCK_COMPRESSION: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Record whether the device was created with `TEXTURE_COMPRESSION_BC` (renderer start, once).
pub fn note_block_compression(offered: bool) {
    let _ = BLOCK_COMPRESSION.set(offered);
}

/// Whether the GPU draws BC7 textures; None before the renderer has started.
pub fn block_compression() -> Option<bool> {
    BLOCK_COMPRESSION.get().copied()
}

/// The environment variable that makes the renderer leave block compression off as though the GPU
/// had none (the fallback's red arm in a rig: `HUMANITY_NO_BC=1`).
pub const NO_BC_VAR: &str = "HUMANITY_NO_BC";

/// The device feature to ask for, from what the adapter offers: `TEXTURE_COMPRESSION_BC` where it
/// has it (asking for exactly what it reports can never fail device creation, the v0.782 rule), none
/// where it does not or `HUMANITY_NO_BC=1` says to leave it off. Logged, and noted for Settings.
pub fn block_compression_feature(adapter: wgpu::Features) -> wgpu::Features {
    let no_bc = std::env::var(NO_BC_VAR).is_ok_and(|v| v.trim() == "1");
    let granted = if no_bc { wgpu::Features::empty() } else { adapter & wgpu::Features::TEXTURE_COMPRESSION_BC };
    log::info!(
        "[Garments] block compression (BC7): {}",
        if no_bc {
            "left off (HUMANITY_NO_BC=1)"
        } else if granted.is_empty() {
            "not offered by this GPU (garment textures uncompressed)"
        } else {
            "offered"
        }
    );
    note_block_compression(!granted.is_empty());
    granted
}

/// The key a pool's memory is reported under in `vram.textures` (materials use their own index).
fn vram_key(pool: usize) -> u64 {
    (1u64 << 48) | pool as u64
}

impl Renderer {
    /// Whether garment textures go to the GPU as BC7 (the device has `TEXTURE_COMPRESSION_BC`).
    pub fn block_compression(&self) -> bool {
        self.device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    }

    /// Allocate a pool's array texture: `layers` slots of `class`, one allocation for the pool's
    /// whole life. Returns the pool's id. Its memory goes into `vram.textures`.
    pub fn create_texture_pool(&mut self, class: SlotClass, layers: u32) -> usize {
        let layers = layers.clamp(1, self.device.limits().max_texture_array_layers);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Garment texture pool"),
            size: wgpu::Extent3d { width: class.width, height: class.height, depth_or_array_layers: layers },
            mip_level_count: class.levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: class.format(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let views = (0..layers)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Garment texture slot"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let id = match self.texture_pools.iter().position(Option::is_none) {
            Some(free) => free,
            None => {
                self.texture_pools.push(None);
                self.texture_pools.len() - 1
            }
        };
        self.texture_pools[id] = Some(GpuPool { texture, views, class });
        humanity_core::frame_costs::set_vram_keyed("vram.textures", vram_key(id), class.slot_bytes() * layers as u64);
        id
    }

    /// Write a garment's levels (biggest first, `class`'s shape) into slot `layer` of pool `pool`:
    /// one queue write a level, no allocation of a texture. Errs on a shape that is not the pool's.
    pub fn texture_pool_write(&mut self, pool: usize, layer: u32, levels: &[Vec<u8>]) -> Result<(), String> {
        let Some(Some(p)) = self.texture_pools.get(pool) else { return Err(format!("no texture pool {pool}")) };
        let c = p.class;
        if levels.len() as u32 != c.levels || layer as usize >= p.views.len() {
            return Err(format!("{} levels into slot {layer} of a pool of {} levels and {} slots", levels.len(), c.levels, p.views.len()));
        }
        for (l, bytes) in levels.iter().enumerate() {
            let l = l as u32;
            if bytes.len() as u64 != c.level_bytes(l) {
                return Err(format!("level {l} has {} bytes, the pool's {}", bytes.len(), c.level_bytes(l)));
            }
            let (w, h) = ((c.width >> l).max(1), (c.height >> l).max(1));
            let (bytes_per_row, rows) = if c.compressed { (w.div_ceil(4) * 16, h.div_ceil(4)) } else { (w * 4, h) };
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &p.texture, mip_level: l, origin: wgpu::Origin3d { x: 0, y: 0, z: layer }, aspect: wgpu::TextureAspect::All },
                bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: Some(rows) },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
        Ok(())
    }

    /// Point material `idx` at slot `layer` of pool `pool` and set its `params2` (the character
    /// material types' notes, `materials.rs`). The material owns no texture of its own then; the
    /// pool's memory is counted once, under the pool.
    pub fn set_material_pool_slot(&mut self, idx: usize, pool: usize, layer: u32, params2: [f32; 4]) {
        let Some(view) = self.texture_pools.get(pool).and_then(|p| p.as_ref()).and_then(|p| p.views.get(layer as usize)) else { return };
        let bg = self.build_albedo_group_from_view(view, &self.albedo_sampler);
        if let Some(mat) = self.materials.get_mut(idx) {
            mat.albedo_bind_group = Some(bg);
            mat.albedo_texture = None;
        }
        self.update_material_params2(idx, params2);
        humanity_core::frame_costs::set_vram_keyed("vram.textures", idx as u64, 0);
    }

    /// A character material (type 25 or 26, `materials.rs`) over slot `layer` of pool `pool`.
    pub fn add_character_material_on_slot(&mut self, base_color: [f32; 4], roughness: f32, material_type: f32, params2: [f32; 4], pool: usize, layer: u32) -> usize {
        let idx = self.add_material_full(base_color, 0.0, roughness, material_type, 0.0);
        self.set_material_pool_slot(idx, pool, layer, params2);
        idx
    }

    /// Let a pool's texture go (the garment texture size changed, or the library was read again).
    /// Every material over its slots must have let them go first (`drop_material_texture`).
    pub fn release_texture_pool(&mut self, pool: usize) {
        if let Some(slot) = self.texture_pools.get_mut(pool) {
            *slot = None;
            humanity_core::frame_costs::set_vram_keyed("vram.textures", vram_key(pool), 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class() -> SlotClass {
        SlotClass { width: 2048, height: 1024, levels: 9, compressed: true }
    }

    /// A SLOT IS HANDED FROM GARMENT TO GARMENT, AND THE POOL'S TEXTURE IS ALLOCATED ONCE: 41
    /// garments (the crowd's count in the look's sweep) come and go through a pool of 8 slots, each
    /// given back when let go; the pool allocates once, and a garment arriving after one left takes
    /// the slot it freed. Seen red 2026-11-10 with `needs_texture` answering true for every garment
    /// (an allocation per garment, as main did): "41 garments made 41 allocations".
    #[test]
    fn the_pool_hands_slots_on_and_allocates_once() {
        let mut pool = SlotPool::new(class(), 8);
        let mut allocated = 0u32;
        let mut held: Vec<usize> = Vec::new();
        for g in 0..41usize {
            if held.len() == 8 {
                let gone = held.remove(0);
                let freed = pool.give_back(gone).expect("a held garment gives its slot back");
                match pool.take(g, |_| None) {
                    Take::Slot { layer, evicted } => {
                        assert_eq!(layer, freed, "garment {g} takes the slot garment {gone} freed");
                        assert_eq!(evicted, None);
                    }
                    Take::Full => panic!("garment {g} found no slot after {gone} left"),
                }
            } else {
                assert!(matches!(pool.take(g, |_| None), Take::Slot { evicted: None, .. }));
            }
            if pool.needs_texture() {
                allocated += 1;
                pool.gpu = Some(0);
            }
            held.push(g);
        }
        assert_eq!(pool.allocations, 1, "41 garments made {} allocations", pool.allocations);
        assert_eq!(allocated, 1);
        assert_eq!(pool.held(), 8);
    }

    /// A FULL POOL TAKES ONLY AN IDLE SLOT, THE LONGEST IDLE FIRST, AND OTHERWISE SAYS SO: with every
    /// slot worn, a newcomer gets `Full` (it waits in its mean colour, never an allocation); once two
    /// holders are idle, it takes the slot of the one idle longest and names it, so its materials let
    /// the slot go. Asking again for a garment that holds a slot gives the same slot.
    #[test]
    fn a_full_pool_takes_only_the_longest_idle_slot() {
        let mut pool = SlotPool::new(class(), 3);
        for g in 0..3 {
            assert!(matches!(pool.take(g, |_| None), Take::Slot { .. }));
        }
        assert_eq!(pool.take(9, |_| None), Take::Full, "every slot worn: wait");
        let idle = |g: usize| match g {
            0 => Some(2.0),
            2 => Some(31.0),
            _ => None,
        };
        let layer_of_2 = pool.slot_of(2).expect("2 holds a slot");
        assert_eq!(pool.take(9, idle), Take::Slot { layer: layer_of_2, evicted: Some(2) }, "the longest idle gives way");
        assert_eq!(pool.slot_of(9), Some(layer_of_2));
        assert_eq!(pool.take(9, |_| None), Take::Slot { layer: layer_of_2, evicted: None }, "a holder keeps its slot");
    }

    /// A SLOT'S MEMORY IS THE CLASS'S LEVELS: BC7 is a byte a texel (a quarter of RGBA8), down to one
    /// block a level; the pack at 1024 a half with its levels to 4 is 2.67 MB as BC7, 10.67 MB as
    /// RGBA8.
    #[test]
    fn a_slot_s_memory_is_its_levels() {
        let bc = class();
        let raw = SlotClass { compressed: false, ..bc };
        assert_eq!(bc.level_bytes(0), 2048 * 1024);
        assert_eq!(raw.level_bytes(0), 2048 * 1024 * 4);
        assert_eq!(bc.level_bytes(8), 2 * 16, "8 by 4 texels: two blocks");
        assert_eq!(raw.slot_bytes(), bc.slot_bytes() * 4);
        assert_eq!(bc.slot_bytes(), 2_796_192);
    }
}
