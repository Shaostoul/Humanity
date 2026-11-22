//! THE MUSIC PAGE'S KEYBOARD SHORTCUTS (docs/design/music-page.md sections 16 and 28): Space pause or
//! play, N or Right the next item, P or Left the previous (through the one list, the same for every
//! kind: queue.rs), S shuffle, R repeat (off, all, one), U Up next, L like, 1 to 4 the tabs, [ and ]
//! fewer or more bars in the piano roll, F follow the music, / the Written music search. Only while no text box
//! has the keyboard (egui's `wants_keyboard_input`): typing a seed, a name or a search never plays,
//! skips or likes anything, and only with no modifier held (Ctrl+N stays the app's). Listed in the
//! page's own help under the title, words by key.

use egui::{Key, Modifiers, RichText};
use humanity_core::i18n::t;

use super::{roll, Shown, Tab};
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::GuiState;

/// Each shortcut as the help lists it: its keys' words and what it does, by key.
pub const HELP: [(&str, &str); 11] = [
    ("music_page.key_space", "music_page.key_space_does"),
    ("music_page.key_next", "music_page.key_next_does"),
    ("music_page.key_prev", "music_page.key_prev_does"),
    ("music_page.key_shuffle", "music_page.key_shuffle_does"),
    ("music_page.key_repeat", "music_page.key_repeat_does"),
    ("music_page.key_up_next", "music_page.key_up_next_does"),
    ("music_page.key_like", "music_page.key_like_does"),
    ("music_page.key_tabs", "music_page.key_tabs_does"),
    ("music_page.key_bars", "music_page.key_bars_does"),
    ("music_page.key_follow", "music_page.key_follow_does"),
    ("music_page.key_search", "music_page.key_search_does"),
];

/// Act on the shortcuts pressed this frame (taking each key, so nothing else acts on it too). `sh` is
/// the piece as the page shows it (Previous, Next and Like change or keep it).
pub(super) fn handle(ctx: &egui::Context, state: &mut GuiState, sh: &mut Shown) {
    if ctx.wants_keyboard_input() || state.music_page.import_picker.is_some() || state.music_page.export_picker.is_some() {
        return;
    }
    let pressed = |k: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
    if pressed(Key::Space) {
        super::transport::play_pause(state);
    }
    // Next and Previous through the one list, the same for every kind of music (queue.rs).
    if pressed(Key::N) | pressed(Key::ArrowRight) {
        super::transport::next(state);
    }
    if pressed(Key::P) | pressed(Key::ArrowLeft) {
        super::transport::previous(state);
    }
    if pressed(Key::S) {
        super::transport::toggle_shuffle(state);
    }
    if pressed(Key::R) {
        super::transport::cycle_repeat(state);
    }
    if pressed(Key::U) {
        state.music_page.up_next_open = !state.music_page.up_next_open;
    }
    if pressed(Key::L) && state.settings.music.written.is_empty() {
        super::like::like(state, sh);
    }
    for (k, tab) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4].into_iter().zip(Tab::ALL) {
        if pressed(k) {
            state.music_page.tab = tab;
        }
    }
    let total = state.music_page.sheet.as_ref().map(|s| s.sheet.bars.len()).unwrap_or(0);
    let fewer = pressed(Key::OpenBracket);
    let more = pressed(Key::CloseBracket);
    if (fewer || more) && total > 0 {
        let steps = roll::length_steps(total);
        let shown = roll::bars_shown(state.music_page.roll_bars, total);
        let k = steps.iter().position(|&s| roll::bars_shown(s, total) >= shown).unwrap_or(steps.len() - 1);
        let k = if more { (k + 1).min(steps.len() - 1) } else { k.saturating_sub(1) };
        state.music_page.roll_bars = steps[k];
    }
    if pressed(Key::F) {
        state.music_page.roll_follow = !state.music_page.roll_follow;
    }
    if pressed(Key::Slash) {
        state.music_page.tab = Tab::Written;
        ctx.memory_mut(|m| m.request_focus(egui::Id::new(super::written::SEARCH_ID)));
    }
}

/// The page's help for its keys: a fold-out under the title listing each shortcut.
pub(super) fn help(ui: &mut egui::Ui, theme: &Theme) {
    let r = egui::CollapsingHeader::new(RichText::new(t("music_page.keys_title")).size(theme.font_size_small).color(theme.text_secondary())).id_salt("music_page_keys").default_open(false).show(ui, |ui| {
        egui::Grid::new("music_page_keys_grid").num_columns(2).spacing([theme.spacing_md, theme.spacing_xs]).show(ui, |ui| {
            for (keys, does) in HELP {
                ui.label(RichText::new(t(keys)).strong().color(theme.text_primary()));
                ui.label(RichText::new(t(does)).color(theme.text_secondary()));
                ui.end_row();
            }
        });
        ui.label(RichText::new(t("music_page.keys_text_note")).size(theme.font_size_small).color(theme.text_muted()));
    });
    r.header_response.hint("music_page.keys_hint");
}
