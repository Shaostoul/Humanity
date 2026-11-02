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
    let home = crate::systems::grow_machines::load_home(&repo_root.join("data/machines/home.ron"))
        .expect("the shipped home loads");
    let manager = AssetManager::new(repo_root.join("data"));

    let mut checked = 0usize;
    let mut worst: (f32, String) = (0.0, String::new());
    for (id, def) in home.catalog.iter() {
        let Some(model) = def.model.as_deref() else { continue };
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
