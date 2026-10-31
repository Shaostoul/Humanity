//! The engine's side of notifications (docs/design/notifications.md): a live message that might
//! deserve a notice is handed to THE rule (`crate::notifications::decide`) here, with what only the
//! engine knows (whether the window has focus, what is on screen), and whatever the rule says is
//! done: the sound through the app's own mixer, the operating system's notice through its worker.
//!
//! Who raises a candidate (each only for a LIVE message, never history or a mailbox page):
//! - a DM on the active server: `engine/mailbox.rs` `on_new`;
//! - a DM on a saved server signed in in the background: `engine/bg_connections.rs` `bg_dm_new`;
//! - a mention in a room, on the active server (`engine/frame_ws_poll.rs`, the `chat` arm) and on
//!   a background server (`bg_connections.rs`, its `chat` arm), through [`mention`].
//!
//! Nothing here runs per frame except [`pump`], two `Option::take`s for what Settings asked.

use crate::engine::state::EngineState;
use crate::gui::GuiState;
use crate::notifications::{self as n, Candidate};

/// The burst guard keeps one time per conversation; past this many, the old ones go.
const LAST_NOTICE_KEPT: usize = 256;

/// Ask the server for this person's synced switches. At every sign-in, so quiet hours are known
/// before the first live message (until they are, nothing notifies: `Why::NotLoaded`).
pub(crate) fn ask_prefs(gs: &GuiState) {
    if let Some(c) = gs.ws_client.as_ref().filter(|c| c.is_connected()) {
        c.send(&serde_json::json!({ "type": "get_notification_prefs" }).to_string());
    }
}

/// A mention candidate for a live room line, when `content` names this person (`@` and `my_name`,
/// the name they are signed in with on that server) and the room is one the protected setup lists.
/// `None` otherwise.
pub(crate) fn mention(gs: &GuiState, server: &str, my_name: &str, room: &str, sender_key: &str, sender: &str, content: &str) -> Option<Candidate> {
    if !n::is_mention(content, my_name) || !crate::engine::protected::lists_commons_room(gs, room) {
        return None;
    }
    let room_name = gs
        .chat_channels
        .iter()
        .chain(gs.connections.iter().flat_map(|c| c.channels.iter()))
        .find(|c| c.id == room)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| room.to_string());
    Some(Candidate {
        kind: "mention".into(),
        from_me: sender_key == gs.profile_public_key,
        server: server.to_string(),
        room: Some(room.to_string()),
        conversation: room.to_string(),
        sender: sender.to_string(),
        room_name,
        text: crate::gui::pages::chat::reply::preview_of(content),
    })
}

/// A DM candidate, for a DM newly taken in from `server`'s mailbox.
pub(crate) fn dm(gs: &GuiState, server: &str, inner: &crate::net::dm_pq::DmInner) -> Candidate {
    let from_me = inner.from == gs.profile_public_key;
    let peer = if from_me { &inner.to } else { &inner.from };
    Candidate {
        kind: "dm".into(),
        from_me,
        server: server.to_string(),
        room: None,
        conversation: format!("dm:{peer}"),
        sender: crate::engine::dm::dm_display_name(gs, &inner.from),
        room_name: String::new(),
        text: crate::gui::pages::chat::reply::preview_of(&inner.text),
    }
}

/// Is `c`'s conversation the one showing: the Chat page (or the in-world chat panel) open on it,
/// on the server it came through? A shared room's Commons view shows it from any server.
pub(crate) fn on_screen(gs: &GuiState, c: &Candidate) -> bool {
    let chat_showing = gs.active_page == crate::gui::GuiPage::Chat || gs.chat_input_active;
    if !chat_showing {
        return false;
    }
    let active = gs.chat_active_channel.as_str();
    let same_server = c.server == crate::gui::pages::chat::norm_server_url(&gs.server_url);
    (same_server && active == c.conversation)
        || c.room.as_deref().is_some_and(|r| crate::gui::pages::chat::commons_room_of(active) == Some(r))
}

/// Hand `c` to the rule, and do what it says.
pub(crate) fn deliver(state: &mut EngineState, c: Candidate) {
    let now = n::now_secs();
    let file = n::data::get();
    let gs = &state.gui_state;
    let synced = n::Synced {
        dm: gs.notif_dm_enabled,
        mentions: gs.notif_mentions_enabled,
        tasks: gs.notif_tasks_enabled,
        quiet_start: gs.notif_dnd_start.as_deref(),
        quiet_end: gs.notif_dnd_end.as_deref(),
        loaded: gs.notif_prefs_loaded,
    };
    let at = n::Moment {
        now,
        minute: n::quiet::local_minute(now),
        focused: state.window_focused,
        on_screen: on_screen(gs, &c),
        last_notice: gs.notify.last_notice.get(&c.conversation).copied(),
    };
    let local = &gs.settings.notifications;
    let out = n::decide(file, &c, &synced, local, &at);
    log::debug!("notifications: a {} -> {}", c.kind, out.why.code());
    if out.why != n::Why::Notify {
        return;
    }
    let show = local.show;
    let sound_id = local.sound_id.clone();
    let last = &mut state.gui_state.notify.last_notice;
    if last.len() >= LAST_NOTICE_KEPT {
        last.retain(|_, t| now - *t < n::decide::BURST_SECS);
    }
    last.insert(c.conversation.clone(), now);
    if out.sound {
        play(state, &sound_id);
    }
    if out.toast {
        let (title, body) = n::decide::words(file, &c, show);
        n::toast::show(n::toast::Toast { title, body, tag: n::toast::tag_for(&c.conversation), group: c.kind });
    }
}

/// Queue the sound `id` from the data file (its catalog entry, or the file's path for it).
fn play(state: &mut EngineState, id: &str) {
    if let Some(s) = n::data::get().sound(id) {
        state.pending_sfx.push((s.id.as_str(), s.path.as_str()));
    }
}

/// Per frame: what Settings > Notifications asked for. A test notice goes straight to the
/// operating system with made-up words (whatever the rule would say, so a person can see one),
/// with the sound if sound is on; Play plays the chosen sound once.
pub(crate) fn pump(state: &mut EngineState) {
    if std::mem::take(&mut state.gui_state.notify.test_requested) {
        let t = crate::i18n::t;
        n::toast::show(n::toast::Toast {
            title: t("notifications.test_title").to_string(),
            body: t("notifications.test_body").to_string(),
            tag: n::toast::tag_for("test"),
            group: "test".into(),
        });
        let s = &state.gui_state.settings.notifications;
        if s.sound {
            let id = s.sound_id.clone();
            play(state, &id);
        }
    }
    if let Some(id) = state.gui_state.notify.preview_sound.take() {
        play(state, &id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gs() -> GuiState {
        let mut gs = GuiState::default();
        gs.server_url = "https://a.example".into();
        gs.user_name = "Ann".into();
        gs.profile_public_key = "me".into();
        gs
    }

    /// On screen means the Chat page (or the in-world panel) open on that conversation, on its
    /// server. Seen red 2026-10-30 with the page check left out: a DM was "on screen" while the
    /// person played in the world with Chat closed, so it never notified.
    #[test]
    fn on_screen_is_the_open_conversation_on_its_own_server() {
        let mut g = gs();
        let c = Candidate { kind: "dm".into(), server: "https://a.example".into(), conversation: "dm:k1".into(), ..Default::default() };
        g.chat_active_channel = "dm:k1".into();
        g.active_page = crate::gui::GuiPage::None;
        assert!(!on_screen(&g, &c), "Chat closed: not on screen");
        g.chat_input_active = true;
        assert!(on_screen(&g, &c), "the in-world chat panel shows it");
        g.chat_input_active = false;
        g.active_page = crate::gui::GuiPage::Chat;
        assert!(on_screen(&g, &c));
        let other = Candidate { server: "https://b.example".into(), ..c.clone() };
        assert!(!on_screen(&g, &other), "the same peer's DM from another server is another store");
        let room = Candidate { kind: "mention".into(), server: "https://b.example".into(), room: Some("general".into()), conversation: "general".into(), ..Default::default() };
        g.chat_active_channel = "commons:general".into();
        assert!(on_screen(&g, &room), "the Commons view shows a shared room from any server");
    }

    /// A mention is `@name`, never a word that merely contains it, and a line of mine is mine.
    /// Seen red 2026-10-30 with `is_mention`'s boundary check removed: "@Annabel hi" was Ann's.
    #[test]
    fn a_mention_candidate_needs_the_at_and_the_whole_name() {
        let g = gs();
        let s = "https://a.example";
        assert!(mention(&g, s, "Ann", "general", "k2", "Ben", "hi Ann").is_none(), "no @");
        assert!(mention(&g, s, "Ann", "general", "k2", "Ben", "@Annabel hi").is_none());
        let m = mention(&g, s, "Ann", "general", "k2", "Ben", "hi @ann").expect("a mention");
        assert!(!m.from_me);
        assert_eq!((m.kind.as_str(), m.room.as_deref(), m.room_name.as_str()), ("mention", Some("general"), "general"));
        assert!(mention(&g, s, "Ann", "general", "me", "Ann", "@ann note to self").is_some_and(|m| m.from_me));
        assert!(mention(&g, s, "Annie", "general", "k2", "Ben", "hi @ann").is_none(), "the name signed in with there");
    }
}
