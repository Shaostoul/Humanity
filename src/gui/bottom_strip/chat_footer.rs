//! The chat footer (rungs 2 and 3 of docs/design/app-bottom-strip.md, section 4; built 2026-11-15):
//! the desktop app's chat as a footer above the unfinished-software warning on every page that
//! carries the top menu, except Chat itself. The operator, 2026-10-30: "Maybe for the desktop app
//! we can make the chat a footer that's anchored and collapsible/expandable to hide the chat on a
//! give nonchat page ... The footer chat would go above the WIP Warning visually." And on
//! 2026-11-15, looking for it: "the other footer widget that's supposed to bring a collapsible
//! chat footer to all the other non-chat pages".
//!
//! THREE STATES ([`Mode`], config.json `chat_footer`):
//! - **Off:** nothing drawn, no room taken. Also on the Chat page (the page is the chat), in the
//!   first-person world, on the title screen, on an in-world screen (the dispatch table never calls
//!   the strip), and when the app is not signed in to a server ([`shows`]).
//! - **Folded** (the default): one line the warning's height: the conversation's name, its newest
//!   message as "name: words" cut to the line, how many other conversations are unread, Open in
//!   Chat, and Show more.
//! - **Unfolded:** a short strip, [`DEFAULT_HEIGHT`] tall at first and resizable from its top edge
//!   between [`MIN_HEIGHT`] and [`MAX_SHARE`] of the window (the height kept in config.json): the
//!   same line on top, the last messages newest at the bottom, and the box you type in.
//!
//! ONE CONVERSATION, TWO VIEWS: it shows `chat_active_channel`, the channel the Chat page has open;
//! its rows are the Chat page's own (`pages/chat/rows.rs`) and so is its composer
//! (`pages/chat/composer.rs`, `Place::Footer`): the same draft (`chat_input`), the same send path
//! (`send_composed_content`: line limits, the send limiter, replies, the protected setup). The
//! unread count is the Chat page's own unread marks, read, never worked out again from messages.
//!
//! THE STACK: `bottom_strip::draw` adds the warning's panel first and this one after it, so it
//! stands on the warning (or on the window's bottom while the warning is muted); the page's own
//! bottom panels come later and stand on it. Its height goes into `bottom_strip::height`, so every
//! window anchored to the bottom (the toasts, the dialogs, the chat videos' dock) stands on it too.
//!
//! THE KEY: data/keymaps.ron's row with the id [`KEY_ID`] (Alt+C as shipped) folds and unfolds it
//! on any page it shows on; the key is read from that row, so the F1 list shows the key that works.
//!
//! COST: folded, one line built from the newest message each frame it draws (a filter over the
//! conversation's messages, microseconds), no cache; unfolded, the Chat page's rows for the last
//! [`ROWS_SHOWN`] messages. It reads no file and never asks for a repaint of its own.

use egui::{Align, Layout, RichText, Sense};
use serde::{Deserialize, Serialize};

use humanity_core::i18n::{t, tf};
use crate::gui::pages::chat::composer::{self, Place};
use crate::gui::pages::chat::rows;
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{GuiPage, GuiState};

/// The keymap row (data/keymaps.ron) whose keys fold and unfold the footer.
pub const KEY_ID: &str = "chat_footer_fold";
/// The unfolded strip's height until the person drags it, in points. The design said "about 180";
/// at 180 the who-can-read line and the box left about 85 points for messages, three lines or so,
/// so 220 (the builder's recommendation, taken at merge 2026-11-15) shows a few more.
pub const DEFAULT_HEIGHT: f32 = 220.0;
/// The lowest the strip may be dragged: its line, two or three messages and the box.
pub const MIN_HEIGHT: f32 = 120.0;
/// The most of the window the strip may take.
pub const MAX_SHARE: f32 = 0.4;
/// How many of the newest messages the strip lays out (the Chat page has the rest).
pub const ROWS_SHOWN: usize = 50;

/// The folded line's panel, by id.
const FOLDED_PANEL: &str = "hos_chat_footer";
/// The unfolded strip's panel, by id (its own, so egui's memory of its height is the strip's).
const OPEN_PANEL: &str = "hos_chat_footer_open";

/// Off, one line, or a short strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Off,
    #[default]
    Folded,
    Unfolded,
}

impl Mode {
    /// Every mode, in the Settings row's order.
    pub const ALL: [Mode; 3] = [Mode::Off, Mode::Folded, Mode::Unfolded];
}

/// The footer's setting: its mode and the unfolded strip's height (config.json `chat_footer`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatFooter {
    #[serde(default)]
    pub mode: Mode,
    #[serde(default = "default_height")]
    pub height: f32,
}

fn default_height() -> f32 {
    DEFAULT_HEIGHT
}

impl Default for ChatFooter {
    fn default() -> Self {
        ChatFooter { mode: Mode::default(), height: DEFAULT_HEIGHT }
    }
}

/// config.json's `chat_footer`, read leniently: a record this build cannot read (a mode it does not
/// know, a height that is not a number) is the default, and never makes the whole file fail to load.
pub fn read_kept<'de, D: serde::Deserializer<'de>>(d: D) -> Result<ChatFooter, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    let mut f: ChatFooter = serde_json::from_value(v).unwrap_or_default();
    if !f.height.is_finite() {
        f.height = DEFAULT_HEIGHT;
    }
    Ok(f)
}

/// Is the app signed in to a server, so there is a conversation to show: a server it connected to,
/// not since disconnected by hand. Not the socket's state of the moment, so a reconnect does not
/// make the footer come and go.
pub(crate) fn signed_in(state: &GuiState) -> bool {
    !state.connected_server_url.is_empty() && !state.ws_manually_disconnected && !state.chat_active_channel.is_empty()
}

/// Whether the footer belongs on this page: a page behind the top menu, not the Chat page, not the
/// first-person world, not the title screen.
pub(crate) fn on_page(page: GuiPage) -> bool {
    !matches!(page, GuiPage::None | GuiPage::Chat | GuiPage::MainMenu)
}

/// Whether the footer draws this frame (section 4.2's Off, all of it).
pub(crate) fn shows(state: &GuiState) -> bool {
    state.chat_footer.mode != Mode::Off && on_page(state.active_page) && signed_in(state)
}

/// How many other conversations have messages not read yet: the Chat page's unread marks (the
/// active server's rooms and conversations, and each other signed-in server's), read as they are.
pub(crate) fn unread_count(state: &GuiState) -> usize {
    let norm = crate::gui::pages::chat::norm_server_url;
    let here = norm(&state.connected_server_url);
    let mine = state.chat_channels.iter().filter(|c| c.unread).count() + state.chat_dms.iter().filter(|d| d.unread).count();
    let others: usize = state
        .connections
        .iter()
        .filter(|c| norm(&c.url) != here)
        .map(|c| c.channels.iter().filter(|ch| ch.unread).count() + c.dms.iter().filter(|d| d.unread).count())
        .sum();
    mine + others
}

/// Draw the footer, after the warning (called by `bottom_strip::draw_at`); returns its height, 0
/// when it did not draw.
pub(crate) fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> f32 {
    if !shows(state) {
        return 0.0;
    }
    if let Some(key) = crate::gui::pages::keymap::shortcut(&state.keymaps, KEY_ID) {
        if ctx.input_mut(|i| i.consume_shortcut(&key)) {
            fold_or_unfold(state);
        }
    }
    match state.chat_footer.mode {
        Mode::Off => 0.0,
        Mode::Folded => draw_folded(ctx, theme, state),
        Mode::Unfolded => draw_unfolded(ctx, theme, state),
    }
}

/// Fold an unfolded footer, unfold a folded one, and keep the choice.
fn fold_or_unfold(state: &mut GuiState) {
    state.chat_footer.mode = match state.chat_footer.mode {
        Mode::Folded => Mode::Unfolded,
        Mode::Unfolded => Mode::Folded,
        Mode::Off => Mode::Off,
    };
    save(state);
}

fn save(state: &GuiState) {
    crate::config::AppConfig::from_gui_state(state).save();
}

/// The folded line: one line on the sidebar's ground, the warning's text size and margins.
fn draw_folded(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> f32 {
    let bottom = ctx.available_rect().bottom();
    let panel = egui::TopBottomPanel::bottom(FOLDED_PANEL)
        .frame(egui::Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(egui::Margin::symmetric(8, 3)))
        .resizable(false)
        .show(ctx, |ui| line(ui, theme, state, false));
    taken(bottom, &panel.response)
}

/// The unfolded strip: the line, the last messages, the box. Resizable from its top edge; the
/// height is kept when the person lets go of the edge, not when a smaller window holds it lower.
fn draw_unfolded(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> f32 {
    // What the Chat page does each frame before its rows and composer (pages/chat.rs `draw`), for
    // the same rows and composer here: the warnings' patterns read once, a reply kept to the
    // conversation it was started in, a video's Volume settings followed, and what the Media cards
    // in the rows asked last frame done (their list would otherwise grow every frame).
    crate::engine::warnings::ensure_loaded(state);
    crate::gui::pages::chat::reply::drop_if_elsewhere(state);
    crate::gui::pages::chat::follow_volume_settings_link(state);
    crate::gui::pages::files::chat_card::apply(state);
    // A row's control whose window is the Chat page's (a person's card, the Report dialog) opens
    // the Chat page, where that window lives; Back returns here.
    let windows_before = (state.chat_user_modal_open, state.reports.dialog.is_some());
    let max = (ctx.screen_rect().height() * MAX_SHARE).max(MIN_HEIGHT);
    // Exactly the kept height (held under a smaller window's 40% without forgetting it): never
    // egui's own resizable panel, which takes the height its content drew as next frame's, so the
    // composer's bar, a pixel or two taller than its room, grew the strip a little every frame
    // (seen in the unfolded test, 2026-11-15: 180 points became 186 and kept climbing).
    let height = state.chat_footer.height.clamp(MIN_HEIGHT, max);
    let bottom = ctx.available_rect().bottom();
    let panel = egui::TopBottomPanel::bottom(OPEN_PANEL)
        .frame(egui::Frame::NONE.fill(theme.bg_primary()))
        .resizable(false)
        .exact_height(height)
        .show(ctx, |ui| {
            // The top edge: drag it up or down for a taller or shorter strip, kept when let go.
            let top = ui.max_rect().top();
            let edge = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), top - 3.0), egui::pos2(ui.max_rect().right(), top + 3.0));
            let grip = ui
                .interact(edge, egui::Id::new(OPEN_PANEL).with("edge"), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                .hint("chat_footer.resize_hint");
            if grip.dragged() {
                state.chat_footer.height = (height - grip.drag_delta().y).clamp(MIN_HEIGHT, max);
            }
            if grip.drag_stopped() {
                save(state);
            }
            if grip.hovered() || grip.dragged() {
                ui.painter().hline(edge.x_range(), top, egui::Stroke::new(2.0_f32, theme.accent()));
            }
            egui::Frame::NONE
                .fill(theme.bg_sidebar_dark())
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| line(ui, theme, state, true));
            let available = ui.available_rect_before_wrap();
            let (input_height, composer_cap) = composer::reserve(ui.ctx(), state, Place::Footer, available.height());
            let messages_rect = egui::Rect::from_min_size(available.min, egui::vec2(available.width(), (available.height() - input_height).max(0.0)));
            let channel = state.chat_active_channel.clone();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(messages_rect), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("chat_footer_messages")
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| rows::draw(ui, theme, state, &channel, Some(ROWS_SHOWN)));
            });
            let input_rect = egui::Rect::from_min_size(egui::pos2(available.min.x, available.max.y - input_height), egui::vec2(available.width(), input_height));
            composer::draw(ui, theme, state, Place::Footer, input_rect, composer_cap);
        });
    let windows_after = (state.chat_user_modal_open, state.reports.dialog.is_some());
    if (windows_after.0 && !windows_before.0) || (windows_after.1 && !windows_before.1) {
        state.go(GuiPage::Chat, crate::gui::How::From);
    }
    taken(bottom, &panel.response)
}

/// The room a panel took at the bottom: from where the room ended before it to the panel's top,
/// which is where egui ends the room for the page. Not the panel's own rect, which also takes in
/// whatever its content drew past its bottom edge (the composer's bar can, by a pixel or two), so
/// it read 182 for a strip of 180 (the unfolded test, 2026-11-15).
fn taken(bottom: f32, panel: &egui::Response) -> f32 {
    (bottom - panel.rect.top()).max(0.0)
}

/// The footer's line: the conversation's name and (folded) its newest message on the left; the
/// unread count, Open in Chat and Show more or Show less on the right.
fn line(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, unfolded: bool) {
    let size = theme.font_size_small;
    // One text row tall, never the height left under it: a right-to-left layout given the whole
    // rest of the strip centres its line in it, and the strip's messages and box went under the
    // window (seen in the unfolded test, 2026-11-15: a 180-point strip drew 358 tall).
    let row_h = ui.fonts(|f| f.row_height(&egui::FontId::proportional(size)));
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), row_h), Layout::right_to_left(Align::Center), |ui| {
        let (word, hint) = if unfolded { ("chat_footer.fold", "chat_footer.fold_hint") } else { ("chat_footer.unfold", "chat_footer.unfold_hint") };
        if link(ui, theme, t(word), size, hint).clicked() {
            fold_or_unfold(state);
        }
        ui.add_space(theme.spacing_md);
        if link(ui, theme, t("chat_footer.open_in_chat"), size, "chat_footer.open_in_chat_hint").clicked() {
            state.go(GuiPage::Chat, crate::gui::How::From);
        }
        let unread = unread_count(state);
        if unread > 0 {
            ui.add_space(theme.spacing_md);
            ui.label(RichText::new(tf("chat_footer.unread", &[("count", &unread.to_string())])).size(size).color(theme.accent()))
                .hint("chat_footer.unread_hint");
        }
        ui.add_space(theme.spacing_md);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let (icon, _) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
            crate::gui::widgets::icons::paint_chat(ui.painter(), icon, theme.text_muted());
            let name = crate::gui::pages::chat::video_dock::conversation_name(state, &state.chat_active_channel);
            ui.label(RichText::new(name).size(size).strong().color(theme.text_primary()));
            if !unfolded {
                ui.add_space(theme.spacing_sm);
                let words = match rows::newest(state, &state.chat_active_channel) {
                    Some(m) => tf("chat_footer.newest", &[("name", &m.sender_name), ("words", &rows::preview(state, m))]),
                    None => t("chat_footer.empty").to_string(),
                };
                ui.add(egui::Label::new(RichText::new(words).size(size).color(theme.text_muted())).truncate());
            }
        });
    });
}

/// A link-styled word on the line (the warning's What this means is the same): text height, so the
/// folded line stays one line tall.
fn link(ui: &mut egui::Ui, theme: &Theme, words: &str, size: f32, hint: &'static str) -> egui::Response {
    ui.add(egui::Label::new(RichText::new(words).size(size).color(theme.accent()).underline()).sense(Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .hint(hint)
}

/// The Settings row (Settings > Appearance): Off, Folded, Unfolded. True when it changed.
pub(crate) fn setting_row(ui: &mut egui::Ui, theme: &Theme, footer: &mut ChatFooter) -> bool {
    let mut changed = false;
    crate::gui::widgets::settings_row(ui, theme, t("chat_footer.setting"), |ui| {
        for mode in Mode::ALL {
            let active = footer.mode == mode;
            let (label, hint) = match mode {
                Mode::Off => (t("chat_footer.off"), "chat_footer.off_hint"),
                Mode::Folded => (t("chat_footer.folded"), "chat_footer.folded_hint"),
                Mode::Unfolded => (t("chat_footer.unfolded"), "chat_footer.unfolded_hint"),
            };
            if crate::gui::widgets::Button::secondary(label).hint(hint).active(active).show(ui, theme) && !active {
                footer.mode = mode;
                changed = true;
            }
        }
    });
    changed
}

/// The Settings row's description: what the footer is, and the key that folds it (read from the
/// keymap, so the words name the key that works).
pub(crate) fn setting_words(keymaps: &[crate::gui::pages::keymap::KeymapContext]) -> String {
    match crate::gui::pages::keymap::shortcut_words(keymaps, KEY_ID) {
        Some(key) => format!("{} {}", t("chat_footer.setting_hint"), tf("chat_footer.key_hint", &[("key", &key)])),
        None => t("chat_footer.setting_hint").to_string(),
    }
}

#[cfg(test)]
#[path = "chat_footer_tests.rs"]
mod tests;
