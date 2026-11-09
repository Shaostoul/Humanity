//! From assets/mod.rs (`model_scale_tests`): every modelled machine in the home is drawn at its
//! declared height, the home loaded the way the simulation loads it (`grow_machines::load_home`).

use crate::assets::*;

/// EVERY MODELLED MACHINE IN THE SHIPPED HOME IS DRAWN AT THE SIZE ITS
/// DATA DECLARES.
///
/// The operator, 2026-09-19, standing in his own bedroom: "Seems like
/// either all the furniture is small or I'm just really big." He was
/// right. `size` in data/machines/*.ron only ever built the PRIMITIVE
/// fallback box; a machine that declared a model got that model at
/// whatever scale it was authored at, and this pack is authored at about
/// half real scale. A chair rendered 0.46 m tall, a bookcase 0.88 m and a
/// desk 0.38 m, next to a player whose eyes are at 1.7 m.
///
/// The second assertion is the one that keeps this honest: if every model
/// were already the right height the fit would be a no-op and this test
/// would pass while proving nothing, so it requires that at least one
/// model really was badly wrong as authored.
#[test]
fn every_modelled_machine_in_the_home_is_drawn_at_its_declared_height() {
    let repo_root = std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT"));
    let home = humanity_systems::grow_machines::load_home(&repo_root.join("data/machines/home.ron"))
        .expect("the shipped home loads");
    let manager = AssetManager::new(repo_root.join("data"));

    let mut checked = 0usize;
    let mut worst: (f32, String) = (0.0, String::new());
    for (id, def) in home.catalog.iter() {
        let Some(model) = def.model.as_deref() else { continue };
        // A download pack's model (Settings > Graphics > Furniture models, `model_packs`) is not
        // in the repo until someone installs the pack; until then the row draws its box, as the
        // body build's fallback does. Its row and its pack are checked together by
        // `model_packs::tests::the_shipped_catalog_agrees_with_what_ships_and_the_rows`, and its
        // height by `model_physics`' row-height test once installed. (Seen 2026-11-09: the Medical
        // Bay rows failed here as "did not load" in a tree without the pack.)
        if model.starts_with("models/packs/") && !repo_root.join("data").join(model).exists() {
            continue;
        }
        let target = def.size.1;
        let (mut cpu, _tex) = manager
            .parse_gltf_mesh_with_texture(model)
            .unwrap_or_else(|e| panic!("machine {id}: model {model} did not load: {e}"));
        let authored = cpu.authored_height();
        cpu.scale_to_height(target);
        let drawn = cpu.authored_height();
        assert!(
            (drawn - target).abs() <= target * 0.01,
            "machine {id} declares {target} m tall but would be drawn {drawn} m"
        );
        let err = if target > 0.0 { (authored - target).abs() / target } else { 0.0 };
        if err > worst.0 {
            worst = (err, format!("{id} ({model}): authored {authored} m, declared {target} m"));
        }
        checked += 1;
    }

    assert!(checked >= 10, "only {checked} modelled machines found; this gate is checking almost nothing");
    assert!(
        worst.0 > 0.25,
        "every model was already within 25 percent of its declared height, so fitting is a \
         no-op and this test proves nothing. Worst seen: {}",
        worst.1
    );
}

/// EVERY MODELLED MACHINE IN THE SHIPPED HOME IS DRAWN CENTRED ON ITS PLACEMENT: after the machine
/// fit (`GltfCpuMesh::fit_for_machine`, what both machine paths draw), each model's footprint is
/// centred on its origin within a centimetre, so it stands where its label, pick volume, collision
/// and the Boxes level's box stand. The second assertion keeps it honest: at least one model was
/// authored off-centre, so centring is not a no-op this test cannot see.
///
/// Seen red 2026-11-09 with the fit only scaling (the Kenney furniture is authored from a corner):
/// "machine bed: footprint centred 0.765 m, 0.900 m from its placement".
#[test]
fn every_modelled_machine_in_the_home_is_drawn_centred_on_its_placement() {
    let repo_root = std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT"));
    let home = humanity_systems::grow_machines::load_home(&repo_root.join("data/machines/home.ron"))
        .expect("the shipped home loads");
    let manager = AssetManager::new(repo_root.join("data"));
    let mut checked = 0usize;
    let mut off_centre_as_authored = 0usize;
    for (id, def) in home.catalog.iter() {
        let Some(model) = def.model.as_deref() else { continue };
        // An uninstalled download pack's row draws its box (see the height test above).
        if model.starts_with("models/packs/") && !repo_root.join("data").join(model).exists() {
            continue;
        }
        let (mut cpu, _tex) = manager
            .parse_gltf_mesh_with_texture(model)
            .unwrap_or_else(|e| panic!("machine {id}: model {model} did not load: {e}"));
        let centre = |cpu: &GltfCpuMesh| {
            let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
            for v in &cpu.vertices {
                lo = [lo[0].min(v.position[0]), lo[1].min(v.position[2])];
                hi = [hi[0].max(v.position[0]), hi[1].max(v.position[2])];
            }
            ((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5)
        };
        let (mut as_authored, _) = manager.parse_gltf_mesh_with_texture(model).expect("it loaded just now");
        as_authored.scale_to_height(def.size.1);
        let (ax, az) = centre(&as_authored);
        if ax.abs().max(az.abs()) > 0.05 {
            off_centre_as_authored += 1;
        }
        cpu.fit_for_machine(def.size.1);
        let (cx, cz) = centre(&cpu);
        assert!(
            cx.abs() <= 0.01 && cz.abs() <= 0.01,
            "machine {id}: footprint centred {:.3} m, {:.3} m from its placement",
            cx.abs(),
            cz.abs()
        );
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} modelled machines found");
    assert!(off_centre_as_authored > 0, "no model was authored off-centre, so this test proves nothing");
}
