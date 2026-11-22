//! The Donate page's "This month" card (2026-11-22; docs/design/live-funding.md). The operator,
//! that day: "Can we update the values live? So people can see what current funding is at least
//! via other things not my second job?"
//!
//! What the CONNECTED server's donations bring in each month, channel by channel, against what
//! running the project costs, each figure with the date it was last confirmed, and one plain
//! sentence: "Donations cover $20 of $241 a month." The figures come from `/api/server-info`'s
//! `funding.channels` and `funding.costs` (storage/funding_figures.rs), set by the server's owner
//! in Server Settings (pages/funding_admin.rs). A server that publishes none draws nothing. The
//! web mirror is web/pages/donate-app.js.

use egui::RichText;
use humanity_core::i18n::{t, tf};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::relay::storage::funding_figures::{money, FundingFigures};

/// The one sentence: what donations cover of what the project costs.
pub fn covered_sentence(f: &FundingFigures) -> String {
    let (given, cost) = (money(f.channels_total()), money(f.costs_total()));
    if f.costs_total() <= 0.0 {
        tf("donate.figures_brings", &[("given", &given)])
    } else if f.channels_total() >= f.costs_total() {
        tf("donate.figures_covered_all", &[("cost", &cost)])
    } else {
        tf("donate.figures_covered", &[("given", &given), ("cost", &cost)])
    }
}

/// Draw the card; nothing when the server publishes no figures.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, figures: &FundingFigures) {
    if figures.is_empty() {
        return;
    }
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("donate.figures_title")).size(theme.font_size_heading).color(theme.text_primary()));
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(covered_sentence(figures)).size(theme.font_size_body).color(theme.accent()));
        ui.add_space(theme.spacing_sm);
        if !figures.channels.is_empty() {
            ui.label(RichText::new(t("donate.figures_channels")).size(theme.font_size_small).color(theme.text_secondary()));
            for c in &figures.channels {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&c.name).size(theme.font_size_body).color(theme.text_primary()));
                    ui.label(RichText::new(tf("donate.figures_per_month", &[("amount", &money(c.monthly_usd))])).size(theme.font_size_body).color(theme.accent()));
                    as_of(ui, theme, &c.as_of);
                    if !c.link.is_empty() && widgets::Button::secondary(t("donate.open")).hint("donate.figures_open_hint").show(ui, theme) {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(&c.link));
                    }
                });
            }
            ui.add_space(theme.spacing_sm);
        }
        if !figures.costs.is_empty() {
            ui.label(RichText::new(t("donate.figures_costs")).size(theme.font_size_small).color(theme.text_secondary()));
            for c in &figures.costs {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&c.name).size(theme.font_size_body).color(theme.text_primary()));
                    ui.label(RichText::new(tf("donate.figures_per_month", &[("amount", &money(c.monthly_usd))])).size(theme.font_size_body).color(theme.accent()));
                    as_of(ui, theme, &c.as_of);
                });
            }
            ui.add_space(theme.spacing_sm);
        }
        widgets::body_hint(ui, theme, t("donate.figures_note"));
    });
    ui.add_space(theme.spacing_lg);
}

/// "as of 2026-11-22", when the row carries a date.
fn as_of(ui: &mut egui::Ui, theme: &Theme, date: &str) {
    if !date.is_empty() {
        ui.label(RichText::new(tf("donate.figures_as_of", &[("date", &date)])).size(theme.font_size_small).color(theme.text_muted()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::GuiState;
    use crate::relay::storage::funding_figures::{FundingChannel, FundingCost};

    fn figures() -> FundingFigures {
        FundingFigures {
            channels: vec![
                FundingChannel { name: "Patreon".into(), link: "https://www.patreon.com/x".into(), monthly_usd: 20.0, as_of: "2026-11-22".into() },
                FundingChannel { name: "CashApp".into(), link: String::new(), monthly_usd: 0.0, as_of: "2026-11-22".into() },
            ],
            costs: vec![FundingCost { name: "Servers".into(), monthly_usd: 41.0, as_of: "2026-11-20".into() }, FundingCost { name: "Tools".into(), monthly_usd: 200.0, as_of: "2026-11-20".into() }],
        }
    }

    /// Draw the whole Donate page headlessly and say whether `text` is on it.
    fn page_shows(state: &mut GuiState, text: &str) -> bool {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut out = None;
        for _ in 0..2 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 4000.0))), ..Default::default() };
            out = Some(ctx.run(input, |ctx| crate::gui::pages::donate::draw(ctx, &theme, state)));
        }
        crate::gui::screen_surface::find_text_in_shapes(&out.expect("two frames ran").shapes, text).is_some()
    }

    /// THE DONATE PAGE SHOWS THE CHANNELS, THE COSTS AND THE COVERED SENTENCE (2026-11-22): a
    /// server's published figures reach the page: each channel with its amount and its date, each
    /// cost, and "Donations cover $20 of $241 a month."; the same figures arriving as a server's
    /// `funding` JSON (apply_server_funding) land on the page state.
    ///
    /// Seen red 2026-11-22 with the page's `donate_figures::draw` call left out of donate.rs: "the
    /// card's title" (the page never drew the card).
    #[test]
    fn the_donate_page_shows_the_channels_the_costs_and_the_covered_sentence() {
        let mut state = GuiState::default();
        state.server_url = "https://united-humanity.us".to_string();
        let served = serde_json::json!({ "enabled": true, "channels": figures().channels, "costs": figures().costs });
        state.apply_server_funding("https://united-humanity.us", Some(&served));
        assert_eq!(state.donate_figures, figures(), "the served figures land on the page state");
        assert!(page_shows(&mut state, "This month"), "the card's title");
        assert!(page_shows(&mut state, "Donations cover $20 of $241 a month."), "the covered sentence");
        assert!(page_shows(&mut state, "Patreon"), "a channel");
        assert!(page_shows(&mut state, "$20 a month"), "a channel's amount");
        assert!(page_shows(&mut state, "Servers"), "a cost");
        assert!(page_shows(&mut state, "$41 a month"), "a cost's amount");
        assert!(page_shows(&mut state, "as of 2026-11-22"), "a channel's date");
        // Donations that meet the costs say so; a server with no costs says what comes in.
        let mut paid = figures();
        paid.channels[0].monthly_usd = 300.0;
        assert_eq!(covered_sentence(&paid), "Donations cover all $241 a month.");
        assert_eq!(covered_sentence(&FundingFigures { costs: vec![], ..figures() }), "Donations bring in $20 a month.");
    }

    /// NOTHING SHOWS WHEN NONE ARE PUBLISHED (2026-11-22): a server whose `funding` carries no
    /// figures (or no funding at all) leaves the card off the page, and a server that publishes
    /// then stops clears it (the reconnect rule of apply_server_funding).
    ///
    /// Seen red 2026-11-22 with `draw` not returning early on empty figures: "the card is drawn
    /// with no figures"; and with the page's call left out, its positive half failed ("assertion
    /// failed: page_shows(&mut state, \"This month\")"), so the check can fail both ways.
    #[test]
    fn nothing_shows_when_none_are_published() {
        let mut state = GuiState::default();
        state.server_url = "https://united-humanity.us".to_string();
        state.apply_server_funding("https://united-humanity.us", Some(&serde_json::json!({ "enabled": true, "goal_usd": 100.0 })));
        assert!(state.donate_figures.is_empty());
        assert!(!page_shows(&mut state, "This month"), "the card is drawn with no figures");
        state.apply_server_funding("https://united-humanity.us", Some(&serde_json::json!({ "channels": figures().channels, "costs": figures().costs })));
        assert!(page_shows(&mut state, "This month"));
        state.apply_server_funding("https://united-humanity.us", None);
        assert!(!page_shows(&mut state, "This month"), "a server that stops publishing clears the card");
    }
}
