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
//!
//! IN A SERVER'S SHARED WORLD (2026-11-21, gui/server_rules.rs) the levels are the server's: each
//! group opens with a note naming the server's rules, and every setting the server rules is drawn
//! greyed at the server's level, tagged "this server's rules". The person's own choices are kept
//! underneath and come back when they leave the world.

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
    draw_server_note(ui, theme, state, hint);
    for s in &gameplay_levels::file().settings {
        ui.add_space(theme.spacing_sm);
        draw_setting(ui, theme, state, hint, s);
    }
}

/// Draw the settings of one group of the file (`Setting::group`: the survival rows at the top of
/// the page, the garden's and the ship's further down), in the file's order.
pub fn draw_group(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, group: &str) {
    draw_server_note(ui, theme, state, hint);
    for s in gameplay_levels::file().settings.iter().filter(|s| s.group == group) {
        ui.add_space(theme.spacing_sm);
        draw_setting(ui, theme, state, hint, s);
    }
}

/// While in a server's shared world: which rules play, and that the choices below are its.
fn draw_server_note(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, hint: HintDisplay) {
    if let Some(r) = &state.settings.server_rules {
        ui.add_space(theme.spacing_sm);
        widgets::setting_hint(ui, theme, hint, &tf("gameplay.server_rules_note", &[("preset", &preset_name(&r.preset))]));
    }
}

/// A ruleset's name: its preset's words, or Custom.
pub fn preset_name(preset: &str) -> String {
    match crate::relay::storage::gameplay_rules::preset(preset) {
        Some(p) => t_dyn(&p.words).to_string(),
        None => t_dyn("gameplay.preset.custom").to_string(),
    }
}

/// One setting: its name, a radio per level, the chosen level's sentence, and its note. Greyed
/// at the server's level while the server rules it.
fn draw_setting(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, s: &Setting) {
    let ruled = state.settings.server_rules_setting(&s.id);
    if ruled {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t_dyn(&s.words)).color(theme.text_secondary()));
            ui.label(RichText::new(t_dyn("gameplay.server_rules_tag")).size(theme.font_size_small).color(theme.accent()));
        });
    } else {
        // Out of a server's world the row is drawn exactly as before the rules (a plain label).
        ui.label(RichText::new(t_dyn(&s.words)).color(theme.text_secondary()));
    }
    let chosen = chosen_level(state, s);
    if ruled {
        ui.add_enabled_ui(false, |ui| draw_choices(ui, theme, state, s, chosen.as_deref()));
    } else {
        draw_choices(ui, theme, state, s, chosen.as_deref());
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

/// A setting's radios (and its slider): a click chooses the person's own level.
fn draw_choices(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, s: &Setting, chosen: Option<&str>) {
    ui.horizontal_wrapped(|ui| {
        for l in &s.levels {
            let selected = chosen == Some(l.id.as_str());
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
        // Greyed at the server's number while it rules the drain; the person's own otherwise.
        let mut server = state.settings.vitals_drain_rate();
        let value = if state.settings.server_rules_setting(&s.id) { &mut server } else { &mut state.settings.vitals_drain };
        if widgets::with_hint(ui, "gameplay.level_slider_hint", |ui| widgets::labeled_slider(ui, theme, t_dyn(&s.words), value, lo..=hi)) {
            state.settings_dirty = true;
        }
    }
}

/// The level chosen: the server's while it rules the setting, else the saved one, or for the
/// vitals drain the level whose number the slider sits on (None between two of them).
fn chosen_level(state: &GuiState, s: &Setting) -> Option<String> {
    if s.id == VITALS_DRAIN && !state.settings.server_rules_setting(&s.id) {
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
        assert!(texts.iter().any(|x| x.starts_with(humanity_core::text::prefix(&says, 30))), "the chosen level's sentence is under its row");
        // Click Body heat's Off: the first "Off" drawn after the words "Body heat".
        let at = crate::gui::widgets::tint_section::texts_at(&out, t_dyn("gameplay.body_heat"))[0];
        let off = crate::gui::widgets::tint_section::texts_at(&out, t_dyn("gameplay.level.off")).into_iter().find(|r| r.top() > at.top()).expect("an Off under Body heat");
        frame(&mut state, Some(off.left_center() - egui::vec2(10.0, 0.0)));
        assert_eq!(state.settings.level("body_heat"), "off", "a click on Body heat's Off chooses it");
        assert!(state.settings_dirty, "and marks the settings to be saved");
    }

    /// IN A SERVER'S SHARED WORLD THE PAGE SHOWS THE SERVER'S RULES, GREYED (2026-11-21): with
    /// the person's own body heat at Off and the server's rules Realistic, the page names the
    /// server's rules in its note, tags every setting the server rules (all but the bodies'
    /// animation) "this server's rules", says Realistic's sentence under Body heat rather than
    /// Off's, and a click on Body heat's Off changes nothing. Out of the world, the person's own.
    ///
    /// Seen red 2026-11-21 with `SettingsState::level` reading the person's own map as before the
    /// rules: "Body heat says the server's Realistic, not the person's Off".
    #[test]
    fn in_a_servers_world_the_page_shows_its_rules_greyed() {
        let mut state = GuiState::default();
        state.settings.set_level("body_heat", "off");
        let realistic = crate::relay::storage::GameplayRules::preset("realistic").unwrap();
        state.settings.server_rules = Some(crate::relay::storage::GameplayRules { levels: realistic.levels_in_play(), ..realistic });
        let out = frame(&mut state, None);
        let texts: Vec<String> = out.shapes.iter().flat_map(|c| texts_of(&c.shape)).collect();
        let note = tf("gameplay.server_rules_note", &[("preset", &t_dyn("gameplay.preset.realistic"))]);
        assert!(texts.iter().any(|x| x.starts_with(humanity_core::text::prefix(&note, 40))), "the page names this server's rules while joined");
        let tag = t_dyn("gameplay.server_rules_tag");
        let ruled = gameplay_levels::file().settings.iter().filter(|s| !s.personal).count();
        assert_eq!(texts.iter().filter(|x| *x == tag).count(), ruled, "every setting the server rules is tagged, the personal one is not");
        let says = t_dyn("gameplay.body_heat_realistic");
        assert!(texts.iter().any(|x| x.starts_with(humanity_core::text::prefix(&says, 30))), "Body heat says the server's Realistic, not the person's Off");
        let at = crate::gui::widgets::tint_section::texts_at(&out, t_dyn("gameplay.body_heat"))[0];
        let off = crate::gui::widgets::tint_section::texts_at(&out, t_dyn("gameplay.level.off")).into_iter().find(|r| r.top() > at.top()).expect("an Off under Body heat");
        frame(&mut state, Some(off.left_center() - egui::vec2(10.0, 0.0)));
        assert!(!state.settings_dirty, "a greyed choice cannot be clicked");
        state.settings.server_rules = None;
        let texts: Vec<String> = frame(&mut state, None).shapes.iter().flat_map(|c| texts_of(&c.shape)).collect();
        assert!(!texts.iter().any(|x| *x == tag), "out of the world: no server's rules on the page");
        assert_eq!(state.settings.level("body_heat"), "off", "and the person's own Off again");
    }

    fn texts_of(s: &egui::Shape) -> Vec<String> {
        match s {
            egui::Shape::Text(t) => vec![t.galley.text().to_string()],
            egui::Shape::Vec(v) => v.iter().flat_map(texts_of).collect(),
            _ => Vec::new(),
        }
    }
}
