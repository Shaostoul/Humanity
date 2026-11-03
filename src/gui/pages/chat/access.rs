//! Chat access levels in the desktop app's chat (2026-11-02, docs/design/chat-access.md).
//!
//! - **The line just above the box you type in**, for a server channel: who can read it ("Anyone
//!   can read this, even without signing up." and so on), then why you cannot post there, or, for a
//!   member not yet verified, the one-link rule with a glossary card saying what Verified means.
//! - **The pickers** an admin sets a channel's levels with: Server Settings > Channels and the
//!   channel's own editor (chat/modals.rs) both draw these two.
//! - **Whether a poster's links show as links.** A line from someone below Verified on this server
//!   has its links and its picture and file addresses drawn as plain words: no picture pulled in,
//!   no card, nothing to click (the website draws them the same way, web/chat/app.js `formatBody`).
//!
//! The levels and the rules are `humanity_protocol::chat_access`, the relay's own, and the relay
//! enforces them (src/relay/chat_access.rs): what is here only says them and draws by them.

use egui::RichText;

use humanity_core::i18n::{t, t_marked};
use humanity_protocol::chat_access::{self as access, PostLevel, Rank, ReadLevel};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{ChatMessage, GuiState};

/// The rank of `key` on the active server, from the member list's role and the roles list (a role
/// the admins made counts by its trust level). Someone the list does not name (a federated poster,
/// the server's own announcements) reads as a Member, so their links are plain words.
pub(crate) fn rank_of(state: &GuiState, key: &str) -> Rank {
    if key.starts_with("bot_") {
        return Rank::Admin; // the server's own bots, signed in with its bot secret
    }
    let Some(user) = state.chat_users.iter().find(|u| u.public_key == key) else { return Rank::Member };
    let trust = state.chat_roles.iter().find(|r| r.id == user.role).map_or(0, |r| r.trust_level);
    access::rank_for_member(&user.role, trust)
}

/// Whether `msg`'s links and picture and file addresses are drawn as links, pictures and files:
/// its poster is Verified or above on this server. In a direct message, a group or the scratchpad
/// (not channels) they are, as before.
pub(crate) fn shows_links(state: &GuiState, msg: &ChatMessage) -> bool {
    super::is_private_channel(&state.chat_active_channel) || access::shows_links(rank_of(state, &msg.sender_key))
}

/// The channel the box you type in posts to, when it is one of the active server's channels.
fn active_channel(state: &GuiState) -> Option<&crate::gui::ChatChannel> {
    let active = state.chat_active_channel.as_str();
    if super::is_private_channel(active) {
        return None;
    }
    let room = super::commons_room_of(active).unwrap_or(active);
    state.chat_channels.iter().find(|c| c.id == room)
}

/// The words of the line above the box for a channel with these levels, for someone of `me`: who
/// can read it, then (marked for the glossary) why `me` cannot post there, or the one-link rule
/// below Verified.
pub(crate) fn audience_words(read: ReadLevel, post: PostLevel, me: Rank) -> (&'static str, Option<&'static str>) {
    let who_reads = match read {
        ReadLevel::Anyone => t("chat.access_read_anyone"),
        ReadLevel::Members => t("chat.access_read_members"),
        ReadLevel::Verified => t("chat.access_read_verified"),
    };
    let more = if !access::may_post(read, post, me) {
        Some(match access::normalized_post(read, post) {
            PostLevel::Mods => t_marked("chat.access_post_mods"),
            PostLevel::Verified => t_marked("chat.access_post_verified"),
            PostLevel::Members => t_marked("chat.access_post_members"),
        })
    } else if me < Rank::Verified {
        Some(t_marked("chat.access_one_link"))
    } else {
        None
    };
    (who_reads, more)
}

/// Paint a channel's read-level badge in `rect` (docs/design/chat-access.md 6.1, the operator:
/// "Anyone, members, verified, mods/admins"): a globe for Anyone, a person for Members, a tick for
/// Verified. Painted shapes, so no glyph the font draws as a box; a channel where only moderators
/// and admins post also carries the sidebar's eye. The website draws the same three icons.
pub(crate) fn paint_level_badge(painter: &egui::Painter, rect: egui::Rect, read: ReadLevel, color: egui::Color32) {
    match read {
        ReadLevel::Anyone => widgets::icons::paint_globe(painter, rect, color),
        ReadLevel::Members => widgets::icons::paint_person(painter, rect, color),
        ReadLevel::Verified => widgets::icons::paint_check(painter, rect, color),
    }
}

/// A badge's hint: who can read the channel and who can post in it.
pub(crate) fn badge_hint(read: ReadLevel, post: PostLevel) -> String {
    let post = access::normalized_post(read, post);
    humanity_core::i18n::tf("chat.badge_hint", &[("read", &read_word(read)), ("post", &post_word(post))])
}

/// Show the badge's hint under the pointer (the sidebar's rows paint their badge, so it has no
/// response of its own to hang a hint on).
pub(crate) fn badge_tooltip(ui: &egui::Ui, channel: &str, read: ReadLevel, post: PostLevel) {
    let hint = badge_hint(read, post);
    egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(("channel_level_badge", channel)), |ui| {
        ui.label(hint);
    });
}

/// The id of the badge on the line above the box.
pub(crate) const COMPOSER_BADGE_ID: &str = "chat_composer_level_badge";

/// The badge as a widget of its own, its hint under the pointer (the line above the box).
fn level_badge(ui: &mut egui::Ui, theme: &Theme, read: ReadLevel, post: PostLevel) -> egui::Response {
    let size = (theme.font_size_small * 1.1).min(14.0);
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
    // A known id, so the badge can be found and rested on (chat/access_tests.rs).
    let resp = ui.interact(rect, egui::Id::new(COMPOSER_BADGE_ID), egui::Sense::hover());
    paint_level_badge(ui.painter(), rect, read, theme.text_muted());
    // At once under the pointer, as the glossary cards beside it open (no hover delay).
    if resp.hovered() {
        badge_tooltip(ui, "composer_line", read, post);
    }
    resp
}

/// The line above the box you type in: who can read this channel, and what you may post here.
/// Nothing for a direct message, a group or the scratchpad.
pub(crate) fn draw_audience_line(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    let Some(ch) = active_channel(state) else { return };
    let (who_reads, more) = audience_words(ch.read_level, ch.post_level, rank_of(state, &state.profile_public_key));
    ui.horizontal_wrapped(|ui| {
        ui.add_space(12.0);
        ui.spacing_mut().item_spacing.x = theme.spacing_xs;
        level_badge(ui, theme, ch.read_level, ch.post_level);
        ui.label(RichText::new(who_reads).size(theme.font_size_small).color(theme.text_muted()));
        if let Some(more) = more {
            widgets::glossary_words(ui, theme, more, theme.font_size_small, theme.text_muted());
        }
    });
}

/// A read level's name, as the pickers show it.
pub(crate) fn read_word(level: ReadLevel) -> &'static str {
    match level {
        ReadLevel::Anyone => t("chat.level_anyone"),
        ReadLevel::Members => t("chat.level_members"),
        ReadLevel::Verified => t("chat.level_verified"),
    }
}

/// A post level's name, as the pickers show it.
pub(crate) fn post_word(level: PostLevel) -> &'static str {
    match level {
        PostLevel::Members => t("chat.level_members"),
        PostLevel::Verified => t("chat.level_verified"),
        PostLevel::Mods => t("chat.level_mods"),
    }
}

fn read_hint(level: ReadLevel) -> &'static str {
    match level {
        ReadLevel::Anyone => t("chat.level_anyone_hint"),
        ReadLevel::Members => t("chat.level_members_hint"),
        ReadLevel::Verified => t("chat.level_verified_hint"),
    }
}

fn post_hint(level: PostLevel) -> &'static str {
    match level {
        PostLevel::Members => t("chat.level_members_hint"),
        PostLevel::Verified => t("chat.level_verified_hint"),
        PostLevel::Mods => t("chat.level_mods_hint"),
    }
}

/// The "Who can read" picker, `width` wide, for the channel `salt` names. True when it changed.
pub(crate) fn read_picker(ui: &mut egui::Ui, salt: &str, width: f32, level: &mut ReadLevel) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(("channel_read_level", salt))
        .width(width)
        .selected_text(read_word(*level))
        .show_ui(ui, |ui| {
            for l in ReadLevel::ALL {
                changed |= ui.selectable_value(level, l, read_word(l)).on_hover_text(read_hint(l)).changed();
            }
        })
        .response
        .on_hover_text(t("chat.level_read_hint"));
    changed
}

/// The "Who can post" picker, `width` wide, for the channel `salt` names. True when it changed.
pub(crate) fn post_picker(ui: &mut egui::Ui, salt: &str, width: f32, level: &mut PostLevel) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(("channel_post_level", salt))
        .width(width)
        .selected_text(post_word(*level))
        .show_ui(ui, |ui| {
            for l in PostLevel::ALL {
                changed |= ui.selectable_value(level, l, post_word(l)).on_hover_text(post_hint(l)).changed();
            }
        })
        .response
        .on_hover_text(t("chat.level_post_hint"));
    changed
}

#[cfg(test)]
#[path = "access_tests.rs"]
mod tests;
