//! The shipped instrument data (`data/music/`), checked the way `just validate-data` checks every
//! data file: it parses, every core instrument's samples are there with their licence text and
//! their source record (music.md rung 6: "a lint that every sample folder carries its licence text
//! and source"), every part an instrument names exists, every name a person reads has its words,
//! and the core set stays under its 30 MB budget (music.md section 14).
//!
//! Seen red 2026-11-06 with one core sample folder's LICENSE.txt deleted: "concert_harp_small:
//! its sample folder carries the library's licence text".

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::catalog::{PackCatalog, Parts};
use super::definition::Instrument;
use super::library::InstrumentLibrary;

fn data() -> PathBuf {
    PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data")
}

fn has_words(key: &str) -> bool {
    humanity_core::i18n::has(key)
}

#[test]
fn the_instrument_data_parses_and_every_sample_has_its_licence() {
    let data = data();
    let parts = Parts::load(&data).expect("data/music/parts.ron parses");
    let catalog = PackCatalog::load(&data).expect("data/music/packs.ron parses");
    let part_ids: BTreeSet<&str> = parts.parts.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(part_ids.len(), parts.parts.len(), "no part is listed twice");
    for p in &parts.parts {
        assert!(has_words(&p.words), "part {}: its name {} is in en.json", p.id, p.words);
        assert!(["synth", "choir", "modal", "membrane", "fm", "plucked", "noise"].contains(&p.synth.kind.as_str()), "part {}: {} is one of the music engine's synthesised kinds", p.id, p.synth.kind);
    }
    // Each part's synthesised voice is an instrument of the score, of the kind it says, and every
    // part a mood's role names is a part here (2026-11-06, with crates/humanity-music on main).
    let music = humanity_music::MusicLibrary::load(&data).expect("the music's data loads");
    for p in &parts.parts {
        let inst = music.instruments.iter().find(|i| i.id == p.synth.preset);
        let kind = inst.map(|i| format!("{:?}", i.kind).split('(').next().unwrap_or("").to_ascii_lowercase());
        assert_eq!(kind.as_deref(), Some(p.synth.kind.as_str()), "part {}: its voice {} is a {} instrument of the score", p.id, p.synth.preset, p.synth.kind);
    }
    for r in music.moods.iter().flat_map(|m| m.roles.iter()).chain(music.styles.iter().flat_map(|s| s.roles_add.iter())) {
        if let Some(part) = &r.part {
            assert!(part_ids.contains(part.as_str()), "a role names the part {part}, which parts.ron does not list");
        }
    }

    // The core set: every instrument in the catalog is in data/music, loads, and carries its
    // licence and source; nothing in data/music/instruments is a stray.
    let lib = InstrumentLibrary::load_roots(&[(data.join("music"), None)]);
    assert!(lib.problems.is_empty(), "the core set loads: {:?}", lib.problems);
    let listed: BTreeSet<String> = catalog.core.instruments.iter().map(|r| r.id.clone()).collect();
    let found: BTreeSet<String> = lib.instruments.iter().map(|a| a.def.id.clone()).collect();
    assert_eq!(listed, found, "the catalog's core instruments are the ones in data/music");
    let mut core_bytes = 0u64;
    for a in &lib.instruments {
        let id = &a.def.id;
        let s = a.sampler();
        assert!(part_ids.contains(a.def.part.as_str()), "{id}: its part {} is in parts.ron", a.def.part);
        assert!(has_words(&a.def.words), "{id}: its name {} is in en.json", a.def.words);
        assert!(has_words(&a.def.licence.words), "{id}: its licence words {} are in en.json", a.def.licence.words);
        assert!(a.samples_dir.join(&a.def.licence.file).is_file(), "{id}: its sample folder carries the library's licence text");
        assert!(a.samples_dir.join("source.ron").is_file(), "{id}: its sample folder carries its source.ron");
        assert!(!a.def.source.url.is_empty() && !a.def.source.fetched.is_empty() && !a.def.credit.is_empty(), "{id}: source, date and credit");
        assert!(s.source_a4_hz > 400.0 && s.source_a4_hz < 470.0, "{id}: a plausible source tuning {}", s.source_a4_hz);
        for z in &s.zones {
            assert!(z.lo_key <= z.hi_key && z.lo_vel <= z.hi_vel && z.lo_vel >= 1, "{id}: zone {} is ordered", z.file);
            assert_eq!(z.recorded_hz.is_some(), s.pitched, "{id}: zone {} has a recorded pitch exactly when pitched", z.file);
        }
        for e in std::fs::read_dir(&a.samples_dir).unwrap().flatten() {
            core_bytes += e.metadata().unwrap().len();
        }
        let r = catalog.core.instruments.iter().find(|r| &r.id == id).unwrap();
        assert_eq!(r.part, a.def.part, "{id}: the catalog says the part its definition plays");
    }
    assert!(core_bytes < 30 * 1024 * 1024, "the core set is under 30 MB: {core_bytes} bytes");
    for e in std::fs::read_dir(data.join("music").join("instruments")).unwrap().flatten() {
        let def = Instrument::parse(&std::fs::read_to_string(e.path()).unwrap()).expect("every instrument file parses");
        if def.sampler().is_some() {
            assert!(listed.contains(&def.id), "{} is a sampled instrument the catalog does not list", def.id);
        }
    }

    // The packs: every one built (a size and a checksum to check downloads against), its words
    // there, its instruments' parts real, no instrument in two places.
    assert!(!catalog.packs.is_empty());
    let mut seen = listed.clone();
    for p in &catalog.packs {
        assert!(p.is_built(), "pack {} is built: a size and a SHA-256", p.id);
        assert!(p.file.ends_with(".tar") && p.url(&catalog.base_url).starts_with("https://github.com/Shaostoul/Humanity/releases/download/"), "pack {}: a release asset", p.id);
        assert!(has_words(&p.words) && has_words(&p.licence), "pack {}: its name and licence words", p.id);
        for r in &p.instruments {
            assert!(part_ids.contains(r.part.as_str()), "pack {}: {}'s part {} is in parts.ron", p.id, r.id, r.part);
            assert!(has_words(&format!("instrument_packs.instrument.{}", r.id)), "pack {}: {} has its name in en.json", p.id, r.id);
            assert!(seen.insert(r.id.clone()), "{} is in two sets", r.id);
        }
    }
    for r in &catalog.core.instruments {
        assert!(has_words(&format!("instrument_packs.instrument.{}", r.id)), "{} has its name in en.json", r.id);
    }
}
