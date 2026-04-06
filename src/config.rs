//! Persistent configuration for the native desktop app.
//!
//! Saved as `config.json` next to the executable. Loaded on startup,
//! saved when onboarding completes or settings change.
//!
//! Private keys are encrypted with AES-256-GCM using a key derived from
//! a user passphrase via PBKDF2-SHA256. The plaintext `private_key_hex`
//! field is only kept temporarily for migration from older config files.

use serde::{Deserialize, Serialize};

/// Number of PBKDF2 iterations for key derivation.
const PBKDF2_ITERATIONS: u32 = 100_000;

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

    /// Legacy plaintext private key (kept only for migration, removed after encryption).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub private_key_hex: String,

    /// AES-256-GCM encrypted private key: base64(iv_12 + ciphertext + tag_16).
    #[serde(default)]
    pub encrypted_private_key: String,
    /// PBKDF2 salt: base64(random 16 bytes).
    #[serde(default)]
    pub key_salt: String,

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

    // Donation addresses (admin-configurable)
    #[serde(default)]
    pub donate_solana_address: String,
    #[serde(default)]
    pub donate_btc_address: String,
    /// Dynamic donation addresses (new flexible format).
    #[serde(default)]
    pub donate_addresses: Vec<DonateAddressConfig>,
}

/// Serializable donation address entry for config persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DonateAddressConfig {
    pub network: String,
    #[serde(default)]
    pub addr_type: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub label: String,
}

fn default_fov() -> f32 { 90.0 }
fn default_mouse_sensitivity() -> f32 { 3.0 }
fn default_master_volume() -> f32 { 0.8 }
fn default_music_volume() -> f32 { 0.5 }
fn default_sfx_volume() -> f32 { 0.7 }
fn default_true() -> bool { true }
fn default_panel_width() -> f32 { 220.0 }

/// Encrypt a private key with AES-256-GCM using a passphrase.
///
/// Returns `(encrypted_base64, salt_base64)`.
#[cfg(feature = "native")]
pub fn encrypt_private_key(key_bytes: &[u8], passphrase: &str) -> Result<(String, String), String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;

    // Generate random 16-byte salt
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|e| format!("RNG failed: {}", e))?;

    // Derive 32-byte AES key via PBKDF2-SHA256
    let mut derived_key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
        passphrase.as_bytes(),
        &salt,
        PBKDF2_ITERATIONS,
        &mut derived_key,
    );

    // Generate random 12-byte IV
    let mut iv = [0u8; 12];
    getrandom::getrandom(&mut iv).map_err(|e| format!("RNG failed: {}", e))?;

    // AES-256-GCM encrypt
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived_key));
    let nonce = GenericArray::from_slice(&iv);
    let ciphertext = cipher.encrypt(nonce, key_bytes)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Combine: iv (12) + ciphertext_with_tag
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&iv);
    combined.extend_from_slice(&ciphertext);

    Ok((B64.encode(&combined), B64.encode(&salt)))
}

/// Decrypt a private key from its encrypted form.
#[cfg(feature = "native")]
pub fn decrypt_private_key(encrypted: &str, salt: &str, passphrase: &str) -> Result<Vec<u8>, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;

    let combined = B64.decode(encrypted)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;

    if combined.len() < 12 + 16 {
        return Err("Encrypted data too short".to_string());
    }

    let (iv, ciphertext) = combined.split_at(12);

    // Derive key from passphrase + salt
    let salt_bytes = B64.decode(salt)
        .map_err(|e| format!("Salt decode failed: {}", e))?;

    let mut derived_key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
        passphrase.as_bytes(),
        &salt_bytes,
        PBKDF2_ITERATIONS,
        &mut derived_key,
    );

    // AES-256-GCM decrypt
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived_key));
    let nonce = GenericArray::from_slice(iv);
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed: wrong passphrase or corrupted data".to_string())?;

    if plaintext.len() != 32 {
        return Err(format!("Expected 32-byte key, got {}", plaintext.len()));
    }

    Ok(plaintext)
}

/// Convert an Ed25519 public key (hex) to a Solana base58 address.
#[cfg(feature = "native")]
pub fn pubkey_hex_to_solana_address(hex_str: &str) -> Result<String, String> {
    let bytes = (0..hex_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i+2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("Hex decode: {}", e))?;
    if bytes.len() != 32 {
        return Err(format!("Expected 32 bytes, got {}", bytes.len()));
    }
    Ok(bs58::encode(&bytes).into_string())
}

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

    /// Returns true if this config has a legacy plaintext key that needs migration.
    #[cfg(feature = "native")]
    pub fn needs_key_migration(&self) -> bool {
        !self.private_key_hex.is_empty() && self.encrypted_private_key.is_empty()
    }

    /// Returns true if an encrypted key exists and needs passphrase to unlock.
    pub fn needs_passphrase(&self) -> bool {
        !self.encrypted_private_key.is_empty()
    }

    /// Build an AppConfig snapshot from the current GuiState.
    pub fn from_gui_state(state: &crate::gui::GuiState) -> Self {
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
            // Never write plaintext key back; use encrypted fields from state
            private_key_hex: String::new(),
            encrypted_private_key: state.encrypted_private_key.clone(),
            key_salt: state.key_salt.clone(),
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
            donate_solana_address: state.donate_solana_address.clone(),
            donate_btc_address: state.donate_btc_address.clone(),
            donate_addresses: state.donate_addresses.iter().map(|a| DonateAddressConfig {
                network: a.network.clone(),
                addr_type: a.addr_type.clone(),
                value: a.value.clone(),
                label: a.label.clone(),
            }).collect(),
        }
    }

    /// Apply loaded config values into a GuiState.
    #[cfg(feature = "native")]
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
        // Donation addresses
        state.donate_solana_address = self.donate_solana_address.clone();
        state.donate_btc_address = self.donate_btc_address.clone();
        state.donate_addresses = self.donate_addresses.iter().map(|a| crate::gui::DonateAddress {
            network: a.network.clone(),
            addr_type: a.addr_type.clone(),
            value: a.value.clone(),
            label: a.label.clone(),
        }).collect();

        // Store encrypted key fields so they persist through save cycles
        state.encrypted_private_key = self.encrypted_private_key.clone();
        state.key_salt = self.key_salt.clone();

        // Migration: if legacy plaintext key exists, flag for passphrase prompt
        if self.needs_key_migration() {
            // Parse the legacy hex key into bytes for migration
            if let Ok(bytes) = (0..self.private_key_hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&self.private_key_hex[i..i+2], 16))
                .collect::<Result<Vec<u8>, _>>()
            {
                if bytes.len() == 32 {
                    state.private_key_bytes = Some(bytes);
                    state.passphrase_needed = true;
                    state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
                    log::info!("Legacy plaintext key found; passphrase required for migration");
                }
            }
        } else if self.needs_passphrase() {
            // Encrypted key exists; need passphrase to unlock
            state.passphrase_needed = true;
            state.passphrase_mode = crate::gui::PassphraseMode::Unlock;
            log::info!("Encrypted key found; passphrase required to unlock");
        }
    }
}
