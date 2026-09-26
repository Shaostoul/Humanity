//! egui GUI system for the native desktop client.
//!
//! Theme loaded from data/gui/theme.ron (hot-reloadable).
//! Widgets are reusable components. Pages compose widgets into screens.

#[cfg(feature = "native")]
pub mod theme;
#[cfg(feature = "native")]
pub mod widgets;
#[cfg(feature = "native")]
pub mod pages;
#[cfg(feature = "native")]
pub mod fonts;
#[cfg(feature = "native")]
pub mod glossary;
/// The one page dispatch table (in-world screens, rung 1): every plain tool
/// page's draw call, shared by the main UI and the world screens.
#[cfg(feature = "native")]
pub mod dispatch;
/// An egui page rendered into its own texture for a screen placed in the
/// 3D world (in-world screens, rung 1).
#[cfg(feature = "native")]
pub mod screen_surface;

/// Install every font fallback chain on an egui context. Called for the main
/// UI context at boot AND for every in-world screen surface context, so a
/// screen can never show tofu the main UI does not: the two contexts get
/// identical font stacks by going through one function. The chains
/// themselves (Hack for arrows and box drawing, the OS CJK and emoji faces)
/// are documented in `fonts.rs`.
#[cfg(feature = "native")]
pub fn install_fonts(ctx: &egui::Context) {
    fonts::install_font_fallbacks(ctx);
}

/// Location-aware rules ("Laws") data loader (v0.496). See pages/laws.rs.
pub mod laws;
/// Server-owner action registry (data/admin/ops_registry.json): what every
/// admin action does and where it lives (app / command / config / VPS shell).
pub mod ops_registry;
/// Every `data/` file the GUI reads, and the shapes it reads them into.
/// Glob-re-exported so pages keep the `crate::gui::load_library` /
/// `crate::gui::Place` spellings they have always used. See `gui/loaders.rs`.
mod loaders;
pub use loaders::*;
mod organize;
pub use organize::*;


// Headless UI snapshot tests (v0.495): render egui pages to PNGs for review +
// regression. Test-only; pulls in egui_kittest (a dev-dependency).
/// The value types the GUI passes around: one struct or enum per thing a page
/// shows (item slot, task, listing, chat message, channel, studio scene, ...)
/// plus the `from_relay_json` mappers that build them. Glob-re-exported so the
/// whole crate keeps the `crate::gui::ChatMessage` spelling.
/// See `gui/state_types.rs`.
mod state_types;
pub use state_types::*;

#[cfg(test)]
mod ui_snapshots;

/// Current engine version (read from Cargo.toml at compile time).
#[cfg(feature = "native")]
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Decide whether the UI-click sound should fire this frame (v0.1112).
///
/// The click sound must fire ONLY when egui reported a REAL widget activation -
/// a button, link, or toggle was actually clicked. The old gate keyed on
/// `egui_ctx.wants_pointer_input()`, which in egui 0.31 is true whenever the
/// pointer is merely OVER any egui area, so clicking empty panel background
/// played the click ("clicking on the screen makes a sound even if I don't
/// click anything"). `PlatformOutput::events` carries an `OutputEvent::Clicked`
/// (or `DoubleClicked`) only when a widget genuinely activated, so scanning it
/// is the precise gate. `ui_sounds_enabled` is the user's master switch for
/// interface sounds (Settings > Audio) - off means never play the click.
///
/// Kept as a small pure function so the decision is unit-testable without a
/// running egui context (a headless click-test is not possible).
#[cfg(feature = "native")]
pub fn ui_click_should_sound(
    events: &[egui::output::OutputEvent],
    ui_sounds_enabled: bool,
) -> bool {
    ui_sounds_enabled
        && events.iter().any(|e| {
            matches!(
                e,
                egui::output::OutputEvent::Clicked(_)
                    | egui::output::OutputEvent::DoubleClicked(_)
            )
        })
}

/// An external entry loaded from `data/external/catalog.json`: either free
/// software you install (`kind == "software"`) or a real-world help service
/// (`kind == "service"`). One catalog, one renderer, both clients (v0.1063).
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub description: String,
    pub url: String,
    /// Software only; services leave this empty.
    #[serde(default)]
    pub license: String,
    /// Software only; services leave this empty.
    #[serde(default)]
    pub platforms: Vec<String>,
    /// Optional download size hint (e.g. "~350MB"). Software only.
    #[serde(default)]
    pub size: String,
    /// Category name, populated during loading from the parent category.
    #[serde(skip)]
    pub category: String,
    /// Kind id from the parent category: "software" or "service".
    #[serde(skip)]
    pub kind: String,
}

/// A single donation address entry (for the dynamic addresses array).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct DonateAddress {
    /// Network display name, e.g. "Solana (SOL)", "Bitcoin (BTC)"
    pub network: String,
    /// Type: "address" or "url"
    pub addr_type: String,
    /// The address or URL value
    pub value: String,
    /// Human-readable label, e.g. "Send SOL or SPL tokens"
    pub label: String,
}

#[cfg(feature = "native")]
impl DonateAddress {
    /// Parse the server's `/api/server-info` `funding` object into address entries.
    /// Entries with an empty `value` are skipped (the shipped server-config.json
    /// leaves placeholder rows blank until the operator fills them in), so an
    /// unconfigured network never renders as a dead button on the Donate page.
    pub fn from_funding_json(funding: &serde_json::Value) -> Vec<Self> {
        let Some(addrs) = funding.get("addresses").and_then(|v| v.as_array()) else {
            return Vec::new();
        };
        addrs
            .iter()
            .filter_map(|a| {
                let value = a.get("value").and_then(|v| v.as_str()).unwrap_or("");
                if value.is_empty() {
                    return None;
                }
                Some(Self {
                    network: a.get("network").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    addr_type: a.get("type").and_then(|v| v.as_str()).unwrap_or("address").to_string(),
                    value: value.to_string(),
                    label: a.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .collect()
    }
}

#[cfg(feature = "native")]
impl GuiState {
    /// Land a finished connect-time /api/server-info fetch into the Donate page's
    /// server-funding fields. `from_url` is the trimmed server URL the fetch was
    /// sent to; a result from a server we are no longer connected to is DISCARDED
    /// (donation info is money-routing data -- showing server A's addresses while
    /// connected to server B sends a donor's money to the wrong operator). A
    /// matching result always REPLACES both fields, so a server whose funding is
    /// disabled/absent clears any previously-shown list instead of inheriting it.
    pub fn apply_server_funding(&mut self, from_url: &str, funding: Option<&serde_json::Value>) {
        if from_url != self.server_url.trim_end_matches('/') {
            return;
        }
        self.donate_funding_server = from_url.to_string();
        self.donate_addresses_server = funding.map(DonateAddress::from_funding_json).unwrap_or_default();
        self.donate_funding_goal = funding.and_then(|f| {
            f.get("goal_usd").and_then(|v| v.as_f64()).filter(|g| *g > 0.0).map(|goal| {
                let label = f.get("goal_label").and_then(|v| v.as_str()).unwrap_or("").to_string();
                (goal, label)
            })
        });
    }

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
        // Sealed-sender DMs: the local history store and fetch high-water
        // are per (identity, server) — drop them so the next server loads
        // its own store from disk and re-fetches its own mailbox.
        if let Some(store) = self.dm_store.take() {
            store.save();
        }
        self.dm_fetch_sent = false;
    }
}

#[cfg(all(test, feature = "native"))]
mod park_unpark_tests {
    use super::{ChatChannel, ChatDm, ChatMessage, GuiState};

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

#[cfg(all(test, feature = "native"))]
mod donate_funding_tests {
    use super::{DonateAddress, GuiState};

    #[test]
    fn funding_result_from_the_connected_server_lands_and_absent_funding_clears() {
        let mut state = GuiState::default();
        state.server_url = "https://united-humanity.us/".to_string();
        let funding = serde_json::json!({
            "goal_usd": 100000.0, "goal_label": "Full-time development",
            "addresses": [{"network": "GitHub Sponsors", "type": "url",
                           "value": "https://github.com/sponsors/Shaostoul", "label": "x"}]
        });
        state.apply_server_funding("https://united-humanity.us", Some(&funding));
        assert_eq!(state.donate_addresses_server.len(), 1);
        assert_eq!(state.donate_funding_goal, Some((100000.0, "Full-time development".to_string())));

        // Reconnect-to-a-server-without-funding: the SAME url reporting no funding
        // must CLEAR the fields, not leave the old list showing (the adversarial
        // review's major finding: stale addresses = money to the wrong operator).
        state.apply_server_funding("https://united-humanity.us", None);
        assert!(state.donate_addresses_server.is_empty(), "absent funding clears the list");
        assert!(state.donate_funding_goal.is_none(), "absent funding clears the goal");
    }

    #[test]
    fn late_result_from_a_previous_server_is_discarded() {
        let mut state = GuiState::default();
        state.server_url = "https://server-b.example".to_string();
        let funding_a = serde_json::json!({
            "addresses": [{"network": "Bitcoin (BTC)", "type": "address", "value": "bc1qA", "label": ""}]
        });
        // A stale fetch tagged with server A's url lands while connected to B.
        state.apply_server_funding("https://server-a.example", Some(&funding_a));
        assert!(state.donate_addresses_server.is_empty(), "server A's addresses must not display on server B");
        assert!(state.donate_funding_server.is_empty());
    }

    #[test]
    fn parses_filled_entries_and_skips_blank_placeholders() {
        // Mirrors the real shipped server-config.json shape: GitHub Sponsors is
        // filled in, the crypto rows are blank placeholders awaiting the operator.
        let funding = serde_json::json!({
            "enabled": true,
            "goal_usd": 100000,
            "addresses": [
                {"network": "GitHub Sponsors", "type": "url",
                 "value": "https://github.com/sponsors/Shaostoul", "label": "Recurring or one-time"},
                {"network": "Solana (SOL)", "type": "address", "value": "", "label": "Send SOL"},
                {"network": "Bitcoin (BTC)", "type": "address", "value": "", "label": "Send BTC"},
            ]
        });
        let parsed = DonateAddress::from_funding_json(&funding);
        assert_eq!(parsed.len(), 1, "blank-value placeholder rows are dropped");
        assert_eq!(parsed[0].network, "GitHub Sponsors");
        assert_eq!(parsed[0].addr_type, "url");
        assert_eq!(parsed[0].value, "https://github.com/sponsors/Shaostoul");
        assert_eq!(parsed[0].label, "Recurring or one-time");
    }

    #[test]
    fn missing_or_malformed_addresses_yield_empty_not_panic() {
        assert!(DonateAddress::from_funding_json(&serde_json::json!({"enabled": true})).is_empty());
        assert!(DonateAddress::from_funding_json(&serde_json::json!({"addresses": "oops"})).is_empty());
        assert!(DonateAddress::from_funding_json(&serde_json::json!(null)).is_empty());
    }

    #[test]
    fn missing_type_defaults_to_address() {
        let funding = serde_json::json!({"addresses": [{"network": "X", "value": "abc123"}]});
        let parsed = DonateAddress::from_funding_json(&funding);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].addr_type, "address");
        assert_eq!(parsed[0].label, "");
    }
}

/// What the passphrase / PIN prompt is for.
///
/// The first three (SetNew, Unlock, Change) gate the BIP39-derived passphrase
/// vault that has existed since the early native client. The PIN variants
/// (v0.278.0 auto-unlock) gate the short PIN that wraps a keychain-stored
/// device key. PIN and passphrase coexist: setting a PIN doesn't remove the
/// passphrase vault — it's an alternate, faster unlock that lives on top.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassphraseMode {
    /// Setting a new passphrase (first time or migration from plaintext).
    SetNew,
    /// Unlocking an existing encrypted key.
    Unlock,
    /// Changing the passphrase (requires old + new).
    Change,
    /// Setting a new PIN — requires the seed already in memory (called
    /// either right after a successful passphrase unlock OR on a freshly
    /// generated identity). Encrypts seed with PIN+device_key.
    PinSetup,
    /// Unlocking with the PIN. Device key already loaded from keychain.
    PinUnlock,
    /// Changing PIN — requires old PIN + new PIN. Re-encrypts the seed
    /// blob; the device_key in the keychain stays the same.
    PinChange,
}

/// Which page/overlay is currently active.
/// Server metadata fetched from a relay's GET /api/server-info, shown in the
/// launcher's server-detail pane (v0.478). A subset of the relay's
/// ServerInfoResponse; serde ignores fields we don't render.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub users_online: u64,
    /// Players currently in the shared GAME world (v0.776, co-presence): the
    /// count of in-world avatars, distinct from users_online (chat/WS peers).
    #[serde(default)]
    pub game_players: u64,
    #[serde(default)]
    pub member_count: u64,
    #[serde(default)]
    pub accord_compliant: bool,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub owner_key: String,
    /// Donation config, present only when the server has `funding.enabled: true`
    /// in its `server-config.json` (the relay omits the field entirely otherwise).
    /// Shape: `{"enabled":true,"goal_usd":...,"addresses":[{"network","type","value","label"}]}`.
    #[serde(default)]
    pub funding: Option<serde_json::Value>,
}

/// The WHERE half of a Play pairing (docs/design/play-characters.md): which
/// kind of world the picker has selected. The WHO half is a character, held
/// separately in `launcher_who`, so the two axes can be composed independently.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherWhere {
    /// A local, self-custodial home save. Playing one is SOLO.
    Home,
    /// An Open Net server world: you bring your own character, the server
    /// trusts what you bring. Playing one is SHARED.
    Server,
    /// A Closed Net server world: the server holds the character so progress
    /// cannot be forged. Arrives with multiplayer; no rows yet.
    ClosedNet,
}

/// One local save as the picker sees it: the WHO (character) and the WHERE
/// (home world) halves of the SAME file. `WorldSave` fuses them today
/// (src/persistence.rs), so the picker presents both axes from one row and
/// greys the pairings the fused file cannot honour yet.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct LauncherHome {
    /// The save/world display name (`WorldSave.name`). The WHERE id today.
    pub world: String,
    /// The character living in that save (`WorldSave.character_name`).
    pub character: String,
    /// Home design id (fibonacci, etc.) for the inline summary.
    pub design: String,
    /// Save timestamp (unix seconds), shown as "last played".
    pub timestamp: u64,
}

#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPage {
    /// In-game, no menu overlay (HUD still visible).
    None,
    /// Title screen: Play, Settings, Quit.
    MainMenu,
    // ── Tool pages ──
    Settings,
    Inventory,
    Tasks,
    Maps,
    Market,
    Profile,
    // v0.1147: GuiPage::Civilization DELETED (operator call 2026-08-17,
    // nav-map open item 7): the standalone "Community Dashboard" page was
    // reachable only through one onboarding link, while the Humanity tab's
    // Mission Dashboard (built FROM civilization.rs's helpers) is the real
    // surface. The module stays; only the page variant died.
    Chat,
    Calculator,
    Notes,
    Calendar,
    Crafting,
    Wallet,
    Guilds,
    Trade,
    Files,
    BugReport,
    // v0.415.0: removed the Resources and Onboarding GuiPage variants. The
    // Resources directory was retired into the Library (v0.374-375) and the
    // standalone onboarding page into the Mission Dashboard + Quests page
    // (v0.373); first boot now lands on Humanity. Saved config strings
    // ("resources" / "onboarding") migrate in config_str_to_page below.
    Library,
    Donate,
    Tools,
    Studio,
    /// Watch live streams inside the app (v0.857). The viewer half of Studio:
    /// lists what is live from /api/live and plays a selected MJPEG stream.
    /// Mirrors the web `/watch` page.
    Watch,
    Quests,
    /// Server / group administration settings page. Opened from the cog
    /// menu on the server or group row in the chat sidebar.
    ServerSettings,
    // v0.479: GuiPage::GameAdmin removed. Game-world bans folded into a
    // subsection of Server Settings > ADMIN (game_admin::draw_section), so the
    // nav has one fewer button. The two ban systems stay structurally separate
    // (disjoint tables + a distinct subsection with the free-speech disclaimer).
    /// Identity hub: DID, Verifiable Credentials, trust score, AI status.
    /// Mirrors the web `/identity` page.
    Identity,
    /// Local + civilization-scope governance: proposals, votes, tally.
    /// Mirrors the web `/governance` page.
    Governance,
    /// Location-aware rules + rights, nested Humanity -> locality (v0.496).
    Laws,
    /// Social key recovery setup + active recovery requests.
    /// Mirrors the web `/recovery` page.
    Recovery,
    // v0.197.0: removed Agents and AiUsage GuiPage variants. Operator
    // 2026-05-08: "That AI Agents page also seems useless. As well as
    // the AI usage." Multi-AI orchestration is handled via
    // data/coordination/* + the relay agent_sessions table — the UI
    // pages weren't pulling their weight. The page modules + state
    // fields were removed in the same release.
    // v0.1145: GuiPage::Cosmos DELETED (merged into Maps). Both variants
    // rendered the identical cosmos::draw page since v0.203.2, so nav
    // highlight and back-stack behaved differently by entry path. "Maps"
    // is the one name (nav map open item 1, operator call 2026-08-16);
    // "cosmos" persists only as the legacy config string parsed to Maps.
    /// QA testing tasks — operator-facing checklist of features to manually verify.
    /// Each task has Mark Passed / Report Issue buttons that post results to chat.
    Testing,
    /// The Browser page: the websites database (`data/web/sites.json`) as
    /// site cards, and, when the readable-web opt-in is on, the in-app web
    /// view that reads a site's pages without a browser engine or
    /// JavaScript (docs/design/readable-web.md). Opt-in off = cards open
    /// the OS browser.
    Browser,
    /// Real — the merged "your actual life" tab (v0.358): one page with a
    /// section_nav sidebar folding in Profile's sections + Inventory, Wallet,
    /// Tasks, Map, Market. Replaces six separate nav buttons. See pages/real.rs.
    Real,
    // v0.415.0: removed the Play GuiPage variant (the v0.360 tab that folded
    // Crafting + Studio). Both are top-level tabs now and nothing navigated to
    // it; the nav's Play button is GuiPage::None (FPS mode), unrelated.
    /// Platform — the software-itself tab (v0.360): section_nav folds Settings,
    /// Recovery, Tools, Bugs, Testing, Browser.
    Platform,
    /// Humanity — the collective / mission tab (v0.360): section_nav folds the
    /// Community/Mission Dashboard (Civilization) + Governance, Directory
    /// (Identity), Donate. What the H button opens, and the first-boot landing.
    Humanity,
    /// Home — your offline homestead (v0.379): the Fibonacci homestead Design
    /// browsed as rooms + bill-of-materials + power/water demand + a self-
    /// sufficiency summary. The "homes as save profiles" surface, offline-first
    /// (server/real homes come later). See pages/homes.rs + homes-as-profiles.md.
    Homes,
    /// Relays — the Relay Control Center (v0.846): one main-menu entry to manage
    /// every relay the operator owns from one PC. Left rail lists the operator's
    /// relays; the detail has Health / Control / Config tabs. Elevates the
    /// Server Settings ops panels into a top-level, multi-relay surface. See
    /// docs/design/in-app-ops.md "Relay Control Center".
    RelayControl,
    // v0.699.0: removed the two-tier-nav-era category-browse subsystem --
    // the 5 Overview* category landing pages (OverviewReality/Sim/Tools/
    // Settings/Dev) and the 12 Settings* sub-page variants (SettingsAccount
    // .. SettingsUpdates). All 17 were dead since the v0.196 single-row nav
    // rewrite: nothing navigated to an Overview page, and the Settings*
    // variants were only reachable as cards on the (unreachable)
    // OverviewSettings. Settings content lives in `settings.rs` (its own
    // internal SettingsCategory router, still fully reachable via the
    // top-level Settings tab); the stranded working pages Calculator/Files
    // (now in Platform) and Trade/Guilds (now in Real) were rehomed in the
    // same release. See the 2026-07-04 page-access audit.
}

/// Pages that can be selected as the startup boot page.
#[cfg(feature = "native")]
pub const BOOT_PAGE_OPTIONS: &[(GuiPage, &str)] = &[
    (GuiPage::Humanity, "Humanity (Mission Dashboard)"),
    (GuiPage::Chat, "Chat"),
    (GuiPage::Tasks, "Tasks"),
    // NOTE: GuiPage::Cosmos was listed here too ("Cosmos") until v0.1144;
    // Maps and Cosmos render the IDENTICAL page, so the dropdown offered the
    // same destination under two names.
    (GuiPage::Maps, "Maps"),
    (GuiPage::Notes, "Notes"),
    (GuiPage::Calendar, "Calendar"),
    (GuiPage::Library, "Library"),
];

#[cfg(feature = "native")]
pub fn page_to_config_str(page: GuiPage) -> &'static str {
    match page {
        GuiPage::Humanity => "humanity",
        GuiPage::Chat => "chat",
        GuiPage::Tasks => "tasks",
        GuiPage::Maps => "maps",
        GuiPage::Notes => "notes",
        GuiPage::Calendar => "calendar",
        GuiPage::Library => "library",
        _ => "humanity",
    }
}

#[cfg(feature = "native")]
pub fn config_str_to_page(s: &str) -> GuiPage {
    match s {
        "humanity" => GuiPage::Humanity,
        "chat" => GuiPage::Chat,
        "tasks" => GuiPage::Tasks,
        "maps" => GuiPage::Maps,
        "notes" => GuiPage::Notes,
        "calendar" => GuiPage::Calendar,
        // Legacy config value: the Cosmos variant merged into Maps (v0.1145).
        "cosmos" => GuiPage::Maps,
        "library" => GuiPage::Library,
        // Retired pages saved in old configs land on their successors:
        // the Resources directory lives in the Library; the onboarding
        // landing is the Mission Dashboard (also the unknown-id default).
        "resources" => GuiPage::Library,
        _ => GuiPage::Humanity,
    }
}

/// Tracks all GUI state for the native app.
#[cfg(feature = "native")]
pub struct GuiState {
    pub active_page: GuiPage,
    /// Last page visited before returning to game view. Escape reopens this page.
    pub last_page: GuiPage,
    /// Navigation back-stack. When a page opens a sub-page (e.g. clicking
    /// the cog on a server row opens ServerSettings from Chat), the source
    /// page is pushed here so Escape returns to it instead of jumping
    /// straight to FPS mode. Operator 2026-05-08: "if I'm in nested pages
    /// like that esc needs to reliably take people back to the previous
    /// menu/page". Use `push_nav_to` / `pop_nav_back` helpers below.
    /// Direct nav-bar clicks DO NOT push (they replace the current page,
    /// not nest under it).
    pub nav_back_stack: Vec<GuiPage>,
    /// In-world interactive chat panel open (v0.772): Enter opens it, which
    /// frees the cursor + disables look/move so you can read + type in the
    /// same relay chat as the Chat page/website without leaving the 3D world.
    pub chat_input_active: bool,
    /// Set true when chat_input_active just opened so the panel requests
    /// keyboard focus for its input exactly once (re-focusing every frame
    /// would steal focus from the channel buttons). (v0.772)
    pub chat_input_focus_pending: bool,
    /// Which view the in-world chat panel shows (increment 1c): Channels /
    /// Dms / Groups / Options. Session-persistent -- Esc-close keeps it, so
    /// the panel reopens where you left off; see `IngameChatMode`.
    pub ingame_chat_mode: IngameChatMode,
    /// Show the passive bottom-left chat feed while playing (hud.rs). Toggled
    /// from the in-world panel's Options tab; persisted in AppConfig so the
    /// preference survives restarts (GUI-first configurability).
    pub hud_chat_feed_visible: bool,
    /// Max height (px) of the in-world chat panel's message list. Adjustable
    /// from the Options tab (slider); persisted in AppConfig.
    pub ingame_chat_panel_height: f32,
    /// Shared-world co-presence status (v0.774), mirrored from the ECS each
    /// frame by the multiplayer block in lib.rs so the paint-only HUD can show
    /// it. `copresence_active` = we've joined the relay's shared game world
    /// (in-world + connected). `copresence_names` = the OTHER players currently
    /// present (RemotePlayer entities). Makes the mission-critical co-presence
    /// visible: without this you can't tell you're in a shared world, or see
    /// when someone else joins.
    pub copresence_active: bool,
    pub copresence_names: Vec<String>,
    /// Dev spawn tool (v0.777, Platform > Dev): when Some(def_id), lib.rs spawns
    /// that creature/NPC in front of the player next frame and clears it. The
    /// generic "spawn any creature/NPC" the operator asked for; species come
    /// from CreatureRegistry (data/creatures.csv, 92 rows).
    pub pending_dev_spawn: Option<String>,
    /// Dev spawn tool: despawn every Creature in the world next frame (cleanup).
    pub pending_dev_despawn_creatures: bool,
    /// Search box text on the Dev page's species list.
    pub dev_spawn_filter: String,
    /// Live count of Creature entities, mirrored from the ECS each frame so the
    /// Dev page can show "N creatures in the world" without a world handle.
    pub dev_creature_count: usize,
    /// Planet Tuner page state: live readout mirror + the PlanetDef editing
    /// copy (see pages::planet_tuner; mirrored by frame_lock each frame).
    pub planet_tuner: pages::planet_tuner::PlanetTunerState,
    /// Walk-up creature editor (v0.778): the entity (as bits) being edited, or
    /// None when the editor is closed. Set by pressing G while facing a creature
    /// (dev/cheats on). While Some, the editor panel is open, the cursor is
    /// freed, and look/move are disabled (same plumbing as the chat panel). The
    /// edit-buffer fields below are snapshotted from the creature on open and
    /// written back live each frame by lib.rs (edit-buffer-then-sync).
    pub dev_edit_target: Option<u64>,
    /// One-shot: fill the edit buffers from the target's components next frame
    /// (set on open; lib.rs clears it after the snapshot).
    pub dev_edit_snapshot_pending: bool,
    pub dev_edit_name: String,
    pub dev_edit_health: f32,
    pub dev_edit_health_max: f32,
    /// True = hostile. Toggling ON maps to AIBehavior "predator" (the only
    /// behavior that hunts the PLAYER without a Faction component -- plain
    /// "aggressive" targets by faction and nothing attaches factions, so it
    /// never attacks anything). Toggling OFF maps to "passive". (v0.779)
    pub dev_edit_hostile: bool,
    /// The creature's ORIGINAL behavior_type at editor-open ("" = it had no
    /// AIBehavior at all). Lets the write-back leave the AI untouched unless
    /// the user actually flips the Hostile toggle -- without this, merely
    /// OPENING the editor lossily collapsed predator/guard to "aggressive"
    /// and force-attached AIBehavior to placed farm animals (breaking their
    /// anchored grazing). (v0.779)
    pub dev_edit_behavior_orig: String,
    pub dev_edit_tint: [f32; 3],
    /// Body-box side length (metres); Creature.body_side.
    pub dev_edit_scale: f32,
    /// Read-only species id label for the editor header.
    pub dev_edit_species: String,
    /// Despawn the edited creature next frame (from the editor's Despawn button).
    pub pending_dev_edit_despawn: bool,
    /// Dev travel (v0.791.x, Platform > Dev > Travel): when Some(body_id),
    /// lib.rs teleports the viewpoint next frame -- it moves ship_world_pos
    /// (f64 Earth-centred metres; planets are up to 4.5e12 m away, far past
    /// f32 camera coordinates) to a sunlit vantage ~4 body radii out, aims the
    /// camera, and turns fly mode on. The special id "home" restores the
    /// stashed pre-teleport position. Offline/local world only (lib.rs +
    /// the Dev page both gate on copresence_active).
    pub pending_dev_teleport: Option<String>,
    /// Dev travel SURFACE landing (2026-07-12): when Some(body_id), lib.rs drops
    /// the viewpoint to LOW altitude over a surface point of that body and
    /// engages surface mode (radial up, gravity settles to standing height, the
    /// planet held STILL), instead of the 4-radii orbit `pending_dev_teleport`
    /// gives. Reuses the same lat/lon surface path as debug/camera_request.json,
    /// so the GUI "Land" matches the AI dev-tool. Only offered for bodies with a
    /// real heightmap.
    pub pending_dev_surface: Option<String>,
    /// Dev fly mode (free flight: no gravity, no wall collision). Synced into
    /// CameraController.fly_mode each frame; forced off when cheats are off or
    /// a shared world is joined.
    pub dev_fly_mode: bool,
    /// TRUE HOVER: no gravity, no ground clamp, altitude held (F9, v0.1109.2).
    ///
    /// Deliberately NOT `dev_fly_mode`. That flag is also set by every dev
    /// TELEPORT - camera bookmarks, the showcase IPC, the probe rig's autopilot
    /// - which means reusing it made a teleport imply noclip, and the camera
    /// came to rest below the terrain looking at the Milky Way through it. Dev
    /// travel wants free 3D movement; hovering wants gravity switched off.
    /// Those are two different requests and now they are two different bits.
    pub dev_hover: bool,
    /// Dev fly speed multiplier, 1.0..=1e9, exponential steps. <=1000x moves
    /// the local camera; above that lib.rs moves ship_world_pos (FTL - the
    /// ship flies, carrying the player). Mouse wheel adjusts it while flying.
    pub dev_fly_speed_mult: f32,
    /// True while a pre-teleport home stash exists in lib.rs (the Dev page
    /// shows "Return home" enabled and the HUD notes you are away).
    pub dev_travel_away: bool,
    pub show_hud: bool,
    /// "Link a device" QR (Settings > Account, v0.837): when true, the account
    /// panel renders a scannable QR of this identity's backup JSON so a phone can
    /// adopt it. Only ever shown behind the passphrase-gated seed reveal (it
    /// encodes the seed). Transient (not persisted).
    pub link_device_qr_show: bool,
    /// Cached QR texture keyed by the payload it was built for, so we build it
    /// once (not every frame) while shown and rebuild only if the identity
    /// changes. Transient; GuiState is not serialized.
    pub link_device_qr: Option<(String, egui::TextureHandle)>,
    pub settings: SettingsState,
    pub chat_input: String,
    /// v0.282.0: peers currently typing in the active channel. Keyed by
    /// sender pubkey to avoid duplicate "X is typing…" rows when a peer
    /// emits multiple rate-limited typing events. Value = (display_name,
    /// when_received) — the renderer prunes entries older than 3 seconds
    /// to match the web client's auto-clear behavior.
    pub chat_typing_users: std::collections::HashMap<String, (String, std::time::Instant)>,
    /// v0.282.0: last time WE sent a typing indicator. Sites that want
    /// to emit typing should consult this for the 3-second rate limit
    /// rather than sending on every keystroke (matches the relay's
    /// `TYPING_RATE_LIMIT_SECS` so we never get silently dropped).
    pub chat_typing_last_sent: Option<std::time::Instant>,
    /// When the user clicks "Reply" on a message, this holds the parent context.
    /// Cleared on send or cancel. Drives the "Replying to ... [X]" banner above the input.
    pub chat_reply_to: Option<ReplyContext>,
    pub chat_messages: Vec<ChatMessage>,
    /// Whether the message search modal is open.
    pub chat_search_open: bool,
    /// Live search input text.
    pub chat_search_query: String,
    /// Most recent search results (cleared when the modal closes).
    pub chat_search_results: Vec<ChatSearchResult>,
    /// Pins per channel (id → list of pinned messages).
    /// Populated by `pins_sync` and `pin_added` server messages.
    pub chat_pins: std::collections::HashMap<String, Vec<ChatPin>>,
    /// Whether the pins modal is open.
    pub chat_pins_open: bool,
    /// `(timestamp_ms, draft_content)` for the message currently being edited.
    /// None = no edit in progress.
    pub chat_edit_target: Option<(u64, String)>,
    /// Timestamps of messages sent from THIS client (for dedup on echo).
    pub chat_sent_timestamps: Vec<u64>,
    pub chat_channels: Vec<ChatChannel>,
    pub chat_active_channel: String,
    pub chat_users: Vec<ChatUser>,
    pub chat_dms: Vec<ChatDm>,
    pub chat_servers: Vec<ChatServer>,
    pub chat_friends: Vec<ChatUser>,
    /// Keys of everyone I follow — RAW from the relay's follow_list, so it
    /// includes offline people that `chat_friends` (filtered against the
    /// online `chat_users`) misses. Drives the follow-direction badges. (v0.721)
    pub chat_following_keys: std::collections::HashSet<String>,
    /// Keys of everyone who follows ME (follow_list.followers + live
    /// follow_update). The relay always sent this; native dropped it until
    /// v0.721 — which is why the "Follows you" badges disappeared.
    pub chat_followers: std::collections::HashSet<String>,
    pub ws_client: Option<crate::net::ws_client::WsClient>,
    pub ws_status: String,
    /// Parked (background) relay connections, sockets still live. The ACTIVE
    /// connection is never in this list -- it lives in the legacy ws_* /
    /// chat_* fields; see ServerConnection for the model.
    pub connections: Vec<ServerConnection>,
    /// Native WebRTC DataChannel P2P manager handle (increment 1). Lazily
    /// started after the WS connect/identify so we have our pubkey hex. `None`
    /// until then. Native-only — relay/wasm builds don't open peer channels.
    #[cfg(feature = "native")]
    pub webrtc: Option<crate::net::webrtc::WebrtcHandle>,
    /// DEV: the peer pubkey hex the chat page armed a P2P self-test for. When
    /// the channel to this peer opens, `lib.rs` auto-sends "native p2p test".
    /// Cleared is fine to leave; it just gates the one-shot test send. inc-1
    /// transport proof only — not user-facing UI.
    #[cfg(feature = "native")]
    pub webrtc_test_peer: Option<String>,
    /// Whether the user manually disconnected (suppresses auto-reconnect).
    pub ws_manually_disconnected: bool,
    /// Countdown to next reconnect attempt (seconds).
    pub ws_reconnect_timer: f32,
    /// Current reconnect delay with exponential backoff (seconds).
    pub ws_reconnect_delay: f32,
    /// Number of consecutive failed reconnect attempts.
    pub ws_reconnect_attempts: u32,
    /// True after the relay sent "Too many connection attempts" (v0.544). Holds the 65s back-off in
    /// place (the connection OPENS before the throttled identify, so the on-connect backoff reset
    /// would otherwise clobber it and loop). Cleared when the next retry actually fires.
    pub ws_rate_limited: bool,
    pub selected_slot: Option<usize>,
    /// Garden selection in the inventory left tree: "crop:<entity_bits>" or
    /// "tower:<id>". Drives the right detail panel for garden objects; mutually
    /// exclusive with selected_slot (selecting one clears the other).
    pub garden_selection: Option<String>,
    /// Start the inventory + garden trees COLLAPSED (operator 2026-06-08: "so when
    /// I first load the lists will start collapsed instead of expanded"). Default
    /// true; toggled by the "Start collapsed" checkbox. The Collapse/Expand-all
    /// buttons force every branch for the current frame.
    pub trees_start_collapsed: bool,
    pub fps: f32,
    pub updater: crate::updater::Updater,
    /// Set true when an update notification toast should show.
    pub update_toast_visible: bool,

    // ── Onboarding state ──

    /// Whether the user has completed first-run onboarding.
    pub onboarding_complete: bool,
    /// Current onboarding step (0 = welcome, 1 = server connect, 2 = identity, 3 = done).
    pub onboarding_step: u8,
    /// Server URL input field.
    pub server_url: String,
    /// The URL the CURRENT ws_client was actually dialed for. server_url
    /// doubles as the editable input draft, so it can drift mid-session
    /// (user typing a new address while still connected); parking and
    /// provenance must key by what we truly connected to, never the draft.
    /// Set at every connect site; empty when never connected.
    pub connected_server_url: String,
    /// Whether currently connected to a server.
    pub server_connected: bool,
    /// Onboarding step 1's "Connect" button (v0.643) used to just set
    /// `server_connected = true` unconditionally with no real check -- the
    /// full WS identify handshake genuinely can't happen yet at this step
    /// (the auto-connect gate in src/lib.rs requires `onboarding_complete`,
    /// and identity/pubkey isn't created until step 2), so this is instead a
    /// lightweight `/health` reachability probe, spawned on a background
    /// thread (mirrors src/updater.rs's `check_now` mpsc pattern) so the UI
    /// thread never blocks on the network. `None` = idle, `Some(rx)` =
    /// checking (poll it once per frame in `draw_step_server`).
    #[allow(clippy::type_complexity)]
    pub server_check_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    /// Chat history fetch result: (channel, body-or-error). Fetched on a
    /// background thread with short timeouts because the old inline ureq
    /// call had NO timeout and ran on the render thread: with the relay
    /// unreachable it stalled every frame loop ~21 s per reconnect cycle
    /// (the 2026-08-13 "app froze in chat" report).
    pub history_rx: Option<std::sync::mpsc::Receiver<(String, Result<String, String>)>>,
    /// Human-readable error from the last failed reachability check (empty
    /// if the last check succeeded or none has run yet).
    pub server_check_error: String,
    /// User display name input.
    pub user_name: String,
    // v0.197.0: removed `context_real`. Real/Sim toggle deleted —
    // pages commit to Real, game-mode equivalents live inside the
    // game loop (FPS) rather than as toggleable views.
    /// Whether the first-run concept tour (Onboarding page) has been
    /// completed. v0.198.0: new users land on Onboarding after identity
    /// setup so they understand what HumanityOS IS before being dropped
    /// into chat. Once they click "Open the Chat" from Onboarding
    /// (or skip it explicitly) this flips true and they go straight
    /// to Chat on subsequent launches. Existing users created before
    /// this field defaults to true via the AppConfig migration so they
    /// don't get force-routed into the tour they've never seen but
    /// don't need.
    pub concept_tour_seen: bool,
    /// Default page to load after onboarding (Chat by default).
    pub default_page: GuiPage,

    // ── Task board state ──
    pub tasks: Vec<GuiTask>,
    pub task_next_id: u32,
    pub task_search: String,
    pub task_filter_priority: Option<TaskPriority>,
    pub task_filter_assignee: String,
    pub task_show_new_form: bool,
    pub task_new_title: String,
    pub task_new_description: String,
    pub task_new_priority: TaskPriority,
    pub task_new_assignee: String,

    // ── Profile state ──
    pub profile_name: String,
    pub profile_bio: String,
    pub profile_public_key: String,
    pub profile_section: ProfileSection,
    // Body & Measurements
    pub profile_height: String,
    pub profile_weight: String,
    pub profile_eye_color: String,
    pub profile_blood_type: String,
    pub profile_hair_color: String,
    pub profile_hair_length: String,
    pub profile_hair_style: String,
    pub profile_hair_texture: String,
    pub profile_neck: String,
    pub profile_shoulders: String,
    pub profile_chest: String,
    pub profile_waist: String,
    pub profile_hips: String,
    pub profile_thighs: String,
    pub profile_inseam: String,
    pub profile_shoe_size: String,
    pub profile_shirt_size: String,
    pub profile_pants_size: String,
    // Identity
    pub profile_pronouns: String,
    pub profile_location: String,
    pub profile_website: String,
    // Private Notes
    pub profile_private_notes: String,
    // Network Profile
    pub profile_network_name: String,
    pub profile_network_bio: String,
    pub profile_network_avatar: String,
    /// Whether to appear in the server's public member directory (audit 2026-06-12
    /// opt-out). On = listed; off sends profile privacy `directory:"unlisted"`.
    /// Defaults to listed; native does not fetch server privacy, so it reflects the
    /// session's intent rather than the stored server state.
    pub profile_directory_listed: bool,
    /// Floating machine labels in the 3D home (v0.428), populated by load_world.
    pub machine_labels: Vec<MachineLabel>,
    /// Sight-blocking wall segments for the HUD (v0.975), packed [ax, az, bx, bz] in the
    /// same world frame the labels project in (station_off applied). Static sight spans
    /// (windows open) + live CLOSED doors, rebuilt per frame by lib.rs. Empty = no
    /// occlusion (legacy layout, build editor, or not near the home).
    pub sight_blockers: Vec<[f32; 4]>,
    /// Index into machine_labels of the machine the player is currently looking at
    /// within interact range (walk-up interaction, v0.431). Recomputed each frame.
    pub targeted_machine: Option<usize>,
    /// Crosshair prompt for the vehicle the player faces ("[E] drive Rover") or
    /// "[E] exit vehicle" while driving. Precomputed in lib.rs like the door prompt.
    pub vehicle_prompt: String,
    /// Crosshair prompt for the farm animal the player faces (v0.751):
    /// "[E] collect Egg (Chicken)" when ready, the regrow countdown otherwise.
    pub livestock_prompt: String,
    /// Transient collect feedback ("+1 Egg from Chicken" / "Your pack is full").
    pub livestock_notice: String,
    /// game_time when livestock_notice was set; lib.rs clears it after 3 s.
    pub livestock_notice_at: f64,
    /// Index into `door_panels` of the door CONTROL PANEL the player is looking at within
    /// arm's reach (v0.567). Drives the "[E] open/close door" prompt; E toggles the door.
    /// Recomputed each frame in first person. Mirrors `targeted_machine`.
    pub targeted_control_panel: Option<usize>,
    /// The crosshair prompt for the targeted control panel (v0.567), precomputed each frame
    /// (the HUD can't see the door's open/locked state, which lives in EngineState). Empty = none.
    pub control_panel_prompt: String,
    /// Index of the machine whose card is pinned open (toggled with E). Stays until E
    /// again or it is cleared. Survives walking away (it is the "opened station").
    pub selected_machine: Option<usize>,
    /// Pinned machine's current auto-recipe id (None when the selected machine
    /// has no AutoRefine). Published per frame by lib.rs. (v0.725)
    pub machine_card_recipe: Option<String>,
    /// Selectable same-station recipes for the pinned machine: (id, display
    /// name), sorted by name. Infinite-of-X: rows come from recipes.csv via
    /// the registry, never hardcoded. Empty when no selector applies. (v0.725)
    pub machine_card_recipe_options: Vec<(String, String)>,
    /// The player's pick from the card's recipe dropdown; lib.rs applies it to
    /// the machine entity's AutoRefine next frame and clears it. (v0.725)
    pub machine_card_recipe_pending: Option<String>,
    /// Pinned machine's container contents (item id, display name, qty) when
    /// it holds something — drives the card panel's "Take" action. (v0.731)
    pub machine_card_container: Option<(String, String, u32)>,
    /// Set by the card panel's Take button; lib.rs moves as much as fits
    /// (volume-gated) from the machine's Container into the backpack. (v0.731)
    pub machine_card_take_pending: bool,
    /// Pack items the pinned machine's container ACCEPTS (class-compatible +
    /// no-mixing respected): (item id, display name, qty carried). Drives the
    /// per-item "Store" buttons — the deposit path that completes the
    /// refinery -> pack -> genset-drum fuel handoff. (v0.733)
    pub machine_card_storable: Vec<(String, String, u32)>,
    /// Item id the player chose to Store; lib.rs moves as much as fits from
    /// the backpack into the machine's Container next frame. (v0.733)
    pub machine_card_store_pending: Option<String>,
    /// True when the pinned machine is a trading post (v0.747, ladder rung 3):
    /// the card shows a Trade button that opens the vendor modal.
    pub machine_card_vendor: bool,
    /// Vendor modal open (v0.747). Opened from the trading post's card.
    pub vendor_open: bool,
    /// The vendor's catalog: goods that exist in BOTH trade_goods.ron and
    /// items.csv, with the player-facing prices (pay / receive).
    pub vendor_goods: Vec<GuiTradeGood>,
    /// Last vendor transaction receipt or refusal.
    pub vendor_status: String,
    /// The player's live credit balance, bridged from the Wallet each frame.
    pub wallet_credits: i64,
    /// Buy/Sell clicked this frame: (item id, quantity). lib.rs settles them
    /// against the ECS inventory + Wallet via economy::vendor_buy/vendor_sell.
    pub pending_vendor_buy: Option<(String, u32)>,
    pub pending_vendor_sell: Option<(String, u32)>,
    /// Equip/unequip intents (v0.750, ladder rung 8): item id to wear / slot
    /// id to clear. The frame bridge moves items between the pack and the
    /// ECS Outfit (slot validated against equipment.csv).
    pub pending_equip: Option<String>,
    pub pending_unequip: Option<String>,
    /// Last equip action's receipt or refusal, shown in the Equipment section.
    pub equip_status: String,
    /// Room volumes (v0.429), for room-based label occlusion: which room is the camera in.
    pub room_bounds: Vec<RoomBounds>,
    /// Hold-Tab "reveal" peek (v0.429): triples the label distances and shows labels
    /// through walls across all owned/explored rooms. True only while Tab is held.
    pub reveal_held: bool,
    /// Distance (meters) at which a machine's DOT appears (the coarsest LOD).
    pub machine_label_dot_dist: f32,
    /// Distance (meters) at which a machine's NAME appears (closer than the dot).
    pub machine_label_name_dist: f32,
    /// Distance (meters) at which the full stat CARD appears (closest). Hold Tab x3 all.
    pub machine_label_card_dist: f32,
    /// Crew NPC nameplates (v0.667): name + live chore label floating over each
    /// relay-driven crew member. Rebuilt every frame from the RemoteNpc components
    /// (lib.rs, just before hud::draw), so it is always in sync with the amber figures.
    pub crew_labels: Vec<CrewLabel>,
    /// Tracked target markers (v0.885, operator design: select things on the
    /// map, see a ring + label in-world). v1 carries the orbital home
    /// station; planets/ships/enemies join as they become selectable.
    /// (name, render-space position, distance in meters).
    pub target_markers: Vec<(String, glam::Vec3, f64)>,

    // ── NPC walk-up talk (v0.797, operator: "I can't interact with NPCs at all") ──
    /// Relay entity_id of the crew NPC the player faces within talk range
    /// (~2.5 m look cone). Recomputed each frame in lib.rs like
    /// targeted_machine; drives the "[E] Talk to X" prompt and the E open.
    pub targeted_npc: Option<u64>,
    /// Crosshair prompt for the faced crew NPC ("[E] Talk to Botanist Yara").
    /// Empty = no NPC in range (or the card is already open).
    pub npc_prompt: String,
    /// Relay entity_id of the crew NPC whose dialogue card is OPEN. Some =
    /// the talk card modal is up: it joins in_world_modal_open so the cursor
    /// frees and gameplay keys stop, exactly like the in-world chat panel.
    /// lib.rs auto-closes it if the NPC wanders beyond 4 m or despawns (the
    /// relay keeps simulating chores; we deliberately do not freeze the NPC).
    pub npc_talk_target: Option<u64>,
    /// Card contents, snapshotted from the RemoteNpc when E opens the card.
    /// The lines were authored RELAY-side (its NPC components); the client
    /// only displays them -- no dialogue is hardcoded here (infinite-of-X).
    pub npc_talk_name: String,
    /// The NPC's live activity line ("Tending the hydroponic racks");
    /// refreshed each frame while the card is open so it tracks their chore.
    pub npc_talk_activity: String,
    /// The dialogue line currently shown on the card.
    pub npc_talk_line: String,
    /// Rotating conversation lines; More / repeat E cycles through these.
    pub npc_talk_dialog: Vec<String>,
    /// Opening lines; one was picked at random for the card's first line.
    pub npc_talk_greetings: Vec<String>,
    /// Index of the last dialog[] line shown; None = still on the greeting.
    pub npc_talk_index: Option<usize>,
    /// Transient confirmation shown after a "Save to server" click.
    pub profile_network_saved_note: String,
    // Interests
    pub profile_interests: Vec<String>,
    pub profile_interest_input: String,
    // Skills
    pub profile_skills: Vec<(String, f32)>,
    // Social Links
    pub profile_social_links: Vec<(String, String)>,
    pub profile_social_platform: String,
    pub profile_social_url: String,
    // Streaming
    pub profile_streaming_url: String,
    pub profile_streaming_live: bool,

    // ── Map state ──
    pub map_planets: Vec<GuiPlanet>,
    /// Seeded entities (You, your home, a vehicle, …) — each a container with its
    /// own contents + optional location. Loaded at startup; the inventory view
    /// renders them as top-level nodes, injecting live items at kind:"backpack".
    /// Empty until loaded. See [`Place`].
    pub places: Vec<Place>,
    /// The loaded Fibonacci homestead blueprint (the first offline "Design"),
    /// browsed on the Home page. None if the blueprint file is absent.
    pub homestead_design: Option<HomesteadDesign>,
    /// The homestead's self-sufficiency loops (energy/water/food/nutrients) from
    /// data/machines/home.ron, rendered as the closure summary on the Home page (v0.432).
    pub homestead_loops: Vec<crate::machines::HomeLoop>,
    /// The curated aeroponic tower configs (nutrition + apothecary), browsed on the
    /// Home page. Empty if data/towers/aeroponic_configs.ron is absent.
    pub tower_configs: Vec<TowerConfig>,
    /// Garden grow-areas (towers/beds/racks/tanks/fields) from home.ron, loaded via
    /// the resolved data_dir, for the Inventory Garden overview + per-medium edit modal.
    pub garden_areas: Vec<GardenArea>,
    /// Grow-media registry (data/garden/grow_media.ron) — the per-medium edit form is
    /// rendered from this, so adding a plot-type is a data edit (infinite-of-X).
    pub grow_media: Vec<GrowMedium>,
    /// Per-area irrigation targets the garden edit modal publishes to the sim, keyed by
    /// tower_id (e.g. "nutrition" -> 0.0..=1.0 water level). lib.rs bridges this into the
    /// DataStore's "garden_irrigation" channel each frame; FarmingSystem tops matching
    /// crops up to the target. Empty = no automated irrigation. Snapshotted by the
    /// inventory page each frame from the garden edit configs.
    pub garden_irrigation: std::collections::HashMap<String, f32>,
    /// Per-area nutrient strength the garden edit modal publishes (tower_id -> 0..1).
    /// Bridged to the DataStore's "garden_nutrient" channel; FarmingSystem scales
    /// matching crops' growth speed by it. Sibling of `garden_irrigation`.
    pub garden_nutrient: std::collections::HashMap<String, f32>,
    /// Organize-layer inventory pool: every seeded item tagged with its container path,
    /// so the nested-container inventory can move items between containers. Seeded from
    /// `places` at startup via `flatten_placed_items`. The live backpack is separate.
    pub placed_items: Vec<PlacedItem>,
    /// Pending backpack <-> container transfers (item_id, qty, is_add). The inventory
    /// page pushes these when an item moves into/out of the live backpack; lib.rs drains
    /// them into the InventorySystem channel each frame. is_add => add to the backpack.
    pub pending_inventory_transfers: Vec<(String, u32, bool)>,
    /// Where each "Take to backpack" came from (2026-09-25), so whatever the
    /// backpack cannot hold goes back to that container instead of
    /// vanishing. Moved to `inflight_take_origins` when the ops are handed to
    /// the InventorySystem, and resolved after its tick.
    pub pending_take_origins: Vec<crate::gui::PlacedItem>,
    pub inflight_take_origins: Vec<crate::gui::PlacedItem>,
    /// Per-tower shared-reservoir compatibility (parallel to `tower_configs`),
    /// computed once from the plant registry in the crop sync. The "make sure
    /// they grow together" check shown on the Home page.
    pub tower_compat: Vec<TowerCompat>,
    /// Creative mode: resource-consuming actions (planting seeds, fertilizing,
    /// crafting) skip the inventory requirement and consumption. OFF = survival
    /// (consume normally). Bridged to the DataStore "creative_mode" slot each
    /// frame so the farming + crafting + vehicle systems read it.
    ///
    /// OWNED by the play mode since task #50 (see `crate::config::PlayMode`):
    /// Normal FORCES it off every frame (the lib.rs bridge enforces survival);
    /// picking Creative/Dev presets it on, after which the Inventory page's
    /// toggle remains a live fine-tune inside those modes (testing real
    /// consumption while in Dev is legitimate). Default true matches the
    /// default Dev mode; AppConfig::apply_to_gui_state re-presets it from the
    /// persisted mode at startup.
    pub creative_mode: bool,
    /// Which section the merged Real tab shows — either a Profile section id
    /// ("body"/"identity"/"notes"/…) or a page id ("inventory"/"wallet"/
    /// "tasks"/"maps"/"market"). Drives `real::draw`'s delegate.
    pub active_real_section: String,
    /// Selected section for the folded Platform tab ("settings"/"recovery"/
    /// "tools"/"bugs"/"testing"/"browser").
    pub active_platform_section: String,
    /// Selected section for the folded Humanity tab ("civilization"/
    /// "governance"/"identity"/"onboarding"/"donate"/"resources").
    pub active_humanity_section: String,
    pub map_selected_planet: Option<usize>,
    pub map_zoom: f32,

    // ── Market state ──
    /// Abilities panel rows (v0.753), rebuilt while the Profile page is open.
    pub abilities: Vec<GuiAbility>,
    /// Cast click -> lib.rs bridges it into the ability_request channel.
    /// (ability id, optional target entity bits - the faced creature, v0.760).
    pub pending_cast: Option<(String, Option<u64>)>,
    /// One-line cast feedback from AbilitySystem ("First Aid restores 35 health").
    pub ability_status: String,
    /// game_time when ability_status was set; lib.rs fades it after a few
    /// seconds so the HUD line does not linger forever. (v0.754)
    pub ability_status_at: f64,
    /// F pressed: swing the held tool (or bare hands) at the faced creature.
    pub pending_swing: bool,
    /// game_time when the next swing is allowed (0.8s between swings). (v0.765)
    pub swing_ready_at: f64,

    pub listings: Vec<GuiListing>,
    /// Set once listing_browse has been sent this connection; cleared on
    /// disconnect so a reconnect re-syncs. (v0.752)
    pub listings_synced: bool,
    /// One-line market feedback ("Listing published", errors).
    pub listing_status: String,
    pub listing_search: String,
    pub listing_filter_category: String,
    /// Detail-view selection by LISTING ID (not index: live broadcasts
    /// reorder the vector under the open detail view).
    pub listing_selected: Option<String>,
    /// Reviews for the OPEN detail listing (v0.755), fetched over REST.
    pub listing_reviews: Vec<GuiReview>,
    /// Which listing the loaded reviews belong to ("" = none loaded).
    pub listing_reviews_for: String,
    /// In-flight REST fetch of the reviews list.
    pub listing_reviews_rx:
        Option<std::sync::mpsc::Receiver<Result<(Vec<GuiReview>, f32, i64), String>>>,
    pub listing_reviews_avg: f32,
    pub listing_reviews_count: i64,
    /// Review form drafts (detail view).
    pub review_rating_draft: i32,
    pub review_comment_draft: String,
    /// P2P trades (v0.756), delivered via targeted private wrappers.
    pub trades: Vec<GuiTrade>,
    /// Set once trade_list_request has been sent this connection.
    pub trades_synced: bool,
    /// One-line trade feedback ("Trade completed", errors).
    pub trade_status: String,
    pub listing_show_new_form: bool,
    pub listing_new_title: String,
    pub listing_new_description: String,
    pub listing_new_price: String,
    pub listing_new_category: String,

    // ── Calculator state ──
    pub calc_display: String,
    pub calc_expression: String,
    pub calc_history: Vec<String>,

    // ── Calendar state ──
    pub cal_year: i32,
    pub cal_month: u32,
    pub cal_selected_day: u32,
    pub cal_events: Vec<GuiCalendarEvent>,
    pub cal_new_title: String,
    pub cal_new_time: String,
    pub cal_new_color: egui::Color32,

    // ── Notes state ──
    pub notes: Vec<GuiNote>,
    pub notes_selected: Option<u64>,
    pub notes_next_id: u64,

    // ── Civilization state (live from GET /api/civilization, v0.757) ──
    /// The relay's aggregated community stats; None until the fetch lands.
    pub civ_stats: Option<GuiCivStats>,
    pub civ_stats_loaded: bool,
    pub civ_stats_rx: Option<std::sync::mpsc::Receiver<Result<GuiCivStats, String>>>,
    pub civ_status: String,

    // ── Watch page state (in-app stream viewer, v0.857) ──
    /// The active viewer decoding a stream in the background. None when not watching.
    pub watch_viewer: Option<crate::net::live_viewer::LiveViewer>,
    /// The current decoded frame as an egui texture, re-uploaded when a new frame lands.
    pub watch_texture: Option<egui::TextureHandle>,
    /// Directory of live streams from GET /api/live: (id, title, viewers).
    /// (id, title, viewers, bound chat room) rows from GET /api/live.
    pub watch_streams: Vec<(String, String, u64, String)>,
    /// Background result channel for the directory fetch.
    pub watch_streams_rx: Option<std::sync::mpsc::Receiver<Vec<(String, String, u64, String)>>>,
    /// Last time the directory was refreshed (egui time seconds), for periodic polling.
    pub watch_last_fetch: f64,
    /// Manual "watch by name" entry on the Watch page.
    pub watch_input: String,

    /// Top-nav button presentation (v0.859): icon+text / icon-only / text-only.
    /// Mirrors the web header's Aa toggle. Cycled by the nav's mode button.
    pub nav_display_mode: NavDisplayMode,

    /// Active confirmation toasts (v0.861), drawn + expired by `draw_toasts`.
    /// Altitude above the drawn planet ground, metres, while a surface is
    /// engaged (v0.867 surface HUD). None away from any surface.
    pub surface_altitude_m: Option<f32>,
    /// Gravity actually applied to the player this frame (m/s^2), captured
    /// at the walk-band integrator; None away from any surface band. Shown
    /// on the F2 overlay so gravity_curve tuning is visible live.
    pub surface_gravity_now: Option<f32>,
    /// Current mouse-wheel speed gear shown next to the altitude.
    pub surface_speed_mult: f32,

    pub toasts: Vec<Toast>,
    /// Camera is below the sea surface (v0.903 diving): the GUI paints the
    /// underwater tint and the HUD can show depth.
    pub underwater: bool,
    /// World-space offset the scene pass applies to home content (the
    /// orbital station ride, v0.881). Labels/nameplates must project with
    /// the SAME offset or they float where the home used to be (v0.911,
    /// the ghost-markers half of the vanishing-home bug).
    pub station_off: glam::Vec3,
    /// Metres below the sea surface while underwater (drives the tint's
    /// depth grading + the HUD depth readout; 0 when surfaced).
    pub underwater_depth_m: f32,
    /// F6 location bookmarks for the Dev > Travel list (v0.913):
    /// (id, category, body label). Loaded from debug/bookmarks.json.
    pub location_bookmarks: Vec<(String, String, String)>,
    /// True when the bookmark file changed (F6 save) and the GUI list
    /// should reload next frame.
    pub location_bookmarks_dirty: bool,
    /// Set by the Travel bookmark list; lib.rs teleports to this F6
    /// bookmark id and clears it.
    pub pending_bookmark_teleport: Option<String>,
    /// Category applied to NEW F6 bookmark saves (typed in Dev > Travel).
    pub bookmark_new_category: String,
    /// Bookmark id the Travel list wants DELETED (lib.rs rewrites the file).
    pub pending_bookmark_delete: Option<String>,
    /// Bookmark id + new category from the Travel list's recategorize
    /// control (lib.rs rewrites the file).
    pub pending_bookmark_recat: Option<(String, String)>,
    /// Toasts queued from engine code that has no egui clock (v0.890, e.g.
    /// the F6 bookmark save in the raw input path). Drained by draw_toasts,
    /// which stamps them with the real egui time.
    pub pending_toasts: Vec<(String, ToastKind)>,
    /// Same queue for NOTICES: longer-lived Info toasts carrying something
    /// the player needs to read rather than a confirmation (the offline
    /// progression "while you were away" line). See `GuiState::notice`.
    pub pending_notices: Vec<String>,
    /// Machine types the player's finished structures serve as (a built
    /// furnace is a "smelter"), mirrored each frame from the world so the
    /// Crafting page's station check agrees with CraftingSystem's.
    pub built_station_types: std::collections::HashSet<String>,

    // ── Wallet state ──
    pub wallet_balance: f64,
    pub wallet_address: String,
    pub wallet_network: WalletNetwork,
    pub wallet_send_to: String,
    pub wallet_send_amount: String,
    pub wallet_transactions: Vec<WalletTransaction>,
    pub wallet_sol_price: f64,

    // ── Crafting state ──
    pub craft_recipes: Vec<GuiRecipe>,
    pub craft_selected: Option<usize>,
    pub craft_selected_category: Option<String>,
    pub craft_status: String,
    /// Buildable blueprints (data/blueprints/basic.ron), bridged from the
    /// blueprint_registry for the Crafting page's Structures section
    /// (v0.746, closure ladder rung 2).
    pub blueprints: Vec<GuiBlueprint>,
    /// Blueprint id the player clicked Build on this frame; lib.rs turns it
    /// into a "build_request" at a spot in front of the camera.
    pub pending_build: Option<String>,
    /// ConstructionSystem's honest status line ("need 4x wood_plank to build
    /// Wooden Wall", "Building...", "... complete"), synced each frame.
    pub build_status: String,
    /// Recipe id the player clicked "Craft" on this frame; the main loop bridges it
    /// to the ECS CraftingSystem. None = nothing pending.
    pub pending_craft_recipe: Option<String>,
    /// Dev/creative provisioning request: stock the player with one stack of every
    /// recipe input (raws + intermediates) so every recipe is craftable immediately.
    pub dev_stock_materials: bool,

    // ── Survival / nutrition state ──
    /// Item id the player clicked "Eat" on this frame; the main loop bridges it to
    /// FoodSystem's consume channel. None = nothing pending.
    pub pending_consume_item: Option<String>,
    /// Item id the player clicked "Drink" on this frame → FoodSystem (restores hydration).
    pub pending_drink_item: Option<String>,
    /// True for the frame the player clicked "Rest" → refills energy via FoodSystem.
    pub pending_rest: bool,
    /// True for the frame the player clicked "Compost" → waste→fertilizer via FoodSystem.
    pub pending_compost: bool,
    /// Crop entity bits the player clicked "Fertilize" on this frame → FarmingSystem.
    pub pending_fertilize_crop: Option<u64>,
    /// Player vitals (satiation/hydration + active status effects), synced from the
    /// ECS each frame for the HUD / inventory page to display.
    pub vitals: GuiVitals,

    /// Vehicle KIT item id the player clicked "Deploy" on this frame → bridged
    /// into the "deploy_kit_request" channel for VehicleSystem (economy Phase 2
    /// Stage 1: the kit unpacks into a real Vehicle entity in front of the player).
    pub pending_deploy_kit: Option<String>,

    // ── Gardening state ──
    /// Seed item id the player clicked "Plant" on this frame → FarmingSystem.
    pub pending_plant_seed: Option<String>,
    /// GUI -> ECS: plant a whole aeroponic tower. The Vec is the plant ids (one per
    /// slot) to spawn as CropInstances; drained into the "plant_tower_request"
    /// channel for FarmingSystem (v0.386). Dev-friendly: no seed consumption yet.
    pub pending_plant_tower: Option<(String, Vec<String>)>,
    /// GUI -> ECS: plant a bed/tray/field grow area (v0.738 grain loop).
    /// (machine_id, plant_id, unit count) — one CropInstance per unit, tagged
    /// with the machine id as its grow-area; drained into "plant_bed_request".
    pub pending_plant_bed: Option<(String, String, u32)>,
    /// Seed item ids to grant the player (the "Dev: stock seeds" starter set);
    /// drained into "stock_seeds_request" for FarmingSystem.
    pub pending_stock_seeds: Option<Vec<String>>,
    /// Crop entity bits the player clicked "Water" on this frame.
    pub pending_water_crop: Option<u64>,
    /// Crop entity bits the player clicked "Harvest" on this frame.
    pub pending_harvest_crop: Option<u64>,
    /// Bulk harvest (v0.739): every mature crop's bits from a Garden group's
    /// "Harvest N ready" button; drained into "harvest_many_request".
    pub pending_harvest_many: Vec<u64>,
    /// Dev: instantly mature all crops (testing affordance, like dev-stock).
    pub dev_grow_crops: bool,
    /// Growing crops, synced from the ECS each frame for the Garden panel.
    pub crops: Vec<GuiCrop>,

    // ── Mining / drones state ──
    /// Set the frame the player clicks "Launch drone" → bridged to DroneSystem's
    /// commission channel: `(target asteroid id, manifest)`. One asteroid per run.
    pub pending_drone_manifest: Option<(String, Vec<(String, u32)>)>,
    /// "Keep mining" (economy automation, v0.663): while true, a launched drone
    /// order becomes a STANDING order that auto-relaunches the same trip each
    /// time a haul is delivered, until the asteroid depletes or this goes false.
    pub auto_mine_enabled: bool,
    /// The most recently launched drone order, kept so RE-CHECKING "Keep mining"
    /// mid-flight (after having unchecked it) can re-arm the standing order --
    /// the checkbox otherwise only took effect at launch time (review fix).
    pub last_drone_order: Option<(String, Vec<(String, u32)>)>,
    /// Previous frame's `auto_mine_enabled`, for rising-edge detection in the
    /// lib.rs bridge.
    pub prev_auto_mine_enabled: bool,
    /// True while a drone is in flight (synced) — one drone per player, so the panel
    /// shows the active drone instead of the builder + disables Launch.
    pub drone_active: bool,
    /// Asteroids (name + remaining ore), synced from the ECS for the Mining panel.
    pub asteroids: Vec<GuiAsteroid>,
    /// Active mining drones (ore / phase / cargo), synced from the ECS.
    pub drones: Vec<GuiDrone>,
    /// World vehicles (Stage 3, v0.680), synced from the ECS each frame for the
    /// Inventory page's Vehicles section.
    pub vehicles: Vec<GuiVehicle>,
    /// GUI -> ECS: summon this vehicle (entity bits) to drive itself to the
    /// player; bridged into the "summon_vehicle" channel.
    pub pending_summon_vehicle: Option<u64>,
    /// Cosmos -> Inventory jump (unified map slice 2): an asteroid id whose
    /// mining modal should open when the Inventory page next draws.
    pub pending_open_mining_modal: Option<String>,
    /// GUI -> engine: chase-cam this vehicle (entity bits) while it self-drives;
    /// any WASD input or arrival breaks the follow (Stage 3, v0.690).
    pub pending_follow_vehicle: Option<u64>,
    /// Live production status lines (v0.681), one per auto machine, synced from
    /// CraftingSystem's "auto_craft_status" channel each frame -- e.g.
    /// "Assemble Rover — 42%" or "Smelt Iron — waiting for Iron Ore x2".
    pub factory_status: Vec<String>,

    // ── Skills / progression state ──
    /// Player skills (live level + XP), synced from the ECS PlayerSkills each
    /// frame for the profile Skills panel. Empty until the first XP is earned.
    pub skills: Vec<GuiSkill>,
    /// Dev: max all skills next frame (testing affordance under #8b skill-gating).
    pub pending_dev_max_skills: bool,
    /// Player quests (active + completed), synced from the ECS QuestTracker each
    /// frame for the profile Quests panel.
    pub quests: Vec<GuiQuest>,
    /// Acceptable-but-unaccepted quests (v0.747.x, rung 4). Before this, quests
    /// with no prerequisite were unreachable authored content.
    pub quests_available: Vec<GuiAvailableQuest>,
    /// Quest id the player clicked Accept on; the frame bridge applies it to
    /// the ECS QuestTracker.
    pub pending_accept_quest: Option<String>,

    // ── Guilds state (live from the relay's REST guild API, v0.757) ──
    pub guilds: Vec<GuiGuild>,
    /// Selection by GUILD ID (refetches reorder the vector).
    pub guild_selected: Option<String>,
    pub guild_search: String,
    pub guild_show_create: bool,
    pub guild_new_name: String,
    pub guild_new_desc: String,
    pub guild_new_color: egui::Color32,
    /// Set once the list fetch has run this session; cleared to refetch.
    pub guilds_loaded: bool,
    /// In-flight list fetch (all guilds + my memberships merged).
    pub guilds_rx: Option<std::sync::mpsc::Receiver<Result<Vec<GuiGuild>, String>>>,
    /// In-flight create/join/leave action; Ok(msg) triggers a refetch.
    pub guild_action_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    /// One-line guild feedback ("Joined", errors).
    pub guild_status: String,
    /// Member list for the OPEN guild detail: (display name, role).
    pub guild_members: Vec<(String, String)>,
    pub guild_members_for: String,
    pub guild_members_rx:
        Option<std::sync::mpsc::Receiver<Result<Vec<(String, String)>, String>>>,

    // ── Bridged game state (written by lib.rs each frame) ──

    /// Player health fraction (0.0 to 1.0). Updated from ECS Health component.
    pub player_health: f32,
    /// Player max health. Updated from ECS Health component.
    pub player_health_max: f32,
    /// Inventory items from the player entity's Inventory component.
    pub inventory_items: Vec<Option<GuiItemSlot>>,
    /// Total inventory slot count.
    pub inventory_max_slots: usize,
    /// Current game time snapshot.
    pub game_time: Option<GuiGameTime>,
    /// Current weather snapshot.
    pub weather: Option<GuiWeather>,
    /// Live home electrical readout (watts), mirrored from ElectricalSystem each frame.
    pub power_generation: f32,
    pub power_consumption: f32,
    pub power_balance: f32,
    /// Live battery state (v0.473): total charge, capacity (watt-hours), and hours of autonomy.
    pub power_battery_wh: f32,
    pub power_battery_capacity_wh: f32,
    pub power_autonomy_hours: f32,
    /// Live home WATER readout (v0.608), mirrored from PlumbingSystem each frame: production + demand
    /// (L/min), stored + capacity (litres), and days of autonomy at the current demand.
    pub water_production_lpm: f32,
    pub water_demand_lpm: f32,
    pub water_stored_l: f32,
    pub water_capacity_l: f32,
    pub water_days_autonomy: f32,
    /// Live home AIR readout (v0.617), mirrored from AtmosphereSystem each frame: O2/CO2 percent, total
    /// pressure (atm), temperature (C), and whether the mix is breathable.
    pub air_o2_pct: f32,
    pub air_co2_pct: f32,
    pub air_pressure_atm: f32,
    pub air_temp_c: f32,
    pub air_breathable: bool,
    /// Character-select showroom (v0.441): when active, the home is hidden and the avatar is
    /// previewed against a backdrop with an orbit camera + the customization panel.
    pub showroom_active: bool,
    /// Construction mode (v0.453): show the home's roof/ceiling. Default OFF so the sky
    /// (stars + the real solar system) stays visible from inside; toggle on for atmosphere
    /// tests or a sealed look.
    pub show_roof: bool,
    /// Hull wrap (ship-superstructure increment D): show the generated exterior hull around
    /// the whole zone cluster. Default ON so the ship reads as a vessel from outside; toggle
    /// off (H in-world, or the Settings checkbox) for unobstructed interior screenshots or a
    /// clear top-down build view. Glass roofs keep their starlight either way (the hull top is
    /// cut open above them).
    pub show_hull: bool,
    /// Construction EDITOR (v0.455): when active, a panel lets the player set each room's
    /// per-wall kind + the uniform height and rebuild the home live. Toggled with B in-world.
    pub construction_active: bool,
    /// The home's MACHINE layout (data/machines/home.ron), loaded at startup + EDITABLE in
    /// the construction editor (v0.519: machine placement -- the #1 home-design parity gap).
    /// The AI edits the same file by hand, so an AI-placed machine is player-editable and
    /// vice versa. Saved by `home_machines_save` alongside the room layout. See
    /// docs/design/home-design.md. None if home.ron is absent.
    pub home_machines: Option<crate::machines::MachineHome>,
    /// The editor's Add-Machine picker selection (a catalog type id). (v0.519)
    pub home_machine_add_type: String,
    /// The editor's Add-Connection pickers: from-machine id, to-machine id, and kind. (v0.523,
    /// Stage 2: players wire machines -- power/water/etc. -- the same connections the AI authors.)
    pub home_conn_from: String,
    pub home_conn_to: String,
    pub home_conn_kind: String,
    /// Conduit node-graph branch picker (v0.581): the from/to endpoints ("m:id" machine or "n:id" node)
    /// + the kind for the Branch button that adds a conduit edge.
    pub conduit_from: String,
    pub conduit_to: String,
    pub conduit_kind: String,
    /// Construction text-command console (v0.578): the input line + the last result/output. An
    /// AI-enumerable ACT surface -- the same struct mutations the gizmos make, driven by typed verbs.
    pub construction_console_input: String,
    pub construction_console_output: String,
    /// Set by the editor's Save to write home_machines back to home.ron (mirrors
    /// `construction_save` for rooms). The engine clears it after writing.
    pub home_machines_save: bool,
    /// Transient "Saved …" confirmation under the editor's Save button —
    /// set by the engine after the write lands so saving is never silent
    /// (v0.735, operator: "the save home button doesn't do anything").
    pub construction_save_note: String,
    /// Editable mirror of the layout's rooms (walls + position + size). The engine fills this
    /// when the editor opens and reads it back when `construction_dirty`. (v0.459)
    pub construction_rooms: Vec<ConstructionRoom>,
    /// Room-type ids from the registry, for the Add-Room picker (sorted, stable). (v0.459)
    pub construction_room_types: Vec<String>,
    /// The rooms.ron + room_actions.ron registry itself, loaded alongside `construction_room_types`
    /// when the editor opens, so the zone detail panel can show what a zone's `room_type` resolves
    /// to (purpose + action labels via `HomeStructure::room_actions_for`) without touching disk
    /// every frame. Empty until the editor first opens. (console-room increment)
    pub room_type_registry: crate::ship::room_types::RoomTypeRegistry,
    /// Current Add-Room picker selection. (v0.459)
    pub construction_add_type: String,
    /// A room index the panel requested to delete; applied after the scroll loop. (v0.459)
    pub construction_remove: Option<usize>,
    /// Show the top-down floor-plan overlay (v0.464). Default OFF: construction uses the free
    /// orbit "astral" camera (drag/pan/dolly/fly); toggle on for the 2D plan when wanted.
    pub construction_plan_view: bool,
    /// Keymap reference (v0.465): true while F1 is held; shows the bindings for the current
    /// screen/mode. Loaded once from data/keymaps.ron.
    pub keymap_visible: bool,
    pub keymaps: Vec<crate::gui::pages::keymap::KeymapContext>,
    /// Help panel pinned OPEN by the top-right "?" button (v0.1212). F1 is a
    /// hold-to-glance; this is the sticky twin, so the panel renders when either
    /// `keymap_visible` (F1 down) or this is true. The button flips to "X" while
    /// set, and Esc clears it. Never persisted: help starts closed each run.
    pub help_panel_pinned: bool,
    /// Diagnostics dev-HUD overlays (v0.482), each toggled by an F-key and shown
    /// stacked in the top-right corner. F2 = performance, F3 = network, F4 =
    /// system. Listed in the F1 keymap so they are discoverable.
    pub show_perf_overlay: bool,
    /// F11 live weather panel (v0.1050). Permanent operator tooling: the
    /// ocean's character is wind-driven, so this is how the sea gets reviewed.
    pub show_weather_panel: bool,
    /// F10 cloud dev panel (v0.1254.4, operator request): GUI buttons for
    /// the cloud bisect toggles that previously needed showcase file drops.
    pub show_cloud_dev_panel: bool,
    /// F10 sidebar collapsed to its slim edge tab (2026-09-05, operator
    /// request for a collapse button so F10 need not be pressed). Session
    /// only, like everything on that panel. While collapsed the cursor is
    /// grabbed again as if the panel were closed; F10 or the tab expands it.
    pub cloud_dev_collapsed: bool,
    /// Mirror of the OS cursor state (`EngineState::cursor_free`), written by
    /// lib.rs `reconcile_cursor` every time it derives the cursor from the
    /// flags, so GUI code can ask "can the operator actually click right
    /// now?" without touching winit. Added 2026-09-05 for the F10 sidebar's
    /// collapse tab (critic finding): under CursorGrabMode::Confined the
    /// INVISIBLE cursor still slides to the window edge on a sustained turn,
    /// so a click-sensing strip at x 0..22 would swallow the operator's next
    /// fire/interact click and pop the sidebar open mid-flight. The tab is
    /// therefore only drawn while this is true (F10 still expands it). Not
    /// authoritative: read-only for GUI code, only lib.rs writes it.
    pub cursor_free: bool,
    /// Panel state mirrored into the renderer flags by lib.rs each frame -
    /// the SAME flags the showcase pins set, so file drops and buttons agree.
    pub cloud_dev_dither_off: bool,
    pub cloud_dev_temporal_off: bool,
    /// Bisect channel: 0 off, 1 coverage alpha, 2 direct sun, 3 ambient.
    pub cloud_dev_map_diag: i32,
    /// Cloud advection clock pin in seconds (negative = live). Makes a rig
    /// capture a function of the build alone; see Renderer::cloud_clock_pin.
    pub cloud_dev_clock_pin: f32,
    /// Comparison switch: put the chord back in the detail scale (v0.1268).
    pub cloud_dev_chord_foot: bool,
    /// Shape fields at a fixed world level of detail (v0.1269 test).
    pub cloud_dev_world_shape_lod: bool,
    /// Disable the mip ring cure (default false - the cure runs).
    pub cloud_dev_ring_cure_off: bool,
    /// Experiment: distance-only march step (v0.1271).
    pub cloud_dev_uniform_step: bool,
    /// Experiment: wide density edge (v0.1271).
    pub cloud_dev_wide_edge: bool,
    /// Edge-width multiplier (0 = shader default), v0.1271.
    pub cloud_dev_edge_mul: f32,
    /// Wide rind metres (0 = shader default), v0.1271.
    pub cloud_dev_rind_wide_m: f32,
    /// Fixed march step metres (0 = off), v0.1271.
    pub cloud_dev_step_m: f32,
    /// Uniform eastward lean (v0.1275), 0 = off.
    pub cloud_dev_shear: f32,
    /// Height-varying warp amplitude km (v0.1279), 0 = default.
    pub cloud_dev_hv_km: f32,
    /// Extinction multiplier (v0.1279), 0 = off.
    pub cloud_dev_sigma_mul: f32,
    /// Sample-anchored march (v0.1272).
    pub cloud_dev_est: bool,
    /// Warp band-limit + rind/4 refine (v0.1272).
    pub cloud_dev_warp_bl: bool,
    /// Carve normaliser floor (v0.1273).
    pub cloud_dev_norm_floor: bool,
    /// Isotropic near step (v0.1274).
    pub cloud_dev_iso_step: bool,
    /// Thin-deck experiment (v0.1275).
    pub cloud_dev_thin_deck: bool,
    /// Height-varying warp (v0.1278).
    pub cloud_dev_hv_warp: bool,
    /// Component bisect (v0.1279).
    pub cloud_dev_no_detail: bool,
    pub cloud_dev_no_puff: bool,
    pub cloud_dev_no_cell: bool,
    pub cloud_dev_no_fray: bool,
    pub cloud_dev_no_bdrop: bool,
    /// Sharp cloud base (v0.1279).
    pub cloud_dev_sharp_base: bool,
    /// Interior relief fade (v0.1279).
    pub cloud_dev_relief_fade: bool,
    /// Coarse sun ladder for deep samples (v0.1279).
    pub cloud_dev_deep_rung: bool,
    /// Synthetic checker density (v0.1279): the projection test.
    pub cloud_dev_checker: bool,
    /// Increment A: the in-cloud light (v0.1280).
    pub cloud_dev_ms: bool,
    /// Increment B 2.1: the three-octave domain warp (v0.1281).
    pub cloud_dev_field: bool,
    /// Perf increment 3 (v0.1287): the per-ray body cluster cache.
    pub cloud_dev_body_cache: bool,
    /// Perf increment 2 (v0.1288): step-economy strength 0..1.
    pub cloud_dev_step_eco: f32,
    /// Performance plan increment 1 (v0.1286): the sun-shadow cache. Off =
    /// the 12-rung sun ladder per pixel, the A/B twin.
    pub cloud_dev_light: bool,
    /// Performance plan increment 4, the far rung: the cloud PROFILE knob
    /// (0 off = the point-sampled field, bit-identical; 1 on = automatic
    /// level; 2..7 = level 0..5 forced; 8 = hard switch; 9 = reference
    /// bake). Showcase key `cloud_profile`.
    pub cloud_dev_profile_knob: i32,
    /// D3 dev bit (2026-09-06): the built-body TOP BOUND. On = the march
    /// finds thin built clouds from above (a from-above SDF lower bound
    /// plus an in-cloud step floor capped at a quarter of the found
    /// cloud's height); off = today's 928 m comb, the A/B twin. Flags-pad
    /// bit 12 of light2_color.w, independent of the profile knob. Showcase
    /// key `cloud_top_bound`.
    pub cloud_dev_top_bound: bool,
    /// Gain on the in-scattered source, 0 = default.
    pub cloud_dev_ms_gain: f32,
    /// Increment C (v0.1282): interior saturation of the built bodies, 0..1.
    pub cloud_dev_int_sat: f32,
    /// Cloud march resolution divisor: 4 quarter (default), 2 half, 1 full.
    pub cloud_dev_res_div: u32,
    /// F10: disable the per-cloud shape frame (A/B the squash + stretch).
    pub cloud_dev_shape_off: bool,
    /// F10 bisect channel: composite discard reasons.
    pub cloud_dev_discard_diag: bool,
    /// True while the panel is driving the weather (random rolls suspended).
    pub weather_manual: bool,
    pub weather_pick_condition: crate::systems::weather::WeatherCondition,
    pub weather_pick_intensity: f32,
    pub weather_pick_wind: f32,
    /// The selected preset's "what to look for" line, shown under the grid.
    pub weather_pick_note: String,
    /// Set on a condition pick so the sim re-runs its 30 s transition.
    pub weather_retrigger: bool,
    /// Time-of-day scrubber (v0.1224). The clock lives inside TimeSystem's own
    /// accumulator (the DataStore copy is overwritten every tick), so the panel
    /// cannot just poke `game_time`: it drains these into the
    /// `time_set_hour_request` / `time_set_scale_request` channels, the same
    /// pattern the screenshot hook has used since v0.871. Until now that hook
    /// was the ONLY way to move the clock - there was no in-app control at all,
    /// which the GUI-first rule does not allow for something this central to
    /// reviewing sky, sea and lighting.
    pub time_hour_request: Option<f32>,
    pub time_scale_request: Option<f32>,
    /// Last hour the scrubber published, so the slider does not fight the
    /// running clock while the operator is dragging it.
    pub time_pick_hour: f32,
    /// Chosen clock speed (1 = real time, 60 = a game day per 20 s) and
    /// whether the clock is held still. Frozen publishes scale 0, which is what
    /// makes a lighting or sky comparison actually repeatable.
    pub time_speed: f32,
    pub time_frozen: bool,
    /// Home-station attitude, mirrored for the F11 panel (v0.1225).
    ///
    /// This is the operator's own proposal - "add a control to the mothership
    /// to actually change its orientation to also change that of the lighting"
    /// - and it is a LIGHTING control, which is why it lives beside the clock
    /// rather than in Settings. Nadir-pointing is how real stations fly and is
    /// what gives the homestead a day; Inertial is the old frozen behaviour,
    /// kept as a deliberate choice instead of an accident.
    pub station_nadir: bool,
    pub station_yaw_deg: f32,
    pub station_pitch_deg: f32,
    pub station_roll_deg: f32,
    /// Set when the panel changes attitude; the frame loop applies it.
    pub station_attitude_dirty: bool,
    /// One-line orbit summary published by the frame loop for the panel.
    pub station_readout: String,
    /// Third-party attributions from data/credits.ron, loaded once at startup.
    /// Several are licence obligations - see src/credits.rs.
    pub credits: crate::credits::Credits,
    /// Names of the OpenStreetMap regions currently drawn in the world.
    ///
    /// Drives the in-world attribution line. ODbL treats a rendered view of
    /// OSM data as a Produced Work needing a visible notice wherever it is
    /// shown, and until v0.1226 the in-world view had the notice only in a
    /// log line and the F12 debug console - neither of which a player sees.
    /// Empty means no OSM geometry is on screen and nothing is owed.
    pub osm_regions_drawn: Vec<String>,
    pub show_network_overlay: bool,
    pub show_system_overlay: bool,
    /// Recent frame times in milliseconds (ring buffer, newest last), for the
    /// performance overlay's frame-time sparkline. Capped at ~120 samples.
    pub frame_times: Vec<f32>,
    /// Count of WebSocket messages received this session (network overlay).
    pub ws_msgs_in: u64,
    /// Settings audio (v0.485). mic_test_active is the toggle: while true, lib.rs
    /// keeps a mic to Opus to speaker loopback running so you can confirm audio
    /// works. The device fields are the chosen input/output (empty = system
    /// default); the *_devices lists are cached (enumerating cpal every frame is
    /// slow), refreshed on demand. mic_meter is a decayed level for the meter.
    pub mic_test_active: bool,
    pub audio_input_device: String,
    pub audio_output_device: String,
    pub audio_input_devices: Vec<String>,
    pub audio_output_devices: Vec<String>,
    pub audio_devices_loaded: bool,
    pub mic_meter: f32,
    /// Previous mic_test_active, so lib.rs starts/stops the loopback only on the
    /// toggle EDGE (not every frame, which would spin-retry a failing start).
    pub mic_test_prev: bool,
    // ── v0.488 voice input prefs (persisted via AppConfig) ──────────────
    /// Mic input gain, 1.0 = 100%. Range 0.0..=2.0 (200%).
    pub voice_gain: f32,
    /// Noise filter applied to the mic before encode.
    pub voice_filter_mode: crate::config::VoiceFilterMode,
    /// When the mic is actually transmitted (open mic / PTT / VAD / push-to-mute).
    pub voice_transmit_mode: crate::config::VoiceTransmitMode,
    /// The push key (egui Key name) for PTT / push-to-mute.
    pub voice_ptt_key: String,
    /// Voice-activation RMS threshold (0.0..=1.0).
    pub voice_vad_threshold: f32,
    /// Runtime: is the push key currently held this frame? Set by lib.rs from
    /// the input state; gates transmit for PTT / push-to-mute. Not persisted.
    pub voice_ptt_held: bool,
    /// Runtime: the settings UI is waiting for the user to press a key to bind
    /// as the push key. Not persisted.
    pub voice_binding_key: bool,
    /// The rebindable game keymap (Settings > Controls, 2026-08-12). Defaults
    /// plus the user's persisted overrides (config `keybind_overrides`);
    /// lib.rs resolves every gameplay key event through this.
    pub keybinds: crate::input::bindings::Keybinds,
    /// Runtime: Some((action, secondary_slot)) while the Controls page waits
    /// for the next key press to become that bind. Esc cancels, Delete clears
    /// the slot. Not persisted.
    pub keybind_capture: Option<(crate::input::bindings::GameAction, bool)>,
    /// Runtime: a bind attempt hit a key another action already holds. The
    /// Controls page shows a confirm (move the key or keep it). Fields:
    /// (action being bound, secondary slot, key name, current holder).
    /// Not persisted.
    pub keybind_conflict:
        Option<(crate::input::bindings::GameAction, bool, String, crate::input::bindings::GameAction)>,
    /// Runtime: one-line status under the bind grid ("Function keys are
    /// reserved..."). Empty = nothing to say. Not persisted.
    pub keybind_status: String,
    /// Live diagnostics sampled from EngineState each frame (only while the
    /// relevant overlay is open, so they cost nothing when hidden). entity_count
    /// = ECS entities, mem_mb = process RSS, uptime_secs = since launch.
    pub diag_entity_count: usize,
    /// Live GPU light count for the F2 overlay (v0.782, uncapped lights).
    pub diag_light_count: usize,
    pub diag_mem_mb: f32,
    pub diag_uptime_secs: u64,
    /// Index into construction_rooms of the room selected/grabbed in the 3D astral editor, for
    /// the highlight tint. None = no selection. (v0.466)
    pub construction_selected_room: Option<usize>,
    /// Editable uniform ceiling height (mirrors layout.default_wall_height).
    pub construction_height: f32,
    /// The active storey the editor is focused on (v0.471). The room tree filters to this level
    /// and new rooms are created on it; the level stepper per room moves a room between storeys.
    pub construction_level: i32,
    /// Set by the panel when a wall/position/size/add/remove changed -> the engine rebuilds.
    pub construction_dirty: bool,
    /// Set by the panel on a MACHINE edit (offset / add / remove / connect) -> the engine refreshes
    /// just the machine meshes live, no full room rebuild. (v0.525)
    pub construction_machines_dirty: bool,
    /// The relay's identify handshake (nonce challenge -> Dilithium proof -> bind) has COMPLETED
    /// on the current socket (v0.794). Set when the first post-bind message (peer_list) arrives;
    /// cleared at every connect call site. Pre-bind, the relay's identify loop silently DISCARDS
    /// any other message type -- a game_join sent the instant is_connected() went true was
    /// dropped, so the client showed "Shared world" while the server never registered the player
    /// (found by the v0.793 autopilot two-instance test; a fast-loading real client could hit
    /// the same race).
    pub ws_identified: bool,
    /// Star catalog tier chooser (v0.800 rung 2; 2026-07-11 rung 4 adds the ultra tier). The
    /// Settings > Graphics card sets the request fields with the tier the user clicked; lib.rs
    /// consumes them (it owns data_dir + the download thread). One download at a time: the dl
    /// slot carries (tier, progress) where progress is (downloaded_bytes, total_bytes,
    /// status_line); None = no download running.
    pub star_catalog_download: Option<crate::renderer::stars::StarCatalogTier>,
    pub star_catalog_remove: Option<crate::renderer::stars::StarCatalogTier>,
    pub star_catalog_dl: Option<(
        crate::renderer::stars::StarCatalogTier,
        std::sync::Arc<std::sync::Mutex<(u64, u64, String)>>,
    )>,
    /// Installed size in bytes per DOWNLOADABLE tier, indexed by StarCatalogTier::index()
    /// (the standard catalog always ships with the app and is not tracked here). Refreshed by
    /// lib.rs at init and after download/remove, not polled per frame.
    pub star_catalog_installed: [Option<u64>; 2],
    /// Ultra Milky Way glow download (2026-07-11): same plumbing as the star catalog
    /// fields above but for the single downloadable galaxy_glow_ultra.png (16384x8192,
    /// ~99 MB). The Settings > Graphics glow chooser sets the request bools; lib.rs
    /// consumes them (it owns data_dir + the download thread). Progress =
    /// (downloaded_bytes, total_bytes, status_line); None = no download running.
    pub galaxy_glow_download: bool,
    pub galaxy_glow_remove: bool,
    pub galaxy_glow_dl: Option<std::sync::Arc<std::sync::Mutex<(u64, u64, String)>>>,
    /// Installed size in bytes of data/galaxy_glow_ultra.png; None = not downloaded.
    /// Refreshed by lib.rs at init and after download/remove, not polled per frame.
    pub galaxy_glow_installed: Option<u64>,
    /// Screenshot capture request (v0.810): set by the Testing page's capture buttons,
    /// consumed by lib.rs at end-of-frame. (0, 0) = capture the window swapchain as-is
    /// (GUI included); any other pair = one-frame offscreen scene render at exactly that
    /// resolution (GUI-free, wallpaper-clean), clamped to the device's max texture
    /// dimension. Same engine path as dropping debug/screenshot_request.json.
    pub screenshot_capture_request: Option<(u32, u32)>,
    /// One-line outcome of the last capture ("Saved debug/screenshot_3.png (3840x2160)"
    /// or the error), shown under the Testing page's capture buttons.
    pub screenshot_last_result: Option<String>,
    /// Text buffers for the Testing page's custom width/height entry (v0.810).
    pub screenshot_custom_width: String,
    pub screenshot_custom_height: String,
    /// SOLO play intent (v0.801): true = never game_join, no avatar in the shared world even
    /// while the chat socket is connected. Set by the launcher (RED offline home = solo, server
    /// card = shared) and by the Dev travel step-out; flipping to true while joined sends
    /// game_leave (world-scoped eviction, chat unaffected). Operator: "I tried to join my
    /// offline world from the character select but that didn't work. As an admin this should
    /// be overridden. If I can't teleport then I can't moderate."
    pub copresence_solo: bool,
    /// Armed whenever a structure or machine edit lands (the dirty consumers set it); the engine's
    /// 60 s autosave + the window-close flush write ship_structure.ron/home.ron and clear it.
    /// Before v0.791 the ship persisted ONLY through the explicit Save button -- quit without
    /// clicking and every wall/light/strip edit was silently lost (inventory autosaves; the ship
    /// didn't), which the operator read as "my saves aren't saving".
    pub construction_unsaved: bool,
    /// Footer placement-palette state (v0.527): the selected category tab, whether the grid is
    /// expanded (1 row -> multi-row).
    pub construction_palette_category: String,
    pub construction_palette_expanded: bool,
    /// The machine type currently "held" for placement (v0.529): the palette puts a type here, the
    /// editor renders it as a ghost following the cursor + drops it where you click the floor (click
    /// the same item again, or Escape/right-click, to cancel). None = not placing.
    pub construction_place_type: Option<String>,
    /// Light type HELD for viewport placement from the palette's Lights
    /// category (v0.784, operator: "add the lights to the bottom build menu").
    /// Click the floor to drop one; stays held for a row of lights; right-click
    /// or re-clicking the palette tile cancels.
    pub construction_place_light: Option<String>,
    /// The STRUCTURAL piece type currently "held" for placement (v0.583): set by the "Structure"
    /// palette category, dropped where you click the floor. Mutually exclusive with
    /// construction_place_type (a machine) + wall_mode. None = not placing a structure.
    pub construction_structure_type: Option<String>,
    /// Place-a-CONDUIT-NODE mode (v0.629): when true, clicking the floor in the 3D view drops a pipe-graph
    /// junction node there (a "main line" point you then drag machine ports onto). Toggled from the Conduit
    /// nodes panel; right-click cancels. Mutually exclusive with the other place modes.
    pub construction_place_conduit_node: bool,
    /// Zone-type id selected in the "Add zone" picker (v0.631, superstructure M1).
    pub zone_add_type: String,
    /// Selected ZONE id (v0.634): picked in the 3D view -> its detail shows on the right + it highlights;
    /// draggable on the floor. Not serialized (a pure selection). None when no zone is selected.
    pub construction_zone_selected: Option<String>,
    /// Rail-graph add-edge picker endpoints (v0.635, superstructure M2).
    pub rail_edge_from: u32,
    pub rail_edge_to: u32,
    /// Index of the placed structure selected in the editor (its detail shows on the right). (v0.583)
    pub construction_structure_selected: Option<usize>,
    /// Camera FOCUS request (v0.593): set to a world (x,y,z) when a left-list row is double-clicked;
    /// the engine snaps the orbit camera to it next frame (so you can see what you clicked) + clears it.
    pub construction_focus_request: Option<(f32, f32, f32)>,
    /// Objects-browser filter text (v0.598): rows whose name contains it (case-insensitive) show;
    /// empty = all. With a non-empty filter, matching type-groups auto-expand.
    pub construction_object_filter: String,
    /// Multi-select set for the object browser (v0.612): Ctrl+click a row to add/remove it; the set is
    /// keyed by a stable "tag:id" string (e.g. "Machine:tower_0", "Wall:3"). Drives group delete +
    /// group nudge. Empty = normal single-selection.
    pub construction_multi: std::collections::HashSet<String>,
    /// LOCKED object-type tags (v0.614): a type in this set ("Wall"/"Struct"/"Machine"/"Light"/"Road"/
    /// "Pipe") can't be selected or grabbed in the viewport -- so you can lock your walls while arranging
    /// machines and never fat-finger them. Toggled per type-group in the object browser.
    pub construction_locked_types: std::collections::HashSet<String>,
    /// Per-type HIDE set (v0.636): object types ("Machine"/"Wall"/"Pipe"/"Zone"/...) whose meshes +
    /// gizmos are skipped in the 3D view (and can't be picked), to declutter a busy build. Mirrors the
    /// lock set; not serialized (a pure view toggle).
    pub construction_hidden_types: std::collections::HashSet<String>,
    /// Selected ROAD-graph node id (v0.597): its detail shows on the right; draggable in the viewport.
    pub construction_road_node_selected: Option<u32>,
    /// Selected CONDUIT-graph node id (v0.597): its detail shows on the right; draggable in the viewport.
    pub construction_conduit_node_selected: Option<String>,
    /// Selected machine-machine CONNECTION (v0.626): (from id, to id) of the pipe/wire picked in the 3D
    /// view, so a connection is a first-class clickable object (detail + Remove on the right panel) like
    /// walls/doors. None when no pipe is selected.
    pub construction_connection_selected: Option<(String, String)>,
    /// Yaw (degrees) applied to the next placed structure -- rotate the held piece with [ and ].
    pub construction_structure_yaw: f32,
    /// Height above the room floor (metres) the next placed structure drops at (v0.588): 0 = on the
    /// floor; set it to a staircase's top so a deck lands as an upper-level landing.
    pub construction_structure_place_y: f32,
    /// Road-graph editor form state (v0.586): the from/to nodes, class, and width for the next edge.
    pub construction_road_from: u32,
    pub construction_road_to: u32,
    pub construction_road_class: String,
    pub construction_road_width: f32,
    /// Dimension overlay toggle (v0.595): the floating measurement text (wall lengths, corner angles,
    /// feature gaps). Default on; turn off from the Options/Dev section to de-clutter the view.
    pub construction_dimension_overlay: bool,
    /// Master toggle for the build-mode HELPER overlays (v0.587): the non-interactive bounds gizmos on
    /// machines + structures, the road graph (node rings + edge lines), and conduit-node markers.
    /// Default on. The interactive editing handles (corner orbs, resize cubes) + the light gizmos (the
    /// diamond is clickable) are always shown -- this only quiets the passive helpers.
    pub construction_show_helpers: bool,
    /// The SHIP: many pressurized zones, each a fixed outer box + freely-designed interior walls
    /// (v0.754, ship-superstructure increment A; the single-home model was v0.534). The editor
    /// edits the zone selected by `construction_zone` (via `ship::ship_structure::zone_body[_mut]`,
    /// free functions on this field so sibling GuiState fields stay borrowable); the engine renders
    /// ALL zones (load_world + rebuild_homestead via `ShipStructure::generate_meshes`) instead of
    /// the old room-AABB layout when present. Loaded in load_world (with one-time adoption of a
    /// legacy home_structure.ron).
    pub ship_structure: Option<crate::ship::ship_structure::ShipStructure>,
    /// Index into `ship_structure.zones` of the zone the construction editor is EDITING (the zone
    /// selector at the top of the Home structure panel, v0.754). All viewport tools + panel edits
    /// operate on this zone; gizmos and ray picks convert between world and zone-local space
    /// through its origin.
    pub construction_zone: usize,
    /// Two-click delete confirm for the zone selector's "Delete zone" (v0.754): armed by the first
    /// click, executed by "Confirm delete", cleared on cancel / zone switch.
    pub construction_zone_delete_arm: bool,
    /// Corridor add-flow state (ship-superstructure increment B, the "Corridors" section under the
    /// zone selector): pending from/to zone indices, the world `lat` centreline, the door-mouth
    /// width/height the corridor cuts through each zone's shell, tube width, glass-top flag, and
    /// the last validation error ("" = none) shown under Create. (The corridor rework replaced the
    /// old per-zone door pickers: corridors own their mouths instead of indexing authored doors.)
    pub construction_corridor_from_zone: usize,
    pub construction_corridor_to_zone: usize,
    pub construction_corridor_lat: f32,
    pub construction_corridor_door_w: f32,
    pub construction_corridor_door_h: f32,
    pub construction_corridor_width: f32,
    pub construction_corridor_glass: bool,
    pub construction_corridor_error: String,
    /// Set by the panel on an interior-wall edit (add / remove / move corner / opening) -> the
    /// engine rebuilds the home mesh and writes ship_structure.ron on Save. (v0.534)
    pub construction_structure_dirty: bool,
    /// Wall-drawing mode (v0.534): true while the "Add wall" tool is active. Click the floor to
    /// drop corner nodes; the first click sets `construction_wall_start`, the second adds the wall
    /// segment and chains (start = the new corner). Escape / right-click exits.
    pub construction_wall_mode: bool,
    /// The pending first corner (x, z metres from the box min corner) while drawing a wall. (v0.534)
    pub construction_wall_start: Option<(f32, f32)>,
    /// The cursor's current floor position (box-local x, z) in build mode (v0.545), set by the engine
    /// each frame so the dimension overlay can show the live segment length + cursor readout.
    pub construction_cursor_world: Option<(f32, f32)>,
    /// Index of the interior wall currently selected in the editor (for remove / opening edits), or
    /// None. (v0.534)
    pub construction_wall_selected: Option<usize>,
    /// Id of the machine currently selected in the editor (clicked in the viewport or the list), or
    /// None. Mutually exclusive with construction_wall_selected -- the right panel shows whichever is
    /// set. (v0.553)
    pub construction_machine_selected: Option<String>,
    /// Index into home_structure.lights of the light selected in the editor (clicked its diamond gizmo),
    /// or None. The right panel shows its detail. Mutually exclusive with wall/machine selection. (v0.576)
    pub construction_light_selected: Option<usize>,
    /// Which of the SELECTED light's rotation rings the build-mode cursor ray is hovering
    /// (0 = X/red, 1 = Y/green, 2 = Z/blue), or None. Recomputed every frame from the SAME
    /// pick test a click uses, so the ring that brightens is exactly the ring a press would
    /// grab (idle -> hover -> active, like the other gizmos). (v0.792)
    pub construction_light_ring_hover: Option<u8>,
    /// Where the player avatar stands in BUILD mode (x, z in box coords), draggable by its pyramid
    /// gizmo. Leaving build mode drops you into first person right here. (v0.557)
    pub build_char_pos: Option<(f32, f32)>,
    /// Snap wall corners to a 0.25 m grid while drawing + dragging (v0.541). Endpoint snapping (to
    /// the box edges + other corners) is always on for airtight seals; this toggles the grid.
    pub construction_grid_snap: bool,
    /// Dev overlay (v0.547): when on, the build widgets (dimension overlay + door interaction rings)
    /// stay visible in NORMAL PLAY, not just in the construction editor. Toggled in the wall editor.
    pub construction_dev_overlay: bool,
    /// Global illumination master switch (v0.571): when FALSE, the sun + fill directional lights are
    /// zeroed so a room is lit ONLY by local placed lights -- the "turn off GI and still see" test.
    /// Default true. Toggled in the wall editor.
    pub gi_enabled: bool,
    /// Sun-position override for the construction editor (operator: the real
    /// astronomical sun direction is tied to Earth's slow orbital drift + a
    /// FIXED ship position that never rotates, so there is no way to get a
    /// better lighting angle while editing -- "the sun is low on the horizon
    /// and I can't roll the ship to change that"). When true (and only while
    /// `construction_active`), the celestial-pass sun direction/color use
    /// `construction_sun_override_hour` via `TimeSystem::sun_direction`/
    /// `sun_color` instead of the real astronomical vector, so a bad real-world
    /// sun angle never blocks seeing your own build. Does not affect normal
    /// gameplay lighting. Default false.
    pub construction_sun_override: bool,
    /// Hour of day (0.0..24.0) used when `construction_sun_override` is on.
    /// Default noon (12.0) -- the best overhead angle for construction work.
    pub construction_sun_override_hour: f32,
    /// Max construction-editor undo steps (v0.575), Blender-style configurable depth. Set in the wall
    /// editor. Default 64; clamped 1..=4096.
    pub construction_undo_depth: usize,
    /// Set by the panel's Save button -> the engine writes the layout back to the RON.
    pub construction_save: bool,
    /// Index into the backdrop list (the names mirror is `showroom_backdrop_names`).
    pub showroom_backdrop: usize,
    /// Backdrop display names, mirrored from the loaded registry for the panel.
    pub showroom_backdrop_names: Vec<String>,
    /// Set true by the "Enter your home" button; the main loop consumes it to leave the
    /// showroom (write appearance/outfit to the player, save, switch to first-person).
    pub showroom_confirm: bool,
    /// The avatar appearance being edited (the live preview source). Synced to the ECS
    /// player Appearance on confirm so it persists in the save.
    pub appearance: crate::ecs::components::Appearance,
    /// Set when the appearance edits change; the main loop rebuilds the avatar mesh.
    pub appearance_dirty: bool,
    /// The equipped cosmetic outfit being edited in the wardrobe (synced to the ECS player
    /// on confirm). slot id -> cosmetic id.
    pub outfit: crate::ecs::components::Outfit,
    /// Set when the outfit changes; the main loop rebuilds the avatar.
    pub outfit_dirty: bool,
    /// Which showroom panel is shown: 0 = the Play picker, 1 = appearance editor
    /// (wetroom mirror), 2 = wardrobe (bedroom).
    pub showroom_mode: u8,
    /// True while the appearance editor was opened by the picker's "Edit look",
    /// so Done (and Esc) return to the picker instead of entering the world.
    /// Launching must never walk through editing (play-characters.md, section 3).
    pub showroom_return_to_picker: bool,
    /// Cosmetic catalog mirror for the wardrobe UI: (id, name, slot).
    pub cosmetics_list: Vec<(String, String, String)>,
    /// The GAME character's name being edited in the showroom (v0.448). DECOUPLED from the
    /// chat profile name (`profile_name` / `user_name`): a character is a local save, not
    /// your network identity. Synced to the player's ECS Name + saved on confirm.
    pub character_name: String,
    /// Whether settings were changed this frame (signals lib.rs to apply them).
    pub settings_dirty: bool,
    /// Request to quit the application.
    pub quit_requested: bool,
    /// Set to true when identity has been recovered from seed phrase and WS needs reconnect.
    pub identity_recovered: bool,
    /// The Ed25519 private key bytes (32 bytes) for signing, if available.
    pub private_key_bytes: Option<Vec<u8>>,
    /// Whether initial channel history has been fetched after connecting.
    pub history_fetched: bool,

    // ── Passphrase / key encryption state ──

    /// Whether a passphrase prompt is needed before the key is usable.
    pub passphrase_needed: bool,
    /// What mode the passphrase prompt is in.
    pub passphrase_mode: PassphraseMode,
    /// Input field for the passphrase.
    pub passphrase_input: String,
    /// Input field for confirming a new passphrase.
    pub passphrase_confirm: String,
    /// Input field for the old passphrase (change mode).
    pub passphrase_old_input: String,
    /// Status/error message for the passphrase prompt.
    pub passphrase_status: String,
    /// The encrypted private key (base64), persisted through save cycles.
    pub encrypted_private_key: String,
    /// The PBKDF2 salt (base64), persisted through save cycles.
    pub key_salt: String,
    /// PBKDF2 iteration count the current vault was encrypted with.
    /// Defaults to `PBKDF2_ITERATIONS_LEGACY` (100_000) for vaults written
    /// before v0.277.0; new encryptions set it to `PBKDF2_ITERATIONS_NEW`
    /// (600_000). The Unlock site re-encrypts to the new count on the
    /// next successful unlock — silent one-time migration per vault.
    pub key_iterations: u32,
    /// Per-section unlock state for `lockable_gate` (private sections like the
    /// Wallet). In memory only — never persisted, so a restart re-locks all.
    pub section_locks: std::collections::HashMap<String, crate::gui::widgets::LockState>,

    // ── v0.278.0 auto-unlock state ──
    /// User's chosen unlock mode: AlwaysPrompt / Keychain / KeychainPin.
    /// Default is AlwaysPrompt — opt-in is explicit.
    pub auto_unlock_mode: crate::auto_unlock::AutoUnlockMode,
    /// AES-GCM blob of the seed encrypted with `PIN ‖ device_key`. Empty
    /// when KeychainPin mode is not set up. Persisted in AppConfig.
    pub pin_encrypted_seed: String,
    /// PBKDF2 salt for the PIN-encrypted seed (base64). Empty when unset.
    pub pin_salt: String,
    /// "Remember on this device" checkbox state on the unlock modal —
    /// when true and the modal completes successfully, the seed is
    /// stashed in the OS keychain and `auto_unlock_mode` flips to
    /// `Keychain`. Resets to false after each modal close.
    pub remember_on_device: bool,
    /// True while the passphrase-unlock worker runs the 600k-iter PBKDF2 OFF
    /// the UI thread (v0.306.0). Drives the "Unlocking…" spinner + disables the
    /// button so the click can't re-fire. Previously the PBKDF2 ran inline on
    /// the click → ~200ms–1s UI freeze on every unlock.
    pub passphrase_unlocking: bool,
    /// Receiver for the background unlock result: `Ok((seed_bytes, optional
    /// (encrypted, salt, iters) when a legacy 100k vault was re-encrypted to
    /// 600k in the worker))`, or `Err(message)` for a wrong passphrase. Drained
    /// each frame by `draw_unlock`; the cheap post-steps (keychain stash,
    /// apply_pq_identity, save) then run on the main thread.
    #[cfg(feature = "native")]
    pub passphrase_unlock_rx: Option<
        std::sync::mpsc::Receiver<Result<(Vec<u8>, Option<(String, String, u32)>), String>>,
    >,
    /// True while the PIN-unlock worker runs `decrypt_seed_with_pin` (PBKDF2)
    /// off the UI thread (v0.307.0). Mirrors `passphrase_unlocking` for the PIN
    /// path (per-launch for KeychainPin users — the same freeze class).
    pub pin_unlocking: bool,
    /// Background PIN-unlock result: `Ok(seed_bytes)` or `Err(message)`.
    #[cfg(feature = "native")]
    pub pin_unlock_rx: Option<std::sync::mpsc::Receiver<Result<Vec<u8>, String>>>,
    /// In-flight clipboard-image upload (v0.307.0): `(target_channel, rx)` where
    /// rx yields the uploaded image URL or an error. The network POST runs on a
    /// worker thread so a big paste doesn't freeze the UI; the drain sends the
    /// chat message with the returned URL on the main thread (needs ws_client +
    /// the signing key).
    #[cfg(feature = "native")]
    pub clipboard_upload: Option<(String, std::sync::mpsc::Receiver<Result<String, String>>)>,
    /// PIN entry buffer (active digit-only field on the PinSetup /
    /// PinUnlock / PinChange modal forms).
    pub pin_input: String,
    /// Confirm-PIN entry buffer for PinSetup / PinChange.
    pub pin_confirm: String,
    /// Current-PIN entry buffer for PinChange.
    pub pin_old_input: String,
    /// Status/error text displayed under the PIN entry fields.
    pub pin_status: String,
    /// Full-PQ: our Kyber768 (ML-KEM-768) public key, base64. Derived
    /// deterministically from the BIP39 seed on recovery/unlock and
    /// advertised at identify; the secret re-derives from the seed on
    /// demand and is never stored. Replaces the old ECDH keypair.
    pub kyber_public_b64: String,
    /// Map of peer Dilithium pubkey hex -> their Kyber768 public base64.
    /// Populated from peer_list, full_user_list, profile_data, peer_joined.
    pub peer_kyber_keys: std::collections::HashMap<String, String>,
    /// Map of voice-channel NAME -> its numeric relay id (as a string). Populated
    /// from voice_channel_list (the id is i64 on the wire). Needed because the
    /// relay tracks voice rooms by numeric id, but the chat UI keys channels by
    /// name; this lets the join/leave send the correct room_id. (Phase C, v0.491.)
    pub voice_channel_ids: std::collections::HashMap<String, String>,
    /// Count of inbound voice Opus frames received (Phase C diagnostics, v0.492).
    /// Proves audio is flowing over the WebRTC pipe before playback (Phase D).
    pub voice_rx_frames: u64,
    /// The numeric id of the voice room we are currently joined to, if any
    /// (Phase C, v0.492). Drives the roster-based WebRTC offer logic.
    pub voice_active_room: Option<String>,
    /// Whether we have already offered to the incumbents present in our first
    /// roster after joining. Per the web's "newcomer offers, incumbents wait"
    /// rule, we offer to the peers present at our join, and let later joiners
    /// offer to us. Reset on each join. (Phase C, v0.492.)
    pub voice_incumbents_captured: bool,
    /// Peers whose voice WebRTC transport is connected (Phase D, v0.494). We send
    /// our captured mic Opus to each of these. Populated on VoiceConnected,
    /// cleared on Closed / leave.
    pub voice_connected_peers: std::collections::HashSet<String>,
    /// Previous "joined to a voice room" state, so lib.rs starts/stops the live
    /// voice session only on the edge. (Phase D, v0.494.)
    pub voice_session_prev: bool,
    /// Incoming 1:1 voice call ringing: `(peer pubkey hex, display name)`.
    /// Set when a `voice_call ring` arrives; the chat page renders the
    /// Accept / Decline modal from this. (v0.703 — closes the parity bug
    /// where a web caller rang a native user forever.)
    pub call_incoming: Option<(String, String)>,
    /// Active 1:1 call: `(peer pubkey hex, display name)`. While set, the
    /// live voice session runs (same pump as voice rooms) and the chat page
    /// shows the in-call bar with Hang up.
    pub call_active: Option<(String, String)>,
    /// Outgoing 1:1 call we placed, waiting for the peer to accept:
    /// `(peer pubkey hex, display name)`. On their `accept` we create the
    /// WebRTC offer and move to `call_active`; on `reject` / timeout we
    /// clear it. (v0.705 — native-initiated calls.)
    pub call_outgoing: Option<(String, String)>,
    /// Ring-out deadline: if the peer hasn't accepted by this instant we
    /// auto-cancel (matches the web's 30 s setTimeout). Not serialized.
    pub call_outgoing_deadline: Option<std::time::Instant>,
    /// Mic muted during a 1:1 call: the voice pump skips sending our Opus
    /// while true (the peer still reaches us; we just stop transmitting).
    pub call_muted: bool,
    /// Open file-attach picker modal (v0.708, in-app file browser).
    /// Some = the chat attach picker is open.
    pub chat_attach_picker: Option<crate::gui::widgets::file_browser::FilePickerState>,
    /// Settings > Media: the in-app picker for the ffmpeg executable
    /// (2026-09-18). Some = the picker is open.
    pub ffmpeg_picker: Option<crate::gui::widgets::file_browser::FilePickerState>,

    // ── Donation address config ──

    /// Admin-configurable Solana donation address (legacy).
    pub donate_solana_address: String,
    /// Admin-configurable Bitcoin donation address (legacy).
    pub donate_btc_address: String,
    /// Dynamic donation addresses configured LOCALLY in Settings (persisted to
    /// AppConfig -- this is a self-hosting operator's own list).
    pub donate_addresses: Vec<DonateAddress>,
    /// Donation addresses fetched from the CONNECTED server's GET /api/server-info
    /// `funding.addresses` (v0.659). Kept separate from `donate_addresses` on
    /// purpose: this list is never persisted (AppConfig ignores it), so a server's
    /// funding info can never clobber an operator's hand-configured local list,
    /// and it refreshes from the server on every connect. The Donate page prefers
    /// this list when non-empty. Previously only the web client fetched this;
    /// native's Donate page stayed empty for everyone except a self-hosting
    /// operator who had manually filled in Settings.
    pub donate_addresses_server: Vec<DonateAddress>,
    /// The connected server's funding goal, from `funding.goal_usd` +
    /// `funding.goal_label` (v0.659). Replaces the Donate page's old hardcoded
    /// fake "$350 / $1000" progress bar -- the card only renders when a real
    /// goal exists.
    pub donate_funding_goal: Option<(f64, String)>,
    /// Which server (trimmed URL) the two fields above came from. Donation info is
    /// MONEY-ROUTING data: server A's addresses must never be displayed while
    /// connected to server B (adversarial review, 2026-07-01), so the connect
    /// handler clears the fields whenever this doesn't match the new server, and
    /// `apply_server_funding` discards results that arrive tagged with a URL we
    /// are no longer connected to.
    pub donate_funding_server: String,
    /// In-flight GET {url}/api/server-info fetch spawned on connect (the
    /// `peer_list` handler in lib.rs), tagged with the trimmed server URL it was
    /// sent to (same style as `server_info_loader`); drained once per frame in
    /// the render loop via `apply_server_funding`. Replaced (old receiver
    /// dropped, its late send fails harmlessly) when a connect targets a
    /// different server, so a stalled fetch can't block future ones.
    pub donate_info_rx: Option<(String, std::sync::mpsc::Receiver<Result<ServerInfo, String>>)>,
    /// Temp fields for the "Add Address" form in settings.
    pub donate_new_network: String,
    /// Temp type for new address ("address" or "url").
    pub donate_new_type: String,
    /// Temp value for new address.
    pub donate_new_value: String,
    /// Temp label for new address.
    pub donate_new_label: String,

    // ── Chat user profile modal ──

    /// Whether the user profile modal is open.
    pub chat_user_modal_open: bool,
    /// Display name of the user shown in the modal.
    pub chat_user_modal_name: String,
    /// Public key of the user shown in the modal.
    pub chat_user_modal_key: String,

    /// "+ Add Server" modal (v0.187.0). Lets the user paste a relay URL
    /// (e.g. https://other-server.example) and connect to it. Maintains
    /// the previous server list rather than swapping — multi-server
    /// support is the eventual goal.
    pub show_add_server_modal: bool,
    pub add_server_url_draft: String,
    pub add_server_name_draft: String,
    /// Active tab index on the Server Settings page (v0.188.0).
    /// 0 = Overview (USER/MOD/ADMIN tiered sections).
    /// 1 = Channels (spreadsheet editor).
    /// 2 = Members (list with role + actions).
    /// 3 = Reports (placeholder for v0.189 mod review surface).
    pub server_settings_tab: u8,
    /// Per-channel-row draft state for the Channels spreadsheet — keyed
    /// by channel id, value is the in-flight edit (name, desc, flags).
    /// Saved when the user clicks the row's Save button.
    pub server_settings_channel_drafts: std::collections::HashMap<String, ChannelDraft>,
    /// Pending "new channel" row at the bottom of the Channels grid.
    pub server_settings_new_channel: ChannelDraft,

    // ── Create group modal (P2P signed-object groups, v0.295+) ──
    pub show_create_group_modal: bool,
    pub new_group_name: String,
    /// Create-group choice: share full message history with members who join
    /// later (signed into group_v1). false = private (default, forward secrecy).
    pub new_group_share_history: bool,
    /// Set after a successful create — the shareable invite ticket to copy.
    /// The modal flips into "share this ticket" mode while `Some`.
    pub create_group_ticket: Option<String>,
    /// Inline status/error for the create modal.
    pub create_group_status: String,

    // ── Join group modal ──
    pub show_join_group_modal: bool,
    pub join_group_invite_code: String,
    /// Inline status/error for the join modal.
    pub join_group_status: String,
    /// Set to the joined group's name after a successful join — the modal flips
    /// into a "✅ Joined …" confirmation view while `Some` so the user has
    /// visible feedback that it worked (instead of the modal silently closing).
    pub join_group_result: Option<String>,

    /// P2P (signed-object) groups the user is a member of — read-only cache of
    /// the relay's `/api/v2/groups` projection. Rendered in the left panel
    /// alongside legacy `chat_groups` during the migration. Refreshed after
    /// create/join and on explicit refresh.
    pub p2p_groups: Vec<crate::net::api_v2::P2pGroupInfo>,
    /// When the P2P-groups projection was last fetched — used to do a one-time
    /// fetch on first render so the list is populated without a manual action.
    pub p2p_groups_last_fetch: Option<std::time::Instant>,

    // ── Active P2P-group conversation (inline, channel-style) ──
    // A P2P group opens like a channel: clicking it sets
    // `chat_active_channel = "p2pgroup:<id>"` and its decrypted messages render
    // in the SAME center panel as channels/DMs (no modal). All network/crypto
    // for a group runs on a BACKGROUND THREAD (v0.303.0) so switching is
    // instant and the periodic refresh never freezes the UI — the worker sends
    // a `GroupLoad` back over a channel and the GUI applies it on the main
    // thread. These fields cache the applied result.
    /// Transient status line for the P2P-group invite action (e.g.
    /// "Invite copied"). Shown briefly in the group header / popup.
    pub p2p_group_invite_status: String,
    /// The group id currently open (matches the active `p2pgroup:<id>` channel).
    /// Empty when no P2P group is open.
    pub p2p_group_active_id: String,
    /// Current epoch number for the open group (used when sending).
    pub p2p_group_chat_epoch: u64,
    /// Decapsulated 32-byte AES key for the current epoch. None = we don't have
    /// a copy yet (no epoch issued, or it isn't sealed to us).
    pub p2p_group_chat_epoch_key: Option<Vec<u8>>,
    /// When we last KICKED OFF a background refresh of the open group — drives
    /// the periodic reload cadence.
    pub p2p_group_last_fetch: Option<std::time::Instant>,
    /// Roster index for the open group: author fingerprint → full pubkey hex
    /// (lets group messages reuse the standard identicon + name resolution).
    pub p2p_group_fp_to_key: std::collections::HashMap<String, String>,
    /// Roster index for the open group: author fingerprint → display name.
    pub p2p_group_fp_to_name: std::collections::HashMap<String, String>,
    /// In-flight background load for the open group: `(group_id, receiver)`.
    /// Drained each frame; applied if it still matches the active group.
    #[cfg(feature = "native")]
    pub p2p_group_loader:
        Option<(String, std::sync::mpsc::Receiver<crate::net::api_v2::GroupLoad>)>,
    /// In-flight background refresh of the whole P2P-group list (keeps the left
    /// rail + member counts fresh when membership changes on another client,
    /// and detects when the open group was disbanded/left elsewhere).
    #[cfg(feature = "native")]
    pub p2p_groups_list_loader:
        Option<std::sync::mpsc::Receiver<Vec<crate::net::api_v2::P2pGroupInfo>>>,
    /// True while a freshly-opened group is still loading (shows "Loading…"
    /// instead of the no-key/no-message hint for that brief window).
    pub p2p_group_loading: bool,
    /// inc-2: object_ids of group messages already handled over the WebRTC mesh
    /// (sent or received P2P), so a push + the 2s relay poll don't double-render
    /// the same message. Mirrors the web's `_p2pGroupSeenObjIds`. Cleared on
    /// group switch (in `spawn_group_load(fresh=true)`).
    #[cfg(feature = "native")]
    pub p2p_group_seen_obj_ids: std::collections::HashSet<String>,

    // ── Sidebar section settings popups (v0.195.0) ──
    // Rendered as floating Areas anchored below the section's cog
    // button. Using GuiState fields instead of egui's popup machinery
    // because the previous `popup_below_widget(... CloseOnClick ...)`
    // pattern self-closed on the trigger click — the popup flickered
    // on for one frame then disappeared (operator bug 2026-05-08).
    pub dm_settings_popup_open: bool,
    pub groups_settings_popup_open: bool,

    /// Notification preferences (v0.641, was unwired client-side despite the relay + web
    /// client already fully supporting it -- see `update_notification_prefs`/
    /// `get_notification_prefs`/`notification_prefs_data` in `src/relay/relay.rs`, mirrored by
    /// `web/pages/settings-app.js`). Defaults match the server's own column defaults
    /// (`notification_prefs` table) so a not-yet-fetched popup shows sensible values instead
    /// of a false "off." `notif_prefs_loaded` is false until the first real
    /// `notification_prefs_data` round-trips.
    pub notif_dm_enabled: bool,
    pub notif_mentions_enabled: bool,
    pub notif_tasks_enabled: bool,
    pub notif_dnd_start: Option<String>,
    pub notif_dnd_end: Option<String>,
    pub notif_prefs_loaded: bool,

    /// Privacy-tier UI transients (2026-08-23): loaded tier defs, the
    /// radio selection, and whether the first-connect chooser is open.
    /// The CHOSEN tier persists as settings.privacy_tier.
    /// Account export/erase UI transients (sovereignty, 2026-08-23).
    pub account_export_status: String,
    /// Worker-thread result for POST /api/account/export. None when no
    /// request is in flight. Never persisted: GuiState is not serialized.
    pub account_export_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    pub account_delete_confirm_input: String,
    pub privacy_tiers_cache: Vec<crate::gui::pages::privacy::PrivacyTier>,
    pub privacy_tier_selection: String,
    pub privacy_tier_prompt_open: bool,
    /// Local encrypted DM history for the ACTIVE server (sealed-sender
    /// cutover, 2026-08-23). The relay's mailbox is a delivery window
    /// that expires and carries no sender; this store is the archive.
    /// One per (identity, server) — reset on server switch.
    pub dm_store: Option<crate::net::dm_store::DmStore>,
    /// Whether we've sent the initial `dm_fetch` on this connection.
    pub dm_fetch_sent: bool,

    // ── Cosmos page state (v0.203.0, Phase 3) ──
    /// Which view the Cosmos page is currently rendering.
    pub cosmos_view: crate::gui::pages::cosmos::CosmosView,
    /// Pan offset (screen pixels). Updated by click-drag on the canvas.
    pub cosmos_pan: egui::Vec2,
    /// Zoom factor — 1.0 = default, > 1.0 = zoomed in, < 1.0 = zoomed out.
    /// Updated by mouse wheel scroll. Clamped in allocate_canvas.
    pub cosmos_zoom: f32,
    /// Currently selected body id in the System view (for the right-side
    /// details panel + map highlight). v0.203.2 — populated by clicking
    /// a body in the left-side browser sidebar OR clicking it on the map.
    pub cosmos_selected_body: Option<String>,
    /// Which planet groups are expanded in the body browser sidebar.
    /// Stored as ids of planets whose moon list is expanded. v0.203.2.
    pub cosmos_expanded_planets: std::collections::HashSet<String>,
    /// Pending focus request — when Some, the next System view render
    /// computes the pan needed to center this body on screen, then
    /// clears the request. Set by sidebar click or "Focus" button.
    /// v0.205.0 (operator pushback on zoom always centering on Sun).
    pub cosmos_focus_request: Option<String>,
    /// Body id the camera continuously follows across frames as it moves
    /// along its orbit, or `None` for no auto-follow. Unlike
    /// `cosmos_focus_request` (a one-shot snap-to), this re-centers every
    /// frame. Set by the body detail card's "Track"/"Stop Tracking" action;
    /// cleared automatically by any other Focus request (see
    /// `gui/pages/cosmos.rs`'s focus-consumption site).
    pub cosmos_tracked_body: Option<String>,
    /// 3D camera state for System view (Phase 4, v0.206.0).
    /// Yaw + pitch + distance + look-at target define the camera; mouse
    /// drag rotates, scroll zooms, sidebar click re-centers the target.
    pub cosmos_camera_3d: crate::gui::pages::cosmos::Cosmos3DCamera,

    /// Cosmos sim-time in seconds since the J2000.0 epoch
    /// (2000-01-01 12:00:00 UTC). Drives Kepler-evolved body positions.
    /// Initialized to "current real-world time" on first cosmos page
    /// open so the user immediately sees today's planetary configuration.
    /// v0.208.0.
    pub cosmos_sim_time_seconds: f64,
    /// Sim speed multiplier — 0 = paused, 1 = real-time (1 second sim =
    /// 1 second real), 86400 = 1 day per real second, etc. Negative
    /// values rewind. v0.208.0.
    pub cosmos_sim_speed: f64,
    /// Wall-clock Instant of the previous frame, used to compute dt for
    /// sim_time advancement. None on first frame. v0.208.0.
    #[allow(clippy::type_complexity)]
    pub cosmos_last_real_instant: Option<std::time::Instant>,
    /// Whether the cosmos sim_time has been initialized (sets to "now"
    /// on first cosmos page draw). v0.208.0.
    pub cosmos_sim_time_initialized: bool,
    /// Which body's pill is currently expanded into an info card on the
    /// 3D System view. Only one body can be expanded at a time — clicking
    /// a different body's pill swaps the expansion; clicking the same
    /// pill collapses it. Independent of `cosmos_selected_body` which
    /// drives the right-side details panel. v0.209.0.
    pub cosmos_expanded_body: Option<String>,
    /// Whether to render Lagrange-point overlay markers (L1-L5 for
    /// Sun-Earth, Earth-Moon, Sun-Mars, Sun-Jupiter, Sun-Saturn pairs).
    /// Off by default to keep the wide view clean. Toggled from the
    /// cosmos canvas overlay button. v0.211.0.
    pub cosmos_show_lagrange: bool,
    /// Whether to render reference-orbit rings (LEO/MEO/GEO/etc) around
    /// supported planets when zoomed close enough. Off by default —
    /// rings only appear when the user explicitly enables AND the camera
    /// is close enough that they're not microscopic on screen. v0.212.0.
    pub cosmos_show_reference_orbits: bool,
    /// Cached forward Sky-Events scan (Phase 4d-quad, v0.248). The scan
    /// is O(days × bodies²) so it must NOT run every frame like the
    /// instant detector — it's recomputed lazily when sim_time drifts
    /// far from `cosmos_upcoming_scan_origin`, throttled by
    /// `cosmos_upcoming_last_scan`. Each entry: when (sim seconds since
    /// J2000), human label, severity (0 info / 1 notable / 2 major).
    pub cosmos_upcoming_events: Vec<crate::gui::pages::cosmos::UpcomingSkyEvent>,
    /// sim_time the cached forward scan was computed at. Recompute when
    /// the live sim_time moves more than ~12h away from this.
    pub cosmos_upcoming_scan_origin: f64,
    /// Wall-clock instant of the last forward scan — throttles recompute
    /// so fast-forward / scrubbing can't trigger a scan every frame.
    pub cosmos_upcoming_last_scan: Option<std::time::Instant>,

    /// Cached server-wide settings received from the relay (v0.200.0).
    /// Populated on `server_settings_state` WS message. None means we
    /// haven't received the state yet (during initial connect, before
    /// any modify happens). UI uses defaults until populated.
    pub server_settings: Option<crate::relay::storage::ServerSettings>,
    /// All role definitions, from the relay's `role_list` WS broadcast
    /// (sent on connect + after any role change). Drives the user-modal
    /// role dropdown + badge colors. Empty until the first broadcast.
    /// v0.241 (roles Phase R2).
    pub chat_roles: Vec<crate::relay::storage::RoleDef>,
    /// Server→Services snapshot from the relay's `service_state` reply
    /// (admin-only; sent after `service_control` start/stop/refresh).
    /// Each entry: soft gate + live daemon active/enabled. Empty until
    /// the admin opens the Services panel (which sends a refresh).
    /// v0.262.16.
    pub service_state: Vec<crate::relay::services::ServiceInfo>,
    /// Per-role working copies for the Server Settings → Roles editor,
    /// keyed by role id. Seeded from `chat_roles` on first edit of a
    /// row; lets the operator tweak label/color/caps before pressing
    /// Save (which sends role_upsert). v0.242 (Phase R3).
    pub roles_drafts: std::collections::HashMap<String, crate::relay::storage::RoleDef>,
    /// The "add a custom role" form draft. id starts empty (operator
    /// types one). v0.242 (Phase R3).
    pub new_role_draft: crate::relay::storage::RoleDef,
    /// Currently-banned users for the Server Settings → Banned users
    /// admin panel. Populated by the `banned_list` WS message (only
    /// admins receive it). Empty until the panel requests it. v0.245.
    pub chat_banned_users: Vec<crate::relay::storage::BannedUser>,
    /// True once a `banned_list_request` has been sent this session so
    /// the panel doesn't re-request every repaint. Reset on disconnect.
    pub chat_banned_requested: bool,

    /// Backup files from the `backup_list` WS message (admin-targeted).
    /// Drives the Server Settings -> Backups panel. v0.938.
    pub backup_list: Vec<crate::relay::storage::backups::BackupEntry>,
    /// True once a `backup_list_request` was sent this session.
    pub backup_list_requested: bool,

    // ── Game admin: game-world bans, STRUCTURALLY SEPARATE from chat bans
    //    (v0.474). Free speech is a right (chat is never affected); playing on
    //    the shared MMO world is a privilege. These read the relay's
    //    `game_banned_keys` table, never `banned_keys`. See pages/game_admin.rs
    //    + docs/design/characters-and-servers.md.
    /// Players banned from the 3D game world only. Populated by the
    /// `game_banned_list` reply (admins only). Empty until requested.
    pub game_bans: Vec<crate::relay::storage::GameBan>,
    /// True once a `game_banned_list_request` was sent this session so the
    /// page doesn't re-request every repaint. Reset on disconnect.
    pub game_bans_requested: bool,
    /// The target public key typed into the Game Admin ban form.
    pub game_admin_target_key: String,
    /// The reason typed into the Game Admin ban form.
    pub game_admin_ban_reason: String,
    /// Last status / error line shown on the Game Admin page.
    pub game_admin_status: String,

    // ── The Play picker (docs/design/play-characters.md). The screen composes
    //    ONE pairing: a WHO (character) entering a WHERE (a local home world or
    //    a server world). Play repeats the last pairing, Characters always
    //    opens the picker. See pages/showroom.rs (mode 0).
    /// Local saves as (character, home) rows, rescanned each time the picker
    /// opens. Each save is a self-custodial home plus the character in it.
    pub launcher_homes: Vec<LauncherHome>,
    /// False until the picker has scanned the saves directory this opening.
    /// Reset to false every time the picker is opened so the rows are fresh
    /// (a save made in-session shows up).
    pub launcher_homes_loaded: bool,
    /// WHO: the save name the selected character lives in ("" = the "+ New
    /// character" row). A character IS its save until the character/world file
    /// split, so its home's name is the only stable id it has today.
    pub launcher_who: String,
    /// WHERE: which kind of world the picker has selected.
    pub launcher_where_kind: LauncherWhere,
    /// WHERE, home half: the selected save name ("" = a fresh homestead).
    pub launcher_selected_world: String,
    /// WHERE, server half: the selected server's id (or the virtual row id for
    /// the connection you are live on), so the card can describe it.
    pub launcher_selected_server: Option<String>,
    /// The WHO of the last successful Enter, persisted to
    /// `AppConfig.default_character`. Play replays it with `launcher_last_world`.
    pub launcher_last_character: String,
    /// The WHERE of the last successful Enter, persisted to
    /// `AppConfig.last_world` as "home:<save name>" or "server:<id>". Empty
    /// means no pairing yet, so Play opens the picker instead of entering.
    pub launcher_last_world: String,
    /// A save the picker asked to load; lib.rs applies it to the live player
    /// after the world loads, then clears this.
    pub launcher_pending_load: Option<String>,
    /// One-shot signal (v0.476): Play wants the picker (the showroom in mode
    /// 0). Distinguishes "Play -> show the picker" from "Esc -> plain
    /// first-person". lib.rs only opens the showroom when this is set, then
    /// clears it -- so Esc to FPS never surfaces the picker.
    pub launcher_open_select: bool,
    /// Set by the picker's "Back" button to cancel the showroom and return to
    /// the menu without entering the world (lib.rs handles it, same as Esc).
    pub showroom_cancel: bool,
    /// Cache of fetched server metadata (GET /api/server-info), keyed by server
    /// id, for the launcher's server-detail pane (v0.478). Avoids refetching.
    pub server_info_cache: std::collections::HashMap<String, ServerInfo>,
    /// In-flight server-info fetch: (server id, result channel). One at a time.
    pub server_info_loader: Option<(String, std::sync::mpsc::Receiver<Result<ServerInfo, String>>)>,
    /// Currently-muted users for the Server Settings → Muted users mod
    /// panel. Populated by the `muted_list` WS message (mods/admins
    /// only). v0.246.
    pub chat_muted_users: Vec<crate::relay::storage::MutedUser>,
    /// True once a `muted_list_request` has been sent this session.
    /// Reset on disconnect (mirrors chat_banned_requested).
    pub chat_muted_requested: bool,
    /// In-progress draft of server settings being edited in the admin
    /// UI. None = not editing. Cloned from `server_settings` when admin
    /// opens the editor. Save button sends a ServerSettingsUpdate WS
    /// message and clears the draft.
    pub server_settings_draft: Option<crate::relay::storage::ServerSettings>,

    // ── Channel edit modal ──
    pub show_channel_edit_modal: bool,
    pub edit_channel_id: String,
    pub edit_channel_name: String,
    pub edit_channel_description: String,
    /// Whether the delete confirmation is showing in the edit modal.
    pub edit_channel_confirm_delete: bool,
    /// Whether the slash commands help modal is visible.
    pub show_help_modal: bool,

    // ── Server settings page (mod / admin actions) ──
    /// Username target for kick/mute/ban/verify/promote actions.
    pub server_settings_target_user: String,
    /// Channel name input for create/delete/readonly actions.
    pub server_settings_channel_name: String,
    /// Last-generated invite code (shown after admin clicks "Generate invite").
    pub server_settings_invite_code: String,
    /// Draft input for "Redeem friend code" (Server Settings → User). (v0.722)
    pub redeem_code_draft: String,
    /// Draft input for "Revoke a device" key prefix (Server Settings → User). (v0.722)
    pub revoke_key_draft: String,
    /// Last action result message (success or error feedback).
    pub server_settings_status: String,
    /// Live server health snapshot (Server Settings → System health, v0.720).
    /// Read-only: /health + /api/stats of the connected server, fetched on a
    /// worker thread. In-app ops slice 1 native parity (docs/design/in-app-ops.md).
    pub system_health: Option<SystemHealth>,
    /// System-health fetch status line ("Loading…", error text, or empty).
    pub system_health_status: String,
    /// In-flight system-health fetch (worker thread → per-frame drain).
    pub system_health_rx: Option<std::sync::mpsc::Receiver<Result<SystemHealth, String>>>,
    // ── Relay Control Center (v0.846) ──
    /// Active detail tab in the Relay Control Center: 0 Health / 1 Control / 2 Config.
    pub relay_cc_tab: usize,
    /// The relay URL currently focused in the Relay Control Center left rail
    /// (defaults to the connected server on first open).
    pub relay_cc_selected: Option<String>,
    /// Rich admin health snapshot from the signed /api/admin/stats (Relay
    /// Control Center → Health). Only populated when the operator is admin.
    pub relay_admin_stats: Option<RelayAdminStats>,
    /// Admin-stats fetch status line ("Loading…", error text, or empty).
    pub relay_admin_stats_status: String,
    /// In-flight admin-stats fetch (worker thread → per-frame drain).
    pub relay_admin_stats_rx: Option<std::sync::mpsc::Receiver<Result<RelayAdminStats, String>>>,

    // ── In-app VPS console (v0.858): run server commands over SSH from the app ──
    /// The command being typed into the console input.
    pub vps_console_input: String,
    /// Accumulated console transcript (commands + their output), newest at the bottom.
    pub vps_console_output: String,
    /// In-flight command result (worker thread → per-frame drain): (label, output, ok).
    pub vps_console_rx: Option<std::sync::mpsc::Receiver<(String, String, bool)>>,
    /// True while a command is running, so the UI can show a spinner + block re-runs.
    pub vps_console_running: bool,

    /// Federated-server list (Server Settings → Federation, v0.722).
    pub federation_servers: Vec<FederationServerRow>,
    /// Federation panel status line ("Loading…", error text, or empty).
    pub federation_status: String,
    /// In-flight federation-list fetch (worker thread → per-frame drain).
    pub federation_rx: Option<std::sync::mpsc::Receiver<Result<Vec<FederationServerRow>, String>>>,
    /// Draft inputs for the Federation "Add server" row.
    pub federation_add_url_draft: String,
    pub federation_add_name_draft: String,
    /// Add-by-key drafts (NAT-friendly pairing: a home node with no public
    /// URL is added by its 64-hex Ed25519 federation key instead).
    pub federation_add_key_draft: String,
    pub federation_add_key_name_draft: String,
    /// Host-a-node autostart (operator field test 2: "I can't seem to
    /// reconnect to my self-relay when I restart the app" -- the node was
    /// simply not running). Set when the node is started, cleared on Stop;
    /// the saved port/db/name reproduce the exact node at next launch.
    pub host_node_autostart: bool,
    pub host_node_port: String,
    pub host_node_db: String,
    pub host_node_name: String,
    /// Whether the danger-zone confirm-delete prompt is showing.
    pub server_settings_confirm_action: Option<String>,

    // ── Debug console state ──

    /// Whether the F12 debug console overlay is visible.
    pub debug_console_visible: bool,
    /// Ring buffer of timestamped debug log lines for the overlay.
    pub debug_log: Vec<String>,

    // ── External catalog (loaded from data/external/catalog.json): free
    //    software plus real-world help services, rendered by the Tools page ──
    pub tools_catalog: Vec<ToolEntry>,

    // ── Page taxonomies (Infinite-of-X migrations, v0.123.0) ──
    /// Equipment slots for the inventory page (`data/inventory/equipment_slots.json`).
    pub equipment_slots: Vec<(String, String)>,
    /// Bug-report severity labels (`data/bugs/taxonomy.json`).
    pub bug_severities: Vec<String>,
    /// Bug-report category labels (`data/bugs/taxonomy.json`).
    pub bug_categories: Vec<String>,
    /// Crafting category filters (`data/crafting/categories.json`).
    pub crafting_category_groups: Vec<CraftCategoryGroup>,
    /// Marketplace category vocabulary (`data/market/categories.json`).
    pub market_categories: Vec<MarketCategory>,
    /// In-app Library: sections of nested categories holding documents
    /// (`data/library/`). Documents only since v0.1063.
    pub library: Vec<LibrarySection>,
    /// Library tag vocabulary (`data/library/tags.json`, carried through
    /// index.json). Drives the filter chips on the Library page.
    pub library_tags: Vec<LibraryTagGroup>,
    /// The syllabus (`data/curriculum/syllabus.json`): every subject a person
    /// needs, with the state of each topic's four layers. The Library renders
    /// it as a third view beside Documents and Dictionary.
    pub curriculum: CurriculumData,
    /// Studio scene presets (`data/studio/scenes.json`).
    pub studio_scene_presets: Vec<StudioScenePreset>,
    /// Studio source presets (`data/studio/sources.json`).
    pub studio_source_presets: Vec<StudioSourcePreset>,
    /// Studio streaming pickers (`data/studio/streaming_config.json`).
    pub studio_streaming_config: StudioStreamingConfig,
    /// Donate page FAQ entries (`data/donate/faq.json`).
    pub donate_faq: Vec<DonateFaqEntry>,
    /// Direct-support donation methods (`data/donate/methods.json`).
    pub donate_methods: Vec<DonateMethod>,
    /// Endorsed charities (`data/donate/charities.json`).
    pub donate_charities: Vec<DonateCharity>,
    /// QA test tasks (`data/testing/qa_tasks.json`) shown on the Testing page.
    pub qa_test_tasks: Vec<QaTestTask>,
    /// Per-task local status: id → "passed" / "issue" / "" (untouched).
    pub qa_test_status: std::collections::HashMap<String, String>,
    /// Per-task draft note (when typing into Report Issue field).
    pub qa_test_note: std::collections::HashMap<String, String>,
    /// Filter chip on the Testing page: "all" / category id.
    pub qa_test_filter: String,
    /// The websites database (`data/web/sites.json`): categories + one record
    /// per site with its embedding-legality and affiliate fields. Shown on the
    /// Browser page; the web mirror (web/pages/web.html) reads the same file.
    pub web_sites: WebSites,
    /// Filter chip on the Browser page: "all" / category id.
    pub browser_filter: String,
    /// The readable web view (widgets/web_view.rs): history, the in-flight
    /// fetch and the page on screen. Only used when `settings.readable_web`.
    pub web_view: widgets::web_view::WebViewState,
    /// Preview opt-in: render the new two-tier nav (Reality / Sim / Tools /
    /// Settings + sub-pages) instead of the legacy single-row nav. Toggled
    /// from the [≡] / [▤] button in the nav itself. Not persisted yet so
    /// each launch starts on the legacy nav until operator picks a winner.
    pub nav_two_tier: bool,
    /// Active top-tier category id when nav_two_tier is on.
    /// One of: "reality", "sim", "tools", "settings".
    pub nav_top_category: String,
    /// True when the player is taking damage / under attack — flips the
    /// nav RGB separator from cyclic spectrum to a pulsing red so the
    /// player can tell mid-menu without sound. Set by combat / damage
    /// systems; cleared after a short cooldown.
    pub attack_pulse_active: bool,
    /// game_time when attack_pulse_active was last set; used to auto-clear
    /// after a few seconds of no new damage events.
    pub attack_pulse_last_hit_at: f64,
    /// Death & recovery (v0.745, loop-map rung 1): Some(cause) while the
    /// player is dead — the death screen overlay shows it ("starvation",
    /// "suffocation", an effect name...). Cleared by respawn.
    pub player_death_cause: Option<String>,
    /// The death screen's Respawn button; lib.rs performs the respawn
    /// (teleport to spawn, reset vitals, remove Dead) and clears it.
    pub pending_respawn: bool,
    // v0.197.0: ai_usage_filters removed (AI Usage page deleted).
    // v0.415.0: onboarding_concepts + onboarding_core_pages removed with the
    // standalone onboarding page. NOTE (audit 2026-07-30): the claim that "the
    // web /onboarding page still reads the JSON files" is NOT true, and may
    // never have been. web/pages/onboarding.html fetches quests.json only, so
    // data/onboarding/core_pages.json and core_concepts.json are read by
    // nothing on either client. They are good plain-language content and are
    // kept as the source for the in-app documentation item in PRIORITIES,
    // rather than deleted, but nothing renders them today.

    // ── Universal help modal (loaded from data/help/topics.json) ──
    /// Registry of help topics. Populated at startup from data/help/topics.json.
    pub help_registry: crate::gui::widgets::help_modal::HelpRegistry,
    /// ID of the currently-open help topic, if any. Setting this opens the help modal.
    pub active_help_topic: Option<String>,

    // ── Onboarding quest chains (loaded from data/onboarding/quests.json) ──
    /// Quest chains displayed on the Onboarding page.
    pub onboarding_quest_chains: Vec<crate::gui::pages::onboarding::QuestChain>,
    /// Map of "chain_id:step_id" -> done?. Persisted via
    /// `AppConfig::onboarding_quest_progress`, written the moment a step is
    /// ticked. (This comment claimed persistence from v0.415 until v0.1066
    /// while no such config field existed and every tick was discarded on
    /// exit; the field is real now.)
    pub onboarding_quest_progress: std::collections::HashMap<String, bool>,

    // ── Inline image cache (for chat attachments) ──
    /// Fetches, decodes, and caches images referenced in chat messages so
    /// they render inline instead of as raw /uploads/... text.
    pub image_cache: crate::gui::widgets::image_cache::ImageCache,
    /// URL of the image currently shown full-screen in the viewer modal.
    /// `None` means the modal is closed.
    pub image_viewer_url: Option<String>,

    // ── Studio state ──
    pub studio: StudioState,

    // ── Chat panel collapse state (persisted in config) ──
    pub chat_connection_collapsed: bool,
    pub chat_dm_collapsed: bool,
    pub chat_groups_collapsed: bool,
    pub chat_servers_collapsed: bool,
    /// COMMONS section (bridged federated rooms) collapse state.
    pub chat_commons_collapsed: bool,
    /// Per-server section collapse (normalized URLs). Persisted: at many
    /// servers (the operator is on 36 Discord servers), collapsed sections
    /// are how the list stays scannable.
    pub chat_server_sections_collapsed: std::collections::HashSet<String>,
    /// URL of the server row currently being dragged for reordering, if any.
    /// Session-only; the resulting ORDER is what persists (saved_servers).
    pub server_drag: Option<String>,
    /// The "what is the Commons?" explainer modal (question mark on the
    /// COMMONS section header).
    pub show_commons_info: bool,
    pub chat_connected_server_collapsed: bool,
    pub chat_friends_collapsed: bool,
    pub chat_members_collapsed: bool,
    /// Collapse state of the Studio quick-access section in the chat right rail.
    pub chat_studio_collapsed: bool,
    /// Set by the winit layer (src/lib.rs window_event) when Ctrl+V is
    /// pressed on the Chat page. The chat page reads + clears this each
    /// frame and, if an image is on the clipboard, uploads it. Needed
    /// because egui-winit swallows Ctrl+V (translates to Event::Paste
    /// text-only, returns before emitting the V key event) so egui's
    /// input layer never sees Ctrl+V for an image clipboard. v0.234.
    pub pending_clipboard_paste: bool,
    /// Highlighted row in the @mention autocomplete popup. Up/Down arrows
    /// move it; Enter / click / hover select. Reset to 0 whenever the
    /// match set changes. v0.235.
    pub chat_mention_index: usize,
    /// Which message's reaction-popup is currently open (timestamp_ms key).
    /// Popups open only on Þ hover; the popup_hovered gate (sticky-on-popup)
    /// is only honored once a popup is actually open for that message.
    /// This prevents the reaction popup from opening when the user just
    /// hovers the message text right of the pill (operator feedback
    /// 2026-05-12 - "if I mouse over the text of a reply the reaction
    /// pill comes up even though I never clicked on the Þ"). v0.229.
    pub chat_open_popup_ts: Option<u64>,
    /// How many DM conversations to show (3, 5, 10, or 0 = all)
    pub chat_dm_display_limit: usize,

    // ── Chat panel resize/lock state ──
    pub chat_left_panel_locked: bool,
    pub chat_right_panel_locked: bool,
    pub chat_left_panel_width: f32,
    pub chat_right_panel_width: f32,

    // ── Identity / Governance / Recovery page state (v0.115.0) ──
    /// DID being looked up on the Identity page.
    pub identity_lookup_did: String,
    /// Set to true when the Identity page wants to issue a fresh fetch.
    pub identity_lookup_pending: bool,
    /// Active scope tab on the Governance page (0=All, 1=Local, 2=Civilization).
    pub governance_scope_tab: usize,
    // ── Governance live data (v0.660) ──
    /// Proposals joined from GET /api/v2/proposals + each object's payload +
    /// tally, loaded on a background thread (see pages/governance.rs).
    pub governance_proposals: Vec<crate::gui::pages::governance::ProposalView>,
    /// In-flight proposal fetch, tagged with the server URL it targets so a
    /// late result from a previous server is discarded (same staleness rule as
    /// `donate_info_rx`).
    pub governance_rx: Option<(String, std::sync::mpsc::Receiver<Result<Vec<crate::gui::pages::governance::ProposalView>, String>>)>,
    /// Which server URL `governance_proposals` BELONGS TO (the data-origin tag,
    /// same rule as `donate_funding_server`): when it doesn't match the current
    /// server, the page clears the list immediately -- another server's
    /// proposals must never render or take votes.
    pub governance_fetched_for: String,
    /// Set to request a refetch (Refresh button; after a vote/proposal lands so
    /// the tally updates). Cleared ONLY when a fetch spawns, never by a fetch
    /// completing -- so an invalidation raised while a fetch was already in
    /// flight still triggers its own refetch instead of being clobbered.
    pub governance_refresh: bool,
    /// Last proposal-list fetch error, shown on the page.
    pub governance_error: String,
    /// Votes cast THIS SESSION: proposal_id -> choice. Server-side votes are
    /// final (INSERT OR IGNORE on (proposal, voter DID)), so this only needs to
    /// stop double-submits and relabel the row; it intentionally doesn't try to
    /// reconstruct votes from earlier sessions.
    pub governance_my_votes: std::collections::HashMap<String, String>,
    /// In-flight vote submission; Ok carries (proposal_id, choice) back.
    pub governance_vote_rx: Option<std::sync::mpsc::Receiver<Result<(String, String), String>>>,
    /// Status line for the last vote/proposal submission.
    pub governance_vote_status: String,
    /// In-flight proposal submission.
    pub governance_propose_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    /// Whether the new-proposal form is open.
    pub governance_show_propose: bool,
    /// New-proposal form fields.
    pub governance_new_title: String,
    pub governance_new_body: String,
    pub governance_new_type_idx: usize,
    pub governance_new_scope_idx: usize,
    pub governance_new_days: f32,
    // ── Laws page (v0.496) ──
    /// Selected jurisdiction id ("silverdale", "usa", ...). Empty => default to
    /// the most-local jurisdiction in the data on first draw.
    pub laws_location: String,
    /// Free-text search on the Laws page.
    pub laws_search: String,
    /// Kind filter tab (0=All, 1=HumanityOS base, 2=Real laws).
    pub laws_filter_tab: usize,
    /// Selected category chip on the Laws page ("" = all). Categories come from
    /// `data/laws/laws.json`'s own `categories` list (v0.661 -- loaded since
    /// v0.496 but never surfaced in the UI until now).
    pub laws_category: String,
    /// Active filter tab on the Governance page (0=Open, 1=All).
    pub governance_filter_tab: usize,
    /// DID being looked up on the Recovery page.
    pub recovery_lookup_did: String,
    /// Set to true when the Recovery page wants to fetch a setup.
    pub recovery_lookup_pending: bool,
    /// Guardian DID being looked up on the Recovery page.
    pub recovery_guardian_did: String,
    /// Set to true when the Recovery page wants to fetch held shares.
    pub recovery_guardian_pending: bool,

    // v0.197.0: AI Usage page form state removed (page deleted).
}

#[cfg(feature = "native")]
impl GuiState {
    /// Remember the pairing that was just entered, so Play can repeat it
    /// (docs/design/play-characters.md, open question 1: automatic last
    /// pairing, no manual default toggle). The caller persists the config.
    pub fn record_pairing(&mut self) {
        self.launcher_last_character = self.launcher_who.clone();
        self.launcher_last_world = match self.launcher_where_kind {
            // "home:" with an empty name is the fresh homestead, which is a
            // real pairing: the empty STRING is what means "nothing recorded".
            LauncherWhere::Home => format!("home:{}", self.launcher_selected_world),
            LauncherWhere::Server => match &self.launcher_selected_server {
                Some(id) => format!("server:{id}"),
                None => String::new(),
            },
            // Closed Net holds its own characters; nothing local to replay.
            LauncherWhere::ClosedNet => String::new(),
        };
    }

    /// Restore the last pairing into the picker's selection and ask lib.rs to
    /// load its character. Returns false when there is nothing to repeat (first
    /// run, or a pairing shape this build no longer understands), which is the
    /// caller's signal to open the picker instead.
    pub fn apply_last_pairing(&mut self) -> bool {
        let last = self.launcher_last_world.clone();
        if let Some(id) = last.strip_prefix("server:") {
            if id.is_empty() {
                return false;
            }
            self.launcher_where_kind = LauncherWhere::Server;
            self.launcher_selected_server = Some(id.to_string());
            // Shared: the co-presence gate joins the server world.
            self.copresence_solo = false;
        } else if let Some(world) = last.strip_prefix("home:") {
            self.launcher_where_kind = LauncherWhere::Home;
            self.launcher_selected_world = world.to_string();
            // Solo: no game_join, no avatar in the shared world.
            self.copresence_solo = true;
        } else {
            return false;
        }
        self.launcher_who = self.launcher_last_character.clone();
        if !self.launcher_who.is_empty() {
            self.launcher_pending_load = Some(self.launcher_who.clone());
        }
        true
    }

    /// Full-PQ: derive the Dilithium3 identity + Kyber768 DM key from
    /// the in-memory BIP39 seed (`private_key_bytes`) and force a clean
    /// reconnect so `identify` re-advertises `kyber_public`. Idempotent;
    /// a no-op if the seed isn't unlocked. MUST be called from every
    /// path that puts the seed into memory (passphrase unlock, seed
    /// recovery, legacy plaintext load) — otherwise the client has its
    /// persisted Dilithium identity but NO Kyber key, so it can neither
    /// send nor receive encrypted DMs (it never advertises a key and
    /// `try_encrypt_dm` fails `no_own_key`).
    pub fn apply_pq_identity(&mut self) {
        let seed = match self.private_key_bytes.as_ref() {
            Some(s) => s.clone(),
            None => return,
        };
        match crate::net::identity::derive_pq_identity(&seed) {
            Ok(pq) => {
                self.profile_public_key = pq.dilithium_hex;
                self.kyber_public_b64 = pq.kyber_public_b64;
                // Force a clean reconnect: drop the socket and clear the
                // reconnect guards so the auto-connect path re-runs and
                // sends `kyber_public` at identify. Without this the
                // relay never learns our Kyber key and no peer can seal
                // a DM to us.
                self.ws_client = None;
                self.ws_manually_disconnected = false;
                self.ws_reconnect_timer = 0.0;
                self.ws_reconnect_attempts = 0;
                // Parked connections were identified under the PREVIOUS
                // identity; their sockets are stale the moment the key
                // changes. Drop them all (closing the sockets) -- they
                // reconnect fresh when visited or, later, by the
                // connect-to-all pass.
                self.connections.clear();
                // Governance vote tracking is PER IDENTITY (adversarial review
                // 2026-07-01): a different seed is a different voter DID, so the
                // previous identity's session votes must not label rows or
                // suppress vote buttons for this one.
                self.governance_my_votes.clear();
                self.governance_vote_rx = None;
                self.governance_vote_status.clear();
                let kp = &self.profile_public_key[..16.min(self.profile_public_key.len())];
                log::info!("PQ identity applied (Dilithium {kp}…); reconnecting to advertise Kyber");
            }
            Err(e) => log::error!("apply_pq_identity: PQ derivation failed: {e}"),
        }
    }

    /// Navigate to a sub-page, pushing the CURRENT page onto the back
    /// stack so Escape returns there. Use this for contextual openings
    /// (cog → ServerSettings, message → details modal, etc.). For
    /// peer-level navigation (clicking a top-tier nav button), set
    /// `active_page` directly and call `clear_nav_back` to drop the
    /// stack — those navigations don't nest.
    pub fn push_nav_to(&mut self, target: GuiPage) {
        // Avoid pushing duplicate top-of-stack — repeatedly opening the
        // same sub-page from the same parent shouldn't bury the parent
        // under N copies of itself.
        if self.nav_back_stack.last() != Some(&self.active_page) {
            self.nav_back_stack.push(self.active_page);
        }
        self.active_page = target;
    }

    /// Pop the back stack and switch to that page. Returns true if a
    /// page was popped (caller can decide what to do if false — e.g.
    /// fall through to "Esc closes menu" behavior at the root level).
    pub fn pop_nav_back(&mut self) -> bool {
        if let Some(prev) = self.nav_back_stack.pop() {
            self.active_page = prev;
            true
        } else {
            false
        }
    }

    /// Drop the back stack — used when navigating laterally (e.g.
    /// clicking a top-tier nav button) so the user doesn't end up with
    /// a stack of unrelated pages from earlier sessions.
    pub fn clear_nav_back(&mut self) {
        self.nav_back_stack.clear();
    }

    /// Push a confirmation toast (v0.861, lives `TOAST_LIFE`). `now` is the current egui time
    /// (`ui.ctx().input(|i| i.time)`). Use this after any save/apply so the action
    /// is never silent -- e.g. `state.toast("Theme saved", ToastKind::Success, now)`.
    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind, now: f64) {
        // Cap the stack so a rapid-fire loop can't grow it unbounded.
        if self.toasts.len() > 6 {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast { text: text.into(), created: now, kind, life: TOAST_LIFE });
    }

    /// A toast the player has to READ, not just notice: an Info toast that
    /// stays up for `NOTICE_LIFE` seconds instead of the 2.6 s confirmation.
    pub fn notice(&mut self, text: impl Into<String>, now: f64) {
        if self.toasts.len() > 6 {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast { text: text.into(), created: now, kind: ToastKind::Info, life: NOTICE_LIFE });
    }

    /// True while an in-world MODAL PANEL is open (the interactive chat panel,
    /// the walk-up creature editor, or the NPC dialogue card). THE single
    /// predicate for the modal input plumbing in lib.rs -- reconcile_cursor's
    /// want_free, the mouse-look gate, the keyboard guard, the mouse-button
    /// guard, and the wheel gate all call this, so a future in-world modal is
    /// a one-line addition HERE instead of a five-site scavenger hunt (missing
    /// one site re-introduces the "typing 'i' opens the inventory" class of
    /// bug). (v0.779; NPC talk card joined v0.797)
    pub fn in_world_modal_open(&self) -> bool {
        self.chat_input_active || self.dev_edit_target.is_some() || self.npc_talk_target.is_some()
    }

    /// The F10 Cloud dev sidebar is open AND expanded (2026-09-05). This is
    /// the "hold Alt" condition made sticky: lib.rs frees the OS cursor and
    /// suppresses mouse-look while it is true (reconcile_cursor + the
    /// DeviceEvent::MouseMotion gate, the same two sites `alt_held` uses),
    /// so the operator can scroll and click the panel without holding Alt.
    /// It is deliberately NOT part of in_world_modal_open: that gate also
    /// swallows key presses (F10 itself, movement keys, F9 flight), and the
    /// operator flies around while flipping these switches. Collapsed or
    /// closed = false = the cursor state the game would have anyway.
    pub fn cloud_dev_sidebar_expanded(&self) -> bool {
        self.show_cloud_dev_panel && !self.cloud_dev_collapsed
    }

    /// THE dev-tooling gate (task #50): the play mode must grant DevTools
    /// (mode == Dev) AND the Settings "Developer cheats" switch must be on.
    /// Every dev affordance funnels through this one predicate -- the Dev
    /// page (spawn/travel), the G walk-up creature editor, the "Dev:"
    /// provisioning buttons (stock materials/seeds, grow all, max skills),
    /// and the fly/FTL per-frame sync -- so the surfaces can't drift apart.
    /// Both flags stay independently togglable (forever-dev norm: the mode
    /// is the master, cheats_enabled the extra kill-switch for demos).
    pub fn dev_cheats_active(&self, theme: &theme::Theme) -> bool {
        self.settings.play_mode.allows(crate::config::Capability::DevTools)
            && theme.cheats_enabled
    }

    /// Close every in-world modal panel (Esc, death, or a mode change). One
    /// list so a close path can't drift out of sync with a panel added later.
    pub fn close_in_world_modals(&mut self) {
        self.chat_input_active = false;
        self.chat_input_focus_pending = false;
        self.dev_edit_target = None;
        self.dev_edit_snapshot_pending = false;
        self.npc_talk_target = None;
    }

    /// Advance the open NPC dialogue card to its next line (v0.797). Shared by
    /// the card's More button and the repeat-E press so the two paths can never
    /// cycle differently. Cycles dialog[]; an NPC that shipped only greetings
    /// cycles those instead, so More always does SOMETHING when lines exist.
    pub fn npc_talk_advance(&mut self) {
        let lines = if self.npc_talk_dialog.is_empty() {
            &self.npc_talk_greetings
        } else {
            &self.npc_talk_dialog
        };
        let Some((i, line)) = crate::net::sync::next_dialog_line(lines, self.npc_talk_index)
        else {
            return; // nothing to say: keep whatever the card shows
        };
        let line = line.to_string();
        self.npc_talk_index = Some(i);
        self.npc_talk_line = line;
    }
}

#[cfg(feature = "native")]
impl Default for GuiState {
    fn default() -> Self {
        Self {
            active_page: GuiPage::MainMenu,
            last_page: GuiPage::Chat,
            nav_back_stack: Vec::new(),
            chat_input_active: false,
            chat_input_focus_pending: false,
            ingame_chat_mode: IngameChatMode::Channels,
            hud_chat_feed_visible: true,
            ingame_chat_panel_height: 160.0,
            copresence_active: false,
            copresence_names: Vec::new(),
            pending_dev_spawn: None,
            pending_dev_despawn_creatures: false,
            dev_spawn_filter: String::new(),
            dev_creature_count: 0,
            planet_tuner: Default::default(),
            dev_edit_target: None,
            dev_edit_snapshot_pending: false,
            dev_edit_name: String::new(),
            dev_edit_health: 100.0,
            dev_edit_health_max: 100.0,
            dev_edit_hostile: false,
            dev_edit_behavior_orig: String::new(),
            dev_edit_tint: [0.7, 0.6, 0.5],
            dev_edit_scale: 0.5,
            dev_edit_species: String::new(),
            pending_dev_edit_despawn: false,
            pending_dev_teleport: None,
            pending_dev_surface: None,
            dev_fly_mode: false,
            dev_hover: false,
            dev_fly_speed_mult: 1.0,
            dev_travel_away: false,
            show_hud: true,
            link_device_qr_show: false,
            link_device_qr: None,
            settings: SettingsState::default(),
            chat_input: String::new(),
            chat_typing_users: std::collections::HashMap::new(),
            chat_typing_last_sent: None,
            chat_reply_to: None,
            chat_messages: Vec::new(),
            chat_search_open: false,
            chat_search_query: String::new(),
            chat_search_results: Vec::new(),
            chat_pins: std::collections::HashMap::new(),
            chat_pins_open: false,
            chat_edit_target: None,
            chat_sent_timestamps: Vec::new(),
            chat_channels: Vec::new(),
            chat_active_channel: "general".to_string(),
            chat_users: Vec::new(),
            chat_dms: Vec::new(),
            chat_servers: Vec::new(),
            chat_friends: Vec::new(),
            chat_following_keys: std::collections::HashSet::new(),
            chat_followers: std::collections::HashSet::new(),
            ws_client: None,
            connections: Vec::new(),
            ws_status: "Not connected".to_string(),
            #[cfg(feature = "native")]
            webrtc: None,
            #[cfg(feature = "native")]
            webrtc_test_peer: None,
            ws_manually_disconnected: false,
            ws_reconnect_timer: 0.0,
            ws_reconnect_delay: crate::net::ws_client::RECONNECT_DELAY_INITIAL_SECS,
            ws_reconnect_attempts: 0,
            ws_rate_limited: false,
            selected_slot: None,
            garden_selection: None,
            trees_start_collapsed: true,
            fps: 0.0,
            updater: crate::updater::Updater::new(VERSION),
            update_toast_visible: false,

            onboarding_complete: false,
            onboarding_step: 0,
            server_url: "https://united-humanity.us".to_string(),
            connected_server_url: String::new(),
            server_connected: false,
            server_check_rx: None,
            history_rx: None,
            server_check_error: String::new(),
            user_name: "Player".to_string(),
            concept_tour_seen: false,
            default_page: GuiPage::Humanity,

            // Task board defaults
            tasks: Vec::new(),
            task_next_id: 1,
            task_search: String::new(),
            task_filter_priority: None,
            task_filter_assignee: String::new(),
            task_show_new_form: false,
            task_new_title: String::new(),
            task_new_description: String::new(),
            task_new_priority: TaskPriority::Medium,
            task_new_assignee: String::new(),

            // Profile defaults
            profile_name: String::new(),
            profile_bio: String::new(),
            profile_public_key: String::new(),
            profile_section: ProfileSection::Identity,
            profile_height: String::new(),
            profile_weight: String::new(),
            profile_eye_color: String::new(),
            profile_blood_type: String::new(),
            profile_hair_color: String::new(),
            profile_hair_length: String::new(),
            profile_hair_style: String::new(),
            profile_hair_texture: String::new(),
            profile_neck: String::new(),
            profile_shoulders: String::new(),
            profile_chest: String::new(),
            profile_waist: String::new(),
            profile_hips: String::new(),
            profile_thighs: String::new(),
            profile_inseam: String::new(),
            profile_shoe_size: String::new(),
            profile_shirt_size: String::new(),
            profile_pants_size: String::new(),
            profile_pronouns: String::new(),
            profile_location: String::new(),
            profile_website: String::new(),
            profile_private_notes: String::new(),
            profile_network_name: String::new(),
            profile_network_bio: String::new(),
            profile_network_avatar: String::new(),
            profile_directory_listed: true,
            machine_labels: Vec::new(),
            sight_blockers: Vec::new(),
            targeted_machine: None,
            vehicle_prompt: String::new(),
            livestock_prompt: String::new(),
            livestock_notice: String::new(),
            livestock_notice_at: 0.0,
            targeted_control_panel: None,
            control_panel_prompt: String::new(),
            selected_machine: None,
            machine_card_recipe: None,
            machine_card_recipe_options: Vec::new(),
            machine_card_recipe_pending: None,
            machine_card_container: None,
            machine_card_take_pending: false,
            machine_card_storable: Vec::new(),
            machine_card_store_pending: None,
            machine_card_vendor: false,
            vendor_open: false,
            vendor_goods: Vec::new(),
            vendor_status: String::new(),
            wallet_credits: 0,
            pending_vendor_buy: None,
            pending_vendor_sell: None,
            pending_equip: None,
            pending_unequip: None,
            equip_status: String::new(),
            room_bounds: Vec::new(),
            reveal_held: false,
            machine_label_dot_dist: 21.0,
            machine_label_name_dist: 13.0,
            machine_label_card_dist: 8.0,
            crew_labels: Vec::new(),
            target_markers: Vec::new(),
            targeted_npc: None,
            npc_prompt: String::new(),
            npc_talk_target: None,
            npc_talk_name: String::new(),
            npc_talk_activity: String::new(),
            npc_talk_line: String::new(),
            npc_talk_dialog: Vec::new(),
            npc_talk_greetings: Vec::new(),
            npc_talk_index: None,
            profile_network_saved_note: String::new(),
            profile_interests: Vec::new(),
            profile_interest_input: String::new(),
            // Populated from data/skills/default_profile.json at startup
            // (see lib.rs `load_default_player_skills`). Empty at construction.
            profile_skills: Vec::new(),
            profile_social_links: Vec::new(),
            profile_social_platform: String::new(),
            profile_social_url: String::new(),
            profile_streaming_url: String::new(),
            profile_streaming_live: false,

            // Map defaults: populated from the cosmos catalog at startup in
            // lib.rs (see `load_planets`). Empty at construction.
            map_planets: Vec::new(),
            places: Vec::new(),
            homestead_design: None,
            homestead_loops: Vec::new(),
            tower_configs: Vec::new(),
            garden_areas: Vec::new(),
            grow_media: Vec::new(),
            garden_irrigation: std::collections::HashMap::new(),
            garden_nutrient: std::collections::HashMap::new(),
            placed_items: Vec::new(),
            pending_inventory_transfers: Vec::new(),
            pending_take_origins: Vec::new(),
            inflight_take_origins: Vec::new(),
            tower_compat: Vec::new(),
            creative_mode: true,
            // Must be an id that EXISTS in real.rs's section_nav list, or the
            // Profile page opens with no sidebar item highlighted (the old
            // "inventory" default was removed from the list long ago).
            active_real_section: "body".to_string(),
            active_platform_section: "recovery".to_string(),
            active_humanity_section: "civilization".to_string(),
            map_selected_planet: None,
            map_zoom: 1.0,

            // Market defaults
            abilities: Vec::new(),
            pending_cast: None,
            ability_status: String::new(),
            ability_status_at: 0.0,
            pending_swing: false,
            swing_ready_at: 0.0,
            listings: Vec::new(),
            listings_synced: false,
            listing_status: String::new(),
            listing_search: String::new(),
            listing_filter_category: String::new(),
            listing_selected: None,
            listing_reviews: Vec::new(),
            listing_reviews_for: String::new(),
            listing_reviews_rx: None,
            listing_reviews_avg: 0.0,
            listing_reviews_count: 0,
            review_rating_draft: 5,
            review_comment_draft: String::new(),
            trades: Vec::new(),
            trades_synced: false,
            trade_status: String::new(),
            listing_show_new_form: false,
            listing_new_title: String::new(),
            listing_new_description: String::new(),
            listing_new_price: String::new(),
            listing_new_category: String::new(),

            // Calculator defaults
            calc_display: "0".to_string(),
            calc_expression: String::new(),
            calc_history: Vec::new(),

            // Calendar defaults
            cal_year: 2026,
            cal_month: 3,
            cal_selected_day: 1,
            cal_events: Vec::new(),
            cal_new_title: String::new(),
            cal_new_time: String::new(),
            cal_new_color: egui::Color32::from_rgb(237, 140, 36), // theme-exempt: seed value of the user's per-event colour picker (calendar.rs color_edit_button_srgba); stored as event data, and Default for GuiState has no Theme in scope.

            // Notes defaults
            notes: Vec::new(),
            notes_selected: None,
            notes_next_id: 1,

            // Civilization defaults
            watch_viewer: None,
            watch_texture: None,
            watch_streams: Vec::new(),
            watch_streams_rx: None,
            watch_last_fetch: 0.0,
            watch_input: String::new(),
            nav_display_mode: NavDisplayMode::default(),
            toasts: Vec::new(),
            underwater: false,
            underwater_depth_m: 0.0,
            station_off: glam::Vec3::ZERO,
            location_bookmarks: Vec::new(),
            location_bookmarks_dirty: true,
            pending_bookmark_teleport: None,
            bookmark_new_category: String::new(),
            pending_bookmark_delete: None,
            pending_bookmark_recat: None,
            pending_toasts: Vec::new(),
            pending_notices: Vec::new(),
            built_station_types: std::collections::HashSet::new(),
            surface_altitude_m: None,
            surface_gravity_now: None,
            surface_speed_mult: 1.0,
            civ_stats: None,
            civ_stats_loaded: false,
            civ_stats_rx: None,
            civ_status: String::new(),

            // Wallet defaults
            wallet_balance: 0.0,
            wallet_address: String::new(),
            wallet_network: WalletNetwork::Devnet,
            wallet_send_to: String::new(),
            wallet_send_amount: String::new(),
            wallet_transactions: Vec::new(),
            wallet_sol_price: 0.0,

            // Crafting defaults
            craft_recipes: Vec::new(),
            craft_selected: None,
            craft_selected_category: None,
            pending_craft_recipe: None,
            dev_stock_materials: false,
            craft_status: String::new(),
            blueprints: Vec::new(),
            pending_build: None,
            build_status: String::new(),
            pending_consume_item: None,
            pending_drink_item: None,
            pending_rest: false,
            pending_compost: false,
            pending_fertilize_crop: None,
            vitals: GuiVitals::default(),
            pending_deploy_kit: None,
            pending_plant_seed: None,
            pending_plant_tower: None,
            pending_plant_bed: None,
            pending_stock_seeds: None,
            pending_water_crop: None,
            pending_harvest_crop: None,
            pending_harvest_many: Vec::new(),
            dev_grow_crops: false,
            crops: Vec::new(),
            pending_drone_manifest: None,
            auto_mine_enabled: false,
            last_drone_order: None,
            prev_auto_mine_enabled: false,
            drone_active: false,
            asteroids: Vec::new(),
            drones: Vec::new(),
            vehicles: Vec::new(),
            pending_summon_vehicle: None,
            pending_open_mining_modal: None,
            pending_follow_vehicle: None,
            factory_status: Vec::new(),
            skills: Vec::new(),
            pending_dev_max_skills: false,
            quests: Vec::new(),
            quests_available: Vec::new(),
            pending_accept_quest: None,

            // Guilds defaults
            guilds: Vec::new(),
            guild_selected: None,
            guilds_loaded: false,
            guilds_rx: None,
            guild_action_rx: None,
            guild_status: String::new(),
            guild_members: Vec::new(),
            guild_members_for: String::new(),
            guild_members_rx: None,
            guild_search: String::new(),
            guild_show_create: false,
            guild_new_name: String::new(),
            guild_new_desc: String::new(),
            guild_new_color: egui::Color32::from_rgb(46, 134, 193), // theme-exempt: seed value of the create-guild colour picker (guilds.rs color_edit_button_srgba); sent to the relay as guild data, and Default for GuiState has no Theme in scope.

            player_health: 100.0,
            player_health_max: 100.0,
            inventory_items: Vec::new(),
            inventory_max_slots: 36,
            game_time: None,
            weather: None,
            power_generation: 0.0,
            power_battery_wh: 0.0,
            power_battery_capacity_wh: 0.0,
            power_autonomy_hours: 0.0,
            power_consumption: 0.0,
            power_balance: 0.0,
            water_production_lpm: 0.0,
            water_demand_lpm: 0.0,
            water_stored_l: 0.0,
            water_capacity_l: 0.0,
            water_days_autonomy: 0.0,
            air_o2_pct: 0.0,
            air_co2_pct: 0.0,
            air_pressure_atm: 0.0,
            air_temp_c: 0.0,
            air_breathable: false,
            showroom_active: false,
            show_roof: false,
            show_hull: true,
            construction_active: false,
            home_machines: None,
            home_machine_add_type: String::new(),
            home_conn_from: String::new(),
            home_conn_to: String::new(),
            home_conn_kind: "power".to_string(),
            conduit_from: String::new(),
            conduit_to: String::new(),
            conduit_kind: "water".to_string(),
            construction_console_input: String::new(),
            construction_console_output: String::new(),
            home_machines_save: false,
            construction_save_note: String::new(),
            construction_rooms: Vec::new(),
            construction_room_types: Vec::new(),
            room_type_registry: Default::default(),
            construction_add_type: String::new(),
            construction_remove: None,
            construction_plan_view: false,
            keymap_visible: false,
            help_panel_pinned: false,
            show_perf_overlay: false,
            show_weather_panel: false,
            show_cloud_dev_panel: false,
            cloud_dev_collapsed: false,
            // Matches EngineState::cursor_free's boot value; reconcile_cursor
            // overwrites it on the first frame it runs.
            cursor_free: false,
            cloud_dev_dither_off: false,
            cloud_dev_temporal_off: false,
            cloud_dev_map_diag: 0,
            cloud_dev_clock_pin: -1.0,
            cloud_dev_chord_foot: false,
            cloud_dev_world_shape_lod: false,
            cloud_dev_ring_cure_off: true,
            cloud_dev_uniform_step: false,
            cloud_dev_wide_edge: false,
            cloud_dev_edge_mul: 0.0,
            cloud_dev_rind_wide_m: 0.0,
            cloud_dev_step_m: 0.0,
            cloud_dev_shear: 0.0,
            cloud_dev_hv_km: 0.0,
            cloud_dev_sigma_mul: 0.0,
            cloud_dev_est: true,
            cloud_dev_warp_bl: true,
            cloud_dev_norm_floor: false,
            cloud_dev_iso_step: false,
            cloud_dev_thin_deck: false,
            cloud_dev_hv_warp: false,
            cloud_dev_no_detail: false,
            cloud_dev_no_puff: false,
            cloud_dev_no_cell: false,
            cloud_dev_no_fray: false,
            cloud_dev_no_bdrop: false,
            cloud_dev_sharp_base: false,
            cloud_dev_relief_fade: false,
            cloud_dev_deep_rung: false,
            cloud_dev_checker: false,
            // ── MULTIPLE SCATTERING, ON BY DEFAULT (v0.1331.8) ──
            //
            // Built v0.1288 behind this bit and left OFF with the note "stays
            // off until the interior gate is met by physics": at a close
            // camera the in-cloud mean reached 58 against a physical target
            // of 135 or more, so it was judged insufficient FOR THE INTERIOR.
            //
            // Nobody then asked what it does from ORBIT, and the answer is
            // that its absence was a physics violation on the DEFAULT tier.
            // Measured at approach-2000km-high, noon over the Sahara, with
            // the terrain in the same frame as the control:
            //
            //   off  cloud 120.1  terrain 174.9  ratio 0.69
            //   on   cloud 187.4  terrain 175.3  ratio 1.07
            //   Low tier, for reference             ratio 1.08
            //
            // Cloud albedo is 0.7 to 0.9 and Sahara sand about 0.35, so a
            // daylit deck MUST read brighter than the desert under it. It did
            // not. A single-scattering Beer-Lambert march is dark by
            // construction, because what makes a real cloud bright is photons
            // bouncing inside it many times, which is precisely this term.
            //
            // It also cut the orbital speckle 1.76 to 1.00 percent with NO
            // change to any filter, which is what identified that grain as
            // largely a SYMPTOM of the missing energy rather than a defect of
            // its own (docs/PRIORITIES.md item 2).
            //
            // Cost, from armed GPU timestamps rather than the present-capped
            // frame time: gpu.cloud_screen 5.78 ms off against 5.76 ms on. It
            // is free because it is an analytic Eddington two-stream source,
            // not extra marching.
            //
            // The interior gate it was parked for is still NOT met: the close
            // camera moves 49.9 to 108.3 against that 135+ target. That is a
            // real improvement and an unfinished one, and its own commit says
            // why - the modelled deck is optically thinner than a real cumulus
            // (tau 13 against 27), so the rest is field density work.
            cloud_dev_ms: true,
            cloud_dev_field: false,
            cloud_dev_body_cache: true,
            cloud_dev_step_eco: 1.0,
            cloud_dev_light: true,
            // Off until the far-rung gates G0..G6 pass (contract default).
            cloud_dev_profile_knob: 0,
            // D3 dev bit: off until its gate passes (the A/B default).
            cloud_dev_top_bound: false,
            // 1.8 (was 1.0), operator 2026-09-25: clouds "still kind of gray
            // while in high orbit". The unit gain was chosen because it matched
            // the Low tier's cloud-to-terrain ratio (1.07 against 1.08), but the
            // Low tier is only another path, not a physical reference, so
            // matching it proved consistency and nothing about brightness. The
            // physical reference is reflectance: a thick sunlit cloud about 0.8,
            // desert sand 0.35-0.4, so thick cloud should be roughly twice the
            // sand in linear light, which in this tonemap is about 215-225
            // against sand near 180 (the 2026-09-25 fidelity review's estimate).
            // Measured on approach-2000-ms* (Sahara, 2000 km, noon, High), thick
            // cloud p50 / p90 against sand 181: gain 1.0 199 / 219, 1.6 213 /
            // 226, 1.8 223 / 232, 2.2 221 / 232 (the ms arms leave the cloud
            // clock free, so sweeps differ slightly). 1.8 sits in the target;
            // at 2.2 the surface texture starts to wash out, and at 55 km
            // (deck-55-oblique-ms18) 1.8 keeps the cauliflower texture intact.
            cloud_dev_ms_gain: 1.8,
            cloud_dev_int_sat: 0.0,
            cloud_dev_res_div: 4,
            cloud_dev_shape_off: false,
            cloud_dev_discard_diag: false,
            weather_manual: false,
            weather_pick_condition: crate::systems::weather::WeatherCondition::Clear,
            weather_pick_intensity: 0.0,
            weather_pick_wind: 8.0,
            weather_pick_note: String::new(),
            weather_retrigger: false,
            time_hour_request: None,
            time_scale_request: None,
            time_pick_hour: 8.0,
            time_speed: 1.0,
            time_frozen: false,
            station_nadir: true,
            station_yaw_deg: 0.0,
            station_pitch_deg: 0.0,
            station_roll_deg: 0.0,
            station_attitude_dirty: false,
            station_readout: String::new(),
            credits: crate::credits::Credits::default(),
            osm_regions_drawn: Vec::new(),
            show_network_overlay: false,
            show_system_overlay: false,
            frame_times: Vec::new(),
            ws_msgs_in: 0,
            mic_test_active: false,
            audio_input_device: String::new(),
            audio_output_device: String::new(),
            audio_input_devices: Vec::new(),
            audio_output_devices: Vec::new(),
            audio_devices_loaded: false,
            mic_meter: 0.0,
            mic_test_prev: false,
            voice_gain: 1.0,
            voice_filter_mode: crate::config::VoiceFilterMode::default(),
            voice_transmit_mode: crate::config::VoiceTransmitMode::default(),
            voice_ptt_key: "CapsLock".to_string(),
            voice_vad_threshold: 0.05,
            voice_ptt_held: false,
            voice_binding_key: false,
            keybinds: crate::input::bindings::Keybinds::default(),
            keybind_capture: None,
            keybind_conflict: None,
            keybind_status: String::new(),
            diag_entity_count: 0,
            diag_light_count: 0,
            diag_mem_mb: 0.0,
            diag_uptime_secs: 0,
            keymaps: Vec::new(),
            construction_selected_room: None,
            construction_height: 3.0,
            construction_level: 0,
            construction_dirty: false,
            construction_machines_dirty: false,
            ws_identified: false,
            star_catalog_download: None,
            star_catalog_remove: None,
            star_catalog_dl: None,
            star_catalog_installed: [None; 2],
            galaxy_glow_download: false,
            galaxy_glow_remove: false,
            galaxy_glow_dl: None,
            galaxy_glow_installed: None,
            screenshot_capture_request: None,
            screenshot_last_result: None,
            // Defaults mirror the 4K quick button so the custom row starts sane.
            screenshot_custom_width: "3840".to_string(),
            screenshot_custom_height: "2160".to_string(),
            copresence_solo: false,
            construction_unsaved: false,
            ship_structure: None,
            construction_zone: 0,
            construction_zone_delete_arm: false,
            construction_corridor_from_zone: 0,
            construction_corridor_to_zone: 0,
            construction_corridor_lat: 0.0,
            construction_corridor_door_w: 2.0,
            construction_corridor_door_h: 2.2,
            construction_corridor_width: 3.0,
            construction_corridor_glass: false,
            construction_corridor_error: String::new(),
            construction_structure_dirty: false,
            construction_wall_mode: false,
            construction_wall_start: None,
            construction_cursor_world: None,
            construction_wall_selected: None,
            construction_machine_selected: None,
            construction_light_selected: None,
            construction_light_ring_hover: None,
            build_char_pos: None,
            construction_grid_snap: true,
            construction_dev_overlay: false,
            gi_enabled: true,
            construction_sun_override: false,
            construction_sun_override_hour: 12.0,
            construction_undo_depth: 64,
            construction_palette_category: String::new(),
            construction_palette_expanded: false,
            construction_place_type: None,
            construction_place_light: None,
            construction_structure_type: None,
            construction_place_conduit_node: false,
            zone_add_type: String::new(),
            construction_zone_selected: None,
            rail_edge_from: 0,
            rail_edge_to: 0,
            construction_structure_selected: None,
            construction_focus_request: None,
            construction_object_filter: String::new(),
            construction_multi: std::collections::HashSet::new(),
            construction_locked_types: std::collections::HashSet::new(),
            construction_hidden_types: std::collections::HashSet::new(),
            construction_road_node_selected: None,
            construction_conduit_node_selected: None,
            construction_connection_selected: None,
            construction_structure_yaw: 0.0,
            construction_structure_place_y: 0.0,
            construction_road_from: 0,
            construction_road_to: 0,
            construction_road_class: String::new(),
            construction_road_width: 4.0,
            construction_dimension_overlay: true,
            construction_show_helpers: true,
            construction_save: false,
            showroom_backdrop: 0,
            showroom_backdrop_names: Vec::new(),
            showroom_confirm: false,
            appearance: crate::ecs::components::Appearance::default(),
            appearance_dirty: false,
            outfit: crate::ecs::components::Outfit::default(),
            outfit_dirty: false,
            showroom_mode: 0,
            showroom_return_to_picker: false,
            cosmetics_list: Vec::new(),
            character_name: "Wanderer".to_string(),
            settings_dirty: false,
            quit_requested: false,
            identity_recovered: false,
            private_key_bytes: None,
            history_fetched: false,
            passphrase_needed: false,
            passphrase_mode: PassphraseMode::Unlock,
            passphrase_input: String::new(),
            passphrase_confirm: String::new(),
            passphrase_old_input: String::new(),
            passphrase_status: String::new(),
            encrypted_private_key: String::new(),
            key_salt: String::new(),
            // Fresh GuiState has no vault yet; the new-encrypt path stamps
            // `PBKDF2_ITERATIONS_NEW` when the user first picks a passphrase.
            // A loaded legacy config overwrites this with its stored value
            // (defaults to 100_000 via serde for pre-v0.277.0 configs).
            key_iterations: crate::config::PBKDF2_ITERATIONS_NEW,
            section_locks: std::collections::HashMap::new(),
            // v0.278.0 auto-unlock — default is opt-out (always prompt).
            // A loaded config overwrites this with the user's stored choice.
            auto_unlock_mode: crate::auto_unlock::AutoUnlockMode::AlwaysPrompt,
            pin_encrypted_seed: String::new(),
            pin_salt: String::new(),
            remember_on_device: false,
            passphrase_unlocking: false,
            #[cfg(feature = "native")]
            passphrase_unlock_rx: None,
            pin_unlocking: false,
            #[cfg(feature = "native")]
            pin_unlock_rx: None,
            #[cfg(feature = "native")]
            clipboard_upload: None,
            pin_input: String::new(),
            pin_confirm: String::new(),
            pin_old_input: String::new(),
            pin_status: String::new(),
            kyber_public_b64: String::new(),
            peer_kyber_keys: std::collections::HashMap::new(),
            voice_channel_ids: std::collections::HashMap::new(),
            voice_rx_frames: 0,
            voice_active_room: None,
            voice_incumbents_captured: false,
            voice_connected_peers: std::collections::HashSet::new(),
            voice_session_prev: false,
            call_incoming: None,
            call_active: None,
            call_outgoing: None,
            call_outgoing_deadline: None,
            call_muted: false,
            chat_attach_picker: None,
            ffmpeg_picker: None,
            donate_solana_address: String::new(),
            donate_btc_address: String::new(),
            donate_addresses: Vec::new(),
            donate_addresses_server: Vec::new(),
            donate_funding_goal: None,
            donate_funding_server: String::new(),
            donate_info_rx: None,
            donate_new_network: String::new(),
            donate_new_type: "address".into(),
            donate_new_value: String::new(),
            donate_new_label: String::new(),
            chat_user_modal_open: false,
            chat_user_modal_name: String::new(),
            chat_user_modal_key: String::new(),
            show_add_server_modal: false,
            add_server_url_draft: String::new(),
            add_server_name_draft: String::new(),
            server_settings_tab: 0,
            server_settings_channel_drafts: std::collections::HashMap::new(),
            server_settings_new_channel: ChannelDraft::default(),
            show_create_group_modal: false,
            dm_settings_popup_open: false,
            groups_settings_popup_open: false,
            notif_dm_enabled: true,
            notif_mentions_enabled: true,
            notif_tasks_enabled: true,
            notif_dnd_start: None,
            notif_dnd_end: None,
            notif_prefs_loaded: false,
            account_export_status: String::new(),
            account_export_rx: None,
            account_delete_confirm_input: String::new(),
            privacy_tiers_cache: Vec::new(),
            privacy_tier_selection: String::new(),
            privacy_tier_prompt_open: false,
            dm_store: None,
            dm_fetch_sent: false,
            server_settings: None,
            chat_roles: Vec::new(),
            service_state: Vec::new(),
            roles_drafts: std::collections::HashMap::new(),
            new_role_draft: {
                // Blank template for a new custom role: empty id (operator
                // types one), sensible non-privileged defaults.
                let mut r = crate::relay::storage::RoleDef::default();
                r.id = String::new();
                r.label = String::new();
                r.color = "#7E57C2".to_string();
                r.trust_level = 1;
                r.built_in = false;
                r.can_stream = false;
                r.can_upload = true;
                r.can_voice = true;
                r.can_image_share = true; // a typical custom role (e.g. "family") shares media
                r.can_file_share = true;
                r.base_tier = "verified".to_string();
                r.sort_order = 50;
                r
            },
            chat_banned_users: Vec::new(),
            chat_banned_requested: false,
            backup_list: Vec::new(),
            backup_list_requested: false,
            // Game admin (game-world bans, separate from chat bans)
            game_bans: Vec::new(),
            game_bans_requested: false,
            game_admin_target_key: String::new(),
            game_admin_ban_reason: String::new(),
            game_admin_status: String::new(),
            // The Play picker (WHO/WHERE pairing)
            launcher_homes: Vec::new(),
            launcher_homes_loaded: false,
            launcher_who: String::new(),
            launcher_where_kind: LauncherWhere::Home,
            launcher_selected_world: String::new(),
            launcher_selected_server: None,
            launcher_last_character: String::new(),
            launcher_last_world: String::new(),
            launcher_pending_load: None,
            launcher_open_select: false,
            showroom_cancel: false,
            server_info_cache: std::collections::HashMap::new(),
            server_info_loader: None,
            chat_muted_users: Vec::new(),
            chat_muted_requested: false,
            server_settings_draft: None,
            cosmos_view: crate::gui::pages::cosmos::CosmosView::System,
            cosmos_pan: egui::Vec2::ZERO,
            cosmos_zoom: 1.0,
            cosmos_selected_body: None,
            cosmos_expanded_planets: std::collections::HashSet::new(),
            cosmos_focus_request: None,
            cosmos_tracked_body: None,
            cosmos_camera_3d: crate::gui::pages::cosmos::Cosmos3DCamera::default(),
            cosmos_sim_time_seconds: 0.0,
            cosmos_sim_speed: 0.0, // Paused by default — operator scrubs / plays.
            cosmos_last_real_instant: None,
            cosmos_sim_time_initialized: false,
            cosmos_upcoming_events: Vec::new(),
            cosmos_upcoming_scan_origin: f64::NAN, // forces first scan
            cosmos_upcoming_last_scan: None,
            cosmos_expanded_body: None,
            cosmos_show_lagrange: false,
            cosmos_show_reference_orbits: false,
            new_group_name: String::new(),
            new_group_share_history: false,
            create_group_ticket: None,
            create_group_status: String::new(),
            show_join_group_modal: false,
            join_group_invite_code: String::new(),
            join_group_status: String::new(),
            join_group_result: None,
            p2p_groups: Vec::new(),
            p2p_groups_last_fetch: None,
            p2p_group_invite_status: String::new(),
            p2p_group_active_id: String::new(),
            p2p_group_chat_epoch: 0,
            p2p_group_chat_epoch_key: None,
            p2p_group_last_fetch: None,
            p2p_group_fp_to_key: std::collections::HashMap::new(),
            p2p_group_fp_to_name: std::collections::HashMap::new(),
            #[cfg(feature = "native")]
            p2p_group_loader: None,
            #[cfg(feature = "native")]
            p2p_groups_list_loader: None,
            p2p_group_loading: false,
            #[cfg(feature = "native")]
            p2p_group_seen_obj_ids: std::collections::HashSet::new(),
            show_channel_edit_modal: false,
            edit_channel_id: String::new(),
            edit_channel_name: String::new(),
            edit_channel_description: String::new(),
            edit_channel_confirm_delete: false,
            server_settings_target_user: String::new(),
            server_settings_channel_name: String::new(),
            server_settings_invite_code: String::new(),
            redeem_code_draft: String::new(),
            revoke_key_draft: String::new(),
            server_settings_status: String::new(),
            system_health: None,
            system_health_status: String::new(),
            system_health_rx: None,
            relay_cc_tab: 0,
            relay_cc_selected: None,
            relay_admin_stats: None,
            relay_admin_stats_status: String::new(),
            relay_admin_stats_rx: None,
            vps_console_input: String::new(),
            vps_console_output: String::new(),
            vps_console_rx: None,
            vps_console_running: false,
            federation_servers: Vec::new(),
            federation_status: String::new(),
            federation_rx: None,
            federation_add_url_draft: String::new(),
            federation_add_name_draft: String::new(),
            federation_add_key_draft: String::new(),
            federation_add_key_name_draft: String::new(),
            host_node_autostart: false,
            host_node_port: String::new(),
            host_node_db: String::new(),
            host_node_name: String::new(),
            server_settings_confirm_action: None,
            show_help_modal: false,
            debug_console_visible: false,
            debug_log: Vec::new(),
            tools_catalog: Vec::new(),
            equipment_slots: Vec::new(),
            bug_severities: Vec::new(),
            bug_categories: Vec::new(),
            crafting_category_groups: Vec::new(),
            market_categories: Vec::new(),
            library: Vec::new(),
            library_tags: Vec::new(),
            curriculum: CurriculumData { subjects: Vec::new(), topics: Vec::new() },
            studio_scene_presets: Vec::new(),
            studio_source_presets: Vec::new(),
            studio_streaming_config: StudioStreamingConfig::default(),
            donate_faq: Vec::new(),
            donate_methods: Vec::new(),
            donate_charities: Vec::new(),
            qa_test_tasks: Vec::new(),
            qa_test_status: std::collections::HashMap::new(),
            qa_test_note: std::collections::HashMap::new(),
            qa_test_filter: "all".to_string(),
            web_sites: WebSites::default(),
            browser_filter: "all".to_string(),
            web_view: widgets::web_view::WebViewState::new(),
            // v0.174.0: default to two-tier nav for fresh installs. Existing
            // users with `nav_two_tier=false` saved in config keep their
            // legacy layout until they flip via [▤]; new sessions land on
            // the two-tier layout immediately.
            nav_two_tier: true,
            nav_top_category: "reality".to_string(),
            attack_pulse_active: false,
            attack_pulse_last_hit_at: 0.0,
            player_death_cause: None,
            pending_respawn: false,
            // v0.197.0: ai_usage_filters removed (page deleted).
            help_registry: crate::gui::widgets::help_modal::HelpRegistry::new(),
            active_help_topic: None,
            onboarding_quest_chains: Vec::new(),
            onboarding_quest_progress: std::collections::HashMap::new(),
            image_cache: crate::gui::widgets::image_cache::ImageCache::new(),
            image_viewer_url: None,
            studio: StudioState::default(),

            // Chat panel collapse state (all expanded by default except connection)
            chat_connection_collapsed: true,
            chat_dm_collapsed: false,
            chat_groups_collapsed: false,
            chat_servers_collapsed: false,
            chat_commons_collapsed: false,
            chat_server_sections_collapsed: std::collections::HashSet::new(),
            server_drag: None,
            show_commons_info: false,
            chat_connected_server_collapsed: false,
            chat_friends_collapsed: false,
            chat_members_collapsed: false,
            chat_studio_collapsed: false,
            chat_open_popup_ts: None,
            pending_clipboard_paste: false,
            chat_mention_index: 0,
            chat_dm_display_limit: 5,

            // Chat panel resize/lock state
            chat_left_panel_locked: false,
            chat_right_panel_locked: false,
            chat_left_panel_width: 220.0,
            chat_right_panel_width: 220.0,

            // Identity / Governance / Recovery page state (v0.115.0)
            identity_lookup_did: String::new(),
            identity_lookup_pending: false,
            governance_scope_tab: 0,
            governance_proposals: Vec::new(),
            governance_rx: None,
            governance_fetched_for: String::new(),
            governance_refresh: false,
            governance_error: String::new(),
            governance_my_votes: std::collections::HashMap::new(),
            governance_vote_rx: None,
            governance_vote_status: String::new(),
            governance_propose_rx: None,
            governance_show_propose: false,
            governance_new_title: String::new(),
            governance_new_body: String::new(),
            governance_new_type_idx: 0,
            governance_new_scope_idx: 0,
            governance_new_days: 7.0,
            laws_location: String::new(),
            laws_search: String::new(),
            laws_filter_tab: 0,
            laws_category: String::new(),
            governance_filter_tab: 0,
            recovery_lookup_did: String::new(),
            recovery_lookup_pending: false,
            recovery_guardian_did: String::new(),
            recovery_guardian_pending: false,

            // v0.197.0: AI Usage page form state removed (page deleted).
        }
    }
}

// These three stayed behind when the data loaders moved to `gui/loaders.rs`:
// they are serde `default = "..."` targets for `SettingsState` just below, and
// a serde default resolves as a path in the scope of the struct that names it.
// Moving them would have meant rewriting those attributes, which is a change
// rather than a motion.
/// Default underwater clarity (v0.1054): mostly physical, but not so dark that
/// a first dive is a black screen. The operator can take it either way.
fn default_water_clarity() -> f32 { 0.35 }
fn default_precip_density() -> f32 { 1.0 }
fn default_fog_density() -> f32 { 1.0 }

#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Account,
    Appearance,
    Animations,
    Widgets,
    Notifications,
    Wallet,
    Audio,
    Graphics,
    Gameplay,
    Controls,
    Privacy,
    /// Video on in-world screens: where ffmpeg is for converting a chosen
    /// file (2026-09-18). Named in the screen's own error text ("set its
    /// path in Settings > Media"), so the section must exist by this name.
    Media,
    Data,
    Updates,
    Credits,
}

/// Profile page sidebar sections.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSection {
    // Private (red)
    BodyMeasurements,
    Identity,
    PrivateNotes,
    // Personal (orange)
    NetworkProfile,
    Interests,
    Skills,
    // v0.415.0: Quests section removed — live game quests render on the
    // top-level Quests page beside the learn-by-doing chains (the operator's
    // "one page, two kinds" model), not buried in Profile.
    // Public (green)
    SocialLinks,
    Streaming,
}

#[cfg(feature = "native")]
pub struct SettingsState {
    pub category: SettingsCategory,
    /// When set, the settings page scrolls to this section and clears the field.
    pub scroll_to_section: Option<SettingsCategory>,
    // Graphics
    pub fullscreen: bool,
    /// How the desktop window is presented (v0.454). Default = WindowedFullscreen.
    pub window_mode: crate::config::WindowMode,
    pub vsync: bool,
    /// Frame-rate caps (v0.1016): foreground = focused window, background =
    /// unfocused (alt-tabbed). The bools are the "Unlimited" / "Sync to
    /// foreground" checkboxes; the pacing itself lives in the redraw loop.
    pub fps_foreground: u32,
    pub fps_foreground_unlimited: bool,
    pub fps_background: u32,
    pub fps_background_sync: bool,
    pub fov: f32,
    pub render_distance: f32,
    /// Master toggle for procedural sky-planet surfaces (v0.763). Off falls
    /// back to smooth flat-colored spheres (the pre-v0.763 look).
    pub planet_detail: bool,
    /// Sky orbit-ring visibility (v0.786): "off" | "planets" | "planets_moons".
    /// Future modes (owned vessels, collision-course asteroids, selected
    /// objects) land when their data exists -- see PRIORITIES.
    pub sky_orbit_mode: String,
    /// Show constellation figures in the FPS sky (v0.786).
    pub sky_constellations: bool,
    /// Show the Milky Way glow layer (2026-07-10): the baked all-sky
    /// texture of real integrated catalog starlight, drawn behind the
    /// star points. Off skips the pass entirely.
    pub sky_milkyway_glow: bool,
    /// Milky Way glow intensity multiplier (0..2, default 1.0). Applied
    /// live as a shader uniform - no rebuild needed.
    pub sky_milkyway_intensity: f32,
    /// Milky Way glow texture tier (2026-07-11): "standard" (8192x4096,
    /// ships with the app) or "ultra" (downloadable 16384x8192, ~512 MB
    /// VRAM). Applies next world entry - the glow layer is built with the
    /// star renderer, and the loader falls back to standard when the ultra
    /// file is missing/corrupt or exceeds the GPU's texture limit.
    pub sky_glow_tier: String,
    /// Star-catalog load CEILING (2026-07-12 dev tooling): "auto" (biggest
    /// installed wins - the player default), "standard"/"minimal" (force the
    /// shipped 120k catalog for a fast boot), "extended", "ultra". Applied next
    /// world entry; HUMANITY_STAR_TIER env overrides without persisting.
    pub star_catalog_tier: String,
    /// Star halos (2026-07-11): soft photographic glow + faint diffraction
    /// cross on the brightest ~50 stars, drawn additively over the star
    /// points. A plain visibility flag - applies live, no GPU state.
    pub sky_star_halos: bool,
    /// Screen-size LOD base threshold in pixels: a sky body subdivides one
    /// icosphere level each time its projected diameter doubles past this.
    /// See terrain::planet::lod_level_for_pixels.
    pub planet_lod_px: f32,
    /// Chunk split threshold px (Planet LOD settings, v0.873).
    pub terrain_split_px: f32,
    /// Max surface patches per planet per frame.
    pub terrain_patch_budget: f32,
    /// Detail draw distance factor (v0.905, Settings > Planets).
    pub terrain_detail_distance: f32,
    /// Patch mesh builds per frame (stream speed).
    pub terrain_builds_per_frame: f32,
    /// Near-field real tree model distance in metres (v0.911; 0 = cards
    /// only). Grass/tree silhouette cards continue past this range.
    pub tree_model_distance: f32,
    /// How many near-field 3D tree MODELS may be drawn at once, nearest first
    /// (v0.1109; was the hardcoded NEAR_TREE_DRAW_BUDGET). This and
    /// `tree_model_distance` have to move together: the budget bounds the
    /// model stage no matter how far the distance reaches, so raising the
    /// distance alone just packs the same models into a tighter ring and lets
    /// silhouette cards carry the difference.
    pub near_tree_budget: f32,
    /// Grass draw distance in metres (v0.1109; was terrain::grass::GRASS_FAR_M):
    /// the surface distance at which the density ramp reaches zero. The near
    /// (6 m) and mid (12 m) anchors stay put, so raising this stretches the
    /// fade rather than sliding the whole field out, which also thickens the
    /// middle distance. Cost rises with the SQUARE of it.
    pub grass_far_m: f32,
    /// Ceiling on how many grass tillers one CPU harvest may emit (v0.1109;
    /// was the hardcoded GRASS_HARVEST_CAP). The harvest walks nearest-first
    /// and breaks at the cap, so hitting it cuts the field's FAR EDGE off in a
    /// hard circle. Exposed so the ceiling is a setting, not a code edit.
    pub grass_harvest_cap: f32,
    /// Vegetation spawn density multiplier (v0.1083, operator: fewer trees,
    /// free up GPU). Scales TREES_PER_CELL at patch build and GRASS_PEAK_PER_M2
    /// in the near-field strand harvest;
    /// 1.0 = the historical full density, default 0.6.
    /// TREES per cell, 0.1..1.0. Was one "vegetation" slider that also drove
    /// grass, which meant nobody could ask for thick grass under thin forest.
    pub tree_density: f32,
    /// GRASS COVERAGE, 0.1..3.0 as a multiplier on the authored leaf-area
    /// target. The artistic knob: how much grass is on the ground.
    pub grass_density: f32,
    /// GRASS DETAIL, 0.1..1.0. The performance knob: blades per tiller and
    /// segments per blade, with width compensating so cover never changes.
    pub grass_detail: f32,
    /// Vegetation LOD (v0.923): tree silhouette-card far cutoff in metres -
    /// the card stage's outer distance. More ladder stages follow.
    pub veg_tree_card_m: f32,
    /// Water near-field mesh depth cap (v0.965, per-type LOD section):
    /// 17 = ~4.8 m wave vertices at the eye, 20 = ~0.6 m. Selection stays
    /// pixel-driven, so this only shapes the closest tens of metres.
    pub water_detail_depth: f32,
    /// Sun shadow map on/off (v0.907, Settings > Planets).
    pub sun_shadows: bool,
    /// How dark a full sun shadow gets, 0..1 (v0.1104). 1.0 = no direct sun
    /// reaches a shadowed surface and the sky-irradiance term fills it, which
    /// is what real shadows are. Lower values leak warm sunlight into shadow.
    pub shadow_strength: f32,
    /// How fast crops grow, as a multiplier on growth progress ONLY -- the
    /// world clock is untouched (operator, 2026-09-20: "10x growth speed (not
    /// clock speed)"). 1x is real agricultural time, where the fastest crop in
    /// `plants.csv` still takes 4.7 real hours; 10x and 100x are the offered
    /// presets. The default is 10x because a 1x default makes plant life
    /// cycles untestable without waiting days.
    pub crop_growth_speed: f32,
    /// Offline progression (operator, 2026-09-21; docs/design/offline-
    /// progression.md): while the game is closed your character keeps living,
    /// so crops grow by the time you were away. Read once when a save is
    /// loaded (save_load::resume_home), not every frame.
    pub offline_progression: bool,
    /// Start every session from the DEFAULT home, keeping only the character
    /// (name, look, outfit). Operator, 2026-09-25: "let's stay in the dev
    /// mode, I don't want to diverge again so that we can make sure I always
    /// see what you build and what our default is." A save that keeps
    /// progress drifts from what a new player sees (his had 1,575 of 1,976
    /// crops dead of thirst while a new player gets a fresh garden). ON by
    /// default until the starting home is finished; the progress save on
    /// disk is left untouched while it is on. Revisit at launch.
    pub fresh_world_each_launch: bool,
    /// Aerial perspective strength (v0.916): how strongly distant land and
    /// sea fade toward sky color. 0 = off, 1 = earthlike.
    pub aerial_strength: f32,
    /// Crepuscular god-ray shaft intensity (0 = off).
    pub godray_intensity: f32,
    /// Ambient-occlusion contact shading strength (0 = off).
    pub ssao_strength: f32,
    /// Max icosphere subdivision level for sky planets (0-9; level 6 is
    /// ~82k faces, levels 8-9 are the heavy close-approach tiers -- see
    /// terrain::planet::MAX_SKY_SUBDIVISION for the face/memory table).
    /// Stored as f32 for the slider; rounded at use.
    pub planet_max_subdiv: f32,
    /// Chunked planetary LOD (2026-07-11): when a heightmap-bearing planet
    /// (Earth) fills the screen, stream camera-following quadtree patches
    /// (~54 m triangles near the surface) instead of the heavy uniform
    /// close-approach spheres. Off keeps the pre-chunk behavior (uniform
    /// levels 8-9 near a planet). See terrain::planet_chunks.
    pub planet_chunked: bool,
    /// Geomorph crossfades (v0.920): terrain LOD splits/merges dissolve
    /// over ~0.3 s with a complementary screen-door dither instead of
    /// popping. Off = instant swaps (the pre-v0.920 behavior, kept as the
    /// A/B reference and safety hatch). See planet_chunks::FadePair.
    pub terrain_lod_fade: bool,
    /// Analytic scattering atmosphere shells (v0.807): per-pixel single
    /// scattering (blue limb, warm terminator, in-atmosphere sky gradient).
    /// Off = the old fresnel-tinted shell -- kept forever-dev style as the
    /// A/B reference and as a safety hatch for GPUs that dislike the math.
    /// See pbr_simple.wgsl type 14 + renderer::atmosphere.
    pub planet_atmo_scatter: bool,
    /// Animated procedural cloud shells (clouds increment 1): the drifting,
    /// sun-lit cloud deck on planets that declare cloud_coverage in their
    /// RON (Earth). Off skips the shell entirely (no material, no draw).
    /// See pbr_simple.wgsl type 15 + renderer::clouds.
    pub planet_clouds: bool,
    /// Live Earth weather (v0.874): real NASA cloud cover placed on the
    /// in-game sky; background fetch + disk cache; procedural fallback
    /// wherever the satellite map has no data.
    pub live_weather: bool,
    /// Track the orbital home station (in-world ring + label; Cosmos toggle).
    pub track_station: bool,
    /// Read websites inside HumanityOS (the readable web, 2026-09-16). OFF
    /// by default and independent of the privacy tier: a person who wants
    /// no website access from the app never has any. On, a Browser-page
    /// card fetches the page (one HTTPS GET for the URL, plus its images)
    /// and draws it in the in-app web view; off, cards open the OS browser.
    pub readable_web: bool,
    /// The video file each in-world screen plays, keyed by the placed
    /// instance id, chosen with the screen's Open button (2026-09-18).
    /// Persisted in AppConfig; the data file's `video:` source is only the
    /// default, a remembered file wins on boot.
    pub screen_media: std::collections::BTreeMap<String, std::path::PathBuf>,
    /// Settings > Media: where ffmpeg is (a file or the folder holding it),
    /// for converting a chosen video into WebM AV1 + Opus. Empty = auto.
    pub ffmpeg_path: String,
    /// Planet close-range surface detail (v0.816): animated ocean waves
    /// (moving sun sparkle, Fresnel sky mirror) + land micro-texture under
    /// the photo albedo, on planets with baked per-pixel imagery (Earth).
    /// Anti-alias faded so the orbit view is identical either way. Applies
    /// LIVE: the sky loop rewrites the material flag every frame. See
    /// pbr_simple.wgsl type 12 + renderer::water.
    pub planet_surface_detail: bool,
    /// FFT ocean (v0.1029, water-fft.md increment 1): JONSWAP-spectrum
    /// FFT chop field instead of the three anchored trains. Experimental,
    /// default off; applies live (the flag rides the per-frame uniform).
    pub water_fft: bool,
    /// Underwater clarity (v0.1054, operator: "I can easily see the sea floor
    /// everywhere as if there's no actual depth darkening... we should
    /// introduce like a setting for it"). 0 = physical: real seawater
    /// extinction, so red dies within metres and the view goes blue then black
    /// with depth. 1 = the old unlimited visibility, which they explicitly want
    /// KEPT because it is how you go find places like Challenger Deep.
    pub water_clarity: f32,
    /// Precipitation density multiplier (v0.1060). Scales BOTH the spawn rate
    /// and the emitter population cap, so it can actually build a downpour
    /// rather than saturating against the RON ceiling. Deliberately allowed to
    /// go to 10x so the operator can find the limits.
    pub precip_density: f32,
    /// Weather fog/dust extinction multiplier (v0.1060). 0 = no weather fog.
    pub fog_density: f32,
    /// GPU particle simulation for precipitation (v0.1068). Default OFF while
    /// it proves itself: the CPU path is the shipped behaviour and stays as the
    /// fallback, exactly like the FFT-ocean toggle did.
    pub gpu_particles: bool,
    /// Far-tree canopy card sheet (v0.1022). Default off: reads as grid
    /// squares at altitude; superseded by the impostor arc. Kept for A/B.
    pub far_tree_sheet: bool,
    /// Cloud quality (clouds increment 3): "low" = the painted deck,
    /// "medium" = the 10-sample field march, "high" = the volumetric
    /// 3D-noise system (default). Applies live: the cloud material is
    /// cached per (body, quality) and rebuilt on the next frame the deck
    /// draws. See pbr_simple.wgsl cloud_layer + renderer::clouds.
    pub cloud_quality: String,
    // Audio
    /// Tiled light lists (clustering L1b): bounds per-pixel light cost by
    /// screen tile, lifting the light cap 256 -> 2048. Off = classic loop.
    pub lights_tiled: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    /// Independent interface-sound volume (button clicks / UI feedback). Routed
    /// through the "ui" bus, so it is separate from `sfx_volume`. See AppConfig::ui_volume.
    pub ui_volume: f32,
    /// Whether interface sounds play at all (the "Interface sounds" toggle).
    pub ui_sounds_enabled: bool,
    // Controls
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    // Appearance
    pub dark_mode: bool,
    pub font_size: f32,
    /// How per-setting descriptions are shown on THIS page (v0.1116): Full
    /// (inline, tightened) / Hover ("(?)" marker) / Off. See `HintDisplay`.
    pub hint_display: HintDisplay,
    // Notifications: no Settings fields - the live prefs are relay-synced
    // (GuiState::notif_*, edited by Settings > Notifications AND the chat DM
    // cog; v0.980 removed the dead notify_*/dnd_* placebos nothing read).
    // Gameplay
    /// Which home design loads (2026-07-01): `"home"` (default, family-scale) or
    /// `"home_solo"` (one-person self-sufficient design, see
    /// `docs/design/homestead-solo-design.md`). Applied via `crate::machines::
    /// home_ron_path` at world-load time -- changing this takes effect on next
    /// world load, not live mid-session (see the Settings UI's own note).
    pub home_variant: String,
    /// Spawn hostile wild creatures (wolf packs etc. from wild_spawns.ron).
    /// Default OFF pre-launch (v0.791, operator: "disable the wolves") -- the
    /// Dev spawn page still places any creature deliberately. Turning it off
    /// mid-session despawns live hostiles; turning it on repopulates on the
    /// next world load. The play-mode system (task #50) deliberately leaves
    /// this INDEPENDENT of the mode: hostile wildlife is a difficulty choice
    /// in every mode, not a cheat, so switching modes never flips it.
    pub hostile_wildlife: bool,
    /// Survival-needs speed: scales hunger/thirst/energy decay in the food
    /// system (1.0 = normal, 0 = paused). v0.791, with slowed base rates.
    pub vitals_drain: f32,
    /// Which survival bars the HUD draws (2026-09-25). See HudVitals.
    pub hud_vitals: crate::config::HudVitals,
    /// Play mode (task #50): Normal | Creative | Dev -- one ladder for every
    /// cheat/scope gate (see `crate::config::PlayMode` + `Capability` for the
    /// tested truth table). Persisted in AppConfig; edited as radios in
    /// Settings > Gameplay; shown as a HUD tag when not Normal. Dev is the
    /// pre-launch default (the operator IS the dev); flips to Normal at
    /// launch.
    pub play_mode: crate::config::PlayMode,
    // Wallet: no Settings fields - the live selector state is
    // GuiState::wallet_network (shared by the Wallet page and Settings >
    // Wallet; v0.980 removed the dead duplicates nothing read).
    // Privacy
    pub profile_visible: bool,
    pub online_status_visible: bool,
    /// Chosen privacy tier id (data/gui/privacy_tiers.json). Empty =
    /// never chosen; the first-connect modal asks once. (2026-08-23)
    pub privacy_tier: String,
    // Data
    pub seed_phrase_visible: bool,
    // Seed phrase recovery
    pub seed_phrase_input: String,
    pub seed_phrase_recovery_status: String,
    pub seed_phrase_show_recover: bool,
}

#[cfg(feature = "native")]
impl Default for SettingsState {
    fn default() -> Self {
        Self {
            category: SettingsCategory::Graphics,
            scroll_to_section: None,
            fullscreen: false,
            window_mode: crate::config::WindowMode::default(),
            vsync: true,
            fps_foreground: 120,
            fps_foreground_unlimited: true,
            fps_background: 30,
            fps_background_sync: true,
            fov: 90.0,
            render_distance: 500.0,
            planet_detail: true,
            sky_orbit_mode: "planets".to_string(),
            sky_constellations: true,
            sky_milkyway_glow: true,
            sky_milkyway_intensity: 1.0,
            sky_glow_tier: "standard".to_string(),
            star_catalog_tier: "auto".to_string(),
            sky_star_halos: true,
            planet_lod_px: 10.0,
            terrain_split_px: 4.0,
            terrain_patch_budget: 2048.0,
            terrain_detail_distance: 1.5,
            terrain_builds_per_frame: 64.0,
            tree_model_distance: crate::lod_registry::category("tree").map(|c| c.model_m).unwrap_or(120.0),
            // v0.1109: defaults are the shipped engine constants, so exposing
            // these three changed nothing about how the world looks.
            near_tree_budget: crate::config::NEAR_TREE_BUDGET_DEFAULT,
            grass_far_m: crate::terrain::grass::GRASS_FAR_M,
            grass_harvest_cap: crate::config::GRASS_HARVEST_CAP_DEFAULT,
            tree_density: 0.6,
            grass_density: 1.0,
            grass_detail: 0.6,
            veg_tree_card_m: crate::lod_registry::category("tree").map(|c| c.card_m).unwrap_or(1500.0),
            water_detail_depth: 20.0,
            sun_shadows: true,
            shadow_strength: 1.0,
            crop_growth_speed: crate::systems::farming::DEFAULT_CROP_GROWTH_SPEED,
            offline_progression: true,
            fresh_world_each_launch: true,
            aerial_strength: 1.0,
            godray_intensity: 0.55,
            ssao_strength: 0.55,
            planet_max_subdiv: 6.0,
            planet_chunked: true,
            terrain_lod_fade: true,
            planet_atmo_scatter: true,
            planet_clouds: true,
            // Default OFF (2026-08-24): the live map's far-placement handoff
            // visibly replaced the deck mid-ascent; procedural is coherent
            // at every altitude. The Settings toggle turns it back on.
            live_weather: false,
            readable_web: false,
            screen_media: std::collections::BTreeMap::new(),
            ffmpeg_path: String::new(),
            track_station: true,
            planet_surface_detail: true,
            water_fft: false,
            water_clarity: default_water_clarity(),
            precip_density: default_precip_density(),
            fog_density: default_fog_density(),
            gpu_particles: true,
            far_tree_sheet: false,
            cloud_quality: "high".to_string(),
            lights_tiled: false,
            master_volume: 0.8,
            music_volume: 0.5,
            sfx_volume: 0.7,
            ui_volume: 1.0,
            ui_sounds_enabled: true,
            mouse_sensitivity: 0.25,
            invert_y: false,
            dark_mode: true,
            font_size: 14.0,
            hint_display: HintDisplay::default(),
            home_variant: "home".to_string(),
            hostile_wildlife: false,
            vitals_drain: 1.0,
            hud_vitals: crate::config::HudVitals::default(),
            play_mode: crate::config::PlayMode::default(),
            profile_visible: true,
            online_status_visible: true,
            privacy_tier: String::new(),
            seed_phrase_visible: false,
            seed_phrase_input: String::new(),
            seed_phrase_recovery_status: String::new(),
            seed_phrase_show_recover: false,
        }
    }
}

#[cfg(all(test, feature = "native"))]
mod ui_click_sound_tests {
    use super::ui_click_should_sound;
    use egui::output::OutputEvent;
    use egui::{WidgetInfo, WidgetType};

    fn clicked_button() -> OutputEvent {
        OutputEvent::Clicked(WidgetInfo::new(WidgetType::Button))
    }

    /// The whole point of the fix: with NO widget-activation event this frame
    /// (the pointer was pressed over empty panel background), the click sound
    /// must NOT fire. This is the operator's complaint reproduced as logic -
    /// before the fix, being over any egui area was enough.
    #[test]
    fn no_click_event_means_no_sound() {
        // Events that are NOT a real click: focus + value-change happen without
        // a pointer click, and empty background produces no events at all.
        let not_clicks = [
            OutputEvent::FocusGained(WidgetInfo::new(WidgetType::TextEdit)),
            OutputEvent::ValueChanged(WidgetInfo::new(WidgetType::Slider)),
        ];
        assert!(
            !ui_click_should_sound(&[], true),
            "empty-background frame (no events) must be silent"
        );
        assert!(
            !ui_click_should_sound(&not_clicks, true),
            "focus/value-change without a click must be silent"
        );
    }

    /// A genuine widget activation DOES fire the sound.
    #[test]
    fn real_click_event_makes_sound() {
        assert!(ui_click_should_sound(&[clicked_button()], true));
        assert!(ui_click_should_sound(
            &[OutputEvent::DoubleClicked(WidgetInfo::new(WidgetType::Button))],
            true
        ));
    }

    /// The "Interface sounds" master switch gates even a real click.
    #[test]
    fn disabled_switch_silences_a_real_click() {
        assert!(
            !ui_click_should_sound(&[clicked_button()], false),
            "with interface sounds disabled, even a real click is silent"
        );
    }
}

#[cfg(all(test, feature = "native"))]
mod map_planet_tests {
    // The Maps page planet list rendered EMPTY for months because
    // load_planets read a bodies.json that stopped shipping and the error
    // only went to stderr. This asserts the OUTCOME (a populated list with
    // believable facts), so a broken data path can never again pass silently.
    #[test]
    fn maps_page_planet_list_is_populated_from_the_catalog() {
        let planets = super::load_planets();
        assert!(
            planets.len() >= 8,
            "expected at least the 8 major planets, got {}",
            planets.len()
        );
        let earth = planets
            .iter()
            .find(|p| p.name == "Earth")
            .expect("Earth present in the Maps planet list");
        assert!((earth.gravity - 9.81).abs() < 0.2, "Earth gravity ~9.81");
        assert!(earth.moons >= 1, "Earth should list at least the Moon");
        assert!(
            (earth.orbit_radius_au - 1.0).abs() < 0.05,
            "Earth orbits at ~1 AU"
        );
        assert!(!earth.atmosphere.is_empty() && earth.atmosphere != "None");
    }
}
