//! GUI system — egui-based native desktop UI rendered as overlay on the 3D scene.
//!
//! All GUI code is gated behind `#[cfg(feature = "native")]` since WASM uses
//! the browser-based HTML/JS UI instead.

#[cfg(feature = "native")]
pub mod theme;
#[cfg(feature = "native")]
pub mod widgets;
#[cfg(feature = "native")]
pub mod pages;

/// Which full-screen GUI page is active (if any).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPage {
    /// No overlay page, player is in-game.
    None,
    /// Title/main menu screen.
    MainMenu,
    /// Settings panel (graphics, audio, controls, etc.).
    Settings,
    /// Inventory grid.
    Inventory,
}

/// Persistent GUI state shared across frames.
#[cfg(feature = "native")]
pub struct GuiState {
    /// Currently active full-screen page (None = in-game).
    pub active_page: GuiPage,
    /// Whether the HUD is visible during gameplay.
    pub show_hud: bool,
    /// Whether the chat overlay is visible.
    pub show_chat: bool,
    /// Chat input buffer.
    pub chat_input: String,
    /// Chat message history (timestamp, sender, message).
    pub chat_messages: Vec<(String, String, String)>,
    /// Currently selected inventory slot (if any).
    pub selected_slot: Option<usize>,
    /// Settings state.
    pub settings: SettingsState,
    /// FPS tracking.
    pub frame_count: u32,
    pub fps_timer: f32,
    pub current_fps: u32,
}

/// Tracks all user-configurable settings exposed in the settings page.
#[cfg(feature = "native")]
pub struct SettingsState {
    // Graphics
    pub fullscreen: bool,
    pub vsync: bool,
    pub fov: f32,
    pub render_distance: f32,
    pub resolution_index: usize,
    // Audio
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    // Controls
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    // Active settings category
    pub active_category: SettingsCategory,
}

#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Graphics,
    Audio,
    Controls,
    Game,
    Account,
}

#[cfg(feature = "native")]
impl Default for SettingsState {
    fn default() -> Self {
        Self {
            fullscreen: false,
            vsync: true,
            fov: 90.0,
            render_distance: 500.0,
            resolution_index: 0,
            master_volume: 0.8,
            music_volume: 0.5,
            sfx_volume: 0.7,
            mouse_sensitivity: 1.0,
            invert_y: false,
            active_category: SettingsCategory::Graphics,
        }
    }
}

#[cfg(feature = "native")]
impl Default for GuiState {
    fn default() -> Self {
        Self {
            active_page: GuiPage::MainMenu,
            show_hud: true,
            show_chat: false,
            chat_input: String::new(),
            chat_messages: Vec::new(),
            selected_slot: None,
            settings: SettingsState::default(),
            frame_count: 0,
            fps_timer: 0.0,
            current_fps: 0,
        }
    }
}

#[cfg(feature = "native")]
impl GuiState {
    /// Update FPS counter. Call once per frame with delta time.
    pub fn update_fps(&mut self, dt: f32) {
        self.frame_count += 1;
        self.fps_timer += dt;
        if self.fps_timer >= 1.0 {
            self.current_fps = self.frame_count;
            self.frame_count = 0;
            self.fps_timer -= 1.0;
        }
    }
}
