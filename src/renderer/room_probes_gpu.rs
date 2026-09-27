//! ROOM GI, RUNG 1, the GPU half (docs/design/room-gi.md; the what and the
//! why are in `room_probes.rs`, whose CPU twin this mirrors).
//!
//! WHAT LIVES HERE. The probe atlas (one Rgba16Float texture: every probe's
//! 8x8 octahedral irradiance tile on top, its 16x16 depth-moment tile below,
//! each with the 1-texel border), the room table the fragments and the update
//! both read, and the per-frame update: a compute pass that traces 64 rays per
//! probe against its room's box (`cs_trace`) and copies the finished tiles
//! into the atlas (`cs_resolve`). `Renderer::update_room_gi` runs it once a
//! frame before the scene pass; `Renderer::set_room_gi_rooms` hands it the
//! ship's rooms whenever the home is built or edited.
//!
//! BINDINGS. The fragments read the atlas, its sampler and the room table
//! through the SHARED camera bind group, group 0 bindings 5 to 7 (see
//! `pipeline::camera_bind_group`). Replacing any of the three (the atlas when
//! the ship outgrows it, the table when the room count doubles) makes every
//! camera bind group stale, so `rebuild_camera_bind_groups` rebuilds both of
//! them with every binding: missing one is the v0.1029 failure, where world
//! entry panics while a menu-only boot stays green.
//!
//! BUDGET. At most `MAX_UPDATES_PER_FRAME` probes are traced per frame: the
//! room the camera stands in first (every frame, so the light you are looking
//! at answers immediately), then the rest of the ship round-robin. The default
//! acre holds about 19k probes, so a far room is refreshed every few frames.

use glam::Vec3;

use super::light::gpu_packed;
use super::room_probes::{
    assign_lights, lattice_for, pack_room, pick_order, ray_rotation, AtlasLayout, GiHeaderGpu, GiParamsGpu, GiRoomGpu, RoomBox,
    HEADER_BYTES, HYSTERESIS, NORMAL_BIAS_M, ROOM_BYTES, SKY_LIGHTING_EXPOSURE,
};
use super::{pipeline, Renderer};

/// Probes the update refreshes per frame, at most.
pub const MAX_UPDATES_PER_FRAME: u32 = 4096;
/// Past this distance from the nearest room the update stops (the probes keep
/// what they have): nobody on a planet's surface needs the station's rooms
/// relit every frame.
pub const UPDATE_REACH_M: f32 = 60.0;

/// The update shader: the sampling part the megashader also compiles, then
/// the two compute entries. One source of truth for how probes are read.
pub const UPDATE_SHADER: &str = concat!(
    include_str!("../../assets/shaders/pbr/85-room-gi.wgsl"),
    include_str!("../../assets/shaders/room_probes_update.wgsl"),
);

const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

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
    /// frame), their lattices and where each room's probes start.
    rooms: Vec<RoomBox>,
    counts: Vec<[u32; 3]>,
    first: Vec<u32>,
    total: u32,
    /// Probe spacing actually used (1 m unless the ship outgrew the cap).
    pub spacing: f32,
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
    clear: wgpu::ComputePipeline,
    /// Round-robin position over every probe of the ship.
    cursor: u32,
    /// Where the window through a camera room bigger than the budget stands.
    cam_cursor: u32,
    /// Updates run so far: seeds each update's ray rotation.
    frame: u32,
    /// The rooms changed: zero the atlas before the next trace (every probe
    /// back to "never updated", so none carries a light that belonged to a
    /// probe the old layout put in the same tile).
    needs_clear: bool,
    /// The atlas or the room table was replaced; the camera bind groups must
    /// be rebuilt before the next draw.
    pub(super) rebind: bool,
    /// Probes traced in the last update (the F2 overlay and the log read it).
    pub last_updated: u32,
    /// The update's bind groups (`make_bind_groups`); None = build them.
    bind_groups: Option<(wgpu::BindGroup, wgpu::BindGroup)>,
}

fn atlas_texture(device: &wgpu::Device, layout: &AtlasLayout, label: &str) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: layout.width, height: layout.height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: ATLAS_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
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
        let clear = make(&resolve_bgl, "cs_clear");
        let layout = AtlasLayout::for_probes(1);
        let (atlas, atlas_view) = atlas_texture(device, &layout, "Room GI Atlas");
        let scratch_layout = AtlasLayout::for_probes(MAX_UPDATES_PER_FRAME);
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
            first: Vec::new(),
            total: 0,
            spacing: super::room_probes::PROBE_SPACING_M,
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
            updates_buf: storage_buffer(device, "Room GI Updates", 8 * MAX_UPDATES_PER_FRAME as u64),
            params_buf,
            trace_bgl,
            resolve_bgl,
            trace,
            resolve,
            clear,
            cursor: 0,
            cam_cursor: 0,
            frame: 0,
            needs_clear: false,
            rebind: false,
            last_updated: 0,
            bind_groups: None,
        }
    }

    /// Probes in the ship.
    pub fn probe_count(&self) -> u32 {
        self.total
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

    /// New rooms (home frame). Recomputes the lattice, grows the atlas and the
    /// room table if they no longer fit (flagging `rebind`), and schedules the
    /// atlas clear. Unchanged rooms are a no-op, which matters because the
    /// construction editor rebuilds the home on every drag frame.
    fn set_rooms(&mut self, device: &wgpu::Device, rooms: Vec<RoomBox>) {
        if rooms == self.rooms {
            return;
        }
        let (spacing, counts) = lattice_for(&rooms);
        let mut first = Vec::with_capacity(rooms.len());
        let mut total = 0u32;
        for c in &counts {
            first.push(total);
            total += c[0] * c[1] * c[2];
        }
        if total > self.layout.probes {
            // A quarter of headroom, so dragging a wall does not reallocate
            // the atlas every frame.
            let cap = (total + total / 4).min(super::room_probes::MAX_TOTAL_PROBES).max(total);
            self.layout = AtlasLayout::for_probes(cap);
            let (tex, view) = atlas_texture(device, &self.layout, "Room GI Atlas");
            self._atlas = tex;
            self.atlas_view = view;
            self.rebind = true;
            self.bind_groups = None;
            log::info!(
                "[RoomGI] atlas {}x{} for {} probes ({:.1} MB)",
                self.layout.width,
                self.layout.height,
                cap,
                self.layout.texel_count() as f64 * 8.0 / 1.048576e6
            );
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
        log::info!(
            "[RoomGI] {} rooms, {} probes at {:.2} m spacing",
            rooms.len(),
            total,
            spacing
        );
        self.rooms = rooms;
        self.counts = counts;
        self.first = first;
        self.total = total;
        self.spacing = spacing;
        self.cursor = 0;
        self.cam_cursor = 0;
        self.needs_clear = true;
    }

    /// The update's two bind groups. Cached (`bind_groups`) and rebuilt only
    /// when a resource they hold is replaced: the atlas, the room table or
    /// the light list.
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

    /// Which room a global probe index belongs to.
    fn room_of(&self, probe: u32) -> u32 {
        (self.first.partition_point(|&f| f <= probe) - 1) as u32
    }

    /// This frame's update list: the camera's room first (all of it, or for a
    /// room bigger than the budget three quarters of the budget, walking
    /// through the room frame by frame), then the rest of the ship round-robin.
    fn build_updates(&mut self, cam_room: Option<usize>) -> Vec<[u32; 2]> {
        let budget = MAX_UPDATES_PER_FRAME.min(self.total) as usize;
        let mut list: Vec<[u32; 2]> = Vec::with_capacity(budget);
        let mut skip = 0..0u32;
        if let Some(ci) = cam_room {
            let c = self.counts[ci];
            let n = c[0] * c[1] * c[2];
            let f = self.first[ci];
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
        let mut guard = 0u32;
        while list.len() < budget && guard < self.total {
            let p = self.cursor;
            self.cursor = (self.cursor + 1) % self.total.max(1);
            guard += 1;
            if !skip.contains(&p) {
                list.push([p, self.room_of(p)]);
            }
        }
        list
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
        let _cost = super::frame_costs::stage("cpu.room_probes");
        let gi = &mut self.room_gi;
        let render_rooms: Vec<RoomBox> = gi.rooms.iter().map(|r| r.shifted(station_off)).collect();
        let (gmin, gmax) = render_rooms.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), r| {
            (a.min(r.min), b.max(r.max))
        });
        let on = !gi.off && gi.total > 0;
        // The camera's room: the smallest box holding the eye.
        let cam_room = render_rooms
            .iter()
            .enumerate()
            .filter(|(_, r)| r.contains(eye, 0.3))
            .min_by(|a, b| a.1.volume().total_cmp(&b.1.volume()))
            .map(|(i, _)| i);
        // The room table, in PICK ORDER (room_probes::pick_order): the header,
        // then every room with its light range. `slot_of[i]` is room i's row.
        let order = pick_order(&render_rooms, cam_room);
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
            table.push(pack_room(&render_rooms[i], gi.counts[i], gi.first[i], first_light, owned[i].len() as u32, vis));
        }
        if table.is_empty() {
            table.push(GiRoomGpu::default());
        }
        let header = GiHeaderGpu {
            gmin: [gmin.x, gmin.y, gmin.z, if on { 1.0 } else { 0.0 }],
            gmax: [gmax.x, gmax.y, gmax.z, 0.0],
            info: [render_rooms.len() as u32, gi.total, gi.layout.irr_per_row, gi.layout.depth_per_row],
            atlas: [gi.layout.width, gi.layout.height, gi.layout.depth_y0, 0],
        };
        self.queue.write_buffer(&gi.rooms_buf, 0, bytemuck::bytes_of(&header));
        self.queue.write_buffer(&gi.rooms_buf, HEADER_BYTES, bytemuck::cast_slice(&table));
        let near = render_rooms.iter().any(|r| r.contains(eye, UPDATE_REACH_M));
        if !on || !near {
            gi.last_updated = 0;
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
        let mut updates = gi.build_updates(cam_room);
        // The shader indexes the table, so each entry carries its room's ROW.
        for u in &mut updates {
            u[1] = slot_of[u[1] as usize];
        }
        let n = updates.len() as u32;
        gi.last_updated = n;
        if n == 0 {
            return;
        }
        self.queue.write_buffer(&gi.updates_buf, 0, bytemuck::cast_slice(&updates));
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
            counts: [n, gi.scratch_layout.irr_per_row, gi.scratch_layout.depth_per_row, gi.scratch_layout.depth_y0],
        };
        self.queue.write_buffer(&gi.params_buf, 0, bytemuck::bytes_of(&params));
        let clear = std::mem::take(&mut gi.needs_clear);
        if gi.rebind {
            self.rebuild_camera_bind_groups();
        }
        if self.room_gi.bind_groups.is_none() {
            let bgs = self.room_gi.make_bind_groups(&self.device, &self.sky_view.target_view);
            self.room_gi.bind_groups = Some(bgs);
        }
        let gi = &self.room_gi;
        let Some((trace_bg, resolve_bg)) = gi.bind_groups.as_ref() else {
            return;
        };
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Room Probe Encoder"),
        });
        {
            // Claimed here, past every early return: a timer pair no pass
            // writes would be read back as a stale sample (particles_gpu's note).
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Room Probe Update"),
                timestamp_writes: self.compute_pass_timer("gpu.room_probes"),
            });
            if clear {
                pass.set_pipeline(&gi.clear);
                pass.set_bind_group(0, resolve_bg, &[]);
                pass.dispatch_workgroups(gi.layout.width.div_ceil(8), gi.layout.height.div_ceil(8), 1);
            }
            pass.set_pipeline(&gi.trace);
            pass.set_bind_group(0, trace_bg, &[]);
            pass.dispatch_workgroups(n, 1, 1);
            pass.set_pipeline(&gi.resolve);
            pass.set_bind_group(0, resolve_bg, &[]);
            pass.dispatch_workgroups(n, 1, 1);
        }
        self.queue.submit(std::iter::once(enc.finish()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed() -> wgpu::naga::Module {
        wgpu::naga::front::wgsl::parse_str(UPDATE_SHADER).expect("the room probe update shader parses")
    }

    /// The update shader (the shared sampling part plus the compute entries)
    /// parses and validates the way the device will, with its three entries.
    #[test]
    fn the_update_shader_validates() {
        let module = parsed();
        let mut v = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        v.validate(&module).expect("the room probe update shader validates");
        let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        for want in ["cs_trace", "cs_resolve", "cs_clear"] {
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

    /// The scratch holds one update's worth of tiles, and fits the texture limit.
    #[test]
    fn the_scratch_holds_a_full_update() {
        let s = AtlasLayout::for_probes(MAX_UPDATES_PER_FRAME);
        assert!(s.probes >= MAX_UPDATES_PER_FRAME && s.height <= 8192);
    }
}
