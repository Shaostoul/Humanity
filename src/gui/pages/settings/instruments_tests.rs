// Child of settings.rs: Settings > Audio > Instruments, drawn headless (an egui context with no
// window) and its text read back, so each test judges what a person sees. What each saw red first
// is at the test.

use super::instruments::draw_instruments;
use crate::gui::theme::load_theme;
use crate::instrument_packs::catalog::{PackCatalog, Parts};
use crate::instrument_packs::install::{self, Failure, InstalledPack, Stage};
use crate::instrument_packs::ui::PacksUi;
use humanity_core::i18n::{t, t_dyn, tf};

fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() {
                    out.push((t.galley.text().to_string(), rect));
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, cs.clip_rect, &mut found);
    }
    found
}

fn draw(packs: &mut PacksUi, frames: Vec<Vec<egui::Event>>) -> Vec<(String, egui::Rect)> {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    let mut last = None;
    for events in frames.into_iter().chain((0..3).map(|_| Vec::new())) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 6000.0))),
            events,
            ..Default::default()
        };
        last = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_instruments(ui, &theme, crate::gui::HintDisplay::Full, theme.info(), packs));
        }));
    }
    texts(&last.expect("frames ran"))
}

fn all(t: &[(String, egui::Rect)]) -> String {
    t.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join("\n")
}

fn click_on(t: &[(String, egui::Rect)], label: &str) -> Vec<Vec<egui::Event>> {
    let at = t.iter().find(|(s, _)| s == label).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("{label:?} is drawn"));
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    vec![vec![egui::Event::PointerMoved(at)], vec![button(true)], vec![button(false)]]
}

const CATALOG: &str = r#"PackCatalog(
    release: "assets-instruments-1",
    base_url: "https://github.com/Shaostoul/Humanity/releases/download/assets-instruments-1/",
    core: (words: "instrument_packs.pack.core", instruments: [(id: "upright_piano_small", part: "piano")], bytes: 9961472),
    packs: [
        (id: "keys", version: 1, family: "keys", words: "instrument_packs.pack.keys", file: "humanity-instruments-keys-v1.tar",
         bytes: 104857600, sha256: "e8850bb4d9ae0d18e549dc9cc496c6486f9a508bd9d160f0b97fc72141134bec",
         licence: "instrument_packs.licence.cc0", instruments: [(id: "grand_piano_kawai", part: "piano")]),
        (id: "percussion", version: 1, family: "percussion", words: "instrument_packs.pack.percussion", file: "humanity-instruments-percussion-v1.tar",
         bytes: 52428800, sha256: "e8850bb4d9ae0d18e549dc9cc496c6486f9a508bd9d160f0b97fc72141134bec",
         licence: "instrument_packs.licence.cc0", instruments: [(id: "bass_drum", part: "deep_drum")]),
    ],
)"#;

fn packs() -> PacksUi {
    let parts_path = std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data");
    PacksUi {
        loaded: true,
        catalog: Some(PackCatalog::parse(CATALOG).expect("the test catalog")),
        parts: Some(Parts::load(&parts_path).expect("parts.ron")),
        ..Default::default()
    }
}

/// SETTINGS LISTS THE PACKS, THEIR SIZES AND LICENCES: the set that comes with the app (always
/// installed), then each pack by name with what it holds, its size, its licence in plain words,
/// whether it is installed, a Download button, and what it would play and what plays that part now
/// (with nothing installed, the music's own synthesised voice).
///
/// Seen red 2026-11-06 with the size line left out of `draw_pack`: "the keys pack shows its size"
/// (the text had no "Size: 100.0 MB" line).
#[test]
fn settings_lists_packs_sizes_and_licences() {
    let mut p = packs();
    let shown = draw(&mut p, vec![vec![]]);
    let text = all(&shown);
    for want in [
        t("instrument_packs.title").to_string(),
        t("instrument_packs.pack.core").to_string(),
        t("instrument_packs.core_status").to_string(),
        t("instrument_packs.pack.keys").to_string(),
        t("instrument_packs.pack.percussion").to_string(),
        tf("instrument_packs.holds", &[("list", &t("instrument_packs.instrument.grand_piano_kawai"))]),
        tf("instrument_packs.licence_label", &[("words", &humanity_core::i18n::plain(humanity_core::i18n::t_marked("instrument_packs.licence.cc0")))]),
        t("instrument_packs.not_installed").to_string(),
        t("instrument_packs.fallback_note").to_string(),
    ] {
        assert!(text.contains(&want), "{want:?} is shown in:\n{text}");
    }
    assert!(text.contains(&tf("instrument_packs.size", &[("mb", &"100.0")])), "the keys pack shows its size:\n{text}");
    assert!(text.contains(&tf("instrument_packs.size", &[("mb", &"50.0")])), "the percussion pack shows its size");
    assert!(text.contains(&tf("instrument_packs.size", &[("mb", &"9.5")])), "the core set shows its size");
    assert_eq!(shown.iter().filter(|(s, _)| s == t("instrument_packs.download")).count(), 2, "a Download button per pack");
    let would = tf("instrument_packs.use.would_play", &[
        ("instrument", &t("instrument_packs.instrument.bass_drum")),
        ("part", &t("instrument_packs.part.deep_drum")),
        ("now", &t("instrument_packs.synth_voice")),
    ]);
    assert!(text.contains(&would), "the drum would play the deep drum part, now synthesised:\n{text}");
}

/// Download asks for that pack; an installed pack shows its size on disk and Remove; a running
/// download shows its progress instead of the buttons; a damaged one says so and offers Try again.
///
/// Seen red 2026-11-06 with the request set to the first pack whatever was clicked: "Download asks
/// for the percussion pack" (left: Some("keys"), right: Some("percussion")).
#[test]
fn download_remove_progress_and_failure_are_shown() {
    let mut p = packs();
    let shown = draw(&mut p, vec![vec![]]);
    // The second Download button is the percussion pack's.
    let mut buttons: Vec<egui::Rect> = shown.iter().filter(|(s, _)| s == t("instrument_packs.download")).map(|(_, r)| *r).collect();
    buttons.sort_by(|a, b| a.top().total_cmp(&b.top()));
    let at = buttons[1].center();
    let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    draw(&mut p, vec![vec![egui::Event::PointerMoved(at)], vec![click(true)], vec![click(false)]]);
    assert_eq!(p.request_download.as_deref(), Some("percussion"), "Download asks for the percussion pack");

    let mut p = packs();
    p.installed.insert("keys".into(), InstalledPack { version: 1, bytes: 99 * 1_048_576 });
    let shown = draw(&mut p, vec![vec![]]);
    let text = all(&shown);
    assert!(text.contains(&tf("instrument_packs.installed", &[("mb", &"99.0")])), "installed, with its size on disk:\n{text}");
    let shown = draw(&mut p, click_on(&shown, t("instrument_packs.remove")));
    assert_eq!(p.request_remove.as_deref(), Some("keys"), "Remove asks to remove it");
    let _ = shown;

    let mut p = packs();
    let progress = install::new_progress();
    {
        let mut g = progress.lock().unwrap();
        g.done = 50 * 1_048_576;
        g.total = 100 * 1_048_576;
        g.stage = Stage::Downloading;
    }
    p.download = Some(("keys".into(), progress.clone()));
    let text = all(&draw(&mut p, vec![vec![]]));
    assert!(text.contains(&tf("instrument_packs.stage.downloading", &[("done", &"50.0"), ("total", &"100.0")])), "its progress:\n{text}");

    progress.lock().unwrap().stage = Stage::Failed(Failure::Damaged("checksum".into()));
    let shown = draw(&mut p, vec![vec![]]);
    let text = all(&shown);
    assert!(text.contains(&tf("instrument_packs.failed.damaged", &[("detail", &"checksum")])), "a damaged download says so:\n{text}");
    assert!(shown.iter().any(|(s, _)| s == t("instrument_packs.retry")), "and offers Try again");
}

/// A PACK SAYS WHAT IT ADDS, AND A SET OF WAVES IS COUNTED (2026-11-13, the brief: "each pack's size and
/// what it adds said in words"): a pack's `about` sentence shows under its name, a set's wave sets show
/// as a count with what they are for (the wavetable instruments), and a pack of waves alone has no
/// empty "Holds:" line. Seen red 2026-11-13 with `draw_pack` before the about and waves lines (its HEAD~ version copied
/// over it, the test run, put back): "the synth waves pack says what it adds".
#[test]
fn a_pack_says_what_it_adds_and_counts_its_waves() {
    let catalog = r#"PackCatalog(
    release: "assets-instruments-1",
    base_url: "https://github.com/Shaostoul/Humanity/releases/download/assets-instruments-1/",
    core: (words: "instrument_packs.pack.core", instruments: [(id: "upright_piano_small", part: "piano")], waves: [(id: "akwf_core", count: 207)], bytes: 9961472),
    packs: [
        (id: "synth_waves", version: 1, family: "synth", words: "instrument_packs.pack.synth_waves", about: Some("instrument_packs.pack_about.synth_waves"),
         file: "humanity-instruments-synth_waves-v1.tar", bytes: 9000000, sha256: "e8850bb4d9ae0d18e549dc9cc496c6486f9a508bd9d160f0b97fc72141134bec",
         licence: "instrument_packs.licence.cc0", credits: [], instruments: [], waves: [(id: "akwf_all", count: 4358)]),
    ],
)"#;
    let mut p = packs();
    p.catalog = Some(PackCatalog::parse(catalog).expect("the test catalog"));
    let text = all(&draw(&mut p, vec![vec![]]));
    assert!(text.contains(t("instrument_packs.pack_about.synth_waves")), "the synth waves pack says what it adds:\n{text}");
    let held = |n: &str, set: &str| tf("instrument_packs.waves_held", &[("count", &n), ("name", &t_dyn(&format!("instrument_packs.waveset.{set}")))]);
    assert!(text.contains(&held("207", "akwf_core")), "the core set counts its waves:\n{text}");
    assert!(text.contains(&held("4358", "akwf_all")), "the pack counts its waves:\n{text}");
    let empty = tf("instrument_packs.holds", &[("list", &"")]);
    assert!(!text.lines().any(|l| l.trim() == empty.trim()), "no empty Holds line for a pack of waves alone:\n{text}");
}

/// EVERY REAL PACK IS LISTED WITH WHAT IT ADDS AND ITS SIZE (2026-11-13, the full packs): drawn from
/// the shipped data/music/packs.ron, Settings shows each pack's name, its about sentence and its
/// size; a pack of many instruments (the full percussion pack holds hundreds) says how many it holds
/// and keeps the list of them, and what each would play, folded until opened, so the page stays a
/// page. RED first (2026-11-13), against the packs.ron written before the full packs were built:
/// "pack keys says what it adds" (the first packs had no about line); and with the folded list opened
/// by default (a mutant, put back): "pack keys's list is folded until opened".
#[test]
fn every_real_pack_shows_what_it_adds_and_its_size() {
    let data = std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data");
    let catalog = PackCatalog::load(&data).expect("data/music/packs.ron");
    let mut p = packs();
    p.catalog = Some(catalog.clone());
    let text = all(&draw(&mut p, vec![vec![]]));
    for pack in &catalog.packs {
        assert!(text.contains(t_dyn(&pack.words)), "pack {} is listed by name:\n{text}", pack.id);
        let about = pack.about.as_deref().unwrap_or_else(|| panic!("pack {} says what it adds", pack.id));
        assert!(text.contains(t_dyn(about)), "pack {}'s about line is shown", pack.id);
        let size = tf("instrument_packs.size", &[("mb", &super::instruments::mb(pack.bytes))]);
        assert!(text.contains(&size), "pack {} shows its size ({size})", pack.id);
        if pack.instruments.len() > super::instruments::LIST_OPEN_MAX {
            let count = tf("instrument_packs.holds_count", &[("count", &pack.instruments.len().to_string())]);
            assert!(text.contains(&count), "pack {} says how many instruments it holds ({count})", pack.id);
            // Each line of the list starts "<instrument>: " (use.would_play and the rest).
            let first = format!("{}: ", t_dyn(&format!("instrument_packs.instrument.{}", pack.instruments[0].id)));
            assert!(!text.lines().any(|l| l.trim_start().starts_with(&first)), "pack {}'s list is folded until opened", pack.id);
        }
    }
}
