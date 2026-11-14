//! Tests for the GPU-driven shadow casters (renderer/shadow_gpu.rs).

use super::*;
use crate::renderer::local_shadows::{cube_face_view, face_keeps, face_test, frustum_planes, near_sun_view, sun_keeps, FaceTest};
use glam::{Mat4, Vec3, Vec4};

/// A camera looking down at a room from a corner (0..1 depth, as the renderer's).
#[cfg(feature = "native")]
fn camera_planes() -> [Vec4; 6] {
    camera_toward(Vec3::new(6.0, 4.5, 6.0), Vec3::new(0.5, 0.5, 0.5))
}

/// A camera at `eye` looking at `target`.
fn camera_toward(eye: Vec3, target: Vec3) -> [Vec4; 6] {
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    frustum_planes(&(Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, 0.05, 200.0) * view))
}

/// A deterministic spread of casters: (centre, radius, sun_only), some unbounded, some tiny, some
/// celestial, many near the faces' edges.
fn scene(n: usize) -> Vec<(Vec3, f32, bool)> {
    let mut s: u32 = 0x9e37_79b9;
    let mut rnd = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        (s as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    (0..n)
        .map(|i| {
            let c = Vec3::new(rnd() * 8.0, 1.5 + rnd() * 2.5, rnd() * 8.0);
            let r = match i % 23 {
                0 => f32::INFINITY,
                1 => 0.001,
                _ => 0.05 + (rnd() * 0.5 + 0.5) * 1.2,
            };
            (c, r, i % 17 == 3)
        })
        .collect()
}

/// THE SHADER'S TEST EQUALS THE PROCESSOR'S RULE: `gpu_keeps` (the WGSL `keeps`, line for line) and
/// `face_keeps` / `sun_keeps` (what the renderer uses for the per-draw loop and for which faces get
/// a tile) agree on every (caster, view) pair of a 2,000-caster scene, every face of a cube light,
/// a spot-like face and the near sun tile, the cull on and pinned off. And the scene exercises every
/// branch: casters kept, left out by the face, and left out by the shadow-caster cull alone.
///
/// Seen red 2026-11-13 with the shader twin's reach test removed (`if r >= 0.0 && !(...)` taken
/// out of `gpu_keeps`): "face 0 caster 12: gpu true, processor false".
#[test]
fn the_gpu_test_keeps_exactly_the_casters_the_processor_rule_keeps() {
    // The camera stands beside the light looking away from it, so the casters behind the light throw
    // their shadows out of view (the shadow-caster cull's share) while those in front throw theirs in.
    let cam = camera_toward(Vec3::new(2.0, 2.2, 0.0), Vec3::new(10.0, 1.5, 0.0));
    let light = Vec3::new(0.5, 2.6, -0.4);
    let reach = 5.2;
    let (res, margin) = (512u32, 4.0f32);
    let texel_m = 2.0 * crate::renderer::local_shadows::face_tan_half(res, margin) / res as f32;
    let casters = scene(2000);
    let (mut kept, mut out, mut culled) = (0, 0, 0);
    for cull_off in [false, true] {
        let p = CullParams { cam: cam.map(|q| q.to_array()), cull_off: u32::from(cull_off), margin, ..Default::default() };
        for face in 0..6 {
            let vp = cube_face_view(light, face, reach, res, margin);
            let planes = frustum_planes(&vp);
            let v = CullView::face(&planes, light, reach, texel_m, face as u32);
            for (i, &(c, r, sun_only)) in casters.iter().enumerate() {
                let packed = CullCaster::new([0.0; 16], c, r, 0, sun_only);
                let gpu = gpu_keeps(&packed, &v, &p);
                let cpu = !sun_only && face_keeps(&planes, &cam, light, reach, texel_m, margin, cull_off, c, r);
                assert_eq!(gpu, cpu, "face {face} caster {i}: gpu {gpu}, processor {cpu}");
                if !cull_off && !sun_only {
                    match face_test(&planes, &cam, light, reach, texel_m, margin, false, c, r) {
                        FaceTest::Kept => kept += 1,
                        FaceTest::Out => out += 1,
                        FaceTest::Culled => culled += 1,
                    }
                }
            }
        }
        // The near sun tile.
        let eye = Vec3::new(1.0, 1.7, 1.0);
        let svp = near_sun_view(eye, Vec3::new(0.3, 1.0, 0.2).normalize(), 6.0, 1024);
        let splanes = frustum_planes(&svp);
        let stexel = 12.0 / 1024.0;
        let sv = CullView::sun(&splanes, eye, stexel, 0);
        for (i, &(c, r, sun_only)) in casters.iter().enumerate() {
            let gpu = gpu_keeps(&CullCaster::new([0.0; 16], c, r, 0, sun_only), &sv, &p);
            let cpu = sun_keeps(&splanes, stexel, c, r);
            assert_eq!(gpu, cpu, "sun caster {i}: gpu {gpu}, processor {cpu}");
        }
    }
    assert!(kept > 100 && out > 100 && culled > 0, "the scene must exercise every branch: kept {kept}, out {out}, culled by the shadow-caster cull {culled}");
}

/// THE FALLBACK RUNS WHEN A FEATURE IS MISSING: without any one of the three features the path is
/// the per-draw loop, and the notice names what is missing; with all three the GPU draws, unless
/// the showcase pin asks for the old loop. Seen red 2026-11-13 with `draw_path` checking only
/// MULTI_DRAW_INDIRECT: "without indirect_count: Gpu".
#[test]
fn the_per_draw_loop_runs_when_the_device_lacks_a_feature() {
    let all = required_features();
    assert_eq!(draw_path(all, false), DrawPath::Gpu);
    assert_eq!(draw_path(all, true), DrawPath::CpuPinned);
    assert_eq!(draw_path(all | wgpu::Features::TIMESTAMP_QUERY, false), DrawPath::Gpu);
    assert!(missing_features(all).is_empty());
    for (f, word) in [
        (wgpu::Features::MULTI_DRAW_INDIRECT, "multi_draw_indirect"),
        (wgpu::Features::MULTI_DRAW_INDIRECT_COUNT, "indirect_count"),
        (wgpu::Features::INDIRECT_FIRST_INSTANCE, "first_instance"),
    ] {
        let granted = all - f;
        assert_eq!(draw_path(granted, false), DrawPath::CpuNoFeature, "without {word}: {:?}", draw_path(granted, false));
        assert_eq!(draw_path(granted, true), DrawPath::CpuNoFeature, "the pin cannot turn on what the device lacks");
        assert_eq!(missing_features(granted), vec![word]);
        assert_eq!(mask_ids(missing_mask(granted)), vec![word]);
    }
    assert_eq!(draw_path(wgpu::Features::empty(), false), DrawPath::CpuNoFeature);
}

/// THE ARENA COPIES A MESH AGAIN WHEN IT CHANGES: kept while its serial holds (nothing copied), a
/// new range and a copy when the renderer replaces it (a new serial), the old range freed for the
/// next mesh. Seen red 2026-11-13 with `place` ignoring the serial: "a replaced mesh is copied
/// again: Kept(...)".
#[test]
fn the_arena_copies_a_mesh_again_when_its_serial_changes() {
    let mut book = ArenaBook::new(1000, 3000);
    book.begin_frame(true);
    let Place::New(a) = book.place(7, 1, 100, 300) else { panic!("first placement is new") };
    assert_eq!(book.take_new(), vec![(7, a)]);
    book.begin_frame(true);
    assert_eq!(book.place(7, 1, 100, 300), Place::Kept(a), "same serial: kept, nothing to copy");
    assert!(book.take_new().is_empty());
    book.begin_frame(true);
    let replaced = book.place(7, 2, 120, 330);
    let Place::New(b) = replaced else { panic!("a replaced mesh is copied again: {replaced:?}") };
    assert_eq!((b.serial, b.vcount, b.icount), (2, 120, 330));
    assert_eq!(book.take_new(), vec![(7, b)]);
    assert_eq!(book.slot(7), Some(b));
    // The old range went back: a mesh of its size fits where it was.
    let Place::New(c) = book.place(8, 3, 100, 300) else { panic!("fits") };
    assert_eq!(book.len(), 2);
    assert!(c.vbase + c.vcount <= b.vbase || b.vbase + b.vcount <= c.vbase, "ranges never overlap: {b:?} {c:?}");
}

/// WITH REUSE OFF THE ARENA IS BUILT AGAIN EVERY FRAME: the same mesh at the same serial is placed
/// and copied anew on each frame. Seen red 2026-11-13 with `begin_frame` ignoring its argument:
/// "reuse off, frame 1: Kept(...)".
#[test]
fn with_reuse_off_the_arena_copies_every_mesh_every_frame() {
    let mut book = ArenaBook::new(1000, 3000);
    for frame in 0..3 {
        book.begin_frame(false);
        let p = book.place(4, 9, 50, 60);
        assert!(matches!(p, Place::New(_)), "reuse off, frame {frame}: {p:?}");
        assert_eq!(book.take_new().len(), 1, "frame {frame} copies the mesh");
    }
    // And on again: kept.
    book.begin_frame(true);
    assert!(matches!(book.place(4, 9, 50, 60), Place::Kept(_)));
}

/// A full arena grows by doubling and keeps every range where it was (the GPU copies the old
/// buffers into the new ones), and a mesh that fits nowhere below takes the new room on top.
#[test]
fn a_full_arena_grows_and_keeps_every_range() {
    let mut book = ArenaBook::new(100, 300);
    let Place::New(a) = book.place(0, 1, 60, 100) else { panic!() };
    let Place::New(b) = book.place(1, 2, 30, 100) else { panic!() };
    assert_eq!(book.place(2, 3, 50, 50), Place::Full);
    let (v, i) = book.grow_for(50, 50, 1 << 20, 1 << 20).expect("room to grow");
    assert!(v >= 150 && i >= 350, "grown to {v} / {i}");
    assert_eq!((book.slot(0), book.slot(1)), (Some(a), Some(b)), "every range kept");
    let Place::New(c) = book.place(2, 3, 50, 50) else { panic!("fits after the grow") };
    let apart = |x: &ArenaSlot, y: &ArenaSlot| x.vbase + x.vcount <= y.vbase || y.vbase + y.vcount <= x.vbase;
    assert!(apart(&a, &c) && apart(&b, &c) && c.vbase + c.vcount <= v, "in free room: {c:?}");
    assert!(book.grow_for(1 << 21, 1, 1 << 20, 1 << 20).is_none(), "past the device's largest buffer: no");
}

/// The cull shader parses, validates and has its three entry points.
#[test]
fn the_cull_shader_validates() {
    let module = wgpu::naga::front::wgsl::parse_str(CULL_WGSL).expect("the cull shader parses");
    let mut v = wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all());
    v.validate(&module).expect("the cull shader validates");
    let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
    for e in ["cull_count", "emit_draws", "write_instances"] {
        assert!(entries.contains(&e), "{e} missing from {entries:?}");
    }
    assert_eq!(std::mem::size_of::<CullParams>(), 128);
    assert_eq!(std::mem::size_of::<CullView>(), 144);
    assert_eq!(std::mem::size_of::<CullCaster>(), 112);
    assert_eq!(std::mem::size_of::<CullBatch>(), 32);
    assert_eq!(std::mem::size_of::<DrawArgs>() as u64, DRAW_ARGS_BYTES);
    // One instance is the transmittance pass's (the sheets read the cull's instances directly).
    assert_eq!(INSTANCE_BYTES, crate::renderer::shadow_transmit::INSTANCE_BYTES);
    // The cutout sets' shader parses and validates.
    let cut = wgpu::naga::front::wgsl::parse_str(CUTOUT_WGSL).expect("the cutout shader parses");
    let mut v = wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all());
    v.validate(&cut).expect("the cutout shader validates");
}

/// THE CUTOUT SETS' TEST IS THE MEGASHADER'S: card hair's cut in `shadow_atlas_cutout.wgsl` is
/// `fs_shadow`'s type 26 branch line for line (the threshold read from the instance's extra, which
/// the renderer fills from the material's `params2.z`), and only that type is routed there.
///
/// Seen red 2026-11-14 with the cutout shader's hair uv written without the colour half's `* 0.5`:
/// "fs_shadow's hair line `let hair_uv = vec2<f32>(clamp(fract(in.uv.x), 0.002, 0.998) * 0.5,
/// in.uv.y);` is not in the cutout shader".
#[test]
fn the_cutout_sets_test_is_fs_shadows() {
    let pbr = crate::renderer::shader_loader::assembled_pbr_source();
    let start = pbr.find("fn fs_shadow(").expect("fs_shadow");
    let fs = &pbr[start..];
    let branch = &fs[fs.find("material_type >= 25.5 && material_type < 26.5").expect("the type 26 branch")..];
    let branch = &branch[..branch.find("    }\n").or_else(|| branch.find("    }\r\n")).expect("its end")];
    let lines: Vec<&str> = branch.lines().map(str::trim).filter(|l| l.starts_with("let ") || l.starts_with("albedo_texture") || l.starts_with("vec2<f32>(uv_dx")).collect();
    assert!(lines.len() >= 3, "the branch's lines: {lines:?}");
    for l in &lines {
        assert!(CUTOUT_WGSL.contains(l), "fs_shadow's hair line `{l}` is not in the cutout shader");
    }
    assert!(branch.contains("if (hair_tex.a < max(material.params2.z, 0.0)) {"), "fs_shadow's threshold");
    assert!(CUTOUT_WGSL.contains("if (hair_tex.a < max(in.extra.y, 0.0)) {"), "the cutout shader's threshold, from the extra");
    for t in [18.9f32, 20.9, 11.9, 25.9, 26.1] {
        assert_eq!(gpu_cutout_type(t), (25.5..26.5).contains(&t), "type {t}");
    }
}

/// THE CASTER UPLOAD WRITES ONLY WHAT CHANGED, and everything with reuse off: a still frame writes
/// nothing, a moved caster writes its own run, runs a few apart are joined, a longer list writes its
/// new tail, and a frame with the reuse switch off (or a new buffer) writes every caster.
///
/// Seen red 2026-11-14 with `changed_runs` ignoring `reuse`: "with reuse off every caster is
/// written: [] of 40".
#[test]
fn the_caster_upload_writes_only_what_changed_and_all_with_reuse_off() {
    let caster = |i: usize, x: f32| CullCaster::new([0.0; 16], Vec3::new(x, i as f32, 0.0), 0.5, i as u32, false);
    let held: Vec<CullCaster> = (0..40).map(|i| caster(i, 0.0)).collect();
    assert!(changed_runs(&held, &held, true, false).is_empty(), "a still frame writes nothing");
    let mut moved = held.clone();
    moved[7] = caster(7, 1.0);
    assert_eq!(changed_runs(&held, &moved, true, false), vec![7..8]);
    moved[12] = caster(12, 1.0);
    moved[30] = caster(30, 1.0);
    assert_eq!(changed_runs(&held, &moved, true, false), vec![7..13, 30..31], "runs under eight apart are joined");
    let mut longer = held.clone();
    longer.extend((40..44).map(|i| caster(i, 0.0)));
    assert_eq!(changed_runs(&held, &longer, true, false), vec![40..44], "the new tail");
    let all = changed_runs(&held, &held, false, false);
    assert_eq!(all, vec![0..40], "with reuse off every caster is written: {all:?} of 40");
    assert_eq!(changed_runs(&held, &held, true, true), vec![0..40], "a new buffer gets every caster");
    assert!(changed_runs(&held, &[], false, false).is_empty());
}

// The GPU parts need the native build's test helpers (humanity_core::test_gpu, pollster); the relay
// build compiles the rest.
#[cfg(feature = "native")]
fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    if !humanity_core::test_gpu::may_use() {
        return None;
    }
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: None, force_fallback_adapter: false }))?;
    if !adapter.features().contains(required_features()) {
        println!("the GPU-driven shadow test needs {:?}: this adapter lacks {:?}, not run", required_features(), missing_features(adapter.features()));
        return None;
    }
    let limits = wgpu::Limits::default().using_resolution(adapter.limits());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("shadow gpu test"), required_features: required_features(), required_limits: limits, ..Default::default() }, None)).ok()?;
    device.on_uncaptured_error(Box::new(|e| panic!("wgpu validation error: {e}")));
    Some((device, queue))
}

/// The two layouts the sets' pipelines bind: the fast path's views (group 0) and a material's albedo
/// (the cutouts' group 1), as the renderer makes them.
#[cfg(feature = "native")]
fn layouts(device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
    let views = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("test views"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: wgpu::BufferSize::new(64) },
            count: None,
        }],
    });
    let tex = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("test albedo"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                count: None,
            },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
        ],
    });
    (views, tex)
}

#[cfg(feature = "native")]
fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, src: &wgpu::Buffer, offset: u64, bytes: u64) -> Vec<u8> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor { label: Some("read back"), size: bytes, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(src, offset, &staging, 0, bytes);
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let out = slice.get_mapped_range().to_vec();
    staging.unmap();
    out
}

/// THE GPU WRITES THE OLD LOOP'S DRAWS: on a known scene (two meshes, 49 casters, two faces of a
/// light and the near sun tile), the cull's indirect arguments are, view by view, one draw per mesh
/// with exactly as many instances as the per-draw loop's run of that mesh had (the processor's rule
/// and `fast_runs`), each reading its mesh's range of the arena; the instances in each draw are its
/// casters' matrices with the view's index in column 0's w; and the arena holds the meshes' own
/// indices. Runs on the GPU (skipped, and said, when the operator's GPU program is running or the
/// adapter lacks the features).
///
/// Seen red 2026-11-13 with `emit_draws` writing `n - 1u` as the instance count: "view 0: the GPU's
/// draws [(9, 6, 0, 0), (36, 5, 9, 5)] against the old loop's [(9, 7, 0, 0), (36, 6, 9, 5)]" (index
/// count, instances, first index, base vertex). Before that, its first run found a real bug: `meta`
/// is a reserved word in WGSL, so the cull shader did not compile on the device.
#[test]
#[cfg(feature = "native")]
fn the_gpu_cull_writes_the_draws_the_old_loop_made() {
    let Some((device, queue)) = device() else { return };
    let cube = crate::renderer::mesh::Mesh::cube(&device);
    let tri_v: Vec<humanity_core::vertex::Vertex> = (0..5).map(|k| humanity_core::vertex::Vertex { position: [k as f32 * 0.1, (k % 2) as f32 * 0.2, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] }).collect();
    let tri = crate::renderer::mesh::Mesh::from_vertices(&device, &tri_v, &[0, 1, 2, 2, 1, 3, 3, 1, 4]);
    let meshes = vec![tri, cube];
    let (vl, tl) = layouts(&device);
    let mut gpu = GpuCasters::new(&device, &vl, &tl);
    assert!(gpu.available);
    gpu.book.begin_frame(true);
    let slots: Vec<ArenaSlot> = meshes.iter().enumerate().map(|(id, m)| gpu.place(&device, id, id as u64 + 1, m).expect("placed")).collect();
    let batches: Vec<CullBatch> = slots.iter().map(ArenaSlot::batch).collect();

    // The scene: a 7 x 7 grid of casters on a coarse grid, alternating meshes, none near a face's edge.
    let cam = camera_planes();
    let light = Vec3::new(0.0, 2.5, 0.0);
    let (reach, res, margin) = (6.0, 512u32, 4.0f32);
    let texel_m = 2.0 * crate::renderer::local_shadows::face_tan_half(res, margin) / res as f32;
    let mut casters: Vec<(Vec3, f32, usize)> = Vec::new();
    for gx in 0..7 {
        for gz in 0..7 {
            let c = Vec3::new(-4.5 + gx as f32 * 1.5, 1.0, -4.5 + gz as f32 * 1.5);
            casters.push((c, 0.3, (gx + gz) % 2));
        }
    }
    let faces = [0usize, 4];
    let mut planes: Vec<[Vec4; 6]> = faces.iter().map(|&f| frustum_planes(&cube_face_view(light, f, reach, res, margin))).collect();
    let eye = Vec3::new(1.0, 1.7, 1.0);
    let splanes = frustum_planes(&near_sun_view(eye, Vec3::new(0.3, 1.0, 0.2).normalize(), 4.0, 1024));
    let stexel = 8.0 / 1024.0;
    planes.push(splanes);
    let mut views: Vec<CullView> = faces.iter().zip(&planes).map(|(&f, p)| CullView::face(p, light, reach, texel_m, 10 + f as u32)).collect();
    views.push(CullView::sun(&splanes, eye, stexel, 3));
    let model_of = |i: usize| Mat4::from_translation(casters[i].0).to_cols_array();
    let packed: Vec<CullCaster> = (0..casters.len()).map(|i| CullCaster::new(model_of(i), casters[i].0, casters[i].1, casters[i].2 as u32, false)).collect();

    // What the per-draw loop would draw: each view's kept casters by the processor's rule, in runs of
    // one mesh (`fast_runs`).
    let mut expected: Vec<Vec<(usize, u32, Vec<u32>)>> = Vec::new();
    for (g, v) in views.iter().enumerate() {
        let kept: Vec<(u32, u32)> = (0..casters.len())
            .filter(|&i| {
                let (c, r, _) = casters[i];
                if v.info[1] > 0.5 {
                    sun_keeps(&splanes, stexel, c, r)
                } else {
                    face_keeps(&planes[g], &cam, light, reach, texel_m, margin, false, c, r)
                }
            })
            .map(|i| (i as u32, g as u32))
            .collect();
        let mut order = Vec::new();
        let runs = crate::renderer::local_shadows::fast_runs(&kept, |ci| casters[ci as usize].2, &mut order);
        expected.push(runs.iter().map(|r| (r.mesh, r.count, order[r.first as usize..(r.first + r.count) as usize].iter().map(|&(c, _)| c).collect())).collect());
    }
    assert!(expected.iter().all(|e| !e.is_empty()), "every view draws something: {expected:?}");
    assert!(expected.iter().map(|e| e.iter().map(|r| r.1).sum::<u32>()).sum::<u32>() < (casters.len() * views.len()) as u32, "and some casters are left out");

    let params = CullParams { cam: cam.map(|q| q.to_array()), margin, ..Default::default() };
    gpu.upload(&device, &queue, params, &views, &packed, &batches, 1, (casters.len() * views.len()) as u64, true);
    let mut enc = device.create_command_encoder(&Default::default());
    assert_eq!(gpu.encode(&mut enc, &meshes, None), 2, "both meshes copied in");
    queue.submit(Some(enc.finish()));

    let (draws_buf, counts_buf, inst_buf) = gpu.outputs();
    let nb = batches.len() as u64;
    let counts: Vec<u32> = bytemuck::cast_slice(&read_back(&device, &queue, counts_buf, 0, views.len() as u64 * 4)).to_vec();
    let draws: Vec<DrawArgs> = bytemuck::cast_slice(&read_back(&device, &queue, draws_buf, 0, views.len() as u64 * nb * DRAW_ARGS_BYTES)).to_vec();
    let inst: Vec<[f32; 16]> = bytemuck::cast_slice::<u8, [f32; 20]>(&read_back(&device, &queue, inst_buf, 0, (casters.len() * views.len()) as u64 * INSTANCE_BYTES)).iter().map(|x| std::array::from_fn(|k| x[k])).collect();
    for (g, exp) in expected.iter().enumerate() {
        let got: Vec<DrawArgs> = draws[g * nb as usize..g * nb as usize + counts[g] as usize].to_vec();
        let mut got_key: Vec<(u32, u32, u32, i32)> = got.iter().map(|d| (d.index_count, d.instance_count, d.first_index, d.base_vertex)).collect();
        let mut exp_key: Vec<(u32, u32, u32, i32)> = exp.iter().map(|(m, n, _)| (slots[*m].icount, *n, slots[*m].ibase, slots[*m].vbase as i32)).collect();
        got_key.sort();
        exp_key.sort();
        assert_eq!(got_key, exp_key, "view {g}: the GPU's draws {got_key:?} against the old loop's {exp_key:?}");
        for d in &got {
            let m = slots.iter().position(|s| s.ibase == d.first_index).expect("a batch's range");
            let mut want: Vec<[u32; 16]> = exp.iter().find(|r| r.0 == m).expect("the mesh's run").2.iter().map(|&c| {
                let mut x = model_of(c as usize);
                x[3] = views[g].index[0] as f32;
                x.map(f32::to_bits)
            }).collect();
            let mut have: Vec<[u32; 16]> = inst[d.first_instance as usize..(d.first_instance + d.instance_count) as usize].iter().map(|x| x.map(f32::to_bits)).collect();
            want.sort();
            have.sort();
            assert_eq!(have, want, "view {g} mesh {m}: the instances are its casters' matrices with the view's index");
        }
    }
    // The arena holds the meshes' own indices at their ranges.
    let (_, ai) = gpu.arena_buffers();
    let tri_i: Vec<u32> = bytemuck::cast_slice(&read_back(&device, &queue, ai, slots[0].ibase as u64 * 4, 9 * 4)).to_vec();
    assert_eq!(tri_i, vec![0, 1, 2, 2, 1, 3, 3, 1, 4]);
}

/// EVERY NEW CASTER CLASS IS CULLED BY THE PROCESSOR'S RULE, set by set: the scene of
/// `the_gpu_cull_writes_the_draws_the_old_loop_made` split into three draw sets, a plain set from the
/// arena, a cutout set from a skinned body's own buffers (its batches read the mesh's own ranges, no
/// copy) and a sheet set from the arena. Each (view, set) gets exactly the draws the processor's rule
/// gives that set's casters, in the set's own run of the view's section, and each instance carries its
/// caster's extra (a cutout's threshold, a sheet's transmittance) beside its matrix.
///
/// Seen red 2026-11-14 with `emit_draws` counting every set's draws in one counter a view (the merged
/// `draw_counts[vi]`): "view 0 set 0: the GPU's draws [(0, 0, 0, 0), (0, 0, 0, 0), (9, 2, 0, 0),
/// (9, 2, 0, 0), (36, 2, 9, 5), (36, 3, 0, 0)] against the rule's [(9, 2, 0, 0), (36, 2, 9, 5)]".
#[test]
#[cfg(feature = "native")]
fn each_new_caster_class_is_culled_by_the_processor_rule() {
    let Some((device, queue)) = device() else { return };
    let cube = crate::renderer::mesh::Mesh::cube(&device);
    let tri_v: Vec<humanity_core::vertex::Vertex> = (0..5).map(|k| humanity_core::vertex::Vertex { position: [k as f32 * 0.1, (k % 2) as f32 * 0.2, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] }).collect();
    let tri = crate::renderer::mesh::Mesh::from_vertices(&device, &tri_v, &[0, 1, 2, 2, 1, 3, 3, 1, 4]);
    let meshes = vec![tri, cube];
    let (vl, tl) = layouts(&device);
    let mut gpu = GpuCasters::new(&device, &vl, &tl);
    gpu.book.begin_frame(true);
    let slots: Vec<ArenaSlot> = meshes.iter().enumerate().map(|(id, m)| gpu.place(&device, id, id as u64 + 1, m).expect("placed")).collect();
    // The batches, set by set: (set, mesh) -> batch; set 1 reads the meshes' own ranges.
    let n_sets = 3u32;
    let mut batches: Vec<CullBatch> = Vec::new();
    let mut batch_of = std::collections::HashMap::new();
    for s in 0..n_sets {
        let first = batches.len() as u32;
        for (m, mesh) in meshes.iter().enumerate() {
            let mut b = if s == 1 { CullBatch { index_count: mesh.index_count, first_index: mesh.first_index, base_vertex: mesh.base_vertex, ..Default::default() } } else { slots[m].batch() };
            b.set = s;
            b.set_first = first;
            batch_of.insert((s, m), batches.len() as u32);
            batches.push(b);
        }
    }
    let cam = camera_planes();
    let light = Vec3::new(0.0, 2.5, 0.0);
    let (reach, res, margin) = (6.0, 512u32, 4.0f32);
    let texel_m = 2.0 * crate::renderer::local_shadows::face_tan_half(res, margin) / res as f32;
    let mut casters: Vec<(Vec3, f32, usize, u32)> = Vec::new();
    for gx in 0..7 {
        for gz in 0..7 {
            let i = gx * 7 + gz;
            casters.push((Vec3::new(-4.5 + gx as f32 * 1.5, 1.0, -4.5 + gz as f32 * 1.5), 0.3, (gx + gz) % 2, (i % 3) as u32));
        }
    }
    let faces = [0usize, 4];
    let mut planes: Vec<[Vec4; 6]> = faces.iter().map(|&f| frustum_planes(&cube_face_view(light, f, reach, res, margin))).collect();
    let eye = Vec3::new(1.0, 1.7, 1.0);
    let splanes = frustum_planes(&near_sun_view(eye, Vec3::new(0.3, 1.0, 0.2).normalize(), 4.0, 1024));
    let stexel = 8.0 / 1024.0;
    planes.push(splanes);
    let mut views: Vec<CullView> = faces.iter().zip(&planes).map(|(&f, p)| CullView::face(p, light, reach, texel_m, 10 + f as u32)).collect();
    views.push(CullView::sun(&splanes, eye, stexel, 3));
    let model_of = |i: usize| Mat4::from_translation(casters[i].0).to_cols_array();
    let extra_of = |i: usize| [casters[i].3 as f32, i as f32 * 0.5, 0.25, 1.0];
    let packed: Vec<CullCaster> = (0..casters.len()).map(|i| CullCaster::new(model_of(i), casters[i].0, casters[i].1, batch_of[&(casters[i].3, casters[i].2)], false).with_extra(extra_of(i))).collect();
    let params = CullParams { cam: cam.map(|q| q.to_array()), margin, ..Default::default() };
    gpu.upload(&device, &queue, params, &views, &packed, &batches, n_sets, (casters.len() * views.len()) as u64, true);
    let mut enc = device.create_command_encoder(&Default::default());
    gpu.encode(&mut enc, &meshes, None);
    queue.submit(Some(enc.finish()));
    let (draws_buf, counts_buf, inst_buf) = gpu.outputs();
    let nb = batches.len();
    let counts: Vec<u32> = bytemuck::cast_slice(&read_back(&device, &queue, counts_buf, 0, views.len() as u64 * n_sets as u64 * 4)).to_vec();
    let draws: Vec<DrawArgs> = bytemuck::cast_slice(&read_back(&device, &queue, draws_buf, 0, (views.len() * nb) as u64 * DRAW_ARGS_BYTES)).to_vec();
    let inst: Vec<[f32; 20]> = bytemuck::cast_slice(&read_back(&device, &queue, inst_buf, 0, (casters.len() * views.len()) as u64 * INSTANCE_BYTES)).to_vec();
    let mut seen_sets = 0;
    for (g, v) in views.iter().enumerate() {
        for s in 0..n_sets {
            // The rule's draws for this set: its kept casters by mesh.
            let mut by_mesh: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
            for (i, &(c, r, m, cs)) in casters.iter().enumerate() {
                let kept = if v.info[1] > 0.5 { sun_keeps(&splanes, stexel, c, r) } else { face_keeps(&planes[g], &cam, light, reach, texel_m, margin, false, c, r) };
                if cs == s && kept {
                    by_mesh.entry(m).or_default().push(i);
                }
            }
            let batch_for = |m: usize| batches[batch_of[&(s, m)] as usize];
            let mut want: Vec<(u32, u32, u32, i32)> = by_mesh.iter().map(|(&m, list)| (batch_for(m).index_count, list.len() as u32, batch_for(m).first_index, batch_for(m).base_vertex)).collect();
            let first = batches.iter().position(|b| b.set == s).unwrap();
            let n = counts[g * n_sets as usize + s as usize] as usize;
            let got: Vec<DrawArgs> = draws[g * nb + first..g * nb + first + n].to_vec();
            let mut have: Vec<(u32, u32, u32, i32)> = got.iter().map(|d| (d.index_count, d.instance_count, d.first_index, d.base_vertex)).collect();
            want.sort();
            have.sort();
            assert_eq!(have, want, "view {g} set {s}: the GPU's draws {have:?} against the rule's {want:?}");
            seen_sets += usize::from(!want.is_empty());
            for d in &got {
                let m = (0..meshes.len()).find(|&m| batch_for(m).first_index == d.first_index && batch_for(m).base_vertex == d.base_vertex && batch_for(m).index_count == d.index_count).expect("a batch of this set");
                let mut want_i: Vec<[u32; 20]> = by_mesh[&m]
                    .iter()
                    .map(|&i| {
                        let mut x = model_of(i);
                        x[3] = v.index[0] as f32;
                        let e = extra_of(i);
                        std::array::from_fn(|k| if k < 16 { x[k].to_bits() } else { e[k - 16].to_bits() })
                    })
                    .collect();
                let mut have_i: Vec<[u32; 20]> = inst[d.first_instance as usize..(d.first_instance + d.instance_count) as usize].iter().map(|x| x.map(f32::to_bits)).collect();
                want_i.sort();
                have_i.sort();
                assert_eq!(have_i, want_i, "view {g} set {s} mesh {m}: each instance is its caster's matrix and extra");
            }
        }
    }
    assert!(seen_sets >= 6, "most (view, set) pairs draw something: {seen_sets}");
}

/// A SKINNED BODY CASTS FROM THE VERTICES SKINNED THIS FRAME: a body's own vertex buffer (the
/// skinning output the main view draws) drawn into a shadow tile through a plain set whose source is
/// that buffer, then a bone "moves" (the buffer is written again, as the compute skinning does each
/// frame) and the next frame's shadow moves with it. The same mesh copied into the arena at an
/// unchanged serial keeps the old shape, which is why the bodies are drawn from their own buffers and
/// never from the arena (and what this test would see if they were).
///
/// Seen red 2026-11-14 by drawing the body's set from the arena copy instead of its own buffer:
/// "after the bone moved the shadow covers 0 of the right half's texels and 128 of the left's".
#[test]
#[cfg(feature = "native")]
fn a_skinned_body_casts_from_the_vertices_skinned_this_frame() {
    use wgpu::util::DeviceExt;
    let Some((device, queue)) = device() else { return };
    let (vl, tl) = layouts(&device);
    let mut gpu = GpuCasters::new(&device, &vl, &tl);
    let quad = |x0: f32| -> Vec<humanity_core::vertex::Vertex> {
        [(x0, -0.5), (x0 + 0.5, -0.5), (x0, 0.5), (x0 + 0.5, 0.5)].iter().map(|&(x, y)| humanity_core::vertex::Vertex { position: [x, y, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] }).collect()
    };
    let usage = wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("skinned body"), contents: bytemuck::cast_slice(&quad(-0.5)), usage });
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("body indices"), contents: bytemuck::cast_slice(&[0u32, 1, 2, 2, 1, 3]), usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_SRC });
    let mesh = crate::renderer::mesh::Mesh { vertex_buffer: vbuf.clone(), prev_vertex_buffer: None, index_buffer: ibuf.clone(), index_count: 6, first_index: 0, base_vertex: 0, bounds: [0.0, 0.0, 0.0, 1.0] };
    let meshes = vec![mesh];
    // One orthographic view straight down the z axis over the unit square, as the near sun tile.
    let vp = Mat4::orthographic_rh(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);
    let view_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("views"), contents: bytemuck::cast_slice(&[vp.to_cols_array()]), usage: wgpu::BufferUsages::STORAGE });
    let view_group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &vl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: view_buf.as_entire_binding() }] });
    let views = vec![CullView::sun(&frustum_planes(&vp), Vec3::ZERO, 2.0 / 32.0, 0)];
    let side = 32u32;
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tile"),
        size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let dview = depth.create_view(&Default::default());
    // One frame: cull the body (one caster) and draw it from its own buffer or from the arena; how
    // many texels of the tile's left and right halves it covers.
    let frame = |gpu: &mut GpuCasters, from_arena: bool| -> (u32, u32) {
        gpu.book.begin_frame(true);
        let slot = gpu.place(&device, 0, 1, &meshes[0]).expect("placed");
        let batch = if from_arena { slot.batch() } else { CullBatch { index_count: 6, ..Default::default() } };
        let casters = vec![CullCaster::new(Mat4::IDENTITY.to_cols_array(), Vec3::ZERO, 1.0, 0, false)];
        gpu.upload(&device, &queue, CullParams { cull_off: 1, ..Default::default() }, &views, &casters, &[batch], 1, 4, true);
        let mut enc = device.create_command_encoder(&Default::default());
        gpu.encode(&mut enc, &meshes, None);
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tile"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &dview, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                ..Default::default()
            });
            let dp = gpu.draw_pipes().expect("pipes");
            pass.set_pipeline(&dp.plain);
            pass.set_bind_group(0, &view_group, &[]);
            let (sv, si) = if from_arena { gpu.arena().expect("arena") } else { (&vbuf, &ibuf) };
            pass.set_vertex_buffer(0, sv.slice(..));
            pass.set_vertex_buffer(1, gpu.instances().expect("instances").slice(..));
            pass.set_index_buffer(si.slice(..), wgpu::IndexFormat::Uint32);
            assert!(gpu.draw_set(&mut pass, 0, 0, 0, 1));
        }
        let row = 256u64;
        let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: row * side as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &depth, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::DepthOnly },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row as u32), rows_per_image: Some(side) } },
            wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        );
        queue.submit(Some(enc.finish()));
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);
        let bytes = slice.get_mapped_range().to_vec();
        let (mut left, mut right) = (0, 0);
        for y in 0..side as usize {
            for x in 0..side as usize {
                let at = y * row as usize + x * 4;
                let d = f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
                if d < 1.0 {
                    if x < side as usize / 2 {
                        left += 1;
                    } else {
                        right += 1;
                    }
                }
            }
        }
        (left, right)
    };
    let (l0, r0) = frame(&mut gpu, false);
    assert!(l0 > 100 && r0 == 0, "the body on the left casts on the left: {l0} left, {r0} right");
    // The arena's copy is taken now, at serial 1.
    let (al, ar) = frame(&mut gpu, true);
    assert!(al > 100 && ar == 0, "the arena copy matches before the move: {al} left, {ar} right");
    // A bone moves: the skinning writes the body's vertices again (the quad now on the right).
    queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&quad(0.0)));
    let (l1, r1) = frame(&mut gpu, false);
    assert!(r1 > 100 && l1 == 0, "after the bone moved the shadow covers {r1} of the right half's texels and {l1} of the left's");
    // The arena, keyed by the unchanged serial, still holds the old shape: drawing from it would leave
    // the shadow behind (why the bodies are drawn from their own buffers).
    let (al1, ar1) = frame(&mut gpu, true);
    assert!(al1 > 100 && ar1 == 0, "the arena copy is a frame stale: {al1} left, {ar1} right");
}
