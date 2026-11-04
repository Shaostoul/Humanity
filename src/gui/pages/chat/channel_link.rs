//! Links to chat channels on the desktop app (2026-11-03). The operator: "Can we make it so I can
//! link people to specific channels like https://united-humanity.us/chat#global or
//! https://united-humanity.us/chat#fire-disciples"
//!
//! A channel's address is `https://<its server>/chat#<its id>`, the website's own address for it
//! (web/chat/app.js opens it, and its address bar shows it). Here:
//!
//! - THE RULES, the twin of `web/shared/chat-links.js`, held to the same answers by
//!   `scripts/tests/fixtures/chat-links-v1.json` (the tests below): which word a hash names, which
//!   channel a word names (its id, else its shown name, either in any case: an admin can rename
//!   what a channel shows), a channel's address, what a link asks for, and whether a server's host
//!   reaches anyone off this computer or its home network.
//! - COPY LINK, on a room's right-click (every server's, chat/left_panel.rs) and at the end of the
//!   channel's header (chat.rs): the address of the room on ITS OWN server, never the official
//!   one for a room elsewhere (the "Copy Channel Link" it replaces always built a
//!   united-humanity.us path, `/chat/<name>`, which opens nothing). A server with no public
//!   address (localhost, a home network) is copied all the same, and the notice says the link
//!   opens only on this computer or its home network.
//! - A LINK THE APP WOULD OPEN IN THE BROWSER (egui's OpenUrl, from a chat message's link or
//!   anywhere else in the app's own window) that names a channel on the server the app is on or on
//!   a SAVED server opens that channel here instead (`open_in_app`, at the end of every frame in
//!   lib.rs). A saved server that is not the one open is switched to first, as its room row does.
//!   A link to any other server's chat opens the browser as before: the app does not dial a server
//!   the person never saved because a message pointed at it. `#dm=<key>` opens that conversation,
//!   as on the website. With no identity unlocked the browser opens it as before (the app cannot
//!   show a chat it is not signed in to).
//! - A link to a server whose channel list has not come yet (a saved server dialled just now)
//!   waits for it (`GuiState::chat_link_pending`, read by `open_pending` when the list arrives,
//!   engine/frame_ws_poll.rs); a word naming no channel there says so, and nothing else opens.

use humanity_core::i18n::{t, tf};

use crate::gui::widgets::Hint as _;
use crate::gui::{GuiPage, GuiState, ToastKind};

/// The longest word a link may name: a channel's id is at most 24 characters, its shown name a
/// little more. Anything longer is no channel.
pub(crate) const MAX_WORD: usize = 64;

/// Percent-decode `s` as JavaScript's decodeURIComponent does: None when a `%` is not followed by
/// two hex digits or the bytes are not UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The channel word an address's hash names (`#fire-disciples` gives `fire-disciples`), or None
/// when it names none: no hash, an empty one, `#profile` and `#joined` (the website reads those as
/// something else), or anything holding `=` (`#dm=<key>`, a device link). Decoded and trimmed.
pub(crate) fn word_of(hash: &str) -> Option<String> {
    let h = hash.strip_prefix('#').unwrap_or(hash);
    if h.is_empty() || h.contains('=') {
        return None;
    }
    let w = percent_decode(h)?;
    let w = w.trim();
    let lower = w.to_lowercase();
    if w.is_empty() || w.chars().count() > MAX_WORD || lower == "profile" || lower == "joined" {
        return None;
    }
    Some(w.to_string())
}

/// The id of the channel in `channels` (each `(id, shown name)`) that `word` names: the one whose
/// id it is, else the one whose shown name it is, each ignoring case.
pub(crate) fn find_id<'a>(word: &str, channels: impl IntoIterator<Item = (&'a str, &'a str)> + Clone) -> Option<&'a str> {
    let w = word.trim().to_lowercase();
    if w.is_empty() {
        return None;
    }
    channels
        .clone()
        .into_iter()
        .find(|(id, _)| id.to_lowercase() == w)
        .or_else(|| channels.into_iter().find(|(_, name)| name.trim().to_lowercase() == w))
        .map(|(id, _)| id)
}

/// The hash that shows channel `id`: `#` and the id, written as JavaScript's encodeURIComponent
/// writes it (letters, digits and `-_.!~*'()` as they are, every other byte as `%XX`).
pub(crate) fn hash_of(id: &str) -> String {
    let mut out = String::from("#");
    for b in id.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Channel `id`'s address on the server at `origin` (`https://united-humanity.us`).
pub(crate) fn link(origin: &str, id: &str) -> String {
    format!("{}/chat{}", origin.trim_end_matches('/'), hash_of(id))
}

/// Whether `host` (a host name or address, a port allowed) can be reached by someone on another
/// network: not this computer (localhost, 127.x, ::1) and not a home or office network (10.x,
/// 172.16 to 31.x, 192.168.x, 169.254.x, 100.64 to 127.x, fc00::/7, fe80::/10, a name ending
/// .local, .lan, .internal, .home.arpa, .localhost or .test, or a bare machine name).
pub(crate) fn is_public_host(host: &str) -> bool {
    let mut h = host.trim().to_lowercase();
    if let Some(rest) = h.strip_prefix('[') {
        h = rest.split(']').next().unwrap_or("").to_string();
    } else if h.matches(':').count() == 1 {
        h = h.split(':').next().unwrap_or("").to_string();
    }
    let h = h.strip_suffix('.').unwrap_or(&h);
    if h.is_empty() || matches!(h, "localhost" | "0.0.0.0" | "::" | "::1") {
        return false;
    }
    if [".localhost", ".local", ".lan", ".internal", ".home.arpa", ".test"].iter().any(|end| h.ends_with(end)) {
        return false;
    }
    if let Ok(v4) = h.parse::<std::net::Ipv4Addr>() {
        let [a, b, _, _] = v4.octets();
        return !(a == 0
            || a == 10
            || a == 127
            || (a == 169 && b == 254)
            || (a == 172 && (16..=31).contains(&b))
            || (a == 192 && b == 168)
            || (a == 100 && (64..=127).contains(&b)));
    }
    if h.contains(':') {
        return !(h.starts_with("fc") || h.starts_with("fd") || ["fe8", "fe9", "fea", "feb"].iter().any(|p| h.starts_with(p)));
    }
    h.contains('.')
}

/// What a link to a server's chat asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChatLink {
    /// `/chat#<channel>` (or the website's read view, `/chat/read#<channel>`).
    Channel { origin: String, word: String },
    /// `/chat#dm=<public key>`: a conversation.
    Dm { origin: String, key: String },
}

impl ChatLink {
    pub(crate) fn origin(&self) -> &str {
        match self {
            ChatLink::Channel { origin, .. } | ChatLink::Dm { origin, .. } => origin,
        }
    }
}

/// What the address `url` asks for, or None for any other address (another page, no hash, a hash
/// naming no channel, a scheme other than http and https). The origin is the scheme and host (and
/// a port that is not the scheme's own), lower case: `https://united-humanity.us`.
pub(crate) fn parse(url: &str) -> Option<ChatLink> {
    let u = url::Url::parse(url.trim()).ok()?;
    if u.scheme() != "https" && u.scheme() != "http" {
        return None;
    }
    let path = u.path().trim_end_matches('/');
    if !matches!(path, "/chat" | "/chat/index.html" | "/chat/read") {
        return None;
    }
    let origin = u.origin().ascii_serialization();
    let fragment = u.fragment().unwrap_or("");
    if let Some(key) = fragment.strip_prefix("dm=") {
        let hex = key.len() >= 16 && key.len() <= 8192 && key.bytes().all(|b| b.is_ascii_hexdigit());
        return hex.then(|| ChatLink::Dm { origin, key: key.to_string() });
    }
    word_of(fragment).map(|word| ChatLink::Channel { origin, word })
}

/// The origin of a server the app keeps (`https://united-humanity.us`, `http://127.0.0.1:3210`):
/// its address as the app dials it, a `ws`/`wss` one as `http`/`https`, an address with no scheme
/// as `https`, and any path dropped. None for an empty or unreadable address.
pub(crate) fn server_origin(server_url: &str) -> Option<String> {
    let s = server_url.trim();
    if s.is_empty() {
        return None;
    }
    let s = if let Some(rest) = s.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = s.strip_prefix("ws://") {
        format!("http://{rest}")
    } else if s.contains("://") {
        s.to_string()
    } else {
        format!("https://{s}")
    };
    let u = url::Url::parse(&s).ok()?;
    if u.scheme() != "https" && u.scheme() != "http" {
        return None;
    }
    Some(u.origin().ascii_serialization())
}

/// Channel `id`'s address on the server at `server_url`, and whether that server is public.
pub(crate) fn channel_address(server_url: &str, id: &str) -> Option<(String, bool)> {
    let origin = server_origin(server_url)?;
    let public = url::Url::parse(&origin).ok().and_then(|u| u.host_str().map(is_public_host)).unwrap_or(false);
    Some((link(&origin, id), public))
}

/// Copy channel `id`'s address on the server at `server_url` and say what was copied.
pub(crate) fn copy_channel_link(ctx: &egui::Context, state: &mut GuiState, server_url: &str, id: &str) {
    let Some((address, public)) = channel_address(server_url, id) else { return };
    ctx.copy_text(address.clone());
    let now = ctx.input(|i| i.time);
    if public {
        state.toast(tf("chat.copy_link_done", &[("link", &address)]), ToastKind::Success, now);
    } else {
        // Words to read, not a flash: the link will not open for someone elsewhere.
        state.notice(tf("chat.copy_link_done_private", &[("link", &address)]), now);
    }
}

/// A room's right-click item, Copy link: the room's address on `server_url`, its own server.
pub(crate) fn copy_link_item(ui: &mut egui::Ui, state: &mut GuiState, server_url: &str, id: &str) {
    if ui.button(t("chat.copy_link")).hint("chat.copy_link_hint").clicked() {
        copy_channel_link(ui.ctx(), state, server_url, id);
        ui.close_menu();
    }
}

/// Every link the app's window would open in the browser this frame that is a chat link to the
/// server the app is on or a saved one: taken out of the frame's output and opened here instead.
/// Called last in the main window's frame (lib.rs), so it sees every link of the frame.
pub(crate) fn open_in_app(ctx: &egui::Context, state: &mut GuiState) {
    let urls: Vec<String> = ctx.output(|o| {
        o.commands
            .iter()
            .filter_map(|c| match c {
                egui::OutputCommand::OpenUrl(u) => Some(u.url.clone()),
                _ => None,
            })
            .collect()
    });
    if urls.is_empty() {
        return;
    }
    let now = ctx.input(|i| i.time);
    let taken: Vec<String> = urls.into_iter().filter(|u| open_link(state, u, now)).collect();
    if !taken.is_empty() {
        ctx.output_mut(|o| o.commands.retain(|c| !matches!(c, egui::OutputCommand::OpenUrl(u) if taken.contains(&u.url))));
    }
}

/// Open the chat link `url` in the app when it is one to the server open here or a saved one;
/// false (the browser opens it) for anything else, or with no identity unlocked.
pub(crate) fn open_link(state: &mut GuiState, url: &str, now: f64) -> bool {
    let Some(link) = parse(url) else { return false };
    if state.private_key_bytes.is_none() {
        return false;
    }
    let here = server_origin(&state.active_server()).is_some_and(|o| o == link.origin());
    if !here {
        let saved = state
            .chat_servers
            .iter()
            .find(|s| server_origin(&s.url).is_some_and(|o| o == link.origin()))
            .map(|s| (s.url.clone(), s.name.clone()));
        let Some((server_url, name)) = saved else { return false };
        state.switch_to_server(&server_url, &name);
    }
    match link {
        ChatLink::Channel { word, .. } => {
            state.chat_link_pending = Some(word);
            open_pending(state, now);
        }
        ChatLink::Dm { key, .. } => {
            if key != state.profile_public_key {
                super::open_dm_conversation(state, &key);
            }
        }
    }
    // On a page, the Chat page shows it; in the world the channel changes under the player's feet
    // (the world's own chat follows it) and the app does not take them out of the world.
    if !matches!(state.active_page, GuiPage::Chat | GuiPage::None | GuiPage::MainMenu) {
        state.go(GuiPage::Chat, crate::gui::How::From);
    }
    true
}

/// The channel a link asked for (`chat_link_pending`), once this server's channel list is here:
/// open it, or say there is no such channel here and stay where the app is. Called where a link
/// is followed and when a channel list arrives (engine/frame_ws_poll.rs), before the room-gone
/// rule, so the room the link names is the room that rule finds.
pub(crate) fn open_pending(state: &mut GuiState, now: f64) {
    if state.chat_channels.is_empty() {
        return;
    }
    let Some(word) = state.chat_link_pending.take() else { return };
    let found = find_id(&word, state.chat_channels.iter().map(|c| (c.id.as_str(), c.name.as_str()))).map(str::to_string);
    let Some(id) = found else {
        state.notice(tf("chat.link_no_channel", &[("name", &word)]), now);
        return;
    };
    if state.chat_active_channel != id {
        state.chat_active_channel = id.clone();
        state.chat_reply_to = None;
        state.chat_messages.clear();
        state.history_fetched = false;
    }
    if let Some(c) = state.chat_channels.iter_mut().find(|c| c.id == id) {
        c.unread = false;
    }
}

#[cfg(test)]
#[path = "channel_link_tests.rs"]
mod tests;
