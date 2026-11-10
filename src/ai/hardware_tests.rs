//! The hardware notices of section 4.7 against made-up computers, the real reading of this one, and
//! the shipped data/ai/choices.ron.

use std::path::PathBuf;

use super::choices::{load, AiChoices, Choice, Needs};
use super::hardware::{check, read, Gpu, Hardware, Notice};

fn plenty() -> Hardware {
    Hardware {
        memory_total_mb: Some(32 * 1024),
        memory_free_mb: Some(20 * 1024),
        pretending: false,
        cores: Some(6),
        threads: 12,
        avx2: Some(true),
        avx512: false,
        gpu: Some(Gpu { name: "Test Card".into(), integrated: false, vram_mb: Some(12 * 1024) }),
        disk_free_mb: Some(500 * 1024),
        folder: Some(PathBuf::from("C:\\Users\\x\\AppData\\Roaming\\HumanityOS")),
    }
}

fn choice(memory_mb: u64, cores: u32, avx2: bool, vram_mb: Option<u64>) -> Choice {
    Choice {
        id: "c".into(),
        feature: "characters".into(),
        words: "ai.choice.characters_rich".into(),
        model: "M".into(),
        maker: "K".into(),
        needs: Needs { memory_mb, cores, disk_mb: 2200, avx2, vram_mb },
        fallback: None,
        rung: 3,
    }
}

/// A computer with plenty has what each choice needs.
#[test]
fn plenty_fits() {
    assert_eq!(check(&choice(2900, 4, true, None), &plenty()), vec![Notice::Fits]);
}

/// Each of 4.7's situations, alone, is the one notice given, with its numbers. Seen red 2026-11-09 with
/// the memory comparison reversed (`free > need`): the plenty computer got the memory notice and the
/// 1 GB one got "Fits".
#[test]
fn each_shortfall_is_named_with_its_numbers() {
    let mut low = plenty();
    low.memory_free_mb = Some(1024);
    assert_eq!(check(&choice(2900, 4, true, None), &low), vec![Notice::Memory { need_mb: 2900, free_mb: 1024, total_mb: Some(32 * 1024) }]);

    let mut two = plenty();
    two.cores = Some(2);
    assert_eq!(check(&choice(400, 4, false, None), &two), vec![Notice::Cores { need: 4, have: 2 }]);

    let mut old = plenty();
    old.avx2 = Some(false);
    assert_eq!(check(&choice(1600, 4, true, None), &old), vec![Notice::NoAvx2]);
    assert_eq!(check(&choice(400, 4, false, None), &old), vec![Notice::Fits], "a choice that does not need AVX2 is not warned");

    let mut arm = plenty();
    arm.avx2 = None;
    assert_eq!(check(&choice(1600, 4, true, None), &arm), vec![Notice::Fits], "another kind of processor is not 'missing' AVX2");

    let mut full = plenty();
    full.disk_free_mb = Some(1000);
    assert!(matches!(&check(&choice(1600, 4, true, None), &full)[..], [Notice::Disk { need_mb: 2200, free_mb: 1000, .. }]));
}

/// The helper's graphics-card route: a card that holds it, one that does not, and a shared-memory card
/// (no line: it has no memory of its own to measure).
#[test]
fn the_graphics_card_route_says_whether_the_card_holds_it() {
    let hw = plenty();
    assert_eq!(check(&choice(5700, 4, true, Some(5700)), &hw), vec![Notice::Fits, Notice::Gpu { name: "Test Card".into(), vram_mb: 12 * 1024, holds: true }]);
    let mut small = plenty();
    small.gpu = Some(Gpu { name: "Small".into(), integrated: false, vram_mb: Some(4096) });
    let n = check(&choice(5700, 4, true, Some(5700)), &small);
    assert_eq!(n[1], Notice::Gpu { name: "Small".into(), vram_mb: 4096, holds: false });
    assert!(n[1].is_warning() && !n[0].is_warning());
    let mut shared = plenty();
    shared.gpu = Some(Gpu { name: "Shared".into(), integrated: true, vram_mb: None });
    assert_eq!(check(&choice(5700, 4, true, Some(5700)), &shared), vec![Notice::Fits]);
}

/// A figure the system did not report is never counted as short.
#[test]
fn unknown_figures_are_not_shortfalls() {
    let hw = Hardware { memory_total_mb: None, memory_free_mb: None, cores: None, disk_free_mb: None, folder: None, gpu: None, ..plenty() };
    assert_eq!(check(&choice(5700, 4, true, Some(5700)), &hw), vec![Notice::Fits]);
}

/// The Dev page's pretend figure replaces the free memory, and says it is pretend.
#[test]
fn the_pretend_figure_replaces_free_memory() {
    super::hardware::set_pretend_free_mb(Some(1536));
    let hw = read();
    super::hardware::set_pretend_free_mb(None);
    assert_eq!(hw.memory_free_mb, Some(1536));
    assert!(hw.pretending);
    assert!(!read().pretending);
}

/// This computer's own reading: threads always; memory and cores on Windows and Linux.
#[test]
fn this_computer_reads() {
    let hw = read();
    assert!(hw.threads >= 1);
    if cfg!(any(windows, target_os = "linux")) {
        let (total, free) = (hw.memory_total_mb.expect("total memory"), hw.memory_free_mb.expect("free memory"));
        assert!(total > 512 && free <= total, "{free} of {total}");
        let cores = hw.cores.expect("physical cores");
        assert!(cores >= 1 && cores <= hw.threads, "{cores} cores, {} threads", hw.threads);
    }
}

/// The shipped file loads, holds a choice for each feature, its fallbacks are lighter choices of the
/// same feature, and every words key it names has its English.
#[test]
fn the_shipped_choices_load_and_have_their_words() {
    let data = PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data");
    let c = load(&data).expect("data/ai/choices.ron loads");
    for f in &c.features {
        assert!(c.choices.iter().any(|x| x.feature == f.id), "{} has a choice", f.id);
    }
    let en: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data.join("i18n/en.json")).unwrap()).unwrap();
    let has = |key: &str| key.split('.').try_fold(&en, |v, part| v.get(part)).is_some_and(|v| v.is_string());
    for f in &c.features {
        for k in [&f.name, &f.about, &f.without] {
            assert!(has(k), "{k} has no words in en.json");
        }
    }
    for x in &c.choices {
        assert!(has(&x.words), "{} has no words in en.json", x.words);
    }
}

/// The file's own checks catch a fallback that is not lighter, a feature not listed, and an id twice.
#[test]
fn the_choices_file_checks_itself() {
    let mut bad = AiChoices { features: vec![], choices: vec![choice(1600, 4, true, None), choice(2900, 4, true, None)] };
    bad.choices[0].fallback = Some("c".into());
    let p = bad.problems();
    assert!(p.iter().any(|x| x.contains("twice")), "{p:?}");
    assert!(p.iter().any(|x| x.contains("which the file does not list")), "{p:?}");
    assert!(p.iter().any(|x| x.contains("needs no less memory")), "{p:?}");
}
