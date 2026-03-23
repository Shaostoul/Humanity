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

/// Which page/overlay is currently active.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPage {
    None,
    MainMenu,
    Settings,
    Inventory,
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
        }
    }
}

#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Graphics,
    Audio,
    Controls,
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
