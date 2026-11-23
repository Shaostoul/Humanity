//! Tests of the secret field (secret_field.rs, 2026-11-22). Each was seen to fail before it passed;
//! what it saw red is in its comment.

use super::*;

const SECRET: &str = "correct horse battery";

/// One headless pass; `draw` false leaves the field out (its window closed), `focused` false is the
/// app not being the window in front. The frame's output.
fn pass(ctx: &egui::Context, text: &mut String, draw: bool, focused: bool, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0))),
        focused,
        events,
        ..Default::default()
    };
    let theme = crate::gui::theme::load_theme();
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Down the page, as in a real window: at the screen's very top egui puts the eye's hint
            // over the eye itself, and a press then lands on the hint (seen 2026-11-22, a Tooltip
            // layer at the pointer).
            ui.add_space(150.0);
            if draw {
                SecretField::new("test_secret", text).desired_width(300.0).show(ui, &theme);
            }
        });
    })
}

/// Whether the secret's own words were drawn in the frame.
fn drawn(out: &egui::FullOutput) -> bool {
    crate::gui::screen_surface::find_text_in_shapes(&out.shapes, SECRET).is_some_and(|f| f.text == SECRET)
}

/// Press the Show / Hide button.
fn press_eye(ctx: &egui::Context, text: &mut String) {
    let id = toggle_id(egui::Id::new("test_secret").with("secret_field"));
    let at = ctx.read_response(id).expect("the Show / Hide button was drawn").rect.center();
    let m = egui::Modifiers::default();
    pass(ctx, text, true, true, vec![egui::Event::PointerMoved(at)]);
    let moved = (
        ctx.read_response(id).map(|r| (r.hovered(), r.contains_pointer())),
        ctx.layer_id_at(at),
        ctx.memory(|m| m.areas().visible_layer_ids().into_iter().map(|l| format!("{:?}", l.order)).collect::<Vec<_>>()),
    );
    pass(ctx, text, true, true, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    let pressed = ctx.read_response(id).map(|r| (r.is_pointer_button_down_on(), r.contains_pointer()));
    let down = ctx.input(|i| (i.pointer.any_down(), i.pointer.interact_pos()));
    pass(ctx, text, true, true, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    let state = ctx.read_response(id).map(|r| (r.rect, r.hovered(), r.clicked(), r.sense));
    LAST_PRESS.with(|l| *l.borrow_mut() = format!("pressed at {at:?}; moved {moved:?}; pressed {pressed:?} {down:?}; the eye after: {state:?}"));
}

thread_local! {
    /// What the last press saw, for a failure's words.
    static LAST_PRESS: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
}

/// Hidden at first; the eye shows the words; the eye again hides them; once shown, the field left
/// out for a pass (its window closed) comes back hidden, and so does a field the app left (another
/// window in front).
///
/// Seen red 2026-11-22 with `is_shown` ignoring the passes the field was not drawn: "the window
/// closed and opened again, and the secret is still shown".
#[test]
fn the_eye_shows_the_secret_and_closing_hides_it_again() {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let mut text = SECRET.to_string();
    let first = pass(&ctx, &mut text, true, true, vec![]);
    assert!(!drawn(&first), "the secret is shown before the eye was pressed");
    press_eye(&ctx, &mut text);
    let after = pass(&ctx, &mut text, true, true, vec![]);
    let field = egui::Id::new("test_secret").with("secret_field");
    let texts: Vec<String> = after
        .shapes
        .iter()
        .filter_map(|cs| if let egui::Shape::Text(t) = &cs.shape { Some(t.galley.text().to_string()) } else { None })
        .collect();
    assert!(
        drawn(&after),
        "pressing the eye does not show the secret (shown now: {}, kept: {:?}, drawn: {texts:?}; {})",
        is_shown(&ctx, field),
        ctx.data(|d| d.get_temp::<(bool, u64)>(shown_id(field))),
        LAST_PRESS.with(|l| l.borrow().clone())
    );
    press_eye(&ctx, &mut text);
    assert!(!drawn(&pass(&ctx, &mut text, true, true, vec![])), "pressing the eye again does not hide it");
    // Shown, then the window closes for a pass and opens again.
    press_eye(&ctx, &mut text);
    assert!(drawn(&pass(&ctx, &mut text, true, true, vec![])));
    pass(&ctx, &mut text, false, true, vec![]);
    assert!(!drawn(&pass(&ctx, &mut text, true, true, vec![])), "the window closed and opened again, and the secret is still shown");
    // Shown, then the person switches to another app.
    press_eye(&ctx, &mut text);
    assert!(drawn(&pass(&ctx, &mut text, true, true, vec![])));
    assert!(!drawn(&pass(&ctx, &mut text, true, false, vec![])), "another window in front, and the secret is still shown");
    assert_eq!(text, SECRET, "the field changed the secret");
}

/// The button is named Show (Hide once shown) for a screen reader.
///
/// Seen red 2026-11-22 with the button given no name: "the eye has no name: []".
#[test]
fn the_eye_is_named_for_a_screen_reader() {
    let ctx = egui::Context::default();
    let mut text = SECRET.to_string();
    pass(&ctx, &mut text, true, true, vec![]);
    // Walk the window with Tab, as a keyboard or screen reader does: the box, then the eye.
    let tab = || {
        vec![
            egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE },
            egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE },
        ]
    };
    let mut names: Vec<String> = Vec::new();
    for _ in 0..3 {
        let out = pass(&ctx, &mut text, true, true, tab());
        names.extend(out.platform_output.events.iter().filter_map(|e| match e {
            egui::output::OutputEvent::FocusGained(i) => i.label.clone(),
            _ => None,
        }));
    }
    assert!(names.iter().any(|n| n == humanity_core::i18n::t("secret_field.show")), "the eye has no name: {names:?}");
}

/// Every box in the passphrase window (each of its six forms: a new passphrase, unlocking,
/// changing it, a new PIN, unlocking with it, changing it) has the eye beside it, and none is the
/// plain hidden box any more. The operator, 2026-11-22: "I also have no way of previewing the
/// passphrase in the encrypt your private key window. It's just the dots with encrypt or skip."
///
/// Seen red 2026-11-22 against the window as it was: "SetNew: no Show / Hide eye beside
/// passphrase_input".
#[test]
fn every_box_in_the_passphrase_window_has_the_eye() {
    use crate::gui::PassphraseMode::*;
    let cases: [(crate::gui::PassphraseMode, &[&str]); 6] = [
        (SetNew, &["passphrase_input", "passphrase_confirm"]),
        (Unlock, &["passphrase_input"]),
        (Change, &["passphrase_old_input", "passphrase_input", "passphrase_confirm"]),
        (PinSetup, &["pin_input", "pin_confirm"]),
        (PinUnlock, &["pin_input"]),
        (PinChange, &["pin_old_input", "pin_input", "pin_confirm"]),
    ];
    let theme = crate::gui::theme::load_theme();
    for (mode, fields) in cases {
        let ctx = egui::Context::default();
        let mut state = crate::gui::GuiState::default();
        state.passphrase_needed = true;
        state.passphrase_mode = mode;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(412.0, 915.0))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| crate::gui::widgets::passphrase_modal::draw(ctx, &theme, &mut state));
        }
        for field in fields {
            let id = egui::Id::new(format!("passphrase_modal.{field}")).with("secret_field");
            assert!(ctx.read_response(toggle_id(id)).is_some(), "{mode:?}: no Show / Hide eye beside {field}");
        }
    }
    let src = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("src/gui/widgets/passphrase_modal.rs")).expect("the modal");
    assert!(!src.contains(".password(true)"), "a plain hidden box is left in the passphrase window");
}
