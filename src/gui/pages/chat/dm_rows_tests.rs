//! The DM list's previews (chat/dm_rows.rs), drawn headlessly (no GPU, no network) and read back
//! from what was drawn. Each test was seen to fail once against the break its comment names.

use super::*;
use crate::config::AppConfig;
use crate::gui::ChatDm;

/// A message long enough to wrap to more than two lines at any width the sidebar has.
const LONG: &str = "The seedlings came up this morning, all twelve of them, and the tomatoes are already taller than the peppers. Come and see them before the weekend if you can, and bring the watering can back with you.";

/// An unlocked app with one conversation, its last message LONG, showing `lines` of it.
fn one_conversation(lines: u8) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut s = GuiState::default();
    s.private_key_bytes = Some(vec![7u8; 32]);
    s.chat_dms = vec![ChatDm { user_name: "Bela".into(), user_key: "dlth3:bela".into(), last_message: LONG.into(), timestamp: String::new(), unread: false }];
    s.settings.dm_preview_lines = lines;
    s
}

/// Every text the left panel draws, settled over three passes: its words and how many rows.
fn texts(state: &mut GuiState) -> Vec<(String, usize)> {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    let mut out = None;
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 900.0))), ..Default::default() };
        out = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| super::super::left_panel::draw_left_panel(ui, &theme, state));
        }));
    }
    fn walk(shape: &egui::Shape, found: &mut Vec<(String, usize)>) {
        match shape {
            egui::Shape::Text(t) => found.push((t.galley.text().to_string(), t.galley.rows.len())),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &out.expect("three passes ran").shapes {
        walk(&clipped.shape, &mut found);
    }
    found
}

/// EACH LEVEL DRAWS WHAT IT SAYS: Names only draws the person's name and no word of the message;
/// Name and one line draws the message on one row; Name and two lines on two, cut short (the
/// message is longer than two rows) rather than spilling into the next conversation.
///
/// Seen red 2026-11-03 on the DM rows as they were (the preview always one row, the setting read
/// by nothing): "the rows of the preview at 0, 1 and 2 lines" (left: [(0, Some(1)), (1, Some(1)),
/// (2, Some(1))], right: [(0, None), (1, Some(1)), (2, Some(2))]).
#[test]
fn each_preview_level_draws_what_it_says() {
    let mut seen = Vec::new();
    for lines in [0u8, 1, 2] {
        let mut state = one_conversation(lines);
        let drawn = texts(&mut state);
        assert!(drawn.iter().any(|(t, _)| t == "@ Bela"), "the conversation's name is drawn at {lines} lines");
        let previews: Vec<usize> = drawn.iter().filter(|(t, _)| t.contains("seedlings")).map(|(_, rows)| *rows).collect();
        assert!(previews.len() <= 1, "one preview a conversation: {previews:?}");
        seen.push((lines, previews.first().copied()));
    }
    assert_eq!(seen, vec![(0, None), (1, Some(1)), (2, Some(2))], "the rows of the preview at 0, 1 and 2 lines");
}

/// THE CHOICE SURVIVES A RESTART: one line for a config written before the choice existed, and a
/// chosen level (names only, two lines) through a save and a load (the GUI state, the JSON and
/// back).
///
/// Seen red 2026-11-03 with the load leg (`apply_to_gui_state`) not reading it: "Names only must
/// stay names only across a restart" (left: 1, right: 0).
#[test]
fn the_preview_choice_survives_a_restart() {
    let old: AppConfig = serde_json::from_str(r#"{"server_url":""}"#).unwrap();
    assert_eq!(old.dm_preview_lines, DEFAULT_LINES, "one line for a config written before the choice");
    assert_eq!(GuiState::default().settings.dm_preview_lines, DEFAULT_LINES, "one line in a fresh app");
    for (lines, what) in [(0u8, "Names only must stay names only"), (2, "Two lines must stay two lines")] {
        let mut state = GuiState::default();
        state.settings.dm_preview_lines = lines;
        let json = serde_json::to_string(&AppConfig::from_gui_state(&state)).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        let mut fresh = GuiState::default();
        back.apply_to_gui_state(&mut fresh);
        assert_eq!(fresh.settings.dm_preview_lines, lines, "{what} across a restart");
    }
}

/// The shipped levels: names only, one line and two lines, each with words and a hint English
/// has, and today's look (one line) among them.
///
/// Seen red 2026-11-03 before the words were added: "chat.dm_preview_names is not in
/// data/i18n/en.json".
#[test]
fn the_shipped_levels_have_their_words() {
    let lines: Vec<u8> = levels().levels.iter().map(|l| l.lines).collect();
    assert_eq!(lines, vec![0, 1, 2], "the levels in data/gui/dm_preview.json");
    assert!(lines.contains(&DEFAULT_LINES), "the default is one of the levels");
    for level in &levels().levels {
        for key in [&level.label, &level.hint] {
            assert!(humanity_core::i18n::has(key), "{key} is not in data/i18n/en.json");
        }
    }
}

/// THE CHOICE IS WHERE A PERSON LOOKS: in the DMs header's cog (its popup open) and on Settings >
/// Privacy, the same three choices by their words.
///
/// Seen red 2026-11-03 before either was drawn: "the DMs cog offers Names only".
#[test]
fn the_choice_is_in_the_dms_cog_and_in_settings() {
    let names = humanity_core::i18n::t("chat.dm_preview_names");
    let mut state = one_conversation(1);
    state.dm_settings_popup_open = true;
    assert!(texts(&mut state).iter().any(|(t, _)| t == names), "the DMs cog offers Names only");

    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = one_conversation(1);
    let mut out = None;
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 4000.0))), ..Default::default() };
        out = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| crate::gui::pages::privacy::draw_privacy_content(ui, &theme, &mut state));
        }));
    }
    let shapes = out.expect("three passes ran").shapes;
    assert!(crate::gui::screen_surface::find_text_in_shapes(&shapes, names).is_some(), "Settings > Privacy offers Names only");
}
