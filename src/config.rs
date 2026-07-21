//! Persistent configuration for the native desktop app.
//!
//! Saved as `config.json` next to the executable. Loaded on startup,
//! saved when onboarding completes or settings change.
//!
//! Private keys are encrypted with AES-256-GCM using a key derived from
//! a user passphrase via PBKDF2-SHA256. The plaintext `private_key_hex`
//! field is only kept temporarily for migration from older config files.

use serde::{Deserialize, Serialize};

/// Number of PBKDF2 iterations for NEW vaults (v0.277.0+).
///
/// Bumped from 100_000 → 600_000 to match the web client (`web/chat/crypto.js`).
/// The web vault is the easier target (browsers run in shared sandboxes), so
/// the native vault had been weaker than its sibling for no good reason.
///
/// Legacy vaults written before this bump are decrypted with their stored
/// iteration count (see `AppConfig.key_iterations`, defaults to 100_000 for
/// configs from before v0.277.0) and silently re-encrypted to 600_000 on the
/// next successful unlock — the user pays the migration cost exactly once.
pub const PBKDF2_ITERATIONS_NEW: u32 = 600_000;

/// Legacy iteration count for vaults written before v0.277.0. New code MUST
/// NOT call `pbkdf2_hmac` with this constant directly — always use the value
/// stored in `AppConfig.key_iterations` so a future bump only needs to update
/// `PBKDF2_ITERATIONS_NEW`, not chase every call site.
pub const PBKDF2_ITERATIONS_LEGACY: u32 = 100_000;

/// How the desktop window is presented (v0.454). The default is `WindowedFullscreen`: a
/// maximized window that KEEPS the title bar + the OS taskbar (operator's preferred default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WindowMode {
    /// Normal resizable window with a title bar, not maximized.
    Windowed,
    /// Maximized window WITH the title bar + taskbar still showing (the default).
    #[default]
    WindowedFullscreen,
    /// Borderless window (no title bar) at a normal size.
    BorderlessWindowed,
    /// Borderless window covering the whole screen (fake fullscreen; taskbar may peek).
    BorderlessFullscreen,
    /// Exclusive (true) fullscreen at the monitor's video mode.
    ExclusiveFullscreen,
}

impl WindowMode {
    /// Cycle order for a "next mode" button + display labels.
    pub const ALL: [WindowMode; 5] = [
        WindowMode::Windowed,
        WindowMode::WindowedFullscreen,
        WindowMode::BorderlessWindowed,
        WindowMode::BorderlessFullscreen,
        WindowMode::ExclusiveFullscreen,
    ];
    pub fn label(self) -> &'static str {
        match self {
            WindowMode::Windowed => "Windowed",
            WindowMode::WindowedFullscreen => "Windowed fullscreen",
            WindowMode::BorderlessWindowed => "Borderless windowed",
            WindowMode::BorderlessFullscreen => "Borderless fullscreen",
            WindowMode::ExclusiveFullscreen => "Exclusive fullscreen",
        }
    }
}

/// Play mode (task #50, v0.799): the one ladder every cheat/scope gate hangs
/// off. Before this, each dev affordance had its own ad-hoc flag standing
/// alone (`GuiState.creative_mode`, `Theme.cheats_enabled`, nothing at all for
/// construction scope). The mode is now the master; the old flags all remain
/// (forever-dev norm: never delete dev tooling) but the mode SETS or gates
/// them:
///   - `GuiState.creative_mode` (free resources): Normal forces it off every
///     frame (lib.rs bridge); picking Creative/Dev presets it on, and inside
///     those modes the Inventory page's toggle stays a fine-tune (testing
///     real consumption while in Dev is legitimate).
///   - `Theme.cheats_enabled` (the Settings dev-cheats switch): still honored
///     as a kill-switch, but every dev tool ALSO requires PlayMode::Dev now
///     (see `GuiState::dev_cheats_active`).
///   - Construction editor scope: Dev edits the whole ship (all zones, zone
///     add/remove, corridors); Normal/Creative are pinned to the HOME zone.
///
/// Persisted in AppConfig (`#[serde(default)]`); surfaced in Settings >
/// Gameplay as three radio buttons; shown as a HUD tag when not Normal so
/// screenshots are honest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlayMode {
    /// Survival rules: resources consume, building stays within your own
    /// homestead zone, no dev tools, no free materials. The player default
    /// at launch.
    Normal,
    /// Free building + free materials, still scoped to your homestead.
    /// Vitals stay on the Gameplay "Vitals drain" slider (0 pauses needs) --
    /// the mode deliberately does NOT touch that slider. No Dev tools.
    Creative,
    /// Everything: whole-ship structural editing, the Dev page (spawn +
    /// travel/FTL), the G creature editor, all "Dev:" provisioning buttons.
    ///
    /// DEFAULT PRE-LAUNCH: the operator IS the dev and builds the whole
    /// mothership in-game. Flip this default to `Normal` at launch (the
    /// `default_is_dev_pre_launch` test below is the tripwire/reminder).
    #[default]
    Dev,
}

/// The specific powers a play mode can grant. Gates ask the mode for ONE of
/// these (via `PlayMode::allows`) instead of comparing modes directly, so the
/// whole permission surface is enumerable and unit-tested as a truth table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Build/edit within your own homestead zone (the construction editor's
    /// home scope). EVERY mode has this: building your home is the game,
    /// not a cheat.
    HomesteadEditing,
    /// Resource-consuming actions (plant / craft / deploy) skip inventory
    /// requirements + consumption -- the "creative_mode" DataStore slot the
    /// farming/crafting/vehicle systems read.
    FreeResources,
    /// Dev tooling: the Dev page (spawn/despawn, teleport, FTL fly), the G
    /// walk-up creature editor, and the "Dev:" provisioning buttons (stock
    /// materials/seeds, grow all, max skills). Each call site ALSO keeps the
    /// `Theme.cheats_enabled` kill-switch (both must be on).
    DevTools,
    /// Whole-ship structural editing: zone add/remove/relabel/move,
    /// corridors, and selecting non-home zones in the construction editor.
    /// The operator's multi-zone mothership is untouchable without this.
    ShipStructureEditing,
}

/// Pure mode -> capability mapping: the single source of truth every gate
/// reaches through `PlayMode::allows`. A free function (not just a method)
/// so the test below reads as a plain truth table.
pub fn play_mode_allows(mode: PlayMode, capability: Capability) -> bool {
    match capability {
        // Building your own home is gameplay in every mode.
        Capability::HomesteadEditing => true,
        // Free resources = anything past survival (Creative and Dev).
        Capability::FreeResources => mode != PlayMode::Normal,
        // Dev tools + ship superstructure are Dev-only: Creative players get
        // free materials, NOT spawn/teleport or the ability to reshape the
        // shared mothership.
        Capability::DevTools | Capability::ShipStructureEditing => mode == PlayMode::Dev,
    }
}

impl PlayMode {
    /// Display order for the Settings radio group.
    pub const ALL: [PlayMode; 3] = [PlayMode::Normal, PlayMode::Creative, PlayMode::Dev];

    /// See `play_mode_allows` (the tested truth table).
    pub fn allows(self, capability: Capability) -> bool {
        play_mode_allows(self, capability)
    }

    pub fn label(self) -> &'static str {
        match self {
            PlayMode::Normal => "Normal",
            PlayMode::Creative => "Creative",
            PlayMode::Dev => "Dev",
        }
    }

    /// The honest one-line description shown under each Settings radio.
    pub fn hint(self) -> &'static str {
        match self {
            PlayMode::Normal => {
                "Survival rules. Resources are consumed, building stays within \
                 your homestead, no cheats. The player default at launch."
            }
            PlayMode::Creative => {
                "Free building and free materials within your homestead. Pair \
                 with the Vitals drain slider below (0 pauses survival needs) \
                 if you want vitals off. No Dev tools."
            }
            PlayMode::Dev => {
                "Everything: whole-ship structural editing, the Dev spawn and \
                 travel page, and every dev toggle. Pre-launch default while \
                 the mothership is being built in-game."
            }
        }
    }
}

/// How the microphone signal is cleaned up before it is encoded + transmitted
/// (v0.488). The chain is always: user gain, then this filter, then the
/// transmit-mode gate, then Opus. "Off" sends the raw mic; the others remove
/// progressively more non-speech noise (rumble, hiss, keyboard clicks, coughs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VoiceFilterMode {
    /// No processing: the raw mic (after gain) goes straight to Opus.
    Off,
    /// Always-safe light clean-up: an ~85 Hz high-pass (kills rumble / desk
    /// thumps / AC hum fundamentals) plus a gentle noise gate. Very low CPU,
    /// never distorts speech.
    Light,
    /// Full noise suppression (RNNoise-class): removes keyboard clicks, coughs,
    /// fans, and steady background noise while preserving the voice. Slightly
    /// more CPU; the strongest option. The default (operator choice, v0.490).
    #[default]
    NoiseSuppression,
}
impl VoiceFilterMode {
    pub const ALL: [VoiceFilterMode; 3] =
        [VoiceFilterMode::Off, VoiceFilterMode::Light, VoiceFilterMode::NoiseSuppression];
    pub fn label(self) -> &'static str {
        match self {
            VoiceFilterMode::Off => "No filter",
            VoiceFilterMode::Light => "Light clean-up",
            VoiceFilterMode::NoiseSuppression => "Noise suppression",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            VoiceFilterMode::Off => "Sends your raw mic. Best with a studio mic in a quiet room.",
            VoiceFilterMode::Light => "Removes rumble and hiss with a high-pass and a soft gate. Safe default.",
            VoiceFilterMode::NoiseSuppression => "Removes keyboards, coughs, fans, and background noise.",
        }
    }
}

/// When the microphone is actually transmitted (v0.488). The filter runs first;
/// this decides whether the cleaned audio is sent at all this moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VoiceTransmitMode {
    /// Always transmit (classic open mic).
    OpenMic,
    /// Transmit only while the push-to-talk key is held. The default (operator
    /// choice, v0.490), with CapsLock as the default push key.
    #[default]
    PushToTalk,
    /// Transmit only when your voice rises above the activation threshold.
    VoiceActivated,
    /// Transmit always, EXCEPT while the push-to-mute key is held.
    PushToMute,
}
impl VoiceTransmitMode {
    pub const ALL: [VoiceTransmitMode; 4] = [
        VoiceTransmitMode::OpenMic,
        VoiceTransmitMode::PushToTalk,
        VoiceTransmitMode::VoiceActivated,
        VoiceTransmitMode::PushToMute,
    ];
    pub fn label(self) -> &'static str {
        match self {
            VoiceTransmitMode::OpenMic => "Open mic",
            VoiceTransmitMode::PushToTalk => "Push to talk",
            VoiceTransmitMode::VoiceActivated => "Voice activated",
            VoiceTransmitMode::PushToMute => "Push to mute",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            VoiceTransmitMode::OpenMic => "Your mic is always live.",
            VoiceTransmitMode::PushToTalk => "Hold the key to talk; silent otherwise.",
            VoiceTransmitMode::VoiceActivated => "Transmits when you speak above the threshold.",
            VoiceTransmitMode::PushToMute => "Always live; hold the key to mute.",
        }
    }
    /// True if this mode needs the push key bound (PTT / push-to-mute).
    pub fn uses_key(self) -> bool {
        matches!(self, VoiceTransmitMode::PushToTalk | VoiceTransmitMode::PushToMute)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub user_name: String,
    #[serde(default)]
    pub public_key_hex: String,
    /// LEGACY: kept for backwards-compatible deserialization of pre-v0.197
    /// configs that still have the field. The Real/Sim toggle was removed
    /// in v0.197.0; this field is read but ignored. New configs serialize
    /// it for compatibility with old binaries on the same machine.
    #[serde(default)]
    pub context_real: bool,
    #[serde(default)]
    pub completed_onboarding: bool,
    /// v0.198.0: whether the user has seen the post-identity Onboarding
    /// concept tour. Defaults to `true` via serde so pre-v0.198 configs
    /// (where the user has been using the app for a while) don't get
    /// force-routed back through the tour. Fresh installs set this to
    /// false in GuiState, see Default impl, so they DO see the tour.
    #[serde(default = "default_true")]
    pub concept_tour_seen: bool,
    // Settings
    #[serde(default = "default_fov")]
    pub fov: f32,
    #[serde(default = "default_mouse_sensitivity")]
    pub mouse_sensitivity: f32,
    /// Invert vertical mouse look (v0.909 - persisted now that it's wired).
    #[serde(default)]
    pub invert_y: bool,
    #[serde(default = "default_master_volume")]
    pub master_volume: f32,
    #[serde(default = "default_music_volume")]
    pub music_volume: f32,
    #[serde(default = "default_sfx_volume")]
    pub sfx_volume: f32,
    #[serde(default)]
    pub fullscreen: bool,
    /// How the window is presented (v0.454). Supersedes the legacy `fullscreen` bool.
    /// Absent in old configs => WindowedFullscreen (the operator's preferred default).
    #[serde(default)]
    pub window_mode: WindowMode,
    #[serde(default = "default_true")]
    pub vsync: bool,
    /// Procedural sky-planet surfaces master toggle (v0.763). Off = the old
    /// flat-colored spheres.
    #[serde(default = "default_true")]
    pub planet_detail: bool,
    /// Sky orbit-ring visibility (v0.786): "off" | "planets" | "planets_moons".
    #[serde(default = "default_sky_orbit_mode")]
    pub sky_orbit_mode: String,
    /// Show constellation figures in the FPS sky (v0.786).
    #[serde(default = "default_true")]
    pub sky_constellations: bool,
    /// Show the Milky Way glow layer (2026-07-10): the baked all-sky texture
    /// of real integrated catalog starlight behind the star points.
    #[serde(default = "default_true")]
    pub sky_milkyway_glow: bool,
    /// Milky Way glow intensity multiplier (0..2, default 1.0).
    #[serde(default = "default_sky_milkyway_intensity")]
    pub sky_milkyway_intensity: f32,
    /// Milky Way glow texture tier (2026-07-11): "standard" (the 8192x4096
    /// galaxy_glow.png that ships with the app) or "ultra" (the downloadable
    /// 16384x8192 galaxy_glow_ultra.png, ~99 MB on disk / ~512 MB VRAM).
    /// Unknown values sanitize to standard at apply time; the loader also
    /// falls back to standard when the ultra file is missing/corrupt or
    /// exceeds this GPU's max texture dimension. Applied at world entry
    /// (the glow layer is built with the star renderer).
    #[serde(default = "default_sky_glow_tier")]
    pub sky_glow_tier: String,
    /// Star halos (2026-07-11): soft photographic glow + faint diffraction
    /// cross on the brightest stars, drawn additively over the star points.
    #[serde(default = "default_true")]
    pub sky_star_halos: bool,
    /// Star-catalog load CEILING (2026-07-12 dev tooling). "auto" (default,
    /// biggest installed wins -- players unchanged), "standard"/"minimal"
    /// (force the shipped 120k stars.bin for a fast boot), "extended" (cap at
    /// 2.5M), "ultra" (cap at 25M). Applied at world entry. The
    /// HUMANITY_STAR_TIER env var overrides this WITHOUT persisting -- the
    /// scripted/dev fast path (see stars::StarCatalogTier::resolve_cap).
    #[serde(default = "default_star_catalog_tier")]
    pub star_catalog_tier: String,
    /// Screen-size LOD base threshold in pixels for sky planets: one more
    /// icosphere subdivision each time the projected diameter doubles past
    /// this. See `terrain::planet::lod_level_for_pixels`.
    #[serde(default = "default_planet_lod_px")]
    pub planet_lod_px: f32,
    /// Max icosphere subdivision level for sky planets (0-9; clamped to
    /// `terrain::planet::MAX_SKY_SUBDIVISION` on load). Levels 8-9 are the
    /// planet-fills-the-screen FTL-approach tiers: heavy meshes (~142 MB /
    /// ~566 MB GPU) that the pixel-doubling LOD ladder only ever requests
    /// for one screen-dominating body at a time.
    #[serde(default = "default_planet_max_subdiv")]
    pub planet_max_subdiv: f32,
    /// Chunked-LOD split threshold in screen pixels (v0.873 Planet LOD
    /// settings): patches subdivide until their triangles subtend about this
    /// many pixels. LOWER = sharper terrain further away, more patches.
    #[serde(default = "default_terrain_split_px")]
    pub terrain_split_px: f32,
    /// Max surface patches drawn per planet per frame. Higher = more of the
    /// horizon holds full detail; bounded well under the renderer's shared
    /// object budget (MAX_OBJECTS 1024, shared with machines/walls/sky).
    #[serde(default = "default_terrain_patch_budget")]
    pub terrain_patch_budget: f32,
    /// Detail draw distance factor (v0.905): how far shader detail octaves
    /// survive (1.0 = classic, 3.0 = detail visible at 3x the range).
    #[serde(default = "default_terrain_detail_distance")]
    pub terrain_detail_distance: f32,
    /// Near-field real tree model distance in metres (v0.911; 0 = cards
    /// only, higher = photoscanned conifers further out, more GPU).
    #[serde(default = "default_tree_model_distance")]
    pub tree_model_distance: f32,
    /// Sun shadow map (v0.907): terrain/vegetation/structure shadows cast
    /// by the sun. Off = the pre-v0.899 unshadowed look (cheaper).
    #[serde(default = "default_true")]
    pub sun_shadows: bool,
    /// God-ray shaft intensity (v0.907 slider; 0 disables the pass).
    #[serde(default = "default_godray_intensity")]
    pub godray_intensity: f32,
    /// Aerial perspective strength (v0.916; 0 = off, 1 = earthlike haze).
    #[serde(default = "default_aerial_strength")]
    pub aerial_strength: f32,
    /// Ambient-occlusion strength (v0.907 slider; 0 disables the pass).
    #[serde(default = "default_ssao_strength")]
    pub ssao_strength: f32,
    /// Patch mesh builds per frame: how fast terrain streams in during a
    /// descent. Higher = faster refinement, a few ms more per frame while
    /// streaming.
    #[serde(default = "default_terrain_builds_per_frame")]
    pub terrain_builds_per_frame: f32,
    /// Chunked planetary LOD (2026-07-11): stream camera-following surface
    /// patches when a heightmap planet fills the screen, instead of the
    /// heavy uniform level 8-9 spheres. See terrain::planet_chunks.
    #[serde(default = "default_true")]
    pub planet_chunked: bool,
    /// Analytic scattering atmosphere shells (v0.807). Off = the simple
    /// fresnel-tinted shell (the v0.763 look), kept as the fallback for
    /// GPUs that dislike the per-pixel scattering math.
    #[serde(default = "default_true")]
    pub planet_atmo_scatter: bool,
    /// Animated procedural cloud shells on planets that declare
    /// cloud_coverage (clouds increment 1). Off = no cloud deck at all.
    #[serde(default = "default_true")]
    pub planet_clouds: bool,
    /// Live Earth weather (v0.874): fetch NASA's real global cloud-cover
    /// map in the background and place the in-game cloud masses where real
    /// clouds are right now. Off = purely procedural coverage, no network.
    #[serde(default = "default_true")]
    pub live_weather: bool,
    /// Track the orbital home station (in-world ring + label; Cosmos page
    /// toggle, v0.885/persisted v0.886).
    #[serde(default = "default_true")]
    pub track_station: bool,
    /// Planet close-range surface detail (v0.816): animated ocean waves +
    /// land micro-texture on imagery planets (Earth). Orbit view identical
    /// either way (anti-alias faded); applies live via the per-frame
    /// material flag. serde-defaulted true so old configs gain the effect.
    #[serde(default = "default_true")]
    pub planet_surface_detail: bool,
    /// Cloud quality (clouds increment 3): "low" = the increment-1 painted
    /// deck, "medium" = the increment-2 10-sample field march, "high" = the
    /// volumetric 3D-noise system (default). Unknown values fall to high at
    /// use time (renderer::clouds::quality_param).
    #[serde(default = "default_cloud_quality")]
    pub cloud_quality: String,
    /// Which home design `data/machines/*.ron` file loads (2026-07-01): `"home"` (default,
    /// the existing family-scale design in `home.ron`) or `"home_solo"` (a one-person
    /// self-sufficient design in `home_solo.ron`, sized to real one-person kWh/L/kcal
    /// needs -- see `docs/design/homestead-solo-design.md`). Data-driven per the project's
    /// GUI-first-configurability rule: exposed as a Settings toggle, not a config-file-only
    /// switch. `#[serde(default)]` (not `default_true`-style) so a bad/unknown value falls
    /// back to `"home"` at load time (see `MachineHome::load`'s caller), never panics.
    #[serde(default = "default_home_variant")]
    pub home_variant: String,
    /// Spawn hostile wild creatures (wild_spawns.ron predator rows). Default
    /// OFF pre-launch (v0.791, operator: "disable the wolves"); the Dev spawn
    /// page still places hostiles deliberately.
    #[serde(default)]
    pub hostile_wildlife: bool,
    /// Survival-needs speed: scales hunger/thirst/energy decay (1.0 = normal,
    /// 0 = paused). Settings > Gameplay slider (v0.791).
    #[serde(default = "default_vitals_drain")]
    pub vitals_drain: f32,
    /// Play mode (task #50): Normal | Creative | Dev -- the ladder every
    /// cheat/scope gate hangs off (see the `PlayMode` docs above). Absent in
    /// old configs => Dev via `#[serde(default)]` (the pre-launch default;
    /// flips to Normal at launch). Applied live: the gates read it per frame.
    #[serde(default)]
    pub play_mode: PlayMode,

    // ── v0.488: native voice input prefs ────────────────────────────────
    // The mic device + speaker device the user picked (empty => system
    // default), the input gain, the noise filter, the transmit mode, the
    // push key, and the voice-activation threshold. All persisted so the
    // user does not re-pick every launch. Defaults are safe for a fresh
    // install: system devices, unity gain, light clean-up, open mic.
    #[serde(default)]
    pub voice_input_device: String,
    #[serde(default)]
    pub voice_output_device: String,
    #[serde(default = "default_voice_gain")]
    pub voice_gain: f32,
    #[serde(default)]
    pub voice_filter_mode: VoiceFilterMode,
    #[serde(default)]
    pub voice_transmit_mode: VoiceTransmitMode,
    /// The push-to-talk / push-to-mute key, as an egui Key name (e.g. "V").
    /// Empty => unbound (the UI prompts the user to set one).
    #[serde(default = "default_voice_ptt_key")]
    pub voice_ptt_key: String,
    /// Voice-activation RMS threshold (0.0..=1.0) for the voice-activated mode.
    #[serde(default = "default_voice_vad_threshold")]
    pub voice_vad_threshold: f32,

    /// Chat timestamp display format (operator-configurable). One of
    /// TimestampFormat::as_str() — "hour_min" (default) … "full". Empty/unknown
    /// → hour_min. Applied app-wide via chat::set_timestamp_format on load.
    #[serde(default)]
    pub timestamp_format: String,

    /// Legacy plaintext private key (kept only for migration, removed after encryption).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub private_key_hex: String,

    /// AES-256-GCM encrypted Ed25519 private key: base64(iv_12 + ciphertext + tag_16).
    #[serde(default)]
    pub encrypted_private_key: String,
    /// PBKDF2 salt: base64(random 16 bytes).
    #[serde(default)]
    pub key_salt: String,
    /// PBKDF2 iteration count this vault was encrypted with. Defaults to
    /// `PBKDF2_ITERATIONS_LEGACY` (100_000) for pre-v0.277.0 configs that
    /// don't have the field. New encryptions write `PBKDF2_ITERATIONS_NEW`
    /// (600_000). The unlock path re-encrypts silently when it sees a value
    /// below `PBKDF2_ITERATIONS_NEW`, so the migration is one-time and
    /// transparent.
    #[serde(default = "default_legacy_iterations")]
    pub key_iterations: u32,

    // ── v0.278.0: auto-unlock ───────────────────────────────────────────
    // Three opt-in unlock modes coexist alongside the passphrase vault:
    //   AlwaysPrompt  — original behavior (default for pre-v0.278 configs)
    //   Keychain      — OS keychain stores raw seed; silent startup
    //   KeychainPin   — keychain stores device key; PIN-encrypted seed
    //                   blob lives in `pin_encrypted_seed` / `pin_salt`
    //
    // The passphrase-encrypted vault (`encrypted_private_key` + `key_salt`)
    // is ALWAYS still present as the recovery path — auto-unlock modes
    // add an alternate unlock; they never replace the passphrase. Losing
    // the keychain entry falls back to the passphrase prompt without data
    // loss.

    /// How the seed should be unlocked at app launch. See `auto_unlock.rs`.
    /// `serde(default)` → `AlwaysPrompt` for pre-v0.278 configs.
    #[cfg(feature = "native")]
    #[serde(default)]
    pub auto_unlock_mode: crate::auto_unlock::AutoUnlockMode,

    /// PIN-encrypted seed blob (KeychainPin mode only). Encrypted with
    /// AES-256-GCM using a key derived from `PIN ‖ device_key` via
    /// PBKDF2-SHA256 (`PIN_PBKDF2_ITERS`). The `device_key` lives in the
    /// OS keychain — this blob without it is useless. Empty when not set.
    #[serde(default)]
    pub pin_encrypted_seed: String,
    /// PBKDF2 salt for the PIN-encrypted seed (base64). Empty when unset.
    #[serde(default)]
    pub pin_salt: String,

    // Full-PQ: no DM keypair is persisted. Dilithium3 (identity) and
    // Kyber768 (DM) both re-derive deterministically from the BIP39 seed
    // (`encrypted_private_key`). The legacy plaintext ECDH fields were
    // removed; old configs that still carry them are ignored by serde.

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
    pub chat_connected_server_collapsed: bool,
    #[serde(default)]
    pub chat_friends_collapsed: bool,
    #[serde(default)]
    pub chat_members_collapsed: bool,
    /// Collapse state of the Studio quick-access section (chat right rail,
    /// above Friends). Defaults expanded so livestream access survives the
    /// main-menu consolidation that folds away the top-nav Studio button.
    #[serde(default)]
    pub chat_studio_collapsed: bool,

    // In-world chat panel (unified-chat increment 1c): the passive
    // bottom-left feed toggle + the interactive panel's message-list height.
    // Both are set from the panel's Options tab (GUI-first configurability).
    #[serde(default = "default_true")]
    pub hud_chat_feed_visible: bool,
    #[serde(default = "default_ingame_chat_panel_height")]
    pub ingame_chat_panel_height: f32,

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

    /// Two-tier nav layout (v0.166.0, default flipped to true in v0.174.0).
    /// When true, the nav bar uses the Reality / Sim / Tools / Settings
    /// layout with sub-pages. When false, falls back to the legacy
    /// single-row nav. Fresh installs default to true; existing users with
    /// the field already saved keep their previous choice.
    #[serde(default = "default_true")]
    pub nav_two_tier: bool,
    /// Active top-tier category when nav_two_tier is on.
    #[serde(default = "default_nav_top_category")]
    pub nav_top_category: String,
    /// Which page to show after startup (v0.220.0). Stored as a string
    /// key ("chat", "onboarding", "tasks", etc.) so the config file
    /// stays readable. Defaults to "onboarding" for new installs;
    /// existing configs without this field also get "onboarding".
    #[serde(default = "default_boot_page")]
    pub default_page: String,
    /// Default character/home the launcher loads on Play (v0.474). A local save
    /// stem ("" = no default, always show the launcher's character picker).
    /// When non-empty, Play skips the launcher and enters the world with this
    /// character. See gui/pages/launcher.rs.
    #[serde(default)]
    pub default_character: String,
}

fn default_nav_top_category() -> String { "reality".to_string() }
fn default_boot_page() -> String { "onboarding".to_string() }

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

fn default_legacy_iterations() -> u32 { PBKDF2_ITERATIONS_LEGACY }
fn default_fov() -> f32 { 90.0 }
// 0.25 with the camera's `dx * sensitivity * 0.01` formula is a calm, precise default;
// the old 3.0 was ~12x too fast (it made every new player's view whip around).
fn default_mouse_sensitivity() -> f32 { 0.25 }
fn default_master_volume() -> f32 { 0.8 }
fn default_music_volume() -> f32 { 0.5 }
fn default_sfx_volume() -> f32 { 0.7 }
fn default_sky_orbit_mode() -> String { "planets".to_string() }
fn default_sky_milkyway_intensity() -> f32 { 1.0 }
fn default_sky_glow_tier() -> String { "standard".to_string() }
fn default_star_catalog_tier() -> String { "auto".to_string() }
fn default_true() -> bool { true }
fn default_home_variant() -> String { "home".to_string() }
fn default_cloud_quality() -> String { "high".to_string() }
fn default_vitals_drain() -> f32 { 1.0 }
fn default_planet_lod_px() -> f32 { 10.0 }
fn default_planet_max_subdiv() -> f32 { 6.0 }
fn default_terrain_split_px() -> f32 { 4.0 }
fn default_terrain_patch_budget() -> f32 { 3072.0 }
fn default_terrain_detail_distance() -> f32 { 1.5 }
fn default_godray_intensity() -> f32 { 0.55 }
fn default_tree_model_distance() -> f32 { 0.0 }
fn default_aerial_strength() -> f32 { 1.0 }
fn default_ssao_strength() -> f32 { 0.55 }
fn default_terrain_builds_per_frame() -> f32 { 64.0 }
fn default_panel_width() -> f32 { 220.0 }
// In-world chat panel message-list height (unified-chat increment 1c).
// Matches the v0.772 hardcoded ScrollArea max_height so existing installs
// see no visual change until they move the Options-tab slider.
fn default_ingame_chat_panel_height() -> f32 { 160.0 }
// v0.488 voice input prefs: unity gain, "V" push key, a modest activation floor.
/// Prettify a stored winit-KeyCode name for display, e.g. "KeyV" -> "V",
/// "Digit1" -> "1", "CapsLock" -> "CapsLock", "Space" -> "Space".
pub fn pretty_ptt_key_name(name: &str) -> String {
    if let Some(c) = name.strip_prefix("Key") {
        return c.to_string();
    }
    if let Some(d) = name.strip_prefix("Digit") {
        return d.to_string();
    }
    name.to_string()
}

fn default_voice_gain() -> f32 { 1.0 }
// CapsLock as the default push-to-talk key (operator choice). Stored as the
// winit KeyCode debug name so the raw-input PTT reader can match it directly.
fn default_voice_ptt_key() -> String { "CapsLock".to_string() }
fn default_voice_vad_threshold() -> f32 { 0.05 }

/// Encrypt a private key with AES-256-GCM using a passphrase.
///
/// Always uses `PBKDF2_ITERATIONS_NEW` (600_000) — the bumped iteration
/// count rolled out in v0.277.0 to match the web client. Returns the
/// iteration count alongside the ciphertext+salt so the caller can stash
/// it in `AppConfig.key_iterations` (we don't infer it from the file:
/// makes vault metadata self-describing for any future bump).
///
/// Returns `(encrypted_base64, salt_base64, iterations)`.
#[cfg(feature = "native")]
pub fn encrypt_private_key(key_bytes: &[u8], passphrase: &str) -> Result<(String, String, u32), String> {
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
        PBKDF2_ITERATIONS_NEW,
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

    Ok((B64.encode(&combined), B64.encode(&salt), PBKDF2_ITERATIONS_NEW))
}

/// Decrypt a private key from its encrypted form using the iteration count
/// stored in the vault. Caller passes `AppConfig.key_iterations` directly so
/// a future bump only needs to change `PBKDF2_ITERATIONS_NEW`, not chase
/// hardcoded counts at every call site.
///
/// A wrong passphrase, a corrupted blob, OR a mismatched iteration count all
/// surface as the same "wrong passphrase" message — AES-GCM auth failure is
/// the only signal the decrypt path returns, and it's deliberately ambiguous
/// to avoid leaking which input was bad.
#[cfg(feature = "native")]
pub fn decrypt_private_key(encrypted: &str, salt: &str, passphrase: &str, iterations: u32) -> Result<Vec<u8>, String> {
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

    // Defensive: a corrupt config with iterations == 0 would silently
    // produce a deterministic-but-useless key. Treat anything below the
    // legacy floor as the legacy floor — keeps the migration path open
    // for vaults written by absurdly-old or fuzzed binaries.
    let iters = iterations.max(PBKDF2_ITERATIONS_LEGACY);

    let mut derived_key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
        passphrase.as_bytes(),
        &salt_bytes,
        iters,
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
        // Explicit override: HUMANITY_DATA_DIR=<dir> puts config.json (which
        // holds the encrypted identity) in that dir instead of the OS default.
        // Lets you run a SECOND native instance with its own identity on the
        // same machine — e.g. for local native↔native P2P testing:
        //   HUMANITY_DATA_DIR=%TEMP%\hum2 v0.317.x_HumanityOS.exe
        // (also handy for portable / multi-profile setups).
        if let Ok(dir) = std::env::var("HUMANITY_DATA_DIR") {
            let dir = dir.trim();
            if !dir.is_empty() {
                let dir = std::path::PathBuf::from(dir);
                let _ = std::fs::create_dir_all(&dir);
                return dir.join("config.json");
            }
        }
        // Portable mode (v0.707): the whole install, INCLUDING the encrypted
        // identity in config.json, lives beside the exe so an external-drive
        // folder travels between machines as one unit.
        if let Some(p) = crate::storage::portable_config_path() {
            return p;
        }
        // Use %APPDATA%/HumanityOS/config.json for a stable location
        // that doesn't change when the exe moves between versioned binaries.
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let dir = std::path::PathBuf::from(appdata).join("HumanityOS");
                let _ = std::fs::create_dir_all(&dir);
                return dir.join("config.json");
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(home) = std::env::var("HOME") {
                let dir = std::path::PathBuf::from(home).join(".config").join("HumanityOS");
                let _ = std::fs::create_dir_all(&dir);
                return dir.join("config.json");
            }
        }
        // Fallback: next to exe
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
}

impl Default for AppConfig {
    fn default() -> Self {
        // The real field defaults live in the serde attributes (default_true,
        // default_fov, ...), and those only run during DESERIALIZATION. The
        // old `#[derive(Default)]` ignored them, so a fresh install (no
        // config.json) booted with every bool false and every float zero:
        // planet_detail off, vsync off, fov 0, silent audio. Parsing an empty
        // object routes "fresh install" through the exact same defaults as
        // "field missing from an old config.json".
        let mut c: Self = serde_json::from_str("{}")
            .expect("AppConfig: every field must carry a serde default (see Default impl)");
        // One deliberate divergence: the serde default for concept_tour_seen
        // is TRUE so long-time users whose config predates the field are not
        // force-routed back through the tour. A genuinely fresh install
        // SHOULD see the tour once, so the no-config path flips it back.
        c.concept_tour_seen = false;
        c
    }
}

impl AppConfig {
    /// Like `load()`, but returns None when no config file exists yet, so a
    /// FRESH install keeps GuiState's designed defaults rather than having
    /// a config applied over them at all. Two layers of the same 2026-07-11
    /// fresh-install fix (found independently by two agents' boot-verifies):
    /// this guard skips the apply entirely on first boot, and the manual
    /// `Default` impl below makes every other default-constructed AppConfig
    /// (corrupt-file `unwrap_or_default`, tests) carry the real serde
    /// defaults instead of derive's all-false/zero fields. Existing configs
    /// behave exactly as before (missing fields get their serde defaults).
    pub fn load_if_exists() -> Option<Self> {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                log::info!("Loaded config from {}", path.display());
                Some(serde_json::from_str(&json).unwrap_or_default())
            }
            Err(_) => {
                log::info!(
                    "No config file at {} (fresh install): keeping built-in defaults",
                    path.display()
                );
                None
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
            // v0.197.0: context_real removed from GuiState. Always
            // serialize true so old binaries on the same machine still
            // see "real mode" if they read the config.
            context_real: true,
            completed_onboarding: state.onboarding_complete,
            concept_tour_seen: state.concept_tour_seen,
            fov: state.settings.fov,
            mouse_sensitivity: state.settings.mouse_sensitivity,
            invert_y: state.settings.invert_y,
            master_volume: state.settings.master_volume,
            music_volume: state.settings.music_volume,
            sfx_volume: state.settings.sfx_volume,
            fullscreen: state.settings.fullscreen,
            window_mode: state.settings.window_mode,
            vsync: state.settings.vsync,
            planet_detail: state.settings.planet_detail,
            sky_orbit_mode: state.settings.sky_orbit_mode.clone(),
            sky_constellations: state.settings.sky_constellations,
            sky_milkyway_glow: state.settings.sky_milkyway_glow,
            sky_milkyway_intensity: state.settings.sky_milkyway_intensity,
            sky_glow_tier: state.settings.sky_glow_tier.clone(),
            sky_star_halos: state.settings.sky_star_halos,
            star_catalog_tier: state.settings.star_catalog_tier.clone(),
            planet_lod_px: state.settings.planet_lod_px,
            terrain_split_px: state.settings.terrain_split_px,
            terrain_patch_budget: state.settings.terrain_patch_budget,
            terrain_detail_distance: state.settings.terrain_detail_distance,
            tree_model_distance: state.settings.tree_model_distance,
            sun_shadows: state.settings.sun_shadows,
            godray_intensity: state.settings.godray_intensity,
            aerial_strength: state.settings.aerial_strength,
            ssao_strength: state.settings.ssao_strength,
            terrain_builds_per_frame: state.settings.terrain_builds_per_frame,
            planet_max_subdiv: state.settings.planet_max_subdiv,
            planet_chunked: state.settings.planet_chunked,
            planet_atmo_scatter: state.settings.planet_atmo_scatter,
            planet_clouds: state.settings.planet_clouds,
            live_weather: state.settings.live_weather,
            track_station: state.settings.track_station,
            planet_surface_detail: state.settings.planet_surface_detail,
            cloud_quality: state.settings.cloud_quality.clone(),
            home_variant: state.settings.home_variant.clone(),
            hostile_wildlife: state.settings.hostile_wildlife,
            vitals_drain: state.settings.vitals_drain,
            play_mode: state.settings.play_mode,
            // v0.488 voice input prefs (top-level GuiState, not SettingsState).
            voice_input_device: state.audio_input_device.clone(),
            voice_output_device: state.audio_output_device.clone(),
            voice_gain: state.voice_gain,
            voice_filter_mode: state.voice_filter_mode,
            voice_transmit_mode: state.voice_transmit_mode,
            voice_ptt_key: state.voice_ptt_key.clone(),
            voice_vad_threshold: state.voice_vad_threshold,
            timestamp_format: crate::gui::pages::chat::timestamp_format().as_str().to_string(),
            // Never write plaintext key back; use encrypted fields from state
            private_key_hex: String::new(),
            encrypted_private_key: state.encrypted_private_key.clone(),
            key_salt: state.key_salt.clone(),
            key_iterations: state.key_iterations,
            // v0.278.0 auto-unlock
            #[cfg(feature = "native")]
            auto_unlock_mode: state.auto_unlock_mode,
            pin_encrypted_seed: state.pin_encrypted_seed.clone(),
            pin_salt: state.pin_salt.clone(),
            chat_connection_collapsed: state.chat_connection_collapsed,
            chat_dm_collapsed: state.chat_dm_collapsed,
            chat_groups_collapsed: state.chat_groups_collapsed,
            chat_servers_collapsed: state.chat_servers_collapsed,
            chat_connected_server_collapsed: state.chat_connected_server_collapsed,
            chat_friends_collapsed: state.chat_friends_collapsed,
            chat_members_collapsed: state.chat_members_collapsed,
            chat_studio_collapsed: state.chat_studio_collapsed,
            hud_chat_feed_visible: state.hud_chat_feed_visible,
            ingame_chat_panel_height: state.ingame_chat_panel_height,
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
            nav_two_tier: state.nav_two_tier,
            nav_top_category: state.nav_top_category.clone(),
            default_page: crate::gui::page_to_config_str(state.default_page).to_string(),
            default_character: state.launcher_default_character.clone(),
        }
    }

    /// Apply loaded config values into a GuiState.
    #[cfg(feature = "native")]
    pub fn apply_to_gui_state(&self, state: &mut crate::gui::GuiState) {
        // Only overwrite if config has non-empty values (preserve defaults)
        if !self.server_url.is_empty() {
            state.server_url = self.server_url.clone();
        }
        if !self.user_name.is_empty() {
            state.user_name = self.user_name.clone();
        }
        if !self.public_key_hex.is_empty() {
            state.profile_public_key = self.public_key_hex.clone();
        }
        // v0.197.0: context_real removed from GuiState — apply step skipped.
        state.onboarding_complete = self.completed_onboarding;
        state.concept_tour_seen = self.concept_tour_seen;
        state.settings.fov = self.fov;
        // Guard against a non-positive saved value (a 0.0 would freeze the camera look).
        state.settings.mouse_sensitivity = if self.mouse_sensitivity > 0.0 {
            self.mouse_sensitivity
        } else {
            default_mouse_sensitivity()
        };
        state.settings.invert_y = self.invert_y;
        state.settings.master_volume = self.master_volume;
        state.settings.music_volume = self.music_volume;
        state.settings.sfx_volume = self.sfx_volume;
        state.settings.fullscreen = self.fullscreen;
        state.settings.window_mode = self.window_mode;
        state.settings.vsync = self.vsync;
        state.settings.planet_detail = self.planet_detail;
        state.settings.sky_orbit_mode = self.sky_orbit_mode.clone();
        state.settings.sky_constellations = self.sky_constellations;
        state.settings.sky_milkyway_glow = self.sky_milkyway_glow;
        // Clamp a hand-edited/corrupt saved value to the slider's range (a
        // huge multiplier would white out the whole sky).
        state.settings.sky_milkyway_intensity = self.sky_milkyway_intensity.clamp(0.0, 2.0);
        // Sanitize the glow tier: anything but the known ultra value (a
        // hand-edited config, a future removed tier) means standard.
        state.settings.sky_glow_tier = if self.sky_glow_tier == "ultra" {
            "ultra".to_string()
        } else {
            "standard".to_string()
        };
        state.settings.sky_star_halos = self.sky_star_halos;
        // Sanitize the star-catalog tier ceiling: known values pass through,
        // anything else (hand-edited/empty/old config) means "auto" so a player
        // who downloaded a bigger catalog is never silently downgraded.
        state.settings.star_catalog_tier = match self.star_catalog_tier.trim().to_ascii_lowercase().as_str() {
            "standard" | "minimal" => "standard".to_string(),
            "extended" => "extended".to_string(),
            "ultra" => "ultra".to_string(),
            _ => "auto".to_string(),
        };
        // Guard a corrupted saved value (a 0/negative threshold would pin
        // every body at max subdivision).
        state.settings.planet_lod_px = if self.planet_lod_px >= 1.0 {
            self.planet_lod_px
        } else {
            default_planet_lod_px()
        };
        state.settings.planet_max_subdiv = self
            .planet_max_subdiv
            .clamp(0.0, crate::terrain::planet::MAX_SKY_SUBDIVISION as f32);
        // Planet LOD knobs (v0.887 fix, operator: "my settings changes for
        // some of the graphics aren't actually saving"): these three SAVED
        // since v0.873 but were never applied at load, so every boot reset
        // them to defaults. Clamps mirror the Settings sliders.
        state.settings.terrain_split_px = self.terrain_split_px.clamp(2.0, 24.0);
        state.settings.terrain_patch_budget = self.terrain_patch_budget.clamp(256.0, 12288.0);
        state.settings.terrain_detail_distance =
            self.terrain_detail_distance.clamp(0.5, 3.0);
        state.settings.tree_model_distance = self.tree_model_distance.clamp(0.0, 400.0);
        state.settings.sun_shadows = self.sun_shadows;
        state.settings.godray_intensity = self.godray_intensity.clamp(0.0, 1.5);
        state.settings.aerial_strength = self.aerial_strength.clamp(0.0, 2.0);
        state.settings.ssao_strength = self.ssao_strength.clamp(0.0, 1.5);
        state.settings.terrain_builds_per_frame =
            self.terrain_builds_per_frame.clamp(6.0, 64.0);
        state.settings.planet_chunked = self.planet_chunked;
        state.settings.planet_atmo_scatter = self.planet_atmo_scatter;
        state.settings.planet_clouds = self.planet_clouds;
        state.settings.live_weather = self.live_weather;
        state.settings.track_station = self.track_station;
        state.settings.planet_surface_detail = self.planet_surface_detail;
        // Guard a corrupted saved value: only the three known tiers pass
        // through; anything else falls back to the high default.
        state.settings.cloud_quality =
            if ["low", "medium", "high"].contains(&self.cloud_quality.as_str()) {
                self.cloud_quality.clone()
            } else {
                default_cloud_quality()
            };
        state.settings.home_variant = self.home_variant.clone();
        state.settings.hostile_wildlife = self.hostile_wildlife;
        state.settings.vitals_drain = self.vitals_drain.clamp(0.0, 5.0);
        // Play mode (task #50): restore the persisted mode, then PRESET the
        // creative (free resources) flag from it -- GuiState defaults that
        // flag to true (early-dev posture), so a Normal-mode player must get
        // survival from frame 0, not from the first bridge tick. Inside
        // Creative/Dev the Inventory page's Creative toggle stays a live
        // fine-tune on top of this preset.
        state.settings.play_mode = self.play_mode;
        state.creative_mode = self.play_mode.allows(Capability::FreeResources);
        // v0.488 voice input prefs.
        state.audio_input_device = self.voice_input_device.clone();
        state.audio_output_device = self.voice_output_device.clone();
        state.voice_gain = self.voice_gain;
        state.voice_filter_mode = self.voice_filter_mode;
        state.voice_transmit_mode = self.voice_transmit_mode;
        state.voice_ptt_key = self.voice_ptt_key.clone();
        state.voice_vad_threshold = self.voice_vad_threshold;
        // Timestamp display format → the app-wide formatter (process global).
        crate::gui::pages::chat::set_timestamp_format(
            crate::gui::pages::chat::TimestampFormat::from_config_str(&self.timestamp_format),
        );
        // Chat panel state
        state.chat_connection_collapsed = self.chat_connection_collapsed;
        state.chat_dm_collapsed = self.chat_dm_collapsed;
        state.chat_groups_collapsed = self.chat_groups_collapsed;
        state.chat_servers_collapsed = self.chat_servers_collapsed;
        state.chat_connected_server_collapsed = self.chat_connected_server_collapsed;
        state.chat_friends_collapsed = self.chat_friends_collapsed;
        state.chat_members_collapsed = self.chat_members_collapsed;
        state.chat_studio_collapsed = self.chat_studio_collapsed;
        state.hud_chat_feed_visible = self.hud_chat_feed_visible;
        // Guard a corrupted saved value: a tiny/zero height would collapse the
        // in-world panel's message list into an unusable sliver.
        state.ingame_chat_panel_height = if self.ingame_chat_panel_height >= 80.0 {
            self.ingame_chat_panel_height.min(400.0)
        } else {
            default_ingame_chat_panel_height()
        };
        state.chat_left_panel_locked = self.chat_left_panel_locked;
        state.chat_right_panel_locked = self.chat_right_panel_locked;
        state.chat_left_panel_width = self.chat_left_panel_width;
        state.chat_right_panel_width = self.chat_right_panel_width;
        // Donation addresses
        state.donate_solana_address = self.donate_solana_address.clone();
        state.donate_btc_address = self.donate_btc_address.clone();
        state.nav_two_tier = self.nav_two_tier;
        if !self.nav_top_category.is_empty() {
            state.nav_top_category = self.nav_top_category.clone();
        }
        if !self.default_page.is_empty() {
            state.default_page = crate::gui::config_str_to_page(&self.default_page);
        }
        // Default launcher character (v0.474). Empty = no default (show picker).
        state.launcher_default_character = self.default_character.clone();
        state.donate_addresses = self.donate_addresses.iter().map(|a| crate::gui::DonateAddress {
            network: a.network.clone(),
            addr_type: a.addr_type.clone(),
            value: a.value.clone(),
            label: a.label.clone(),
        }).collect();

        // Store encrypted key fields so they persist through save cycles
        state.encrypted_private_key = self.encrypted_private_key.clone();
        state.key_salt = self.key_salt.clone();
        state.key_iterations = self.key_iterations;
        // v0.278.0 auto-unlock — pull mode + PIN-encrypted seed in.
        // Startup logic (see lib.rs) inspects state.auto_unlock_mode to
        // decide between silent keychain load / PIN modal / passphrase modal.
        state.auto_unlock_mode = self.auto_unlock_mode;
        state.pin_encrypted_seed = self.pin_encrypted_seed.clone();
        state.pin_salt = self.pin_salt.clone();
        // Full-PQ: Kyber/Dilithium re-derive from the seed — nothing to load.

        // Key handling: default to limited mode (no passphrase prompt on startup).
        // Users can unlock their key later from Settings > Security.
        // This ensures zero-friction startup for new and returning users.
        if self.needs_key_migration() {
            // Parse the legacy hex key into bytes silently (available in memory)
            if let Ok(bytes) = (0..self.private_key_hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&self.private_key_hex[i..i+2], 16))
                .collect::<Result<Vec<u8>, _>>()
            {
                if bytes.len() == 32 {
                    state.private_key_bytes = Some(bytes);
                    log::info!("Legacy plaintext key loaded into memory (encrypt via Settings)");
                    // Full-PQ: derive Dilithium identity + Kyber DM key from
                    // the seed so a legacy-key launch is DM-capable from the
                    // first auto-connect (advertises kyber_public).
                    state.apply_pq_identity();
                }
            }
        } else if self.needs_passphrase() {
            // v0.278.0 auto-unlock: honor the user's mode choice on startup.
            // Default (AlwaysPrompt) preserves the pre-v0.278 behavior — no
            // modal, user clicks Unlock when they need signing.
            use crate::auto_unlock::{AutoUnlockMode, KeychainSlot};
            match self.auto_unlock_mode {
                AutoUnlockMode::Keychain => {
                    // Try silent load. The `profile_public_key` is the
                    // Dilithium hex identity; falls back to public_key_hex
                    // for legacy configs that haven't refreshed yet.
                    let identity = if !self.public_key_hex.is_empty() {
                        self.public_key_hex.as_str()
                    } else {
                        ""
                    };
                    if identity.is_empty() {
                        log::warn!("Auto-unlock (Keychain): no public_key_hex in config; skipping silent unlock");
                    } else {
                        match crate::auto_unlock::keychain_load(KeychainSlot::Seed, identity) {
                            Ok(Some(seed)) => {
                                state.private_key_bytes = Some(seed.to_vec());
                                state.apply_pq_identity();
                                log::info!("Auto-unlock (Keychain): seed loaded from OS keychain — silent unlock OK");
                            }
                            Ok(None) => {
                                // Keychain entry gone (user cleared it, new
                                // device, OS reinstall). Fall back to the
                                // passphrase prompt without panicking the
                                // user — `needs_passphrase()` will surface
                                // the Settings unlock path.
                                log::warn!("Auto-unlock (Keychain): no keychain entry — falling back to passphrase mode");
                            }
                            Err(e) => {
                                // Platform-level failure (no keychain
                                // backend, permissions denied). Same
                                // graceful degrade — keep the user signed
                                // in via Settings Unlock.
                                log::warn!("Auto-unlock (Keychain): keychain load failed: {}. Falling back to passphrase mode.", e);
                            }
                        }
                    }
                }
                AutoUnlockMode::KeychainPin => {
                    // Don't try to unlock automatically — wait for user
                    // input. The startup PIN modal is queued by the UI
                    // layer (chat unlock button + Settings) so we don't
                    // pop a modal over the splash. The seed stays locked
                    // until the user types their PIN.
                    log::info!("Auto-unlock (KeychainPin): waiting for user PIN entry");
                }
                AutoUnlockMode::AlwaysPrompt => {
                    log::info!("Encrypted key found; running in limited mode (unlock via Settings)");
                }
            }
        }
        // passphrase_needed stays false — no modal on startup
    }
}

#[cfg(test)]
mod play_mode_tests {
    //! Guard the play-mode permission surface (task #50). The truth table IS
    //! the spec: if a gate ever needs a different answer, change the table
    //! here deliberately, in the same commit as the gate.
    use super::*;

    #[test]
    fn mode_capability_truth_table() {
        use Capability::*;
        use PlayMode::*;
        // (mode, capability, allowed) -- the WHOLE permission surface,
        // exhaustively: 3 modes x 4 capabilities.
        let table = [
            // Normal: survival. Only building your own home.
            (Normal, HomesteadEditing, true),
            (Normal, FreeResources, false),
            (Normal, DevTools, false),
            (Normal, ShipStructureEditing, false),
            // Creative: free materials, still homestead-scoped, no dev tools.
            (Creative, HomesteadEditing, true),
            (Creative, FreeResources, true),
            (Creative, DevTools, false),
            (Creative, ShipStructureEditing, false),
            // Dev: everything (the operator building the mothership).
            (Dev, HomesteadEditing, true),
            (Dev, FreeResources, true),
            (Dev, DevTools, true),
            (Dev, ShipStructureEditing, true),
        ];
        for (mode, cap, want) in table {
            assert_eq!(
                play_mode_allows(mode, cap),
                want,
                "play_mode_allows({mode:?}, {cap:?}) should be {want}"
            );
            // The method must never drift from the free function.
            assert_eq!(mode.allows(cap), want);
        }
    }

    #[test]
    fn default_is_dev_pre_launch() {
        // PRE-LAUNCH ONLY: the operator is the dev. At launch this flips to
        // Normal -- update PlayMode's #[default] AND this assertion together
        // (this test existing is the reminder that the flip is deliberate).
        assert_eq!(PlayMode::default(), PlayMode::Dev);
    }

    #[test]
    fn play_mode_serde_round_trips_and_defaults() {
        for m in PlayMode::ALL {
            let json = serde_json::to_string(&m).unwrap();
            let back: PlayMode = serde_json::from_str(&json).unwrap();
            assert_eq!(m, back, "PlayMode must survive a config round-trip");
        }
        // A pre-v0.799 config has no play_mode field at all: serde(default)
        // must fill in Dev (today's behavior for the operator's install).
        let minimal = r#"{"server_url":"","user_name":"","public_key_hex":"","completed_onboarding":false}"#;
        let cfg: AppConfig = serde_json::from_str(minimal).unwrap();
        assert_eq!(cfg.play_mode, PlayMode::Dev);
    }
}

#[cfg(all(test, feature = "native"))]
mod pbkdf2_migration_tests {
    //! Guard the 100k → 600k PBKDF2 migration (v0.277.0). Pre-v0.277.0
    //! vaults must still decrypt with their stored legacy iter count, and
    //! a new encrypt must stamp 600_000 — otherwise the silent-upgrade
    //! path in `passphrase_modal::draw_unlock` runs forever on every unlock
    //! (correct, but wastes CPU and never persists the bump).
    use super::*;

    /// Helper: encrypt a 32-byte test key at an explicit iter count.
    /// Mirrors `encrypt_private_key` minus the iter-count constant so
    /// the test can simulate a pre-v0.277.0 100k-iter vault.
    fn encrypt_at(key_bytes: &[u8], passphrase: &str, iters: u32) -> (String, String, u32) {
        use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
        use aes_gcm::aead::generic_array::GenericArray;
        use base64::Engine;
        use base64::engine::general_purpose::STANDARD as B64;
        let mut salt = [0u8; 16];
        getrandom::getrandom(&mut salt).unwrap();
        let mut derived = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(passphrase.as_bytes(), &salt, iters, &mut derived);
        let mut iv = [0u8; 12];
        getrandom::getrandom(&mut iv).unwrap();
        let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived));
        let ct = cipher.encrypt(GenericArray::from_slice(&iv), key_bytes).unwrap();
        let mut combined = Vec::with_capacity(12 + ct.len());
        combined.extend_from_slice(&iv);
        combined.extend_from_slice(&ct);
        (B64.encode(&combined), B64.encode(&salt), iters)
    }

    #[test]
    fn new_encrypts_use_600k_iters() {
        let key = [42u8; 32];
        let (_enc, _salt, iters) = encrypt_private_key(&key, "hunter2").unwrap();
        assert_eq!(iters, PBKDF2_ITERATIONS_NEW);
        assert_eq!(iters, 600_000);
    }

    #[test]
    fn legacy_vault_decrypts_with_stored_iters() {
        let key = [7u8; 32];
        let (enc, salt, iters) = encrypt_at(&key, "legacy", PBKDF2_ITERATIONS_LEGACY);
        assert_eq!(iters, 100_000);
        let decrypted = decrypt_private_key(&enc, &salt, "legacy", iters).unwrap();
        assert_eq!(decrypted, key);
    }

    #[test]
    fn new_vault_decrypts_at_new_iters() {
        let key = [9u8; 32];
        let (enc, salt, iters) = encrypt_private_key(&key, "passw0rd").unwrap();
        let decrypted = decrypt_private_key(&enc, &salt, "passw0rd", iters).unwrap();
        assert_eq!(decrypted, key);
    }

    #[test]
    fn wrong_iter_count_fails_decrypt() {
        // Vault written at 100k; trying to unlock at 600k must fail —
        // the derived key bytes are different, so AES-GCM auth rejects.
        // This is what enforces the "iter count is metadata, not a guess"
        // contract.
        let key = [1u8; 32];
        let (enc, salt, _) = encrypt_at(&key, "pw", PBKDF2_ITERATIONS_LEGACY);
        let result = decrypt_private_key(&enc, &salt, "pw", PBKDF2_ITERATIONS_NEW);
        assert!(result.is_err(), "decrypt with wrong iter count must fail");
    }

    #[test]
    fn migration_round_trip() {
        // Simulate the silent-upgrade path: encrypt at 100k, decrypt at 100k,
        // re-encrypt (which now stamps 600k), decrypt at 600k.
        let key = [123u8; 32];
        let pass = "migrate-me";

        let (enc_old, salt_old, iters_old) = encrypt_at(&key, pass, PBKDF2_ITERATIONS_LEGACY);
        let decrypted_old = decrypt_private_key(&enc_old, &salt_old, pass, iters_old).unwrap();
        assert_eq!(decrypted_old, key);

        let (enc_new, salt_new, iters_new) = encrypt_private_key(&decrypted_old, pass).unwrap();
        assert_eq!(iters_new, PBKDF2_ITERATIONS_NEW);
        // Salt MUST be fresh — re-encrypt picks a new salt; reusing the
        // old one would be a regression (same passphrase + same salt at
        // a different iter count is fine cryptographically, but the new
        // encrypt path is supposed to roll the salt every time).
        assert_ne!(salt_new, salt_old);

        let decrypted_new = decrypt_private_key(&enc_new, &salt_new, pass, iters_new).unwrap();
        assert_eq!(decrypted_new, key);
    }

    #[test]
    fn corrupt_zero_iter_count_is_clamped() {
        // Defensive path: a fuzzed/corrupt config with iters=0 must NOT
        // silently derive a deterministic zero-iter key. The decrypt
        // function clamps to the legacy floor, so a 100k-iter vault still
        // unlocks even if iters arrives as 0.
        let key = [5u8; 32];
        let (enc, salt, _) = encrypt_at(&key, "pw", PBKDF2_ITERATIONS_LEGACY);
        let decrypted = decrypt_private_key(&enc, &salt, "pw", 0).unwrap();
        assert_eq!(decrypted, key);
    }

    #[test]
    fn fresh_install_defaults_apply_serde_field_defaults() {
        // Guards the manual Default impl (found 2026-07-11 by the planet
        // aesthetics boot test): the derived Default ignored every serde
        // field default, so fresh installs booted with planet_detail=false,
        // vsync=false, fov=0, all volumes 0. If a future field is added
        // without a serde default, `from_str("{}")` fails and this test
        // catches it before a fresh install panics.
        let c = AppConfig::default();
        assert!(c.vsync);
        assert!(c.planet_detail);
        assert!(c.planet_chunked);
        assert!(c.planet_atmo_scatter);
        assert!(c.planet_surface_detail);
        assert_eq!(c.cloud_quality, "high");
        assert!(c.sky_constellations);
        assert!(c.sky_milkyway_glow);
        assert_eq!(c.sky_glow_tier, "standard");
        assert!(c.sky_star_halos);
        assert_eq!(c.fov, 90.0);
        assert_eq!(c.master_volume, 0.8);
        assert_eq!(c.vitals_drain, 1.0);
        assert_eq!(c.planet_max_subdiv, 6.0);
        // Fresh installs see the concept tour exactly once: the serde
        // default is true (pre-v0.198 configs skip it) but the no-config
        // path deliberately flips it back to false.
        assert!(!c.concept_tour_seen);
        // And an EMPTY EXISTING config (old file, field absent) keeps the
        // veteran behavior: tour marked seen.
        let old: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(old.concept_tour_seen);
    }
}
