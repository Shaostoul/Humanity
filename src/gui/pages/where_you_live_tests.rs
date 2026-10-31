//! Tests of "Choose where you live" (where_you_live.rs; ship homes increment 6, lane 1E): the widget
//! drawn headlessly where the app draws it (Settings > Gameplay's own content, the Home page, the
//! choice, a server's card), its words read back from what was DRAWN, and its household choice
//! clicked. Each was seen failing once on purpose; the break and the failure text are in its comment.

use super::*;
use crate::config::AppConfig;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::persistence::{SaveSlot, ShipPlace};
use crate::save_load::{active_home, set_active_home, ActiveHome};
use crate::ship::starts::{HeldHome, HomeRefusal, StartOffer};
use std::path::Path;

// ── The data ─────────────────────────────────────────────────────────────────────────────────────

/// The repo's starts file, as the game reads it.
fn the_starts() -> Starts {
    Starts::parse(&std::fs::read(Path::new("data").join(crate::ship::starts::STARTS_FILE)).expect("data/homes/starts.ron"))
        .expect("the starts file keeps its rules")
}

/// A server's answer to a guest who holds no home there: every start of `starts`, with 15
/// cabins free and every homestead held.
pub(crate) fn guest_answer(starts: &Starts) -> HomeAnswer {
    let free = |id: &str| match id {
        "cabin" => 15,
        _ => 0,
    };
    HomeAnswer {
        plot: None,
        starts: starts.starts.iter().map(|s| StartOffer::of(s, free(&s.id))).collect(),
        chose: false,
        refused: None,
        message: String::new(),
    }
}

/// A server's answer to someone who lives in its cabin c2.
pub(crate) fn held_answer(starts: &Starts) -> HomeAnswer {
    HomeAnswer {
        plot: Some(HeldHome { id: "c2".into(), kind: "cabin".into(), start: "cabin".into() }),
        ..guest_answer(starts)
    }
}

/// Your own home, a homestead for a family: the live save until anything sets another.
fn offline_homestead() -> ActiveHome {
    ActiveHome::offline()
}

/// A server's save in which you live in its cabin c2, just you.
pub(crate) fn server_cabin() -> ActiveHome {
    let server = "https://ship.example".to_string();
    let slot = SaveSlot::Server { server: server.clone(), identity: "ab".repeat(32) };
    ActiveHome {
        path: crate::persistence::slot_path(&std::env::temp_dir(), &slot),
        slot,
        start: "cabin".into(),
        household: "solo".into(),
        server,
        ship_place: Some(ShipPlace { ship_id: "mothership-1".into(), ship_hash: "a-hash".into(), plot: "c2".into() }),
    }
}

// ── Drawing headlessly ───────────────────────────────────────────────────────────────────────────

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

fn screen(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 4000.0))), events, ..Default::default() }
}

/// One frame of `draw` in a tall panel (so every row is on screen).
fn frame(ctx: &egui::Context, events: Vec<egui::Event>, draw: &mut dyn FnMut(&mut egui::Ui)) -> egui::FullOutput {
    ctx.run(screen(events), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw(ui));
    })
}

/// One frame of the Home page as the app draws it.
fn home_page(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    ctx.run(screen(Vec::new()), |ctx| crate::gui::pages::homes::draw(ctx, theme, state))
}

fn visit<'a>(shape: &'a egui::Shape, f: &mut dyn FnMut(&'a egui::epaint::TextShape)) {
    match shape {
        egui::Shape::Text(t) => f(t),
        egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, f)),
        _ => {}
    }
}

/// Every text drawn, whole (a wrapped label is one text).
fn texts(out: &egui::FullOutput) -> Vec<String> {
    let mut all = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut |t| all.push(t.galley.text().to_string()));
    }
    all
}

/// The colour `text` is drawn in (its first section's), when it is drawn.
fn color_of(out: &egui::FullOutput, text: &str) -> Option<egui::Color32> {
    let mut found = None;
    for cs in &out.shapes {
        visit(&cs.shape, &mut |t| {
            if found.is_none() && t.galley.text() == text {
                found = t.galley.job.sections.first().map(|s| s.format.color);
            }
        });
    }
    found
}

/// The texts of `want` that `out` does not draw exactly.
fn missing(out: &egui::FullOutput, want: &[&str]) -> Vec<String> {
    let drawn = texts(out);
    want.iter().filter(|w| !drawn.iter().any(|d| d == *w)).map(|w| w.to_string()).collect()
}

/// The texts of `never` that `out` draws (exactly, or as the start of a text).
fn present(out: &egui::FullOutput, never: &[&str]) -> Vec<String> {
    let drawn = texts(out);
    never.iter().filter(|n| drawn.iter().any(|d| d == *n || d.starts_with(*n))).map(|n| n.to_string()).collect()
}

/// Click the text `label` where `draw` draws it (its first time drawn).
fn click(ctx: &egui::Context, label: &str, draw: &mut dyn FnMut(&mut egui::Ui)) {
    let out = frame(ctx, Vec::new(), draw);
    let found = find_text_in_shapes(&out.shapes, label).unwrap_or_else(|| panic!("{label:?} is drawn: {:?}", texts(&out)));
    assert_eq!(found.text, label, "{label:?} is drawn as itself");
    click_at(ctx, found.rect.center(), draw);
}

/// Click at `pos`: the canonical three frames (move, press, release).
fn click_at(ctx: &egui::Context, pos: egui::Pos2, draw: &mut dyn FnMut(&mut egui::Ui)) {
    let m = egui::Modifiers::default();
    frame(ctx, vec![egui::Event::PointerMoved(pos)], draw);
    frame(ctx, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], draw);
    frame(ctx, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], draw);
}

/// The centre of the `n`th (from 0) text drawn exactly as `text`.
fn nth_center(out: &egui::FullOutput, text: &str, n: usize) -> egui::Pos2 {
    let mut centres = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut |t| {
            if t.galley.text() == text {
                centres.push(egui::Rect::from_min_size(t.pos, t.galley.size()).center());
            }
        });
    }
    *centres.get(n).unwrap_or_else(|| panic!("{text:?} is drawn {} times, not {}", centres.len(), n + 1))
}

// ── The tests ────────────────────────────────────────────────────────────────────────────────────

/// WHERE YOU LIVE, IN WORDS (plan 3.9): every row and line, read back from what is drawn where the
/// app draws it, for your own homestead, a server's cabin and a guest on a server.
///
/// - Your own homestead. Settings > Gameplay (its real content) says "Where you live", "In this
///   world you live in a Homestead (a family of three).", offers "A family of three" and "Just
///   you" with "Takes effect the next time you enter the world.", says how to live somewhere
///   else, and no longer says "Home Design" or its Family/Solo radios. The new-home choice has
///   the title and hint, then every start from data/homes/starts.ron in its order: the Cabin with
///   its sentence, "Shared: the mess hall, the bathroom, the recreation..." and its food and water
///   line; the Apartment GREYED (muted, no radio) with its sentence and why it is not offered;
///   "Homestead (most people start here)" with its sentence, its own line and no shared places,
///   and its households. The Home page says the same line and shows the homestead's section.
/// - A server's cabin. Settings says "In this world you live in a Cabin (just you)." with no
///   household choice (a cabin has one). The server's card says "Where you live here: a Cabin,
///   c2." with the cabin's sentence and the note, and no choice. The Home page says the line and
///   shows NO homestead section (the cabin's household file has no loops).
/// - A guest on a server. The card says "You have no home on this server yet.", the title and
///   hint, "15 free" and Choose for the Cabin, the Apartment greyed with the SERVER's reason,
///   "none free" for the Homestead (its Choose does nothing), and the note; Choose on the Cabin
///   chooses the cabin. A refusal shows the server's own sentence. A server that renamed its
///   Cabin is shown in its words, never this game's.
///
/// Seen red 2026-10-12 with "Home Design" kept as the title (the section's heading put back to
/// "Home Design"): "Settings > Gameplay is missing these", left: ["Where you live"].
#[test]
fn where_you_live_in_words() {
    let starts = the_starts();
    let (ctx, theme) = ctx_and_theme();
    let cabin = starts.get("cabin").unwrap().clone();
    let apartment = starts.get("apartment").unwrap().clone();
    let homestead = starts.get("homestead").unwrap().clone();

    // ── Your own homestead ──
    set_active_home(offline_homestead());
    let mut gs = GuiState::default();
    let out = frame(&ctx, Vec::new(), &mut |ui| crate::gui::pages::settings::draw_gameplay_content(ui, &theme, &mut gs));
    let want = [
        "Where you live",
        "In this world you live in a Homestead (a family of three).",
        "A family of three",
        "Just you",
        "Takes effect the next time you enter the world.",
        "Your own home on this computer stays the one you have. To live somewhere else, choose a home \
         on a server's card in Characters (Play). Moving house comes in a later update.",
    ];
    assert_eq!(missing(&out, &want), Vec::<String>::new(), "Settings > Gameplay is missing these");
    assert_eq!(present(&out, &["Home Design", "Family (default)", "Solo --"]), Vec::<String>::new(), "and no longer says these");

    reset_draft();
    let out = frame(&ctx, Vec::new(), &mut |ui| {
        draw_choice(ui, &theme, &starts);
    });
    let want = [
        "Choose where you live",
        "Where you live sets how much space you hold and how much looking after it needs. \
         It never changes how deep the world runs or how hard it is: that is your play mode.",
        "Cabin",
        cabin.sentence.as_str(),
        "Food: free crew meals in the mess hall. Water and power: the ship's, counted on your meter.",
        "Apartment",
        apartment.sentence.as_str(),
        apartment.not_offered_why.as_str(),
        "Food: shared dinners with your block. Water and power: the ship's, counted on your meter.",
        "Homestead (most people start here)",
        homestead.sentence.as_str(),
        "Food: you grow it. Water and power: you collect and make them.",
        "A family of three",
        "Just you",
    ];
    assert_eq!(missing(&out, &want), Vec::<String>::new(), "the new-home choice is missing these");
    let drawn = texts(&out);
    // The shared places are the zone types' labels: the recreation room's comes with lane 1A's
    // zone types (until then its id reads as its own words), so only its first word is pinned.
    let shared: Vec<&String> = drawn.iter().filter(|t| t.starts_with("Shared: ")).collect();
    assert_eq!(shared.len(), 2, "the Cabin and the Apartment share places aboard, the Homestead none: {shared:?}");
    assert!(shared[0].starts_with("Shared: the mess hall, the bathroom, the recreation"), "the Cabin's: {}", shared[0]);
    assert!(shared[1].starts_with("Shared: the apartment block, the mess hall, the recreation"), "the Apartment's: {}", shared[1]);
    assert_eq!(color_of(&out, "Apartment"), Some(theme.text_muted()), "the Apartment, not offered, is greyed");
    assert_eq!(color_of(&out, apartment.not_offered_why.as_str()), Some(theme.text_muted()));
    assert_eq!(color_of(&out, "Cabin"), Some(theme.text_primary()), "an offered start is not");
    let order: Vec<usize> = ["Cabin", "Apartment", "Homestead (most people start here)"]
        .iter()
        .map(|n| drawn.iter().position(|t| t == n).unwrap())
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "in the file's order: {order:?}");
    // The Apartment has no radio: clicking its name picks nothing.
    click(&ctx, "Apartment", &mut |ui| {
        draw_choice(ui, &theme, &starts);
    });
    assert_eq!(draft_choice(&starts).start, "homestead", "a start not offered cannot be picked");
    click(&ctx, "Cabin", &mut |ui| {
        draw_choice(ui, &theme, &starts);
    });
    assert_eq!(draft_choice(&starts), Choice { start: "cabin".into(), household: "solo".into() }, "an offered one can");
    reset_draft();

    let mut gs = GuiState::default();
    gs.homestead_design = crate::gui::load_homestead_design(Path::new("data"));
    gs.home_machines = crate::systems::grow_machines::load_home(&Path::new("data").join("machines").join("home.ron"));
    let out = home_page(&ctx, &theme, &mut gs);
    assert_eq!(
        missing(&out, &["Where you live", "In this world you live in a Homestead (a family of three).", "Your Home"]),
        Vec::<String>::new(),
        "the Home page of a homestead"
    );

    // ── A server's cabin ──
    set_active_home(server_cabin());
    let mut gs = GuiState::default();
    let out = frame(&ctx, Vec::new(), &mut |ui| crate::gui::pages::settings::draw_gameplay_content(ui, &theme, &mut gs));
    assert_eq!(missing(&out, &["Where you live", "In this world you live in a Cabin (just you)."]), Vec::<String>::new());
    assert_eq!(
        present(&out, &["A family of three", "Just you", "Takes effect the next time you enter the world."]),
        Vec::<String>::new(),
        "a cabin has one household, so no choice of one"
    );
    let held = held_answer(&starts);
    let out = frame(&ctx, Vec::new(), &mut |ui| {
        draw_server(ui, &theme, &starts, Some(&held), false);
    });
    let note = "On a server you start fresh: only your name and look come with you. Your home there is kept on this computer.";
    assert_eq!(missing(&out, &["Where you live here: a Cabin, c2.", cabin.sentence.as_str(), note]), Vec::<String>::new());
    assert_eq!(present(&out, &["You have no home on this server yet.", "Choose", "Choose where you live"]), Vec::<String>::new());

    let mut gs = GuiState::default();
    gs.homestead_design = crate::gui::load_homestead_design(Path::new("data"));
    gs.home_machines = crate::systems::grow_machines::load_home(&Path::new("data").join("machines").join("cabin.ron"));
    let out = home_page(&ctx, &theme, &mut gs);
    assert_eq!(missing(&out, &["Where you live", "In this world you live in a Cabin (just you)."]), Vec::<String>::new());
    assert_eq!(
        present(&out, &["Your Home", "Closed-loop self-sufficiency", "What one home cannot close"]),
        Vec::<String>::new(),
        "a cabin's Home page has no homestead section"
    );

    // ── A guest on a server ──
    let guest = guest_answer(&starts);
    let out = frame(&ctx, Vec::new(), &mut |ui| {
        draw_server(ui, &theme, &starts, Some(&guest), false);
    });
    let want = [
        "You have no home on this server yet.",
        "Choose where you live",
        "Cabin",
        cabin.sentence.as_str(),
        "15 free",
        "Apartment",
        apartment.not_offered_why.as_str(),
        "Homestead (most people start here)",
        "none free",
        note,
    ];
    assert_eq!(missing(&out, &want), Vec::<String>::new(), "a guest's card is missing these");
    assert_eq!(texts(&out).iter().filter(|t| *t == "Choose").count(), 2, "Choose on the Cabin and the Homestead, none on the Apartment");
    assert_eq!(color_of(&out, "Apartment"), Some(theme.text_muted()), "greyed");
    assert_eq!(
        present(&out, &["Where you live here:", "A family of three", "Just you"]),
        Vec::<String>::new(),
        "no household to pick for a home none of which is free"
    );

    let mut chosen = None;
    click(&ctx, "Choose", &mut |ui| {
        if let Some(c) = draw_server(ui, &theme, &starts, Some(&guest), false) {
            chosen = Some(c);
        }
    });
    assert_eq!(chosen, Some(Choice { start: "cabin".into(), household: "solo".into() }), "Choose on the Cabin chooses it");
    // The Homestead's Choose (the second) is disabled: none is free.
    let out = frame(&ctx, Vec::new(), &mut |ui| {
        draw_server(ui, &theme, &starts, Some(&guest), false);
    });
    let mut chosen = None;
    click_at(&ctx, nth_center(&out, "Choose", 1), &mut |ui| {
        if let Some(c) = draw_server(ui, &theme, &starts, Some(&guest), false) {
            chosen = Some(c);
        }
    });
    assert_eq!(chosen, None, "the Homestead's Choose does nothing: none is free");

    // With homesteads free, the Homestead's row offers its households, and Choose takes the one
    // picked.
    let mut open = guest_answer(&starts);
    open.starts.iter_mut().find(|o| o.id == "homestead").unwrap().free = 12;
    let draw_open = |ui: &mut egui::Ui, chosen: &mut Option<Choice>| {
        if let Some(c) = draw_server(ui, &theme, &starts, Some(&open), false) {
            *chosen = Some(c);
        }
    };
    let mut chosen = None;
    let out = frame(&ctx, Vec::new(), &mut |ui| draw_open(ui, &mut chosen));
    assert_eq!(missing(&out, &["12 free", "A family of three", "Just you"]), Vec::<String>::new());
    click(&ctx, "Just you", &mut |ui| draw_open(ui, &mut chosen));
    let out = frame(&ctx, Vec::new(), &mut |ui| draw_open(ui, &mut chosen));
    click_at(&ctx, nth_center(&out, "Choose", 1), &mut |ui| draw_open(ui, &mut chosen));
    assert_eq!(chosen, Some(Choice { start: "homestead".into(), household: "solo".into() }), "a homestead for just you");
    reset_draft();

    // A refusal says the server's own sentence.
    let full = HomeAnswer {
        refused: Some(HomeRefusal::Full),
        message: crate::ship::starts::refusal_words(HomeRefusal::Full, &cabin, None, &[]),
        ..guest_answer(&starts)
    };
    let out = frame(&ctx, Vec::new(), &mut |ui| {
        draw_server(ui, &theme, &starts, Some(&full), false);
    });
    assert_eq!(missing(&out, &["No cabin is free on this ship right now."]), Vec::<String>::new());
    assert_eq!(color_of(&out, "No cabin is free on this ship right now."), Some(theme.warning()));

    // The server's words, never this game's.
    let mut renamed = guest_answer(&starts);
    renamed.starts[0].name = "Bunk room".into();
    renamed.starts[0].sentence = "A bunk of your own, by the mess hall.".into();
    let out = frame(&ctx, Vec::new(), &mut |ui| {
        draw_server(ui, &theme, &starts, Some(&renamed), false);
    });
    assert_eq!(missing(&out, &["Bunk room", "A bunk of your own, by the mess hall."]), Vec::<String>::new());
    assert_eq!(present(&out, &["Cabin", cabin.sentence.as_str()]), Vec::<String>::new(), "the server renamed its Cabin");

    // No answer yet.
    let out = frame(&ctx, Vec::new(), &mut |ui| {
        draw_server(ui, &theme, &starts, None, true);
    });
    assert_eq!(missing(&out, &["Asking the server where you live...", note]), Vec::<String>::new());
    set_active_home(ActiveHome::offline());
}

/// THE HOUSEHOLD CHOICE IS THE SAVE'S (plan 3.9): choosing "Just you" in Settings > Gameplay sets
/// the live save's household (`ActiveHome::household`), which the household file the world loads
/// follows (`machines::home_ron_path` gives home_solo.ron), and NEVER AppConfig: nothing that
/// AppConfig would write changes, and Settings is not marked to be saved. Choosing "A family of
/// three" again puts it back.
///
/// Seen red 2026-10-12 with the AppConfig write kept (settings.rs also setting
/// `settings.home_variant` and `settings_dirty` when the household changed): "Settings is not
/// marked to be saved: the household is the save's" failed.
#[test]
fn the_household_choice_is_the_saves() {
    let (ctx, theme) = ctx_and_theme();
    let data = Path::new("data");
    set_active_home(offline_homestead());
    let mut gs = GuiState::default();
    let config_before = serde_json::to_value(AppConfig::from_gui_state(&gs)).expect("AppConfig serializes");
    assert_eq!(crate::save_load::active_home_machines_path(data), data.join("machines").join("home.ron"));

    click(&ctx, "Just you", &mut |ui| crate::gui::pages::settings::draw_gameplay_content(ui, &theme, &mut gs));
    assert_eq!(active_home().household, "solo", "the live save's household is now solo");
    assert_eq!(crate::save_load::active_home_machines_path(data), data.join("machines").join("home_solo.ron"), "and the household file follows it");
    assert!(!gs.settings_dirty, "Settings is not marked to be saved: the household is the save's");
    assert_eq!(serde_json::to_value(AppConfig::from_gui_state(&gs)).unwrap(), config_before, "nothing AppConfig writes changed");
    let out = frame(&ctx, Vec::new(), &mut |ui| crate::gui::pages::settings::draw_gameplay_content(ui, &theme, &mut gs));
    assert_eq!(missing(&out, &["In this world you live in a Homestead (just you)."]), Vec::<String>::new(), "Settings says so");

    click(&ctx, "A family of three", &mut |ui| crate::gui::pages::settings::draw_gameplay_content(ui, &theme, &mut gs));
    assert_eq!(active_home().household, "family");
    assert_eq!(crate::save_load::active_home_machines_path(data), data.join("machines").join("home.ron"));
    assert!(!gs.settings_dirty);
    assert_eq!(serde_json::to_value(AppConfig::from_gui_state(&gs)).unwrap(), config_before);
    set_active_home(ActiveHome::offline());
}

/// NO TIER WORD ON THE CHOICE (plan 3.9 "Never written"): wherever the widget is drawn (the
/// choice, a server's card held, as a guest, refused, with no answer, Settings for a homestead
/// and a cabin, the Home page's line), no word drawn is tier, rank, level or berth, or their
/// plurals. Where you live is how much space you hold, never a step on a ladder.
///
/// Seen red 2026-10-12 with "tier" added to the hint ("...that is your play mode, not a tier."):
/// "the choice says \"tier\"" failed.
#[test]
fn no_tier_word_on_the_choice() {
    let starts = the_starts();
    let (ctx, theme) = ctx_and_theme();
    let hint = crate::gui::HintDisplay::Full;
    let guest = guest_answer(&starts);
    let held = held_answer(&starts);
    let refused = HomeAnswer {
        refused: Some(HomeRefusal::HoldsOne),
        message: crate::ship::starts::refusal_words(
            HomeRefusal::HoldsOne,
            starts.get("homestead").unwrap(),
            held.plot.as_ref(),
            &[],
        ),
        ..held_answer(&starts)
    };
    let mut drawn = Vec::new();
    reset_draft();
    drawn.extend(texts(&frame(&ctx, Vec::new(), &mut |ui| {
        draw_choice(ui, &theme, &starts);
    })));
    for answer in [Some(&guest), Some(&held), Some(&refused), None] {
        for asking in [false, true] {
            drawn.extend(texts(&frame(&ctx, Vec::new(), &mut |ui| {
                draw_server(ui, &theme, &starts, answer, asking);
            })));
        }
    }
    for active in [offline_homestead(), server_cabin()] {
        set_active_home(active);
        drawn.extend(texts(&frame(&ctx, Vec::new(), &mut |ui| {
            draw_settings_section_with(ui, &theme, hint, &starts);
            draw_home_line_with(ui, &theme, &starts);
        })));
    }
    set_active_home(ActiveHome::offline());
    assert!(drawn.len() > 40, "the widget drew its words: {}", drawn.len());
    for text in &drawn {
        for word in text.to_lowercase().split(|c: char| !c.is_ascii_alphabetic()) {
            assert!(
                !["tier", "tiers", "rank", "ranks", "level", "levels", "berth", "berths"].contains(&word),
                "the choice says {word:?}: {text:?}"
            );
        }
    }
}
