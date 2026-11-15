//! The model inspector's tests (docs/design/model-inspector.md). They read the planted models made
//! by scripts/gen-inspect-fixtures.js, the same files the website's and the command line's twin
//! reads (scripts/tests/model-inspect.test.js), each built with known faults.

use super::*;
use crate::assets::model_physics::ModelPhysics;
use std::path::Path;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/inspect").join(rel)
}

fn conventions() -> Conventions {
    let text = std::fs::read_to_string(Path::new(env!("HUMANITY_REPO_ROOT")).join("data/models/inspect_conventions.json")).expect("the conventions file");
    Conventions::parse(&text).expect("the conventions parse")
}

fn row(id: &str, size: [f32; 3]) -> CatalogRow {
    CatalogRow { file: "test.ron".into(), id: id.into(), label: id.into(), shape: "box".into(), size }
}

/// Inspect a fixture with its physics file and the given catalog rows.
fn run(rel: &str, catalog: Vec<CatalogRow>) -> Inspection {
    let path = fixture(rel);
    let bytes = std::fs::read(&path).expect("the fixture");
    let model = load::load_local(&path).expect("the viewer reads it");
    let ctx = Context { dir: path.parent().map(Path::to_path_buf), physics: ModelPhysics::beside(&path), catalog, conventions: conventions(), conventions_error: None };
    inspect(&path.file_name().unwrap().to_string_lossy(), &bytes, &model, &ctx)
}

fn codes(i: &Inspection) -> Vec<&'static str> {
    i.findings.iter().map(|f| f.code).collect()
}

fn named<'a>(i: &'a Inspection, code: &str) -> Vec<String> {
    i.findings.iter().filter(|f| f.code == code).map(|f| f.vars.iter().find(|(k, _)| *k == "name").map(|(_, v)| v.clone()).unwrap_or_default()).collect()
}

/// THE PLANTED MODEL'S FAULTS ARE ALL FOUND, AND ITS GOOD PARTS PASS. Its node tree reads with
/// every empty classed by the conventions; the muzzle pointing into the model, the unnamed empty,
/// the empty no convention names, the door too small and the tilted mount are each a finding; the
/// level seat is not; the physics hull standing 1 m off the model is a collision finding; and the
/// three planted catalog rows are judged off, turned and fitting.
///
/// Seen red 2026-11-14 with breaks made in inspect.rs and then put back: the outward test flipped
/// to `> 0.0` ("assertion `left == right` failed, left: []": muzzle_01 missing from
/// empty_wrong_way); and, alone, the collision union taken without the physics hull
/// (`retain(.. && !w.label.starts_with("hull"))`: "left: 1, right: 2" collision shapes).
#[test]
fn the_planted_model_s_faults_are_found_and_its_good_parts_pass() {
    let i = run("planted/planted.glb", vec![row("planted_wide", [2.0, 0.5, 1.0]), row("planted_turned", [0.8, 0.5, 1.0]), row("planted_fits", [1.0, 0.5, 0.8])]);
    let c = codes(&i);
    // The tree.
    assert_eq!(i.nodes.len(), 9, "every node in the scene: {:?}", i.nodes.iter().map(|n| &n.name).collect::<Vec<_>>());
    assert_eq!(i.nodes[0].kind, NodeKind::Group, "the root holds the rest");
    let kind_of = |name: &str| i.nodes.iter().find(|n| n.name == name).map(|n| n.kind.clone());
    assert_eq!(kind_of("body"), Some(NodeKind::Mesh));
    assert_eq!(kind_of("UCX_body"), Some(NodeKind::Collision));
    assert_eq!(kind_of("muzzle_01"), Some(NodeKind::Empty(Some("muzzle".into()))));
    assert_eq!(kind_of("seat_main"), Some(NodeKind::Empty(Some("seat".into()))));
    assert_eq!(kind_of("Empty.003"), Some(NodeKind::Empty(None)));
    assert_eq!(i.empties().count(), 6);
    // The visible bounds leave the collision mesh out.
    assert!((i.min - Vec3::new(-0.5, 0.0, -0.4)).length() < 1e-5 && (i.max - Vec3::new(0.5, 0.5, 0.4)).length() < 1e-5, "{:?}..{:?}", i.min, i.max);
    // The empties.
    assert_eq!(named(&i, "empty_wrong_way"), vec!["muzzle_01".to_string()]);
    assert_eq!(named(&i, "empty_unknown"), vec!["Empty.003".to_string()]);
    assert_eq!(named(&i, "empty_size"), vec!["door_a".to_string()]);
    assert_eq!(named(&i, "empty_not_level"), vec!["mount_top".to_string()]);
    assert!(c.contains(&"empty_unnamed"), "{c:?}");
    assert!(!i.findings.iter().any(|f| f.vars.iter().any(|(_, v)| v == "seat_main")), "the level seat is fine");
    // Collision: two shapes (the named mesh and the hull), standing 1 m off.
    assert_eq!(i.colliders.len(), 2);
    let off = i.findings.iter().find(|f| f.code == "collision_off").expect("the hull off the model is found");
    assert_eq!(off.vars.iter().find(|(k, _)| *k == "gap").unwrap().1, "1.00");
    // The catalog rows.
    let verdict = |id: &str| i.catalog.iter().find(|f| f.row.id == id).map(|f| f.verdict);
    assert_eq!(verdict("planted_wide"), Some(Verdict::Off));
    assert_eq!(verdict("planted_turned"), Some(Verdict::Turned));
    assert_eq!(verdict("planted_fits"), Some(Verdict::Fits));
    assert!(c.contains(&"catalog_box") && c.contains(&"catalog_turned") && c.contains(&"catalog_fits"));
    // Its pivot is on its base and centred; it is one part the game draws as you see it.
    assert!(c.contains(&"pivot_base_centred") && !c.contains(&"engine_differs") && !c.contains(&"scale_huge"), "{c:?}");
    // Problems come first, and every finding has words (its key is in en.json, filled).
    assert_eq!(i.findings[0].level, Level::Problem);
    for f in &i.findings {
        let w = f.words();
        assert!(!w.starts_with("model_inspect.") && !w.contains("{{"), "{}: {w}", f.code);
    }
}

/// A MODEL EXPORTED IN CENTIMETRES IS CALLED OUT, AND A SECOND PART THE GAME WOULD DROP IS SAID.
/// Seen red 2026-11-14 with `scale.tallest_m` read as at least 500: "180 m is found".
#[test]
fn a_model_in_centimetres_and_a_part_the_game_drops_are_found() {
    let i = run("huge/huge.glb", Vec::new());
    let c = codes(&i);
    let huge = i.findings.iter().find(|f| f.code == "scale_huge").expect("180 m is found");
    assert_eq!(huge.vars.iter().find(|(k, _)| *k == "cm").unwrap().1, "1.80");
    assert!(c.contains(&"engine_differs"), "{c:?}");
    assert_eq!(i.parts, 2);
    assert_eq!(i.findings.iter().find(|f| f.code == "engine_differs").unwrap().level, Level::Check, "no catalog row draws it, so it is a check");
}

/// A .GLTF READS ITS OWN OUTSIDE FILES, AND A MISSING PICTURE IS NAMED. The viewer draws it (its
/// buffer read from beside it); the report names the missing texture. Outside files climbing out
/// of the folder, absolute paths and addresses are never read.
/// Seen red 2026-11-14 with `outside_file` answering None for every name ("the viewer reads it:
/// This 3D file could not be read: its file missing.bin is not beside it."); and, alone, with the
/// missing-texture arm turned off (`None if false =>`): "the missing picture is named".
#[test]
fn a_gltf_reads_its_own_files_and_names_a_missing_picture() {
    let i = run("missing/missing.gltf", Vec::new());
    let miss = i.findings.iter().find(|f| f.code == "texture_missing").expect("the missing picture is named");
    assert_eq!(miss.vars.iter().find(|(k, _)| *k == "file").unwrap().1, "missing_palette.png");
    assert_eq!(i.textures.len(), 1);
    assert!(i.textures[0].size.is_none());
    let dir = fixture("missing");
    assert!(load::outside_file(&dir, "missing.bin").is_some());
    assert!(load::outside_file(&dir, "../planted/physics.ron").is_none(), "never above its folder");
    assert!(load::outside_file(&dir, "..%2Fplanted%2Fphysics.ron").is_none(), "not even spelled with escapes");
    assert!(load::outside_file(&dir, &fixture("planted/physics.ron").to_string_lossy()).is_none(), "never an absolute path");
    assert!(load::outside_file(&dir, "https://example.com/x.bin").is_none(), "never an address");
    // Without its folder (a file fetched from a server) the same file is refused, as before.
    let bytes = std::fs::read(fixture("missing/missing.gltf")).unwrap();
    assert!(load::load_gltf(&bytes, None).is_err());
}

/// THE INSPECTOR'S FIT IS THE GAME'S FIT. For every catalog row in data/machines that draws a model
/// on this computer, `inspect::fit` gives the same fitted width, height and depth as the engine's
/// own `GltfCpuMesh::fit_for_machine` (what both machine paths draw), within a millimetre, so the
/// report's numbers are the game's. Seen red 2026-11-14 with `fit` scaling to the box's width
/// instead of its height ("machine chair: inspector 0.50 x 1.15 x 0.50, engine 0.39 x 0.90 x 0.39").
#[test]
fn the_inspector_s_fit_is_the_game_s_fit() {
    let repo = PathBuf::from(env!("HUMANITY_REPO_ROOT"));
    let manager = crate::assets::AssetManager::new(repo.join("data"));
    let tol = conventions().tolerance;
    let mut checked = 0;
    for file in std::fs::read_dir(repo.join("data/machines")).unwrap().flatten().map(|e| e.path()) {
        let Some(home) = humanity_systems::grow_machines::load_home(&file) else { continue };
        for (id, def) in &home.catalog {
            let Some(model) = def.model.as_deref() else { continue };
            let Ok((mut cpu, _)) = manager.parse_gltf_mesh_with_texture(model) else { continue };
            let (lo, hi) = cpu.vertices.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(a, b), v| (a.min(Vec3::from(v.position)), b.max(Vec3::from(v.position))));
            let r = CatalogRow { file: String::new(), id: id.clone(), label: String::new(), shape: def.shape.clone(), size: [def.size.0, def.size.1, def.size.2] };
            let f = fit(lo, hi, &r, &tol);
            cpu.fit_for_machine(def.size.1);
            let (elo, ehi) = cpu.vertices.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(a, b), v| (a.min(Vec3::from(v.position)), b.max(Vec3::from(v.position))));
            let e = ehi - elo;
            assert!(
                (f.fitted[0] - e.x).abs() < 1e-3 && (f.fitted[1] - e.y).abs() < 1e-3 && (f.fitted[2] - e.z).abs() < 1e-3,
                "machine {id}: inspector {:.2} x {:.2} x {:.2}, engine {:.2} x {:.2} x {:.2}",
                f.fitted[0], f.fitted[1], f.fitted[2], e.x, e.y, e.z
            );
            // And the box drawn in the file's frame lands where the engine puts the box: centred on
            // the fitted footprint, from the floor up.
            let centre_game = (Vec3::new(f.min.x, 0.0, f.min.z) + Vec3::new(f.max.x, 0.0, f.max.z)) * 0.5 * f.scale - Vec3::new((lo.x + hi.x) * 0.5, 0.0, (lo.z + hi.z) * 0.5) * f.scale;
            assert!(centre_game.length() < 1e-3, "machine {id}: its box sits {centre_game:?} off the engine's placement");
            checked += 1;
        }
    }
    assert!(checked >= 10, "only {checked} catalog models were checked");
}

/// EVERY CATALOG BOX AGAINST ITS FITTED MODEL, IN REPORT MODE (2026-11-14, for the furniture fixes
/// that follow; docs/design/model-inspector.md, section 4). Lists each catalog row in data/machines
/// whose box its fitted model does not fit within the conventions' tolerance (10% of the side, at
/// least 5 cm), and each row whose model is not on this computer. It does NOT fail on them yet: run
/// `just test-lib catalog_boxes --nocapture` (or `node scripts/inspect-model.js --catalog`) for the
/// list. It fails only when it checks nothing, and its judging is held by the planted model's test
/// above (a box off, a box turned, a box that fits).
#[test]
fn catalog_boxes_against_their_fitted_models_report() {
    let repo = PathBuf::from(env!("HUMANITY_REPO_ROOT"));
    let tol = conventions().tolerance;
    let mut seen: Vec<(String, [f32; 3], String)> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut checked = 0;
    let mut files: Vec<PathBuf> = std::fs::read_dir(repo.join("data/machines")).unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for file in files {
        let Some(home) = humanity_systems::grow_machines::load_home(&file) else { continue };
        let fname = file.file_name().unwrap().to_string_lossy().to_string();
        for (id, def) in &home.catalog {
            let Some(model) = def.model.as_deref() else { continue };
            let size = [def.size.0, def.size.1, def.size.2];
            if seen.iter().any(|(i, s, m)| i == id && *s == size && m == model) {
                continue;
            }
            seen.push((id.clone(), size, model.to_string()));
            let path = [repo.join("data").join(model), repo.join(model)].into_iter().find(|p| p.is_file());
            let Some(path) = path else {
                lines.push(format!("{fname} {id}: its model {model} is not on this computer"));
                continue;
            };
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let Ok(gltf) = gltf::Gltf::from_slice(&bytes) else { continue };
            let Ok(bufs) = buffers(&gltf, path.parent()) else { continue };
            let Some(prim) = gltf.document.meshes().next().and_then(|m| m.primitives().next()) else { continue };
            let Some((pos, _)) = triangles(&prim, &bufs) else { continue };
            let (lo, hi) = pos.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(a, b), v| (a.min(*v), b.max(*v)));
            let r = CatalogRow { file: fname.clone(), id: id.clone(), label: def.label.clone(), shape: def.shape.clone(), size };
            let f = fit(lo, hi, &r, &tol);
            checked += 1;
            if f.verdict != Verdict::Fits {
                lines.push(format!(
                    "{fname} {id}: box {} x {} m, model fitted to {} m tall is {} x {} m{}",
                    m2(f.box_wd[0]),
                    m2(f.box_wd[1]),
                    m2(f.fitted[1]),
                    m2(f.fitted[0]),
                    m2(f.fitted[2]),
                    if f.verdict == Verdict::Turned { ", turned across its box (a quarter turn fits)" } else if f.across { ", turned across its box and the wrong size" } else { "" }
                ));
            }
        }
    }
    let missing = lines.iter().filter(|l| l.ends_with("is not on this computer")).count();
    eprintln!("Catalog boxes that do not fit their models: {} of {checked} checked rows; {missing} models not on this computer:", lines.len() - missing);
    for l in &lines {
        eprintln!("  {l}");
    }
    assert!(checked >= 10, "only {checked} catalog models were checked");
}
