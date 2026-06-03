//! Humanity — the collective / mission tab (page carve, v0.360).
//!
//! What the **H** button opens. Folds the Community/Mission Dashboard
//! (Civilization) + Governance, Directory (Identity), Onboarding, Donate,
//! Resources into one tab via a `section_nav` sidebar, delegating content to each
//! page's `draw`. The rich mission page (per `docs/design/humanity-page.md` —
//! three scopes, end-poverty-via-voluntary-cooperation, AI as Humanity) replaces
//! the placeholder Civilization dashboard content incrementally; the fold makes
//! them one tab now so the nav condenses and the mission has a home.

use egui::{Frame, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use super::{civilization, governance, identity, onboarding, donate, resources};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::SidePanel::left("humanity_section_nav")
        .default_width(200.0)
        .min_width(160.0)
        .max_width(270.0)
        .frame(
            Frame::none()
                .fill(theme.bg_sidebar())
                .inner_margin(egui::Margin::symmetric(8, 12))
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ctx, |ui| {
            let c = theme.nav_reality();
            let items = [
                SectionNavItem::new("civilization", "Mission Dashboard", c),
                SectionNavItem::new("governance", "Governance", c),
                SectionNavItem::new("identity", "Directory", c),
                SectionNavItem::new("onboarding", "Onboarding", c),
                SectionNavItem::new("donate", "Donate", c),
                SectionNavItem::new("resources", "Resources", c),
            ];
            if let Some(clicked) = widgets::section_nav(
                ui,
                theme,
                Some("Humanity"),
                &items,
                &state.active_humanity_section,
            ) {
                state.active_humanity_section = clicked;
            }
        });

    let section = state.active_humanity_section.clone();
    match section.as_str() {
        "governance" => governance::draw(ctx, theme, state),
        "identity" => identity::draw(ctx, theme, state),
        "onboarding" => onboarding::draw(ctx, theme, state),
        "donate" => donate::draw(ctx, theme, state),
        "resources" => resources::draw(ctx, theme, state),
        _ => civilization::draw(ctx, theme, state),
    }
}
