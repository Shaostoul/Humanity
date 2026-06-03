//! Platform — the software-itself tab (page carve, v0.360).
//!
//! Folds Settings + Recovery + Tools + Bugs + Testing + Browser into one tab via
//! a `section_nav` sidebar, delegating content to each page's `draw`. Takes
//! `&mut Theme` because the Settings section edits the live theme; the other
//! sections take `&Theme` (a `&mut` coerces down).

use egui::{Frame, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use super::{settings, recovery, tools, bugs, testing, browser};

pub fn draw(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    egui::SidePanel::left("platform_section_nav")
        .default_width(190.0)
        .min_width(150.0)
        .max_width(260.0)
        .frame(
            Frame::none()
                .fill(theme.bg_sidebar())
                .inner_margin(egui::Margin::symmetric(8, 12))
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ctx, |ui| {
            let c = theme.nav_settings();
            let items = [
                SectionNavItem::new("settings", "Settings", c),
                SectionNavItem::new("recovery", "Recovery", c),
                SectionNavItem::new("tools", "Tools", c),
                SectionNavItem::new("bugs", "Bugs", c),
                SectionNavItem::new("testing", "Testing", c),
                SectionNavItem::new("browser", "Browser", c),
            ];
            if let Some(clicked) = widgets::section_nav(
                ui,
                theme,
                Some("Platform"),
                &items,
                &state.active_platform_section,
            ) {
                state.active_platform_section = clicked;
            }
        });

    let section = state.active_platform_section.clone();
    match section.as_str() {
        "recovery" => recovery::draw(ctx, theme, state),
        "tools" => tools::draw(ctx, theme, state),
        "bugs" => bugs::draw(ctx, theme, state),
        "testing" => testing::draw(ctx, theme, state),
        "browser" => browser::draw(ctx, theme, state),
        _ => settings::draw(ctx, theme, state),
    }
}
