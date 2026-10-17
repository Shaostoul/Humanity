//! Tests of the covered recovery phrase and device-link code (phrase_cover.rs). Each was seen to
//! fail once before it passed; what it saw red is in its comment.

use super::*;

/// The phrase for seed [7; 32] (identity.rs pins it), and its 24 words.
fn phrase() -> String {
    crate::net::identity::mnemonic_from_seed(&[7u8; 32]).expect("a phrase")
}

/// One headless pass drawing `draw`, with the window in front or not; its output.
fn pass(ctx: &egui::Context, focused: bool, events: Vec<egui::Event>, draw: impl FnMut(&egui::Context)) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
        focused,
        events,
        ..Default::default()
    };
    ctx.run(input, draw)
}

/// Every text drawn, whole.
fn texts(out: &egui::FullOutput) -> Vec<String> {
    fn walk(shape: &egui::Shape, found: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => found.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        walk(&cs.shape, &mut found);
    }
    found
}

/// How many of the phrase's words are on screen (as a numbered cell, `NN. word`).
fn words_on_screen(out: &egui::FullOutput, phrase: &str) -> usize {
    let drawn = texts(out);
    phrase.split_whitespace().enumerate().filter(|(i, w)| drawn.iter().any(|t| t.trim() == format!("{}. {w}", i + 1))).count()
}

/// Where the text exactly `text` is drawn.
fn at(out: &egui::FullOutput, text: &str) -> egui::Pos2 {
    fn walk(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == text => Some(t.galley.rect.translate(t.pos.to_vec2()).center()),
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, text)),
            _ => None,
        }
    }
    out.shapes.iter().find_map(|cs| walk(&cs.shape, text)).unwrap_or_else(|| panic!("{text:?} is drawn"))
}

/// Press and release at `pos`.
fn click(pos: egui::Pos2) -> [Vec<egui::Event>; 2] {
    let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() };
    [vec![egui::Event::PointerMoved(pos), button(true)], vec![button(false)]]
}

/// COVERED UNTIL SHOWN, AND COVERED AGAIN (operator, 2026-10-16: livestreamers and people signing
/// up in public must not put their recovery phrase on screen by accident). The words start
/// covered (not drawn at all: dots in their place, and the line saying when it is safe); Show the
/// words shows them; they cover themselves when the window is no longer in front, when their
/// screen is left and comes back, and on Hide the words.
///
/// Seen red 2026-10-16 with `lifted` ignoring the window's focus: "switching to another window
/// covers them" found all 24 words still drawn.
#[test]
fn the_words_are_covered_until_shown_and_cover_themselves_again() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let phrase = phrase();
    let draw = |ctx: &egui::Context| {
        egui::CentralPanel::default().show(ctx, |ui| words(ui, &theme, "test_phrase", &phrase));
    };
    let shown_after = |ctx: &egui::Context, focused: bool, events: Vec<egui::Event>| words_on_screen(&pass(ctx, focused, events, draw), &phrase);

    pass(&ctx, true, vec![], draw);
    let out = pass(&ctx, true, vec![], draw);
    assert_eq!(words_on_screen(&out, &phrase), 0, "nothing of the phrase is drawn at first");
    assert_eq!(texts(&out).iter().filter(|t| t.ends_with(COVERED_WORD)).count(), 24, "24 covered places");
    assert!(texts(&out).iter().any(|t| t == WARNING), "the line saying when it is safe");

    let [press, release] = click(at(&out, SHOW));
    pass(&ctx, true, press, draw);
    pass(&ctx, true, release, draw);
    assert_eq!(shown_after(&ctx, true, vec![]), 24, "Show the words shows all 24");

    assert_eq!(shown_after(&ctx, false, vec![]), 0, "switching to another window covers them");
    assert_eq!(shown_after(&ctx, true, vec![]), 0, "and they stay covered when the window comes back");

    // Shown, then the screen is left for a pass and comes back: covered.
    let out = pass(&ctx, true, vec![], draw);
    let [press, release] = click(at(&out, SHOW));
    pass(&ctx, true, press, draw);
    pass(&ctx, true, release, draw);
    assert_eq!(shown_after(&ctx, true, vec![]), 24);
    pass(&ctx, true, vec![], |ctx: &egui::Context| {
        egui::CentralPanel::default().show(ctx, |ui| ui.label("another screen"));
    });
    assert_eq!(shown_after(&ctx, true, vec![]), 0, "a screen that comes back opens covered");

    // Shown, then Hide the words.
    let out = pass(&ctx, true, vec![], draw);
    let [press, release] = click(at(&out, SHOW));
    pass(&ctx, true, press, draw);
    pass(&ctx, true, release, draw);
    let out = pass(&ctx, true, vec![], draw);
    assert_eq!(words_on_screen(&out, &phrase), 24);
    let [press, release] = click(at(&out, HIDE));
    pass(&ctx, true, press, draw);
    pass(&ctx, true, release, draw);
    assert_eq!(shown_after(&ctx, true, vec![]), 0, "Hide the words covers them");
}

/// THE DEVICE-LINK CODE IS COVERED THE SAME WAY: it carries the account, and a QR on a stream can
/// be scanned from the recording. Covered, the code is not drawn at all.
///
/// Seen red 2026-10-16 with `code` drawing the code whatever the cover: "the code is not drawn
/// while covered" found it drawn.
#[test]
fn the_device_link_code_is_covered_until_shown() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let drawn = std::cell::Cell::new(false);
    let draw = |ctx: &egui::Context| {
        egui::CentralPanel::default().show(ctx, |ui| {
            code(ui, &theme, "test_code", 200.0, |ui| {
                drawn.set(true);
                ui.label("THE CODE");
            })
        });
    };
    pass(&ctx, true, vec![], draw);
    let out = pass(&ctx, true, vec![], draw);
    assert!(!drawn.get(), "the code is not drawn while covered");
    assert!(texts(&out).iter().any(|t| t == CODE_HIDDEN) && texts(&out).iter().any(|t| t == CODE_WARNING));
    let [press, release] = click(at(&out, SHOW_CODE));
    pass(&ctx, true, press, draw);
    pass(&ctx, true, release, draw);
    let out = pass(&ctx, true, vec![], draw);
    assert!(drawn.get() && texts(&out).iter().any(|t| t == "THE CODE"), "Show the code shows it");
    drawn.set(false);
    pass(&ctx, false, vec![], draw);
    assert!(!drawn.get(), "and switching windows covers it");
}

/// FIRST RUN COVERS THE WORDS (the case the operator named): the identity step right after
/// Generate New Identity shows its backup card with the 24 words covered, and they show only on
/// Show the words.
///
/// Seen red 2026-10-16 with the first-run card's own word grid put back: "nothing of the phrase
/// on the first-run screen" found 24.
#[test]
fn first_run_shows_the_backup_card_with_the_words_covered() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = crate::gui::GuiState::default();
    state.onboarding_complete = false;
    state.onboarding_step = 2;
    state.user_name = "Explorer".to_string();
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.settings.seed_phrase_visible = true;
    let phrase = phrase();
    let mut frame = |events: Vec<egui::Event>| pass(&ctx, true, events, |ctx: &egui::Context| crate::gui::pages::main_menu::draw(ctx, &theme, &mut state));
    frame(vec![]);
    let out = frame(vec![]);
    assert!(texts(&out).iter().any(|t| t == crate::gui::pages::main_menu::FIRST_RUN_DONE), "the first-run backup card is up");
    assert_eq!(words_on_screen(&out, &phrase), 0, "nothing of the phrase on the first-run screen");
    let [press, release] = click(at(&out, SHOW));
    frame(press);
    frame(release);
    assert_eq!(words_on_screen(&frame(vec![]), &phrase), 24, "until Show the words");
}
