//! The shadow pool's tests (renderer/shadow_pool.rs).

use super::*;

/// THE POOL PACKS EACH PERSON END TO END AND STOPS AT ITS REACH: ranges in the order met, the total
/// the sum, and a person whose range would pass the pool's reach left out (None) while the ones after
/// it that still fit are packed.
///
/// Seen red 2026-11-14 with a person past the reach packed anyway: "the second does not fit, the
/// third does", [Some(0), Some(600), Some(1100)] against [Some(0), None, Some(600)].
#[test]
fn the_pool_packs_each_person_end_to_end_and_stops_at_its_reach() {
    let (bases, total) = plan(&[100, 250, 50], 1_000);
    assert_eq!(bases, vec![Some(0), Some(100), Some(350)]);
    assert_eq!(total, 400);
    let (bases, total) = plan(&[600, 500, 300], 1_000);
    assert_eq!(bases, vec![Some(0), None, Some(600)], "the second does not fit, the third does");
    assert_eq!(total, 900);
    let (bases, total) = plan(&[], 1_000);
    assert!(bases.is_empty() && total == 0);
}

/// THE PALETTE ROWS ARE THE MODEL THE VERTEX STAGE APPLIES: `rows_to_model` places a point where
/// skinning_vs.wgsl's `skin_apply` (dot of each row with the point) puts it, so a pooled body's copy in
/// its own space lands where the middle tier's vertex-stage skinning drew it.
///
/// Seen red 2026-11-14 with the rows read as columns: "[0.3, 1.7, -0.2]: rows_to_model puts it at
/// [3.3528295, 0.9186673, 12.827591], skin_apply at [3.3528295, 0.8764318, 12.406645]".
#[test]
fn the_palette_rows_are_the_model_the_vertex_stage_applies() {
    let m = Mat4::from_scale_rotation_translation(Vec3::splat(1.1), Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.2), Vec3::new(3.0, -1.0, 12.5));
    let c = m.to_cols_array_2d();
    // The palette's rows as pose.rs writes them: row r is (m[0][r], m[1][r], m[2][r], m[3][r]).
    let rows: Vec<[f32; 4]> = (0..3).map(|r| [c[0][r], c[1][r], c[2][r], c[3][r]]).collect();
    let model = skinning::rows_to_model(&rows);
    for p in [Vec3::ZERO, Vec3::new(0.3, 1.7, -0.2), Vec3::new(-0.5, 0.9, 0.4)] {
        let skin_apply = Vec3::new(Vec4::from(rows[0]).dot(p.extend(1.0)), Vec4::from(rows[1]).dot(p.extend(1.0)), Vec4::from(rows[2]).dot(p.extend(1.0)));
        let placed = model.transform_point3(p);
        assert!(placed.distance(skin_apply) < 1e-5, "{p}: rows_to_model puts it at {placed}, skin_apply at {skin_apply}");
    }
}

#[cfg(feature = "native")]
fn device() -> Option<crate::renderer::test_gpu::Gpu> {
    // The one way a test gets a device (BUG-308): the shared DX12 instance and the GPU turn.
    crate::renderer::test_gpu::open(crate::renderer::test_gpu::Ask::new("shadow pool test"))
}

#[cfg(feature = "native")]
fn read_floats(device: &wgpu::Device, queue: &wgpu::Queue, src: &wgpu::Buffer, floats: u64) -> Vec<f32> {
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("read back"), size: floats * 4, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(src, 0, &buf, 0, floats * 4);
    queue.submit(Some(enc.finish()));
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let out = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
    out
}

/// A POOLED BODY IS SKINNED AT ITS OWN RANGE, WITH NO MOTION TARGET: one body (three vertices on one
/// joint) for two people, the first with its joint moved 1 m along x and the second 2 m up, planned
/// into the pool end to end and skinned by the shadow pool's one compute pass. Each person's copy is
/// at its own range (the second at 3, past the body's own source range at 0), each moved by its own
/// joint, the pool past them untouched, and the last-pose buffer never written (a shadow job binds a
/// stand-in there; writing it would race nothing, but it is work the shadow has no use for).
///
/// Seen red 2026-11-14 with skinning.wgsl writing the copy at `vertex_base` (the merged output place):
/// "person 0 vertex 0 at pool vertex 0: its joint's move", [0.0, 2.0, 0.0] against [1.0, 0.0, 0.0]
/// (the second person's copy landed on the first's).
#[test]
#[cfg(feature = "native")]
fn a_pooled_body_is_skinned_at_its_own_range_with_no_motion_target() {
    use wgpu::util::DeviceExt;
    let Some(opened) = device() else { return };
    let (device, queue) = (&opened.device, &opened.queue);
    let mut pool = ShadowPool::new(&device);
    let joint = humanity_core::vertex::SkinnedVertex { position: [0.0; 3], normal: [0.0, 1.0, 0.0], uv: [0.25, 0.75], joints: [0; 4], weights: [u16::MAX, 0, 0, 0] };
    let source: Vec<humanity_core::vertex::SkinnedVertex> = [[0.0, 0.0, 0.0], [0.5, 1.0, 0.0], [0.0, 1.8, 0.2]].iter().map(|&p| humanity_core::vertex::SkinnedVertex { position: p, ..joint }).collect();
    let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("body"), contents: bytemuck::cast_slice(&source), usage: wgpu::BufferUsages::STORAGE });
    let morphs = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("no morphs"), contents: bytemuck::cast_slice(&[[0.0f32; 4]; 2]), usage: wgpu::BufferUsages::STORAGE });
    // The palette: per person its model matrix (identity), its morph block (nothing on) and its one
    // joint (a translation), three rows each; the second person's model matrix at matrix 4.
    let translate = |t: [f32; 3]| [[1.0, 0.0, 0.0, t[0]], [0.0, 1.0, 0.0, t[1]], [0.0, 0.0, 1.0, t[2]]];
    let identity = translate([0.0; 3]);
    let mut rows: Vec<[f32; 4]> = Vec::new();
    for t in [[1.0, 0.0, 0.0], [0.0, 2.0, 0.0]] {
        rows.extend(identity);
        rows.extend([[0.0; 4]; 3]);
        rows.extend([[0.0; 4]; 3]);
        rows.extend(translate(t));
    }
    let palette = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("palette"), contents: bytemuck::cast_slice(&rows), usage: wgpu::BufferUsages::STORAGE });
    let people = [(0usize, 0u32, 3u32, 0u32), (0, 4, 3, 0)];
    let bases = pool.plan_jobs(&device, &queue, &people);
    assert_eq!(bases, vec![Some(0), Some(3)]);
    let mut enc = device.create_command_encoder(&Default::default());
    let bufs = [palette.clone(), palette.clone(), vertices.clone(), morphs.clone()];
    pool.encode_jobs(&device, &mut enc, |_| Some(bufs.clone()), None);
    queue.submit(Some(enc.finish()));
    let out = read_floats(&device, &queue, pool.out().expect("the pool"), 8 * 8);
    for (person, (base, t)) in [(0usize, [1.0f32, 0.0, 0.0]), (3, [0.0, 2.0, 0.0])].iter().enumerate() {
        for (i, v) in source.iter().enumerate() {
            let at = (base + i) * 8;
            let want = [v.position[0] + t[0], v.position[1] + t[1], v.position[2] + t[2]];
            assert_eq!(&out[at..at + 3], &want[..], "person {person} vertex {i} at pool vertex {}: its joint's move", base + i);
            assert_eq!(&out[at + 6..at + 8], &v.uv[..], "person {person} vertex {i}: the uv comes through for the cutouts");
        }
    }
    assert!(out[6 * 8..8 * 8].iter().all(|&f| f == 0.0), "the pool past the two people is untouched: {:?}", &out[6 * 8..8 * 8]);
    let prev = read_floats(&device, &queue, &pool.no_prev, 64);
    assert!(prev.iter().all(|&f| f == 0.0), "no last-pose write for a shadow job: {:?}", &prev[..8]);
}
