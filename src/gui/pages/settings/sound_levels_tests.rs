// Child of pages/settings/sound_levels.rs (#[path]): Settings > Audio > Sound levels, drawn
// headlessly the way the page draws it, against the repo's own data and sound files. Each test on
// its own thread's levels (`sound_levels::use_on_this_thread`). Every test was seen failing on
// purpose once; the break and what it said are at the test.

use super::*;
use std::collections::BTreeMap;

fn repo() -> &'static Path {
    Path::new(env!("HUMANITY_REPO_ROOT"))
}

/// The section's state, reading the repo's data and sound files.
fn section() -> SoundLevelsUi {
    SoundLevelsUi::reading(repo().join("data"), repo().join("assets"))
}

fn headless() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

/// One headless frame of the section as the Settings page draws it (`draw_kept`, its state in
/// egui's memory, handed in from `s` and read back into it), tall enough that every row is on
/// screen; its output and whether a level changed.
fn frame(ctx: &egui::Context, theme: &Theme, s: &mut SoundLevelsUi, events: Vec<egui::Event>) -> (egui::FullOutput, bool) {
    let id = egui::Id::new(KEPT_ID);
    ctx.data_mut(|d| d.insert_temp(id, s.clone()));
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 2600.0))), events, ..Default::default() };
    let mut dirty = false;
    let out = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| dirty |= draw_kept(ui, theme, HintDisplay::Full, theme.accent()));
    });
    *s = ctx.data(|d| d.get_temp::<SoundLevelsUi>(id)).expect("the section keeps its state in egui's memory");
    (out, dirty)
}

/// Two quiet frames (egui lays out on the first), the second's output.
fn settled(ctx: &egui::Context, theme: &Theme, s: &mut SoundLevelsUi) -> egui::FullOutput {
    frame(ctx, theme, s, Vec::new());
    frame(ctx, theme, s, Vec::new()).0
}

/// Click at `pos` the way a person does (move, press, release: three frames). True when a level
/// changed on any of them.
fn click_at(ctx: &egui::Context, theme: &Theme, s: &mut SoundLevelsUi, pos: egui::Pos2) -> bool {
    let m = egui::Modifiers::default();
    let a = frame(ctx, theme, s, vec![egui::Event::PointerMoved(pos)]).1;
    let b = frame(ctx, theme, s, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]).1;
    let c = frame(ctx, theme, s, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]).1;
    a || b || c
}

/// Every text the frame drew, with where.
fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, all: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => all.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, all)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut all);
    }
    all
}

/// Where the text exactly `words` was drawn (the first).
fn at(out: &egui::FullOutput, words: &str) -> egui::Rect {
    texts(out).into_iter().find(|(t, _)| t == words).unwrap_or_else(|| panic!("{words:?} is drawn")).1
}

/// The drawn text exactly `words` on the same row as `row` (the nearest one by height).
fn on_row(out: &egui::FullOutput, words: &str, row: egui::Rect) -> egui::Rect {
    texts(out)
        .into_iter()
        .filter(|(t, _)| t == words)
        .min_by(|a, b| (a.1.center().y - row.center().y).abs().total_cmp(&(b.1.center().y - row.center().y).abs()))
        .unwrap_or_else(|| panic!("a {words:?} is drawn"))
        .1
}

/// THE LIST SHOWS ONLY SHIPPED FILES: every entry of data/sounds.toml whose file exists under
/// assets/ is listed, and none whose file does not (the catalogue names 103 sounds, about 25 have
/// files); each listed sound is in its kind's group (the click under interface, the chat chime
/// under notifications as its `category` says, the crash under sound effects) and has its name in
/// plain words by key (`sound_names.<id>` in data/i18n/en.json), so a sound file added without
/// its name fails here. The expected set is worked out here from the TOML and the disk, not from
/// the code under test. RED first (2026-11-06): with `SoundCatalog::shipped` not checking the
/// file, "listed but not shipped: ambient.campfire_crackle" (103 listed, 25 shipped).
#[test]
fn the_list_shows_only_shipped_files() {
    let rows = shipped_rows(&repo().join("data"), &repo().join("assets"));
    let toml: toml::Table = std::fs::read_to_string(repo().join("data/sounds.toml")).unwrap().parse().unwrap();
    let mut want = Vec::new();
    let mut named = 0;
    for (group, table) in &toml {
        for (name, entry) in table.as_table().into_iter().flatten() {
            named += 1;
            let path = entry.get("path").and_then(|p| p.as_str()).unwrap();
            if repo().join("assets").join(path).is_file() {
                want.push(format!("{group}.{name}"));
            }
        }
    }
    want.sort();
    let listed: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    for id in &listed {
        assert!(want.contains(id), "listed but not shipped: {id} ({} listed, {} shipped)", listed.len(), want.len());
    }
    assert_eq!(listed, want, "every shipped sound is listed, by id");
    assert!(want.len() >= 20 && named > want.len(), "the catalogue names more sounds ({named}) than ship ({})", want.len());
    let kind = |id: &str| rows.iter().find(|r| r.id == id).map(|r| r.kind.as_str()).unwrap_or_else(|| panic!("{id} is listed"));
    assert_eq!(kind("sfx.button_click"), "interface", "the click is an interface sound (its bus, ui)");
    assert_eq!(kind("sfx.chat_message"), "notification", "the chat chime is a notification (its bus, notifications)");
    assert_eq!(kind("sfx.crash"), "sfx");
    for r in &rows {
        let key = format!("sound_names.{}", r.id);
        assert!(humanity_core::i18n::has(&key), "{} has no name in plain words: add {key} to data/i18n/en.json (node scripts/i18n-add.js)", r.id);
    }
    assert_eq!(sound_name("sfx.button_click"), "Button click");
    assert_eq!(sound_name("mod.rain_on_tin"), "rain on tin", "a sound with no words yet shows its id's last part");
}

/// PLAY PREVIEWS AT THE SLIDER'S LEVEL, as drawn: a press at the left end of the button click's
/// slider sets the click to -24 dB (and asks for a save); its row's Play asks the engine for the
/// click; and the engine's half (`play_preview`, through the app's own mixer on kira's mock
/// backend) plays it at exactly the game's one-shot of it at -24 dB, which is 10^(-24/20) of the
/// click at 0 dB. RED first (2026-11-06): with the click's Play asking for another sound, "Play
/// asks for the click" failed (left: Some("sfx.button_hover"), right: Some("sfx.button_click")).
#[test]
fn play_previews_at_the_sliders_level() {
    sound_levels_reset_thread();
    let (ctx, theme) = headless();
    let mut s = section();
    let out = settled(&ctx, &theme, &mut s);
    let name = at(&out, "Button click");
    // The slider starts right after the row's label column; its left end is -24 dB.
    let x = name.left() + theme.settings_label_width + ctx.style().spacing.item_spacing.x + 2.0;
    let changed = click_at(&ctx, &theme, &mut s, egui::pos2(x, name.center().y));
    assert!(changed, "moving the slider asks for a save");
    assert_eq!(sl::level_db("sfx.button_click"), sl::MIN_DB, "a press at the slider's left end is -24 dB");
    let out = settled(&ctx, &theme, &mut s);
    assert!(texts(&out).iter().any(|(t, r)| t == "-24.0 dB" && (r.center().y - name.center().y).abs() < 12.0), "the row shows -24.0 dB");
    let play = on_row(&out, "Play", at(&out, "Button click"));
    click_at(&ctx, &theme, &mut s, play.center());
    assert_eq!(s.preview.as_deref(), Some("sfx.button_click"), "Play asks for the click");
    let catalog = SoundCatalog::load(&repo().join("data"));
    let mut audio = humanity_core::audio::AudioManager::without_device();
    audio.set_data_dir(&repo().join("data"));
    // The engine's half, as lib.rs's audio stage calls it: the ask taken from egui's memory.
    let heard = play_preview(&ctx, &mut audio, &catalog).expect("the engine plays it");
    let kept = ctx.data(|d| d.get_temp::<SoundLevelsUi>(egui::Id::new(KEPT_ID))).unwrap();
    assert!(kept.preview.is_none(), "and the ask is used up");
    assert!(play_preview(&ctx, &mut audio, &catalog).is_none(), "so the next frame plays nothing");
    // The game's own call: the catalogue's path and its volume as the catalogue gives it
    // (`volume_or_default`, an f32 widened, 0.2000000029...: a literal 0.2 differs in the 8th digit).
    let click = humanity_core::audio::sounds::asset_path(&repo().join("assets"), "audio/ui/button_click.ogg");
    let vol = catalog.volume_or_default("sfx.button_click");
    let (_, game) = audio.one_shot(&click, vol, "ui").unwrap();
    assert_eq!(heard, game, "the preview plays at the slider's level, as the game would");
    sl::reset("sfx.button_click");
    let (_, at_zero) = audio.one_shot(&click, vol, "ui").unwrap();
    assert!((20.0 * (heard / at_zero).log10() + 24.0).abs() < 1e-9, "-24 dB under the click at 0 dB: {:.3}", 20.0 * (heard / at_zero).log10());
}

/// RESET RETURNS A SOUND TO ITS LEVELLED DEFAULT, as drawn: the crash set to +6 dB shows "+6.0 dB";
/// its row's Reset puts it back to 0 dB (nothing kept for it, a save asked for) while the click,
/// also set, keeps its level; then Reset all puts the click back too. RED first (2026-11-06): with
/// Reset's click left unhandled, "Reset asks for a save" failed (the crash stayed at +6 dB).
#[test]
fn reset_returns_a_sound_to_its_levelled_default() {
    sound_levels_reset_thread();
    sl::set_level("sfx.crash", 6.0);
    sl::set_level("sfx.button_click", -3.0);
    let (ctx, theme) = headless();
    let mut s = section();
    let out = settled(&ctx, &theme, &mut s);
    let crash = at(&out, "Crash");
    assert!(texts(&out).iter().any(|(t, r)| t == "+6.0 dB" && (r.center().y - crash.center().y).abs() < 12.0), "the crash's row shows +6.0 dB");
    let reset = on_row(&out, "Reset", crash);
    assert!(click_at(&ctx, &theme, &mut s, reset.center()), "Reset asks for a save");
    assert_eq!(sl::level_db("sfx.crash"), 0.0, "Reset puts the crash back to 0 dB");
    assert!(!sl::all().contains_key("sfx.crash"), "and nothing is kept for it");
    assert_eq!(sl::level_db("sfx.button_click"), -3.0, "the click keeps its level");
    let out = settled(&ctx, &theme, &mut s);
    assert!(click_at(&ctx, &theme, &mut s, at(&out, "Reset all").center()), "Reset all asks for a save");
    assert!(!sl::any(), "Reset all puts every sound back: {:?}", sl::all());
}

/// The search box shows only the sounds that match it (by name, ignoring case), and says so when
/// none does. RED first (2026-11-06): with `matches` always true, "the search for door hides the
/// click" failed.
#[test]
fn the_search_shows_only_matching_sounds() {
    sound_levels_reset_thread();
    let (ctx, theme) = headless();
    let mut s = section();
    s.search = "DOOR".into();
    let shown: Vec<String> = texts(&settled(&ctx, &theme, &mut s)).into_iter().map(|(t, _)| t).collect();
    assert!(shown.iter().any(|t| t == "Door opens") && shown.iter().any(|t| t == "Door closes"), "the doors are shown");
    assert!(!shown.iter().any(|t| t == "Button click"), "the search for door hides the click");
    s.search = "no sound is called this".into();
    let shown: Vec<String> = texts(&settled(&ctx, &theme, &mut s)).into_iter().map(|(t, _)| t).collect();
    assert!(shown.iter().any(|t| t == t_dyn("sound_levels.none_found")), "an empty search says so");
}

/// THE SECTION READS THE FOLDERS THE APP RESOLVED (BUG-268): built for a data folder outside the
/// working directory (an app folder unpacked somewhere else, its `data/` and `assets/` side by
/// side), it lists that folder's one shipped sound, read from that folder's catalogue and found in
/// the `assets` beside it, and nothing of the repo's (the tests run from the repo root, so a
/// relative "data" or "assets" would read the repo's sounds instead). RED first (2026-11-07) with
/// `for_data` keeping the old relative "assets": "the assets beside the data folder (left:
/// "assets", right: the temp app folder's assets)".
#[test]
fn the_section_reads_the_folders_the_app_resolved() {
    let app = humanity_core::test_temp::dir("sound_levels_app_folder");
    std::fs::create_dir_all(app.join("data")).unwrap();
    std::fs::create_dir_all(app.join("assets/audio/ui")).unwrap();
    std::fs::write(app.join("data/sounds.toml"), "[sfx.only_here]\npath = \"audio/ui/only_here.ogg\"\nbus = \"ui\"\n").unwrap();
    std::fs::copy(repo().join("assets/audio/ui/button_click.ogg"), app.join("assets/audio/ui/only_here.ogg")).unwrap();
    let s = SoundLevelsUi::for_data(app.join("data"));
    assert_eq!(s.assets_dir, app.join("assets"), "the assets beside the data folder");
    let rows: Vec<String> = shipped_rows(&s.data_dir, &s.assets_dir).into_iter().map(|r| r.id).collect();
    assert_eq!(rows, vec!["sfx.only_here".to_string()], "the app folder's own sound");
}

/// This thread's levels, empty.
fn sound_levels_reset_thread() {
    sl::use_on_this_thread(BTreeMap::new());
}
