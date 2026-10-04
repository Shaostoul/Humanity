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
        self.server_settings_draft = None;
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
    }

    /// The ACTIVE server confirmed it erased our account (relay `account_erased`; BUG-135):
    /// close the connection as Disconnect does and remember the server it was DIALED for (never
    /// the server field being edited), so nothing dials it again by itself. The caller saves
    /// the config, and takes the game out of the shared world (engine/account_erase.rs).
    pub fn account_erased_on_active(&mut self) {
        let dialed = if self.connected_server_url.trim().is_empty() {
            self.server_url.clone()
        } else {
            self.connected_server_url.clone()
        };
        self.disconnect_active();
        self.account_erased_on.insert(erased_entry(&self.profile_public_key, &dialed));
    }

    /// The same for a PARKED server: the person switched servers between sending the erase and
    /// its receipt, so it arrived on a background link (engine/bg_connections.rs). That link is
    /// closed and never redialed by itself.
    pub fn account_erased_on_parked(&mut self, ci: usize) {
        let Some(conn) = self.connections.get_mut(ci) else { return };
        if let Some(ref mut ws) = conn.ws {
            ws.disconnect();
        }
        conn.ws = None;
        conn.identified = false;
        conn.manually_disconnected = true;
        conn.status = "Disconnected".to_string();
        let entry = erased_entry(&self.profile_public_key, &conn.url);
        self.account_erased_on.insert(entry);
    }

    /// True when this identity erased its account on the server at `url`: the app does not
    /// dial it by itself, and the Chat page says what pressing Connect there does.
    pub fn account_erased_here(&self, url: &str) -> bool {
        self.account_erased_on.contains(&erased_entry(&self.profile_public_key, url))
    }

    /// The person pressed Connect for `url`: they chose to sign up there again.
    pub fn forget_account_erased(&mut self, url: &str) {
        let entry = erased_entry(&self.profile_public_key, url);
        self.account_erased_on.remove(&entry);
    }
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
    use super::{erased_entry, ERASED_CONNECT_NOTE};
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
        state.account_erased_on_active();
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
        state.account_erased_on_active();
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
        state.account_erased_on_parked(0);
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
    }
}
