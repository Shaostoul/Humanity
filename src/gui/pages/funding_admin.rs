//! Server Settings > ADMIN > Funding figures (2026-11-22; docs/design/live-funding.md). The
//! operator, that day: "Can we update the values live? So people can see what current funding is
//! at least via other things not my second job?"
//!
//! The owner (and admins) of a server enter what donations bring in each month, one row per way
//! people give (Patreon, GitHub Sponsors, CashApp...), and what running the project costs, one row
//! per cost, so the Donate page of everyone connected shows them (pages/donate_figures.rs). The
//! figures are edited as a draft, then saved: one `server_settings_update` carrying
//! `funding_figures`, which the relay checks is from an admin or the owner (the existing settings
//! update path, handlers/server_settings_update.rs), holds to its shape, stamps with today's date
//! where a figure is new or changed (or was marked "still right today"), and saves
//! (storage/funding_figures.rs). The figures shown come from the server's `server_settings_state`.
//! Inside the admin-gated section. The web mirror is web/chat/chat-funding-admin.js.

use egui::RichText;
use humanity_core::i18n::{t, tf};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::GuiState;
use crate::relay::storage::funding_figures::{FundingChannel, FundingCost, FundingFigures, MAX_ROWS, MAX_USD};

/// Draw the panel.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::section_header(ui, theme, t("funding_admin.title"));
    widgets::body_hint(ui, theme, t("funding_admin.explainer"));
    ui.add_space(theme.spacing_sm);
    let Some(current) = state.server_settings.as_ref().map(|s| s.funding_figures.clone()) else {
        widgets::body_hint(ui, theme, t("funding_admin.not_connected"));
        return;
    };
    // The working copy: the admin's draft, or the server's figures.
    let mut d = state.admin_console.funding_draft.clone().unwrap_or_else(|| current.clone());

    ui.label(RichText::new(t("funding_admin.channels_heading")).color(theme.text_secondary()));
    let mut drop_channel = None;
    for (i, c) in d.channels.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(140.0)).hint("funding_admin.name_hint");
            ui.add(egui::TextEdit::singleline(&mut c.link).desired_width(260.0)).hint("funding_admin.link_hint");
            amount(ui, theme, &mut c.monthly_usd);
            date_note(ui, theme, &c.as_of);
            if widgets::Button::secondary(t("funding_admin.still_right")).hint("funding_admin.still_right_hint").show(ui, theme) {
                c.as_of.clear();
            }
            if widgets::Button::ghost(t("funding_admin.remove")).hint("funding_admin.remove_hint").show(ui, theme) {
                drop_channel = Some(i);
            }
        });
    }
    if let Some(i) = drop_channel {
        d.channels.remove(i);
    }
    ui.add_enabled_ui(d.channels.len() < MAX_ROWS, |ui| {
        if widgets::Button::secondary(t("funding_admin.add_channel")).hint("funding_admin.add_channel_hint").show(ui, theme) {
            d.channels.push(FundingChannel::default());
        }
    });
    ui.add_space(theme.spacing_sm);

    ui.label(RichText::new(t("funding_admin.costs_heading")).color(theme.text_secondary()));
    let mut drop_cost = None;
    for (i, c) in d.costs.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(200.0)).hint("funding_admin.name_hint");
            amount(ui, theme, &mut c.monthly_usd);
            date_note(ui, theme, &c.as_of);
            if widgets::Button::secondary(t("funding_admin.still_right")).hint("funding_admin.still_right_hint").show(ui, theme) {
                c.as_of.clear();
            }
            if widgets::Button::ghost(t("funding_admin.remove")).hint("funding_admin.remove_hint").show(ui, theme) {
                drop_cost = Some(i);
            }
        });
    }
    if let Some(i) = drop_cost {
        d.costs.remove(i);
    }
    ui.add_enabled_ui(d.costs.len() < MAX_ROWS, |ui| {
        if widgets::Button::secondary(t("funding_admin.add_cost")).hint("funding_admin.add_cost_hint").show(ui, theme) {
            d.costs.push(FundingCost::default());
        }
    });
    ui.add_space(theme.spacing_sm);

    let differs = d != current;
    ui.add_enabled_ui(differs, |ui| {
        if widgets::Button::primary(t("funding_admin.save")).hint("funding_admin.save_hint").show(ui, theme) {
            let ok = send(state, &d);
            state.admin_console.results.insert("funding".into(), (ok, t(if ok { "funding_admin.asked" } else { "funding_admin.not_sent" }).to_string()));
        }
    });
    if let Some((ok, words)) = state.admin_console.results.get("funding") {
        ui.label(RichText::new(words).size(theme.font_size_small).color(if *ok { theme.accent() } else { theme.text_secondary() }));
    }
    widgets::body_hint(ui, theme, t("funding_admin.note"));
    // A draft that came true on the server is no longer a draft.
    state.admin_console.funding_draft = if differs { Some(d) } else { None };
}

/// A monthly amount: dollars, 0 to the relay's ceiling.
fn amount(ui: &mut egui::Ui, _theme: &Theme, usd: &mut f64) {
    ui.add(egui::DragValue::new(usd).range(0.0..=MAX_USD).speed(1.0).prefix("$").suffix(t("funding_admin.usd_unit"))).hint("funding_admin.amount_hint");
}

/// The row's date, or that it is set when saved.
fn date_note(ui: &mut egui::Ui, theme: &Theme, as_of: &str) {
    let words = if as_of.is_empty() { t("funding_admin.as_of_new").to_string() } else { tf("funding_admin.as_of_now", &[("date", &as_of)]) };
    ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
}

/// Send `server_settings_update` with only the figures (the other settings are left as they are).
/// False when there is no open connection to send it on.
fn send(state: &GuiState, figures: &FundingFigures) -> bool {
    match state.ws_client.as_ref() {
        Some(client) if client.is_connected() => {
            client.send(&update_frame(figures).to_string());
            true
        }
        _ => false,
    }
}

/// The update the panel sends.
pub fn update_frame(figures: &FundingFigures) -> serde_json::Value {
    serde_json::json!({ "type": "server_settings_update", "funding_figures": figures })
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
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1200.0))), time: Some(i as f64 / 60.0), events, ..Default::default() };
            out = Some(ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw(ui, &theme, state));
            }));
        }
        out.unwrap()
    }

    /// THE PANEL SHOWS THE SERVER'S FIGURES AND AN EDIT BECOMES THE UPDATE (2026-11-22): a server
    /// with a Patreon row shows its date; "Add a way people give" adds a row to the draft; the
    /// update the panel sends is a settings update the relay reads as those figures; a draft equal
    /// to the server's is no draft; with no connection the panel says so.
    ///
    /// Seen red 2026-11-22 with the add button pushing nothing: "the click added a channel".
    #[test]
    fn the_panel_shows_the_servers_figures_and_an_edit_becomes_the_update() {
        let mut state = GuiState::default();
        let out = frame(&mut state, None);
        assert!(!crate::gui::widgets::tint_section::texts_at(&out, t("funding_admin.not_connected")).is_empty(), "no server, no editor");
        let mut settings = crate::relay::storage::ServerSettings::default();
        settings.funding_figures = FundingFigures { channels: vec![FundingChannel { name: "Patreon".into(), link: "https://www.patreon.com/x".into(), monthly_usd: 20.0, as_of: "2026-11-22".into() }], costs: vec![] };
        state.server_settings = Some(settings);
        let out = frame(&mut state, None);
        assert!(!crate::gui::widgets::tint_section::texts_at(&out, "as of 2026-11-22").is_empty(), "the row's date is shown");
        assert!(state.admin_console.funding_draft.is_none(), "an untouched panel has no draft");
        let at = crate::gui::widgets::tint_section::texts_at(&out, t("funding_admin.add_channel"))[0];
        frame(&mut state, Some(at.center()));
        let draft = state.admin_console.funding_draft.clone().expect("the click added a channel");
        assert_eq!(draft.channels.len(), 2, "the new row is in the draft");
        let sent: crate::relay::relay::RelayMessage = serde_json::from_value(update_frame(&draft)).expect("the relay reads the update");
        let crate::relay::relay::RelayMessage::ServerSettingsUpdate { funding_figures, .. } = sent else { panic!("not a settings update") };
        assert_eq!(funding_figures, Some(draft), "the update names the figures");
    }
}
