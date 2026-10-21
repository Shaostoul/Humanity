//! Dev, the developer tools tab (2026-10-20): Bugs, Testing, Performance, Dev
//! and Planet Tuner in one side list, each section drawn by its own page.
//!
//! It replaced the Platform tab (page carve v0.360), which the operator called
//! redundant on 2026-10-20 ("I feel like the platform page we had before is
//! redundant now"). What Platform held went where it belongs:
//! - Calculator, Notes, Calendar and Browser are HumanityOS tools on the Apps
//!   page (data/gui/humanityos_tools.json), each opening its own page.
//! - Recovery through trusted people is a subsection of Settings > Account,
//!   beside the recovery phrase (gui/pages/settings/recovery.rs).
//! - The five developer tools stayed together here. They are permanent: the
//!   project is in perpetual development, so none of them is trimmed.
//!
//! The web mirror is the Dev Workbench (web/pages/dev.html), which carries the
//! same list of five, linking the ones the website has.

use egui::{Frame, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use super::{bugs, dev, performance, planet_tuner, testing};

/// The Dev tab's sections, in the side list's order: (id, label). The id is
/// what `GuiState::active_dev_section` holds.
pub const SECTIONS: &[(&str, &str)] = &[
    ("bugs", "Bugs"),
    ("testing", "Testing"),
    ("performance", "Performance"),
    ("dev", "Dev"),
    ("planet_tuner", "Planet Tuner"),
];

/// The section a fresh session opens on.
pub const FIRST_SECTION: &str = "bugs";

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::SidePanel::left("dev_section_nav")
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
            let items: Vec<SectionNavItem> =
                SECTIONS.iter().map(|&(id, label)| SectionNavItem::new(id, label, c)).collect();
            if let Some(clicked) = widgets::section_nav(ui, theme, Some("Dev"), &items, &state.active_dev_section) {
                state.active_dev_section = clicked;
            }
        });

    let section = state.active_dev_section.clone();
    match section.as_str() {
        "testing" => testing::draw(ctx, theme, state),
        // Resource budgets increment 1: live GPU/CPU/VRAM/RAM pies.
        "performance" => performance::draw(ctx, theme, state),
        "dev" => dev::draw(ctx, theme, state),
        "planet_tuner" => planet_tuner::draw(ctx, theme, state),
        _ => bugs::draw(ctx, theme, state),
    }
}

#[cfg(test)]
#[path = "dev_tab_tests.rs"]
mod tests;
