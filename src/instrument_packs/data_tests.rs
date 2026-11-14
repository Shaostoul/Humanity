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
    for r in music.moods.iter().flat_map(|m| m.roles.iter()).chain(music.genres.iter().flat_map(|s| s.roles_add.iter())) {
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
    // The core set's wave sets count toward the same budget (2026-11-13, AKWF).
    fn tree_bytes(p: &std::path::Path) -> u64 {
        std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| if e.path().is_dir() { tree_bytes(&e.path()) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }).sum()).unwrap_or(0)
    }
    for w in &catalog.core.waves {
        core_bytes += tree_bytes(&super::waves::WaveSet::dir(&data.join("music"), &w.id));
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

/// A CREDITED PACK CARRIES ITS CREDIT (2026-11-13, the operator: "credit-only CC BY libraries allowed
/// in OPTIONAL packs, never in the core set"). Every pack whose licence is not public domain names
/// the rows of `data/credits.ron` its licence obliges the app to show, and each row exists, is marked
/// attribution-required, and is shown on the Credits page and on the pack's own row in Settings >
/// Audio > Instruments (whose licence words name the makers). The core set stays public domain: every
/// core instrument's licence words are a public-domain key. RED first (2026-11-13), with the choir and
/// electric piano packs in packs.ron before their credits.ron rows were written: "pack electric_pianos:
/// its credit greg_sullivan_epianos is not a row of data/credits.ron".
#[test]
fn credited_packs_carry_their_credits_and_the_core_stays_public_domain() {
    const PUBLIC_DOMAIN: [&str; 3] = ["instrument_packs.licence.cc0", "instrument_packs.licence.unlicense", "instrument_packs.licence.cc0_unlicense"];
    let data = data();
    let catalog = PackCatalog::load(&data).expect("packs.ron");
    let credits = humanity_core::credits::Credits::load(&data);
    assert!(!credits.sources.is_empty(), "data/credits.ron loads");
    for p in &catalog.packs {
        let public = PUBLIC_DOMAIN.contains(&p.licence.as_str());
        assert!(public || !p.credits.is_empty(), "pack {}: its licence {} is not public domain, so it names its credits", p.id, p.licence);
        for id in &p.credits {
            let row = credits.get(id).unwrap_or_else(|| panic!("pack {}: its credit {id} is not a row of data/credits.ron", p.id));
            assert!(row.attribution_required, "pack {}: the credit {id} is marked attribution_required", p.id);
            assert!(row.shown_in.iter().any(|s| s == "Credits"), "pack {}: the credit {id} is shown on the Credits page", p.id);
            assert!(row.shown_in.iter().any(|s| s.starts_with("Settings > Audio > Instruments")), "pack {}: the credit {id} names the pack's row in Settings > Audio > Instruments", p.id);
        }
    }
    let lib = InstrumentLibrary::load_roots(&[(data.join("music"), None)]);
    for a in &lib.instruments {
        assert!(PUBLIC_DOMAIN.contains(&a.def.licence.words.as_str()), "core instrument {} is public domain, not {}", a.def.id, a.def.licence.words);
    }
}

/// THE CORE SET'S WAVE SETS (2026-11-13, AKWF): each one the catalog lists is in data/music/waves with
/// its index, its licence text and every wave it lists, each a plain PCM WAV of the length the index
/// says (a lossy codec would smear a single cycle), public domain, and counted in the 30 MB budget
/// (the core size check above adds the waves folders). RED first (2026-11-13) with the catalog
/// listing akwf_core before the pipeline had written it: "...data/music/waves/akwf_core/waves.ron: The
/// system cannot find the path specified. (os error 3)" (the folder moved aside, the test run, put back).
#[test]
fn the_core_wave_sets_are_whole_plain_pcm_and_public_domain() {
    let data = data();
    let catalog = PackCatalog::load(&data).expect("packs.ron");
    let music = data.join("music");
    for w in &catalog.core.waves {
        let set = super::waves::WaveSet::load(&music, &w.id).unwrap_or_else(|e| panic!("{e}"));
        let dir = super::waves::WaveSet::dir(&music, &w.id);
        assert_eq!(set.waves.len() as u32, w.count, "{}: the catalog's count is the index's", w.id);
        assert!(has_words(&set.words) && has_words(&set.licence.words), "{}: its name and licence words are in en.json", w.id);
        assert_eq!(set.licence.words, "instrument_packs.licence.cc0", "{}: public domain", w.id);
        assert!(dir.join(&set.licence.file).is_file(), "{}: its licence text is beside it", w.id);
        for wave in &set.waves {
            let bytes = std::fs::read(dir.join(&wave.file)).unwrap_or_else(|e| panic!("{}: {}: {e}", w.id, wave.file));
            assert!(bytes.len() > 44 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE", "{}: {} is a WAV", w.id, wave.file);
            let data_bytes = u64::from(wave.frames) * u64::from(wave.channels) * u64::from(wave.bits / 8);
            assert!(bytes.len() as u64 >= data_bytes + 44, "{}: {} holds its {} frames", w.id, wave.file, wave.frames);
        }
    }
}

/// EVERY PART THE INSTRUMENTS LIST IS PLAYED BY THE MUSIC OR SAYS IT IS NOT YET (BUG-266): each part
/// `parts.ron` and `packs.ron` list is named by a mood's role or a genre's added role or palette
/// (`score::named_parts`, what the score attaches recordings to), unless `parts.ron` marks it
/// `not_used_yet` (Settings > Audio > Instruments then says the music does not use it yet); a
/// marked part the music does name fails too, so the mark cannot go stale; and a `fallback_part` is a
/// part. RED first (2026-11-07) against the data before the fix (only harp, steel_pan and deep_drum
/// named, no marks): "packs.ron and parts.ron list parts no mood or genre plays, unmarked: [\"bass_strings\",
/// \"bowed_vibraphone\", ...]".
#[test]
fn every_part_listed_is_played_or_marked_not_used_yet() {
    let data = data();
    let parts = Parts::load(&data).expect("parts.ron");
    let catalog = PackCatalog::load(&data).expect("packs.ron");
    let music = humanity_music::MusicLibrary::load(&data).expect("the music's data loads");
    let named: BTreeSet<String> = super::score::named_parts(&music).into_iter().map(|(p, _, _)| p).collect();
    let listed: BTreeSet<String> = parts
        .parts
        .iter()
        .map(|p| p.id.clone())
        .chain(catalog.core.instruments.iter().chain(catalog.packs.iter().flat_map(|p| p.instruments.iter())).map(|r| r.part.clone()))
        .collect();
    let marked = |id: &str| parts.part(id).is_some_and(|p| p.not_used_yet);
    let unmarked: Vec<&String> = listed.iter().filter(|p| !named.contains(*p) && !marked(p)).collect();
    assert!(unmarked.is_empty(), "packs.ron and parts.ron list parts no mood or genre plays, unmarked: {unmarked:?}");
    let stale: Vec<&String> = listed.iter().filter(|p| named.contains(*p) && marked(p)).collect();
    assert!(stale.is_empty(), "parts marked not_used_yet that the music does play: {stale:?}");
    for p in &parts.parts {
        if let Some(f) = &p.fallback_part {
            assert!(parts.part(f).is_some() && f != &p.id, "part {}: its fallback_part {f} is another part", p.id);
        }
    }
}

/// A SUSTAINED PART'S RECORDINGS ARE HELD PAST THEIR END (BUG-266): the core set's violins have no
/// loop points of their own (8 s recordings) and a calm chord lasts up to 13 s, so attached to the
/// score each zone gets a crossfaded loop over its middle (`score::soft_loop`), while a struck
/// bell's recordings are left to ring out. Only those two parts are attached here (the rest of the
/// music's parts set aside), to keep the decoding short. RED first (2026-11-07) with `attach` giving
/// no zone a loop of its own making: "the violins' zone for middle C loops (None, 0)".
#[test]
fn sustained_recordings_get_a_crossfaded_loop() {
    let data = data();
    let mut music = humanity_music::MusicLibrary::load(&data).expect("the music");
    for r in music.moods.iter_mut().flat_map(|m| m.roles.iter_mut()).chain(music.genres.iter_mut().flat_map(|s| s.roles_add.iter_mut())) {
        if !matches!(r.part.as_deref(), Some("strings_high" | "glockenspiel")) {
            r.part = None;
        }
    }
    for e in music.genres.iter_mut().flat_map(|s| s.palette.iter_mut()) {
        e.part = None;
    }
    let parts = Parts::load(&data).expect("parts.ron");
    let installed = InstrumentLibrary::load_roots(&[(data.join("music"), None)]);
    let attached = super::score::attach(&mut music, &installed, &parts, 432.0, &mut super::samples::SampleBank::new(), None);
    assert!(attached.problems.is_empty(), "{:?}", attached.problems);
    let zone = |id: &str, midi: i32| {
        let b = attached.samples.get(&super::score::recorded_id(id)).unwrap_or_else(|| panic!("{id} is attached"));
        let z = b.zone(b.pick(midi, 0.8, 0).expect("a zone"));
        (z.loop_range, z.loop_xfade, z.frames.len())
    };
    let (range, xfade, len) = zone("violins_small", 60);
    assert!(range.is_some() && xfade > 0, "the violins' zone for middle C loops ({range:?}, {xfade})");
    let (a, b) = range.unwrap();
    assert!(a >= xfade && b > a + xfade && b <= len, "inside the recording: {a}..{b} of {len}, faded over {xfade}");
    let (range, xfade, _) = zone("glockenspiel_small", 84);
    assert!(range.is_none() && xfade == 0, "a struck bell rings out: {range:?}");
}

/// The libraries whose whole recordings the full packs hold (2026-11-13, the operator: "I'm
/// interested in having all the instruments"), by the name their definitions carry.
const FULL_LIBRARIES: &[&str] = &[
    "VSCO 2 Community Edition",
    "Versilian Community Sample Library (VCSL)",
    "Karoryfer Black And Green Guitars",
    "FreePats Upright Piano KW (small set)",
    "jSteelDrum (version 1), a C tenor steel pan",
    "Martin HD-28 steel-string guitar (Discord SFZ GM Bank, 026 Acoustic Guitar (steel))",
];

/// A FULL PACK HOLDS MORE THAN THE CORE SET'S CUT (2026-11-13). The core set keeps a small cut of a
/// recording (one loudness layer, one take, every second or third key, a few seconds of each note);
/// the full packs keep every velocity layer and round robin the library has, up to 30 s of each
/// sample. So every core instrument made from those libraries has its whole recording in a pack
/// (`full_of` in packs.ron), and that pack's instrument has more layers, more round robins or more
/// samples than the core one, or, where the core set already kept every sample (the Martin's 15
/// notes, the tenor saxophone, the wine glasses), more seconds of them. Every pack instrument also
/// carries what it holds (the pipeline writes it from the definition it built). RED first
/// (2026-11-13), against the packs.ron written before the full packs were built: "bass_drum_small
/// (Versilian Community Sample Library (VCSL)): no pack holds its whole recording (full_of)".
#[test]
fn full_packs_hold_more_than_the_core_cut() {
    let data = data();
    let catalog = PackCatalog::load(&data).expect("packs.ron");
    let core = InstrumentLibrary::load_roots(&[(data.join("music"), None)]);
    let pack_refs: Vec<_> = catalog.packs.iter().flat_map(|p| p.instruments.iter().map(move |r| (p, r))).collect();
    for a in &core.instruments {
        if !FULL_LIBRARIES.contains(&a.def.source.library.as_str()) {
            continue;
        }
        let id = &a.def.id;
        let Some((pack, full)) = pack_refs.iter().find(|(_, r)| r.full_of.as_deref() == Some(id.as_str())) else {
            panic!("{id} ({}): no pack holds its whole recording (full_of)", a.def.source.library);
        };
        let zones = &a.sampler().zones;
        let core_layers = {
            let mut by_key: std::collections::BTreeMap<(u8, u8), BTreeSet<(u8, u8)>> = Default::default();
            for z in zones {
                by_key.entry((z.lo_key, z.hi_key)).or_default().insert((z.lo_vel, z.hi_vel));
            }
            by_key.values().map(|s| s.len() as u32).max().unwrap_or(0)
        };
        let core_rr = zones.iter().map(|z| z.rr as u32).max().unwrap_or(0);
        let core_zones = zones.len() as u32;
        let more = full.layers > core_layers || full.round_robins > core_rr || full.zones > core_zones;
        if !more {
            // The core set kept every sample: the pack's are longer.
            let core_seconds: f32 = zones
                .iter()
                .map(|z| {
                    let (rate, frames) = humanity_music::decode::decode_file(&a.samples_dir.join(&z.file)).expect("a core sample decodes");
                    frames.len() as f32 / rate as f32
                })
                .sum();
            // Strictly more, beyond rounding: the tails the core set cut, given back (the Martin's
            // 15 notes are short already, so its whole recording is about a second longer).
            assert!(
                full.zones == core_zones && full.seconds > core_seconds + 0.5,
                "{id}: pack {}'s {} keeps no more than the core cut ({} layers, {} round robins, {} samples, {:.1} s against {core_layers}, {core_rr}, {core_zones}, {core_seconds:.1} s)",
                pack.id,
                full.id,
                full.layers,
                full.round_robins,
                full.zones,
                full.seconds
            );
        }
    }
    for (pack, r) in &pack_refs {
        assert!(r.layers > 0 && r.round_robins > 0 && r.zones > 0 && r.seconds > 0.0, "pack {}: {} says what it holds", pack.id, r.id);
        if let Some(f) = &r.full_of {
            assert!(catalog.core.instruments.iter().any(|c| &c.id == f), "pack {}: {} is the whole recording of {f}, a core instrument", pack.id, r.id);
        }
    }
}
