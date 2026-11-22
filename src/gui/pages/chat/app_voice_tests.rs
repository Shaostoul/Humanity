// Child of gui/pages/chat/rows.rs (#[path]): the app's own voice in the chat has a mark no row from
// the network can carry (the owner branch's seventh review, H1 (a), 2026-11-21). The whole Chat
// page is drawn headlessly and the SHAPES egui produced are read: what is measured is what was
// drawn, not what the rows were told.
//
// Seen red 2026-11-21 by mutation (scratchpad ownfix10-red-rust.log): with the App branch taken
// out of rows.rs (every line through `message_row`, as before), "the app line carries the app's
// mark" failed (0 diamonds); with the rows grouped by sender name alone (as before), "a frame named
// System after an app line still shows its own name" failed (no "System" drawn).

use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::{ChatMessage, GuiState, LineOrigin};

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
    let (shapes, accent) = page_with(vec![
        line("System", "", "an app line, the app's own", 1_000, LineOrigin::App),
        line("System", "", "a forged line from the network", 2_000, LineOrigin::Network),
    ]);
    let app = find_text_in_shapes(&shapes, "an app line, the app's own").expect("the app line is drawn");
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

/// No frame decoder can make a row the app's own: `LineOrigin::App` is written only by the app's
/// own lines (engine/friend_code.rs `system_line`, the chat page's "not sent" line), and the
/// files that turn frames into rows never name it.
#[test]
fn only_local_code_writes_the_apps_voice() {
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("src");
    let mut writers = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") && !path.to_string_lossy().ends_with("_tests.rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                if text.contains("LineOrigin::App") {
                    writers.push(path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    writers.sort();
    // rows.rs and hud.rs only READ it, to draw the mark; ui_snapshots.rs is the GPU snapshot's own
    // test data (snapshot_chat_app_voice).
    assert_eq!(writers, ["engine/friend_code.rs", "gui/pages/chat.rs", "gui/pages/chat/rows.rs", "gui/pages/hud.rs", "gui/ui_snapshots.rs"], "the files naming the app's voice");
    for decoder in ["engine/frame_ws_poll.rs", "engine/bg_connections.rs", "engine/net_route.rs", "engine/dm.rs", "gui/pages/chat/p2p_groups.rs"] {
        let text = std::fs::read_to_string(root.join(decoder)).unwrap();
        assert!(!text.contains("LineOrigin"), "{decoder} turns frames into rows and never sets their origin");
    }
}
