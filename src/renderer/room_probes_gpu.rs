//! ROOM GI, RUNG 1, the GPU half (docs/design/room-gi.md; the what and the
//! why are in `room_probes.rs`, whose CPU twin this mirrors).
//!
//! WHAT LIVES HERE. The probe atlas (one Rgba16Float texture: every resident
//! probe's 8x8 octahedral irradiance tile on top, its 16x16 depth-moment tile
//! below, each with the 1-texel border, and in its LAST ROW one texel per room
//! holding the room's ambient colour), the room table the fragments and the
//! update both read, and the per-frame compute pass (`RoomGi::encode`):
//! - `cs_ambient` and `cs_ambient_resolve`: every room's AMBIENT COLOUR
//!   (`room_probes::room_ambient`) into the atlas's last row, every frame;
//! - `cs_seed`: the probes of each room that came into the atlas this frame
//!   set to that colour (PROBE STREAMING, `room_probes::Residency`);
//! - `cs_trace` and `cs_resolve`: 64 rays per updated probe against its
//!   room's box, and the finished tiles into the atlas.
//! `Renderer::update_room_gi` runs it once a frame before the scene pass;
//! `Renderer::set_room_gi_rooms` hands it the ship's rooms whenever the home
//! is built or edited.
//!
//! PROBE STREAMING (2026-10-21). The atlas holds the Bounced light detail
//! level's probe budget (`ProbeBudget::max_probes`), made once per level and
//! never grown, and the rooms nearest the player hold those probes
//! (`Residency::step`, every frame). A room out of the atlas, and every room
//! on the Ambient level (a budget of 0, the atlas its last row alone), shows
//! its ambient colour; a room coming in is seeded with that colour; a room
//! going out fades to it first. So the ship's size no longer decides any
//! room's probe spacing (`room_probes::lattice_for`).
//!
//! BINDINGS. The fragments read the atlas, its sampler and the room table
//! through the SHARED camera bind group, group 0 bindings 5 to 7 (see
//! `pipeline::camera_bind_group`). Replacing any of the three (the atlas when
//! the level changes, the table when the room count doubles) makes every
//! camera bind group stale, so `rebuild_camera_bind_groups` rebuilds both of
//! them with every binding: missing one is the v0.1029 failure, where world
//! entry panics while a menu-only boot stays green.
//!
//! BUDGET. At most `MAX_UPDATES_PER_FRAME` probes are traced per frame: the
//! room the camera stands in first (every frame, so the light you are looking
//! at answers immediately), then the other resident rooms round-robin.
//!
//! THE FLOORS BETWEEN STOREYS (ship homes increment 7, lane L) ride in the same
//! room table, after the rooms: every solid floor slab once, grouped into
//! layers for the sun (`room_probes::layered_rows`), then each scene
//! light's RUN of the slabs its range reaches, copied, which `update_room_gi`
//! names in the light's record (`light::gpu_packed_with_slabs`) and rewrites
//! the scene light buffer with. The fragments read them through
//! `HAS_SLAB_OCCLUSION`'s two guards (85-room-gi.wgsl); the probe update reads
//! none (`room_probes::light_irradiance` says why). A home with no upper floor
//! writes no slab row and leaves the light buffer as `set_point_lights` wrote
//! it, so nothing changes for it.

use glam::Vec3;

use super::bounced_light_detail::{mb_words, probe_memory_bytes};
use super::light::{gpu_packed, gpu_packed_with_slabs};
use super::room_probes::{
    assign_lights, lattice_for, layered_rows, light_anchor, pack_room, pick_order, ray_rotation, room_ambient, run_plane,
    slabs_touched, AtlasLayout, DirectSource, GiHeaderGpu, GiParamsGpu, GiRoomGpu, OccluderSlab, ProbeBudget, Residency,
    ResidencyStep, RoomBox, Sun, HEADER_BYTES, HYSTERESIS, MAX_AMBIENT_ROOMS, MAX_SEEDS_PER_FRAME, NORMAL_BIAS_M,
    ROOM_BYTES, SKY_LIGHTING_EXPOSURE,
};
use super::{pipeline, Renderer};

/// Probes the update refreshes per frame, at most.
pub const MAX_UPDATES_PER_FRAME: u32 = 4096;

/// The most rows the lights' slab runs may take in one frame. The homestead's
/// upper floor and its 49 lights (a strip counts once per segment) take 197
/// (149 slab rows and 48 layer rows; 34 KB a frame with the sun's 14), the
/// longest run 11 of its 13 merged slabs (counted by
/// `engine::room_gi::tests::the_homestead_upper_floor_fits_the_light_runs`
/// with `--nocapture`); this bounds the dev stress test
/// (showcase `lights:N` puts up to 2048 lights round the camera) to 1.3 MB a
/// frame. A light past the cap names no run, so a floor does not stop it, and
/// the log says so once.
pub const MAX_SLAB_RUN_ROWS: usize = 8192;
/// Past this distance from the nearest room the update stops (the probes and
/// the ambient colours keep what they have): nobody on a planet's surface
/// needs the station's rooms relit every frame.
pub const UPDATE_REACH_M: f32 = 60.0;

/// The update shader: the sampling part the megashader also compiles, then
/// the compute entries. One source of truth for how probes are read.
pub const UPDATE_SHADER: &str = concat!(
    include_str!("../../assets/shaders/pbr/85-room-gi.wgsl"),
    include_str!("../../assets/shaders/room_probes_update.wgsl"),
);

pub(super) const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The probe slots the scratch holds: one update's worth, or the level's
/// whole budget when that is smaller (the Ambient level's scratch is its
/// ambient row alone).
pub fn scratch_slots(max_probes: u32) -> u32 {
    max_probes.min(MAX_UPDATES_PER_FRAME)
}

pub struct RoomGi {
    /// The dev switch (showcase `{"room_gi":"0"}`): the room table says "off",
    /// every fragment keeps the old ambient floor, and nothing is dispatched,
    /// so a same-boot A/B measures exactly what room GI costs and adds.
    pub off: bool,
    /// The second dev switch (showcase `{"room_gi_vis":"1"}`): every room runs
    /// DDGI's Chebyshev visibility test in its fragments and its update, which
    /// rung 1 skips because inside a box it is an identity. For A/B of the
    /// test's cost and proof that it changes nothing yet.
    pub force_visibility: bool,
    /// The rooms in the HOME frame (render space adds the station offset each
    /// frame), their lattices and each one's probe count.
    rooms: Vec<RoomBox>,
    counts: Vec<[u32; 3]>,
    probes: Vec<u32>,
    /// Probes in every room, resident or not (the log's figure).
    total: u32,
    /// The Bounced light detail level's spacing and budget (`set_budget`).
    budget: ProbeBudget,
    /// Which rooms hold probes, and where (probe streaming).
    residency: Residency,
    layout: AtlasLayout,
    _atlas: wgpu::Texture,
    pub(super) atlas_view: wgpu::TextureView,
    pub(super) sampler: wgpu::Sampler,
    _scratch: wgpu::Texture,
    scratch_view: wgpu::TextureView,
    scratch_layout: AtlasLayout,
    pub(super) rooms_buf: wgpu::Buffer,
    rooms_cap: usize,
    lights_buf: wgpu::Buffer,
    lights_cap: usize,
    updates_buf: wgpu::Buffer,
    params_buf: wgpu::Buffer,
    trace_bgl: wgpu::BindGroupLayout,
    resolve_bgl: wgpu::BindGroupLayout,
    trace: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    ambient: wgpu::ComputePipeline,
    ambient_resolve: wgpu::ComputePipeline,
    seed: wgpu::ComputePipeline,
    /// Round-robin position over the atlas's probe slots.
    cursor: u32,
    /// Where the window through a camera room bigger than the budget stands.
    cam_cursor: u32,
    /// Updates run so far: seeds each update's ray rotation.
    frame: u32,
    /// The atlas or the room table was replaced; the camera bind groups must
    /// be rebuilt before the next draw.
    pub(super) rebind: bool,
    /// Probes traced and seeded in the last update (the log and the dev drop
    /// read them).
    pub last_updated: u32,
    pub last_seeded: u32,
    /// The update's bind groups (`make_bind_groups`); None = build them.
    bind_groups: Option<(wgpu::BindGroup, wgpu::BindGroup)>,
    /// Every SOLID floor slab of the ship in the HOME frame, adjacent ones
    /// merged (`engine::room_gi::occluder_slabs`); empty for a one-storey home.
    slabs: Vec<OccluderSlab>,
    /// The dev switch (showcase `{"slab_occlusion":"0"}`): no slab rows and no
    /// runs, so lamps and the sun pass every floor as they did before storeys.
    /// The same-boot A/B of what the floors take away and what they cost.
    pub slabs_off: bool,
    /// Which half of the floors' test runs while they are on: the sun's
    /// (`"sun"` writes no light runs) or the lamps' (`"lamps"` writes no sun
    /// list), both by default. For attributing the floors' cost at runtime,
    /// where a shader variant would change the code generation it measures.
    pub slabs_sun: bool,
    pub slabs_lamps: bool,
    /// Some light in the scene light buffer names a run. A frame with none
    /// must then rewrite the buffer once: a light `set_point_lights` did not
    /// repack this frame would otherwise keep a run whose rows now hold
    /// something else.
    slab_runs_live: bool,
    /// Rows written after the rooms in the last frame: the slabs (the sun's
    /// list) and the lights' runs.
    pub last_slab_rows: u32,
    pub last_run_rows: u32,
    /// When the dev drop (`debug/room_gi.json`) was last written.
    dumped_at: Option<std::time::Instant>,
}

fn atlas_texture(device: &wgpu::Device, layout: &AtlasLayout, label: &str) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: layout.width, height: layout.height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: ATLAS_FORMAT,
        // COPY_SRC and COPY_DST: packing the atlas moves rooms' tiles through
        // the scratch (`RoomGi::encode_moves`); the GPU tests read tiles back.
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn storage_buffer(device: &wgpu::Device, label: &str, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.max(16),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn rooms_buffer(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
    storage_buffer(device, "Room GI Rooms", HEADER_BYTES + ROOM_BYTES * cap.max(1) as u64)
}

fn entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None }
}

fn buffer_ty(ty: wgpu::BufferBindingType) -> wgpu::BindingType {
    wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }
}

fn texture_ty() -> wgpu::BindingType {
    wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    }
}

fn storage_texture_ty() -> wgpu::BindingType {
    wgpu::BindingType::StorageTexture {
        access: wgpu::StorageTextureAccess::WriteOnly,
        format: ATLAS_FORMAT,
        view_dimension: wgpu::TextureViewDimension::D2,
    }
}

/// Copy `n` probes' tiles (both kinds) from slot `src.2` of one atlas-laid
/// texture to slot `dst.2` of another, a rectangle per run of tiles that stays
/// within one tile row of both.
fn copy_tiles(
    enc: &mut wgpu::CommandEncoder,
    src: (&wgpu::Texture, &AtlasLayout, u32),
    dst: (&wgpu::Texture, &AtlasLayout, u32),
    n: u32,
) {
    use super::room_probes::{DEPTH_TILE, IRR_TILE};
    fn region(tex: &wgpu::Texture, at: (u32, u32)) -> wgpu::TexelCopyTextureInfo<'_> {
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d { x: at.0, y: at.1, z: 0 },
            aspect: wgpu::TextureAspect::All,
        }
    }
    // (tiles per row, tile size, origin of slot p) for each tile kind.
    let kinds: [(fn(&AtlasLayout) -> u32, u32, fn(&AtlasLayout, u32) -> (u32, u32)); 2] = [
        (|l| l.irr_per_row, IRR_TILE, |l, p| l.irr_origin(p)),
        (|l| l.depth_per_row, DEPTH_TILE, |l, p| l.depth_origin(p)),
    ];
    for (per_row, tile, origin) in kinds {
        let mut p = 0;
        while p < n {
            let (s, d) = (src.2 + p, dst.2 + p);
            let run = (n - p).min(per_row(src.1) - s % per_row(src.1)).min(per_row(dst.1) - d % per_row(dst.1));
            enc.copy_texture_to_texture(
                region(src.0, origin(src.1, s)),
                region(dst.0, origin(dst.1, d)),
                wgpu::Extent3d { width: run * tile, height: tile, depth_or_array_layers: 1 },
            );
            p += run;
        }
    }
}

/// What one frame asks of the compute pass (`RoomGi::encode`): the room
/// count (one ambient workgroup each), then the update list's two parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassWork {
    pub rooms: u32,
    pub traced: u32,
    pub seeded: u32,
}

impl RoomGi {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Room Probe Update"),
            source: wgpu::ShaderSource::Wgsl(UPDATE_SHADER.into()),
        });
        let read_only = wgpu::BufferBindingType::Storage { read_only: true };
        let trace_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Room Probe Trace BGL"),
            entries: &[
                entry(0, buffer_ty(wgpu::BufferBindingType::Uniform)),
                entry(1, buffer_ty(read_only)),
                entry(2, buffer_ty(read_only)),
                entry(3, buffer_ty(read_only)),
                entry(4, texture_ty()),
                entry(5, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
                entry(6, storage_texture_ty()),
                entry(7, texture_ty()),
                entry(8, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let resolve_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Room Probe Resolve BGL"),
            entries: &[
                entry(0, buffer_ty(wgpu::BufferBindingType::Uniform)),
                entry(1, buffer_ty(read_only)),
                entry(3, buffer_ty(read_only)),
                entry(10, texture_ty()),
                entry(11, storage_texture_ty()),
            ],
        });
        let make = |bgl: &wgpu::BindGroupLayout, entry_point: &str| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Room Probe PL"),
                bind_group_layouts: &[bgl],
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry_point),
                layout: Some(&layout),
                module: &shader,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let trace = make(&trace_bgl, "cs_trace");
        let resolve = make(&resolve_bgl, "cs_resolve");
        let ambient = make(&trace_bgl, "cs_ambient");
        let ambient_resolve = make(&resolve_bgl, "cs_ambient_resolve");
        let seed = make(&resolve_bgl, "cs_seed");
        // No rooms yet: the ambient row alone. The first rooms lay the atlas
        // out at the level's budget (`relayout`).
        let layout = AtlasLayout::for_probes(0);
        let (atlas, atlas_view) = atlas_texture(device, &layout, "Room GI Atlas");
        let scratch_layout = AtlasLayout::for_probes(0);
        let (scratch, scratch_view) = atlas_texture(device, &scratch_layout, "Room GI Scratch");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Room GI Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Room GI Params"),
            size: std::mem::size_of::<GiParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            off: false,
            force_visibility: false,
            rooms: Vec::new(),
            counts: Vec::new(),
            probes: Vec::new(),
            total: 0,
            budget: ProbeBudget { spacing_m: ProbeBudget::default().spacing_m, max_probes: 0 },
            residency: Residency::new(0, 0),
            layout,
            _atlas: atlas,
            atlas_view,
            sampler,
            _scratch: scratch,
            scratch_view,
            scratch_layout,
            rooms_buf: rooms_buffer(device, 1),
            rooms_cap: 1,
            lights_buf: storage_buffer(device, "Room GI Lights", 64 * 64),
            lights_cap: 64,
            updates_buf: storage_buffer(
                device,
                "Room GI Updates",
                8 * (MAX_UPDATES_PER_FRAME + MAX_SEEDS_PER_FRAME) as u64,
            ),
            params_buf,
            trace_bgl,
            resolve_bgl,
            trace,
            resolve,
            ambient,
            ambient_resolve,
            seed,
            cursor: 0,
            cam_cursor: 0,
            frame: 0,
            rebind: false,
            last_updated: 0,
            last_seeded: 0,
            bind_groups: None,
            slabs: Vec::new(),
            slabs_off: false,
            slabs_sun: true,
            slabs_lamps: true,
            slab_runs_live: false,
            last_slab_rows: 0,
            last_run_rows: 0,
            dumped_at: None,
        }
    }

    /// Solid floor slabs in the ship (home frame), for the dev switch's log.
    pub fn slab_count(&self) -> usize {
        self.slabs.len()
    }

    /// Probes in every room of the ship, resident or not.
    pub fn probe_count(&self) -> u32 {
        self.total
    }

    /// Rooms holding probes and the probes they hold (fading rooms included).
    pub fn resident(&self) -> (usize, u32) {
        (self.residency.resident_rooms(), self.residency.resident_probes())
    }

    /// GPU memory room GI holds: the atlas and the scratch (8 bytes a texel)
    /// and its buffers. The Performance page counts it with the render targets.
    pub fn vram_bytes(&self) -> u64 {
        (self.layout.texel_count() + self.scratch_layout.texel_count()) as u64 * 8
            + self.rooms_buf.size()
            + self.lights_buf.size()
            + self.updates_buf.size()
            + self.params_buf.size()
    }

    /// The Bounced light detail level's budget, as last set.
    pub fn budget(&self) -> ProbeBudget {
        self.budget
    }

    /// New rooms (home frame). Recomputes the lattice, carries the probes of
    /// every room that did not change over, and grows the room table if it no
    /// longer fits (flagging `rebind`). Unchanged rooms are a no-op, which
    /// matters because the construction editor rebuilds the home on every
    /// drag frame.
    fn set_rooms(&mut self, device: &wgpu::Device, rooms: Vec<RoomBox>) {
        if rooms == self.rooms {
            return;
        }
        self.relayout(device, rooms);
    }

    /// A new Bounced light detail level (Settings, or the dev request
    /// `{"bounced_light_detail":"low"}`). The same rooms are laid out again at
    /// the new spacing, and a new budget makes the atlas again at its size
    /// (smaller too): every room starts out of it, showing its ambient colour,
    /// and the nearest come back in seeded with it the same frame. False when
    /// nothing changed.
    fn set_budget(&mut self, device: &wgpu::Device, budget: ProbeBudget) -> bool {
        if budget == self.budget {
            return false;
        }
        self.budget = budget;
        let rooms = std::mem::take(&mut self.rooms);
        self.relayout(device, rooms);
        true
    }

    /// Lay `rooms` out at the current budget: the lattice, the atlas and the
    /// scratch (made again only when the budget's size changed), the
    /// residency (carried over otherwise), the room table.
    fn relayout(&mut self, device: &wgpu::Device, mut rooms: Vec<RoomBox>) {
        if rooms.len() > MAX_AMBIENT_ROOMS {
            log::warn!(
                "[RoomGI] {} rooms; the ambient row holds {MAX_AMBIENT_ROOMS}, so the rest keep the old floor",
                rooms.len()
            );
            rooms.truncate(MAX_AMBIENT_ROOMS);
        }
        let lattice = lattice_for(&rooms, self.budget);
        let probes: Vec<u32> = (0..rooms.len()).map(|i| lattice.probes_in(i)).collect();
        let cap = self.budget.max_probes;
        if self.layout.probes != cap {
            self.layout = AtlasLayout::for_probes(cap);
            let (tex, view) = atlas_texture(device, &self.layout, "Room GI Atlas");
            self._atlas = tex;
            self.atlas_view = view;
            self.scratch_layout = AtlasLayout::for_probes(scratch_slots(cap));
            let (tex, view) = atlas_texture(device, &self.scratch_layout, "Room GI Scratch");
            self._scratch = tex;
            self.scratch_view = view;
            self.rebind = true;
            self.bind_groups = None;
            self.residency = Residency::new(cap, rooms.len());
            log::info!(
                "[RoomGI] atlas {}x{} for {} probes ({:.1} MB), scratch {} slots",
                self.layout.width,
                self.layout.height,
                cap,
                self.layout.texel_count() as f64 * 8.0 / 1.048576e6,
                self.scratch_layout.probes
            );
        } else {
            self.residency = self.residency.carried_over((&self.rooms, &self.counts), (&rooms, &lattice.counts));
        }
        if rooms.len() > self.rooms_cap {
            let mut cap = self.rooms_cap.max(1);
            while cap < rooms.len() {
                cap *= 2;
            }
            self.rooms_cap = cap;
            self.rooms_buf = rooms_buffer(device, cap);
            self.rebind = true;
            self.bind_groups = None;
        }
        let total: u32 = probes.iter().sum();
        let finest = lattice.spacing.iter().copied().fold(f32::INFINITY, f32::min);
        let coarsened = lattice.spacing.iter().filter(|s| **s > self.budget.spacing_m * 1.0001).count();
        log::info!(
            "[RoomGI] {} rooms, {} probes at {:.2} m ({} coarsened alone to fit a room's cap of {}); \
             the {} nearest the player hold up to {} at once ({})",
            rooms.len(),
            total,
            if finest.is_finite() { finest } else { self.budget.spacing_m },
            coarsened,
            self.budget.per_room(),
            if self.budget.ambient_only() { "none: Ambient level," } else { "rooms" },
            cap,
            mb_words(probe_memory_bytes(cap))
        );
        self.rooms = rooms;
        self.counts = lattice.counts;
        self.probes = probes;
        self.total = total;
        self.cursor = 0;
        self.cam_cursor = 0;
    }

    /// The update's two bind groups. Cached (`bind_groups`) and rebuilt only
    /// when a resource they hold is replaced: the atlas, the scratch, the room
    /// table or the light list.
    fn make_bind_groups(&self, device: &wgpu::Device, sky: &wgpu::TextureView) -> (wgpu::BindGroup, wgpu::BindGroup) {
        let trace = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Room Probe Trace BG"),
            layout: &self.trace_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.rooms_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.lights_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.updates_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.atlas_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.scratch_view) },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(sky) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let resolve = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Room Probe Resolve BG"),
            layout: &self.resolve_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.rooms_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.updates_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::TextureView(&self.scratch_view) },
                wgpu::BindGroupEntry { binding: 11, resource: wgpu::BindingResource::TextureView(&self.atlas_view) },
            ],
        });
        (trace, resolve)
    }

    /// This frame's trace list: the camera's room first (all of it, or for a
    /// room bigger than the budget three quarters of the budget, walking
    /// through the room frame by frame), then the other resident rooms
    /// round-robin, a cursor walking the atlas's slots. Rooms fading out are
    /// not traced. Entries are (probe, room index).
    fn build_updates(&mut self, cam_room: Option<usize>) -> Vec<[u32; 2]> {
        let mut ranges: Vec<(u32, u32, usize)> = (0..self.rooms.len())
            .filter_map(|i| self.residency.updated(i).map(|r| (r.first, r.count, i)))
            .collect();
        ranges.sort();
        let traced: u32 = ranges.iter().map(|r| r.1).sum();
        let budget = MAX_UPDATES_PER_FRAME.min(scratch_slots(self.budget.max_probes)).min(traced) as usize;
        let mut list: Vec<[u32; 2]> = Vec::with_capacity(budget);
        let mut skip = 0..0u32;
        if let Some(cam) = cam_room.and_then(|ci| ranges.iter().find(|r| r.2 == ci).copied()) {
            let (f, n, ci) = cam;
            if n as usize <= budget {
                list.extend((f..f + n).map(|p| [p, ci as u32]));
                skip = f..f + n;
            } else {
                let take = (budget * 3 / 4) as u32;
                for k in 0..take {
                    list.push([f + (self.cam_cursor + k) % n, ci as u32]);
                }
                self.cam_cursor = (self.cam_cursor + take) % n;
            }
        }
        // Round-robin over the resident ranges in atlas order. Every pass of
        // the loop either fills the list, finishes a range, or wraps, so two
        // wraps' worth of passes covers every range.
        let mut guard = 0usize;
        while list.len() < budget && guard < 2 * ranges.len() + 2 {
            guard += 1;
            let Some(&(first, count, room)) = ranges.iter().find(|r| r.0 + r.1 > self.cursor) else {
                self.cursor = 0;
                continue;
            };
            let start = self.cursor.max(first);
            let take = (first + count - start).min((budget - list.len()) as u32);
            for p in start..start + take {
                if !skip.contains(&p) {
                    list.push([p, room as u32]);
                }
            }
            self.cursor = start + take;
        }
        list
    }

    /// Record this frame's compute pass: the ambient colours (`work.rooms`
    /// workgroups and their copy), the seeds, then the traces and their copy,
    /// in that order, so the seeds take this frame's colour and the traces
    /// read the seeded probes. `bgs` are `make_bind_groups`' pair.
    fn encode(
        &self,
        enc: &mut wgpu::CommandEncoder,
        bgs: &(wgpu::BindGroup, wgpu::BindGroup),
        work: PassWork,
        timestamp_writes: Option<wgpu::ComputePassTimestampWrites<'_>>,
    ) {
        let (trace_bg, resolve_bg) = bgs;
        let mut pass =
            enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Room Probe Update"), timestamp_writes });
        if work.rooms > 0 {
            pass.set_pipeline(&self.ambient);
            pass.set_bind_group(0, trace_bg, &[]);
            pass.dispatch_workgroups(work.rooms, 1, 1);
            pass.set_pipeline(&self.ambient_resolve);
            pass.set_bind_group(0, resolve_bg, &[]);
            pass.dispatch_workgroups(work.rooms.div_ceil(64), 1, 1);
        }
        if work.seeded > 0 {
            pass.set_pipeline(&self.seed);
            pass.set_bind_group(0, resolve_bg, &[]);
            pass.dispatch_workgroups(work.seeded, 1, 1);
        }
        if work.traced > 0 {
            pass.set_pipeline(&self.trace);
            pass.set_bind_group(0, trace_bg, &[]);
            pass.dispatch_workgroups(work.traced, 1, 1);
            pass.set_pipeline(&self.resolve);
            pass.set_bind_group(0, resolve_bg, &[]);
            pass.dispatch_workgroups(work.traced, 1, 1);
        }
    }

    /// PACK THE ATLAS on the GPU (`room_probes::Residency::pack`): each move
    /// copies a room's tiles down, before this frame's compute pass reads or
    /// writes any. A texture cannot be copied onto itself, so each move goes
    /// through the scratch, `scratch_layout.probes` probes at a time (the
    /// scratch is free outside the compute pass), lowest chunk first: a move
    /// is always down, so a chunk never lands on probes a later chunk has
    /// still to read. Copies in one encoder run in order, so a room of any
    /// size moves in this one frame.
    fn encode_moves(&self, enc: &mut wgpu::CommandEncoder, moves: &[super::room_probes::TileMove]) {
        let chunk = self.scratch_layout.probes;
        if chunk == 0 {
            return;
        }
        for m in moves {
            let mut done = 0;
            while done < m.count {
                let n = chunk.min(m.count - done);
                copy_tiles(enc, (&self._atlas, &self.layout, m.from + done), (&self._scratch, &self.scratch_layout, 0), n);
                copy_tiles(enc, (&self._scratch, &self.scratch_layout, 0), (&self._atlas, &self.layout, m.to + done), n);
                done += n;
            }
        }
    }

    /// One line when the resident set changed, so the rig log shows the
    /// probes following the player.
    fn log_residency(&self, step: &ResidencyStep) {
        if !step.changed() {
            return;
        }
        log::info!(
            "[RoomGI] resident {} of {} rooms, {} of {} probes ({} in, {} fading out, {} out, {} waiting, {} moved to pack the atlas)",
            self.residency.resident_rooms(),
            self.rooms.len(),
            self.residency.resident_probes(),
            self.budget.max_probes,
            step.entered.len(),
            step.started_leaving.len(),
            step.left.len(),
            step.waiting.len(),
            step.moved.len()
        );
    }
}

impl Renderer {
    /// Hand room GI the ship's rooms (home frame), after every home build or
    /// edit. See `engine::room_gi::rooms_changed`.
    pub fn set_room_gi_rooms(&mut self, rooms: Vec<RoomBox>) {
        self.room_gi.set_rooms(&self.device, rooms);
        if self.room_gi.rebind {
            self.rebuild_camera_bind_groups();
        }
    }

    /// Hand the renderer the ship's SOLID floor slabs (home frame), after
    /// every home build or edit, beside the rooms (`engine::room_gi::
    /// rooms_changed`). From the next frame they keep lamp light and sunlight
    /// on their own storey (`update_room_gi` writes their rows). Kept apart
    /// from the rooms' layout, so a Bounced light detail change, which lays
    /// the rooms out again, leaves them as they are.
    pub fn set_room_gi_slabs(&mut self, slabs: Vec<OccluderSlab>) {
        if slabs != self.room_gi.slabs {
            log::info!("[RoomGI] {} solid floor slabs keep lamp light and sunlight to their storey", slabs.len());
            self.room_gi.slabs = slabs;
        }
    }

    /// The Bounced light detail level's budget, every frame and before every
    /// rooms hand-over (`engine::room_gi`). A no-op while it is unchanged; a
    /// change lays the rooms out again, remakes the atlas at the new size and
    /// rebinds it, before this frame's update.
    pub fn set_room_gi_budget(&mut self, budget: ProbeBudget) {
        if self.room_gi.set_budget(&self.device, budget) {
            log::info!(
                "[RoomGI] bounced light detail: {:.2} m spacing, up to {} probes resident ({})",
                budget.spacing_m,
                budget.max_probes,
                mb_words(probe_memory_bytes(budget.max_probes))
            );
            if self.room_gi.rebind {
                self.rebuild_camera_bind_groups();
            }
        }
    }

    /// Rebuild BOTH camera bind groups (the scene's and the sun-shadow pass's)
    /// with every group-0 binding. The one place that does it after `new`, so
    /// a resource that grows (the light list, the environment regions, the
    /// room GI atlas or table) can never leave a bind group pointing at a
    /// replaced buffer or missing a binding.
    pub(crate) fn rebuild_camera_bind_groups(&mut self) {
        let layout = &self.pipeline.camera_bind_group_layout;
        self.camera_bind_group = pipeline::camera_bind_group(
            &self.device,
            layout,
            "Camera Bind Group",
            &self.camera_buffer,
            &self.lights_buffer,
            &self.tile_counts_buffer,
            &self.tile_indices_buffer,
            &self.env_regions_buffer,
            &self.room_gi,
        );
        self.light_camera_bind_group = pipeline::camera_bind_group(
            &self.device,
            layout,
            "Light Camera BG",
            &self.light_camera_buffer,
            &self.lights_buffer,
            &self.tile_counts_buffer,
            &self.tile_indices_buffer,
            &self.env_regions_buffer,
            &self.room_gi,
        );
        self.room_gi.rebind = false;
    }

    /// One frame of room GI, before the scene pass. `station_off` moves the
    /// home frame into render space (the same offset the scene objects and
    /// the lights get), `eye` is the render camera, and `sky_up` is the local
    /// up in the hull frame when this frame has a sky-view table (None aboard
    /// a station in orbit, where the glass lid looks out at space).
    pub fn update_room_gi(&mut self, station_off: Vec3, eye: Vec3, sky_up: Option<Vec3>) {
        let _cost = crate::frame_costs::stage("cpu.room_probes");
        let gi = &mut self.room_gi;
        let render_rooms: Vec<RoomBox> = gi.rooms.iter().map(|r| r.shifted(station_off)).collect();
        let (gmin, gmax) = render_rooms.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), r| {
            (a.min(r.min), b.max(r.max))
        });
        let on = !gi.off && !gi.rooms.is_empty();
        let near = render_rooms.iter().any(|r| r.contains(eye, UPDATE_REACH_M));
        // The camera's room: the smallest box holding the eye.
        let cam_room = render_rooms
            .iter()
            .enumerate()
            .filter(|(_, r)| r.contains(eye, 0.3))
            .min_by(|a, b| a.1.volume().total_cmp(&b.1.volume()))
            .map(|(i, _)| i);
        // PROBE STREAMING: which rooms hold probes this frame (room_probes::
        // Residency). Only while the probes are being updated at all; a room
        // that came in is seeded below, in this frame's pass.
        let step = if on && near { gi.residency.step(&render_rooms, &gi.probes, eye) } else { ResidencyStep::default() };
        gi.log_residency(&step);
        // The room table, in PICK ORDER (room_probes::pick_order): the header,
        // then every room with its light range. `slot_of[i]` is room i's row.
        let order = pick_order(&render_rooms, eye);
        let mut slot_of = vec![0u32; render_rooms.len()];
        for (slot, &i) in order.iter().enumerate() {
            slot_of[i] = slot as u32;
        }
        let owned = assign_lights(&render_rooms, &self.cur_lights);
        let mut packed_lights: Vec<[f32; 16]> = Vec::new();
        let mut table: Vec<GiRoomGpu> = Vec::with_capacity(render_rooms.len().max(1));
        for &i in &order {
            let first_light = packed_lights.len() as u32;
            packed_lights.extend(owned[i].iter().map(|&k| gpu_packed(&self.cur_lights[k])));
            // Rung 1 traces only each room's box, where DDGI's visibility test
            // is an identity, so no room asks for it unless the dev switch
            // forces it (room_probes::ROOM_FLAG_VISIBILITY).
            let vis = gi.force_visibility;
            let at = gi.residency.placement(i);
            table.push(pack_room(&render_rooms[i], i as u32, gi.counts[i], at, first_light, owned[i].len() as u32, vis));
        }
        // ── FLOORS BETWEEN STOREYS (ship homes increment 7, lane L) ──
        // After the rooms: every solid floor slab once (the sun's list, its
        // count in the header's atlas.w), then each scene light's RUN of the
        // slabs its range reaches, copied, which the light's record names as
        // (first, count) (light::gpu_packed_with_slabs). Render space: the
        // same station offset as the rooms here and as the lights, which lib.rs
        // offset by it before `set_point_lights` earlier this frame. Written
        // whether or not room GI is on: a floor is lighting, not bounce.
        let render_slabs: Vec<OccluderSlab> =
            if gi.slabs_off { Vec::new() } else { gi.slabs.iter().map(|s| s.shifted(station_off)).collect() };
        // The sun's list (`room_probes::layered_rows`): the slabs grouped into
        // layers behind a layer row each, so a fragment crosses each layer's
        // plane once and skips a layer its path misses (a low sun's), and
        // NEAREST THE EYE FIRST, because the walk stops at the first slab that
        // blocks and most of what is on screen stands under the nearest one.
        // Measured 2026-10-21 with the upper-floor layout's slabs, floors on
        // against off, same boot: in the merge order home-racks-noon read
        // gpu.scene +1.25 ms (the mushroom room's slab was the 12th of 13);
        // sorted, +0.0, while the dawn park console-face-6 still read +0.47
        // (13 crossings, none blocking) until the layers. The runs below are
        // copies, so any order serves them.
        let sun_list =
            if render_slabs.is_empty() || !gi.slabs_sun { Vec::new() } else { layered_rows(&render_slabs, eye) };
        table.extend(sun_list.iter().copied());
        // Per scene light: (first row, rows, the one layer's plane).
        let mut runs: Vec<(u32, u32, f32)> = Vec::new();
        if !render_slabs.is_empty() && gi.slabs_lamps {
            let mut first = 0usize;
            let mut capped = 0usize;
            for l in &self.cur_lights {
                let touched: Vec<OccluderSlab> = slabs_touched(l, &render_slabs).iter().map(|&k| render_slabs[k]).collect();
                // Layered like the sun's list, nearest the light first, and the
                // one layer's plane in the light's record (`run_plane`): a lamp
                // on the fragment's own storey then reads no row at all.
                let rows = if touched.is_empty() { Vec::new() } else { layered_rows(&touched, light_anchor(l)) };
                if first + rows.len() > MAX_SLAB_RUN_ROWS {
                    capped += 1;
                    runs.push((0, 0, 0.0));
                    continue;
                }
                runs.push((first as u32, rows.len() as u32, run_plane(&rows)));
                first += rows.len();
                table.extend(rows);
            }
            static CAPPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if capped > 0 && !CAPPED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                log::warn!(
                    "[RoomGI] {capped} lights name no floor slabs: their runs would pass {MAX_SLAB_RUN_ROWS} rows, so a floor does not stop them"
                );
            }
        }
        gi.last_slab_rows = sun_list.len() as u32;
        gi.last_run_rows = (table.len() - render_rooms.len() - sun_list.len()) as u32;
        if table.is_empty() {
            table.push(GiRoomGpu::default());
        }
        // The rows past the rooms can outgrow the table: grow it like
        // `relayout` does, and rebuild the camera bind groups before anything
        // draws with the old buffer.
        let grew = table.len() > gi.rooms_cap;
        if grew {
            let mut cap = gi.rooms_cap.max(1);
            while cap < table.len() {
                cap *= 2;
            }
            gi.rooms_cap = cap;
            gi.rooms_buf = rooms_buffer(&self.device, cap);
            gi.rebind = true;
            gi.bind_groups = None;
        }
        // The scene lights' runs: rewrite the buffer `set_point_lights` filled
        // when any light names one, or when one did last frame (so no stale run
        // survives). A home with no slabs never rewrites it.
        let rewrite_lights = !runs.is_empty() || gi.slab_runs_live;
        gi.slab_runs_live = runs.iter().any(|r| r.1 > 0);
        let header = GiHeaderGpu {
            gmin: [gmin.x, gmin.y, gmin.z, if on { 1.0 } else { 0.0 }],
            gmax: [gmax.x, gmax.y, gmax.z, 0.0],
            info: [render_rooms.len() as u32, gi.residency.resident_probes(), gi.layout.irr_per_row, gi.layout.depth_per_row],
            atlas: [gi.layout.width, gi.layout.height, gi.layout.depth_y0, sun_list.len() as u32],
        };
        if grew {
            self.rebuild_camera_bind_groups();
        }
        let gi = &mut self.room_gi;
        self.queue.write_buffer(&gi.rooms_buf, 0, bytemuck::bytes_of(&header));
        self.queue.write_buffer(&gi.rooms_buf, HEADER_BYTES, bytemuck::cast_slice(&table));
        if rewrite_lights && !self.cur_lights.is_empty() {
            let packed: Vec<[f32; 16]> = self
                .cur_lights
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let (first, count, plane) = runs.get(i).copied().unwrap_or((0, 0, 0.0));
                    gpu_packed_with_slabs(l, first, count, plane)
                })
                .collect();
            self.queue.write_buffer(&self.lights_buffer, 0, bytemuck::cast_slice(&packed));
        }
        if !on || !near {
            gi.last_updated = 0;
            gi.last_seeded = 0;
            return;
        }
        if packed_lights.is_empty() {
            packed_lights.push([0.0; 16]);
        }
        if packed_lights.len() > gi.lights_cap {
            while gi.lights_cap < packed_lights.len() {
                gi.lights_cap *= 2;
            }
            gi.lights_buf = storage_buffer(&self.device, "Room GI Lights", 64 * gi.lights_cap as u64);
            gi.bind_groups = None;
        }
        self.queue.write_buffer(&gi.lights_buf, 0, bytemuck::cast_slice(&packed_lights));
        // The list: this frame's traces, then every probe of each room that
        // came in (cs_seed reads its entries after the traced ones). The
        // shader indexes the table, so each entry carries its room's ROW.
        let mut list = gi.build_updates(cam_room);
        let traced = list.len() as u32;
        for &i in &step.entered {
            let at = gi.residency.placement(i);
            list.extend((at.first_probe..at.first_probe + gi.probes[i]).map(|p| [p, i as u32]));
        }
        let seeded = list.len() as u32 - traced;
        for u in &mut list {
            u[1] = slot_of[u[1] as usize];
        }
        gi.last_updated = traced;
        gi.last_seeded = seeded;
        if !list.is_empty() {
            self.queue.write_buffer(&gi.updates_buf, 0, bytemuck::cast_slice(&list));
        }
        let rot = ray_rotation(gi.frame);
        gi.frame = gi.frame.wrapping_add(1);
        let (sd, sc, si) = self.cur_sun;
        let sun_dir = Vec3::from(sd).normalize_or_zero();
        let up = sky_up.unwrap_or(Vec3::Y).normalize_or_zero();
        let params = GiParamsGpu {
            rot: [rot.row(0).extend(0.0).to_array(), rot.row(1).extend(0.0).to_array(), rot.row(2).extend(0.0).to_array()],
            sun_dir: [sun_dir.x, sun_dir.y, sun_dir.z, si],
            sun_color: [sc[0], sc[1], sc[2], if sky_up.is_some() && self.sky_view_uniform.is_some() { 1.0 } else { 0.0 }],
            sky_up: [up.x, up.y, up.z, SKY_LIGHTING_EXPOSURE],
            misc: [HYSTERESIS, 0.0, NORMAL_BIAS_M, 0.0],
            counts: [traced, gi.scratch_layout.irr_per_row, gi.scratch_layout.depth_per_row, gi.scratch_layout.depth_y0],
        };
        self.queue.write_buffer(&gi.params_buf, 0, bytemuck::bytes_of(&params));
        // The dev drop, once a second while frame costs are dumped.
        if crate::frame_costs::dump_enabled() && gi.dumped_at.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0) {
            gi.dumped_at = Some(std::time::Instant::now());
            let sun = Sun { dir: sun_dir, color: sc, intensity: si };
            let lights: Vec<Vec<super::light::RoomLight>> =
                owned.iter().map(|o| o.iter().map(|&k| self.cur_lights[k]).collect()).collect();
            write_dev_drop(gi, &render_rooms, &lights, sun, eye, cam_room);
        }
        if gi.rebind {
            self.rebuild_camera_bind_groups();
        }
        if self.room_gi.bind_groups.is_none() {
            let bgs = self.room_gi.make_bind_groups(&self.device, &self.sky_view.target_view);
            self.room_gi.bind_groups = Some(bgs);
        }
        let gi = &self.room_gi;
        let Some(bgs) = gi.bind_groups.as_ref() else {
            return;
        };
        let work = PassWork { rooms: render_rooms.len() as u32, traced, seeded };
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Room Probe Encoder"),
        });
        // A packed atlas moves its rooms' tiles first (`encode_moves`), so the
        // pass below reads and writes them where the table now says they are.
        gi.encode_moves(&mut enc, &step.moved);
        // The timer pair is claimed here, past every early return: a pair no
        // pass writes would be read back as a stale sample (particles_gpu's
        // note).
        gi.encode(&mut enc, bgs, work, self.compute_pass_timer("gpu.room_probes"));
        self.queue.submit(std::iter::once(enc.finish()));
    }
}

/// `debug/room_gi.json` (dev tooling, written once a second while
/// `HUMANITY_FRAME_COSTS=1` asks for the frame-cost drop): the level, the
/// atlas's memory, which rooms hold probes and each room's ambient colour as
/// the CPU twin works it out (`room_ambient`, no sky: aboard in orbit the GPU's
/// sky term is 0 too), so a rig can show the probes following the player
/// within the budget and that no room is left dark.
fn write_dev_drop(
    gi: &RoomGi,
    rooms: &[RoomBox],
    lights: &[Vec<super::light::RoomLight>],
    sun: Sun,
    eye: Vec3,
    cam_room: Option<usize>,
) {
    let per_room: Vec<serde_json::Value> = rooms
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let direct = DirectSource::Scene { lights: lights.get(i).cloned().unwrap_or_default(), sun: Some(sun) };
            let amb = room_ambient(r, &direct, |_| Vec3::ZERO);
            let at = gi.residency.placement(i);
            serde_json::json!({
                "index": i,
                "min": [r.min.x, r.min.y, r.min.z],
                "max": [r.max.x, r.max.y, r.max.z],
                "probes": gi.probes.get(i).copied().unwrap_or(0),
                "weight": at.weight,
                "first_probe": at.first_probe,
                "priority_m": super::room_probes::room_priority(r, eye),
                "lights": lights.get(i).map_or(0, |l| l.len()),
                "ambient_twin": [amb.x, amb.y, amb.z],
            })
        })
        .collect();
    let body = serde_json::json!({
        "budget_probes": gi.budget.max_probes,
        "spacing_m": gi.budget.spacing_m,
        "rooms": rooms.len(),
        "resident_rooms": gi.residency.resident_rooms(),
        "resident_probes": gi.residency.resident_probes(),
        "all_probes": gi.total,
        "atlas_bytes": gi.layout.texel_count() as u64 * 8,
        "scratch_bytes": gi.scratch_layout.texel_count() as u64 * 8,
        "vram_bytes": gi.vram_bytes(),
        "traced": gi.last_updated,
        "seeded": gi.last_seeded,
        "eye": [eye.x, eye.y, eye.z],
        "camera_room": cam_room,
        "per_room": per_room,
    });
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write("debug/room_gi.json", body.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed() -> wgpu::naga::Module {
        wgpu::naga::front::wgsl::parse_str(UPDATE_SHADER).expect("the room probe update shader parses")
    }

    /// The update shader (the shared sampling part plus the compute entries)
    /// parses and validates the way the device will, with its five entries.
    #[test]
    fn the_update_shader_validates() {
        let module = parsed();
        let mut v = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        v.validate(&module).expect("the room probe update shader validates");
        let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        for want in ["cs_trace", "cs_resolve", "cs_ambient", "cs_ambient_resolve", "cs_seed"] {
            assert!(entries.contains(&want), "{want} missing from {entries:?}");
        }
    }

    /// The WGSL records are exactly as large as the Rust ones that fill them:
    /// naga lays the structs out from the shader text, so a field added on one
    /// side only fails here instead of shifting every later field on the GPU.
    #[test]
    fn the_wgsl_records_match_the_rust_records() {
        let module = parsed();
        let size_of = |name: &str| {
            let (_, ty) = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some(name))
                .unwrap_or_else(|| panic!("struct {name} missing"));
            ty.inner.size(module.to_ctx())
        };
        assert_eq!(size_of("GiHeader") as u64, HEADER_BYTES);
        assert_eq!(size_of("GiRoom") as u64, ROOM_BYTES);
        assert_eq!(size_of("GiParams") as usize, std::mem::size_of::<GiParamsGpu>());
        assert_eq!(size_of("GiLight"), 64, "one light is the renderer's 64-byte GpuLight");
    }

    /// The WGSL divides probe indices by CONSTANT tiles-per-row (a multiply
    /// and a shift instead of an integer division per probe), so those
    /// constants must be what `AtlasLayout` lays the atlas out with, for the
    /// atlas and the scratch alike.
    #[test]
    fn the_shader_tile_rows_match_the_atlas_layout() {
        for probes in [1, 484, MAX_UPDATES_PER_FRAME, super::super::room_probes::MAX_TOTAL_PROBES] {
            let l = AtlasLayout::for_probes(probes);
            let irr = format!("const GI_IRR_PER_ROW: u32 = {}u;", l.irr_per_row);
            let depth = format!("const GI_DEPTH_PER_ROW: u32 = {}u;", l.depth_per_row);
            assert!(UPDATE_SHADER.contains(&irr), "85-room-gi.wgsl must declare {irr}");
            assert!(UPDATE_SHADER.contains(&depth), "85-room-gi.wgsl must declare {depth}");
        }
    }

    /// The floors' constants are the same number in the WGSL and the Rust twin
    /// (85-room-gi.wgsl is the head of this shader, so UPDATE_SHADER holds
    /// them): the edge tolerance, the sun's reach, and the many-layers
    /// sentinel the light record carries. Seen red first (2026-10-21) with
    /// SLAB_EDGE_TOL set to 0.02 in the WGSL: "left: 0.02, right: 0.01".
    #[test]
    fn the_floor_constants_match_the_cpu_twin() {
        use super::super::room_probes::{RUN_MANY_LAYERS, SLAB_EDGE_TOL_M, SLAB_SUN_REACH_M};
        assert_eq!(wgsl_f32("SLAB_EDGE_TOL"), SLAB_EDGE_TOL_M);
        assert_eq!(wgsl_f32("SLAB_SUN_REACH"), SLAB_SUN_REACH_M);
        assert_eq!(wgsl_f32("SLAB_RUN_MANY_LAYERS"), RUN_MANY_LAYERS);
        // And the layer occupancy grid's shape.
        use super::super::room_probes::{LAYER_GRID_X, LAYER_GRID_Z};
        assert!(UPDATE_SHADER.contains(&format!("const SLAB_GRID_X: u32 = {LAYER_GRID_X}u;")));
        assert!(UPDATE_SHADER.contains(&format!("const SLAB_GRID_Z: u32 = {LAYER_GRID_Z}u;")));
        assert!(LAYER_GRID_X * LAYER_GRID_Z <= 7 * 32, "the grid fits the layer row's seven spare lanes");
    }

    /// The value of `const NAME: f32 = ...;` in the update shader.
    fn wgsl_f32(name: &str) -> f32 {
        let decl = format!("const {name}: f32 = ");
        let at = UPDATE_SHADER.find(&decl).unwrap_or_else(|| panic!("the update shader declares {decl}"));
        let rest = &UPDATE_SHADER[at + decl.len()..];
        rest[..rest.find(';').expect("the declaration ends")].trim().parse().expect("a number")
    }

    /// The ambient colour's and the seed's constants are the twin's
    /// (room_probes_update.wgsl, LOCKSTEP with room_probes.rs). Seen red
    /// first (2026-10-21) with the WGSL seed count out of step with the twin: "assertion `left == right` failed".
    #[test]
    fn the_ambient_constants_match_the_cpu_twin() {
        use super::super::room_probes::{AMBIENT_CELLS, AMBIENT_GRID, AMBIENT_SKY_DIRS, SEED_COUNT};
        assert!(UPDATE_SHADER.contains(&format!("const GI_AMB_GRID: u32 = {AMBIENT_GRID}u;")));
        assert!(UPDATE_SHADER.contains(&format!("const GI_AMB_CELLS: u32 = {AMBIENT_CELLS}u;")));
        assert!(UPDATE_SHADER.contains(&format!("const GI_AMB_SKY_DIRS: u32 = {AMBIENT_SKY_DIRS}u;")));
        assert_eq!(wgsl_f32("GI_SEED_COUNT"), SEED_COUNT);
        // One cell of every face and one sky direction per thread of the
        // 64-wide workgroup.
        assert_eq!(AMBIENT_CELLS, 64);
        assert_eq!(AMBIENT_SKY_DIRS, 64);
        assert!(UPDATE_SHADER.contains("@compute @workgroup_size(64)\nfn cs_ambient(")
            || UPDATE_SHADER.contains("@compute @workgroup_size(64)\r\nfn cs_ambient("));
    }

    /// The scratch holds one update's worth of tiles, and fits the texture limit.
    #[test]
    fn the_scratch_holds_a_full_update() {
        let s = AtlasLayout::for_probes(MAX_UPDATES_PER_FRAME);
        assert!(s.probes >= MAX_UPDATES_PER_FRAME && s.height <= 8192);
        assert_eq!(scratch_slots(0), 0, "the Ambient level's scratch is its ambient row");
        assert_eq!(scratch_slots(8192), MAX_UPDATES_PER_FRAME);
    }

    // ── On a real device (ignored: CI has no adapter). Run them deliberately:
    //      node scripts/lib-tests.js room_probes_gpu::tests:: --ignored --nocapture

    /// A device, or None (then the test says so and passes).
    #[cfg(feature = "native")]
    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).ok()?;
        device.on_uncaptured_error(Box::new(|e| panic!("wgpu validation error: {e}")));
        Some((device, queue))
    }

    /// IEEE half float bits to f32 (the atlas is Rgba16Float).
    #[cfg(feature = "native")]
    fn f16_to_f32(bits: u16) -> f32 {
        let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
        let exp = ((bits >> 10) & 0x1f) as i32;
        let frac = (bits & 0x3ff) as f32;
        match exp {
            0 => sign * frac * 2f32.powi(-24),
            31 => if frac == 0.0 { sign * f32::INFINITY } else { f32::NAN },
            _ => sign * (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
        }
    }

    /// Read rows `y0..y0 + rows` of a texture back as RGBA f32 (the atlas is
    /// Rgba16Float).
    #[cfg(feature = "native")]
    fn read_rows(device: &wgpu::Device, queue: &wgpu::Queue, tex: &wgpu::Texture, y0: u32, rows: u32) -> Vec<[f32; 4]> {
        let width = tex.width();
        let row_bytes = (width * 8).div_ceil(256) * 256;
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("read back"),
            size: (row_bytes * rows) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: y0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row_bytes), rows_per_image: Some(rows) },
            },
            wgpu::Extent3d { width, height: rows, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));
        buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        device.poll(wgpu::Maintain::Wait);
        let data = buf.slice(..).get_mapped_range();
        let mut out = Vec::with_capacity((width * rows) as usize);
        for r in 0..rows {
            for x in 0..width {
                let at = (r * row_bytes + x * 8) as usize;
                let h = |k: usize| f16_to_f32(u16::from_le_bytes([data[at + 2 * k], data[at + 2 * k + 1]]));
                out.push([h(0), h(1), h(2), h(3)]);
            }
        }
        out
    }

    /// The device half of a level change: `set_budget` remakes the atlas at
    /// the new level's size (smaller for Low; the ambient row alone for the
    /// Ambient level), flags the camera bind groups for the rebind, and the
    /// same level again does nothing.
    #[cfg(feature = "native")]
    #[test]
    #[ignore = "needs a GPU adapter; run explicitly (see above)"]
    fn a_level_change_remakes_the_atlas_on_a_real_device() {
        use super::super::bounced_light_detail::detail_level;
        let Some((device, _queue)) = device() else {
            println!("no GPU adapter; skipping");
            return;
        };
        let mut gi = RoomGi::new(&device);
        let high = detail_level("high").budget();
        gi.set_budget(&device, high);
        gi.set_rooms(&device, super::super::bounced_light_detail::tests::ship());
        let at_high = gi.layout.probes;
        println!("High: {} probes in all, atlas for {at_high}", gi.total);
        assert_eq!(at_high, high.max_probes, "the atlas is the level's budget");
        gi.rebind = false;
        let low = detail_level("low").budget();
        assert!(gi.set_budget(&device, low), "a new level is a change");
        println!("Low: atlas for {}", gi.layout.probes);
        assert!(gi.layout.probes < at_high && gi.layout.probes == low.max_probes, "the atlas gave its memory back");
        assert!(gi.rebind && gi.bind_groups.is_none(), "the rebind is flagged");
        gi.rebind = false;
        assert!(!gi.set_budget(&device, low), "the same level again is not");
        assert!(!gi.rebind);
        let ambient = detail_level("ambient").budget();
        assert!(gi.set_budget(&device, ambient));
        assert_eq!((gi.layout.height, gi.layout.probes), (1, 0), "the Ambient level's atlas is its ambient row");
        assert_eq!(gi.scratch_layout.height, 1);
        println!("Ambient: room GI holds {} bytes of video memory", gi.vram_bytes());
    }

    /// THE GPU'S AMBIENT COLOUR, SEED AND PACKING ARE THE TWIN'S. Two rooms (a
    /// lamp in a 40 x 4 x 40 m hall, the sun through the glass lid of the
    /// other) through the real `cs_ambient`, `cs_ambient_resolve` and `cs_seed`
    /// on the device: the atlas's ambient row reads what `room_ambient` works
    /// out (to the half floats it is stored in), and a seeded probe's tiles
    /// read that colour with `SEED_COUNT` as its count and the box distance as
    /// its depth. Then the hall, seeded at slot 100, is packed down to slot 0
    /// (`encode_moves`: its 8,000 probes through the 4,096-probe scratch in two
    /// chunks, each landing on probes the move has read), and every probe reads at its new slot what it
    /// held at its old one. Seen red first (2026-10-21) with the WGSL seed count out of
    /// step with the twin (4 against 8): "seeded irradiance texel (0, 0): [0.0074043274,
    /// 0.005783081, 0.0043907166, 4.0] against [0.007406088, 0.0057834745, 0.0043931664]".
    #[cfg(feature = "native")]
    #[test]
    #[ignore = "needs a GPU adapter; run explicitly (see above)"]
    fn the_gpu_ambient_colour_seed_and_packing_are_the_twin_s() {
        use super::super::light::RoomLight;
        use super::super::room_probes::{
            probe_grid, probe_position, room_ambient, texel_direction, trace_box, Placement, TileMove, DEPTH_N, IRR_N,
            NOT_RESIDENT, SEED_COUNT,
        };
        let Some((device, queue)) = device() else {
            println!("no GPU adapter; skipping");
            return;
        };
        let mut gi = RoomGi::new(&device);
        gi.set_budget(&device, ProbeBudget::default());
        let mut hall = RoomBox { min: Vec3::ZERO, max: Vec3::new(40.0, 4.0, 40.0), refl: [[0.5; 3]; 6], lid_transmittance: 0.0 };
        hall.refl[2] = [0.3, 0.25, 0.2];
        let mut glass_room = RoomBox { min: Vec3::new(48.0, 0.0, 0.0), max: Vec3::new(58.0, 3.0, 10.0), ..hall.clone() };
        glass_room.lid_transmittance = 0.65;
        glass_room.refl[3] = [0.1; 3];
        gi.set_rooms(&device, vec![hall.clone(), glass_room.clone()]);
        let n = gi.probes[0];
        println!("the hall holds {n} probes; the scratch {}", gi.scratch_layout.probes);
        assert!(n > gi.scratch_layout.probes, "the move must take more than one chunk");
        let lamp = RoomLight::point(Vec3::new(10.0, 3.7, 10.0), [1.0, 0.9, 0.8], 30.0, 30.0);
        let sun = Sun { dir: Vec3::new(0.2, 1.0, 0.1).normalize(), color: [1.0, 0.95, 0.9], intensity: 3.0 };
        let first = 100u32;
        let header = GiHeaderGpu {
            gmin: [0.0, 0.0, 0.0, 1.0],
            gmax: [58.0, 4.0, 40.0, 0.0],
            info: [2, n, gi.layout.irr_per_row, gi.layout.depth_per_row],
            atlas: [gi.layout.width, gi.layout.height, gi.layout.depth_y0, 0],
        };
        let table = [
            pack_room(&hall, 0, gi.counts[0], Placement { first_probe: first, weight: 1.0 }, 0, 1, false),
            pack_room(&glass_room, 1, gi.counts[1], NOT_RESIDENT, 1, 0, false),
        ];
        queue.write_buffer(&gi.rooms_buf, 0, bytemuck::bytes_of(&header));
        queue.write_buffer(&gi.rooms_buf, HEADER_BYTES, bytemuck::cast_slice(&table));
        queue.write_buffer(&gi.lights_buf, 0, bytemuck::cast_slice(&[gpu_packed(&lamp)]));
        let seeds: Vec<[u32; 2]> = (first..first + n).map(|p| [p, 0]).collect();
        queue.write_buffer(&gi.updates_buf, 0, bytemuck::cast_slice(&seeds));
        let params = GiParamsGpu {
            rot: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
            sun_dir: [sun.dir.x, sun.dir.y, sun.dir.z, sun.intensity],
            sun_color: [sun.color[0], sun.color[1], sun.color[2], 0.0],
            sky_up: [0.0, 1.0, 0.0, SKY_LIGHTING_EXPOSURE],
            misc: [HYSTERESIS, 0.0, NORMAL_BIAS_M, 0.0],
            counts: [0, gi.scratch_layout.irr_per_row, gi.scratch_layout.depth_per_row, gi.scratch_layout.depth_y0],
        };
        queue.write_buffer(&gi.params_buf, 0, bytemuck::bytes_of(&params));
        let sky = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("no sky"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let bgs = gi.make_bind_groups(&device, &sky.create_view(&Default::default()));
        let mut enc = device.create_command_encoder(&Default::default());
        gi.encode(&mut enc, &bgs, PassWork { rooms: 2, traced: 0, seeded: n }, None);
        queue.submit(std::iter::once(enc.finish()));
        let row = read_rows(&device, &queue, &gi._atlas, gi.layout.amb_y0, 1);
        let twin = [
            room_ambient(&hall, &DirectSource::Scene { lights: vec![lamp], sun: Some(sun) }, |_| Vec3::ZERO),
            room_ambient(&glass_room, &DirectSource::Scene { lights: vec![], sun: Some(sun) }, |_| Vec3::ZERO),
        ];
        for (i, want) in twin.iter().enumerate() {
            let got = Vec3::new(row[i][0], row[i][1], row[i][2]);
            println!("room {i}: GPU {got}, twin {want}");
            assert!(want.min_element() > 1.0e-3, "room {i} is lit in the twin");
            let rel = ((got - *want).abs() / *want).max_element();
            assert!(rel < 4.0e-3, "room {i}: the GPU's ambient colour {got} against the twin's {want} ({:.2}%)", rel * 100.0);
        }
        let w = gi.layout.width as usize;
        let all = read_rows(&device, &queue, &gi._atlas, 0, gi.layout.amb_y0);
        let at = |all: &[[f32; 4]], x: u32, y: u32| all[y as usize * w + x as usize];
        // A seeded probe: the hall's sixth.
        let local = 5u32;
        let io = gi.layout.irr_origin(first + local);
        for (bx, by) in [(0, 0), (1, 1), (4, 7), (IRR_N + 1, IRR_N + 1)] {
            let t = at(&all, io.0 + bx, io.1 + by);
            let rel = ((Vec3::new(t[0], t[1], t[2]) - twin[0]).abs() / twin[0]).max_element();
            assert!(rel < 4.0e-3 && t[3] == SEED_COUNT, "seeded irradiance texel ({bx}, {by}): {t:?} against {}", twin[0]);
        }
        let c = gi.counts[0];
        let p = probe_position(&hall, c, probe_grid(c, local));
        let dg = gi.layout.depth_origin(first + local);
        for (x, y) in [(0u32, 0u32), (7, 3), (DEPTH_N - 1, DEPTH_N - 1)] {
            let (t, _) = trace_box(p, texel_direction(DEPTH_N, x, y), hall.min, hall.max);
            let got = at(&all, dg.0 + 1 + x, dg.1 + 1 + y);
            assert!((got[0] - t).abs() < 0.01 * t.max(1.0) && (got[1] - t * t).abs() < 0.02 * (t * t).max(1.0), "depth texel ({x}, {y}): {got:?} against {t}");
        }
        // Mark a few probes so the move is seen to carry each one's own tiles,
        // not just the colour every seeded probe shares: rewrite their first
        // irradiance texel through a tiny upload.
        let marks: Vec<u32> = vec![0, 1, gi.scratch_layout.probes - 1, gi.scratch_layout.probes, n - 1];
        for (k, &m) in marks.iter().enumerate() {
            let o = gi.layout.irr_origin(first + m);
            let v = [half_bits(10.0 + k as f32), half_bits(0.0), half_bits(0.0), half_bits(1.0)];
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &gi._atlas, mip_level: 0, origin: wgpu::Origin3d { x: o.0 + 1, y: o.1 + 1, z: 0 }, aspect: wgpu::TextureAspect::All },
                bytemuck::cast_slice(&v),
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(8), rows_per_image: Some(1) },
                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            );
        }
        let before = read_rows(&device, &queue, &gi._atlas, 0, gi.layout.amb_y0);
        let mut enc = device.create_command_encoder(&Default::default());
        gi.encode_moves(&mut enc, &[TileMove { room: 0, from: first, to: 0, count: n }]);
        queue.submit(std::iter::once(enc.finish()));
        let after = read_rows(&device, &queue, &gi._atlas, 0, gi.layout.amb_y0);
        for (k, &m) in marks.iter().enumerate() {
            let o = gi.layout.irr_origin(m);
            let got = at(&after, o.0 + 1, o.1 + 1);
            assert_eq!(got[0], 10.0 + k as f32, "probe {m}'s own mark moved with it: {got:?}");
        }
        let mut worst = 0usize;
        for m in (0..n).step_by(37).chain([n - 1]) {
            for (orig, tile) in [(gi.layout.irr_origin(m), super::super::room_probes::IRR_TILE), (gi.layout.depth_origin(m), super::super::room_probes::DEPTH_TILE)] {
                let src = if tile == super::super::room_probes::IRR_TILE { gi.layout.irr_origin(first + m) } else { gi.layout.depth_origin(first + m) };
                for y in 0..tile {
                    for x in 0..tile {
                        if at(&after, orig.0 + x, orig.1 + y) != at(&before, src.0 + x, src.1 + y) {
                            worst += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(worst, 0, "{worst} texels differ between a probe's old slot and its new one");
    }

    /// f32 to IEEE half float bits (round to nearest, the values here are exact).
    #[cfg(feature = "native")]
    fn half_bits(v: f32) -> u16 {
        let b = v.to_bits();
        let sign = ((b >> 16) & 0x8000) as u16;
        let exp = ((b >> 23) & 0xff) as i32 - 127 + 15;
        if v == 0.0 {
            return sign;
        }
        let mant = ((b >> 13) & 0x3ff) as u16;
        sign | ((exp as u16) << 10) | mant
    }
}
