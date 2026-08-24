//! Privacy tiers (operator direction, 2026-08-23): every user chooses
//! exactly how private to be, ONCE, up front — and the default is
//! maximum privacy. Tiers are DATA (`data/gui/privacy_tiers.json`, per
//! the infinite-of-x rule): four presets from "Private" (hidden
//! presence, unlisted directory — the default) to "Spotlight" (maximum
//! discoverability, for streamers who WANT to be found).
//!
//! A tier is a preset over two real switches, both individually
//! overridable afterwards in Settings → Privacy:
//! - `hide_presence` (server-enforced: never online, no last_seen
//!   stored, no join/leave/typing signals — relay `privacy_update`)
//! - directory listing (`privacy.directory` in the profile privacy
//!   JSON — the existing member-directory opt-out)
//!
//! The first-connect modal (drawn globally from lib.rs) records the
//! choice in AppConfig so it never re-asks; until a choice is made the
//! relay keeps new accounts presence-hidden (fail-private).

use egui::{Frame, RichText, Rounding, Stroke, Vec2};
use serde::Deserialize;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};

#[derive(Debug, Clone, Deserialize)]
pub struct PrivacyTier {
    pub id: String,
    pub name: String,
    pub tagline: String,
    pub description: String,
    #[serde(default)]
    pub hide_presence: bool,
    #[serde(default)]
    pub directory_unlisted: bool,
    #[serde(default)]
    pub promote_streaming: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct TierFile {
    #[serde(default = "default_version")]
    #[allow(dead_code)]
    version: u32,
    default_tier: String,
    tiers: Vec<PrivacyTier>,
}

fn default_version() -> u32 { 1 }

/// Load tiers from data (cached on GuiState after first use).
pub fn load_tiers() -> (String, Vec<PrivacyTier>) {
    let path = std::path::Path::new("data/gui/privacy_tiers.json");
    match std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<TierFile>(&s).ok())
    {
        Some(f) if !f.tiers.is_empty() => (f.default_tier, f.tiers),
        _ => {
            // Fail-private fallback if the data file is missing/broken:
            // one tier, maximum privacy.
            (
                "private".to_string(),
                vec![PrivacyTier {
                    id: "private".into(),
                    name: "Private".into(),
                    tagline: "Maximum privacy.".into(),
                    description: "You never appear online and are not listed in the public directory.".into(),
                    hide_presence: true,
                    directory_unlisted: true,
                    promote_streaming: false,
                }],
            )
        }
    }
}

fn ensure_tiers_loaded(state: &mut GuiState) {
    if state.privacy_tiers_cache.is_empty() {
        let (default_id, tiers) = load_tiers();
        state.privacy_tiers_cache = tiers;
        if state.privacy_tier_selection.is_empty() {
            state.privacy_tier_selection = if state.settings.privacy_tier.is_empty() {
                default_id
            } else {
                state.settings.privacy_tier.clone()
            };
        }
    }
}

/// Apply a tier: server-side presence flag + directory listing + local
/// persistence. Safe to call repeatedly.
pub fn apply_tier(state: &mut GuiState, tier_id: &str) {
    ensure_tiers_loaded(state);
    let Some(tier) = state.privacy_tiers_cache.iter().find(|t| t.id == tier_id).cloned() else {
        return;
    };
    // 1) Presence (server-enforced).
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            client.send(
                &serde_json::json!({ "type": "privacy_update", "hide_presence": tier.hide_presence })
                    .to_string(),
            );
            // 2) Directory listing rides the profile privacy JSON (the
            // same shape the Profile page's Save sends).
            let privacy = if tier.directory_unlisted {
                "{\"directory\":\"unlisted\"}".to_string()
            } else {
                "{}".to_string()
            };
            let mut msg = serde_json::json!({
                "type": "profile_update",
                "bio": state.profile_network_bio.trim(),
                "socials": "{}",
                "privacy": privacy,
            });
            let avatar = state.profile_network_avatar.trim();
            if !avatar.is_empty() {
                msg["avatar_url"] = serde_json::Value::String(avatar.to_string());
            }
            client.send(&msg.to_string());
        }
    }
    state.profile_directory_listed = !tier.directory_unlisted;
    state.settings.online_status_visible = !tier.hide_presence;
    // 3) Persist the choice so the first-connect prompt never re-asks.
    state.settings.privacy_tier = tier.id.clone();
    state.privacy_tier_selection = tier.id.clone();
    crate::config::AppConfig::from_gui_state(state).save();
}

/// One selectable tier card. Returns true when clicked.
fn tier_card(ui: &mut egui::Ui, theme: &Theme, tier: &PrivacyTier, selected: bool) -> bool {
    let width = ui.available_width();
    let resp = Frame::NONE
        .fill(if selected { theme.bg_panel() } else { theme.bg_card() })
        .stroke(if selected {
            Stroke::new(2.0, theme.accent())
        } else {
            Stroke::new(1.0, theme.border())
        })
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.card_padding)
        .show(ui, |ui| {
            ui.set_min_width(width - theme.card_padding as f32 * 2.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&tier.name)
                        .size(theme.font_size_body)
                        .color(theme.text_primary())
                        .strong(),
                );
                ui.label(
                    RichText::new(&tier.tagline)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            });
            ui.label(
                RichText::new(&tier.description)
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        })
        .response;
    let clicked = ui
        .interact(resp.rect, ui.id().with(&tier.id), egui::Sense::click())
        .clicked();
    if clicked {
        return true;
    }
    false
}

/// The shared tier chooser body (modal + Settings section reuse it).
/// Returns Some(tier_id) when the user clicked Apply.
fn draw_tier_chooser(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, apply_label: &str) -> Option<String> {
    ensure_tiers_loaded(state);
    let tiers = state.privacy_tiers_cache.clone();
    let mut selection = state.privacy_tier_selection.clone();
    for tier in &tiers {
        if tier_card(ui, theme, tier, selection == tier.id) {
            selection = tier.id.clone();
        }
        ui.add_space(theme.spacing_xs);
    }
    state.privacy_tier_selection = selection.clone();
    ui.add_space(theme.spacing_sm);
    let mut applied = None;
    ui.horizontal(|ui| {
        if widgets::Button::primary(apply_label).show(ui, theme) {
            applied = Some(selection);
        }
    });
    applied
}

/// First-connect modal: shown (from lib.rs, over any page) the first time
/// this identity is connected and no tier has ever been chosen.
pub fn draw_privacy_tier_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if !state.privacy_tier_prompt_open {
        return;
    }
    let mut applied: Option<String> = None;
    egui::Window::new("Choose your privacy")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(460.0, 0.0))
        .frame(Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("How visible do you want to be?")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new(
                    "Your messages are end-to-end encrypted whatever you pick. This only \
                     controls whether others can see you online and find you in directories. \
                     You can change it any time in Settings, and adjust each switch separately.",
                )
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);
            applied = draw_tier_chooser(ui, theme, state, "Use this privacy level");
        });
    if let Some(tier_id) = applied {
        apply_tier(state, &tier_id);
        state.privacy_tier_prompt_open = false;
    }
}

/// Settings → Privacy section: the same chooser plus individual switches.
pub(crate) fn draw_privacy_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(
        RichText::new("Privacy")
            .size(theme.font_size_title)
            .color(theme.text_primary()),
    );
    ui.add_space(theme.spacing_xs);
    ui.label(
        RichText::new(
            "Messages are end-to-end encrypted regardless of these settings; the server \
             also stores no record of who you message. These control what OTHER people \
             can observe about you.",
        )
        .size(theme.font_size_small)
        .color(theme.text_muted()),
    );
    ui.add_space(theme.spacing_md);
    if let Some(tier_id) = draw_tier_chooser(ui, theme, state, "Apply tier") {
        apply_tier(state, &tier_id);
    }
    if !state.settings.privacy_tier.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new(format!("Current tier: {}", state.settings.privacy_tier))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
    }
    ui.add_space(theme.spacing_md);
    widgets::subsection_label(ui, theme, "Fine-grained switches");
    widgets::body_hint(
        ui, theme,
        "A tier is just a preset over these. Changing one moves you off the preset, which is fine.",
    );
    // Directory listing (same switch as the Profile page).
    if ui
        .checkbox(
            &mut state.profile_directory_listed,
            "List me in the public member directory",
        )
        .changed()
    {
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let privacy = if state.profile_directory_listed {
                    "{}".to_string()
                } else {
                    "{\"directory\":\"unlisted\"}".to_string()
                };
                client.send(&serde_json::json!({
                    "type": "profile_update",
                    "bio": state.profile_network_bio.trim(),
                    "socials": "{}",
                    "privacy": privacy,
                }).to_string());
            }
        }
    }
    // Presence: the SAME persisted switch the old Settings toggle showed,
    // now actually ENFORCED server-side via privacy_update (before
    // 2026-08-23 the toggle was written to config and read by nothing).
    let mut visible_presence = state.settings.online_status_visible;
    if ui
        .checkbox(&mut visible_presence, "Show others when I am online")
        .on_hover_text("Off = you never appear online, no last-seen time is stored server-side, and no join/leave or typing signals are sent. Friends can still message you.")
        .changed()
    {
        state.settings.online_status_visible = visible_presence;
        state.settings_dirty = true;
        send_presence_flag(state);
    }
}

/// Push the presence flag to the connected relay (hide = NOT visible).
pub(crate) fn send_presence_flag(state: &GuiState) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            client.send(&serde_json::json!({
                "type": "privacy_update",
                "hide_presence": !state.settings.online_status_visible,
            }).to_string());
        }
    }
}
