//! Platform — the software-itself tab (page carve, v0.360; trimmed v0.361).
//!
//! Folds Recovery + Tools + Bugs + Testing + Browser into one tab via a
//! `section_nav` sidebar, delegating content to each page's `draw`. Settings was
//! pulled OUT to its own top-level tab (operator 2026-06-04: "have settings as
//! its own top level page ... never buried in another menu"), so this no longer
//! needs `&mut Theme`.

use egui::{Frame, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use super::{recovery, tools, bugs, testing, browser, calculator, files};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
            let c = theme.nav_tools();
            let items = [
                SectionNavItem::new("recovery", "Recovery", c),
                SectionNavItem::new("tools", "Tools", c),
                SectionNavItem::new("calculator", "Calculator", c),
                SectionNavItem::new("files", "Files", c),
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
        "tools" => tools::draw(ctx, theme, state),
        "calculator" => calculator::draw(ctx, theme, state),
        "files" => files::draw(ctx, theme, state),
        "bugs" => bugs::draw(ctx, theme, state),
        "testing" => testing::draw(ctx, theme, state),
        "browser" => browser::draw(ctx, theme, state),
        _ => recovery::draw(ctx, theme, state),
    }
}
