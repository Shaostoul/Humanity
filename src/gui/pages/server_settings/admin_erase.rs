//! Server Settings > Members: "Erase their data" (section 10i of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10): the action on a member's row, its confirm,
//! and the server's receipt. The rules (who is offered it, the typed-name check, the frame, the
//! receipt's words) are src/net/admin_erase.rs and are unit tested there; this file draws them and
//! sends the frame over the chat socket, the way the page's other admin actions (`unban`) do.
//!
//! Nothing is sent until the confirm's Erase is pressed with the member's exact name typed. A
//! refusal from the server arrives as a `private` notice, which the app shows in Chat; the receipt
//! (`admin_erase_done`) is shown above the member list.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::net::admin_erase::{self, EraseConfirm};

/// The Actions cell of a member's row: "Erase their data" where `may_offer` allows it, else
/// nothing. `viewer_role` is this app's role on the server. It only opens the confirm.
pub(crate) fn draw_row_action(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    viewer_role: &str,
    key: &str,
    name: &str,
    role: &str,
) {
    if !admin_erase::may_offer(viewer_role, &state.profile_public_key, key, role) {
        return;
    }
    let btn = widgets::Button::danger(admin_erase::ACTION_LABEL)
        .tooltip("Delete everything this server stores about this member. Asks you to type their name first.");
    super::row_button(ui, theme, 200.0, btn, || {
        state.admin_erase.confirm = Some(EraseConfirm { target: key.to_string(), name: name.to_string(), typed: String::new() });
    });
}

/// The confirm, while it is open: 10i's words on what the erase does and does not do, a field to
/// type their name, and Erase, enabled only when the typed name is exactly theirs. Drawn once per
/// frame by the page (it is a window over the page, not part of a row).
pub(crate) fn draw_confirm(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut c) = state.admin_erase.confirm.take() else { return };
    let mut open = true;
    let mut erase = false;
    let mut cancel = false;
    widgets::dialog(ctx, theme, "admin_erase_confirm", admin_erase::ACTION_LABEL, &mut open, |ui| {
        ui.set_min_width(420.0);
        ui.set_max_width(480.0);
        ui.label(RichText::new(admin_erase::confirm_words(&c.name)).color(theme.text_primary()));
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new(admin_erase::TYPE_LABEL).size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::TextEdit::singleline(&mut c.typed).hint_text("their name").desired_width(240.0));
        ui.add_space(theme.spacing_sm);
        // Read after the field, so the button follows what was typed this very frame.
        let ready = admin_erase::name_matches(&c.typed, &c.name);
        ui.horizontal(|ui| {
            let tip = if ready { "Erase their data from this server now." } else { "Type their name exactly to enable this." };
            if widgets::Button::danger("Erase").disabled(!ready).tooltip(tip).show(ui, theme) {
                erase = true;
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                cancel = true;
            }
        });
    });
    // The name is checked again here, so nothing but an exact match is ever sent.
    if erase && admin_erase::name_matches(&c.typed, &c.name) {
        state.admin_erase.status = if send(state, &c) {
            state.admin_erase.receipt = None;
            format!("Asked the server to erase {}'s data. Its receipt appears here; if it refuses, it says why in Chat.", c.name)
        } else {
            "Not connected to this server, so nothing was sent.".to_string()
        };
        return;
    }
    if open && !cancel {
        state.admin_erase.confirm = Some(c);
    }
}

/// Send the erase over the active server's chat socket. False when there is no open socket.
fn send(state: &GuiState, c: &EraseConfirm) -> bool {
    match state.ws_client.as_ref() {
        Some(client) if client.is_connected() => {
            client.send(&admin_erase::erase_frame(&c.target, &c.typed).to_string());
            true
        }
        _ => false,
    }
}

/// Above the member list: the line about an erase sent and not answered yet, and the last receipt
/// with what went, table by table, until it is dismissed.
pub(crate) fn draw_receipt(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !state.admin_erase.status.is_empty() {
        ui.label(
            RichText::new(&state.admin_erase.status)
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
    }
    let Some((partial, words)) = state.admin_erase.receipt.as_ref().map(|r| (r.partial, admin_erase::receipt_words(r))) else {
        return;
    };
    let kind = if partial { widgets::AlertKind::Warning } else { widgets::AlertKind::Success };
    widgets::alert(ui, theme, kind, &words);
    if widgets::Button::secondary("Dismiss").show(ui, theme) {
        state.admin_erase.receipt = None;
    }
    ui.add_space(theme.spacing_sm);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::screen_surface::find_text_in_shapes;
    use crate::gui::ChatUser;

    /// One headless frame of the Members tab with the confirm over it, as the page draws them.
    fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 900.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| super::super::draw_members_tab(ui, theme, state, true));
            draw_confirm(ctx, theme, state);
        })
    }

    /// Two quiet frames (a window sizes itself on the first), returning the second's output.
    fn settled(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
        frame(ctx, theme, state, Vec::new());
        frame(ctx, theme, state, Vec::new())
    }

    /// Click at `pos` the way a person does (move, press, release: egui's three frames).
    fn click_at(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, pos: egui::Pos2) {
        let m = egui::Modifiers::default();
        frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    }

    /// Click the drawn text that is exactly `label` (a button's label).
    fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, label: &str) {
        let out = settled(ctx, theme, state);
        let at = find_text_in_shapes(&out.shapes, label).unwrap_or_else(|| panic!("{label} is drawn")).rect.center();
        click_at(ctx, theme, state, at);
    }

    fn headless() -> (egui::Context, Theme) {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        (ctx, theme)
    }

    /// This app ("me", with `my_role`) on a server whose roster holds a member, an admin, a
    /// moderator and the owner.
    fn roster(my_role: &str) -> GuiState {
        let mut state = GuiState::default();
        state.profile_public_key = "me".to_string();
        let user = |name: &str, key: &str, role: &str| ChatUser { name: name.into(), public_key: key.into(), role: role.into(), status: "online".into() };
        state.chat_users = vec![
            user("Me", "me", my_role),
            user("Ann", "ann", "member"),
            user("Bob", "bob", "admin"),
            user("Cy", "cy", "mod"),
            user("Zed", "zed", "owner"),
        ];
        state
    }

    /// 10i's "who", as drawn: an admin or the owner sees "Erase their data" on the member's and
    /// the moderator's rows only (never their own, an admin's or the owner's), and a moderator or
    /// a member sees it nowhere.
    ///
    /// Seen red 2026-10-10 with `may_offer` also accepting a "mod" or "moderator" viewer: "a
    /// \"mod\" is offered the erase".
    #[test]
    fn the_member_list_offers_it_only_to_an_admin_and_never_on_an_admins_row() {
        let (ctx, theme) = headless();
        for viewer in ["admin", "owner"] {
            let mut state = roster(viewer);
            let out = settled(&ctx, &theme, &mut state);
            let found = find_text_in_shapes(&out.shapes, admin_erase::ACTION_LABEL).expect("the action is drawn for an admin");
            assert_eq!(found.matches, 2, "an {viewer} is offered it on {} rows", found.matches);
            let ann = find_text_in_shapes(&out.shapes, "Ann").expect("Ann's row").rect;
            assert!((found.rect.center().y - ann.center().y).abs() < 4.0, "the first offer is not on Ann's row");
        }
        for viewer in ["mod", "moderator", "member", ""] {
            let mut state = roster(viewer);
            let out = settled(&ctx, &theme, &mut state);
            assert!(find_text_in_shapes(&out.shapes, admin_erase::ACTION_LABEL).is_none(), "a {viewer:?} is offered the erase");
        }
    }

    /// The confirm, driven the way an admin does: the row's action opens it and sends nothing;
    /// their name typed in the wrong case leaves Erase dead; their exact name (spaces around it
    /// trimmed) enables it, and Erase sends exactly 10i's frame, once, and closes the confirm.
    ///
    /// Seen red 2026-10-10 twice: with the name check made case-blind (`eq_ignore_ascii_case` in
    /// `name_matches`), "a wrong name sent a frame"; and with the frame's `confirm_name` renamed
    /// `name`, the frames assertion (left: `"name": "Ann"`).
    #[test]
    fn erase_needs_their_exact_name_and_sends_the_protocols_frame() {
        let (ctx, theme) = headless();
        let mut state = roster("admin");
        let (client, sent) = crate::net::ws_client::WsClient::recording();
        state.ws_client = Some(client);

        click(&ctx, &theme, &mut state, admin_erase::ACTION_LABEL);
        let open = state.admin_erase.confirm.clone().expect("the action opens the confirm");
        assert_eq!((open.target.as_str(), open.name.as_str()), ("ann", "Ann"), "the confirm is for the row clicked");
        assert!(sent.try_iter().next().is_none(), "opening the confirm sent something");
        let out = settled(&ctx, &theme, &mut state);
        assert!(find_text_in_shapes(&out.shapes, &admin_erase::confirm_words("Ann")).is_some(), "10i's words are not shown");

        // The field, found by its hint while it is empty, and clicked again later by where it is.
        let field = find_text_in_shapes(&out.shapes, "their name").expect("the name field").rect.center();
        click_at(&ctx, &theme, &mut state, field);
        frame(&ctx, &theme, &mut state, vec![egui::Event::Text("ann".into())]);
        assert_eq!(state.admin_erase.confirm.as_ref().map(|c| c.typed.as_str()), Some("ann"), "typing did not reach the field");
        click(&ctx, &theme, &mut state, "Erase");
        assert!(sent.try_iter().next().is_none(), "a wrong name sent a frame");
        assert!(state.admin_erase.confirm.is_some(), "a wrong name closed the confirm");

        click_at(&ctx, &theme, &mut state, field);
        let select_all = egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
        frame(&ctx, &theme, &mut state, vec![select_all]);
        frame(&ctx, &theme, &mut state, vec![egui::Event::Text(" Ann ".into())]);
        click(&ctx, &theme, &mut state, "Erase");
        let frames: Vec<serde_json::Value> = sent.try_iter().map(|f| serde_json::from_str(&f).expect("a JSON frame")).collect();
        assert_eq!(frames, vec![serde_json::json!({ "type": "admin_erase", "target": "ann", "confirm_name": "Ann" })]);
        assert!(state.admin_erase.confirm.is_none(), "the confirm stayed open after Erase");
        assert!(state.admin_erase.status.contains("Asked the server"), "{}", state.admin_erase.status);
    }

    /// The server's `admin_erase_done`, handed over by the message pump's catch-all, shows its
    /// receipt above the member list, and the pump really does hand it over (read from the source,
    /// as engine/account_erase.rs reads its dispatch lines: the pump needs the whole engine, which
    /// no unit test can build).
    ///
    /// Seen red 2026-10-10 twice: with `draw_receipt` not called by the Members tab, "the receipt
    /// is not shown"; and with the pump's `admin_erase::on_frame` call deleted, "the message pump
    /// does not hand admin_erase_done over".
    #[test]
    fn the_receipt_is_shown_to_the_admin() {
        let (ctx, theme) = headless();
        let mut state = roster("admin");
        let done = serde_json::json!({
            "type": "admin_erase_done", "name": "Ann",
            "receipt": [["messages", 4], ["profile", 1], ["server_members", 1]], "partial": false
        });
        assert!(admin_erase::on_frame(&mut state.admin_erase, &done));
        let out = settled(&ctx, &theme, &mut state);
        let words = "Erased Ann's data from this server (messages: 4, profile: 1, server_members: 1). Anything on their own devices is untouched.";
        assert!(find_text_in_shapes(&out.shapes, words).is_some(), "the receipt is not shown");
        click(&ctx, &theme, &mut state, "Dismiss");
        assert!(state.admin_erase.receipt.is_none(), "Dismiss kept the receipt");

        let pump = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/engine/frame_ws_poll.rs"))
            .expect("read the message pump");
        assert!(
            pump.contains("crate::net::admin_erase::on_frame(&mut state.gui_state.admin_erase, &val)"),
            "the message pump does not hand admin_erase_done over"
        );
    }
}
