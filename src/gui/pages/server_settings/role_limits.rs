//! Server Settings > Roles: each role's upload limits, in a table under the roles table, beside
//! the per-file upload limit (docs/design/media-showcase.md, follow-ups B1 to B5, and the
//! operator's answers of 2026-10-11, numbers 2, 3 and 10: "make it configurable in the server
//! menu? Like the file size limits?").
//!
//! - Per file (MB): `max_upload_mb`, the existing limit on one file in a room or Shared Files (the
//!   same field the roles table edits; both edit the same working copy).
//! - Media file (MB): `max_media_file_mb`, the largest single Media file; 0 means up to the
//!   person's share of the Media space.
//! - Private file (MB): `max_private_file_mb`, the largest file in a DM or a P2P group.
//! - Private held (MB): `max_private_held_mb`, how much of those one person may keep on the
//!   server at once; 0 means no limit.
//! - A day (MB) and An hour (uploads): `max_upload_mb_per_day` and `max_uploads_per_hour`, over
//!   every upload route; 0 means no limit.
//! - Forwarded a day (MB): `max_forwarded_mb_per_day`, how much of one person's big files sent
//!   straight to another device may pass through this server's call relay in a UTC day (counted
//!   there, never stored; device-to-device big files, docs/design/media-showcase.md); 0 means no
//!   limit.
//!
//! Every role save sends the whole role, these six included (the relay keeps the stored value of
//! a limit a save leaves out, and gives a new role the careful defaults). The edits
//! live in the same working copy as the roles table's (`GuiState::roles_drafts`), so a Save in
//! either sends both.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::relay::storage::RoleDef;

/// The relay's own range for each of the six, as (lowest, highest): read through its
/// `with_limits_in_range`, the function a save goes through, so this editor never offers a number
/// the relay would change (the module that names them is the relay's own).
pub(crate) fn ranges() -> (RoleDef, RoleDef) {
    let at = |n: i64| {
        RoleDef {
            max_media_file_mb: n,
            max_private_file_mb: n,
            max_upload_mb_per_day: n,
            max_uploads_per_hour: n,
            max_private_held_mb: n,
            max_forwarded_mb_per_day: n,
            ..Default::default()
        }
        .with_limits_in_range()
    };
    (at(i64::MIN), at(i64::MAX))
}

/// The line under the private-file limits: why there is no switch for their type.
pub(crate) const PRIVATE_LINE: &str =
    "Files in private conversations are encrypted on the sender's device, so the server cannot see what kind of file they are; only their size can be limited.";

/// The line under the table about the forwarded day: what passes through and what does not.
pub(crate) const FORWARDED_LINE: &str =
    "Forwarded a day: files too big for the server go straight from one person's device to the other's. Unless the two are friends who turned on direct connections, they pass through this server's call relay so neither sees the other's address. Nothing is stored; this is how much one person may send that way in a UTC day.";

/// The table's columns, after the role's name.
pub(crate) const COLUMNS: [&str; 7] = [
    "Per file (MB)",
    "Media file (MB, 0 = up to the share)",
    "Private file (MB)",
    "Private held (MB, 0 = no limit)",
    "A day (MB, 0 = no limit)",
    "An hour (uploads, 0 = no limit)",
    "Forwarded a day (MB, 0 = no limit)",
];

/// The table. Returns a role whose Save was pressed, to send (`role_upsert`).
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) -> Option<RoleDef> {
    let roles = state.chat_roles.clone();
    if roles.is_empty() {
        return None;
    }
    ui.add_space(theme.spacing_md);
    widgets::subsection_label(ui, theme, "Upload limits per role");
    widgets::body_hint(
        ui,
        theme,
        "How large and how much each role may upload, counted by the server over every upload: chat, Shared Files, the Media page and files in private conversations. \
         A day is the UTC day, an hour the clock hour. Save sends the whole role.",
    );
    let mut save = None;
    let (lo, hi) = ranges();
    egui::Grid::new("server_role_upload_limits").num_columns(COLUMNS.len() + 2).spacing([theme.spacing_lg, theme.spacing_sm]).striped(true).show(ui, |ui| {
        let hdr = |ui: &mut egui::Ui, t: &str| {
            ui.label(RichText::new(t).size(theme.font_size_small).color(theme.text_secondary()).strong());
        };
        hdr(ui, "Role");
        for c in COLUMNS {
            hdr(ui, c);
        }
        hdr(ui, "");
        ui.end_row();
        for role in &roles {
            let draft = state.roles_drafts.entry(role.id.clone()).or_insert_with(|| role.clone());
            ui.label(RichText::new(&draft.label).size(theme.font_size_body).color(theme.text_primary()));
            super::int_input(ui, &mut draft.max_upload_mb, 1, 10_000);
            super::int_input(ui, &mut draft.max_media_file_mb, lo.max_media_file_mb, hi.max_media_file_mb);
            super::int_input(ui, &mut draft.max_private_file_mb, lo.max_private_file_mb, hi.max_private_file_mb);
            super::int_input(ui, &mut draft.max_private_held_mb, lo.max_private_held_mb, hi.max_private_held_mb);
            super::int_input(ui, &mut draft.max_upload_mb_per_day, lo.max_upload_mb_per_day, hi.max_upload_mb_per_day);
            super::int_input(ui, &mut draft.max_uploads_per_hour, lo.max_uploads_per_hour, hi.max_uploads_per_hour);
            super::int_input(ui, &mut draft.max_forwarded_mb_per_day, lo.max_forwarded_mb_per_day, hi.max_forwarded_mb_per_day);
            if widgets::Button::primary("Save").show(ui, theme) {
                save = Some(draft.clone());
            }
            ui.end_row();
        }
    });
    ui.label(RichText::new(PRIVATE_LINE).size(theme.font_size_small).color(theme.text_muted()));
    ui.label(RichText::new(FORWARDED_LINE).size(theme.font_size_small).color(theme.text_muted()));
    save
}

/// The limits a new role starts with from a preset tier (the relay's own defaults by tier,
/// storage/mod.rs and the follow-ups' B3 and B5): Admin none; Moderator 4096 MB and 200 uploads a
/// day and hour, 120 MB held; Verified 1024 MB and 60, 60 MB held; Unverified 100 MB and 20, 30 MB
/// held. A private file is 6 MB and a Media file up to the share for all. Forwarded a day: Admin
/// none, every other tier 2048 MB.
pub(crate) fn prefill(role: &mut RoleDef, tier: &str) {
    let (day, hour, held, forwarded) = match tier {
        "admin" => (0, 0, 0, 0),
        "mod" => (4096, 200, 120, 2048),
        "verified" => (1024, 60, 60, 2048),
        _ => (100, 20, 30, 2048),
    };
    role.max_media_file_mb = 0;
    role.max_private_file_mb = 6;
    role.max_upload_mb_per_day = day;
    role.max_uploads_per_hour = hour;
    role.max_private_held_mb = held;
    role.max_forwarded_mb_per_day = forwarded;
}

/// The `role_upsert` frame for `role`: the whole role, the six limits included.
pub(crate) fn upsert_frame(role: &RoleDef) -> serde_json::Value {
    serde_json::json!({ "type": "role_upsert", "role": role })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::screen_surface::find_text_in_shapes;

    fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> (egui::FullOutput, Option<RoleDef>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 900.0))),
            events,
            ..Default::default()
        };
        let mut saved = None;
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| saved = draw(ui, theme, state));
        });
        (out, saved)
    }

    /// THE SIX LIMITS ARE EDITED BESIDE THE PER-FILE LIMIT AND GO IN EVERY ROLE SAVE: the table
    /// shows each role's limits under their column, with the lines about private files and the
    /// forwarded day; an edit lands in the role's working copy, and its Save sends the whole role
    /// with all six, in the relay's own field names.
    ///
    /// Seen red 2026-10-11 with the Media file field drawn over a copy of the role instead of its
    /// working copy: "the edited Media limit is in the save" (left: Some(0) / right: Some(2048)).
    /// The forwarded day (device-to-device big files), seen red 2026-10-11 with its field drawn
    /// over `max_uploads_per_hour` instead: "a field shows 2048" (no field showed the role's
    /// forwarded day to type into).
    #[test]
    fn the_five_upload_limits_are_edited_and_saved_with_the_role() {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut state = GuiState::default();
        let mut verified = RoleDef { id: "verified".into(), label: "Verified".into(), ..Default::default() };
        prefill(&mut verified, "verified");
        state.chat_roles = vec![verified];

        let (out, _) = frame(&ctx, &theme, &mut state, Vec::new());
        for text in COLUMNS.iter().copied().chain(["Verified", PRIVATE_LINE, FORWARDED_LINE]) {
            assert!(find_text_in_shapes(&out.shapes, text).is_some(), "the table shows {text:?}");
        }

        // Type `text` into the one field showing exactly `shows`.
        let m = egui::Modifiers::default();
        let type_into = |state: &mut GuiState, shows: &str, text: &str| {
            let (out, _) = frame(&ctx, &theme, state, Vec::new());
            let found = find_text_in_shapes(&out.shapes, shows).unwrap_or_else(|| panic!("a field shows {shows}"));
            assert_eq!(found.text, shows);
            let at = found.rect.center();
            frame(&ctx, &theme, state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
            frame(&ctx, &theme, state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
            let all = egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
            frame(&ctx, &theme, state, vec![all, egui::Event::Text(text.into())]);
        };
        // The forwarded day: the only field that shows exactly "2048" (Verified's default).
        type_into(&mut state, "2048", "512");
        // An admin types 2048 into the Media file field: the only field that shows exactly "0".
        type_into(&mut state, "0", "2048");

        // Its Save sends the whole role.
        let (out, _) = frame(&ctx, &theme, &mut state, Vec::new());
        let save_at = find_text_in_shapes(&out.shapes, "Save").expect("a Save").rect.center();
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos: save_at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        let (_, saved) = frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos: save_at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
        let saved = saved.expect("Save hands the role over");
        let f = upsert_frame(&saved);
        assert_eq!(f["type"], "role_upsert");
        assert_eq!(f["role"]["max_media_file_mb"].as_i64(), Some(2048), "the edited Media limit is in the save");
        assert_eq!(f["role"]["max_forwarded_mb_per_day"].as_i64(), Some(512), "the edited forwarded day is in the save");
        for (field, want) in [("max_private_file_mb", 6), ("max_upload_mb_per_day", 1024), ("max_uploads_per_hour", 60), ("max_private_held_mb", 60)] {
            assert_eq!(f["role"][field].as_i64(), Some(want), "{field} goes with every save");
        }
        // The relay reads them back exactly (its own RoleDef).
        let back: RoleDef = serde_json::from_value(f["role"].clone()).unwrap();
        assert_eq!((back.max_media_file_mb, back.max_private_held_mb, back.max_forwarded_mb_per_day), (2048, 60, 512));
    }
}
