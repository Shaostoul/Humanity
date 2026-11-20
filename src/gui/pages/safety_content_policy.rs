//! Settings > Safety > What is not allowed, and how to report it (2026-11-20): two sentences and
//! one button that opens the content policy in the Library (docs/legal/content-policy.md, copied
//! into data/library/ by scripts/build-library.js under the slug `content-policy`).
//!
//! WHY. The Digital Public Goods Standard's indicator 9b asks for a written policy on illegal
//! content, child sexual abuse material first, and the processes for reporting and removing it
//! (docs/reference/findings/2026-11-18-digital-public-goods.md); the policy rests on the dated
//! finding docs/reference/findings/2026-11-20-csam-reporting.md. A policy nobody can find protects
//! no one, so it is one press away from the page where a person chooses who can reach them. The
//! words say the two things a person most needs before they read further: sealed messages mean a
//! report starts from the conversation (never by forwarding a picture), and the emergency number
//! comes first.
//!
//! The website's Safety panel mirrors it (web/chat/chat-privacy.js `contentPolicySafetyHtml`), and
//! the site's footer links the same page. Words and the button's hint by key (`content_policy.*`
//! in data/i18n/en.json). Child of safety.rs, drawn last on the page, as on the website.

use humanity_core::i18n::{t, t_marked};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState, How};

/// The Library slug of docs/legal/content-policy.md (`gui/loaders.rs` `library_slug_of_file`;
/// the website links `/library#content-policy`).
pub(crate) const CONTENT_POLICY_SLUG: &str = "content-policy";

/// The section, drawn at the end of Settings > Safety, online or not (the policy is the
/// project's, not a server's).
pub(crate) fn draw_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32) {
    widgets::subsection_header(ui, theme, accent, t("content_policy.heading"), "");
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        widgets::glossary_words(ui, theme, t_marked("content_policy.body"), theme.font_size_small, theme.text_muted());
        ui.add_space(theme.spacing_sm);
        let pressed = ui.scope(|ui| {
            widgets::Button::secondary(t("content_policy.open")).hint("content_policy.open_hint").show(ui, theme)
        });
        #[cfg(test)]
        tests::record_button(pressed.response.rect);
        if pressed.inner {
            open_policy(state);
        }
    });
}

/// Open the Library on the content policy. Should the shipped Library lack it (a data/ folder
/// from before 2026-11-20), the Library opens as it is and the log says so; the test below keeps
/// the slug shipped.
pub(crate) fn open_policy(state: &mut GuiState) {
    if !super::library::show_doc(state, CONTENT_POLICY_SLUG) {
        log::warn!("Safety: the Library has no {CONTENT_POLICY_SLUG:?} document; opening the Library as it is");
    }
    state.go(GuiPage::Library, How::From);
}

#[cfg(test)]
#[path = "safety_content_policy_tests.rs"]
mod tests;
