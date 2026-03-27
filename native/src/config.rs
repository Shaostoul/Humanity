//! Persistent configuration for the native desktop app.
//!
//! Saved as `config.json` next to the executable. Loaded on startup,
//! saved when onboarding completes or settings change.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub server_url: String,
    pub user_name: String,
    pub public_key_hex: String,
    pub context_real: bool,
    pub completed_onboarding: bool,
    // Settings
    #[serde(default = "default_fov")]
    pub fov: f32,
    #[serde(default = "default_mouse_sensitivity")]
    pub mouse_sensitivity: f32,
    #[serde(default = "default_master_volume")]
    pub master_volume: f32,
    #[serde(default = "default_music_volume")]
    pub music_volume: f32,
    #[serde(default = "default_sfx_volume")]
    pub sfx_volume: f32,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default = "default_true")]
    pub vsync: bool,
    /// Ed25519 private key bytes (hex-encoded for JSON storage).
    #[serde(default)]
    pub private_key_hex: String,

    // Chat panel collapse state
    #[serde(default = "default_true")]
    pub chat_connection_collapsed: bool,
    #[serde(default)]
    pub chat_dm_collapsed: bool,
    #[serde(default)]
    pub chat_groups_collapsed: bool,
    #[serde(default)]
    pub chat_servers_collapsed: bool,
    #[serde(default)]
    pub chat_friends_collapsed: bool,
    #[serde(default)]
    pub chat_members_collapsed: bool,

    // Chat panel resize/lock state
    #[serde(default)]
    pub chat_left_panel_locked: bool,
    #[serde(default)]
    pub chat_right_panel_locked: bool,
    #[serde(default = "default_panel_width")]
    pub chat_left_panel_width: f32,
    #[serde(default = "default_panel_width")]
    pub chat_right_panel_width: f32,
}

fn default_fov() -> f32 { 90.0 }
fn default_mouse_sensitivity() -> f32 { 3.0 }
fn default_master_volume() -> f32 { 0.8 }
fn default_music_volume() -> f32 { 0.5 }
fn default_sfx_volume() -> f32 { 0.7 }
fn default_true() -> bool { true }
fn default_panel_width() -> f32 { 220.0 }

impl AppConfig {
    pub fn config_path() -> std::path::PathBuf {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        exe_dir.join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                log::info!("Loaded config from {}", path.display());
                serde_json::from_str(&json).unwrap_or_default()
            }
            Err(_) => {
                log::info!("No config file found at {}, using defaults", path.display());
                Self::default()
            }
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            match std::fs::write(&path, &json) {
                Ok(_) => log::info!("Saved config to {}", path.display()),
                Err(e) => log::warn!("Failed to save config to {}: {}", path.display(), e),
            }
        }
    }

    /// Build an AppConfig snapshot from the current GuiState.
    pub fn from_gui_state(state: &crate::gui::GuiState) -> Self {
        let private_key_hex = state.private_key_bytes.as_ref()
            .map(|bytes| bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>())
            .unwrap_or_default();
        Self {
            server_url: state.server_url.clone(),
            user_name: state.user_name.clone(),
            public_key_hex: state.profile_public_key.clone(),
            context_real: state.context_real,
            completed_onboarding: state.onboarding_complete,
            fov: state.settings.fov,
            mouse_sensitivity: state.settings.mouse_sensitivity,
            master_volume: state.settings.master_volume,
            music_volume: state.settings.music_volume,
            sfx_volume: state.settings.sfx_volume,
            fullscreen: state.settings.fullscreen,
            vsync: state.settings.vsync,
            private_key_hex,
            chat_connection_collapsed: state.chat_connection_collapsed,
            chat_dm_collapsed: state.chat_dm_collapsed,
            chat_groups_collapsed: state.chat_groups_collapsed,
            chat_servers_collapsed: state.chat_servers_collapsed,
            chat_friends_collapsed: state.chat_friends_collapsed,
            chat_members_collapsed: state.chat_members_collapsed,
            chat_left_panel_locked: state.chat_left_panel_locked,
            chat_right_panel_locked: state.chat_right_panel_locked,
            chat_left_panel_width: state.chat_left_panel_width,
            chat_right_panel_width: state.chat_right_panel_width,
        }
    }

    /// Apply loaded config values into a GuiState.
    pub fn apply_to_gui_state(&self, state: &mut crate::gui::GuiState) {
        state.server_url = self.server_url.clone();
        state.user_name = self.user_name.clone();
        state.profile_public_key = self.public_key_hex.clone();
        state.context_real = self.context_real;
        state.onboarding_complete = self.completed_onboarding;
        state.settings.fov = self.fov;
        state.settings.mouse_sensitivity = self.mouse_sensitivity;
        state.settings.master_volume = self.master_volume;
        state.settings.music_volume = self.music_volume;
        state.settings.sfx_volume = self.sfx_volume;
        state.settings.fullscreen = self.fullscreen;
        state.settings.vsync = self.vsync;
        // Chat panel state
        state.chat_connection_collapsed = self.chat_connection_collapsed;
        state.chat_dm_collapsed = self.chat_dm_collapsed;
        state.chat_groups_collapsed = self.chat_groups_collapsed;
        state.chat_servers_collapsed = self.chat_servers_collapsed;
        state.chat_friends_collapsed = self.chat_friends_collapsed;
        state.chat_members_collapsed = self.chat_members_collapsed;
        state.chat_left_panel_locked = self.chat_left_panel_locked;
        state.chat_right_panel_locked = self.chat_right_panel_locked;
        state.chat_left_panel_width = self.chat_left_panel_width;
        state.chat_right_panel_width = self.chat_right_panel_width;
        // Restore private key bytes from hex
        if !self.private_key_hex.is_empty() {
            if let Ok(bytes) = (0..self.private_key_hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&self.private_key_hex[i..i+2], 16))
                .collect::<Result<Vec<u8>, _>>()
            {
                if bytes.len() == 32 {
                    state.private_key_bytes = Some(bytes);
                }
            }
        }
    }
}
