//! Dev, the developer tools page (2026-10-20): the developer's tools, Performance, Planet Tuner,
//! Claude and Journal, each part drawn by its own page.
//!
//! ITS PARTS ARE THE DEV ROW'S PAGES since navigation rung 2 (2026-11-03; docs/design/navigation.md
//! section 16.1, question 9, the operator: "Yes."): the side list it had went, and the top menu's
//! second row under Dev lists Bugs, Testing, Performance, Planet Tuner, Claude and Journal
//! (data/gui/navigation.json). Bugs and Testing are pages of their own (BugReport, Testing); the
//! other four are this page's parts, which the row opens by setting `GuiState::active_dev_section`.
//! The Dev button itself opens the developer's tools (`FIRST_SECTION`, dev.rs: the creature
//! spawner and friends). A part the row does not name is never shown.
//!
//! It replaced the Platform tab (page carve v0.360), which the operator called
//! redundant on 2026-10-20 ("I feel like the platform page we had before is
//! redundant now"). What Platform held went where it belongs:
//! - Calculator, Notes, Calendar and Browser are HumanityOS tools on the Apps
//!   page (data/gui/humanityos_tools.json), each opening its own page.
//! - Recovery through trusted people is a subsection of Settings > Account,
//!   beside the recovery phrase (gui/pages/settings/recovery.rs).
//! - The developer tools stayed together here. They are permanent: the
//!   project is in perpetual development, so none of them is trimmed.
//! - Claude (2026-10-22, the operator's integrations between HumanityOS and
//!   Claude Code) joined them: what the agents are doing, read from this
//!   computer's files (dev_agents/).
//! - Journal (2026-10-24): the play journal, what the game recorded and the F8 marks
//!   (dev_journal.rs, docs/design/play-journal.md).
//!
//! The web mirror is the Dev Workbench (web/pages/dev.html), which carries the
//! same list, linking the ones the website has (its Claude tab shows what a
//! website can reach).

use crate::gui::GuiState;
use crate::gui::theme::Theme;
use super::{dev, dev_agents, dev_journal, performance, planet_tuner};

/// The Dev page's parts: (id, what it is). The id is what `GuiState::active_dev_section` holds and
/// what a Dev row item's `section` names in data/gui/navigation.json (gui/navigation.rs refuses a
/// part not listed here).
pub const SECTIONS: &[(&str, &str)] = &[
    ("dev", "the developer's tools"),
    ("performance", "Performance"),
    ("planet_tuner", "Planet Tuner"),
    ("claude", "Claude"),
    ("journal", "Journal"),
];

/// The part the Dev button opens: the developer's tools.
pub const FIRST_SECTION: &str = "dev";

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let section = state.active_dev_section.clone();
    match section.as_str() {
        // Resource budgets increment 1: live GPU/CPU/VRAM/RAM pies.
        "performance" => performance::draw(ctx, theme, state),
        "planet_tuner" => planet_tuner::draw(ctx, theme, state),
        // What Claude is doing (2026-10-22): read-only, from this computer's files.
        "claude" => dev_agents::draw(ctx, theme, state),
        // The play journal (2026-10-24): what the game recorded, and the F8 marks.
        "journal" => dev_journal::draw(ctx, theme, state),
        _ => dev::draw(ctx, theme, state),
    }
}

#[cfg(test)]
#[path = "dev_tab_tests.rs"]
mod tests;
