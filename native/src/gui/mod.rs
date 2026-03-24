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
    None,
    MainMenu,
    Settings,
    Inventory,
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

/// Tracks all GUI state for the native app.
#[cfg(feature = "native")]
pub struct GuiState {
    pub active_page: GuiPage,
    pub show_chat: bool,
    pub show_hud: bool,
    pub settings: SettingsState,
    pub chat_input: String,
    pub chat_messages: Vec<String>,
    pub selected_slot: Option<usize>,
    pub fps: f32,
    pub updater: crate::updater::Updater,
    /// Set true when an update notification toast should show.
    pub update_toast_visible: bool,

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
            selected_slot: None,
            fps: 0.0,
            updater: crate::updater::Updater::new(VERSION),
            update_toast_visible: false,

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
