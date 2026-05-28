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

/// Current engine version (read from Cargo.toml at compile time).
#[cfg(feature = "native")]
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A tool entry loaded from data/tools/catalog.json.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub description: String,
    pub url: String,
    pub license: String,
    pub platforms: Vec<String>,
    /// Optional download size hint (e.g. "~350MB").
    #[serde(default)]
    pub size: String,
    /// Category name, populated during loading from the parent category.
    #[serde(skip)]
    pub category: String,
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
    Civilization,
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
    Resources,
    Donate,
    Tools,
    Studio,
    /// First-run orientation plus permanent reference page.
    /// Mirrors the web `/onboarding` page.
    Onboarding,
    /// Server / group administration settings page. Opened from the cog
    /// menu on the server or group row in the chat sidebar.
    ServerSettings,
    /// Identity hub: DID, Verifiable Credentials, trust score, AI status.
    /// Mirrors the web `/identity` page.
    Identity,
    /// Local + civilization-scope governance: proposals, votes, tally.
    /// Mirrors the web `/governance` page.
    Governance,
    /// Social key recovery setup + active recovery requests.
    /// Mirrors the web `/recovery` page.
    Recovery,
    // v0.197.0: removed Agents and AiUsage GuiPage variants. Operator
    // 2026-05-08: "That AI Agents page also seems useless. As well as
    // the AI usage." Multi-AI orchestration is handled via
    // data/coordination/* + the relay agent_sessions table — the UI
    // pages weren't pulling their weight. The page modules + state
    // fields were removed in the same release.
    /// Cosmos page (v0.203.0, Phase 3). Three-mode astronomical map:
    /// System (Sol planets), Galactic (Sol-centered nearby stars in ly),
    /// Night Sky (Earth-centered celestial sphere with constellations).
    /// Lives under Sim category in the nav. See pages/cosmos.rs.
    Cosmos,
    /// QA testing tasks — operator-facing checklist of features to manually verify.
    /// Each task has Mark Passed / Report Issue buttons that post results to chat.
    Testing,
    /// Curated bookmarks page. First step toward the in-app browser — for
    /// now each card opens its URL in the OS default browser via egui's
    /// open_url. Data lives in `data/browser/bookmarks.json`.
    Browser,
    /// Top-category overview / landing pages (v0.181.0). Each top-tier
    /// nav button (Reality / Sim / Tools / Settings / Dev) lands on the
    /// matching overview, which renders a card grid of every sub-page
    /// in that category with a one-line description. Sub-page nav items
    /// remain available for direct navigation; the overview is the
    /// "browse / discover what's in this category" entry point.
    OverviewReality,
    OverviewSim,
    OverviewTools,
    OverviewSettings,
    OverviewDev,
    /// Settings sub-pages (v0.182.0). Each replaces one section of the
    /// former single-scroll Settings page. The sub-tier nav lists these
    /// individually so users navigate straight to a category instead of
    /// scrolling. The legacy `GuiPage::Settings` variant still works
    /// (renders the all-in-one view) for backwards compatibility with
    /// saved deep links, but the nav no longer points to it.
    SettingsAccount,
    SettingsAppearance,
    SettingsAnimations,
    SettingsWidgets,
    SettingsNotifications,
    SettingsWallet,
    SettingsAudio,
    SettingsGraphics,
    SettingsControls,
    SettingsPrivacy,
    SettingsData,
    SettingsUpdates,
}

/// Pages that can be selected as the startup boot page.
#[cfg(feature = "native")]
pub const BOOT_PAGE_OPTIONS: &[(GuiPage, &str)] = &[
    (GuiPage::Onboarding, "Landing / Onboarding"),
    (GuiPage::Chat, "Chat"),
    (GuiPage::Tasks, "Tasks"),
    (GuiPage::Maps, "Maps"),
    (GuiPage::Notes, "Notes"),
    (GuiPage::Calendar, "Calendar"),
    (GuiPage::Cosmos, "Cosmos"),
    (GuiPage::Resources, "Resources"),
];

#[cfg(feature = "native")]
pub fn page_to_config_str(page: GuiPage) -> &'static str {
    match page {
        GuiPage::Onboarding => "onboarding",
        GuiPage::Chat => "chat",
        GuiPage::Tasks => "tasks",
        GuiPage::Maps => "maps",
        GuiPage::Notes => "notes",
        GuiPage::Calendar => "calendar",
        GuiPage::Cosmos => "cosmos",
        GuiPage::Resources => "resources",
        _ => "onboarding",
    }
}

#[cfg(feature = "native")]
pub fn config_str_to_page(s: &str) -> GuiPage {
    match s {
        "onboarding" => GuiPage::Onboarding,
        "chat" => GuiPage::Chat,
        "tasks" => GuiPage::Tasks,
        "maps" => GuiPage::Maps,
        "notes" => GuiPage::Notes,
        "calendar" => GuiPage::Calendar,
        "cosmos" => GuiPage::Cosmos,
        "resources" => GuiPage::Resources,
        _ => GuiPage::Onboarding,
    }
}

/// Item slot data bridged from ECS Inventory for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiItemSlot {
    /// Item ID from items.csv.
    pub item_id: String,
    /// Human-readable item name (looked up from ItemRegistry).
    pub name: String,
    /// Quantity in this stack.
    pub quantity: u32,
}

/// Game time snapshot bridged from TimeSystem for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiGameTime {
    pub hour: f32,
    pub day_count: u32,
    pub season: String,
    pub is_daytime: bool,
}

/// Weather snapshot bridged from WeatherSystem for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiWeather {
    pub condition: String,
    pub temperature: f32,
    pub wind_speed: f32,
}

/// Task priority levels for the task board.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Task status for kanban columns.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
}

/// A task for the GUI task board.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiTask {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub assignee: String,
    pub labels: Vec<String>,
}

/// A marketplace listing.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiListing {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub price: f64,
    pub seller: String,
    pub category: String,
}

/// Planet data for the map viewer.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiPlanet {
    pub name: String,
    pub planet_type: String,
    pub radius_km: f64,
    pub gravity: f64,
    pub atmosphere: String,
    pub moons: u32,
    pub orbit_radius_au: f64,
}

/// A calendar event.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiCalendarEvent {
    pub title: String,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub time: String,
    pub color: egui::Color32,
}

/// A note entry.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiNote {
    pub id: u64,
    pub title: String,
    pub content: String,
    /// Unix timestamp of last modification.
    pub modified: u64,
}

/// Wallet network selector.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletNetwork {
    Mainnet,
    Devnet,
    Testnet,
}

#[cfg(feature = "native")]
impl WalletNetwork {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mainnet => "Mainnet",
            Self::Devnet => "Devnet",
            Self::Testnet => "Testnet",
        }
    }
}

/// A wallet transaction entry.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct WalletTransaction {
    pub signature: String,
    pub direction: String,
    pub amount: f64,
    pub counterparty: String,
    pub timestamp: String,
}

/// A crafting recipe for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiRecipe {
    pub id: String,
    pub name: String,
    pub category: String,
    pub inputs: Vec<(String, u32)>,
    pub outputs: Vec<(String, u32)>,
    pub craft_time_sec: f32,
    pub station_required: String,
    pub description: String,
}

/// A guild for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiGuild {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub color: egui::Color32,
    pub members: Vec<String>,
    pub is_member: bool,
}

/// A chat message received from or sent to the relay server.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct ChatMessage {
    pub sender_name: String,
    pub sender_key: String,
    pub content: String,
    /// Display-formatted timestamp string (e.g. "12:34:56").
    pub timestamp: String,
    /// Original numeric timestamp (ms since epoch). Used for reaction targeting
    /// (Reaction message references messages by sender_key + timestamp_ms).
    pub timestamp_ms: u64,
    pub channel: String,
    /// Reactions: emoji → list of sender public keys who reacted.
    /// Count = `reactions[emoji].len()`. Stored as Vec rather than count so
    /// we can prevent duplicate reactions per user and toggle.
    pub reactions: std::collections::HashMap<String, Vec<String>>,
    /// If this message is a reply, the parent message context.
    pub reply_to: Option<ReplyContext>,
}

/// Cached parent-message context for a thread reply.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ReplyContext {
    pub sender_key: String,
    pub sender_name: String,
    /// Preview snippet of the parent message (truncated to ~100 chars by render).
    pub preview: String,
    pub timestamp_ms: u64,
}

/// One row in the search results list.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatSearchResult {
    pub channel: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp_ms: u64,
}

/// One pinned message in a channel. Mirrors the relay's PinData type
/// so the WS handler can decode without a separate adapter.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatPin {
    pub from_key: String,
    pub from_name: String,
    pub content: String,
    pub original_timestamp: u64,
    pub pinned_by: String,
    pub pinned_at: u64,
}

/// A user visible in the chat user list.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatUser {
    pub name: String,
    pub public_key: String,
    pub role: String,
    pub status: String,
}

/// A channel in the channel list.
/// A DM that the user clicked Send on, but which we COULDN'T encrypt
/// (recipient's ECDH key not known, our key not set, or encryption
/// errored). Stored on GuiState so a confirmation modal can pop up
/// asking the user to either send it as plaintext anyway, or cancel.
///
/// Backstory: before v0.199 the code silently sent the DM as plaintext
/// with only a log message. Operator security audit (B3, 2026-04-30)
/// flagged this as a downgrade attack vector — an attacker who can
/// suppress ECDH key announcements could strip encryption from a DM
/// the user thinks is private. The confirmation modal forces explicit
/// user opt-in for any plaintext send.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct PendingUnencryptedDm {
    /// Recipient's public key (Ed25519 hex).
    pub partner_key: String,
    /// Recipient's display name (best-known label for the modal copy).
    pub partner_name: String,
    /// The plaintext message body the user typed.
    pub content: String,
    /// Original send timestamp (ms since epoch). Reused on confirm so
    /// the eventual sent message has the same `ts` the user clicked Send at.
    pub timestamp_ms: u64,
    /// Why we can't encrypt — one of:
    ///   "missing_peer_key"     — recipient hasn't broadcast their ECDH pub key yet
    ///   "no_own_ecdh"          — this client doesn't have its own ECDH key set
    ///   "encryption_failed: X" — encrypt_dm() errored with X
    pub reason: String,
}

#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatChannel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    /// Whether voice is currently active/joined for this channel.
    pub voice_joined: bool,
    /// Whether voice is enabled for this channel (shows mic icon).
    pub voice_enabled: bool,
    /// Whether the channel is read-only for non-admins. Settable from the
    /// Server Settings → Channels page and the chat channel-edit modal;
    /// persisted by the relay's admin-gated `channel_update` handler.
    pub read_only: bool,
    /// Whether the channel federates to peer servers. Settable from
    /// Server Settings → Channels; persisted via `channel_update`.
    pub federated: bool,
}

/// In-flight edit state for one row of the Server Settings → Channels
/// spreadsheet (v0.188.0). Cloned from the live channel into the draft
/// when the row is opened, written back via slash command on Save.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct ChannelDraft {
    pub name: String,
    pub description: String,
    pub read_only: bool,
    pub federated: bool,
    pub voice_enabled: bool,
}

/// A DM conversation entry for the left panel.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatDm {
    pub user_name: String,
    pub user_key: String,
    pub last_message: String,
    pub timestamp: String,
    pub unread: bool,
}

/// A group chat entry for the left panel.
/// Each group acts like a mini-server with its own channels.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatGroup {
    pub name: String,
    pub id: String,
    pub member_count: u32,
    /// Group channels (default: just #general)
    pub channels: Vec<ChatChannel>,
    /// Whether this group's channel list is collapsed in the sidebar
    pub collapsed: bool,
}

/// A server entry for the left panel (each server has text + voice channels).
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatServer {
    pub name: String,
    pub channels: Vec<ChatChannel>,
    pub voice_channels: Vec<String>,
    /// Stable id (typically `srv_<url>`) for nav highlighting + dedupe.
    /// (v0.187.0)
    pub id: String,
    /// Relay URL for this server (e.g. https://example.com). When the
    /// user clicks this server's row in the chat sidebar, the client
    /// reconnects to this URL. (v0.187.0)
    pub url: String,
    /// True iff the websocket to this server is currently open.
    /// (v0.187.0)
    pub connected: bool,
}

/// Studio source type variants.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub enum StudioSourceType {
    Camera(u32),
    Screen(u32),
    Microphone(u32),
    ChatOverlay,
    Image(String),
    Text(String),
    Timer,
}

/// A source in the broadcasting studio.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct StudioSource {
    pub name: String,
    pub source_type: StudioSourceType,
    pub visible: bool,
    /// Normalized position (0.0..1.0) within the preview area.
    pub position: (f32, f32),
    /// Normalized size (0.0..1.0) within the preview area.
    pub size: (f32, f32),
    pub opacity: f32,
    pub z_order: u32,
}

/// A scene preset storing which sources are active.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct StudioScene {
    pub name: String,
    pub is_default: bool,
    /// Per-source visibility override (indexed same as StudioState.sources).
    pub source_visibility: Vec<bool>,
}

/// All state for the broadcasting studio page.
#[cfg(feature = "native")]
pub struct StudioState {
    pub scenes: Vec<StudioScene>,
    pub active_scene_index: usize,
    pub sources: Vec<StudioSource>,
    pub selected_source_index: Option<usize>,
    pub is_live: bool,
    pub is_paused: bool,
    pub is_afk: bool,
    pub afk_start_time: f64,
    pub live_start_time: f64,
    pub stream_platform: String,
    pub stream_key: String,
    pub stream_server_url: String,
    pub stream_resolution: String,
    pub stream_bitrate: u32,
    pub stream_fps: u32,
    pub chat_overlay_channel: String,
    pub chat_overlay_font_size: f32,
    pub chat_overlay_position: String,
    pub chat_overlay_opacity: f32,
    pub chat_overlay_max_messages: u32,
    pub chat_overlay_bg_opacity: f32,
}

#[cfg(feature = "native")]
impl Default for StudioState {
    fn default() -> Self {
        // Scenes and sources are populated at startup from data/studio/{scenes,sources}.json
        // by `apply_studio_presets` in lib.rs. Default starts empty — if the data files are
        // missing, the studio page renders a blank scene list rather than crashing.
        Self {
            scenes: Vec::new(),
            active_scene_index: 0,
            sources: Vec::new(),
            selected_source_index: None,
            is_live: false,
            is_paused: false,
            is_afk: false,
            afk_start_time: 0.0,
            live_start_time: 0.0,
            stream_platform: "HumanityOS Server".into(),
            stream_key: String::new(),
            stream_server_url: "wss://united-humanity.us/ws".into(),
            stream_resolution: "1920x1080".into(),
            stream_bitrate: 3500,
            stream_fps: 30,
            chat_overlay_channel: "general".into(),
            chat_overlay_font_size: 14.0,
            chat_overlay_position: "Top-Right".into(),
            chat_overlay_opacity: 0.8,
            chat_overlay_max_messages: 15,
            chat_overlay_bg_opacity: 0.3,
        }
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
    pub show_chat: bool,
    pub show_hud: bool,
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
    pub chat_groups: Vec<ChatGroup>,
    pub chat_servers: Vec<ChatServer>,
    pub chat_friends: Vec<ChatUser>,
    pub ws_client: Option<crate::net::ws_client::WsClient>,
    pub ws_status: String,
    /// Whether the user manually disconnected (suppresses auto-reconnect).
    pub ws_manually_disconnected: bool,
    /// Countdown to next reconnect attempt (seconds).
    pub ws_reconnect_timer: f32,
    /// Current reconnect delay with exponential backoff (seconds).
    pub ws_reconnect_delay: f32,
    /// Number of consecutive failed reconnect attempts.
    pub ws_reconnect_attempts: u32,
    pub selected_slot: Option<usize>,
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
    /// Whether currently connected to a server.
    pub server_connected: bool,
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
    pub map_selected_planet: Option<usize>,
    pub map_zoom: f32,

    // ── Market state ──
    pub listings: Vec<GuiListing>,
    pub listing_next_id: u32,
    pub listing_search: String,
    pub listing_filter_category: String,
    pub listing_selected: Option<usize>,
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

    // ── Civilization state ──
    pub civ_population: u32,
    pub civ_buildings: u32,
    pub civ_resources: u32,
    pub civ_tech_level: u32,
    pub civ_food: f32,
    pub civ_energy: f32,
    pub civ_water: f32,
    pub civ_happiness: f32,
    pub civ_events: Vec<String>,

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
    pub craft_category: usize,
    pub craft_status: String,

    // ── Guilds state ──
    pub guilds: Vec<GuiGuild>,
    pub guild_selected: Option<usize>,
    pub guild_search: String,
    pub guild_show_create: bool,
    pub guild_new_name: String,
    pub guild_new_desc: String,
    pub guild_new_color: egui::Color32,
    pub guild_next_id: u64,

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

    // ── Donation address config ──

    /// Admin-configurable Solana donation address (legacy).
    pub donate_solana_address: String,
    /// Admin-configurable Bitcoin donation address (legacy).
    pub donate_btc_address: String,
    /// Dynamic donation addresses fetched from server config.
    pub donate_addresses: Vec<DonateAddress>,
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

    // ── Channel create modal ──
    pub show_create_channel_modal: bool,
    pub new_channel_name: String,
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
    pub new_channel_description: String,

    // ── Create group modal (P2P signed-object groups, v0.295+) ──
    pub show_create_group_modal: bool,
    pub new_group_name: String,
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

    // ── Sidebar section settings popups (v0.195.0) ──
    // Rendered as floating Areas anchored below the section's cog
    // button. Using GuiState fields instead of egui's popup machinery
    // because the previous `popup_below_widget(... CloseOnClick ...)`
    // pattern self-closed on the trigger click — the popup flickered
    // on for one frame then disappeared (operator bug 2026-05-08).
    pub dm_settings_popup_open: bool,
    pub groups_settings_popup_open: bool,

    /// Pending unencrypted-DM confirmation (v0.199.0).
    ///
    /// When the user tries to send a DM but the recipient's ECDH key
    /// is missing or encryption fails, we DO NOT silently send plaintext
    /// (operator security audit B3 / 2026-04-30). Instead we stash the
    /// would-be message here and pop a modal asking the user to either
    /// confirm "Send unencrypted anyway" or cancel.
    pub dm_unencrypted_confirm: Option<PendingUnencryptedDm>,

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
    /// Last action result message (success or error feedback).
    pub server_settings_status: String,
    /// Whether the danger-zone confirm-delete prompt is showing.
    pub server_settings_confirm_action: Option<String>,

    // ── Debug console state ──

    /// Whether the F12 debug console overlay is visible.
    pub debug_console_visible: bool,
    /// Ring buffer of timestamped debug log lines for the overlay.
    pub debug_log: Vec<String>,

    // ── Tools catalog (loaded from data/tools/catalog.json) ──
    pub tools_catalog: Vec<ToolEntry>,

    // ── Page taxonomies (Infinite-of-X migrations, v0.123.0) ──
    /// Equipment slots for the inventory page (`data/inventory/equipment_slots.json`).
    pub equipment_slots: Vec<(String, String)>,
    /// Bug-report severity labels (`data/bugs/taxonomy.json`).
    pub bug_severities: Vec<String>,
    /// Bug-report category labels (`data/bugs/taxonomy.json`).
    pub bug_categories: Vec<String>,
    /// Crafting category filters (`data/crafting/categories.json`).
    pub crafting_categories: Vec<String>,
    /// Marketplace category filters (`data/market/categories.json`).
    pub market_categories: Vec<String>,
    /// Curated resources by category (`data/resources/catalog.json`).
    pub resource_categories: Vec<ResourceCategory>,
    /// Studio scene presets (`data/studio/scenes.json`).
    pub studio_scene_presets: Vec<StudioScenePreset>,
    /// Studio source presets (`data/studio/sources.json`).
    pub studio_source_presets: Vec<StudioSourcePreset>,
    /// Studio streaming pickers (`data/studio/streaming_config.json`).
    pub studio_streaming_config: StudioStreamingConfig,
    /// Donate page FAQ entries (`data/donate/faq.json`).
    pub donate_faq: Vec<DonateFaqEntry>,
    /// QA test tasks (`data/testing/qa_tasks.json`) shown on the Testing page.
    pub qa_test_tasks: Vec<QaTestTask>,
    /// Per-task local status: id → "passed" / "issue" / "" (untouched).
    pub qa_test_status: std::collections::HashMap<String, String>,
    /// Per-task draft note (when typing into Report Issue field).
    pub qa_test_note: std::collections::HashMap<String, String>,
    /// Filter chip on the Testing page: "all" / category id.
    pub qa_test_filter: String,
    /// Bookmark categories (`data/browser/bookmarks.json`) shown on the Browser page.
    pub browser_bookmarks: Vec<BrowserCategory>,
    /// Filter chip on the Browser page: "all" / category id.
    pub browser_filter: String,
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
    /// Onboarding concept cards (`data/onboarding/core_concepts.json`).
    pub onboarding_concepts: Vec<OnboardingConcept>,
    /// Onboarding core-page shortcuts (`data/onboarding/core_pages.json`).
    pub onboarding_core_pages: Vec<OnboardingCorePage>,
    // v0.197.0: ai_usage_filters removed (AI Usage page deleted).

    // ── Universal help modal (loaded from data/help/topics.json) ──
    /// Registry of help topics. Populated at startup from data/help/topics.json.
    pub help_registry: crate::gui::widgets::help_modal::HelpRegistry,
    /// ID of the currently-open help topic, if any. Setting this opens the help modal.
    pub active_help_topic: Option<String>,

    // ── Onboarding quest chains (loaded from data/onboarding/quests.json) ──
    /// Quest chains displayed on the Onboarding page.
    pub onboarding_quest_chains: Vec<crate::gui::pages::onboarding::QuestChain>,
    /// Map of "chain_id:step_id" -> done?. Persisted via AppConfig for local progress.
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
    pub chat_connected_server_collapsed: bool,
    pub chat_friends_collapsed: bool,
    pub chat_members_collapsed: bool,
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
}

#[cfg(feature = "native")]
impl Default for GuiState {
    fn default() -> Self {
        Self {
            active_page: GuiPage::MainMenu,
            last_page: GuiPage::Chat,
            nav_back_stack: Vec::new(),
            show_chat: false,
            show_hud: true,
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
            chat_groups: Vec::new(),
            chat_servers: Vec::new(),
            chat_friends: Vec::new(),
            ws_client: None,
            ws_status: "Not connected".to_string(),
            ws_manually_disconnected: false,
            ws_reconnect_timer: 0.0,
            ws_reconnect_delay: 5.0,
            ws_reconnect_attempts: 0,
            selected_slot: None,
            fps: 0.0,
            updater: crate::updater::Updater::new(VERSION),
            update_toast_visible: false,

            onboarding_complete: false,
            onboarding_step: 0,
            server_url: "https://united-humanity.us".to_string(),
            server_connected: false,
            user_name: "Player".to_string(),
            concept_tour_seen: false,
            default_page: GuiPage::Onboarding,

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

            // Map defaults: populated from data/solar_system/bodies.json at
            // startup in lib.rs (see `load_planets`). Empty at construction.
            map_planets: Vec::new(),
            map_selected_planet: None,
            map_zoom: 1.0,

            // Market defaults
            listings: Vec::new(),
            listing_next_id: 1,
            listing_search: String::new(),
            listing_filter_category: String::new(),
            listing_selected: None,
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
            cal_new_color: egui::Color32::from_rgb(237, 140, 36),

            // Notes defaults
            notes: Vec::new(),
            notes_selected: None,
            notes_next_id: 1,

            // Civilization defaults
            civ_population: 0,
            civ_buildings: 0,
            civ_resources: 0,
            civ_tech_level: 1,
            civ_food: 0.5,
            civ_energy: 0.5,
            civ_water: 0.5,
            civ_happiness: 0.5,
            civ_events: Vec::new(),

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
            craft_category: 0,
            craft_status: String::new(),

            // Guilds defaults
            guilds: Vec::new(),
            guild_selected: None,
            guild_search: String::new(),
            guild_show_create: false,
            guild_new_name: String::new(),
            guild_new_desc: String::new(),
            guild_new_color: egui::Color32::from_rgb(46, 134, 193),
            guild_next_id: 1,

            player_health: 1.0,
            player_health_max: 100.0,
            inventory_items: Vec::new(),
            inventory_max_slots: 36,
            game_time: None,
            weather: None,
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
            // v0.278.0 auto-unlock — default is opt-out (always prompt).
            // A loaded config overwrites this with the user's stored choice.
            auto_unlock_mode: crate::auto_unlock::AutoUnlockMode::AlwaysPrompt,
            pin_encrypted_seed: String::new(),
            pin_salt: String::new(),
            remember_on_device: false,
            pin_input: String::new(),
            pin_confirm: String::new(),
            pin_old_input: String::new(),
            pin_status: String::new(),
            kyber_public_b64: String::new(),
            peer_kyber_keys: std::collections::HashMap::new(),
            donate_solana_address: String::new(),
            donate_btc_address: String::new(),
            donate_addresses: Vec::new(),
            donate_new_network: String::new(),
            donate_new_type: "address".into(),
            donate_new_value: String::new(),
            donate_new_label: String::new(),
            chat_user_modal_open: false,
            chat_user_modal_name: String::new(),
            chat_user_modal_key: String::new(),
            show_create_channel_modal: false,
            show_add_server_modal: false,
            add_server_url_draft: String::new(),
            add_server_name_draft: String::new(),
            server_settings_tab: 0,
            server_settings_channel_drafts: std::collections::HashMap::new(),
            server_settings_new_channel: ChannelDraft::default(),
            new_channel_name: String::new(),
            new_channel_description: String::new(),
            show_create_group_modal: false,
            dm_settings_popup_open: false,
            groups_settings_popup_open: false,
            dm_unencrypted_confirm: None,
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
            chat_muted_users: Vec::new(),
            chat_muted_requested: false,
            server_settings_draft: None,
            cosmos_view: crate::gui::pages::cosmos::CosmosView::System,
            cosmos_pan: egui::Vec2::ZERO,
            cosmos_zoom: 1.0,
            cosmos_selected_body: None,
            cosmos_expanded_planets: std::collections::HashSet::new(),
            cosmos_focus_request: None,
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
            create_group_ticket: None,
            create_group_status: String::new(),
            show_join_group_modal: false,
            join_group_invite_code: String::new(),
            join_group_status: String::new(),
            join_group_result: None,
            p2p_groups: Vec::new(),
            p2p_groups_last_fetch: None,
            show_channel_edit_modal: false,
            edit_channel_id: String::new(),
            edit_channel_name: String::new(),
            edit_channel_description: String::new(),
            edit_channel_confirm_delete: false,
            server_settings_target_user: String::new(),
            server_settings_channel_name: String::new(),
            server_settings_invite_code: String::new(),
            server_settings_status: String::new(),
            server_settings_confirm_action: None,
            show_help_modal: false,
            debug_console_visible: false,
            debug_log: Vec::new(),
            tools_catalog: Vec::new(),
            equipment_slots: Vec::new(),
            bug_severities: Vec::new(),
            bug_categories: Vec::new(),
            crafting_categories: Vec::new(),
            market_categories: Vec::new(),
            resource_categories: Vec::new(),
            studio_scene_presets: Vec::new(),
            studio_source_presets: Vec::new(),
            studio_streaming_config: StudioStreamingConfig::default(),
            donate_faq: Vec::new(),
            qa_test_tasks: Vec::new(),
            qa_test_status: std::collections::HashMap::new(),
            qa_test_note: std::collections::HashMap::new(),
            qa_test_filter: "all".to_string(),
            browser_bookmarks: Vec::new(),
            browser_filter: "all".to_string(),
            // v0.174.0: default to two-tier nav for fresh installs. Existing
            // users with `nav_two_tier=false` saved in config keep their
            // legacy layout until they flip via [▤]; new sessions land on
            // the two-tier layout immediately.
            nav_two_tier: true,
            nav_top_category: "reality".to_string(),
            attack_pulse_active: false,
            attack_pulse_last_hit_at: 0.0,
            onboarding_concepts: Vec::new(),
            onboarding_core_pages: Vec::new(),
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
            chat_connected_server_collapsed: false,
            chat_friends_collapsed: false,
            chat_members_collapsed: false,
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
            governance_filter_tab: 0,
            recovery_lookup_did: String::new(),
            recovery_lookup_pending: false,
            recovery_guardian_did: String::new(),
            recovery_guardian_pending: false,

            // v0.197.0: AI Usage page form state removed (page deleted).
        }
    }
}

/// Load the tools catalog from data/tools/catalog.json.
/// `data_dir` is the root data directory (e.g. from AssetManager).
/// Returns an empty Vec on any error (graceful degradation).
#[cfg(feature = "native")]
pub fn load_tools_catalog(data_dir: &std::path::Path) -> Vec<ToolEntry> {
    /// JSON shape for the catalog file (categories with nested tools).
    #[derive(serde::Deserialize)]
    struct Catalog {
        categories: Vec<CatalogCategory>,
    }
    #[derive(serde::Deserialize)]
    struct CatalogCategory {
        name: String,
        tools: Vec<ToolEntry>,
        #[allow(dead_code)]
        #[serde(default)]
        id: String,
        #[allow(dead_code)]
        #[serde(default)]
        extensions: Vec<String>,
    }

    let path = data_dir.join("tools").join("catalog.json");
    let bytes = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[tools] Failed to read {}: {}", path.display(), e);
            return Vec::new();
        }
    };
    let catalog: Catalog = match serde_json::from_str(&bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[tools] Failed to parse catalog.json: {}", e);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for cat in catalog.categories {
        for mut tool in cat.tools {
            tool.category = cat.name.clone();
            out.push(tool);
        }
    }
    out
}

/// Load solar system planets from `data/solar_system/bodies.json`.
/// Falls back to an empty Vec if the file is missing or malformed so the
/// game still boots (the map page will show a "no planets loaded" state).
#[cfg(feature = "native")]
pub fn load_planets(data_dir: &std::path::Path) -> Vec<GuiPlanet> {
    #[derive(serde::Deserialize)]
    struct Bodies {
        bodies: Vec<Body>,
    }
    #[derive(serde::Deserialize)]
    struct Body {
        #[serde(default)]
        name: String,
        #[serde(default, rename = "type")]
        type_: String,
        #[serde(default)]
        physical: Option<Physical>,
        #[serde(default)]
        orbit: Option<Orbit>,
        #[serde(default)]
        atmosphere: Option<Atmosphere>,
        #[serde(default)]
        moons: Vec<serde_json::Value>,
    }
    #[derive(serde::Deserialize, Default)]
    struct Physical {
        #[serde(default)]
        radius_km: f64,
        #[serde(default)]
        surface_gravity_ms2: f64,
    }
    #[derive(serde::Deserialize, Default)]
    struct Orbit {
        #[serde(default)]
        semi_major_axis_au: f64,
    }
    #[derive(serde::Deserialize, Default)]
    struct Atmosphere {
        #[serde(default)]
        composition: std::collections::BTreeMap<String, f64>,
        #[serde(default)]
        surface_pressure_atm: Option<f64>,
    }

    let path = data_dir.join("solar_system").join("bodies.json");
    let bytes = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[planets] Could not read {}: {}", path.display(), e);
            return Vec::new();
        }
    };
    let parsed: Bodies = match serde_json::from_str(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[planets] Could not parse bodies.json: {}", e);
            return Vec::new();
        }
    };

    parsed
        .bodies
        .into_iter()
        .filter(|b| {
            // Show real planets and dwarf planets. Stars have no orbit_au,
            // moons have a parent != "sun". We display the simple solar
            // system view so we skip the Sun itself.
            matches!(
                b.type_.as_str(),
                "terrestrial" | "gas_giant" | "ice_giant" | "dwarf_planet"
            )
        })
        .map(|b| {
            let phys = b.physical.unwrap_or_default();
            let orbit = b.orbit.unwrap_or_default();
            let atm = b.atmosphere.unwrap_or_default();
            let atm_str = if atm.composition.is_empty() {
                if atm.surface_pressure_atm.map_or(true, |p| p < 0.001) {
                    "None".to_string()
                } else {
                    "Trace".to_string()
                }
            } else {
                // Join top 3 components by percentage, descending.
                let mut pairs: Vec<(String, f64)> =
                    atm.composition.into_iter().collect();
                pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                pairs
                    .into_iter()
                    .take(3)
                    .map(|(k, _)| k)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let planet_type = match b.type_.as_str() {
                "terrestrial" => "Rocky",
                "gas_giant" => "Gas Giant",
                "ice_giant" => "Ice Giant",
                "dwarf_planet" => "Dwarf",
                other => other,
            }
            .to_string();

            GuiPlanet {
                name: b.name,
                planet_type,
                radius_km: phys.radius_km,
                gravity: phys.surface_gravity_ms2,
                atmosphere: atm_str,
                moons: b.moons.len() as u32,
                orbit_radius_au: orbit.semi_major_axis_au,
            }
        })
        .collect()
}

// ─── Infinite-of-X data loaders (v0.123.0) ─────────────────────────────────
//
// One small JSON file per page taxonomy. All loaders share the same shape:
// graceful fallback to an empty Vec on missing/malformed input so the GUI still
// boots — pages render an empty filter row instead of crashing. The empty-vec
// path is also what the page sees during the brief window before lib.rs wires
// the loaders into GuiState at startup.

/// One curated resource entry shown on the Resources page.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResourceEntry {
    pub title: String,
    pub description: String,
    pub url: String,
}

/// A category of resources with parallel Real-mode and Sim-mode lists.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResourceCategory {
    pub name: String,
    #[serde(default)]
    pub real_resources: Vec<ResourceEntry>,
    #[serde(default)]
    pub sim_resources: Vec<ResourceEntry>,
}

/// A streaming-studio scene preset.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StudioScenePreset {
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub source_visibility: Vec<bool>,
}

/// A streaming-studio source preset. Kinds: `camera|screen|microphone|chat_overlay|image|text|timer`.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StudioSourcePreset {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub device: u32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub position: (f32, f32),
    #[serde(default)]
    pub size: (f32, f32),
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub z_order: u32,
}
#[cfg(feature = "native")]
fn one() -> f32 { 1.0 }

/// Convert a deserialised preset into the runtime [`StudioSource`].
/// Unknown `kind` values fall back to `Camera(0)` — the most benign default.
#[cfg(feature = "native")]
pub fn studio_source_from_preset(p: &StudioSourcePreset) -> StudioSource {
    let source_type = match p.kind.as_str() {
        "camera" => StudioSourceType::Camera(p.device),
        "screen" => StudioSourceType::Screen(p.device),
        "microphone" => StudioSourceType::Microphone(p.device),
        "chat_overlay" => StudioSourceType::ChatOverlay,
        "image" => StudioSourceType::Image(p.text.clone()),
        "text" => StudioSourceType::Text(p.text.clone()),
        "timer" => StudioSourceType::Timer,
        _ => StudioSourceType::Camera(p.device),
    };
    StudioSource {
        name: p.name.clone(),
        source_type,
        visible: p.visible,
        position: p.position,
        size: p.size,
        opacity: p.opacity,
        z_order: p.z_order,
    }
}

/// Convert a deserialised preset into the runtime [`StudioScene`].
#[cfg(feature = "native")]
pub fn studio_scene_from_preset(p: &StudioScenePreset) -> StudioScene {
    StudioScene {
        name: p.name.clone(),
        is_default: p.is_default,
        source_visibility: p.source_visibility.clone(),
    }
}

/// Read a JSON file under `data/` and deserialise into `T`. Logs and returns
/// `None` on any error so callers can fall back gracefully.
#[cfg(feature = "native")]
fn read_data_json<T: serde::de::DeserializeOwned>(
    data_dir: &std::path::Path,
    relative: &str,
) -> Option<T> {
    let path = data_dir.join(relative);
    let bytes = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[data] failed to read {}: {}", path.display(), e);
            return None;
        }
    };
    match serde_json::from_str::<T>(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[data] failed to parse {}: {}", path.display(), e);
            None
        }
    }
}

/// Load equipment slot definitions from `data/inventory/equipment_slots.json`.
#[cfg(feature = "native")]
pub fn load_equipment_slots(data_dir: &std::path::Path) -> Vec<(String, String)> {
    #[derive(serde::Deserialize)]
    struct Slot { id: String, label: String }
    #[derive(serde::Deserialize)]
    struct File { slots: Vec<Slot> }
    read_data_json::<File>(data_dir, "inventory/equipment_slots.json")
        .map(|f| f.slots.into_iter().map(|s| (s.id, s.label)).collect())
        .unwrap_or_default()
}

/// Load `(severities, categories)` for the bug reporter from `data/bugs/taxonomy.json`.
#[cfg(feature = "native")]
pub fn load_bug_taxonomy(data_dir: &std::path::Path) -> (Vec<String>, Vec<String>) {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)] severities: Vec<String>,
        #[serde(default)] categories: Vec<String>,
    }
    read_data_json::<File>(data_dir, "bugs/taxonomy.json")
        .map(|f| (f.severities, f.categories))
        .unwrap_or_default()
}

/// Load crafting category filters from `data/crafting/categories.json`.
#[cfg(feature = "native")]
pub fn load_crafting_categories(data_dir: &std::path::Path) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct File { categories: Vec<String> }
    read_data_json::<File>(data_dir, "crafting/categories.json")
        .map(|f| f.categories)
        .unwrap_or_default()
}

/// Load marketplace category filters from `data/market/categories.json`.
#[cfg(feature = "native")]
pub fn load_market_categories(data_dir: &std::path::Path) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct File { categories: Vec<String> }
    read_data_json::<File>(data_dir, "market/categories.json")
        .map(|f| f.categories)
        .unwrap_or_default()
}

/// Load curated resource categories from `data/resources/catalog.json`.
#[cfg(feature = "native")]
pub fn load_resource_categories(data_dir: &std::path::Path) -> Vec<ResourceCategory> {
    #[derive(serde::Deserialize)]
    struct File { categories: Vec<ResourceCategory> }
    read_data_json::<File>(data_dir, "resources/catalog.json")
        .map(|f| f.categories)
        .unwrap_or_default()
}

/// Load streaming-studio scene presets from `data/studio/scenes.json`.
#[cfg(feature = "native")]
pub fn load_studio_scenes(data_dir: &std::path::Path) -> Vec<StudioScenePreset> {
    #[derive(serde::Deserialize)]
    struct File { scenes: Vec<StudioScenePreset> }
    read_data_json::<File>(data_dir, "studio/scenes.json")
        .map(|f| f.scenes)
        .unwrap_or_default()
}

/// Load streaming-studio source presets from `data/studio/sources.json`.
#[cfg(feature = "native")]
pub fn load_studio_sources(data_dir: &std::path::Path) -> Vec<StudioSourcePreset> {
    #[derive(serde::Deserialize)]
    struct File { sources: Vec<StudioSourcePreset> }
    read_data_json::<File>(data_dir, "studio/sources.json")
        .map(|f| f.sources)
        .unwrap_or_default()
}

/// Picker options for the Broadcasting Studio page (platforms, resolutions, etc.).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct StudioStreamingConfig {
    #[serde(default)] pub platforms: Vec<String>,
    #[serde(default)] pub resolutions: Vec<String>,
    #[serde(default)] pub fps: Vec<u32>,
    #[serde(default)] pub chat_positions: Vec<String>,
}

/// Load streaming pickers from `data/studio/streaming_config.json`.
#[cfg(feature = "native")]
pub fn load_studio_streaming_config(data_dir: &std::path::Path) -> StudioStreamingConfig {
    read_data_json::<StudioStreamingConfig>(data_dir, "studio/streaming_config.json")
        .unwrap_or_default()
}

/// One Q&A entry on the Donate page FAQ.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DonateFaqEntry {
    pub question: String,
    pub answer: String,
}

/// Load donate-page FAQ entries from `data/donate/faq.json`.
#[cfg(feature = "native")]
pub fn load_donate_faq(data_dir: &std::path::Path) -> Vec<DonateFaqEntry> {
    #[derive(serde::Deserialize)]
    struct File { entries: Vec<DonateFaqEntry> }
    read_data_json::<File>(data_dir, "donate/faq.json")
        .map(|f| f.entries)
        .unwrap_or_default()
}

/// One QA test task surfaced on the Testing page.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct QaTestTask {
    pub id: String,
    #[serde(default)] pub version: String,
    pub feature: String,
    pub what_to_test: String,
    pub expected: String,
    #[serde(default)] pub category: String,
    #[serde(default)] pub note: Option<String>,
}

/// Load QA test tasks from `data/testing/qa_tasks.json`.
#[cfg(feature = "native")]
pub fn load_qa_test_tasks(data_dir: &std::path::Path) -> Vec<QaTestTask> {
    #[derive(serde::Deserialize)]
    struct File { tasks: Vec<QaTestTask> }
    read_data_json::<File>(data_dir, "testing/qa_tasks.json")
        .map(|f| f.tasks)
        .unwrap_or_default()
}

/// One bookmark on the Browser page (curated link to an external site).
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BrowserBookmark {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)] pub description: String,
    #[serde(default)] pub icon: String,
}

/// A category of bookmarks. Color is one of: accent, info, success, warning, danger.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BrowserCategory {
    pub id: String,
    pub name: String,
    #[serde(default = "default_browser_color")] pub color: String,
    pub bookmarks: Vec<BrowserBookmark>,
}

#[cfg(feature = "native")]
fn default_browser_color() -> String { "accent".to_string() }

/// Load browser bookmarks from `data/browser/bookmarks.json`.
#[cfg(feature = "native")]
pub fn load_browser_bookmarks(data_dir: &std::path::Path) -> Vec<BrowserCategory> {
    #[derive(serde::Deserialize)]
    struct File { categories: Vec<BrowserCategory> }
    read_data_json::<File>(data_dir, "browser/bookmarks.json")
        .map(|f| f.categories)
        .unwrap_or_default()
}

/// Concept card shown on the onboarding page.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OnboardingConcept {
    pub title: String,
    pub body: String,
}

/// Load onboarding concepts from `data/onboarding/core_concepts.json`.
#[cfg(feature = "native")]
pub fn load_onboarding_concepts(data_dir: &std::path::Path) -> Vec<OnboardingConcept> {
    #[derive(serde::Deserialize)]
    struct File { concepts: Vec<OnboardingConcept> }
    read_data_json::<File>(data_dir, "onboarding/core_concepts.json")
        .map(|f| f.concepts)
        .unwrap_or_default()
}

/// Core page shortcut shown on the onboarding page. `page_id` is mapped to a
/// `GuiPage` variant by the onboarding draw code.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OnboardingCorePage {
    pub page_id: String,
    pub label: String,
    pub description: String,
}

/// Load onboarding core-page shortcuts from `data/onboarding/core_pages.json`.
#[cfg(feature = "native")]
pub fn load_onboarding_core_pages(data_dir: &std::path::Path) -> Vec<OnboardingCorePage> {
    #[derive(serde::Deserialize)]
    struct File { pages: Vec<OnboardingCorePage> }
    read_data_json::<File>(data_dir, "onboarding/core_pages.json")
        .map(|f| f.pages)
        .unwrap_or_default()
}

// v0.197.0: AiUsageFilters and load_ai_usage_filters removed (AI Usage
// page deleted along with its data/ai_usage/filters.json loader).

/// Load default task project names from `data/tasks/default_projects.json`.
/// These seed the task board for brand-new identities; existing users keep
/// their own list.
#[cfg(feature = "native")]
pub fn load_default_task_projects(data_dir: &std::path::Path) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct File { projects: Vec<String> }
    read_data_json::<File>(data_dir, "tasks/default_projects.json")
        .map(|f| f.projects)
        .unwrap_or_default()
}

/// Load the starting per-skill XP profile applied to brand-new identities,
/// from `data/skills/default_profile.json`. The skill catalog itself lives in
/// `data/skills/skills.csv`; this file is just the initial XP weights.
#[cfg(feature = "native")]
pub fn load_default_player_skills(data_dir: &std::path::Path) -> Vec<(String, f32)> {
    #[derive(serde::Deserialize)]
    struct Skill { name: String, xp: f32 }
    #[derive(serde::Deserialize)]
    struct File { skills: Vec<Skill> }
    read_data_json::<File>(data_dir, "skills/default_profile.json")
        .map(|f| f.skills.into_iter().map(|s| (s.name, s.xp)).collect())
        .unwrap_or_default()
}

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
    Controls,
    Privacy,
    Data,
    Updates,
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
    pub vsync: bool,
    pub fov: f32,
    pub render_distance: f32,
    // Audio
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    // Controls
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    // Appearance
    pub dark_mode: bool,
    pub font_size: f32,
    // Notifications
    pub notify_dm: bool,
    pub notify_mentions: bool,
    pub notify_tasks: bool,
    pub dnd_start: String,
    pub dnd_end: String,
    // Wallet
    pub wallet_network: WalletNetwork,
    pub custom_rpc_url: String,
    // Privacy
    pub profile_visible: bool,
    pub online_status_visible: bool,
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
            vsync: true,
            fov: 90.0,
            render_distance: 500.0,
            master_volume: 0.8,
            music_volume: 0.5,
            sfx_volume: 0.7,
            mouse_sensitivity: 3.0,
            invert_y: false,
            dark_mode: true,
            font_size: 14.0,
            notify_dm: true,
            notify_mentions: true,
            notify_tasks: true,
            dnd_start: String::new(),
            dnd_end: String::new(),
            wallet_network: WalletNetwork::Devnet,
            custom_rpc_url: String::new(),
            profile_visible: true,
            online_status_visible: true,
            seed_phrase_visible: false,
            seed_phrase_input: String::new(),
            seed_phrase_recovery_status: String::new(),
            seed_phrase_show_recover: false,
        }
    }
}
