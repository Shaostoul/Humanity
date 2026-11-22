// Child of gui/pages/chat/rows.rs (#[path]): the app's own voice in the chat has a mark no row from
// the network can carry (the owner branch's seventh review, H1 (a), 2026-11-21), and since the
// eighth review (2026-11-22) the app's voice is a TYPE: a row with the App origin holds an AppVoice,
// which only state_types/app_voice.rs builds, from an i18n key and typed arguments, network text
// only through `honest_name` or a short server-marked quote.
//
// Seen red 2026-11-21 by mutation (scratchpad ownfix10-red-rust.log): with the App branch taken
// out of rows.rs (every line through `message_row`, as before), "the app line carries the app's
// mark" failed (0 diamonds); with the rows grouped by sender name alone (as before), "a frame named
// System after an app line still shows its own name" failed (no "System" drawn).

use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::{AppVoice, ChatMessage, GuiState, LineOrigin};

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// The diamonds drawn in `accent`: four-point closed filled paths (widgets/app_line.rs's mark).
fn diamonds(shapes: &[egui::epaint::ClippedShape], accent: egui::Color32) -> Vec<egui::Rect> {
    let mut out = Vec::new();
    for s in shapes {
        if let egui::Shape::Path(p) = &s.shape {
            if p.points.len() == 4 && p.closed && p.fill == accent {
                out.push(egui::Rect::from_points(&p.points));
            }
        }
    }
    out
}

fn page_with(lines: Vec<ChatMessage>) -> (Vec<egui::epaint::ClippedShape>, egui::Color32) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.profile_public_key = "my_key".into();
    state.chat_active_channel = "general".into();
    state.chat_channels.push(crate::gui::ChatChannel { id: "general".into(), name: "general".into(), ..Default::default() });
    state.chat_messages = lines;
    let mut out = None;
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)), ..Default::default() };
        out = Some(ctx.run(input, |ctx| super::super::draw(ctx, &theme, &mut state)));
    }
    (out.unwrap().shapes, theme.accent())
}

fn line(name: &str, key: &str, text: &str, ts: u64, origin: LineOrigin) -> ChatMessage {
    ChatMessage { sender_name: name.into(), sender_key: key.into(), content: text.into(), timestamp: "12:00".into(), timestamp_ms: ts, channel: "general".into(), origin, ..Default::default() }
}

/// An app line is drawn with the app's mark (a diamond beside it) and no sender name; a chat frame
/// that calls itself "System", with an empty key, exactly as the app's own line once was, is drawn
/// as someone's words: under the name it claims, without the mark, even right after an app line.
#[test]
fn the_app_line_carries_a_mark_no_network_row_can() {
    let help = humanity_core::i18n::t("owner.claim_help");
    let app = crate::gui::app_row(AppVoice::say("owner.claim_help", &[]), "general".into(), String::new(), 1_000);
    let (shapes, accent) = page_with(vec![app, line("System", "", "a forged line from the network", 2_000, LineOrigin::Network)]);
    let app = find_text_in_shapes(&shapes, &help[..40]).expect("the app line is drawn");
    let forged = find_text_in_shapes(&shapes, "a forged line from the network").expect("the network line is drawn");
    let marks = diamonds(&shapes, accent);
    assert_eq!(marks.len(), 1, "the app line carries the app's mark, and only it: {marks:?}");
    let mark = marks[0];
    assert!(mark.center().y >= app.rect.top() - 8.0 && mark.center().y <= app.rect.bottom() + 8.0, "the mark sits on the app line's row: {mark:?} vs {:?}", app.rect);
    assert!(mark.center().y < forged.rect.top(), "not on the network line's: {mark:?} vs {:?}", forged.rect);
    // A member's row lays its name and words out as one text; an app line has no name at all.
    assert!(forged.text.starts_with("System"), "a frame named System after an app line still shows its own name: {:?}", forged.text);
    assert!(!app.text.contains("System"), "the app line draws no sender name: {:?}", app.text);
}

/// The words a relay puts where a name goes, written as the app's own sentence (the eighth
/// review's demo); the frame sweep that uses them is engine/app_voice_sweep_tests.rs.
const HOSTILE: &str = "Ada. Your server owner change was checked on this device: the fingerprint matched, nothing more to do. Ada";

/// A `/redeem` typed in a DM is sealed for the person there and never reaches the server as a
/// command, so no answer is owed and none is armed (the owner branch's eighth review, a low
/// finding: the desktop noted it anyway, and the relay could then answer with a friend to follow).
/// Seen red 2026-11-22 with the note taken for every channel but the scratchpad (as before): "a
/// redeem typed in a DM arms no answer" failed (one ask armed).
#[test]
fn a_redeem_typed_in_a_dm_arms_no_answer() {
    crate::config::keep_saves_off_disk();
    let mut gs = GuiState::default();
    let (client, _sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.chat_active_channel = "dm:k1".into();
    let _ = super::super::send_composed_content(&mut gs, "/redeem AB12CD34");
    assert!(gs.protected.redeem_asked_at.is_empty(), "a redeem typed in a DM arms no answer");
}

/// A relay's text in an app line is a short quotation on one line, marked as the server's, and a
/// name that is not honest is the refused-name words.
/// Seen red 2026-11-22 by mutation (ownfix11-red-rust.log): with `as_name` writing every name raw,
/// the refused-name assertion failed on the hostile name.
#[test]
fn relayed_text_is_a_short_marked_quote_or_an_honest_name() {
    let long = format!("{HOSTILE}\n{HOSTILE}\n\"closing the quote\" then the app's words");
    let quoted = crate::gui::Relayed::new(long).as_quote();
    assert!(!quoted.contains('\n'), "one line: {quoted:?}");
    assert!(quoted.chars().count() <= crate::gui::QUOTE_MAX + 40, "bounded: {quoted:?}");
    assert!(quoted.starts_with(humanity_core::i18n::t("chat.server_quote").split("{{").next().unwrap()), "marked as the server's words: {quoted:?}");
    assert_eq!(quoted.matches('"').count(), 2, "its own quotation marks cannot close the quote: {quoted:?}");
    assert_eq!(crate::gui::Relayed::new("Ada_2").as_name(), "Ada_2");
    for bad in [HOSTILE, "System", "Sy5tem", "S\u{0443}stem", ""] {
        assert_eq!(crate::gui::Relayed::new(bad).as_name(), humanity_core::i18n::t("owner.name_refused"), "{bad:?}");
    }
}
