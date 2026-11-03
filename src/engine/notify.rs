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
//! header's bell), and (rung 2, part 2) it carries a token a click on the notice hands back
//! (`notifications::clicks`, opened by engine/tray.rs), and brings the in-world banner and the
//! taskbar's flash as `clicks::cues` says ([`act_on`]). Nothing here runs per frame except
//! [`pump`]: two `Option::take`s for what Settings asked, and the bell's panel shut off the header.

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
        // One Commons room with another of my linked servers: each carrier delivers its own copy
        // of the line, and the notice is one (decide::burst_key).
        commons: crate::gui::pages::chat::in_commons(gs, &server, room),
        server,
        room: Some(room.to_string()),
        conversation: room.to_string(),
        sender: sender.to_string(),
        room_name,
        text: crate::gui::pages::chat::reply::preview_of(content),
        // A room's line is the room's: the always-allow list lifts quiet hours for a person's own
        // conversation only (rung 3), so it is not asked here.
        ..Default::default()
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

/// Is `key` on this identity's always-allow list (rung 3, `notifications::allow`)?
fn always_allowed(gs: &GuiState, key: &str) -> bool {
    gs.settings.notifications.always_allows(&gs.profile_public_key, key)
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
        from_allowed: !from_me && always_allowed(gs, &inner.from),
        server,
        room: None,
        conversation: format!("dm:{peer}"),
        sender: crate::engine::dm::dm_display_name(gs, &inner.from),
        room_name: String::new(),
        text: crate::gui::pages::chat::reply::preview_of(&inner.text),
        ..Default::default()
    }
}

/// A call candidate (rung 3), for a ring that rings here (`call_relay::on_ring`: let through by
/// "who can reach me", and the person idle). It says who, and nothing else: a call has no message.
pub(crate) fn call(gs: &GuiState, from: &str, from_name: &str) -> Candidate {
    let server = crate::gui::pages::chat::norm_server_url(&gs.server_url);
    Candidate {
        kind: "call".into(),
        from_me: from == gs.profile_public_key,
        from_friend: is_friend(gs, &server, from),
        from_allowed: always_allowed(gs, from),
        server,
        room: None,
        conversation: format!("dm:{from}"),
        sender: from_name.to_string(),
        ..Default::default()
    }
}

/// A task candidate (rung 3), for a live `task_notice` from `server` (relay
/// `handlers/task_notice.rs`): a task given to me, or one of mine changed. `None` for a frame with
/// no task, or one whose change was made by someone I blocked. Its text is the task's title, shown
/// only with "who and the message".
pub(crate) fn task(gs: &GuiState, server: &str, frame: &serde_json::Value) -> Option<Candidate> {
    let id = frame.get("task_id").and_then(|v| v.as_i64())?;
    let s = |k: &str| frame.get(k).and_then(|v| v.as_str()).unwrap_or("");
    let by_key = s("by_key");
    if !by_key.is_empty() && crate::engine::block::is_blocked(gs, by_key) {
        return None;
    }
    Some(Candidate {
        kind: "task".into(),
        from_me: !by_key.is_empty() && by_key == gs.profile_public_key,
        server: crate::gui::pages::chat::norm_server_url(server),
        room: None,
        conversation: format!("task:{id}"),
        sender: s("by_name").to_string(),
        text: s("title").to_string(),
        variant: if s("change") == "assigned" { "assigned".into() } else { String::new() },
        ..Default::default()
    })
}

/// A game-event candidate (rung 3) for one folded notice (`notifications::game`). Local: no
/// server, nothing networked; `room_name` carries where the first event happened ("your home"
/// for one with no place, a crop planted in no grow area).
pub(crate) fn game(f: &n::game::Folded) -> Candidate {
    let place = if f.first_where.is_empty() { humanity_core::i18n::t("notifications.event_where_home").to_string() } else { f.first_where.clone() };
    Candidate {
        kind: "game_event".into(),
        conversation: format!("game:{}", f.event),
        room_name: place,
        event: f.event.clone(),
        count: f.count,
        ..Default::default()
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
        from_allowed: always_allowed(gs, from),
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

/// Hand `c` to the rule, and do what it says ([`act_on`]).
pub(crate) fn deliver(state: &mut EngineState, c: Candidate) {
    let now = n::now_secs();
    let out = outcome(state, &c, now);
    act_on(state, c, out, now);
}

/// What THE rule says for `c` at `now`, with what only the engine knows (the window's focus, what
/// is on screen, the burst guard's time).
pub(crate) fn outcome(state: &EngineState, c: &Candidate, now: i64) -> n::Outcome {
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
        on_screen: on_screen(gs, c),
        last_notice: last_notice(&gs.notify, c),
    };
    let out = n::decide(file, c, &synced, &gs.settings.notifications, &at);
    log::debug!("notifications: a {} -> {}", c.kind, out.why.code());
    out
}

/// Do what the rule said for `c`: the list, the burst guard, the sound, the operating system's
/// notice (with the token a click on it hands back), and the cues that need no sound: the
/// in-world banner and the taskbar's flash (rung 2, part 2, `notifications::clicks::cues`). The
/// words of the notice and of the banner are `decide::words`, as the person's "What a notice
/// shows" allows.
pub(crate) fn act_on(state: &mut EngineState, c: Candidate, out: n::Outcome, now: i64) {
    if out.why != n::Why::Notify {
        return;
    }
    let file = n::data::get();
    let local = &state.gui_state.settings.notifications;
    let show = local.show;
    // This kind's own sound (rung 3); the rule already said whether a sound plays at all.
    let sound_id = local.sound_of(file, &c.kind).map(str::to_string);
    // Broadcasting (Studio's Go Live): the frame goes to viewers, so no banner (clicks::cues).
    let broadcasting = state.live_publisher.is_some() || state.gui_state.studio.is_live;
    let window = n::clicks::Window { focused: state.window_focused, hidden: state.tray.hidden, in_world: hud_showing(state), broadcasting };
    let cues = n::clicks::cues(window);
    let is_call = c.kind == "call";
    let (title, body) = n::decide::words(file, &c, show);
    let rt = &mut state.gui_state.notify;
    if out.list {
        rt.list.add(&c, show, now);
    }
    // A kind without the burst guard (a ring) keeps no time for it, so a DM right after is its own.
    if file.kind(&c.kind).is_none_or(|k| k.burst) {
        note_notice(rt, &c, now);
    }
    // No banner for a call: the ringing screen is already over the world (rung 3).
    if cues.banner && !is_call {
        // The banner's second line only when the notice shows the message: its "open HumanityOS
        // to read it" would be odd on HumanityOS's own screen.
        let line = if show == n::Show::NameAndText { body.clone() } else { String::new() };
        rt.banner = Some(n::clicks::Banner { title: title.clone(), body: line, at: std::time::Instant::now() });
    }
    let launch = if out.toast { rt.targets.token(n::list::Entry::of(&c, None, now)) } else { String::new() };
    let sound = if out.sound { sound_id } else { None };
    if let Some(id) = sound.as_deref() {
        play(state, id);
    }
    let tag = n::toast::tag_for(&n::decide::burst_key(&c));
    if is_call {
        // The ring repeats while it rings, and its notice is withdrawn when it ends (pump_ringing).
        let caller = c.conversation.strip_prefix("dm:").unwrap_or_default().to_string();
        let at = std::time::Instant::now();
        state.gui_state.notify.ringing = Some(n::Ringing {
            key: caller,
            tag: tag.clone(),
            group: c.kind.clone(),
            sound,
            next_ring: at + RING_EVERY,
            until: at + RING_FOR,
        });
    }
    if out.toast {
        let expire_ms = if is_call { RING_FOR.as_millis() as u32 } else { n::toast::EXPIRE_MS };
        n::toast::show(n::toast::Toast { title, body, tag, group: c.kind.clone(), launch, expire_ms });
    }
    // The taskbar button's flash, until the window is in front: a person's launch only (a rig's
    // window never flashes in the operator's taskbar), never while hidden (cues).
    if cues.flash && crate::engine::tray::allowed() {
        state.window.request_user_attention(Some(winit::window::UserAttentionType::Informational));
    }
}

/// Is the person playing, with the HUD showing (where the banner is drawn, `pages/hud.rs`)?
fn hud_showing(state: &EngineState) -> bool {
    let g = &state.gui_state;
    state.world_loaded && g.active_page == crate::gui::GuiPage::None && g.show_hud && !g.showroom_active && !g.construction_active
}

/// When `c`'s conversation on its server last notified, for the burst guard.
fn last_notice(rt: &n::Runtime, c: &Candidate) -> Option<i64> {
    rt.last_notice.get(&n::decide::burst_key(c)).copied()
}

/// `c` notified at `now`: the burst guard's time for its conversation on its server (old ones
/// dropped past [`LAST_NOTICE_KEPT`]).
fn note_notice(rt: &mut n::Runtime, c: &Candidate, now: i64) {
    let last = &mut rt.last_notice;
    if last.len() >= LAST_NOTICE_KEPT {
        last.retain(|_, t| now - *t < n::decide::BURST_SECS);
    }
    last.insert(n::decide::burst_key(c), now);
}

/// Queue the sound `id` from the data file (its catalog entry, or the file's path for it).
fn play(state: &mut EngineState, id: &str) {
    if let Some(s) = n::data::get().sound(id) {
        state.pending_sfx.push((s.id.as_str(), s.path.as_str()));
    }
}

/// A ringing call's sound plays again this often (rung 3).
pub(crate) const RING_EVERY: std::time::Duration = std::time::Duration::from_secs(3);
/// And for at most this long, whatever happens: the caller gives up at 30 seconds, and the rest
/// covers a hang-up that never arrives. Linux's call notice expires after it too.
pub(crate) const RING_FOR: std::time::Duration = std::time::Duration::from_secs(45);

/// Per frame: what Settings > Notifications asked for. A test notice goes straight to the
/// operating system with made-up words (whatever the rule would say, so a person can see one),
/// with a direct message's sound if sound is on; Play plays the chosen sound once. And (rung 3)
/// a ringing call's sound and its withdrawal, and the game events the simulation raised.
pub(crate) fn pump(state: &mut EngineState) {
    // The panel shuts on a page without the header (rung 2 part 1's leftover).
    crate::gui::notice_panel::keep_to_header(&mut state.gui_state);
    if std::mem::take(&mut state.gui_state.notify.test_requested) {
        let t = humanity_core::i18n::t;
        // A click on it shows the window (it leads to no conversation).
        let launch = state.gui_state.notify.targets.window_token();
        n::toast::show(n::toast::Toast {
            title: t("notifications.test_title").to_string(),
            body: t("notifications.test_body").to_string(),
            tag: n::toast::tag_for("test"),
            group: "test".into(),
            launch,
            expire_ms: n::toast::EXPIRE_MS,
        });
        let s = &state.gui_state.settings.notifications;
        if s.sound {
            if let Some(id) = s.sound_of(n::data::get(), "dm").map(str::to_string) {
                play(state, &id);
            }
        }
    }
    if let Some(id) = state.gui_state.notify.preview_sound.take() {
        play(state, &id);
    }
    pump_ringing(state);
    pump_game_events(state);
}

/// A ringing call (rung 3): its sound again every [`RING_EVERY`] while the same caller still
/// rings, and once the ring has ended however it ended (answered, declined, the caller's hang-up
/// or give-up, a block, the window closing into the tray) or [`RING_FOR`] passed, the sound stops
/// and the operating system's notice is withdrawn (a call has no banner). One comparison a frame.
pub(crate) fn pump_ringing(state: &mut EngineState) {
    let now = std::time::Instant::now();
    match ring_step(&mut state.gui_state, now) {
        RingStep::Ended(r) => {
            n::toast::withdraw(&r.tag, &r.group);
            log::info!("notifications: the ring ended, its notice withdrawn");
        }
        RingStep::Again(Some(id)) => play(state, &id),
        RingStep::Again(None) | RingStep::Wait => {}
    }
}

/// What a ringing call's notice does this frame.
#[derive(Debug)]
pub(crate) enum RingStep {
    /// Nothing ringing, or not time to ring again.
    Wait,
    /// Ring again (its sound, when it has one).
    Again(Option<String>),
    /// The ring is over: withdraw this notice.
    Ended(n::Ringing),
}

/// [`pump_ringing`]'s decision, on the GUI state alone (so a test can drive it): the ring is over
/// once `call_incoming` no longer names the caller, which is what answering, declining, the
/// caller's hang-up, a block and closing into the tray all do, or once [`RING_FOR`] has passed.
pub(crate) fn ring_step(gs: &mut GuiState, now: std::time::Instant) -> RingStep {
    let Some(r) = gs.notify.ringing.as_mut() else { return RingStep::Wait };
    let still = gs.call_incoming.as_ref().is_some_and(|(k, _)| *k == r.key);
    if !still || now >= r.until {
        return gs.notify.ringing.take().map_or(RingStep::Wait, RingStep::Ended);
    }
    if now < r.next_ring {
        return RingStep::Wait;
    }
    r.next_ring = now + RING_EVERY;
    RingStep::Again(r.sound.clone())
}

/// The game events the simulation raised since the last frame (`systems::push_game_event`), into
/// the folder, and the folded notices now due through THE rule (rung 3). One lock of the list a
/// frame, as the sound list already has; the folder is looked at only when it holds something.
fn pump_game_events(state: &mut EngineState) {
    let now = now_f64();
    let raised: Vec<(String, String)> = state
        .data_store
        .get::<std::sync::Mutex<Vec<(String, String)>>>(humanity_systems::GAME_EVENTS_KEY)
        .and_then(|m| m.lock().ok().map(|mut v| std::mem::take(&mut *v)))
        .unwrap_or_default();
    let rt = &mut state.gui_state.notify;
    for (event, place) in &raised {
        rt.game.offer(event, place, now);
    }
    if rt.game.is_idle() {
        return;
    }
    let file = n::data::get();
    let due = rt.game.due(now, |e| file.game_event(e).map_or(600.0, |g| g.every_secs as f64));
    for f in due {
        deliver(state, game(&f));
    }
}

/// Seconds since the epoch, with the fraction (the game folder's gather is two seconds).
fn now_f64() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
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
        let room = Candidate { kind: "mention".into(), server: "https://b.example".into(), room: Some("global".into()), conversation: "global".into(), ..Default::default() };
        g.chat_active_channel = "commons:global".into();
        assert!(on_screen(&g, &room), "the Commons view shows a shared room from any server");
    }

    /// The burst guard holds a conversation's second notice back for five seconds on ITS server:
    /// two servers' rooms of one name (each has a "global") are two conversations. Seen red
    /// 2026-10-31 with the guard keyed by the conversation alone (part 1's leftover): a mention in
    /// B's #global two seconds after one in A's #global found A's time.
    #[test]
    fn the_burst_guard_keeps_each_servers_rooms_apart() {
        let mut rt = n::Runtime::default();
        let a = Candidate { kind: "mention".into(), server: "https://a.example".into(), room: Some("global".into()), conversation: "global".into(), ..Default::default() };
        let b = Candidate { server: "https://b.example".into(), ..a.clone() };
        note_notice(&mut rt, &a, 100);
        assert_eq!(last_notice(&rt, &a), Some(100));
        assert_eq!(last_notice(&rt, &b), None, "B's #global is another room");
        note_notice(&mut rt, &b, 102);
        assert_eq!((last_notice(&rt, &a), last_notice(&rt, &b)), (Some(100), Some(102)));
    }

    /// A Commons room carried by two of my linked servers is ONE conversation: each carrier delivers
    /// its own copy of a line said there, and a mention bridged between them is one notice (the
    /// second copy inside the burst guard), one sound and one entry in the list, not two. Seen red
    /// 2026-10-31 on part 2 as merged (the critic's finding: two notices, a bell count of 2), with
    /// `burst_key` keying it by server: "B's copy is inside A's five seconds".
    #[test]
    fn a_commons_room_is_one_conversation_whichever_carrier_brings_it() {
        let mut g = gs();
        let (client, _sent) = crate::net::ws_client::WsClient::recording();
        g.ws_client = Some(client);
        g.chat_channels.push(crate::gui::ChatChannel { id: "global".into(), name: "global".into(), federated: true, ..Default::default() });
        g.connections.push(crate::gui::ServerConnection {
            url: "https://b.example".into(),
            identified: true,
            channels: vec![crate::gui::ChatChannel { id: "global".into(), name: "global".into(), federated: true, ..Default::default() }],
            ..Default::default()
        });
        g.federation_links.link_for_tests("https://a.example", "https://b.example");
        let a = room_line(&g, "https://a.example", "Ann", "global", "k2", "Ben", "@ann look", None).expect("A's copy");
        let b = room_line(&g, "https://b.example/", "Ann", "global", "k2", "Ben", "@ann look", None).expect("B's copy");
        assert!(a.commons && b.commons, "both carriers' copies are the Commons room");
        let mut rt = n::Runtime::default();
        note_notice(&mut rt, &a, 100);
        assert_eq!(last_notice(&rt, &b), Some(100), "B's copy is inside A's five seconds");
        rt.list.add(&a, n::Show::Name, 100);
        rt.list.add(&b, n::Show::Name, 101);
        assert_eq!((rt.list.len(), rt.list.unseen()), (1, 1), "one entry, one on the bell");
        // Unlinked, the same room name on two servers stays two rooms.
        g.federation_links = Default::default();
        let a2 = room_line(&g, "https://a.example", "Ann", "global", "k2", "Ben", "@ann look", None).unwrap();
        let b2 = room_line(&g, "https://b.example", "Ann", "global", "k2", "Ben", "@ann look", None).unwrap();
        assert!(!a2.commons && !b2.commons);
        assert_ne!(n::decide::burst_key(&a2), n::decide::burst_key(&b2));
    }

    /// A ringing call's notice (rung 3) rings again every three seconds while the same caller
    /// still rings, and ends, to be withdrawn, however the ring ends: answered or declined (the
    /// ringing screen takes `call_incoming`), the caller's hang-up (frame_ws_poll clears it), a
    /// block (`engine/block.rs` `forget_live` clears it too), another caller's ring in its place,
    /// or the 45 seconds passing. Seen red 2026-11-01 with the caller check left out of
    /// `ring_step` ("answered: the notice ends").
    #[test]
    fn a_ringing_call_rings_again_and_ends_however_the_ring_ends() {
        let start = std::time::Instant::now();
        let ringing = |g: &mut GuiState| {
            g.call_incoming = Some(("ann".into(), "Ann".into()));
            g.notify.ringing = Some(n::Ringing {
                key: "ann".into(),
                tag: "t".into(),
                group: "call".into(),
                sound: Some("sfx.notification".into()),
                next_ring: start + RING_EVERY,
                until: start + RING_FOR,
            });
        };
        let mut g = gs();
        ringing(&mut g);
        assert!(matches!(ring_step(&mut g, start), RingStep::Wait), "not yet time");
        assert!(matches!(ring_step(&mut g, start + RING_EVERY), RingStep::Again(Some(_))), "rings again");
        assert!(matches!(ring_step(&mut g, start + RING_EVERY), RingStep::Wait), "once per three seconds");
        g.call_incoming = None; // answered or declined (the ringing screen), or hung up
        assert!(matches!(ring_step(&mut g, start + RING_EVERY), RingStep::Ended(_)), "answered: the notice ends");
        assert!(g.notify.ringing.is_none() && matches!(ring_step(&mut g, start), RingStep::Wait));
        ringing(&mut g);
        g.call_incoming = Some(("cy".into(), "Cy".into()));
        assert!(matches!(ring_step(&mut g, start), RingStep::Ended(_)), "another caller's ring in its place");
        ringing(&mut g);
        assert!(matches!(ring_step(&mut g, start + RING_FOR), RingStep::Ended(_)), "45 seconds at most");
    }

    /// A folded game notice says how many and where in the event's own words, "your home" for an
    /// event with no place, and only that something needs you when the notice shows nothing.
    /// Seen red 2026-11-01 with `game` passing an empty place through: "One crop is ripe in ."
    #[test]
    fn a_folded_game_notice_says_how_many_and_where() {
        let file = n::data::get();
        let many = game(&n::game::Folded { event: "crop_ripe".into(), count: 12, first_where: "Tower 1".into() });
        assert_eq!(n::decide::words(file, &many, n::Show::Name), ("Crops ready to harvest".to_string(), "12 crops are ripe, the first in Tower 1.".to_string()));
        let one = game(&n::game::Folded { event: "crop_ripe".into(), count: 1, first_where: String::new() });
        assert_eq!(n::decide::words(file, &one, n::Show::Name).1, "One crop is ripe in your home.");
        let hidden = n::decide::words(file, &many, n::Show::Nothing);
        assert!(!hidden.0.contains("Crops") && !hidden.1.contains("Tower"), "only that something needs you: {hidden:?}");
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
        let line = |name: &str, from: &str, text: &str, reply: Option<&str>| room_line(&g, s, name, "global", from, "Ben", text, reply);
        assert_eq!(kind_of(line("Ann", "k2", "hi Ann", None)), "room_line", "no @");
        assert_eq!(kind_of(line("Ann", "k2", "@Annabel hi", None)), "room_line");
        let m = line("Ann", "k2", "hi @ann", None).expect("a mention");
        assert!(!m.from_me);
        assert_eq!((m.kind.as_str(), m.room.as_deref(), m.room_name.as_str()), ("mention", Some("global"), "global"));
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
                "from_key": from_key, "content": text, "timestamp": ts, "channel": "global" })
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
