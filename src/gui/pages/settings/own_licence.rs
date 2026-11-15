//! Settings > Credits, its first section: HumanityOS's own licences (2026-11-14; LICENSING.md and
//! docs/reference/findings/2026-11-14-licence-change.md). Child of settings.rs.
//!
//! The rest of the Credits page credits other people's data. This says what the project's own work
//! is under (the code AGPL-3.0-or-later, the Library and docs CC BY-SA 4.0, the data and the Accord
//! CC0), and puts the source one press away: the AGPL's own advice for a program people use over a
//! network is that "its interface could display a "Source" link that leads users to an archive of
//! the code" (its "How to Apply These Terms"). The website says the same in its footer
//! (web/shared/shell.js, `footer.licence`) and on its download page (`download.source_licence`).

use egui::RichText;
use humanity_core::i18n::{t, t_marked};

use crate::gui::theme::Theme;
use crate::gui::widgets;

/// The folder-by-folder map of which licence covers what.
const LICENSING_URL: &str = "https://github.com/Shaostoul/Humanity/blob/main/LICENSING.md";
/// The code this app is built from.
const SOURCE_URL: &str = "https://github.com/Shaostoul/Humanity";
/// What anyone may do with the name, and why a fork goes by its own.
const NAME_POLICY_URL: &str = "https://github.com/Shaostoul/Humanity/blob/main/docs/legal/trademark-policy.md";

/// One button that opens a page of the project's own in the browser.
fn link_button(ui: &mut egui::Ui, theme: &Theme, label_key: &'static str, hint_key: &'static str, url: &str) {
    if widgets::Button::secondary(t(label_key)).hint(hint_key).show(ui, theme) {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
}

/// The section, drawn at the top of Settings > Credits.
pub(crate) fn draw_own_licence(ui: &mut egui::Ui, theme: &Theme) {
    ui.label(
        RichText::new(t("credits.own_heading"))
            .size(theme.font_size_heading)
            .strong()
            .color(theme.text_primary()),
    );
    widgets::glossary_words(ui, theme, t_marked("credits.own_body"), theme.font_size_small, theme.text_secondary());
    ui.add_space(theme.spacing_sm);
    ui.horizontal_wrapped(|ui| {
        link_button(ui, theme, "credits.licensing", "credits.licensing_hint", LICENSING_URL);
        link_button(ui, theme, "credits.source", "credits.source_hint", SOURCE_URL);
        link_button(ui, theme, "credits.name_policy", "credits.name_policy_hint", NAME_POLICY_URL);
    });
    ui.add_space(theme.spacing_md);
    ui.separator();
    ui.add_space(theme.spacing_sm);
}
