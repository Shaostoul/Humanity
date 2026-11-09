//! The model packs: the shipped catalog agrees with what ships and with the catalogue rows, and a
//! model pack installs through the shared pack installer (a tar built here, never the internet).
//! What each test saw red first is at the test.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::catalog::{packs_dir, ModelPack, ModelPackCatalog};
use crate::instrument_packs::install::{self, Failure};

fn repo() -> PathBuf {
    PathBuf::from(env!("HUMANITY_REPO_ROOT"))
}

/// A plain ustar archive of `files` (name, bytes), the shape `instrument_packs::tar` reads.
fn tar_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, data) in files {
        let mut h = [0u8; 512];
        h[..name.len()].copy_from_slice(name.as_bytes());
        h[100..108].copy_from_slice(b"0000644\0");
        h[108..116].copy_from_slice(b"0000000\0");
        h[116..124].copy_from_slice(b"0000000\0");
        h[124..136].copy_from_slice(format!("{:011o}\0", data.len()).as_bytes());
        h[136..148].copy_from_slice(b"00000000000\0");
        h[148..156].copy_from_slice(b"        ");
        h[156] = b'0';
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        let sum: u32 = h.iter().map(|&b| b as u32).sum();
        h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        out.extend_from_slice(&h);
        out.extend_from_slice(data);
        out.resize(out.len() + (512 - data.len() % 512) % 512, 0);
    }
    out.resize(out.len() + 1024, 0);
    out
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("hos-model-packs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("a scratch folder");
    d
}

fn pack_for(tar: &[u8]) -> ModelPack {
    ModelPack {
        id: "fixture-models".to_string(),
        version: 1,
        words: "model_packs.pack.simready_medical".to_string(),
        file: "fixture.tar".to_string(),
        bytes: tar.len() as u64,
        sha256: hex::encode(Sha256::digest(tar)),
        licence: "model_packs.licence.cc_by".to_string(),
        models: vec!["box_a01".to_string()],
    }
}

/// A MODEL PACK INSTALLS THROUGH THE SHARED INSTALLER, AND A DAMAGED ONE IS REFUSED: the archive's
/// files land in `<data>/models/packs/<id>/`, listed as installed at the pack's version; one changed
/// byte is refused as damaged with nothing installed. Seen red 2026-11-09 with `install_entry`
/// giving the installer version 0 (the manifest check refused the pack as "not a pack of this
/// catalog").
#[test]
fn a_model_pack_installs_and_a_damaged_one_is_refused() {
    let tar = tar_of(&[("pack.ron", b"(id: \"fixture-models\", version: 1, instruments: [])\n"), ("box_a01/box_a01.glb", b"not really a model")]);
    let pack = pack_for(&tar);
    let data = scratch("install");
    let dir = packs_dir(&data);
    install::install_from_reader(&mut tar.as_slice(), &pack.install_entry(), &dir, &install::new_progress()).expect("the pack installs");
    assert!(dir.join("fixture-models").join("box_a01").join("box_a01.glb").exists(), "its model is where a catalogue row names it");
    assert_eq!(install::installed_packs(&dir).get("fixture-models").map(|p| p.version), Some(1), "listed as installed at its version");
    install::remove_pack(&dir, "fixture-models").expect("removed");
    let mut bad = tar.clone();
    bad[600] ^= 1;
    let got = install::install_from_reader(&mut bad.as_slice(), &pack.install_entry(), &dir, &install::new_progress());
    assert!(matches!(got, Err(Failure::Damaged(_))), "a changed byte is refused: {got:?}");
    assert!(install::installed_packs(&dir).is_empty(), "nothing installed after a refusal");
    let _ = std::fs::remove_dir_all(&data);
}

fn simready_dirs() -> Vec<String> {
    let root = repo().join("assets").join("models").join("simready");
    let mut v: Vec<String> = std::fs::read_dir(&root)
        .map(|r| r.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// THE SHIPPED CATALOG AGREES WITH WHAT SHIPS AND WITH THE ROWS: it parses; every pack is built
/// (a size and a SHA-256 from the same run as its archive); the in-app count is the number of model
/// folders in `assets/models/simready/`; and every model of every pack is named by a catalogue row
/// as `models/packs/<id>/<model>/<model>.glb`, so installing the pack changes what is drawn. Seen red
/// 2026-11-09 with the core count one higher than the folders ("the in-app count is the folders
/// that ship": 41 against 40) and, alone, with the IV pole's row pointing at the in-app folder
/// instead of the pack ("a catalogue row draws models/packs/simready-medical/iv_pole_a01/...").
#[test]
fn the_shipped_catalog_agrees_with_what_ships_and_the_rows() {
    let catalog = ModelPackCatalog::load(&repo().join("data")).expect("data/models/packs.ron reads");
    assert_eq!(catalog.core.models as usize, simready_dirs().len(), "the in-app count is the folders that ship");
    let rows = std::fs::read_to_string(repo().join("data").join("machines").join("home.ron")).expect("home.ron");
    for pack in &catalog.packs {
        assert!(pack.is_built(), "pack {} has a size and a SHA-256", pack.id);
        assert!(!pack.models.is_empty(), "pack {} holds models", pack.id);
        for m in &pack.models {
            let path = format!("models/packs/{}/{m}/{m}.glb", pack.id);
            assert!(rows.contains(&format!("\"{path}\"")), "a catalogue row draws {path}");
        }
    }
    assert!(Path::new(&repo().join("data").join("models").join("packs.ron")).exists());
}
