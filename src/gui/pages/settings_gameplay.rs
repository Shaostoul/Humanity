//! Settings > Gameplay: the settings with levels (2026-11-11), drawn from
//! data/gameplay/levels.json (gui/gameplay_levels.rs).
//!
//! The operator, that day: "I feel like we need to provide more than two options for our gameplay
//! settings. Including just turning settings off completely. Like body heat, illness, etc." So each
//! setting is a row of choices, gentlest first (Off wherever a system can be off), and under them
//! the chosen level's sentence saying what it changes (docs/design/graphics-settings.md's rule,
//! applied to gameplay: say what a choice changes before it is chosen; every radio's hint says it
//! too), then the setting's longer note, its numbers read from the data it describes.
//!
//! Since 2026-11-12 the garden's and the ship's settings (garden pests, pollination, soil pH,
//! picking, ship life support) are rows of the file too, in its "garden" group: the page draws
//! the "survival" group at the top of its Survival card and the "garden" group under its own
//! heading further down (`draw_group`).
//!
//! Nothing here lists a setting: the loop draws the file. The one thing that is per setting is
//! [`note`], the long description some systems write from their own data (body heat's balance,
//! illness's course, the drone's battery), and where the level is kept: a level id in
//! `settings.gameplay`, except the vitals drain, whose levels are points on its slider.

use egui::RichText;
use humanity_core::i18n::{t_dyn, tf};

use crate::gui::gameplay_levels::{self, Setting};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiState, HintDisplay};

/// The setting whose levels are points on `settings.vitals_drain` rather than a saved level id.
const VITALS_DRAIN: &str = "vitals_drain";

/// Draw every setting of data/gameplay/levels.json, in the file's order, every group. Marks the
/// settings dirty when a level changes.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay) {
    for s in &gameplay_levels::file().settings {
        ui.add_space(theme.spacing_sm);
        draw_setting(ui, theme, state, hint, s);
    }
}

/// Draw the settings of one group of the file (`Setting::group`: the survival rows at the top of
/// the page, the garden's and the ship's further down), in the file's order.
pub fn draw_group(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, group: &str) {
    for s in gameplay_levels::file().settings.iter().filter(|s| s.group == group) {
        ui.add_space(theme.spacing_sm);
        draw_setting(ui, theme, state, hint, s);
    }
}

/// One setting: its name, a radio per level, the chosen level's sentence, and its note.
fn draw_setting(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, s: &Setting) {
    ui.label(RichText::new(t_dyn(&s.words)).color(theme.text_secondary()));
    let chosen = chosen_level(state, s);
    ui.horizontal_wrapped(|ui| {
        for l in &s.levels {
            let selected = chosen.as_deref() == Some(l.id.as_str());
            if ui.radio(selected, RichText::new(t_dyn(&l.words)).color(theme.text_primary())).hint_dyn(&l.says).clicked() && !selected {
                if s.id == VITALS_DRAIN {
                    state.settings.vitals_drain = l.value.unwrap_or(1.0);
                } else {
                    state.settings.gameplay.insert(s.id.clone(), l.id.clone());
                }
                state.settings_dirty = true;
            }
        }
    });
    if let Some([lo, hi]) = s.slider {
        if widgets::with_hint(ui, "gameplay.level_slider_hint", |ui| widgets::labeled_slider(ui, theme, t_dyn(&s.words), &mut state.settings.vitals_drain, lo..=hi)) {
            state.settings_dirty = true;
        }
    }
    // What the chosen level changes (a slider between two levels says its own number).
    match s.levels.iter().find(|l| chosen.as_deref() == Some(l.id.as_str())) {
        Some(l) => widgets::setting_hint(ui, theme, hint, &fill(state, s, &l.id, t_dyn(&l.says))),
        None => widgets::setting_hint(ui, theme, hint, &tf("gameplay.vitals_drain_between", &[("rate", &format!("{:.2}", state.settings.vitals_drain))])),
    }
    for line in note(state, s) {
        widgets::setting_hint(ui, theme, hint, &line);
    }
}

/// The level chosen: the saved one, or for the vitals drain the level whose number the slider
/// sits on (None between two of them).
fn chosen_level(state: &GuiState, s: &Setting) -> Option<String> {
    if s.id == VITALS_DRAIN {
        let v = state.settings.vitals_drain;
        return s.levels.iter().find(|l| l.value.is_some_and(|x| (x - v).abs() < 0.01)).map(|l| l.id.clone());
    }
    Some(state.settings.level(&s.id).to_string())
}

/// A level's sentence with its numbers filled in: the walking pace's speeds come from
/// data/game.csv (published by engine::walk_pace), so the words never drift from the speeds.
fn fill(state: &GuiState, s: &Setting, level: &str, words: &str) -> String {
    if s.id != "walk_pace" {
        return words.to_string();
    }
    match state.walk_pace_speeds.iter().find(|(id, _, _)| id == level) {
        Some((_, walk, run)) => words.replace("{{walk}}", &format!("{walk:.1}")).replace("{{run}}", &format!("{run:.1}")),
        None => words.replace("{{walk}}", "?").replace("{{run}}", "?"),
    }
}

/// The setting's longer note: its own key in the file, or the words a system writes from its data.
fn note(state: &GuiState, s: &Setting) -> Vec<String> {
    let mut lines: Vec<String> = s.note.iter().map(|k| t_dyn(k).to_string()).collect();
    match s.id.as_str() {
        "body_heat" => lines.push(super::settings::body_heat_hint()),
        "illness" => {
            lines.push(humanity_systems::illness::mode_hint());
            lines.push(humanity_systems::wounds::mode_hint());
        }
        "animal_care" => lines.push(humanity_systems::animal_care::mode_hint()),
        "death" => {
            let interact = humanity_core::input::bindings::pretty_key_name(state.keybinds.pair(humanity_core::input::bindings::GameAction::Interact).0);
            lines.push(super::settings::death_hint(&state.death_pack.rules, &interact));
        }
        "mining_drone" => lines.push(humanity_systems::mining::mode_hint()),
        _ => {}
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draw Settings > Gameplay's levels once, clicking where `click` says (or nowhere), and
    /// return the frame and the state.
    fn frame(state: &mut GuiState, click: Option<egui::Pos2>) -> egui::FullOutput {
        let theme = crate::gui::theme::load_theme();
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        theme.apply_to_egui(&ctx);
        let mut out = None;
        for i in 0..3 {
            let mut events = Vec::new();
            if let (Some(p), 2) = (click, i) {
                events.push(egui::Event::PointerMoved(p));
                events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
                events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
            }
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 2400.0))), time: Some(i as f64 / 60.0), events, ..Default::default() };
            out = Some(ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw(ui, &theme, state, HintDisplay::Full));
            }));
        }
        out.unwrap()
    }

    /// SETTINGS > GAMEPLAY DRAWS EVERY LEVEL OF THE FILE, AND A CLICK CHOOSES ONE (2026-11-11):
    /// each setting's name and each level's name are on the page, the chosen level's sentence is
    /// under its row, and a click on Body heat's Off saves "off" for body heat.
    ///
    /// Seen red 2026-11-11 with `draw` drawing only the first two levels of each setting (the old
    /// two-choice rows): "the page shows Normal for Hostile wildlife".
    #[test]
    fn the_page_draws_every_level_and_a_click_chooses_one() {
        let mut state = GuiState::default();
        let out = frame(&mut state, None);
        let texts: Vec<String> = out.shapes.iter().flat_map(|c| texts_of(&c.shape)).collect();
        for s in &gameplay_levels::file().settings {
            assert!(texts.iter().any(|x| x == t_dyn(&s.words)), "the page names {}", s.id);
            for l in &s.levels {
                assert!(texts.iter().any(|x| x == t_dyn(&l.words)), "the page shows {} for {}", t_dyn(&l.words), t_dyn(&s.words));
            }
        }
        let says = t_dyn("gameplay.body_heat_forgiving");
        assert!(texts.iter().any(|x| x.starts_with(&says[..says.len().min(30)])), "the chosen level's sentence is under its row");
        // Click Body heat's Off: the first "Off" drawn after the words "Body heat".
        let at = crate::gui::widgets::tint_section::texts_at(&out, t_dyn("gameplay.body_heat"))[0];
        let off = crate::gui::widgets::tint_section::texts_at(&out, t_dyn("gameplay.level.off")).into_iter().find(|r| r.top() > at.top()).expect("an Off under Body heat");
        frame(&mut state, Some(off.left_center() - egui::vec2(10.0, 0.0)));
        assert_eq!(state.settings.level("body_heat"), "off", "a click on Body heat's Off chooses it");
        assert!(state.settings_dirty, "and marks the settings to be saved");
    }

    fn texts_of(s: &egui::Shape) -> Vec<String> {
        match s {
            egui::Shape::Text(t) => vec![t.galley.text().to_string()],
            egui::Shape::Vec(v) => v.iter().flat_map(texts_of).collect(),
            _ => Vec::new(),
        }
    }
}
