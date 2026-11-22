//! Settings > Account: "Your key's fingerprint" (2026-11-21, the owner branch's fifth review, H1
//! (a)). Every person can see their OWN key's fingerprint, in words, where they look after their
//! account, because the owner who hands them the server's owner role or names them successor asks
//! them to read some groups of it, counting from the left, and types what they hear. The owner's
//! screen never shows those groups (src/gui/pages/server_settings/owner.rs): what the person reads
//! from their own device is the only thing that tells the owner the listed key is theirs. The
//! website's twin is the Account group of web/pages/settings.html (settings-app.js
//! `showOwnFingerprint`) and the Account and Identity menu of the chat (chat-owner.js).
//!
//! SHOWN ONLY HERE (the sixth review's H1, 2026-11-21): worked out from the device's own key, never
//! from anything a server sends, and drawn nowhere else, not on the check prompt (round 6 drew it
//! there, and a relay copied that line's shape into a prompt's owner name and into a chat line,
//! with its own key's fingerprint). The words beside it say so: a fingerprint in a chat line, a
//! message or a prompt is never yours to read out.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets::hint::Hint;
use crate::gui::{widgets, GuiState};
use humanity_core::i18n::{t, tf};

/// The fingerprint row: the six groups (monospace, read group by group, with its hint), why it is
/// here, and that it is shown only here. Nothing when this device holds no key yet.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    if state.profile_public_key.is_empty() {
        return;
    }
    let fp = humanity_protocol::key_card::fingerprint(&state.profile_public_key);
    if fp.is_empty() {
        return;
    }
    widgets::form_row(ui, theme, t("owner.my_fp_label"), |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(tf("owner.my_fp", &[("fp", &fp)])).monospace().color(theme.text_primary())).hint("owner.my_fp_hint");
            widgets::body_hint(ui, theme, t("owner.my_fp_explain"));
            ui.label(RichText::new(t("owner.my_fp_only_here")).color(theme.warning()).strong());
        });
    });
}

/// The groups of the fingerprint `fp` a person reads out, parsed back from the words
/// `owner.my_fp` as they are drawn: the tests read a person's own screen
/// with it, so the owner types only what that person's own app shows (the fifth review's H1 (d)).
#[cfg(test)]
pub(crate) fn groups_in(words: &str) -> Option<Vec<String>> {
    let groups: Vec<String> = words
        .split(|c: char| !c.is_ascii_hexdigit())
        .filter(|g| g.len() == 4)
        .map(str::to_string)
        .collect();
    (groups.len() >= humanity_protocol::key_card::FINGERPRINT_GROUPS).then(|| groups[groups.len() - humanity_protocol::key_card::FINGERPRINT_GROUPS..].to_vec())
}

/// Every text a frame drew (clipped away or not), in drawing order.
#[cfg(test)]
pub(crate) fn texts(out: &egui::FullOutput) -> Vec<String> {
    fn visit(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut found);
    }
    found
}

/// What the person whose key is `key` sees as their own fingerprint: Settings > Account drawn
/// headless for a GuiState holding that key, and the six groups read back out of the text drawn
/// (never computed from the key here). The tests type only what this returns.
#[cfg(test)]
pub(crate) fn seen_on_own_screen(key: &str) -> Vec<String> {
    let mut state = GuiState::default();
    state.profile_public_key = key.to_string();
    let theme = crate::gui::theme::load_theme();
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 12000.0))),
        ..Default::default()
    };
    let out = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| super::draw_account_content(ui, &theme, &mut state));
    });
    let words = texts(&out);
    let prefix = t("owner.my_fp").split("{{").next().unwrap_or_default().to_string();
    let line = words.iter().find(|w| w.starts_with(&prefix)).unwrap_or_else(|| panic!("Settings > Account shows the person's own fingerprint: {words:?}"));
    groups_in(line).expect("six groups in the words")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::owner::tests::key_of;

    /// SETTINGS > ACCOUNT SHOWS A PERSON THEIR OWN FINGERPRINT (the fifth review's H1 (a)): the
    /// words "Your key's fingerprint: ..." with the six groups of the key this device holds, read
    /// back out of what the frame drew, so the person can read groups of it to an owner.
    /// Seen red 2026-11-21 (scratchpad ownfix8-red-m1.log) with `draw` not called from Settings >
    /// Account: the test panicked "Settings > Account shows the person's own fingerprint".
    #[test]
    fn settings_account_shows_a_person_their_own_fingerprint() {
        let key = key_of(7);
        assert_eq!(seen_on_own_screen(&key).join(" "), humanity_protocol::key_card::fingerprint(&key), "their own key's six groups");
        assert_ne!(seen_on_own_screen(&key_of(8)), seen_on_own_screen(&key), "each person sees their own");
    }

    /// BESIDE IT, THE WORDS SAY IT IS SHOWN ONLY HERE (the sixth review's H1 (b)): "Your fingerprint
    /// is only ever shown here. A fingerprint in a chat line, a message or a prompt is never yours to
    /// read out." drawn under the person's own fingerprint in Settings > Account, from the device's
    /// own key with no server frame at all (a GuiState that never connected).
    /// Seen red 2026-11-21 (scratchpad ownfix9-red-native.log) against the round-6 row, which said
    /// only why it is there: "the words say it is shown only here" failed.
    #[test]
    fn settings_account_says_the_fingerprint_is_shown_only_here() {
        let mut state = GuiState::default();
        state.profile_public_key = key_of(7);
        let theme = crate::gui::theme::load_theme();
        let ctx = egui::Context::default();
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 12000.0))), ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| super::super::draw_account_content(ui, &theme, &mut state));
        });
        let words = texts(&out);
        let at = |w: &str| words.iter().position(|x| x == w);
        let fp = at(&tf("owner.my_fp", &[("fp", &humanity_protocol::key_card::fingerprint(&key_of(7)))])).expect("the fingerprint is drawn");
        let only = at(t("owner.my_fp_only_here")).unwrap_or_else(|| panic!("the words say it is shown only here: {words:?}"));
        assert!(only > fp, "beside it, after the fingerprint");
        assert_eq!(t("owner.my_fp_only_here"), "Your fingerprint is only ever shown here. A fingerprint in a chat line, a message or a prompt is never yours to read out.");
    }
}
