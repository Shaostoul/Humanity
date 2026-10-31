//! The header's notices (rung 2, 2026-10-31, docs/design/notifications.md section 3): the bell
//! with the count of notices not opened yet, and the panel of the last 100 it opens, each entry
//! opening its conversation.
//!
//! - **Where:** an anchored spot just left of the help button (`pages/keymap.rs`
//!   `draw_help_toggle`), so it never moves when the nav row wraps; the nav row reserves its width
//!   (`pages/escape_menu.rs`). On every page that has the header; not in the first-person world
//!   (the in-world banner is rung 2's second part) nor on the title screen.
//! - **What a row shows:** the kind, who and where, as a notice's title says it
//!   (`notifications::decide::words` with the person's "What a notice shows"), and when; the
//!   message itself only when that setting shows it (`notifications::list` keeps it only then).
//! - **Seen means the panel was opened:** opening marks every entry seen, and while it stays open
//!   a new one is seen as it arrives.
//! - **Nothing here is saved:** the list is the run's (`notifications::Runtime::list`).
//!
//! The website's twin is the bell in the chat page's header (web/chat/chat-notify.js).

use egui::{Context, RichText};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiPage, GuiState};
use crate::i18n::t;
use crate::notifications::{self as n, list::Entry};

/// The bell is the help button's size, beside it.
pub const BELL_SIZE: f32 = crate::gui::pages::keymap::HELP_TOGGLE_SIZE;
/// The gap between the bell and the help button.
pub const BELL_GAP: f32 = 6.0;
/// How wide the panel is.
const PANEL_WIDTH: f32 = 340.0;

/// Should the bell show: every page with the header, never the world view or the title screen.
fn shows(state: &GuiState) -> bool {
    !matches!(state.active_page, GuiPage::None | GuiPage::MainMenu)
}

/// The bell, and the panel while it is open. Called right after the nav bar.
pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    if !shows(state) {
        state.notify.panel_open = false;
        return;
    }
    let unseen = state.notify.list.unseen();
    let open = state.notify.panel_open;
    let x = -(8.0 + crate::gui::pages::keymap::HELP_TOGGLE_SIZE + BELL_GAP);
    let area = egui::Area::new(egui::Id::new("notice_bell_button"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(x, 5.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let label = widgets::notice_bell::label_for(unseen);
            widgets::notice_bell::notice_bell(ui, theme, BELL_SIZE, unseen, open, &label)
        });
    let anchor = area.response.rect;
    let mut just_opened = false;
    if area.inner.clicked() {
        state.notify.panel_open = !open;
        just_opened = !open;
    }
    if state.notify.panel_open {
        // Seen means the panel was opened; a notice arriving while it is open is seen at once.
        state.notify.list.mark_seen();
        if !draw_panel(ctx, theme, state, anchor, just_opened) {
            state.notify.panel_open = false;
        }
    }
}

/// The panel under the bell. False when it should close (Escape, a click outside it and the bell,
/// or an entry opened).
fn draw_panel(ctx: &Context, theme: &Theme, state: &mut GuiState, anchor: egui::Rect, just_opened: bool) -> bool {
    let mut keep_open = true;
    let mut picked: Option<Entry> = None;
    let mut clear = false;
    let show = state.settings.notifications.show;
    let file = n::data::get();
    let area = egui::Area::new(egui::Id::new("notice_panel"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::RIGHT_TOP)
        .fixed_pos(egui::pos2(anchor.right(), anchor.bottom() + 4.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(theme.bg_card()).stroke(egui::Stroke::new(1.0, theme.border())).show(ui, |ui| {
                ui.set_width(PANEL_WIDTH);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("notifications.panel_heading")).size(theme.font_size_body).color(theme.text_primary()).strong());
                    if !state.notify.list.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::Button::secondary(t("notifications.panel_clear")).hint("notifications.panel_clear_hint").show(ui, theme) {
                                clear = true;
                            }
                        });
                    }
                });
                ui.label(RichText::new(t("notifications.panel_note")).size(theme.font_size_small).color(theme.text_muted()));
                ui.add_space(theme.spacing_xs);
                if state.notify.list.is_empty() {
                    ui.label(RichText::new(t("notifications.panel_empty")).size(theme.font_size_small).color(theme.text_secondary()));
                    return;
                }
                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                    for (i, e) in state.notify.list.entries().enumerate() {
                        if entry_row(ui, theme, state, file, e, show, i) {
                            picked = Some(e.clone());
                        }
                    }
                });
            });
        });
    if clear {
        state.notify.list.clear();
    }
    if let Some(e) = picked {
        open_entry(state, &e);
        keep_open = false;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        keep_open = false;
    }
    // A click outside the panel and the bell closes it; the opening click is on the bell.
    if !just_opened && ctx.input(|i| i.pointer.any_click()) {
        let at = ctx.input(|i| i.pointer.interact_pos());
        if at.is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p)) {
            keep_open = false;
        }
    }
    keep_open
}

/// One entry: its title (the kind, who and where), the message when kept, and when it came and
/// through which server. True when clicked.
fn entry_row(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, file: &n::data::File, e: &Entry, show: n::Show, i: usize) -> bool {
    let c = e.candidate();
    let (title, _) = n::decide::words(file, &c, show);
    let when = crate::gui::pages::chat::format_timestamp((e.at.max(0) as u64) * 1000);
    let server = server_label(state, &e.server);
    let resp = ui
        .push_id(("notice_entry", i), |ui| {
            egui::Frame::none().inner_margin(egui::Margin::symmetric(6, 4)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&title).size(theme.font_size_body).color(theme.text_primary()));
                if show == n::Show::NameAndText {
                    if let Some(text) = e.text.as_deref() {
                        let body = n::decide::clean(text, 120);
                        ui.label(RichText::new(body).size(theme.font_size_small).color(theme.text_secondary()));
                    }
                }
                ui.label(RichText::new(format!("{when} \u{00b7} {server}")).size(theme.font_size_small).color(theme.text_muted()));
            })
        })
        .inner
        .response;
    let resp = ui.interact(resp.rect, egui::Id::new(("notice_entry_click", i)), egui::Sense::click()).hint("notifications.entry_hint");
    if resp.hovered() {
        ui.painter().rect_stroke(resp.rect, egui::Rounding::same(theme.border_radius as u8), egui::Stroke::new(1.0, theme.accent()), egui::StrokeKind::Inside);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// A server's name for an entry: its saved name, else its address.
fn server_label(state: &GuiState, url: &str) -> String {
    let norm = crate::gui::pages::chat::norm_server_url;
    state
        .chat_servers
        .iter()
        .find(|s| norm(&s.url) == url)
        .map(|s| s.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| url.to_string())
}

/// Open the conversation `e` came from: switch to its server when it is another one this device
/// keeps, then the DM, the room, or the shared room's Commons view, on the Chat page.
pub(crate) fn open_entry(state: &mut GuiState, e: &Entry) {
    let norm = crate::gui::pages::chat::norm_server_url;
    if norm(&state.server_url) != e.server {
        match state.chat_servers.iter().find(|s| norm(&s.url) == e.server).map(|s| (s.url.clone(), s.name.clone())) {
            Some((url, name)) => {
                state.switch_to_server(&url, &name);
            }
            None => {
                // A server no longer saved here: its conversation cannot be opened.
                log::info!("notices: {} is no longer a saved server", e.server);
                return;
            }
        }
    }
    if let Some(peer) = e.conversation.strip_prefix("dm:") {
        crate::gui::pages::chat::open_dm_conversation(state, peer);
    } else if let Some(room) = e.room.as_deref() {
        if state.chat_channels.iter().any(|c| c.id == room) {
            state.chat_active_channel = room.to_string();
            state.chat_reply_to = None;
            state.chat_messages.clear();
            state.history_fetched = false;
            if let Some(c) = state.chat_channels.iter_mut().find(|c| c.id == room) {
                c.unread = false;
            }
        } else {
            crate::gui::pages::chat::open_commons_room(state, room);
        }
    }
    if state.active_page != GuiPage::Chat {
        state.push_nav_to(GuiPage::Chat);
    }
}
