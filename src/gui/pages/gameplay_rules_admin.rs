//! Server Settings > ADMIN > Gameplay rules (2026-11-21; docs/design/server-rules.md). The
//! operator, that day: "Let's make sure the first server we have setup, via united-humanity is the
//! forgiving game mode. I think realistic will be too much for most people."
//!
//! The rules every player plays by in this server's shared world (src/relay/storage/
//! gameplay_rules.rs): a preset of data/gameplay/presets.json (Forgiving, a new server's, or
//! Realistic), or Custom, a level for each setting of data/gameplay/levels.json picked here. The
//! admin picks, then applies: one `server_settings_update` carrying `gameplay_rules`, which the
//! relay checks is from an admin, saves, and sends to every game in the world at once. The rules
//! shown come from the server's `server_settings_state`. Inside the admin-gated section (the relay
//! checks the role again). The web mirror is web/chat/chat-game-admin.js.

use egui::RichText;
use humanity_core::i18n::{t, t_dyn, tf};

use crate::gui::gameplay_levels;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::GuiState;
use crate::relay::storage::gameplay_rules::{self, GameplayRules, CUSTOM};

/// Draw the panel.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::section_header(ui, theme, t("server_rules.title"));
    widgets::body_hint(ui, theme, t("server_rules.explainer"));
    ui.add_space(theme.spacing_sm);
    let current = state.server_settings.as_ref().map(|s| s.gameplay_rules.clone());
    // A pick that came true on the server is no longer a pick.
    if state.admin_console.rules_draft.is_some() && state.admin_console.rules_draft == current {
        state.admin_console.rules_draft = None;
    }
    match &current {
        Some(c) => ui.label(RichText::new(tf("server_rules.now", &[("preset", &super::settings_gameplay::preset_name(&c.preset))])).size(theme.font_size_small).color(theme.text_primary())),
        None => ui.label(RichText::new(t("server_rules.not_connected")).size(theme.font_size_small).color(theme.text_secondary())),
    };
    let chosen = state.admin_console.rules_draft.clone().or_else(|| current.clone()).unwrap_or_default();
    // The presets, then Custom, each saying what it changes.
    ui.horizontal_wrapped(|ui| {
        for p in &gameplay_rules::presets().presets {
            let selected = chosen.preset == p.id;
            if ui.radio(selected, RichText::new(t_dyn(&p.words)).color(theme.text_primary())).hint("server_rules.preset_hint").clicked() && !selected {
                state.admin_console.rules_draft = GameplayRules::preset(&p.id);
            }
        }
        let custom = chosen.preset == CUSTOM;
        if ui.radio(custom, RichText::new(t("gameplay.preset.custom")).color(theme.text_primary())).hint("server_rules.preset_hint").clicked() && !custom {
            // Custom starts from the levels in play, so it changes nothing until a level is picked.
            state.admin_console.rules_draft = Some(GameplayRules::custom(chosen.levels_in_play()));
        }
    });
    let says = match gameplay_rules::preset(&chosen.preset) {
        Some(p) => t_dyn(&p.says),
        None => t("gameplay.preset.custom_says"),
    };
    widgets::body_hint(ui, theme, says);
    if chosen.preset == CUSTOM {
        draw_custom(ui, theme, state, &chosen);
    }
    ui.add_space(theme.spacing_sm);
    let differs = current.as_ref() != Some(&chosen);
    ui.add_enabled_ui(differs, |ui| {
        if widgets::Button::primary(t("server_rules.apply")).hint("server_rules.apply_hint").show(ui, theme) {
            state.game_admin_status = if send(state, &chosen) {
                tf("server_rules.asked", &[("preset", &super::settings_gameplay::preset_name(&chosen.preset))])
            } else {
                t("server_rules.not_sent").to_string()
            };
        }
    });
}

/// Custom's levels: each setting a server rules, a radio per level, its sentence as the hint.
fn draw_custom(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, chosen: &GameplayRules) {
    for s in gameplay_levels::file().settings.iter().filter(|s| !s.personal) {
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(t_dyn(&s.words)).color(theme.text_secondary()));
        let level = gameplay_levels::level_of(&chosen.levels, &s.id);
        ui.horizontal_wrapped(|ui| {
            for l in &s.levels {
                let selected = level == l.id;
                if ui.radio(selected, RichText::new(t_dyn(&l.words)).color(theme.text_primary())).hint_dyn(&l.says).clicked() && !selected {
                    let mut next = chosen.clone();
                    next.levels.insert(s.id.clone(), l.id.clone());
                    state.admin_console.rules_draft = Some(next);
                }
            }
        });
    }
    widgets::body_hint(ui, theme, t("server_rules.personal_note"));
}

/// Send `server_settings_update` with only the rules (the other settings are left as they are).
/// False when there is no open connection to send it on.
fn send(state: &GuiState, rules: &GameplayRules) -> bool {
    match state.ws_client.as_ref() {
        Some(client) if client.is_connected() => {
            client.send(&update_frame(rules).to_string());
            true
        }
        _ => false,
    }
}

/// The update the panel sends.
pub fn update_frame(rules: &GameplayRules) -> serde_json::Value {
    serde_json::json!({ "type": "server_settings_update", "gameplay_rules": rules })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draw the panel once, clicking where `click` says (or nowhere).
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
                egui::CentralPanel::default().show(ctx, |ui| draw(ui, &theme, state));
            }));
        }
        out.unwrap()
    }

    /// THE PANEL SHOWS THE SERVER'S RULES AND A PICK BECOMES THE UPDATE (2026-11-21): a server
    /// that plays Forgiving says so; a click on Realistic makes it the admin's pick, and the
    /// update the panel sends names it; Custom starts from the levels in play and draws every
    /// setting the server rules (not the bodies' animation); and the relay reads the frame as the
    /// ruleset it names.
    ///
    /// Seen red 2026-11-21 with Custom starting from no levels: "Custom starts from the levels in
    /// play / left: (\"custom\", {}) / right: (\"custom\", {\"animal_care\": \"realistic\", ...})".
    #[test]
    fn the_panel_shows_the_servers_rules_and_a_pick_becomes_the_update() {
        let mut state = GuiState::default();
        state.server_settings = Some(crate::relay::storage::ServerSettings::default());
        let out = frame(&mut state, None);
        let now = tf("server_rules.now", &[("preset", &t("gameplay.preset.forgiving"))]);
        assert!(!crate::gui::widgets::tint_section::texts_at(&out, &now).is_empty(), "the panel says the server plays Forgiving");
        let at = crate::gui::widgets::tint_section::texts_at(&out, t("gameplay.preset.realistic"))[0];
        frame(&mut state, Some(at.left_center() - egui::vec2(10.0, 0.0)));
        let pick = state.admin_console.rules_draft.clone().expect("a click on Realistic picks it");
        assert_eq!(pick.preset, "realistic");
        let sent: crate::relay::relay::RelayMessage = serde_json::from_value(update_frame(&pick)).expect("the relay reads the update");
        let crate::relay::relay::RelayMessage::ServerSettingsUpdate { gameplay_rules, .. } = sent else { panic!("not a settings update") };
        assert_eq!(gameplay_rules, Some(pick.clone()), "the update names the pick");
        // Custom, from Realistic: every level Realistic plays, and a row per ruled setting.
        let out = frame(&mut state, None);
        let at = crate::gui::widgets::tint_section::texts_at(&out, t("gameplay.preset.custom"))[0];
        let out = { frame(&mut state, Some(at.left_center() - egui::vec2(10.0, 0.0))); frame(&mut state, None) };
        let custom = state.admin_console.rules_draft.clone().expect("Custom picked");
        assert_eq!((custom.preset.as_str(), &custom.levels), (CUSTOM, &pick.levels_in_play()), "Custom starts from the levels in play");
        for s in &gameplay_levels::file().settings {
            let drawn = !crate::gui::widgets::tint_section::texts_at(&out, t_dyn(&s.words)).is_empty();
            assert_eq!(drawn, !s.personal, "{}: drawn in Custom only when the server rules it", s.id);
        }
    }
}
