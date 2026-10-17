//! Tests of the 3D model viewer (docs/design/model-viewer.md). The files are the shared fixtures
//! in scripts/tests/fixtures (made by scripts/gen-model-fixtures.js), which the website's tests
//! read too. Each test was seen to fail once before it passed; what it saw red is in its comment.

use super::camera::Orbit;
use super::load::{self, Alpha, Model};
use super::*;
use glam::Vec3;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

/// Every triangle's three corners, in order, from every chunk.
fn triangles(m: &Model) -> Vec<[Vec3; 3]> {
    let mut out = Vec::new();
    for c in &m.chunks {
        for d in &c.draws {
            for t in c.indices[d.first as usize..(d.first + d.count) as usize].chunks(3) {
                let p = |i: u32| Vec3::from_array(c.vertices[i as usize].position);
                out.push([p(t[0]), p(t[1]), p(t[2])]);
            }
        }
    }
    out
}

/// THE CUBE READS WITH ITS NODES, ITS MATERIAL AND ITS PICTURE: 12 triangles, every vertex moved by
/// both nodes (the corners land at x 1..5, y and z -2..2), one material with its 2x2 picture (and
/// its mipmap) and roughness 0.5, normals still unit length after the scale.
///
/// Seen red 2026-10-16 with the parent node's matrix left out of `walk`: "the nodes apply" found
/// the box at x -2..2.
#[test]
fn the_cube_reads_with_its_nodes_material_and_picture() {
    let m = load::load("cube.glb", &fixture("model-cube.glb")).expect("reads");
    assert_eq!(m.triangles, 12);
    assert!(close(m.bounds.min, Vec3::new(1.0, -2.0, -2.0)) && close(m.bounds.max, Vec3::new(5.0, 2.0, 2.0)), "the nodes apply: {:?}", m.bounds);
    assert!(close(m.bounds.center, Vec3::new(3.0, 0.0, 0.0)));
    assert!((m.bounds.radius - 12f32.sqrt()).abs() < 1e-4, "radius {}", m.bounds.radius);
    assert_eq!(m.chunks.len(), 1);
    assert_eq!((m.chunks[0].vertices.len(), m.chunks[0].indices.len()), (24, 36));
    assert!(m.chunks[0].vertices.iter().all(|v| (Vec3::from_array(v.normal).length() - 1.0).abs() < 1e-4), "unit normals");
    assert_eq!(m.materials.len(), 1);
    let mat = &m.materials[0];
    assert_eq!((mat.base_texture.map(|t| t.image), mat.metallic, mat.roughness, mat.alpha, mat.double_sided), (Some(0), 0.0, 0.5, Alpha::Opaque, false));
    // The file asks for sharp pixels (NEAREST both ways, so no mipmaps), repeating.
    let sharp = load::Sampling { mag_nearest: true, min_nearest: true, mip_nearest: None, wrap: [load::Wrap::Repeat; 2] };
    assert_eq!(mat.base_texture.map(|t| t.sampling), Some(sharp), "the file's own sampler");
    assert_eq!(m.images.len(), 1);
    assert_eq!((m.images[0].width, m.images[0].height, m.images[0].levels.len()), (2, 2, 2), "2x2 and its 1x1 mipmap");
    assert_eq!(&m.images[0].levels[0][0..4], &[255, 0, 0, 255], "the first pixel is red");
}

/// BOTH STL FORMS READ THE SAME WEDGE, TURNED TO Y-UP: the binary and ASCII files give the same two
/// triangles, (x, y, z) -> (x, z, -y), with normals worked out from the corners (the files' own are
/// zero).
///
/// Seen red 2026-10-16 with the z-up turn left out: "y-up" found the first triangle flat in z.
#[test]
fn both_stl_forms_read_the_same_wedge_turned_y_up() {
    let binary = load::load("wedge.stl", &fixture("model-wedge.stl")).expect("binary reads");
    let ascii = load::load("wedge.stl", &fixture("model-wedge-ascii.stl")).expect("ascii reads");
    assert_eq!(triangles(&binary), triangles(&ascii), "the same triangles");
    let t = triangles(&binary);
    assert_eq!(t.len(), 2);
    assert!(close(t[0][1], Vec3::new(10.0, 0.0, 0.0)) && close(t[0][2], Vec3::new(0.0, 0.0, -10.0)), "y-up: {:?}", t[0]);
    assert!(close(t[1][2], Vec3::new(0.0, 5.0, 0.0)));
    let normals: Vec<Vec3> = binary.chunks[0].vertices.iter().map(|v| Vec3::from_array(v.normal)).collect();
    assert!(close(normals[0], Vec3::Y) && close(normals[3], Vec3::X), "from the corners: {normals:?}");
    assert!(binary.materials[0].double_sided, "both sides drawn");
    assert!(close(binary.bounds.min, Vec3::new(0.0, 0.0, -10.0)) && close(binary.bounds.max, Vec3::new(10.0, 5.0, 0.0)));
}

/// The cube's JSON and binary chunk, to break on purpose.
fn glb_parts(b: &[u8]) -> (serde_json::Value, Vec<u8>) {
    let jl = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    let json = serde_json::from_slice(&b[20..20 + jl]).unwrap();
    let bl = u32::from_le_bytes(b[20 + jl..24 + jl].try_into().unwrap()) as usize;
    (json, b[28 + jl..28 + jl + bl].to_vec())
}

fn glb_from(json: &serde_json::Value, bin: &[u8]) -> Vec<u8> {
    let mut j = serde_json::to_vec(json).unwrap();
    while j.len() % 4 != 0 {
        j.push(b' ');
    }
    let mut out = b"glTF".to_vec();
    out.extend(2u32.to_le_bytes());
    out.extend(((12 + 8 + j.len() + 8 + bin.len()) as u32).to_le_bytes());
    out.extend((j.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(j);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(bin);
    out
}

/// A BROKEN FILE IS REFUSED IN WORDS, NEVER READ OUTSIDE THE FILE: a wrong start, a cut-off file,
/// an accessor running past its buffer, an index past the vertices, a Draco file, nodes that loop
/// back on themselves. Each is an error (none a panic), in the spec's words where it has them.
///
/// Seen red 2026-10-16 with `check_accessor` not checking the count: the long accessor was not
/// refused in its own words ("runs past its buffer view"); the reader quietly read fewer.
#[test]
fn a_broken_file_is_refused_in_words_never_read_outside_the_file() {
    let cube = fixture("model-cube.glb");
    let refused = |bytes: &[u8]| load::load("x.glb", bytes).expect_err("refused");
    assert!(refused(b"nope, not a model at all").contains("not a binary glTF"));
    for cut in [13, 40, 100, cube.len() / 2, cube.len() - 9] {
        refused(&cube[..cut]);
    }
    let (json, bin) = glb_parts(&cube);
    let mut long = json.clone();
    long["accessors"][0]["count"] = 1000.into();
    assert!(refused(&glb_from(&long, &bin)).contains("runs past its buffer view"));
    let mut wild = bin.clone();
    let at = json["bufferViews"][3]["byteOffset"].as_u64().unwrap() as usize;
    wild[at..at + 2].copy_from_slice(&999u16.to_le_bytes());
    assert!(refused(&glb_from(&json, &wild)).contains("an index points past the vertices"));
    let mut draco = json.clone();
    draco["extensionsUsed"] = serde_json::json!(["KHR_draco_mesh_compression"]);
    draco["extensionsRequired"] = serde_json::json!(["KHR_draco_mesh_compression"]);
    assert_eq!(refused(&glb_from(&draco, &bin)), load::DRACO_WORDS);
    let mut looped = json.clone();
    looped["nodes"][1]["children"] = serde_json::json!([0]);
    refused(&glb_from(&looped, &bin));
    let mut quantized = json.clone();
    quantized["accessors"][0]["componentType"] = 5123.into();
    refused(&glb_from(&quantized, &bin));
    assert!(load::load("x.stl", b"solid nothing\nendsolid").expect_err("no triangles").contains("no triangles"));
    assert!(load::load("x.stl", &[0u8; 90]).is_err(), "a binary STL whose count does not match its size");
}

/// EACH TEXTURE IS READ AS ITS FILE SAYS, AND THE LIMITS HOLD (2026-10-16): a texture with no
/// sampler is smooth, mipmapped and repeating; a mesh past 15,000,000 vertices, nodes that repeat
/// each other a million times, and Draco positions the file only uses (not requires) are each
/// refused in the words both apps share.
///
/// Seen red 2026-10-16 with the node visits uncounted: "nodes that repeat each other" got the
/// too-many-triangles words instead (12,582,912 triangles: the cube 2^20 times over).
#[test]
fn each_texture_is_read_as_its_file_says_and_the_limits_hold() {
    let cube = fixture("model-cube.glb");
    let (json, bin) = glb_parts(&cube);
    let mut plain = json.clone();
    plain["textures"][0].as_object_mut().unwrap().remove("sampler");
    let m = load::load("x.glb", &glb_from(&plain, &bin)).expect("reads");
    assert_eq!(m.materials[0].base_texture.map(|t| t.sampling), Some(load::Sampling::default()), "no sampler: the defaults");
    assert_eq!(load::Sampling::default(), load::Sampling { mag_nearest: false, min_nearest: false, mip_nearest: Some(false), wrap: [load::Wrap::Repeat; 2] });

    let mut many = json.clone();
    many["accessors"][0]["count"] = 15_000_001u64.into();
    assert_eq!(load::load("x.glb", &glb_from(&many, &bin)).expect_err("too many"), load::too_many_vertices_words(15_000_001));
    assert!(load::too_many_vertices_words(15_000_001).starts_with("This model has 15,000,001 vertices, more than the 15,000,000"));

    // Twenty nodes, each with the next one twice as its children: the last mesh 2^20 times.
    let mut repeat = json.clone();
    let nodes: Vec<serde_json::Value> = (0..21).map(|i| if i < 20 { serde_json::json!({ "children": [i + 1, i + 1] }) } else { serde_json::json!({ "mesh": 0 }) }).collect();
    repeat["nodes"] = serde_json::Value::Array(nodes);
    assert_eq!(load::load("x.glb", &glb_from(&repeat, &bin)).expect_err("repeats"), load::NODES_REPEAT_WORDS, "nodes that repeat each other");

    let mut draco = json.clone();
    draco["extensionsUsed"] = serde_json::json!(["KHR_draco_mesh_compression"]);
    draco["accessors"][0].as_object_mut().unwrap().remove("bufferView");
    assert_eq!(load::load("x.glb", &glb_from(&draco, &bin)).expect_err("draco"), load::DRACO_WORDS, "positions only in the compressed copy");
    assert_eq!(fetch_failed("timed out"), "This model could not be fetched: timed out.");
}

/// A BIG MODEL IS SPLIT INTO CHUNKS THAT KEEP EVERY TRIANGLE: over the chunk size (here 7 vertices
/// a chunk, in the app 4 million: one GPU buffer), whole triangles go into chunks, none lost, none
/// changed, none split.
///
/// Seen red 2026-10-16 with the current triangle dropped when a chunk filled: "every triangle,
/// in order" found 4 of 10.
#[test]
fn a_big_model_is_split_into_chunks_that_keep_every_triangle() {
    let mut stl = vec![0u8; 80];
    stl.extend(10u32.to_le_bytes());
    for i in 0..10 {
        stl.extend([0u8; 12]);
        for corner in [[i as f32, 0.0, 0.0], [i as f32 + 1.0, 0.0, 0.0], [i as f32, 1.0, 0.0]] {
            for c in corner {
                stl.extend(c.to_le_bytes());
            }
        }
        stl.extend([0u8; 2]);
    }
    let whole = load::load_stl(&stl).expect("reads");
    let split = load::stl_build(&stl).unwrap().finish_in(7).expect("splits");
    assert!(split.chunks.len() > 1 && split.chunks.iter().all(|c| c.vertices.len() <= 7), "chunks of at most 7: {:?}", split.chunks.iter().map(|c| c.vertices.len()).collect::<Vec<_>>());
    assert_eq!(triangles(&split), triangles(&whole), "every triangle, in order");
    assert_eq!(split.triangles, 10);
}

/// THE CAMERA FITS, TURNS, MOVES AND ZOOMS AS THE SPEC SAYS: the distance r / sin(17.5 degrees) *
/// 1.15, the start at yaw 35 and pitch 20, pitch held at 89 degrees, zoom held to 0.05 r .. 50 r,
/// a pan that moves the target across the view (not towards it), and back to the fit.
///
/// Seen red 2026-10-16 with the pitch unclamped: "held at 89 degrees" found 1000 radians.
#[test]
fn the_camera_fits_turns_moves_and_zooms_as_the_spec_says() {
    let m = load::load("cube.glb", &fixture("model-cube.glb")).unwrap();
    let r = m.bounds.radius;
    let mut o = Orbit::fit(&m.bounds);
    let d = r / 17.5f32.to_radians().sin() * 1.15;
    assert!((o.distance - d).abs() < 1e-4);
    let (y, p) = (35f32.to_radians(), 20f32.to_radians());
    assert!(close(o.eye(), m.bounds.center + d * Vec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos())));
    o.turn(0.0, 1e6);
    assert!((o.pitch - 89f32.to_radians()).abs() < 1e-5, "held at 89 degrees: {}", o.pitch);
    o.zoom(1e9);
    assert!((o.distance - 0.05 * r).abs() < 1e-4);
    o.zoom(-1e9);
    assert!((o.distance - 50.0 * r).abs() < 1e-2);
    o.reset();
    let start = o.target;
    let forward = (o.target - o.eye()).normalize();
    o.pan(10.0, 0.0, 100.0);
    let moved = o.target - start;
    assert!(moved.length() > 0.0 && moved.dot(forward).abs() < 1e-4, "across the view: {moved:?}");
    o.reset();
    assert!(close(o.target, m.bounds.center) && (o.distance - d).abs() < 1e-4, "back to the fit");
}

/// One pass of a viewer, `width` points wide, with `events`; its output.
fn pass(ctx: &egui::Context, theme: &crate::gui::theme::Theme, viewer: &mut Viewer, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))), events, ..Default::default() };
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| viewer.show(ui, theme, 600.0));
    })
}

fn texts(out: &egui::FullOutput) -> Vec<String> {
    fn walk(s: &egui::Shape, found: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => found.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut found));
    found
}

fn callbacks(out: &egui::FullOutput) -> usize {
    out.shapes.iter().filter(|c| matches!(c.shape, egui::Shape::Callback(_))).count()
}

/// THE VIEWER FETCHES, READS AND DRAWS, AND THE MOUSE TURNS AND ZOOMS IT: the words while it loads,
/// then the GPU callback over the view and the line with its triangle count; a drag turns it, a
/// scroll over it zooms it (and is taken from the page), a double-click starts over.
///
/// Seen red 2026-10-16 with the scroll left on the page: "the scroll is the viewer's" found it
/// still there for the window to scroll.
#[test]
fn the_viewer_fetches_reads_and_draws_and_the_mouse_turns_it() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let bytes = fixture("model-cube.glb");
    let size = bytes.len() as i64;
    let mut viewer = Viewer::open("cube.glb", size, move || Ok(bytes));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !viewer.ready() {
        let out = pass(&ctx, &theme, &mut viewer, vec![]);
        if !viewer.ready() {
            assert_eq!(callbacks(&out), 0, "no view before the model is in");
        }
        assert!(std::time::Instant::now() < deadline, "the model is read");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    pass(&ctx, &theme, &mut viewer, vec![]);
    let out = pass(&ctx, &theme, &mut viewer, vec![]);
    assert_eq!(callbacks(&out), 1, "the GPU draws the view");
    assert!(texts(&out).iter().any(|t| t.starts_with("cube.glb: drag to turn it") && t.ends_with("12 triangles.")), "{:?}", texts(&out));
    assert!(texts(&out).iter().any(|t| t == NO_GRAPHICS), "the words under the view, for where nothing draws over them");

    let start = viewer.orbit.unwrap();
    let at = egui::pos2(300.0, 150.0);
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    pass(&ctx, &theme, &mut viewer, vec![egui::Event::PointerMoved(at), button(true)]);
    pass(&ctx, &theme, &mut viewer, vec![egui::Event::PointerMoved(at + egui::vec2(40.0, 0.0))]);
    pass(&ctx, &theme, &mut viewer, vec![egui::Event::PointerMoved(at + egui::vec2(80.0, 0.0)), button(false)]);
    assert!(viewer.orbit.unwrap().yaw < start.yaw, "a drag right turns it");

    let before = viewer.orbit.unwrap().distance;
    let scroll = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, 120.0), modifiers: Default::default() };
    let mut taken = true;
    for events in [vec![egui::Event::PointerMoved(at), scroll], vec![], vec![], vec![], vec![]] {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))), events, ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                viewer.show(ui, &theme, 600.0);
                taken &= ui.input(|i| i.smooth_scroll_delta.y) == 0.0;
            });
        });
    }
    assert!(viewer.orbit.unwrap().distance < before, "a scroll up zooms in");
    assert!(taken, "the scroll is the viewer's, not the page's");

    // egui spreads a wheel step over several frames: let it finish before starting over.
    for _ in 0..120 {
        pass(&ctx, &theme, &mut viewer, vec![]);
    }
    let pos = egui::pos2(300.0, 150.0);
    let click = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    for _ in 0..2 {
        pass(&ctx, &theme, &mut viewer, vec![egui::Event::PointerMoved(pos), click(true)]);
        pass(&ctx, &theme, &mut viewer, vec![click(false)]);
    }
    pass(&ctx, &theme, &mut viewer, vec![]);
    assert!((viewer.orbit.unwrap().distance - start.distance).abs() < 1e-4 && (viewer.orbit.unwrap().yaw - start.yaw).abs() < 1e-5, "a double-click starts over");
}

/// A MODEL OVER 200 MB IS NEVER FETCHED: the size the page already knows is checked first, and the
/// words say why and what to do instead.
///
/// Seen red 2026-10-16 with the size check after the fetch: "nothing is fetched" found the fetch run.
#[test]
fn a_model_over_the_limit_is_never_fetched() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let fetched = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = fetched.clone();
    let mut viewer = Viewer::open("huge.glb", 300 * 1024 * 1024, move || {
        seen.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(Vec::new())
    });
    std::thread::sleep(std::time::Duration::from_millis(50));
    let out = pass(&ctx, &theme, &mut viewer, vec![]);
    assert!(!fetched.load(std::sync::atomic::Ordering::SeqCst), "nothing is fetched");
    assert!(texts(&out).iter().any(|t| t.starts_with("This model is 300") && t.ends_with("Download it to see it.")), "{:?}", texts(&out));
}
