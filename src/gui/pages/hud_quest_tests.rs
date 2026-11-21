// Child of hud.rs (#[path]): the HUD's quest line with several quests running (2026-11-20,
// engine piece P2 of docs/design/quest-chains.md, section 1.5).

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::GuiQuest;

/// Two settle frames of the HUD alone, returning the second's shapes.
fn hud_shapes(state: &GuiState) -> Vec<egui::epaint::ClippedShape> {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut shapes = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))),
            ..Default::default()
        };
        shapes = ctx.run(input, |ctx| draw(ctx, &theme, state, 0.0, glam::Mat4::IDENTITY, glam::Vec3::ZERO)).shapes;
    }
    shapes
}

/// A quest under way on its first of three steps, its step's words its name's.
fn running(name: &str, tracked: bool) -> GuiQuest {
    GuiQuest { id: name.to_lowercase(), name: name.to_string(), step_index: 0, step_total: 3, step_desc: format!("{name} step"), tracked, ..Default::default() }
}

/// Which of `names` the HUD drew, by name and by its step line; it must draw exactly one.
fn drawn(state: &GuiState, names: &[&str]) -> String {
    let shapes = hud_shapes(state);
    let shown: Vec<&str> = names.iter().copied().filter(|n| find_text_in_shapes(&shapes, n).is_some()).collect();
    assert_eq!(shown.len(), 1, "the HUD draws one quest's name, drew {shown:?}");
    let line = quest_line(&format!("{} step", shown[0]), 0, 3);
    assert!(find_text_in_shapes(&shapes, &line).is_some(), "the HUD draws {}'s step line \"{line}\"", shown[0]);
    shown[0].to_string()
}

/// THE HUD SHOWS THE TRACKED QUEST, ELSE THE MOST RECENTLY ACCEPTED. With two quests running
/// the HUD draws the one marked tracked, whichever was accepted first; with three and none
/// marked (a mirror that marked none), the last accepted; with three and the first marked, the
/// first; a finished quest is never drawn. Seen red 2026-11-21 with `hud_quest` as it was, the
/// first under way: "assertion `left == right` failed: two running, the second tracked; left: \"Water\",
/// right: \"Food\"".
#[test]
fn the_hud_shows_the_tracked_quest_with_two_and_three_running() {
    let mut state = GuiState::default();
    state.quests = vec![running("Water", false), running("Food", true)];
    assert_eq!(drawn(&state, &["Water", "Food"]), "Food", "two running, the second tracked");

    state.quests = vec![running("Water", false), running("Food", false), running("Warmth", false)];
    assert_eq!(drawn(&state, &["Water", "Food", "Warmth"]), "Warmth", "three running, none marked: the most recent");

    state.quests = vec![running("Water", true), running("Food", false), running("Warmth", false)];
    assert_eq!(drawn(&state, &["Water", "Food", "Warmth"]), "Water", "three running, the first tracked");

    let mut done = running("Rest", true);
    done.completed = true;
    state.quests = vec![running("Water", false), running("Food", false), done];
    assert_eq!(drawn(&state, &["Water", "Food", "Rest"]), "Food", "a finished quest is never the HUD's");
}
