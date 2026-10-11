//! Headless tests of Server Settings > ADMIN > Media (media.rs). Each was seen to fail once before
//! it passed; what it saw red is in its comment.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::theme::Theme;

const SERVER: &str = "http://127.0.0.1:9";
const MB: i64 = 1024 * 1024;

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1100.0, 1600.0))),
        events,
        ..Default::default()
    };
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw_admin(ui, theme, state));
    })
}

fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    let out = frame(ctx, theme, state, Vec::new());
    let found = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn"));
    let pos = found.rect.center();
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

fn shows(out: &egui::FullOutput, text: &str) -> bool {
    find_text_in_shapes(&out.shapes, text).is_some()
}

fn item(id: i64, title: &str, size: i64) -> api::Item {
    api::Item { id, title: title.into(), url: format!("/uploads/{id}_x.png"), visibility: "public".into(), size_bytes: size, uploader_key: "k2".into(), uploader_name: "Ann".into(), ..Default::default() }
}

/// An admin on `SERVER` with the server's settings, its categories, a suggestion and the usage
/// already in (nothing is asked of the network).
pub(crate) fn admin_state() -> GuiState {
    let mut state = GuiState::default();
    state.server_url = SERVER.to_string();
    state.profile_public_key = "me".to_string();
    state.upload_token = Some((SERVER.to_string(), "tok".to_string()));
    // An unlocked identity, so a Remove that did not ask first would really start (and fail at
    // once: nothing listens at the test's address).
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.chat_users.push(crate::gui::ChatUser { name: "Me".into(), public_key: "me".into(), role: "admin".into(), status: String::new() });
    let mut s = crate::relay::storage::ServerSettings::default();
    s.media_budget_mb = 50 * 1024;
    s.media_members_note = "The relay's own words about members.".into();
    state.server_settings = Some(s);
    crate::gui::pages::media::sync_server(&mut state);
    let a = &mut state.media.admin;
    a.requested = true;
    a.cats = vec![
        api::CategoryEdit { id: Some(1), name: "General".into(), description: String::new() },
        api::CategoryEdit { id: Some(2), name: "Fire flow arts".into(), description: "Staff, poi, darts".into() },
    ];
    a.suggestions = vec![api::Suggestion { id: 4, suggester_key: "k2".into(), suggester_name: "Ann".into(), name: "Builds".into(), note: "Ships and homes".into(), state: "open".into(), ..Default::default() }];
    a.usage = Some(api::Usage {
        used_bytes: 12 * 1024 * MB + 410 * MB,
        budget_bytes: 50 * 1024 * MB,
        per_person_bytes: 512 * MB,
        items: 2,
        largest: vec![item(9, "Contact staff, slow spin", 900 * MB), item(8, "Garden", 2 * MB)],
        oldest: vec![item(8, "Garden", 2 * MB)],
        least_viewed: vec![],
        waiting: vec![api::Waiting { filename: "1700_clip.webm".into(), uploader_name: "Ben".into(), size_bytes: 40 * MB, uploaded_at: 1, expires_at: 2 }],
    });
    state
}

/// THE SECTION SHOWS THE MEDIA SETTINGS, with the relay's own words under the switch: who sees
/// the page, the space and each person's share, and previews only.
///
/// Seen red 2026-10-11 with the line taken from this app's constant instead of the settings the
/// relay sent: "the relay's words are under the switch".
#[test]
fn the_section_shows_the_media_settings_in_the_relays_words() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = admin_state();
    frame(&ctx, &theme, &mut state, Vec::new());
    let out = frame(&ctx, &theme, &mut state, Vec::new());
    for text in ["Who sees the Media page", "Public", "Members only", "Media space (MB)", "Each person's share (MB)", "Public visitors get previews only"] {
        assert!(shows(&out, text), "the section shows {text:?}");
    }
    assert!(shows(&out, "The relay's own words about members."), "the relay's words are under the switch");

    // Members only, picked: the working copy says so, and Save is offered.
    click(&ctx, &theme, &mut state, "Members only");
    assert_eq!(state.server_settings_draft.as_ref().map(|d| d.media_visibility.as_str()), Some("members"));
    assert!(shows(&frame(&ctx, &theme, &mut state, Vec::new()), "Unsaved changes."));
}

/// THE SAVE CARRIES THE MEDIA SETTINGS: the one `server_settings_update` frame every Save sends
/// holds the four, with the working copy's values.
///
/// Seen red 2026-10-11 with `media_per_person_mb` left out of `settings_update_frame`: "left:
/// Null / right: Number(64)".
#[test]
fn the_save_carries_the_media_settings() {
    let mut d = crate::relay::storage::ServerSettings::default();
    d.media_visibility = "members".into();
    d.media_budget_mb = 4096;
    d.media_per_person_mb = 64;
    d.media_public_previews_only = false;
    let f = super::super::settings_update_frame(&d);
    assert_eq!(f["type"], "server_settings_update");
    assert_eq!(f["media_visibility"], "members");
    assert_eq!(f["media_budget_mb"], 4096);
    assert_eq!(f["media_per_person_mb"], serde_json::json!(64));
    assert_eq!(f["media_public_previews_only"], false);
}

/// THE RELAY'S WORDS ARRIVE WITH THE SETTINGS: the line under the switch is read from the frame,
/// which the settings' own parse skips.
///
/// Seen red 2026-10-11 without the read in `on_server_settings_state`: "left: \"Members are
/// everyone signed in to this server. ...\" / right: \"Words from the relay.\"".
#[test]
fn the_relays_words_arrive_with_the_settings() {
    let mut state = GuiState::default();
    let mut s = serde_json::to_value(crate::relay::storage::ServerSettings::default()).unwrap();
    s["media_members_note"] = serde_json::json!("Words from the relay.");
    state.on_server_settings_state(&serde_json::json!({ "type": "server_settings_state", "settings": s }));
    assert_eq!(state.server_settings.unwrap().media_members_note, "Words from the relay.");
}

/// THE METER AND THE PRUNE LISTS: "12.4 GB of 50 GB used", the four lists, each row with Remove,
/// which asks first; the waiting list says what waits and by whom.
///
/// Seen red 2026-10-11 with the meter written in megabytes: "the meter reads in gigabytes"; and
/// with Remove sending at once: "Remove asks first" (with no identity in the fixture nothing
/// could be sent, so that failed only at "it asks"; the fixture now has one).
#[test]
fn the_meter_and_the_prune_lists() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = admin_state();
    state.media.admin.cats.clear(); // the categories' Remove buttons out of the way
    let out = frame(&ctx, &theme, &mut state, Vec::new());
    assert!(shows(&out, "12.4 GB of 50 GB used"), "the meter reads in gigabytes");
    for text in ["Largest", "Oldest", "Least viewed", "Waiting to be published", "Contact staff, slow spin by Ann, 900 MB, 0 views"] {
        assert!(shows(&out, text), "the section shows {text:?}");
    }
    click(&ctx, &theme, &mut state, "Remove");
    assert!(state.media.jobs.is_empty(), "Remove asks first");
    assert!(shows(&frame(&ctx, &theme, &mut state, Vec::new()), "Remove it?"), "it asks");
    assert!(matches!(state.media.admin.confirm, Some(api::RemoveTarget::Item(9))), "about the largest item");

    state.media.admin.prune_tab = 3;
    state.media.admin.confirm = None;
    let out = frame(&ctx, &theme, &mut state, Vec::new());
    assert!(shows(&out, "1700_clip.webm by Ben, 40 MB, deleted after a day unless published"), "the waiting list");
}

/// THE CATEGORIES AND THE SUGGESTIONS: General stays (no Remove), the others can go; a new one is
/// added to the working list; a suggestion names who made it, with Accept and Decline.
///
/// Seen red 2026-10-11 with Remove drawn on General too: "General has no Remove: one for Fire
/// flow arts only" (left: 2 / right: 1).
#[test]
fn the_categories_and_the_suggestions() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = admin_state();
    state.media.admin.usage = None; // the prune lists' Remove buttons out of the way
    let out = frame(&ctx, &theme, &mut state, Vec::new());
    assert!(find_text_in_shapes(&out.shapes, "stays").is_some_and(|f| f.text == "stays"), "General stays");
    let removes = find_text_in_shapes(&out.shapes, "Remove").map(|f| f.matches).unwrap_or(0);
    assert_eq!(removes, 1, "General has no Remove: one for Fire flow arts only");
    assert!(shows(&out, "\"Builds\", suggested by Ann") && shows(&out, "Accept") && shows(&out, "Decline"), "the suggestion names who made it");
    state.media.admin.new_cat = "Builds".into();
    click(&ctx, &theme, &mut state, "Add");
    assert_eq!(state.media.admin.cats.last().map(|c| (c.id, c.name.as_str())), Some((None, "Builds")), "a new category has no id until saved");
    assert!(state.media.admin.cats_dirty, "and Save is offered");
}
