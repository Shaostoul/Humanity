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

/// What the passphrase prompt is for.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassphraseMode {
    /// Setting a new passphrase (first time or migration from plaintext).
    SetNew,
    /// Unlocking an existing encrypted key.
    Unlock,
    /// Changing the passphrase (requires old + new).
    Change,
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
    /// Multi-AI agent coordination dashboard: scope registry + status + overrides.
    /// Mirrors the web `/agents` page.
    Agents,
    /// AI subscription quota tracker + usage event log.
    /// Mirrors the web `/ai-usage` page.
    AiUsage,
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
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub sender_name: String,
    pub sender_key: String,
    pub content: String,
    pub timestamp: String,
    pub channel: String,
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
    pub show_chat: bool,
    pub show_hud: bool,
    pub settings: SettingsState,
    pub chat_input: String,
    pub chat_messages: Vec<ChatMessage>,
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
    /// Real/Sim context mode. true = real (default), false = sim.
    pub context_real: bool,
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
    /// ECDH P-256 private key (32 bytes, hex-encoded). For E2E encrypted DMs.
    pub ecdh_private_hex: String,
    /// ECDH P-256 public key (base64 SEC1 uncompressed, 65 bytes).
    pub ecdh_public_b64: String,
    /// Map of peer public key hex -> their ECDH public key base64.
    /// Populated from peer_list, full_user_list, profile_data, peer_joined.
    pub peer_ecdh_keys: std::collections::HashMap<String, String>,
    /// Temporary input field for importing an ECDH key (e.g. from web client).
    /// Format: "privateKeyPkcs8Base64|publicKeyRawBase64" (matching web's backup format).
    pub ecdh_import_input: String,
    /// Status message from the last ECDH import attempt.
    pub ecdh_import_status: String,

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
    pub new_channel_description: String,

    // ── Create group modal ──
    pub show_create_group_modal: bool,
    pub new_group_name: String,

    // ── Join group modal ──
    pub show_join_group_modal: bool,
    pub join_group_invite_code: String,

    // ── Channel edit modal ──
    pub show_channel_edit_modal: bool,
    pub edit_channel_id: String,
    pub edit_channel_name: String,
    pub edit_channel_description: String,
    /// Whether the delete confirmation is showing in the edit modal.
    pub edit_channel_confirm_delete: bool,
    /// Whether the slash commands help modal is visible.
    pub show_help_modal: bool,

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

    // ── AI Usage page form state (v0.121.0) ──
    pub ai_usage_quota_provider: String,
    pub ai_usage_quota_window: String,
    pub ai_usage_quota_used: String,
    pub ai_usage_quota_limit: String,
    pub ai_usage_quota_resets: String,
    pub ai_usage_event_provider: String,
    pub ai_usage_event_model: String,
    pub ai_usage_event_input: String,
    pub ai_usage_event_output: String,
    pub ai_usage_event_notes: String,
}

#[cfg(feature = "native")]
impl Default for GuiState {
    fn default() -> Self {
        Self {
            active_page: GuiPage::MainMenu,
            last_page: GuiPage::Chat,
            show_chat: false,
            show_hud: true,
            settings: SettingsState::default(),
            chat_input: String::new(),
            chat_messages: Vec::new(),
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
            context_real: true,
            default_page: GuiPage::Chat,

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
            profile_skills: vec![
                ("Farming".into(), 0.3),
                ("Crafting".into(), 0.1),
                ("Trading".into(), 0.0),
                ("Building".into(), 0.05),
                ("Cooking".into(), 0.0),
                ("Mining".into(), 0.0),
                ("Combat".into(), 0.0),
                ("Navigation".into(), 0.0),
            ],
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
            ecdh_private_hex: String::new(),
            ecdh_public_b64: String::new(),
            peer_ecdh_keys: std::collections::HashMap::new(),
            ecdh_import_input: String::new(),
            ecdh_import_status: String::new(),
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
            new_channel_name: String::new(),
            new_channel_description: String::new(),
            show_create_group_modal: false,
            new_group_name: String::new(),
            show_join_group_modal: false,
            join_group_invite_code: String::new(),
            show_channel_edit_modal: false,
            edit_channel_id: String::new(),
            edit_channel_name: String::new(),
            edit_channel_description: String::new(),
            edit_channel_confirm_delete: false,
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

            // AI Usage page form state (v0.121.0)
            ai_usage_quota_provider: "claude".to_string(),
            ai_usage_quota_window: "5h".to_string(),
            ai_usage_quota_used: String::new(),
            ai_usage_quota_limit: String::new(),
            ai_usage_quota_resets: String::new(),
            ai_usage_event_provider: "claude".to_string(),
            ai_usage_event_model: String::new(),
            ai_usage_event_input: String::new(),
            ai_usage_event_output: String::new(),
            ai_usage_event_notes: String::new(),
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

#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Account,
    Appearance,
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
