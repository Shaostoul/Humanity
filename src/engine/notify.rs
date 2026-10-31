//! The engine's side of notifications (docs/design/notifications.md): a live message that might
//! deserve a notice is handed to THE rule (`crate::notifications::decide`) here, with what only the
//! engine knows (whether the window has focus, what is on screen), and whatever the rule says is
//! done: the sound through the app's own mixer, the operating system's notice through its worker.
//!
//! Who raises a candidate (each only for a LIVE message, never history or a mailbox page):
//! - a DM on the active server: `engine/mailbox.rs` `on_new`;
//! - a DM on a saved server signed in in the background: `engine/bg_connections.rs` `bg_dm_new`;
//! - a contact request, on either (rung 2): the same two, when its sender is newly listed under
//!   Requests, through [`contact_request`];
//! - a line in a room, on the active server (`engine/frame_ws_poll.rs`, the `chat` arm) and on a
//!   background server (`bg_connections.rs`, its `chat` arm), through [`room_line`]: a mention, a
//!   reply to a line of mine (rung 2), or an ordinary line, which the rule lets through only in a
//!   room set to every line;
//! - a shared room's line from a linked server (`federated_chat`, rung 2), on either, through
//!   [`federated_line`].
//!
//! Whatever the rule lets through also goes in the in-app list (`notifications::list`, the
//! header's bell). Nothing here runs per frame except [`pump`], two `Option::take`s for what
//! Settings asked.

use crate::engine::state::EngineState;
use crate::gui::GuiState;
use crate::notifications::{self as n, Candidate};

/// The burst guard keeps one time per conversation; past this many, the old ones go.
const LAST_NOTICE_KEPT: usize = 256;

/// A shared room's line older than this is not offered to the rule: when a link between servers
/// catches up it re-sends its numbered lines live (relay `handlers/federation_log.rs`), and those
/// are history in all but the frame they ride.
pub(crate) const FEDERATED_LIVE_MS: u64 = 5 * 60 * 1000;

/// Is `key` a friend of mine on `server` (we follow each other there; the client-held store's
/// `DmStore::is_friend`, mirrored on GuiState for the active server and on each parked link)?
fn is_friend(gs: &GuiState, server: &str, key: &str) -> bool {
    let norm = crate::gui::pages::chat::norm_server_url;
    if norm(&gs.server_url) == server {
        return gs.chat_following_keys.contains(key) && gs.chat_followers.contains(key);
    }
    gs.connections.iter().find(|c| norm(&c.url) == server).is_some_and(|c| c.following_keys.contains(key) && c.followers.contains(key))
}

/// Ask the server for this person's synced switches. At every sign-in, so quiet hours are known
/// before the first live message (until they are, nothing notifies: `Why::NotLoaded`).
pub(crate) fn ask_prefs(gs: &GuiState) {
    if let Some(c) = gs.ws_client.as_ref().filter(|c| c.is_connected()) {
        c.send(&serde_json::json!({ "type": "get_notification_prefs" }).to_string());
    }
}

/// A candidate for a live line in a room on `server`, whose words are `content`: a `mention` when
/// it names this person (`@` and `my_name`, the name they are signed in with on that server), else
/// a `reply` when it answers a line of theirs (`reply_to_from`, the replied-to line's author, is
/// their key), else a `room_line`, which the rule lets through only in a room set to every line.
/// `None` when the room is one the protected setup hides.
#[allow(clippy::too_many_arguments)]
pub(crate) fn room_line(
    gs: &GuiState,
    server: &str,
    my_name: &str,
    room: &str,
    sender_key: &str,
    sender: &str,
    content: &str,
    reply_to_from: Option<&str>,
) -> Option<Candidate> {
    if !crate::engine::protected::lists_commons_room(gs, room) {
        return None;
    }
    let me = gs.profile_public_key.as_str();
    let kind = if n::is_mention(content, my_name) {
        "mention"
    } else if !me.is_empty() && reply_to_from == Some(me) {
        "reply"
    } else {
        "room_line"
    };
    let room_name = gs
        .chat_channels
        .iter()
        .chain(gs.connections.iter().flat_map(|c| c.channels.iter()))
        .find(|c| c.id == room)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| room.to_string());
    let server = crate::gui::pages::chat::norm_server_url(server);
    Some(Candidate {
        kind: kind.into(),
        from_me: sender_key == me,
        from_friend: is_friend(gs, &server, sender_key),
        server,
        room: Some(room.to_string()),
        conversation: room.to_string(),
        sender: sender.to_string(),
        room_name,
        text: crate::gui::pages::chat::reply::preview_of(content),
    })
}

/// A candidate for a live `federated_chat` frame on `server`: a shared room's line said on a
/// linked server, read as [`room_line`] reads a local one (it carries no reply). `None` for a line
/// with no words, one from someone I blocked (`from_key`, the author's key the relay carries since
/// 2026-10-22), or one older than [`FEDERATED_LIVE_MS`] (a link catching up).
pub(crate) fn federated_line(gs: &GuiState, server: &str, my_name: &str, frame: &serde_json::Value, now_ms: u64) -> Option<Candidate> {
    let s = |k: &str| frame.get(k).and_then(|v| v.as_str()).unwrap_or("");
    let ts = frame.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
    let (content, room, from_key) = (s("content"), s("channel"), s("from_key"));
    if content.is_empty() || room.is_empty() || ts == 0 || now_ms.saturating_sub(ts) > FEDERATED_LIVE_MS {
        return None;
    }
    if crate::engine::block::is_blocked(gs, from_key) {
        return None;
    }
    let server_name = if s("server_name").is_empty() { "federated" } else { s("server_name") };
    let sender = format!("{} ({server_name})", s("from_name"));
    room_line(gs, server, my_name, room, from_key, &sender, content, None)
}

/// A DM candidate, for a DM newly taken in from `server`'s mailbox.
pub(crate) fn dm(gs: &GuiState, server: &str, inner: &crate::net::dm_pq::DmInner) -> Candidate {
    let from_me = inner.from == gs.profile_public_key;
    let peer = if from_me { &inner.to } else { &inner.from };
    let server = crate::gui::pages::chat::norm_server_url(server);
    Candidate {
        kind: "dm".into(),
        from_me,
        from_friend: !from_me && is_friend(gs, &server, &inner.from),
        server,
        room: None,
        conversation: format!("dm:{peer}"),
        sender: crate::engine::dm::dm_display_name(gs, &inner.from),
        room_name: String::new(),
        text: crate::gui::pages::chat::reply::preview_of(&inner.text),
    }
}

/// A contact-request candidate (rung 2), for `from` newly listed under `server`'s Requests by a
/// live DM: a contact request, or a DM the person's own setting refused (engine/reach.rs). It says
/// who, never what (a request's words are a claimed name and a pass). Never from a friend: a
/// friend's message is not filed as a request.
pub(crate) fn contact_request(gs: &GuiState, server: &str, from: &str) -> Candidate {
    Candidate {
        kind: "contact_request".into(),
        from_me: from == gs.profile_public_key,
        server: crate::gui::pages::chat::norm_server_url(server),
        room: None,
        conversation: format!("dm:{from}"),
        sender: crate::engine::dm::dm_display_name(gs, from),
        ..Default::default()
    }
}

/// Is `key` listed under Requests in `store`? Asked before and after a live DM is taken in, so a
/// newly listed request (and only that) becomes a candidate.
pub(crate) fn requested(store: Option<&crate::net::dm_store::DmStore>, key: &str) -> bool {
    store.is_some_and(|s| s.requests().iter().any(|r| r.key == key))
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
        quiet_friends: gs.notif_quiet_friends,
        quiet_mentions: gs.notif_quiet_mentions,
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
    let rt = &mut state.gui_state.notify;
    if out.list {
        rt.list.add(&c, show, now);
    }
    let last = &mut rt.last_notice;
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

    fn kind_of(c: Option<Candidate>) -> String {
        c.map(|c| c.kind).unwrap_or_default()
    }

    /// A mention is `@name`, never a word that merely contains it, and a line of mine is mine;
    /// a line that is not a mention is a reply when it answers a line of MINE, else an ordinary
    /// line. Seen red 2026-10-30 with `is_mention`'s boundary check removed: "@Annabel hi" was
    /// Ann's; seen red 2026-10-31 with the reply test reading `reply_to_from.is_some()`: a reply to
    /// Cy's line notified Ann.
    #[test]
    fn a_room_line_is_a_mention_a_reply_to_me_or_an_ordinary_line() {
        let g = gs();
        let s = "https://a.example/";
        let line = |name: &str, from: &str, text: &str, reply: Option<&str>| room_line(&g, s, name, "general", from, "Ben", text, reply);
        assert_eq!(kind_of(line("Ann", "k2", "hi Ann", None)), "room_line", "no @");
        assert_eq!(kind_of(line("Ann", "k2", "@Annabel hi", None)), "room_line");
        let m = line("Ann", "k2", "hi @ann", None).expect("a mention");
        assert!(!m.from_me);
        assert_eq!((m.kind.as_str(), m.room.as_deref(), m.room_name.as_str()), ("mention", Some("general"), "general"));
        assert_eq!(m.server, "https://a.example", "the server normalised, as the mutes name it");
        assert!(line("Ann", "me", "@ann note to self", None).is_some_and(|m| m.from_me));
        assert_eq!(kind_of(line("Annie", "k2", "hi @ann", None)), "room_line", "the name signed in with there");
        assert_eq!(kind_of(line("Ann", "k2", "agreed", Some("me"))), "reply", "a reply to my line");
        assert_eq!(kind_of(line("Ann", "k2", "agreed", Some("k3"))), "room_line", "a reply to someone else's");
        assert_eq!(kind_of(line("Ann", "k2", "@ann agreed", Some("me"))), "mention", "a mention first");
    }

    /// A shared room's line from a linked server is a candidate only while it is live and from
    /// someone not blocked, and a line of mine said on the other server is mine. Seen red
    /// 2026-10-31 with the age check removed: a line older than five minutes, as a link re-sends
    /// while catching up, mentioned Ann again ("a link catching up").
    #[test]
    fn a_federated_line_is_live_unblocked_and_knows_its_author() {
        let g = gs();
        let now = 10_000_000u64;
        let f = |ts: u64, from_key: &str, text: &str| {
            serde_json::json!({ "type": "federated_chat", "server_id": "srvB", "server_name": "B", "from_name": "Cy",
                "from_key": from_key, "content": text, "timestamp": ts, "channel": "general" })
        };
        let c = federated_line(&g, "https://a.example", "Ann", &f(now - 1000, "k9", "@ann look"), now).expect("live");
        assert_eq!((c.kind.as_str(), c.sender.as_str(), c.from_me), ("mention", "Cy (B)", false));
        assert!(federated_line(&g, "https://a.example", "Ann", &f(now - FEDERATED_LIVE_MS - 1, "k9", "@ann look"), now).is_none(), "a link catching up");
        assert!(federated_line(&g, "https://a.example", "Ann", &f(now, "me", "@ann hi"), now).is_some_and(|c| c.from_me));
        assert!(federated_line(&g, "https://a.example", "Ann", &f(now, "k9", ""), now).is_none());
    }

    /// A DM from a friend (we follow each other, on the server it came through) carries that, so
    /// the friends' quiet-hours exception can let it through; a contact request never does. Seen
    /// red 2026-10-31 with `is_friend` reading only my follows: a person I follow who does not
    /// follow me counted as a friend.
    #[test]
    fn a_dm_knows_a_friend_on_its_own_server() {
        let mut g = gs();
        let inner = |from: &str| crate::net::dm_pq::DmInner {
            from: from.into(),
            to: "me".into(),
            ts: 1,
            text: "hi".into(),
            sig_b64: String::new(),
            cert: None,
        };
        g.chat_following_keys.insert("k2".into());
        assert!(!dm(&g, "https://a.example", &inner("k2")).from_friend, "I follow them; they do not follow me");
        g.chat_followers.insert("k2".into());
        assert!(dm(&g, "https://a.example/", &inner("k2")).from_friend);
        assert!(!dm(&g, "https://b.example", &inner("k2")).from_friend, "a friend here is not one on another server's store");
        let r = contact_request(&g, "https://a.example/", "k5");
        assert_eq!((r.kind.as_str(), r.conversation.as_str(), r.server.as_str(), r.from_friend), ("contact_request", "dm:k5", "https://a.example", false));
    }
}
