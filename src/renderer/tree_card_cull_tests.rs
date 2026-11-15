//! The tree card cull's tests (renderer/tree_card_cull.rs).

use super::*;
use humanity_terrain::planet_chunks::PatchSlot;
use humanity_terrain::planet_surface::pack_color_to_uv_flags;

fn vert(uv: [f32; 2], p: [f32; 3]) -> Vertex {
    Vertex { position: p, normal: [0.0, 1.0, 0.0], uv }
}

/// A sprite card's uv (tile 0, u 0.5): what `Mesh::planet_surface_vertices` hands the arena.
const SPRITE_UV: [f32; 2] = [-1.25, 0.5];

/// A patch mesh: `g` ground vertices in a fan, then `quads` sprite cards whose corners sit around
/// `centre(q)` (a 6 m by 10 m card standing on it).
fn patch_with(g: u32, quads: u32, centre: impl Fn(u32) -> Vec3) -> (Vec<Vertex>, Vec<u32>) {
    let mut vs: Vec<Vertex> = (0..g).map(|i| vert([1000.0, 0.5], [i as f32, 0.0, 0.0])).collect();
    let mut is = Vec::new();
    for i in 1..g.saturating_sub(1) {
        is.extend([0, i, i + 1]);
    }
    for q in 0..quads {
        let c = centre(q);
        let base = vs.len() as u32;
        for (dx, dy) in [(-3.0, -5.0), (3.0, -5.0), (3.0, 5.0), (-3.0, 5.0)] {
            vs.push(vert(SPRITE_UV, (c + Vec3::new(dx, dy, 0.0)).to_array()));
        }
        is.extend(QUAD_INDICES.map(|p| base + p));
    }
    (vs, is)
}

fn patch(g: u32, quads: u32) -> (Vec<Vertex>, Vec<u32>) {
    patch_with(g, quads, |q| Vec3::new(q as f32 * 10.0, 0.0, 0.0))
}

/// THE TAIL IS FOUND ONLY WHEN IT IS EXACTLY THE CARDS: the cull draws the ground's range without
/// the tail, so a tail read wrong would hide ground (or draw cards twice).
///
/// Seen red 2026-11-14 on the patch builder as it was (the skirt emitted after the trees): the
/// planet_chunks half of this pair, `vegetation_bakes_into_deep_land_patches_deterministically`,
/// read "the cards are not the mesh's tail: 0 trailing card vertices for 281 trees"; the skirted
/// case below is that mesh's shape.
#[test]
fn the_card_tail_is_found_only_when_it_is_exactly_the_cards() {
    let (vs, is) = patch(10, 3);
    assert_eq!(card_tail_quads(&vs, &is), 3);
    // A coloured card (a planet without the sprite atlas) carries the flag bit instead.
    let mut coloured = vs.clone();
    for v in &mut coloured[10..] {
        v.uv = pack_color_to_uv_flags([0.1, 0.3, 0.1], false, true);
    }
    assert_eq!(card_tail_quads(&coloured, &is), 3, "a coloured card's flag bit marks a card");
    // The skirt after the cards (the patch builder before 2026-11-14): no tail.
    let (mut skirted, mut sis) = (vs.clone(), is.clone());
    let b = skirted.len() as u32;
    skirted.extend((0..3).map(|i| vert([1000.0, 0.5], [i as f32, -5.0, 0.0])));
    sis.extend([b, b + 1, b + 2]);
    assert_eq!(card_tail_quads(&skirted, &sis), 0, "a skirt after the cards was read as a card tail");
    // A ground triangle reaching into the cards: the cull would hide it.
    let mut reach = is.clone();
    reach[0] = 11;
    assert_eq!(card_tail_quads(&vs, &reach), 0, "a ground index into the card tail passed");
    // A card wound some other way: not the builder's twelve.
    let mut wrong = is.clone();
    let n = wrong.len();
    wrong.swap(n - 1, n - 2);
    assert_eq!(card_tail_quads(&vs, &wrong), 0, "a card wound differently passed");
    // No cards, and a tail that is not whole quads.
    let (g, gi) = patch(10, 0);
    assert_eq!(card_tail_quads(&g, &gi), 0);
    assert_eq!(card_tail_quads(&vs[1..], &is), 0, "three trailing card vertices short of a quad passed");
}

/// The record bounds the quad: its centre is the corners' mean and its radius reaches every corner.
#[test]
fn a_quad_record_bounds_its_corners() {
    let r = quad_record([[0.0, 0.0, 0.0], [6.0, 0.0, 0.0], [6.0, 10.0, 0.0], [0.0, 10.0, 0.0]]);
    assert_eq!(&r[..3], &[3.0, 5.0, 0.0]);
    assert!((r[3] - 34f32.sqrt()).abs() < 1.0e-5, "radius {}", r[3]);
}

fn slot(vstart: u32, vcount: u32, istart: u32, icount: u32) -> PatchSlot {
    PatchSlot { vstart, vcount, istart, icount }
}

/// THE PLAN: a patch with a card tail becomes a job whose ground draw stops short of its cards, with
/// its card draw RIGHT AFTER its ground in the main list (the old draw order) and its compact
/// regions after the previous job's; a patch without one, a patch whose cards no longer fit the
/// reserved range, or any patch with the cull off, draws whole.
#[test]
fn the_plan_draws_the_ground_short_of_the_cards_and_packs_the_regions() {
    let draws = vec![
        PatchDraw { slot: slot(0, 30, 0, 60), position: Vec3::new(1.0, 2.0, 3.0), fade: 0.0 },
        PatchDraw { slot: slot(30, 10, 60, 24), position: Vec3::ZERO, fade: 0.0 },
        PatchDraw { slot: slot(40, 26, 84, 48), position: Vec3::new(-4.0, 0.0, 0.0), fade: 0.0 },
    ];
    let tail = |v: u32| match v {
        0 => Some(CardTail { quads: 3, rec: 100 }),
        40 => Some(CardTail { quads: 2, rec: 7 }),
        _ => None,
    };
    let p = plan(&draws, true, (1000, 1_000_000), tail);
    assert_eq!(p.ground, vec![60 - 36, 24, 48 - 24]);
    assert_eq!(p.jobs.len(), 2);
    assert_eq!((p.jobs[0].rec, p.jobs[0].quads, p.jobs[0].v0, p.jobs[0].dst, p.jobs[0].sdst), (100, 3, 30 - 12, 0, 0));
    assert_eq!((p.jobs[1].rec, p.jobs[1].quads, p.jobs[1].v0, p.jobs[1].dst, p.jobs[1].sdst), (7, 2, 40 + 26 - 8, 36, 18));
    assert_eq!(p.jobs[0].pos, [1.0, 2.0, 3.0, 0.0]);
    let a = |x: &IndirectArgs| (x.index_count, x.instance_count, x.first_index, x.base_vertex, x.first_instance);
    let main: Vec<_> = p.main_args.iter().map(a).collect();
    assert_eq!(main, vec![(24, 1, 0, 0, 0), (0, 1, 1000, 0, 0), (24, 1, 60, 30, 1), (24, 1, 84, 40, 2), (0, 1, 1036, 0, 2)], "ground, then that patch's cards in the window");
    assert_eq!((p.jobs[0].arg, p.jobs[1].arg), (1, 4));
    assert_eq!((p.sun_args[1].first_index, p.sun_args[1].first_instance), (18, 2));
    assert_eq!((p.main_indices, p.sun_indices), (60, 30));
    let full = plan(&draws, true, (1000, 40), tail);
    assert_eq!((full.jobs.len(), full.ground[2], full.main_args.len()), (1, 48, 4), "a patch past the reserved range draws whole");
    let off = plan(&draws, false, (1000, 1_000_000), tail);
    assert!(off.jobs.is_empty());
    assert_eq!(off.ground, vec![60, 24, 48], "with the cull off every patch draws whole");
}

/// THE SHOWCASE PIN: "0" turns the cull off (the red twin), anything else back on.
#[test]
fn the_showcase_pin_turns_the_cull_off_and_on() {
    set_pinned_on(false);
    assert!(!pinned_on());
    set_pinned_on(true);
    assert!(pinned_on());
}

#[test]
fn the_cull_shader_validates() {
    let module = wgpu::naga::front::wgsl::parse_str(CULL_WGSL).expect("the card cull shader parses");
    let mut v = wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all());
    v.validate(&module).expect("the card cull shader validates");
    assert!(module.entry_points.iter().any(|e| e.name == "cull_cards"));
    assert_eq!(std::mem::size_of::<CullJob>(), 48, "the shader's Job is 48 bytes");
    assert!(CULL_WGSL.contains("const WORKGROUP: u32 = 64u;") && CULL_WGSL.contains("@workgroup_size(64)"), "the quad loop's stride is the workgroup size");
    assert!(CULL_WGSL.contains(&format!("const JITTER_MARGIN: f32 = {JITTER_MARGIN};")), "the jitter margin drifted from its Rust twin");
}

// The GPU part needs the native build's test helpers (humanity_core::test_gpu, pollster).
#[cfg(feature = "native")]
fn device() -> Option<crate::renderer::test_gpu::Gpu> {
    // The one way a test gets a device (BUG-308): the shared DX12 instance, the GPU turn, and the
    // adapter's resolution limits as this test asked for them before.
    crate::renderer::test_gpu::open(crate::renderer::test_gpu::Ask::new("card cull test").adapter_resolution())
}

#[cfg(feature = "native")]
fn read_u32s(device: &wgpu::Device, queue: &wgpu::Queue, src: &wgpu::Buffer, words: u64) -> Vec<u32> {
    let bytes = (words * 4).max(4);
    let staging = device.create_buffer(&wgpu::BufferDescriptor { label: Some("read back"), size: bytes, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(src, 0, &staging, 0, bytes);
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let out = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
    staging.unmap();
    out
}

/// THE GPU KEEPS EXACTLY THE QUADS THE RUST TWIN KEEPS: cards scattered all round a camera (inside
/// the hide radius, past the far cutoff, behind it, off to the sides, outside the sun's box), in
/// three patches under a rotated batch, culled on the device. Per patch and per view the indirect
/// count is twelve (main) or six (sun) indices a kept quad, the compact indices are each kept
/// quad's own vertices in the builder's order, and the kept set is `cpu_keeps`'s. Runs on the GPU
/// (skipped, and said, when the operator's GPU program is running).
///
/// Seen red 2026-11-14 with the shader's sun near plane written the OpenGL way (`r3 + r2`, depth
/// -1 to 1, where wgpu's is 0 to 1): "patch 0 sun: the GPU kept 145 quads, the twin 104". (The
/// first scene, a high sun and a deep box, kept the mutant: nothing in it lay past the depth planes.)
#[test]
#[cfg(feature = "native")]
fn the_gpu_cull_keeps_exactly_the_quads_the_rust_twin_keeps() {
    let Some(opened) = device() else {
        println!("the card cull GPU test needs a GPU it may use: not run");
        return;
    };
    let (device, queue) = (&opened.device, &opened.queue);
    let mut cull = CardCull::new(&device);
    // The camera at the origin looking down -z with the celestial projection; hide 40 m, far 300 m.
    let cam_pos = Vec3::new(0.0, 5.0, 0.0);
    let view = Mat4::look_at_rh(cam_pos, cam_pos + Vec3::new(0.3, -0.05, -1.0), Vec3::Y);
    let cam_vp = humanity_terrain::celestial_projection::celestial_projection(60.0, 16.0 / 9.0) * view;
    let (hide, far) = (40.0f32, 300.0f32);
    // The sun's box, built the way celestial.rs builds it: 200 m half-extent around the camera, and
    // only 80 m deep each way along the sun, so cards fall past all six of its planes.
    let sun = Vec3::new(0.8, 0.35, 0.3).normalize();
    let extent = 200.0;
    let light_view = Mat4::look_at_rh(cam_pos + sun * 4000.0, cam_pos, Vec3::Y);
    let light_vp = Mat4::orthographic_rh(-extent, extent, -extent, extent, 4000.0 - 80.0, 4000.0 + 80.0) * light_view;
    let rot = Mat4::from_rotation_y(0.7) * Mat4::from_rotation_x(0.1);
    use wgpu::util::DeviceExt;
    let uniform = |label: &str, bytes: &[u8]| device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: bytes, usage: wgpu::BufferUsages::UNIFORM });
    let mut cam = [0.0f32; 20];
    cam[..16].copy_from_slice(&cam_vp.to_cols_array());
    cam[16..19].copy_from_slice(&cam_pos.to_array());
    let mut light = [0.0f32; 20];
    light[..16].copy_from_slice(&light_vp.to_cols_array());
    let mut su = [0.0f32; 24];
    su[..16].copy_from_slice(&light_vp.to_cols_array());
    su[16] = 1.0;
    su[19] = hide;
    su[20] = far;
    let (cam_b, light_b, su_b, rot_b) = (uniform("cam", bytemuck::cast_slice(&cam)), uniform("light", bytemuck::cast_slice(&light)), uniform("su", bytemuck::cast_slice(&su)), uniform("rot", bytemuck::cast_slice(&rot.to_cols_array())));
    // Three patches of N cards each on a spiral out to 500 m (patch-local), at three translations.
    const N: u32 = 360;
    let positions = [Vec3::new(10.0, 0.0, -20.0), Vec3::new(-150.0, 2.0, 30.0), Vec3::new(60.0, -1.0, -250.0)];
    let mut draws = Vec::new();
    let mut locals: Vec<Vec<Vec3>> = Vec::new();
    let (mut vstart, mut istart) = (1000u32, 0u32);
    for (k, pos) in positions.iter().enumerate() {
        let centre = |q: u32| {
            let a = q as f32 * 2.4 + k as f32;
            let r = 5.0 + q as f32 * 1.4;
            Vec3::new(a.cos() * r, (q % 7) as f32, a.sin() * r)
        };
        let (vs, is) = patch_with(12, N, centre);
        cull.records.add(&device, &queue, vstart, &vs, &is);
        locals.push((0..N).map(|q| centre(q)).collect());
        draws.push(PatchDraw { slot: slot(vstart, vs.len() as u32, istart, is.len() as u32), position: *pos, fade: 0.0 });
        vstart += vs.len() as u32 + 17;
        istart += is.len() as u32;
    }
    // The arena's index buffer stand-in, the main window reserved at index 256 (a 1 KB offset).
    let region = (256u32, 16384u32);
    let index = device.create_buffer(&wgpu::BufferDescriptor { label: Some("arena indices"), size: (region.0 + region.1) as u64 * 4, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    cull.dispatch(&device, &queue, &Inputs { camera: &cam_b, light: &light_b, shadow: &su_b, rot: &rot_b, index: &index, region }, &draws, true, || None);
    let [main_args, sun_args, sun_idx] = cull.outputs();
    let p = plan(&draws, true, region, |v| cull.records.tail(v));
    assert_eq!(p.jobs.len(), 3, "every patch fits the window");
    let margs = read_u32s(&device, &queue, &main_args, 5 * p.main_args.len() as u64);
    let sargs = read_u32s(&device, &queue, &sun_args, 5 * 3);
    let midx = read_u32s(&device, &queue, &index, (region.0 + region.1) as u64);
    let sidx = read_u32s(&device, &queue, &sun_idx, p.sun_indices as u64);
    let (mut any_main_cut, mut any_sun_cut, mut any_kept) = (false, false, false);
    for (k, job) in p.jobs.iter().enumerate() {
        // Twelve hand-built corners' bounding sphere (the record) moved into render space.
        let twin: Vec<(bool, bool)> = locals[k]
            .iter()
            .map(|l| {
                let r = 34f32.sqrt();
                cpu_keeps(rot.transform_vector3(*l) + positions[k], r, cam_vp, cam_pos, hide, far, light_vp)
            })
            .collect();
        let decode = |idx: &[u32], per: usize, count: u32, dst: u32, pattern: &[u32]| -> Vec<u32> {
            let mut kept: Vec<u32> = idx[dst as usize..(dst + count) as usize]
                .chunks_exact(per)
                .map(|c| {
                    let q = (c[0] - job.v0) / 4;
                    let base = job.v0 + 4 * q;
                    assert!(c.iter().zip(pattern).all(|(&i, &pt)| i == base + pt), "patch {k}: indices {c:?} are not quad {q}'s own in the builder's order");
                    q
                })
                .collect();
            kept.sort_unstable();
            kept
        };
        let m = job.arg as usize * 5;
        let (mc, sc) = (margs[m], sargs[k * 5]);
        assert_eq!(&margs[m + 1..m + 5], &[1, region.0 + job.dst, 0, k as u32], "patch {k}: the card draw's other arguments");
        assert_eq!(margs[m - 5], draws[k].slot.icount - 12 * N, "patch {k}: its ground draw, right before its cards, stops short of them");
        assert_eq!(mc % 12, 0);
        assert_eq!(sc % 6, 0);
        let gpu_main = decode(&midx, 12, mc, region.0 + job.dst, &QUAD_INDICES);
        let gpu_sun = decode(&sidx, 6, sc, job.sdst, &QUAD_INDICES[..6]);
        let twin_main: Vec<u32> = (0..N).filter(|&q| twin[q as usize].0).collect();
        let twin_sun: Vec<u32> = (0..N).filter(|&q| twin[q as usize].1).collect();
        assert_eq!(gpu_main, twin_main, "patch {k} main: the GPU kept {} quads, the twin {}", gpu_main.len(), twin_main.len());
        assert_eq!(gpu_sun, twin_sun, "patch {k} sun: the GPU kept {} quads, the twin {}", gpu_sun.len(), twin_sun.len());
        any_main_cut |= twin_main.len() < N as usize;
        any_sun_cut |= twin_sun.len() < N as usize;
        any_kept |= !twin_main.is_empty() && !twin_sun.is_empty();
    }
    assert!(any_main_cut && any_sun_cut && any_kept, "the scene must keep some quads and cut some in each view, or it tests nothing");
}
