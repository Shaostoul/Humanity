//! Privacy tiers (operator direction, 2026-08-23): every user chooses
//! exactly how private to be, ONCE, up front — and the default is
//! maximum privacy. Tiers are DATA (`data/gui/privacy_tiers.json`, per
//! the infinite-of-x rule): presets from "Private" (hidden presence,
//! unlisted directory — the default) to "Open" (seen online, listed).
//! (A fourth, "Spotlight", promised to show a live stream to viewers and
//! carried a `promote_streaming` field nothing read; it was taken out until
//! that is built, BUG-238, 2026-11-03. tests/settings_used_lint.rs fails on
//! a tier switch that this app or the website does not read.)
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
//! relay keeps new accounts presence-hidden (fail-private), and this page
//! draws presence off until then, as the website does (BUG-240, 2026-11-03:
//! it drew it on). The directory box is the server's copy, and a server
//! lists a member who never chose, so it draws ticked then, truly.

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

/// The levels in a privacy-levels file, or why it is refused. A level's `name`, `tagline` and
/// `description` are KEYS in data/i18n/en.json (2026-11-03, BUG-237: the website draws the same
/// cards from the same file, in the same words), so a file with a missing key is refused whole,
/// as `gui::navigation` refuses a menu row: a key shown as itself is worse than the fallback.
fn parse_tiers(text: &str) -> Result<(String, Vec<PrivacyTier>), String> {
    let f = serde_json::from_str::<TierFile>(text).map_err(|e| format!("not readable: {e}"))?;
    if f.tiers.is_empty() {
        return Err("no levels".into());
    }
    for tier in &f.tiers {
        for key in [&tier.name, &tier.tagline, &tier.description] {
            if !humanity_core::i18n::has(key) {
                return Err(format!("level {}: \"{key}\" is not a key in data/i18n/en.json", tier.id));
            }
        }
    }
    if !f.tiers.iter().any(|t| t.id == f.default_tier) {
        return Err(format!("the default level \"{}\" is not one of its levels", f.default_tier));
    }
    Ok((f.default_tier, f.tiers))
}

/// The one level used when the file is missing or refused: the most private (fail-private).
fn fallback_tiers() -> (String, Vec<PrivacyTier>) {
    (
        "private".to_string(),
        vec![PrivacyTier {
            id: "private".into(),
            name: "privacy.tier_private_name".into(),
            tagline: "privacy.tier_private_tagline".into(),
            description: "privacy.tier_private_description".into(),
            hide_presence: true,
            directory_unlisted: true,
        }],
    )
}

/// Load tiers from data (cached on GuiState after first use).
pub fn load_tiers() -> (String, Vec<PrivacyTier>) {
    let path = std::path::Path::new("data/gui/privacy_tiers.json");
    match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| parse_tiers(&s)) {
        Ok(levels) => levels,
        Err(why) => {
            log::warn!("data/gui/privacy_tiers.json refused ({why}); using the one Private level");
            fallback_tiers()
        }
    }
}

fn ensure_tiers_loaded(state: &mut GuiState) {
    if state.privacy_tiers_cache.is_empty() {
        let (default_id, tiers) = load_tiers();
        // A last level the file no longer has (Spotlight, taken out 2026-11-03, BUG-238)
        // pre-selects the default: picked, not applied, until the person presses the button.
        let chosen = &state.settings.privacy_tier;
        let known = tiers.iter().any(|t| &t.id == chosen);
        state.privacy_tiers_cache = tiers;
        if state.privacy_tier_selection.is_empty() {
            state.privacy_tier_selection = if known { chosen.clone() } else { default_id };
        }
    }
}

/// The words of the level last used (`settings.privacy_tier`), or None when none was chosen or
/// the file no longer has it.
fn current_tier_name(state: &GuiState) -> Option<&'static str> {
    state
        .privacy_tiers_cache
        .iter()
        .find(|t| t.id == state.settings.privacy_tier)
        .map(|t| humanity_core::i18n::t_dyn(&t.name))
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

/// One selectable tier card. Returns true when clicked. Its words are keys (the levels' file).
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
                    RichText::new(humanity_core::i18n::t_dyn(&tier.name))
                        .size(theme.font_size_body)
                        .color(theme.text_primary())
                        .strong(),
                );
                ui.label(
                    RichText::new(humanity_core::i18n::t_dyn(&tier.tagline))
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            });
            ui.label(
                RichText::new(humanity_core::i18n::t_dyn(&tier.description))
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
    // A popup asking for a decision (gui/layers.rs): its shade covers the menus, and Back and Escape
    // leave it alone (the page under it does not move) until a level is chosen.
    let shade = crate::gui::layers::shade(ctx, egui::Id::new("privacy_tier_modal"), crate::gui::layers::Close::Decide, Theme::c32(&theme.bg_modal));
    let window = crate::gui::layers::window(ctx, egui::Id::new("privacy_tier_modal"), t("privacy.modal_window"), crate::gui::layers::Layer::PopupWindow)
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
    if let Some(w) = window {
        crate::gui::layers::seat(ctx, &shade, w.response.layer_id);
    }
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
/// - the directory: only `privacy.directory = "listed"` in the profile's privacy map puts the
///   person in `/api/members` and its search (storage/members.rs `MEMBER_DIR_VISIBLE`; anything
///   else, never choosing included, leaves them out since BUG-241, 2026-11-03), and being out
///   makes the copy for other servers the withdrawal (humanity_protocol::profile_copy), so
///   `GET /api/profile/{key}` answers not found.
/// - always visible on a server the person uses: their name, key and role in the full user list
///   (storage/pins.rs `list_all_users_with_keys`, which reads no privacy map), and their profile
///   to anyone who asks for it by name (handlers/profile.rs `handle_profile_request`,
///   storage/profile.rs `get_public_profile`: bio, links, pictures always; pronouns, location
///   and website unless kept for friends).
/// - kept for friends (BUG-239, 2026-11-03, `privacy.kept_for_friends`, the website's Profile page
///   shows the same key under its locks): `handle_profile_request` shows such a field only to
///   someone presenting a friendship pass its owner gave them on this server, not withdrawn
///   (handlers/friend_passes.rs `friend_pass`). Each app gives one to every mutual follow and with
///   each contact request it sends (engine/reach.rs `contact_request_puts`; the web's
///   chat-dm-store.js `passesOwed`), and Unfollow and Block withdraw it. Following itself is held
///   in each app since the server's follows table went (2026-08-24).
/// - channel posts: public in a channel open to Anyone (BUG-223, docs/design/chat-access.md).
/// The website's Settings > Privacy says the same, from the same keys, and since BUG-237
/// (2026-11-03) has the same level cards and both switches (web/pages/settings-privacy.js).
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
    // The level's own words, not its id ("Private", not "private"); the website shows the same.
    if let Some(name) = current_tier_name(state) {
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new(tf("privacy.current_tier", &[("tier", &name)]))
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
    // How much of a message the DM list shows (2026-11-03, chat/dm_rows.rs): a preview can be read
    // by anyone who can see the screen, so the choice sits with the privacy switches. The same
    // choice is in the DMs header's cog.
    widgets::subsection_label(ui, theme, t("chat.dm_preview_title"));
    widgets::body_hint(ui, theme, t("chat.dm_preview_intro"));
    ui.add_space(theme.spacing_xs);
    if let Some(lines) = crate::gui::pages::chat::dm_rows::picker(ui, theme, state.settings.dm_preview_lines) {
        state.settings.dm_preview_lines = lines;
        state.settings_dirty = true;
    }
    ui.add_space(theme.spacing_md);
    widgets::subsection_label(ui, theme, t("privacy.seen_title"));
    // Each key written out (not a loop over names) so tests/i18n_lint.rs checks every one exists.
    for words in [
        t_marked("privacy.seen_people"),
        t_marked("privacy.seen_profile"),
        t_marked("privacy.kept_for_friends"),
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
#[cfg(test)]
mod tests {
    use super::*;

    /// One headless frame (no GPU) of Settings > Privacy, and whether each switch's box is drawn
    /// ticked: (the directory, "Show others when I am online"). egui paints a ticked box's mark as
    /// one open three-point line (egui widgets/checkbox.rs `Shape::line`) inside the box, which
    /// stands just left of the label on its row; an unticked box has none. The pixels, not the
    /// field the box binds, so a box drawn from something else would be caught too.
    fn drawn_switches(state: &mut GuiState) -> (bool, bool) {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 2400.0))),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run(input(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw_privacy_content(ui, &theme, state));
            }));
        }
        let shapes = out.expect("a frame").shapes;
        let mut marks: Vec<egui::Rect> = Vec::new();
        fn visit(shape: &egui::Shape, marks: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, marks)),
                egui::Shape::Path(p) if !p.closed && p.points.len() == 3 => {
                    marks.push(egui::Rect::from_points(&p.points));
                }
                _ => {}
            }
        }
        shapes.iter().for_each(|cs| visit(&cs.shape, &mut marks));
        let ticked = |key: &'static str| {
            let label = crate::gui::screen_surface::find_text_in_shapes(&shapes, t(key))
                .unwrap_or_else(|| panic!("{key} is drawn"))
                .rect;
            marks.iter().any(|m| {
                let row = label.y_range().expand(label.height() * 0.5);
                row.contains(m.center().y) && m.max.x <= label.min.x + 1.0 && m.min.x >= label.min.x - 40.0
            })
        };
        (ticked("privacy.directory_label"), ticked("privacy.presence_label"))
    }

    /// BEFORE THE FIRST CHOICE THE SWITCHES SAY WHAT A SERVER DOES (BUG-240 and BUG-241,
    /// 2026-11-03). A server keeps a new member presence-hidden until a privacy level is chosen
    /// (relay storage/members.rs `join_server_on`, `hide_presence = 1`), so a fresh install draws
    /// "Show others when I am online" unticked, as the website does (web/shared/privacy-tiers.js
    /// `presenceHidden` null). And since BUG-241 a server lists only a member whose privacy map says
    /// `"directory": "listed"` (`MEMBER_DIR_VISIBLE`), which nobody has until they choose it, so
    /// the directory box is unticked too, as the website's `directoryListed` draws it; the box
    /// still follows each server's own copy once it comes (own_profile.rs `show`).
    /// Seen red 2026-11-03 with the old presence default (`online_status_visible: true` in
    /// Settings::default and the config's `default_true`): "a fresh install draws presence as
    /// hidden (GuiState::default)", left (true, true), right (true, false); and with the old
    /// directory default (`profile_directory_listed: true`, BUG-241): left (true, false), right
    /// (false, false).
    #[test]
    fn a_fresh_install_draws_presence_hidden_as_a_server_keeps_it() {
        let mut fresh = GuiState::default();
        assert_eq!(drawn_switches(&mut fresh), (false, false), "a fresh install draws both switches as a server keeps a new member (GuiState::default)");
        // The way a first launch really starts: no config.json, so AppConfig's defaults applied.
        let mut first_launch = GuiState::default();
        crate::config::AppConfig::default().apply_to_gui_state(&mut first_launch);
        assert!(first_launch.settings.privacy_tier.is_empty(), "no level chosen yet");
        assert_eq!(drawn_switches(&mut first_launch), (false, false), "a fresh install draws both switches as a server keeps a new member (no config.json)");
    }

    /// A choice already made loads as it was: the new default reaches only a config that never
    /// held the switch (every save writes it). Seen both ways, and the box follows the file.
    #[test]
    fn a_choice_already_made_loads_as_it_was() {
        for (file, seen) in [
            (r#"{"privacy_tier":"open","online_status_visible":true}"#, true),
            (r#"{"privacy_tier":"private","online_status_visible":false}"#, false),
            (r#"{"privacy_tier":"","online_status_visible":true}"#, true),
        ] {
            let cfg: crate::config::AppConfig = serde_json::from_str(file).expect("a config");
            let mut gs = GuiState::default();
            cfg.apply_to_gui_state(&mut gs);
            assert_eq!(gs.settings.online_status_visible, seen, "{file}");
            assert_eq!(drawn_switches(&mut gs).1, seen, "{file}: the box shows the choice");
            let back = crate::config::AppConfig::from_gui_state(&gs);
            assert_eq!(back.online_status_visible, seen, "{file}: and the next save keeps it");
        }
    }

    /// The shipped levels load as written (not the fallback), each word a key en.json has, and the
    /// default is the most private. Seen red, 2026-11-03, with the Private level's name key
    /// misspelt in the file: parse_tiers refused it ("privacy.tier_privat_name" is not a key).
    #[test]
    fn the_shipped_levels_load_in_words_by_key() {
        let text = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/gui/privacy_tiers.json"));
        let (default_id, tiers) = parse_tiers(text).expect("the shipped levels are accepted");
        assert_eq!(default_id, "private");
        let ids: Vec<&str> = tiers.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["private", "balanced", "open"], "Spotlight is gone until it is built (BUG-238)");
        assert!(tiers[0].hide_presence && tiers[0].directory_unlisted, "the default hides presence and leaves the directory");
        for t in &tiers {
            for key in [&t.name, &t.tagline, &t.description] {
                assert_ne!(humanity_core::i18n::t_dyn(key), key.as_str(), "{key} has words");
            }
        }
    }

    /// A level whose words are missing, or a default that is not a level, refuses the file whole;
    /// the fallback is one Private level, in words that exist.
    #[test]
    fn a_file_with_a_missing_key_is_refused_whole() {
        let level = |name: &str| format!(
            r#"{{"id":"private","name":"{name}","tagline":"privacy.tier_private_tagline","description":"privacy.tier_private_description","hide_presence":true,"directory_unlisted":true}}"#
        );
        let file = |default: &str, name: &str| format!(r#"{{"default_tier":"{default}","tiers":[{}]}}"#, level(name));
        assert!(parse_tiers(&file("private", "privacy.tier_private_name")).is_ok(), "the good file, as a control");
        let why = parse_tiers(&file("private", "privacy.no_such_key")).unwrap_err();
        assert!(why.contains("privacy.no_such_key"), "{why}");
        assert!(parse_tiers(&file("loud", "privacy.tier_private_name")).is_err(), "a default that is not a level");
        assert!(parse_tiers(r#"{"default_tier":"private","tiers":[]}"#).is_err(), "no levels");
        let (id, tiers) = fallback_tiers();
        assert_eq!((id.as_str(), tiers.len()), ("private", 1));
        assert!(tiers[0].hide_presence && tiers[0].directory_unlisted);
        assert!(humanity_core::i18n::has(&tiers[0].name) && humanity_core::i18n::has(&tiers[0].description));
    }
}
