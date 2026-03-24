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

/// Which page/overlay is currently active.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPage {
    /// In-game, no menu overlay (HUD still visible).
    None,
    /// Title screen: Play, Settings, Quit.
    MainMenu,
    /// Escape menu: full nav to all pages.
    EscapeMenu,
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
}

/// Tracks all GUI state for the native app.
#[cfg(feature = "native")]
pub struct GuiState {
    pub active_page: GuiPage,
    pub show_chat: bool,
    pub show_hud: bool,
    pub settings: SettingsState,
    pub chat_input: String,
    pub chat_messages: Vec<ChatMessage>,
    pub chat_channels: Vec<ChatChannel>,
    pub chat_active_channel: String,
    pub chat_users: Vec<ChatUser>,
    pub ws_client: Option<crate::net::ws_client::WsClient>,
    pub ws_status: String,
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
}

#[cfg(feature = "native")]
impl Default for GuiState {
    fn default() -> Self {
        Self {
            active_page: GuiPage::MainMenu,
            show_chat: false,
            show_hud: true,
            settings: SettingsState::default(),
            chat_input: String::new(),
            chat_messages: Vec::new(),
            chat_channels: vec![
                ChatChannel { id: "general".into(), name: "general".into(), description: "General discussion".into(), category: "Text".into() },
                ChatChannel { id: "stream".into(), name: "stream".into(), description: "Live stream chat".into(), category: "Text".into() },
                ChatChannel { id: "dev".into(), name: "dev".into(), description: "Development".into(), category: "Text".into() },
            ],
            chat_active_channel: "general".to_string(),
            chat_users: Vec::new(),
            ws_client: None,
            ws_status: "Not connected".to_string(),
            selected_slot: None,
            fps: 0.0,
            updater: crate::updater::Updater::new(VERSION),
            update_toast_visible: false,

            onboarding_complete: false,
            onboarding_step: 0,
            server_url: "https://united-humanity.us".to_string(),
            server_connected: false,
            user_name: String::new(),
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

            // Map defaults
            map_planets: default_planets(),
            map_selected_planet: Some(2), // Earth
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
        }
    }
}

/// Default solar system planet data for the map viewer.
#[cfg(feature = "native")]
fn default_planets() -> Vec<GuiPlanet> {
    vec![
        GuiPlanet { name: "Mercury".into(), planet_type: "Rocky".into(), radius_km: 2439.7, gravity: 3.7, atmosphere: "None".into(), moons: 0, orbit_radius_au: 0.39 },
        GuiPlanet { name: "Venus".into(), planet_type: "Rocky".into(), radius_km: 6051.8, gravity: 8.87, atmosphere: "CO2, N2 (dense)".into(), moons: 0, orbit_radius_au: 0.72 },
        GuiPlanet { name: "Earth".into(), planet_type: "Rocky".into(), radius_km: 6371.0, gravity: 9.81, atmosphere: "N2, O2".into(), moons: 1, orbit_radius_au: 1.0 },
        GuiPlanet { name: "Mars".into(), planet_type: "Rocky".into(), radius_km: 3389.5, gravity: 3.72, atmosphere: "CO2 (thin)".into(), moons: 2, orbit_radius_au: 1.52 },
        GuiPlanet { name: "Jupiter".into(), planet_type: "Gas Giant".into(), radius_km: 69911.0, gravity: 24.79, atmosphere: "H2, He".into(), moons: 95, orbit_radius_au: 5.2 },
        GuiPlanet { name: "Saturn".into(), planet_type: "Gas Giant".into(), radius_km: 58232.0, gravity: 10.44, atmosphere: "H2, He".into(), moons: 146, orbit_radius_au: 9.54 },
        GuiPlanet { name: "Uranus".into(), planet_type: "Ice Giant".into(), radius_km: 25362.0, gravity: 8.87, atmosphere: "H2, He, CH4".into(), moons: 28, orbit_radius_au: 19.2 },
        GuiPlanet { name: "Neptune".into(), planet_type: "Ice Giant".into(), radius_km: 24622.0, gravity: 11.15, atmosphere: "H2, He, CH4".into(), moons: 16, orbit_radius_au: 30.06 },
    ]
}

#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Graphics,
    Audio,
    Controls,
    Updates,
}

#[cfg(feature = "native")]
pub struct SettingsState {
    pub category: SettingsCategory,
    pub fullscreen: bool,
    pub vsync: bool,
    pub fov: f32,
    pub render_distance: f32,
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
}

#[cfg(feature = "native")]
impl Default for SettingsState {
    fn default() -> Self {
        Self {
            category: SettingsCategory::Graphics,
            fullscreen: false,
            vsync: true,
            fov: 90.0,
            render_distance: 500.0,
            master_volume: 0.8,
            music_volume: 0.5,
            sfx_volume: 0.7,
            mouse_sensitivity: 3.0,
            invert_y: false,
        }
    }
}
