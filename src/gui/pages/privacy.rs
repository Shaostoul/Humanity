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

use egui::{Frame, RichText, CornerRadius, Stroke, Vec2};
use humanity_core::i18n::{t, t_marked, tf};
use serde::Deserialize;

use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
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
        }
    }
    // 2) Directory listing rides the profile privacy JSON, in the whole profile every save sends
    // (own_profile.rs `pump`, BUG-194 and BUG-196; it holds Step F's recovery-phrase guard, so
    // holding the phrase the profile is not sent while the presence flag still is).
    state.profile_directory_listed = !tier.directory_unlisted;
    crate::gui::pages::own_profile::want_save(state);
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
            Stroke::new(2.0_f32, theme.accent())
        } else {
            Stroke::new(1.0_f32, theme.border())
        })
        .corner_radius(CornerRadius::same(theme.border_radius as u8))
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
        .hint("privacy.tier_card_hint")
        .clicked();
    if clicked {
        return true;
    }
    false
}

/// The shared tier chooser body (modal + Settings section reuse it).
/// Returns Some(tier_id) when the user clicked Apply.
fn draw_tier_chooser(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    apply_label: &str,
    apply_hint: &'static str,
) -> Option<String> {
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
        if widgets::Button::primary(apply_label).hint(apply_hint).show(ui, theme) {
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
    egui::Window::new(t("privacy.modal_window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(460.0, 0.0))
        .frame(Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new(t("privacy.modal_title"))
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
            // "Direct messages", not "your messages" (2026-11-03): channel posts are public
            // (BUG-223), so the old "Your messages are end-to-end encrypted" overpromised.
            widgets::glossary_words(ui, theme, t_marked("privacy.modal_intro"), theme.font_size_small, theme.text_secondary());
            ui.add_space(theme.spacing_sm);
            applied = draw_tier_chooser(ui, theme, state, t("privacy.modal_apply"), "privacy.modal_apply_hint");
        });
    if let Some(tier_id) = applied {
        apply_tier(state, &tier_id);
        state.privacy_tier_prompt_open = false;
    }
}

/// Settings → Privacy section: the same chooser plus individual switches, then what every
/// switch leaves visible.
///
/// THE PAGE SAYS EXACTLY WHAT OTHERS CAN SEE (BUG-235, 2026-11-03). A "Profile Visible to
/// Others" toggle stood below this card until then: saved, loaded and read by nothing, so a
/// person who turned it off was told their profile was hidden and it was not. It was removed
/// rather than left while a real one is designed, and these words took its place. Each switch
/// says what it hides, and the last block says what none of them hides, so nobody leaves the
/// page believing they are less visible than they are. The facts, and where each is enforced:
/// - presence: relay `privacy_update` (storage/members.rs `set_hide_presence`); a hidden member
///   stays in the full user list, masked as offline (handlers/broadcast.rs).
/// - the directory: `privacy.directory = "unlisted"` in the profile's privacy map leaves the
///   person out of `/api/members` and its search (storage/members.rs `MEMBER_DIR_VISIBLE`), and
///   makes the copy for other servers the withdrawal (humanity_protocol::profile_copy), so
///   `GET /api/profile/{key}` answers not found.
/// - always visible on a server the person uses: their name, key and role in the full user list
///   (storage/pins.rs `list_all_users_with_keys`, which reads no privacy map), and their profile
///   to anyone who asks for it by name (handlers/profile.rs `handle_profile_request`,
///   storage/profile.rs `get_public_profile`: bio, links, pictures always; pronouns, location
///   and website unless kept for friends).
/// - channel posts: public in a channel open to Anyone (BUG-223, docs/design/chat-access.md).
/// The website's Settings > Privacy says the same, from the same keys.
pub(crate) fn draw_privacy_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(
        RichText::new(t("privacy.title"))
            .size(theme.font_size_title)
            .color(theme.text_primary()),
    );
    ui.add_space(theme.spacing_xs);
    widgets::glossary_words(ui, theme, t_marked("privacy.intro"), theme.font_size_small, theme.text_muted());
    ui.add_space(theme.spacing_md);
    if let Some(tier_id) = draw_tier_chooser(ui, theme, state, t("privacy.apply_tier"), "privacy.apply_tier_hint") {
        apply_tier(state, &tier_id);
    }
    if !state.settings.privacy_tier.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new(tf("privacy.current_tier", &[("tier", &state.settings.privacy_tier)]))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
    }
    ui.add_space(theme.spacing_md);
    widgets::subsection_label(ui, theme, t("privacy.switches_title"));
    widgets::body_hint(ui, theme, t("privacy.switches_intro"));
    ui.add_space(theme.spacing_xs);
    // Directory listing (same switch as the Profile page).
    if ui
        .checkbox(&mut state.profile_directory_listed, t("privacy.directory_label"))
        .hint("privacy.directory_does")
        .changed()
    {
        // The whole profile goes (own_profile.rs `pump`, with Step F's recovery-phrase guard;
        // BUG-194: this sent the bio alone, and so wiped the picture and everything else).
        crate::gui::pages::own_profile::want_save(state);
    }
    // What it does, always shown (not a hover hint the hint mode can hide): this is the line a
    // person reads to know what they are hidden from, and what they are not.
    widgets::glossary_words(ui, theme, t_marked("privacy.directory_does"), theme.font_size_small, theme.text_muted());
    ui.add_space(theme.spacing_sm);
    // Presence: the SAME persisted switch the old Settings toggle showed,
    // now actually ENFORCED server-side via privacy_update (before
    // 2026-08-23 the toggle was written to config and read by nothing).
    let mut visible_presence = state.settings.online_status_visible;
    if ui
        .checkbox(&mut visible_presence, t("privacy.presence_label"))
        .hint("privacy.presence_does")
        .changed()
    {
        state.settings.online_status_visible = visible_presence;
        state.settings_dirty = true;
        send_presence_flag(state);
    }
    widgets::body_hint(ui, theme, t("privacy.presence_does"));
    ui.add_space(theme.spacing_md);
    widgets::subsection_label(ui, theme, t("privacy.seen_title"));
    // Each key written out (not a loop over names) so tests/i18n_lint.rs checks every one exists.
    for words in [
        t_marked("privacy.seen_people"),
        t_marked("privacy.seen_profile"),
        t_marked("privacy.seen_posts"),
        t_marked("privacy.seen_no_switch"),
    ] {
        widgets::glossary_words(ui, theme, words, theme.font_size_small, theme.text_secondary());
        ui.add_space(theme.spacing_xs);
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
