//! The ACTIVE relay connection's life on the GUI side: parking it while another server is
//! active, bringing a parked one back, closing it on purpose, and what an erased account
//! does to it.
//!
//! `park_active_connection`, `unpark_connection`, `reset_per_server_transients` and their
//! tests moved here VERBATIM from `gui/mod.rs` (2026-10-04), apart from the tests' import
//! line, which names `crate::gui` now that their parent is this module. `gui/mod.rs` stood
//! exactly at its line budget (tests/file_size_ratchet.rs), and the erase handling below
//! belongs beside the parking it has to agree with.
//!
//! ERASED ACCOUNTS (BUG-135). Erasing your account on a server (Settings > Account, the relay's
//! `account_delete`) removes your name, plot and progress there, and the relay then sends
//! `account_erased` (relay.rs). Until then the app stayed connected, so the next automatic
//! reconnect (every relay deploy restarts it; any network drop) identified again, which
//! registered the name again, and the game joined and claimed a new plot: the account came
//! back without the person choosing to come back. Now the receipt closes that server's
//! connection the way the Chat page's Disconnect does (`disconnect_active`), and remembers the
//! server for this identity (`account_erased_on`, kept in the config so a restart does not
//! dial it either). Nothing dials a remembered server by itself: not the boot auto-connect
//! (lib.rs), not the background links (engine/bg_connections.rs). The Chat page's Connect does,
//! after saying that it signs you up again (`ERASED_CONNECT_NOTE`), and pressing it forgets the
//! server (`forget_account_erased`).

use super::{pages, GuiState, ServerConnection};

/// What the Chat page's connect box says under a server whose account this identity erased.
/// One sentence, naming the control it sits above (BUG-135). The web login screen says the same
/// with its own button's name (web/chat/app.js `ERASED_ENTER_NOTE`).
pub const ERASED_CONNECT_NOTE: &str =
    "Your account on this server was erased, so pressing Connect signs you up again as a new account on this server.";

/// What the app says instead when the server's erase did not finish: part of it failed
/// (`<table>_FAILED` in the relay's receipt, storage/account.rs `delete_account`), so the
/// account may still partly exist there and "signs you up again" would not be true. One
/// sentence with the next step; it is shown in the Chat page's connect box and kept under the
/// game's HUD (engine/account_erase.rs). The web login screen has its own (web/chat/app.js
/// `ERASE_UNFINISHED_NOTE`), naming its own button.
pub const ERASE_UNFINISHED_NOTE: &str =
    "The erase of your account on this server did not finish, so open Chat, press Connect and erase it again from Settings.";

/// How an erase on a server ended, as the relay's `account_erased` said (`partial`). Either
/// way the app leaves that server and never dials it by itself; the two only differ in what
/// it tells the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EraseOutcome {
    /// Everything this server kept about the account went.
    Erased,
    /// Part of the erase failed on the server, so the account may still partly exist there.
    Unfinished,
}

impl EraseOutcome {
    /// Read from the relay's `account_erased` frame: `"partial": true` when any part of the
    /// erase failed (relay msg_handlers.rs `handle_account_delete`). Both sockets' handlers
    /// read it here (engine/account_erase.rs).
    pub fn from_receipt(frame: &serde_json::Value) -> Self {
        if frame.get("partial").and_then(|v| v.as_bool()) == Some(true) {
            EraseOutcome::Unfinished
        } else {
            EraseOutcome::Erased
        }
    }
}

/// How an erased server is remembered for an identity: its public key and the server's
/// normalized URL (`pages::chat::norm_server_url`), so the address with or without a trailing
/// slash is the same server, and restoring a different identity is not refused a server
/// someone else erased. A space joins them: neither a hex key nor a normalized URL holds one.
pub fn erased_entry(public_key: &str, url: &str) -> String {
    format!("{public_key} {}", pages::chat::norm_server_url(url))
}

#[cfg(feature = "native")]
impl GuiState {
    /// Park the ACTIVE connection (socket and all per-server chat state) into
    /// `connections`, keyed by normalized URL, leaving the legacy fields empty
    /// and ready for another server. The socket stays OPEN: a parked server
    /// keeps receiving in the background (src/engine/bg_connections.rs) and a
    /// later unpark restores it instantly, no reconnect, no lost messages.
    /// Admin-domain state (roles, bans, server settings, refetch latches) is
    /// reset instead of parked -- those pages refetch on demand and must never
    /// show one server's data while another is active.
    pub fn park_active_connection(&mut self) {
        use std::mem::take;
        // Key by the URL the socket was DIALED for, never the server_url
        // input draft (the user may have edited it since connecting).
        let dialed = if self.connected_server_url.trim().is_empty() {
            self.server_url.clone()
        } else {
            self.connected_server_url.clone()
        };
        let url = pages::chat::norm_server_url(&dialed);
        let conn = ServerConnection {
            url: url.clone(),
            display_url: dialed.trim().to_string(),
            ws: self.ws_client.take(),
            status: take(&mut self.ws_status),
            identified: take(&mut self.ws_identified),
            manually_disconnected: take(&mut self.ws_manually_disconnected),
            reconnect_timer: take(&mut self.ws_reconnect_timer),
            reconnect_delay: std::mem::replace(&mut self.ws_reconnect_delay, crate::net::ws_client::RECONNECT_DELAY_INITIAL_SECS),
            reconnect_attempts: take(&mut self.ws_reconnect_attempts),
            rate_limited: take(&mut self.ws_rate_limited),
            msgs_in: take(&mut self.ws_msgs_in),
            history_fetched: take(&mut self.history_fetched),
            // Re-arm the background history backfill for this server's
            // FEDERATED rooms at every park (field test 5): the active life
            // only fetched the room the user had open, so a parked buffer is
            // usually NARROW -- without this, clicking through servers left
            // every carrier with no #general history and the Commons view
            // merged to empty. The merge dedups, so re-fetching is cheap.
            history_queue: self
                .chat_channels
                .iter()
                .filter(|c| c.federated)
                .map(|c| c.id.clone())
                .collect(),
            history_rx: None,
            messages: take(&mut self.chat_messages),
            channels: take(&mut self.chat_channels),
            active_channel: std::mem::replace(&mut self.chat_active_channel, "general".to_string()),
            users: take(&mut self.chat_users),
            dms: take(&mut self.chat_dms),
            pins: take(&mut self.chat_pins),
            friends: take(&mut self.chat_friends),
            following_keys: take(&mut self.chat_following_keys),
            followers: take(&mut self.chat_followers),
            sent_timestamps: take(&mut self.chat_sent_timestamps),
        };
        // Only remember a connection that ever existed; an empty-URL park
        // (fresh boot) would otherwise create a ghost entry.
        if !conn.url.is_empty() && (conn.ws.is_some() || !conn.messages.is_empty()) {
            self.connections.retain(|c| c.url != conn.url);
            self.connections.push(conn);
        }
        self.ws_status = "Not connected".to_string();
        self.server_connected = false;
        self.reset_per_server_transients();
    }

    /// Restore a parked connection into the legacy active fields. Returns
    /// true when `url` matched a parked entry (caller must then SKIP the
    /// fresh-connect path: the restored socket is already live). On false
    /// the caller connects as before.
    pub fn unpark_connection(&mut self, url: &str) -> bool {
        let key = pages::chat::norm_server_url(url);
        let Some(pos) = self.connections.iter().position(|c| c.url == key) else {
            return false;
        };
        let conn = self.connections.swap_remove(pos);
        self.server_url = conn.display_url.clone();
        self.connected_server_url = conn.display_url;
        self.ws_client = conn.ws;
        self.ws_status = conn.status;
        self.ws_identified = conn.identified;
        self.ws_manually_disconnected = conn.manually_disconnected;
        self.ws_reconnect_timer = conn.reconnect_timer;
        self.ws_reconnect_delay = conn.reconnect_delay;
        self.ws_reconnect_attempts = conn.reconnect_attempts;
        self.ws_rate_limited = conn.rate_limited;
        self.ws_msgs_in = conn.msgs_in;
        self.history_fetched = conn.history_fetched;
        self.chat_messages = conn.messages;
        self.chat_channels = conn.channels;
        self.chat_active_channel = if conn.active_channel.is_empty() {
            "general".to_string()
        } else {
            conn.active_channel
        };
        self.chat_users = conn.users;
        self.chat_dms = conn.dms;
        self.chat_pins = conn.pins;
        self.chat_friends = conn.friends;
        self.chat_following_keys = conn.following_keys;
        self.chat_followers = conn.followers;
        self.chat_sent_timestamps = conn.sent_timestamps;
        self.server_connected = self.ws_identified;
        self.reset_per_server_transients();
        true
    }

    /// Clear state that must never leak across a server switch and cannot be
    /// parked: admin-domain caches (their pages refetch via request latches),
    /// in-flight fetches, and per-conversation UI transients.
    fn reset_per_server_transients(&mut self) {
        self.chat_banned_requested = false;
        self.chat_muted_requested = false;
        self.game_bans_requested = false;
        self.backup_list_requested = false;
        self.chat_roles.clear();
        self.chat_banned_users.clear();
        self.chat_muted_users.clear();
        self.server_settings = None;
        self.server_settings_requested = false;
        self.erase_memory_days = None;
        self.server_settings_draft = None;
        self.server_settings_changed_underneath = false;
        self.chat_typing_users.clear();
        self.history_rx = None;
        self.chat_reply_to = None;
        self.chat_edit_target = None;
        self.chat_search_results.clear();
        // Trades are per server (2026-10-02): a switch re-fetches its own.
        self.trades.clear();
        self.trades_synced = false;
        // Sealed-sender DMs: the local history store and fetch high-water
        // are per (identity, server) — drop them so the next server loads
        // its own store from disk and re-fetches its own mailbox.
        if let Some(store) = self.dm_store.take() {
            store.save();
        }
        self.dm_fetch_sent = false;
    }

    /// Close the ACTIVE server's connection on purpose: nothing reconnects it by itself
    /// (`ws_manually_disconnected` holds the backoff reconnect and the boot auto-connect off)
    /// until the person presses Connect. The Chat page's Disconnect, Server Settings'
    /// Disconnect and an erased account (`account_erased_on_active`) all close it here.
    pub fn disconnect_active(&mut self) {
        if let Some(ref mut client) = self.ws_client {
            client.disconnect();
        }
        self.ws_client = None;
        self.ws_status = "Disconnected".to_string();
        self.ws_manually_disconnected = true;
        self.chat_users.clear();
        // The self-dropped-socket path that clears this (engine/frame_ws_poll.rs) never runs once
        // `ws_client` is None, so an unanswered request would otherwise stay marked as made.
        self.server_settings_requested = false;
    }

    /// The ACTIVE server confirmed it erased our account (relay `account_erased`; BUG-135):
    /// close the connection as Disconnect does and remember the server it was DIALED for (never
    /// the server field being edited), so nothing dials it again by itself. `outcome` says
    /// whether the erase finished. The caller saves the config, and takes the game out of the
    /// shared world (engine/account_erase.rs).
    pub fn account_erased_on_active(&mut self, outcome: EraseOutcome) {
        let dialed = if self.connected_server_url.trim().is_empty() {
            self.server_url.clone()
        } else {
            self.connected_server_url.clone()
        };
        self.disconnect_active();
        self.account_erased_on.insert(erased_entry(&self.profile_public_key, &dialed), outcome);
    }

    /// The same for a PARKED server: the person switched servers between sending the erase and
    /// its receipt, so it arrived on a background link (engine/bg_connections.rs). That link is
    /// closed and never redialed by itself.
    pub fn account_erased_on_parked(&mut self, ci: usize, outcome: EraseOutcome) {
        let Some(conn) = self.connections.get_mut(ci) else { return };
        if let Some(ref mut ws) = conn.ws {
            ws.disconnect();
        }
        conn.ws = None;
        conn.identified = false;
        conn.manually_disconnected = true;
        conn.status = "Disconnected".to_string();
        let entry = erased_entry(&self.profile_public_key, &conn.url);
        self.account_erased_on.insert(entry, outcome);
    }

    /// True when this identity erased its account on the server at `url`, finished or not:
    /// the app does not dial it by itself, and the Chat page says what to do there.
    pub fn account_erased_here(&self, url: &str) -> bool {
        self.account_erased_on.contains_key(&erased_entry(&self.profile_public_key, url))
    }

    /// The sentence the Chat page's connect box shows above Connect for the server at `url`:
    /// that Connect signs up again, or, when the erase did not finish there, to erase again.
    /// None for a server this identity never erased on.
    pub fn erase_note(&self, url: &str) -> Option<&'static str> {
        match self.account_erased_on.get(&erased_entry(&self.profile_public_key, url))? {
            EraseOutcome::Erased => Some(ERASED_CONNECT_NOTE),
            EraseOutcome::Unfinished => Some(ERASE_UNFINISHED_NOTE),
        }
    }

    /// Whether the app may dial the active server by itself this frame: at boot, after an
    /// unlock (which clears `ws_manually_disconnected`), or after a server switch (lib.rs, the
    /// auto-connect). Never with no identity unlocked (a locked seed would register a keyless
    /// name-squatter), never after a Disconnect, and never on a server this identity erased
    /// its account on (BUG-135): that would sign up again without the person asking.
    pub fn may_auto_connect(&self) -> bool {
        !self.server_url.is_empty()
            && self.ws_client.is_none()
            && !self.user_name.is_empty()
            && self.onboarding_complete
            && !self.ws_manually_disconnected
            && !self.account_erased_here(&self.server_url)
            && self.ws_reconnect_timer <= 0.0
            && self.ws_reconnect_attempts == 0
            && self.private_key_bytes.is_some()
    }

    /// The person pressed Connect for `url`: they chose to sign up there again.
    pub fn forget_account_erased(&mut self, url: &str) {
        let entry = erased_entry(&self.profile_public_key, url);
        self.account_erased_on.remove(&entry);
    }

    /// The Chat page's Connect, pressed for `url`: whether this connection's identify says
    /// `sign_up_again` (true when this identity's account there was erased, so the note above
    /// the button said Connect signs up again), and the person's choice is then made: the
    /// erase is forgotten here, so it is said once, by this one connection. The relay keeps
    /// its own memory of the erase for a limited time and signs nothing up again without the
    /// field (relay handlers/sign_ups.rs); an automatic reconnect never carries it, and a later
    /// press finds nothing to forget. Returns false on a server never erased here.
    pub fn take_sign_up_again(&mut self, url: &str) -> bool {
        let chosen = self.account_erased_here(url);
        self.forget_account_erased(url);
        chosen
    }

    /// The relay's `server_settings_state` (sent after `server_settings_request`, and to
    /// everyone after an admin's change): cached for the admin page and the message limits.
    /// The erase window is read from the RAW frame, never from the parsed settings, whose
    /// missing fields are filled with defaults: an older relay that has no such setting would
    /// otherwise be shown promising "30 days" it does not keep (review finding 6).
    pub fn on_server_settings_state(&mut self, frame: &serde_json::Value) {
        let Some(s) = frame.get("settings") else { return };
        self.erase_memory_days = s.get("erased_accounts_ttl_days").and_then(|v| v.as_i64()).filter(|d| *d > 0);
        match serde_json::from_value::<crate::relay::storage::ServerSettings>(s.clone()) {
            Ok(settings) => {
                log::info!(
                    "Server settings updated (max_chars: u={} v={} m={} a={})",
                    settings.max_chars_unverified, settings.max_chars_verified,
                    settings.max_chars_mod, settings.max_chars_admin
                );
                let before = self.server_settings.take();
                self.server_settings_changed_underneath = changed_under_edits(
                    self.server_settings_draft.as_ref(),
                    before.as_ref(),
                    &settings,
                    self.server_settings_changed_underneath,
                );
                self.server_settings_draft = draft_after_settings_arrive(self.server_settings_draft.take(), before.as_ref(), &settings);
                self.server_settings = Some(settings);
            }
            Err(e) => log::warn!("Failed to parse server_settings_state: {e}"),
        }
    }

    /// Ask the active server for its settings once on every socket that has signed in, whether
    /// or not they are known (engine/frame_ws_poll.rs calls it every frame). Asking at connect,
    /// before the identify handshake finished, never worked: the relay drops every message other
    /// than the identify until it has bound the socket (review finding 14). Asking only while
    /// they were unknown (final review of 085441749, findings 1 and 2) left the page waiting for
    /// the whole session when one answer was lost or did not parse, and after a reconnect never
    /// saw a change another admin saved meanwhile, which a Save of the stale copy then reverted.
    /// The answer goes only to the asker (relay.rs), so asking per sign-in is cheap.
    pub fn ask_server_settings_once(&mut self) {
        if !self.server_settings_wanted() {
            return;
        }
        if let Some(ref client) = self.ws_client {
            client.send(&serde_json::json!({ "type": "server_settings_request" }).to_string());
            self.server_settings_requested = true;
        }
    }

    /// The relay accepted this socket's sign-in (its first `peer_list`, engine/frame_ws_poll.rs).
    /// Returns true the first time on a socket: every new socket starts unidentified. A newly
    /// signed-in socket has not asked for the server's settings yet, whatever an earlier socket
    /// did, so `ask_server_settings_once` asks on it: every new socket (Connect, the reconnect
    /// timer, the name-collision reconnect) signs in through here. A parked link brought back
    /// is already signed in; `reset_per_server_transients` clears the mark for it.
    pub fn socket_signed_in(&mut self) -> bool {
        let first_on_socket = !self.ws_identified;
        self.ws_identified = true;
        if first_on_socket {
            self.server_settings_requested = false;
        }
        first_on_socket
    }

    /// Whether `ask_server_settings_once` sends the request this frame.
    pub(crate) fn server_settings_wanted(&self) -> bool {
        should_ask_server_settings(self.ws_identified, self.server_settings_requested)
    }

    /// The Server Settings page's Save and Revert drop the working copy, and with it the note
    /// that the server's settings changed under its edits: the next copy is made from them.
    pub fn discard_server_settings_draft(&mut self) {
        self.server_settings_draft = None;
        self.server_settings_changed_underneath = false;
    }
}

/// Whether the Server Settings page says the server's settings changed under the admin's
/// unsaved edits, once a `server_settings_state` has arrived (`on_server_settings_state`).
/// True when the copy holds edits (it differs from `before`, the settings it was made from,
/// so `draft_after_settings_arrive` keeps it) and the settings that arrived are not those
/// (another admin saved, or they changed while this app was offline); still true while those
/// edits are kept, once said (`already`). Save sends the whole copy, so without the note it
/// would quietly put the old values back over the other change. False for a copy with no
/// edits, which follows the new settings, and for none. Pure.
pub(crate) fn changed_under_edits(
    draft: Option<&crate::relay::storage::ServerSettings>,
    before: Option<&crate::relay::storage::ServerSettings>,
    now: &crate::relay::storage::ServerSettings,
    already: bool,
) -> bool {
    match (draft, before) {
        (Some(d), Some(b)) if d != b => already || b != now,
        _ => false,
    }
}

/// The Server Settings page's working copy this frame (`server_settings_draft`): the copy it
/// holds, or a fresh one made from the server's settings. None while those have not arrived,
/// so the page shows no form and nothing can be saved (review of BUG-135 option 2, second
/// round, finding 3: the copy was made from DEFAULTS the first frame the admin section drew,
/// before the answer to `ask_server_settings_once` came, and Save, or the #local checkbox,
/// which sends the whole copy, wrote every default back; the relay's expiry pass then applied
/// a lowered DM-mailbox or retention window at once). Pure.
pub(crate) fn page_draft(
    draft: Option<&crate::relay::storage::ServerSettings>,
    cached: Option<&crate::relay::storage::ServerSettings>,
) -> Option<crate::relay::storage::ServerSettings> {
    let cached = cached?;
    Some(draft.cloned().unwrap_or_else(|| cached.clone()))
}

/// The page's working copy when a `server_settings_state` arrives (`on_server_settings_state`):
/// replaced by the new settings while it holds no unsaved edits (it still equals `before`, the
/// settings it was made from), so a Save's own echo or another admin's change shows at once
/// instead of the old values reading as unsaved changes; kept when it holds edits; replaced
/// when there were no settings under it. None stays None: the page makes it when it draws.
/// Pure.
pub(crate) fn draft_after_settings_arrive(
    draft: Option<crate::relay::storage::ServerSettings>,
    before: Option<&crate::relay::storage::ServerSettings>,
    now: &crate::relay::storage::ServerSettings,
) -> Option<crate::relay::storage::ServerSettings> {
    match draft {
        None => None,
        Some(d) if before.map_or(true, |b| *b == d) => Some(now.clone()),
        Some(d) => Some(d),
    }
}

/// `ask_server_settings_once`'s decision: only on a signed-in connection, and once per signed-in
/// socket (`asked` is cleared when a socket signs in, drops, is disconnected or the server
/// changes), whether or not the settings are already known. Pure.
pub(crate) fn should_ask_server_settings(identified: bool, asked: bool) -> bool {
    identified && !asked
}

#[cfg(all(test, feature = "native"))]
mod park_unpark_tests {
    use crate::gui::{ChatChannel, ChatDm, ChatMessage, GuiState};

    fn connected_state(url: &str) -> GuiState {
        let mut state = GuiState::default();
        state.server_url = url.to_string();
        state.connected_server_url = url.to_string();
        state.chat_messages.push(ChatMessage {
            sender_name: "Ada".into(),
            content: format!("hello from {url}"),
            channel: "ops".into(),
            timestamp_ms: 42,
            ..Default::default()
        });
        state.chat_channels.push(ChatChannel {
            id: "ops".into(),
            name: "Ops".into(),
            ..Default::default()
        });
        state.chat_active_channel = "ops".to_string();
        state.ws_identified = true;
        state.history_fetched = true;
        state.chat_dms.push(ChatDm {
            user_name: "Bela".into(),
            user_key: "bela_key".into(),
            last_message: "see you there".into(),
            timestamp: "12:00".into(),
            unread: true,
        });
        state
            .chat_pins
            .entry("ops".to_string())
            .or_default();
        state
    }

    #[test]
    fn park_then_unpark_restores_messages_room_and_identity() {
        let mut state = connected_state("https://a.example/");
        state.park_active_connection();
        assert_eq!(state.connections.len(), 1, "one parked entry");
        assert!(state.chat_messages.is_empty(), "legacy buffer handed off");
        assert_eq!(state.chat_active_channel, "general", "fresh default room");
        assert!(!state.ws_identified);

        // Visit another server, then come back.
        state.server_url = "https://b.example".to_string();
        state.connected_server_url = "https://b.example".to_string();
        assert!(state.unpark_connection("https://a.example"), "trailing-slash difference must still match");
        assert_eq!(state.connections.len(), 0);
        assert_eq!(state.chat_messages.len(), 1);
        assert_eq!(state.chat_messages[0].content, "hello from https://a.example/");
        assert_eq!(state.chat_active_channel, "ops", "returns to the room you left");
        assert_eq!(state.connected_server_url, "https://a.example/");
        assert!(state.ws_identified, "identified state survives the round trip");
        assert!(state.history_fetched, "no needless history refetch on return");
        // DMs, groups, and pins ride the round trip too (DM/groups
        // verification pass, 2026-08-14): losing a DM list on switch
        // would read as vanished conversations.
        assert_eq!(state.chat_dms.len(), 1);
        assert_eq!(state.chat_dms[0].user_key, "bela_key");
        assert!(state.chat_dms[0].unread, "unread DM mark survives");
        assert!(state.chat_pins.contains_key("ops"));
    }

    #[test]
    fn park_keys_by_dialed_url_not_the_edited_input_draft() {
        let mut state = connected_state("https://a.example");
        // User typed a new address into the input without connecting yet.
        state.server_url = "https://c.example".to_string();
        state.park_active_connection();
        assert!(state.unpark_connection("https://a.example"), "parked under what was dialed");
        assert_eq!(state.chat_messages.len(), 1);
    }

    #[test]
    fn empty_park_leaves_no_ghost_and_transients_reset_on_switch() {
        let mut state = GuiState::default();
        state.park_active_connection();
        assert!(state.connections.is_empty(), "nothing to park, nothing stored");

        let mut state = connected_state("https://a.example");
        state.chat_banned_requested = true;
        state.server_settings_draft = None;
        state.park_active_connection();
        assert!(!state.chat_banned_requested, "admin refetch latch reset for the next server");
        assert!(!state.unpark_connection("https://never-visited.example"));
    }
}

/// BUG-135: what the app does when a server confirms it erased our account. No socket is
/// opened here (the receipt's handling never needs one: it closes whatever is there).
#[cfg(all(test, feature = "native"))]
mod erased_account_tests {
    use super::{erased_entry, EraseOutcome, ERASED_CONNECT_NOTE, ERASE_UNFINISHED_NOTE};
    use crate::gui::{GuiState, ServerConnection};

    const KEY: &str = "ab12cd34";

    fn on(url: &str) -> GuiState {
        let mut state = GuiState::default();
        state.profile_public_key = KEY.to_string();
        state.server_url = url.to_string();
        state.connected_server_url = url.to_string();
        state
    }

    /// The receipt closes the server like Disconnect and remembers it, so neither the
    /// backoff reconnect nor the boot auto-connect dials it (both wait on these two answers),
    /// while every other server is dialed as before.
    ///
    /// Seen red 2026-10-04 on 1c41de3b9 with the receipt doing nothing (the app before
    /// BUG-135's fix): "the server was left open to the automatic reconnect".
    #[test]
    fn an_erase_receipt_closes_the_server_and_nothing_dials_it_by_itself() {
        let mut state = on("https://a.example/");
        state.ws_status = "Connected".to_string();
        state.account_erased_on_active(EraseOutcome::Erased);
        assert!(state.ws_manually_disconnected, "the server was left open to the automatic reconnect");
        assert!(state.ws_client.is_none());
        assert_eq!(state.ws_status, "Disconnected");
        assert!(state.account_erased_here("https://a.example"), "the erased server is not remembered");
        assert!(state.account_erased_here(" https://a.example "), "the same server written another way");
        assert!(!state.account_erased_here("https://b.example"), "another server stays dialable");
        // Pressing Connect there is the person choosing to sign up again.
        state.forget_account_erased("https://a.example");
        assert!(!state.account_erased_here("https://a.example/"));
    }

    /// The server remembered is the one the socket was DIALED for, not the server field being
    /// edited, and only for the identity that erased it. Seen red 2026-10-04 on 1c41de3b9 with
    /// the receipt doing nothing: "assertion failed: state.account_erased_here(\"https://a.example\")".
    #[test]
    fn the_erased_server_is_the_dialed_one_and_only_for_that_identity() {
        let mut state = on("https://a.example");
        state.server_url = "https://typing-another.example".to_string();
        state.account_erased_on_active(EraseOutcome::Erased);
        assert!(state.account_erased_here("https://a.example"));
        assert!(!state.account_erased_here("https://typing-another.example"));
        // Restoring a different identity does not inherit the refusal.
        state.profile_public_key = "ffee9988".to_string();
        assert!(!state.account_erased_here("https://a.example"));
        assert_eq!(erased_entry(KEY, "https://a.example/"), format!("{KEY} {}", crate::gui::pages::chat::norm_server_url("https://a.example")));
    }

    /// A receipt on a parked link (switched away before it came) closes that link and holds
    /// it closed: the background redial skips a link marked manually disconnected.
    ///
    /// Seen red 2026-10-04 on 1c41de3b9 with the receipt on a parked link doing nothing:
    /// "assertion failed: conn.manually_disconnected && conn.ws.is_none() && !conn.identified".
    #[test]
    fn an_erase_receipt_on_a_parked_server_holds_that_link_closed() {
        let mut state = on("https://b.example");
        state.connections.push(ServerConnection {
            url: crate::gui::pages::chat::norm_server_url("https://a.example"),
            display_url: "https://a.example".to_string(),
            identified: true,
            status: "Connected".to_string(),
            ..Default::default()
        });
        state.account_erased_on_parked(0, EraseOutcome::Erased);
        let conn = &state.connections[0];
        assert!(conn.manually_disconnected && conn.ws.is_none() && !conn.identified);
        assert!(state.account_erased_here("https://a.example"));
        assert!(!state.account_erased_here("https://b.example"), "the active server is untouched");
        assert!(!state.ws_manually_disconnected);
    }

    /// The Chat page's sentence: one sentence, naming the button it sits above, saying what
    /// pressing it does.
    ///
    /// Seen red 2026-10-04 on 1c41de3b9 against a note in the old words: "assertion `left ==
    /// right` failed: Your account on this server was erased. Reconnect to come back.".
    #[test]
    fn the_connect_box_says_connecting_signs_up_again_in_one_sentence() {
        assert_eq!(ERASED_CONNECT_NOTE.matches(". ").count(), 0, "{ERASED_CONNECT_NOTE}");
        assert!(ERASED_CONNECT_NOTE.ends_with('.'));
        assert!(ERASED_CONNECT_NOTE.contains("Connect") && ERASED_CONNECT_NOTE.contains("signs you up again"));
        // The unfinished erase's sentence: one sentence, a next step, and no promise of a
        // fresh account (the old one may still partly exist).
        assert_eq!(ERASE_UNFINISHED_NOTE.matches(". ").count(), 0, "{ERASE_UNFINISHED_NOTE}");
        assert!(ERASE_UNFINISHED_NOTE.ends_with('.'));
        assert!(ERASE_UNFINISHED_NOTE.contains("did not finish") && ERASE_UNFINISHED_NOTE.contains("erase it again"));
        assert!(!ERASE_UNFINISHED_NOTE.contains("signs you up again"));
    }

    /// Review of BUG-135: an erase that partly failed on the relay (`<table>_FAILED` in its
    /// receipt) still closes the server, but the app must not say that Connect signs you up
    /// again as a new account, which is untrue while the old one partly exists; it says the
    /// erase did not finish and to erase again. The decision comes from the receipt frame.
    ///
    /// Seen red 2026-10-04 on 825aa0af4 with the frame's `partial` ignored: "assertion `left ==
    /// right` failed: an unfinished erase promised a fresh sign-up / left: Some(\"Your account on
    /// this server was erased, so pressing Connect signs you up again as a new account on this
    /// server.\")".
    #[test]
    fn an_unfinished_erase_says_to_erase_again_instead_of_signing_up() {
        let partial = serde_json::json!({ "type": "account_erased", "to": KEY, "partial": true });
        let whole = serde_json::json!({ "type": "account_erased", "to": KEY, "partial": false });
        let mut state = on("https://a.example");
        state.account_erased_on_active(EraseOutcome::from_receipt(&partial));
        assert_eq!(state.erase_note("https://a.example"), Some(ERASE_UNFINISHED_NOTE), "an unfinished erase promised a fresh sign-up");
        assert!(state.ws_manually_disconnected && state.account_erased_here("https://a.example"), "still left, still undialed");
        let mut state = on("https://a.example");
        state.account_erased_on_active(EraseOutcome::from_receipt(&whole));
        assert_eq!(state.erase_note("https://a.example"), Some(ERASED_CONNECT_NOTE));
        assert_eq!(state.erase_note("https://b.example"), None, "no note where nothing was erased");
    }

    /// BUG-135, the operator's option 2 (2026-10-04): the relay remembers an erase for a while
    /// and signs nothing up again unless the identify says `sign_up_again`. Only the Chat page's
    /// Connect, pressed under the erase note, says it, and only for that one connection: the
    /// choice is taken once, so a reconnect after it (or a second press) does not repeat it,
    /// and a server never erased here never gets it.
    ///
    /// Seen red 2026-10-04 with `take_sign_up_again` returning the erase without forgetting it:
    /// "the choice is made: the note is gone".
    #[test]
    fn the_connect_after_an_erase_says_sign_up_again_once() {
        let mut state = on("https://a.example");
        state.account_erased_on_active(EraseOutcome::Erased);
        assert!(!state.take_sign_up_again("https://b.example"), "a server never erased here got sign_up_again");
        assert!(state.take_sign_up_again("https://a.example/"), "the Connect under the erase note did not say sign_up_again");
        assert!(!state.account_erased_here("https://a.example"), "the choice is made: the note is gone");
        assert!(!state.take_sign_up_again("https://a.example"), "a second press after coming back said sign_up_again again");
        // An unfinished erase: its note also sits above Connect, and pressing it signs in to
        // finish the erase, which the relay allows only with the field.
        let mut state = on("https://a.example");
        state.account_erased_on_active(EraseOutcome::Unfinished);
        assert!(state.take_sign_up_again("https://a.example"));
    }

    /// The wiring of the above, read from the source the way `both_sockets_hand_the_erase_receipt_here`
    /// reads it (a click cannot be driven in a unit test): the Chat page's Connect takes the
    /// choice and picks the signing-up-again connect from it, and nothing else in the app
    /// dials with it. An automatic reconnect never reaches either line.
    ///
    /// Seen red 2026-10-04 with the Connect button's choice line replaced by `if false {`: "the
    /// Connect button does not take the sign-up-again choice".
    #[test]
    fn only_the_chat_pages_connect_signs_up_again() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        let panel = read("src/gui/pages/chat/left_panel.rs");
        assert!(panel.contains("let connect = if state.take_sign_up_again(&url) {"), "the Connect button does not take the sign-up-again choice");
        assert!(panel.contains("WsClient::connect_signing_up_again"), "the Connect button never signs up again");
        let mut callers = Vec::new();
        for dir in ["src/gui", "src/engine", "src/net", "src/lib.rs"] {
            let path = root.join(dir);
            let files: Vec<std::path::PathBuf> = if path.is_file() {
                vec![path]
            } else {
                let mut v = Vec::new();
                let mut stack = vec![path];
                while let Some(d) = stack.pop() {
                    for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                        let p = e.path();
                        if p.is_dir() { stack.push(p) } else if p.extension().is_some_and(|x| x == "rs") { v.push(p) }
                    }
                }
                v
            };
            for f in files {
                let src = std::fs::read_to_string(&f).unwrap_or_default();
                if src.contains("connect_signing_up_again") && !f.ends_with("ws_client.rs") && !f.ends_with("connections.rs") {
                    callers.push(f.display().to_string());
                }
            }
        }
        assert_eq!(callers.len(), 1, "only the Chat page's Connect may sign up again: {callers:?}");
    }

    /// The boot, unlock and server-switch auto-connect (lib.rs) never dials a server this
    /// identity erased its account on, finished or not, and still dials every other one.
    ///
    /// Seen red 2026-10-04 on 825aa0af4 with the erased check left out of the decision:
    /// "the auto-connect would sign up again on the erased server".
    #[test]
    fn the_auto_connect_never_dials_an_erased_server() {
        let mut state = on("https://a.example");
        state.user_name = "Ada".to_string();
        state.onboarding_complete = true;
        state.private_key_bytes = Some(vec![7u8; 32]);
        state.ws_manually_disconnected = false;
        state.ws_reconnect_timer = 0.0;
        state.ws_reconnect_attempts = 0;
        assert!(state.may_auto_connect(), "the setup itself must allow a connect");
        state.account_erased_on.insert(erased_entry(KEY, "https://a.example/"), EraseOutcome::Unfinished);
        assert!(!state.may_auto_connect(), "the auto-connect would sign up again on the erased server");
        state.server_url = "https://b.example".to_string();
        assert!(state.may_auto_connect(), "another server is still dialed");
        // A locked identity never dials, erased or not.
        state.private_key_bytes = None;
        assert!(!state.may_auto_connect());
    }
}

/// Review of BUG-135 option 2, second round, finding 3: the Server Settings page's working copy
/// is made only from the server's real settings, and follows them when they arrive.
#[cfg(all(test, feature = "native"))]
mod settings_draft_tests {
    use super::{draft_after_settings_arrive, page_draft};
    use crate::gui::GuiState;
    use crate::relay::storage::ServerSettings;

    fn real() -> ServerSettings {
        let mut s = ServerSettings::default();
        s.dm_mailbox_ttl_days = 365;
        s.message_retention_days = 90;
        s
    }

    /// Before the server's answer there is no working copy, so the page shows no form and
    /// nothing can be saved: Save (or the #local checkbox, which sends the whole copy) wrote
    /// every default back, and the relay's expiry pass then applied a lowered mailbox or
    /// retention window at once. Once the settings arrive the copy is made from them.
    ///
    /// Seen red 2026-10-04 with the page's old seeding (from the settings, or from defaults
    /// before they came): "assertion `left == right` failed: the page made its working copy from
    /// defaults before the server's settings arrived / left: Some(ServerSettings { ...,
    /// dm_mailbox_ttl_days: 30, message_retention_days: 0, ... }) / right: None".
    #[test]
    fn the_page_never_edits_defaults_as_if_they_were_the_servers_settings() {
        assert_eq!(page_draft(None, None), None, "the page made its working copy from defaults before the server's settings arrived");
        assert_eq!(page_draft(Some(&ServerSettings::default()), None), None, "a working copy with no settings under it was shown");
        assert_eq!(page_draft(None, Some(&real())), Some(real()), "the working copy was not made from the server's settings");
        let mut edited = real();
        edited.server_name = "Mine".into();
        assert_eq!(page_draft(Some(&edited), Some(&real())), Some(edited), "unsaved edits were dropped");
    }

    /// When the settings arrive: a working copy with no unsaved edits (it equals the settings
    /// it was made from) follows them, one with edits is kept, one with nothing real under it
    /// is replaced, and none stays none (the page makes it when it draws).
    ///
    /// Seen red 2026-10-04 with nothing touching the copy when settings arrive (as on
    /// 8695b08d4): "assertion `left == right` failed: a working copy with no unsaved edits kept
    /// the old settings / left: Some(ServerSettings { ..., dm_mailbox_ttl_days: 30, ... }) /
    /// right: Some(ServerSettings { ..., dm_mailbox_ttl_days: 365, ... })".
    #[test]
    fn the_working_copy_follows_the_servers_settings_unless_it_holds_edits() {
        let older = ServerSettings::default();
        assert_eq!(draft_after_settings_arrive(Some(older.clone()), Some(&older), &real()), Some(real()), "a working copy with no unsaved edits kept the old settings");
        let mut edited = older.clone();
        edited.server_name = "Mine".into();
        assert_eq!(draft_after_settings_arrive(Some(edited.clone()), Some(&older), &real()), Some(edited), "unsaved edits were overwritten");
        assert_eq!(draft_after_settings_arrive(Some(older.clone()), None, &real()), Some(real()), "a working copy with nothing real under it was kept");
        assert_eq!(draft_after_settings_arrive(None, Some(&older), &real()), None);

        // Through the state, as the frame does it: a copy made before a Save, the relay's
        // broadcast of the saved settings, and the page then shows them, not the old ones.
        let mut state = GuiState::default();
        state.server_settings = Some(older.clone());
        state.server_settings_draft = Some(older.clone());
        state.on_server_settings_state(&serde_json::json!({ "type": "server_settings_state", "settings": real() }));
        assert_eq!(state.server_settings_draft, Some(real()), "after the settings arrived the page still showed the old ones as unsaved changes");
    }

    /// Final review of 085441749, findings 1 and 2: the settings were asked for only while
    /// unknown and not yet asked, and the "asked" mark was cleared only when the socket dropped
    /// by itself or the server changed. Disconnect and Connect cleared nothing, so an answer that
    /// was lost (a slow link) or did not parse left the page waiting for the whole session; and
    /// after any reconnect the settings were "known", so a change another admin saved while this
    /// app was offline was never seen, and Save, which sends the whole copy, put the old values
    /// back. Now every newly signed-in socket asks, known or not, and Disconnect clears the mark.
    ///
    /// Seen red 2026-10-04 with the old rules: "a newly signed-in socket did not ask for the
    /// settings because an earlier socket had them".
    #[test]
    fn every_new_sign_in_asks_for_the_settings_known_or_not() {
        // Known from an earlier socket, asked there; a new socket signs in.
        let mut state = GuiState::default();
        state.server_settings = Some(real());
        state.server_settings_requested = true;
        state.ws_identified = false;
        assert!(state.socket_signed_in(), "the first sign-in on a socket was not reported as first");
        assert!(state.server_settings_wanted(), "a newly signed-in socket did not ask for the settings because an earlier socket had them");
        // Once asked on this socket, not again every frame; a later peer_list is not a new sign-in.
        state.server_settings_requested = true;
        assert!(!state.socket_signed_in());
        assert!(!state.server_settings_wanted(), "asked twice on one socket");
    }

    /// The same review, finding 1: Disconnect (`disconnect_active`) sets `ws_client` to None, so
    /// the self-dropped-socket path that cleared the "asked" mark never ran. Cleared here too.
    ///
    /// Seen red 2026-10-04: "Disconnect left the request marked as made, so the page could wait
    /// for the whole session".
    #[test]
    fn disconnect_clears_the_settings_request_mark() {
        // Asked, the answer lost, then Disconnect: Connect's socket asks again.
        let mut state = GuiState::default();
        state.ws_identified = true;
        state.server_settings_requested = true;
        state.disconnect_active();
        assert!(!state.server_settings_requested, "Disconnect left the request marked as made, so the page could wait for the whole session");
    }

    /// Final review of 085441749, finding 2, second half: when the server's settings arrive and
    /// the page's copy holds unsaved edits, the copy is kept (the edits are the admin's), and the
    /// page says the settings changed under it, because Save sends the whole copy and would put
    /// the old values back over the other change. Not said when the settings that arrive are the
    /// ones the copy was made from (a reconnect's answer), and gone with the copy.
    ///
    /// Seen red 2026-10-04 with nothing saying so: "the page did not say the server's settings
    /// changed under unsaved edits".
    #[test]
    fn the_page_says_when_the_servers_settings_change_under_unsaved_edits() {
        use super::changed_under_edits as said;
        let older = ServerSettings::default();
        let mut edited = older.clone();
        edited.server_name = "Mine".into();
        assert!(said(Some(&edited), Some(&older), &real(), false), "the page did not say the server's settings changed under unsaved edits");
        assert!(!said(Some(&edited), Some(&older), &older, false), "said for settings that did not change (a reconnect's answer)");
        assert!(said(Some(&edited), Some(&older), &older, true), "the note went away while the edits it is about are still there");
        assert!(!said(Some(&older), Some(&older), &real(), false), "said for a copy with no edits, which follows the settings");
        assert!(!said(None, Some(&older), &real(), true), "said with no copy");
        assert!(!said(Some(&edited), None, &real(), false), "said with no settings under the copy");

        // Through the state: the edits kept, the note shown, and dropped with the copy.
        let mut state = GuiState::default();
        state.server_settings = Some(older.clone());
        state.server_settings_draft = Some(edited.clone());
        state.on_server_settings_state(&serde_json::json!({ "type": "server_settings_state", "settings": real() }));
        assert_eq!(state.server_settings_draft, Some(edited), "unsaved edits were overwritten");
        assert!(state.server_settings_changed_underneath, "the page did not say the server's settings changed under unsaved edits");
        state.discard_server_settings_draft();
        assert!(!state.server_settings_changed_underneath, "the note outlived the copy it was about");
    }
}
