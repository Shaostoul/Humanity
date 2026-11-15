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
    CatalogRow { file: "test.ron".into(), id: id.into(), label: id.into(), shape: "box".into(), size, model_yaw: 0.0 }
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
/// own `GltfCpuMesh::fit_for_machine` (what the machine draw path does), its `model_yaw` turn
/// included, within a millimetre, so the
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
            let positions: Vec<Vec3> = cpu.vertices.iter().map(|v| Vec3::from(v.position)).collect();
            let (lo, hi) = turned_bounds(&positions, def.model_yaw);
            let r = CatalogRow { file: String::new(), id: id.clone(), label: String::new(), shape: def.shape.clone(), size: [def.size.0, def.size.1, def.size.2], model_yaw: def.model_yaw };
            let f = fit(lo, hi, &r, &tol);
            cpu.fit_for_machine(def.size.1, def.model_yaw);
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

/// A catalog row the box check knows does not fit, and why (data/models/catalog_fit_exceptions.json).
#[derive(serde::Deserialize)]
struct FitException {
    file: String,
    id: String,
    reason: String,
}

#[derive(serde::Deserialize)]
struct FitExceptions {
    exceptions: Vec<FitException>,
}

/// EVERY CATALOG BOX FITS ITS FITTED MODEL (2026-11-15, the furniture fixes; docs/design/model-inspector.md,
/// section 4). Each catalog row in data/machines that draws a model on this computer has a box its
/// fitted model (turned by its `model_yaw`, scaled to its height, `fit`) fits within the conventions'
/// tolerance (10% of the side, at least 5 cm), because the walk, the collision and the flush
/// placement all follow the box while the eye follows the model. It FAILS on any row that does not,
/// unless data/models/catalog_fit_exceptions.json lists it with its reason; an exception that names
/// no such row or that fits now fails too, so the list only shrinks. A row whose model is in a
/// download pack not installed here (data/models/packs.ron, Settings > Graphics > Furniture models)
/// is named with its pack and not judged: its box is judged wherever the pack is installed.
/// `node scripts/inspect-model.js --catalog` prints the same list.
///
/// In report mode from 2026-11-14 (11 rows off, the mirror fixed by BUG-309). Seen red 2026-11-15
/// with one fix undone (the wardrobe's box back to 1.20 m wide): "cabin.ron wardrobe: box 1.20 x
/// 0.59 m, model fitted to 2.00 m tall is 0.94 x 0.59 m".
#[test]
fn every_catalog_box_fits_its_fitted_model() {
    let repo = PathBuf::from(env!("HUMANITY_REPO_ROOT"));
    let tol = conventions().tolerance;
    let exceptions: FitExceptions = serde_json::from_str(&std::fs::read_to_string(repo.join("data/models/catalog_fit_exceptions.json")).expect("data/models/catalog_fit_exceptions.json")).expect("the exceptions parse");
    for e in &exceptions.exceptions {
        assert!(!e.reason.trim().is_empty(), "the exception {} {} says no reason", e.file, e.id);
    }
    let packs = crate::model_packs::catalog::ModelPackCatalog::load(&repo.join("data")).expect("data/models/packs.ron loads");
    let mut seen: Vec<(String, [f32; 3], String, f32)> = Vec::new();
    let mut off: Vec<String> = Vec::new();
    let mut excused: Vec<(String, String)> = Vec::new();
    let mut not_here: Vec<String> = Vec::new();
    let mut checked = 0;
    let mut files: Vec<PathBuf> = std::fs::read_dir(repo.join("data/machines")).unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for file in files {
        let Some(home) = humanity_systems::grow_machines::load_home(&file) else { continue };
        let fname = file.file_name().unwrap().to_string_lossy().to_string();
        for (id, def) in &home.catalog {
            let Some(model) = def.model.as_deref() else { continue };
            let size = [def.size.0, def.size.1, def.size.2];
            if seen.iter().any(|(i, s, m, y)| i == id && *s == size && m == model && *y == def.model_yaw) {
                continue;
            }
            seen.push((id.clone(), size, model.to_string(), def.model_yaw));
            let path = [repo.join("data").join(model), repo.join(model)].into_iter().find(|p| p.is_file());
            let Some(path) = path else {
                let pack = packs.packs.iter().find(|p| model.starts_with(&format!("models/packs/{}/", p.id)));
                assert!(pack.is_some(), "{fname} {id}: its model {model} is not on this computer and no download pack (data/models/packs.ron) holds it");
                not_here.push(format!("{fname} {id}: in the {} pack, not installed here", pack.unwrap().id));
                continue;
            };
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{fname} {id}: {model} could not be read: {e}"));
            let gltf = gltf::Gltf::from_slice(&bytes).unwrap_or_else(|e| panic!("{fname} {id}: {model} does not parse: {e}"));
            let bufs = buffers(&gltf, path.parent()).unwrap_or_else(|e| panic!("{fname} {id}: {model}: {e}"));
            let prim = gltf.document.meshes().next().and_then(|m| m.primitives().next()).unwrap_or_else(|| panic!("{fname} {id}: {model} has no mesh"));
            let (pos, _) = triangles(&prim, &bufs).unwrap_or_else(|| panic!("{fname} {id}: {model} has no triangles"));
            let (lo, hi) = turned_bounds(&pos, def.model_yaw);
            let r = CatalogRow { file: fname.clone(), id: id.clone(), label: def.label.clone(), shape: def.shape.clone(), size, model_yaw: def.model_yaw };
            let f = fit(lo, hi, &r, &tol);
            checked += 1;
            if f.verdict == Verdict::Fits {
                continue;
            }
            if exceptions.exceptions.iter().any(|e| e.file == fname && &e.id == id) {
                excused.push((fname.clone(), id.clone()));
                continue;
            }
            off.push(format!(
                "{fname} {id}: box {} x {} m, model fitted to {} m tall is {} x {} m{}",
                m2(f.box_wd[0]),
                m2(f.box_wd[1]),
                m2(f.fitted[1]),
                m2(f.fitted[0]),
                m2(f.fitted[2]),
                if f.verdict == Verdict::Turned { ", turned across its box (a quarter turn, model_yaw, fits)" } else if f.across { ", turned across its box and the wrong size" } else { "" }
            ));
        }
    }
    eprintln!("Catalog boxes checked against their fitted models: {checked}; {} known exception(s); {} model(s) in packs not installed here", excused.len(), not_here.len());
    for l in &not_here {
        eprintln!("  {l}");
    }
    assert!(checked >= 10, "only {checked} catalog models were checked");
    assert!(off.is_empty(), "{} catalog box(es) do not fit their fitted models (fix the box or the model's fit, or list the row with its reason in data/models/catalog_fit_exceptions.json):\n  {}", off.len(), off.join("\n  "));
    for e in &exceptions.exceptions {
        assert!(excused.iter().any(|(f, i)| f == &e.file && i == &e.id), "the exception {} {} names no catalog row whose box is off its model (it fits now, or the row is gone): take it out of data/models/catalog_fit_exceptions.json", e.file, e.id);
    }
}

/// A ROW THAT SAYS ITS MODEL CLEARS THE SKIRTING HAS ROOM FOR IT (room finishes met furniture,
/// 2026-11-15). Every catalog row in data/machines with `clears_skirting` draws a model that, turned
/// and fitted as the game draws it (`fit_for_machine`), stands back from its footprint's edge on
/// every side, below the tallest skirting profile's top, by at least the deepest skirting profile's
/// depth (data/blueprints/room_finishes.ron): a skirting fits under its top whichever side meets the
/// wall, so the flush check (crates/humanity-ship/src/furniture_fit_tests.rs) may stand it at the
/// wall's face. Seen red 2026-11-15 with `clears_skirting` set on the nightstand, whose back is
/// solid to the floor: "home.ron nightstand claims clears_skirting, but its model stands back only
/// 0.0000 m from its +z edge below 0.170 m (a skirting needs 0.020 m)".
#[test]
fn a_row_that_clears_the_skirting_has_room_for_it() {
    // A millimetre: the scale the inspector and the engine agree to, far under the flush check's 5 mm
    // (the desk's legs stand 19.9 mm in from its top's edge against the deepest skirting's 20 mm).
    const CLEAR_SLACK_M: f32 = 0.001;
    use humanity_ship::room_finishes::{catalog, ProfileKind};
    let repo = PathBuf::from(env!("HUMANITY_REPO_ROOT"));
    let manager = crate::assets::AssetManager::new(repo.join("data"));
    let skirtings: Vec<_> = catalog().profiles_of(ProfileKind::Skirting).collect();
    let need = skirtings.iter().map(|p| p.depth()).fold(0.0f32, f32::max);
    let top = skirtings.iter().map(|p| p.height()).fold(0.0f32, f32::max);
    assert!(need > 0.0 && top > 0.0, "room_finishes.ron has skirtings");
    let mut claims = 0;
    for file in std::fs::read_dir(repo.join("data/machines")).unwrap().flatten().map(|e| e.path()) {
        let Some(home) = humanity_systems::grow_machines::load_home(&file) else { continue };
        let fname = file.file_name().unwrap().to_string_lossy().to_string();
        for (id, def) in home.catalog.iter().filter(|(_, d)| d.clears_skirting) {
            claims += 1;
            let model = def.model.as_deref().unwrap_or_else(|| panic!("{fname} {id} claims clears_skirting but draws no model: a plain box is solid to the floor"));
            let (mut cpu, _) = manager.parse_gltf_mesh_with_texture(model).unwrap_or_else(|e| panic!("{fname} {id}: {model}: {e}"));
            cpu.fit_for_machine(def.size.1, def.model_yaw);
            let all: Vec<Vec3> = cpu.vertices.iter().map(|v| Vec3::from(v.position)).collect();
            let (lo, hi) = turned_bounds(&all, 0.0);
            let low: Vec<Vec3> = all.iter().copied().filter(|v| v.y <= lo.y + top).collect();
            let (llo, lhi) = turned_bounds(&low, 0.0);
            for (side, back) in [("-x", llo.x - lo.x), ("+x", hi.x - lhi.x), ("-z", llo.z - lo.z), ("+z", hi.z - lhi.z)] {
                assert!(
                    back >= need - CLEAR_SLACK_M,
                    "{fname} {id} claims clears_skirting, but its model stands back only {back:.4} m from its {side} edge below {top:.3} m (a skirting needs {need:.3} m)"
                );
            }
        }
    }
    assert!(claims >= 1, "no row claims clears_skirting, so this checks nothing");
}

/// THE MODEL TURNS THE WAY ITS ROW SAYS (model_yaw, model inspector rung 2, 2026-11-15). The home's
/// double bed (data/machines/home.ron), whose model has its headboard at its file's far +Z, turned by
/// its row's model_yaw 270 and fitted as the game draws it (`fit_for_machine`): it fills the row's
/// box (2.04 x 1.73 m), and its headboard (every vertex over 90% of its height) stands at the box's
/// -X end, where the homes put the head, by glam's turn (the instance `rotation`'s sense, which turns
/// the anchors with it). Seen red with the turn's sign flipped in `turn_about_up` (`-s` for `s`):
/// "the headboard reaches x 1.020; the bed's -X end is -1.020".
#[test]
fn the_model_turns_the_way_its_row_says() {
    let repo = PathBuf::from(env!("HUMANITY_REPO_ROOT"));
    let manager = crate::assets::AssetManager::new(repo.join("data"));
    let home = humanity_systems::grow_machines::load_home(&repo.join("data/machines/home.ron")).expect("home.ron");
    let def = home.catalog.get("bed").expect("the bed row");
    assert_eq!(def.model_yaw, 270.0, "the bed's model is turned inside its box");
    let (mut cpu, _) = manager.parse_gltf_mesh_with_texture(def.model.as_deref().expect("a model")).expect("the bed's model loads");
    cpu.fit_for_machine(def.size.1, def.model_yaw);
    let all: Vec<Vec3> = cpu.vertices.iter().map(|v| Vec3::from(v.position)).collect();
    let (lo, hi) = turned_bounds(&all, 0.0);
    assert!((hi.x - lo.x - def.size.0).abs() < 0.01 && (hi.z - lo.z - def.size.2).abs() < 0.01, "the turned bed is {:.3} x {:.3} m, its box {} x {} m", hi.x - lo.x, hi.z - lo.z, def.size.0, def.size.2);
    let head = all.iter().filter(|v| v.y > hi.y * 0.9).map(|v| v.x).fold(f32::NEG_INFINITY, f32::max);
    assert!(head < lo.x + 0.25, "the headboard reaches x {head:.3}; the bed's -X end is {:.3}", lo.x);
}
