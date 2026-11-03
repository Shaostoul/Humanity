//! BUG-242: a room row looks the same under every server. The operator, 2026-11-03: "For some
//! reason the formatting changes depending on the active server. The active server everything is
//! really tight. The inactive server channels are loose." The active server's rooms and every
//! other server's rooms were drawn by two blocks of code in chat/left_panel.rs, each with its own
//! indent, row height and marks. The sidebar is drawn headlessly (no GPU, no network: the active
//! server's link is a test client that counts as connected, the other's rooms are its parked
//! connection's) and the rows are measured from what was drawn.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::{ChatChannel, ChatServer, ServerConnection};

const HOME: &str = "http://127.0.0.1:9";
const OTHER: &str = "http://127.0.0.1:19";

/// Two saved servers, `active` the one the app is on (its link up), the other's link parked and
/// its room list opened. Each has a room without voice and a room with voice, so a voice mark that
/// pushes a name sideways shows as two left edges.
fn two_servers(active: &str) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut s = GuiState::default();
    s.user_name = "Ada".into();
    s.profile_public_key = "me".into();
    s.private_key_bytes = Some(vec![7u8; 32]);
    let saved = |name: &str, url: &str| ChatServer { name: name.into(), channels: Vec::new(), voice_channels: Vec::new(), id: format!("srv_{url}"), url: url.into(), connected: true };
    s.chat_servers = vec![saved("Home server", HOME), saved("Second server", OTHER)];
    let room = |id: &str, voice: bool| ChatChannel { id: id.into(), name: id.into(), voice_enabled: voice, ..Default::default() };
    let rooms_of = |url: &str| if url == HOME { vec![room("alpha", false), room("beta", true)] } else { vec![room("delta", false), room("epsilon", true)] };
    let other = if active == HOME { OTHER } else { HOME };
    s.server_url = active.into();
    s.connected_server_url = active.into();
    s.ws_client = Some(crate::net::ws_client::WsClient::recording().0);
    s.chat_channels = rooms_of(active);
    s.chat_active_channel = "nowhere".into(); // no row is the open room, so no row is lit
    s.connections.push(ServerConnection { url: other.into(), display_url: other.into(), identified: true, channels: rooms_of(other), ..Default::default() });
    s.chat_server_folds.insert(norm_server_url(other), false);
    s
}

/// The sidebar drawn at a Font Size, in a window 420 points across, settled over three passes.
fn draw_at(state: &mut GuiState, percent: f32) -> (Vec<egui::epaint::ClippedShape>, f32) {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    state.settings.font_size = percent;
    let mut out = None;
    for _ in 0..3 {
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 900.0))), ..Default::default() };
        crate::gui::font_size::before_frame(&ctx, &mut input, &mut state.settings.font_size);
        out = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| left_panel::draw_left_panel(ui, &theme, state));
        }));
    }
    (out.expect("three passes ran").shapes, ctx.pixels_per_point())
}

/// One room row as drawn: where its name starts, its middle, and the height of the row's own
/// ground (the filled rect behind the name, as wide as the list; `None` when the row has none).
#[derive(Debug, Clone, Copy)]
struct Row {
    left: f32,
    mid: f32,
    height: Option<f32>,
}

fn row(shapes: &[egui::epaint::ClippedShape], name: &str) -> Row {
    let text = find_text_in_shapes(shapes, &format!("# {name}")).unwrap_or_else(|| panic!("the room row # {name} is drawn")).rect;
    let mid = text.center();
    let height = shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) if r.rect.contains(mid) && r.rect.width() > 300.0 && r.rect.height() < text.height() * 3.0 => Some(r.rect.height()),
            _ => None,
        })
        .reduce(f32::max);
    Row { left: text.left(), mid: mid.y, height }
}

/// A server's two rooms, measured: each name's left edge, each row's height, the step from its
/// header's middle to its first room's, and from the first room's to the second's (a row's height
/// and its gap to the next).
#[derive(Debug)]
struct Rooms {
    left_plain: f32,
    left_voice: f32,
    height_plain: Option<f32>,
    height_voice: Option<f32>,
    header_to_first: f32,
    first_to_second: f32,
}

fn rooms(shapes: &[egui::epaint::ClippedShape], header: &str, plain: &str, voice: &str) -> Rooms {
    let head = find_text_in_shapes(shapes, header).unwrap_or_else(|| panic!("the server row {header} is drawn")).rect.center().y;
    let (a, b) = (row(shapes, plain), row(shapes, voice));
    Rooms { left_plain: a.left, left_voice: b.left, height_plain: a.height, height_voice: b.height, header_to_first: a.mid - head, first_to_second: b.mid - a.mid }
}

fn same(a: f32, b: f32, ppp: f32) -> bool {
    (a - b).abs() <= 0.5 / ppp + 0.01
}

/// A ROOM ROW UNDER THE ACTIVE SERVER AND ONE UNDER ANOTHER SERVER ARE THE SAME ROW: the same
/// left edge (with or without a voice mark), the same height, the same step from the server's
/// name and to the next room, at 100% and 200% Font Size, whichever of the two servers is active.
///
/// Seen red 2026-11-03 on the two paths as they were (3a7e6eee5's left_panel.rs), at 100% with
/// Home server active: "the active server's rooms (Home server) and another server's (Second
/// server) are drawn differently at 100%: Rooms { left_plain: 16.4475, left_voice: 30.4475,
/// height_plain: Some(15.875), height_voice: Some(15.875), header_to_first: 19.9375,
/// first_to_second: 15.875 } against Rooms { left_plain: 32.0, left_voice: 32.0, height_plain:
/// None, height_voice: None, header_to_first: 23.0, first_to_second: 22.0 }". That is the
/// operator's two screenshots in numbers: the active rows start 8 points in from the panel's
/// edge, a voice mark pushes one name 14 points right, and they step 15.9 points (the theme's row
/// height, shorter than a line of the body text); the other server's are indented 24 points,
/// step 22 (a line of the body text) and have no ground of their own.
#[test]
fn a_room_row_is_the_same_under_the_active_server_and_another() {
    for percent in [100.0, 200.0] {
        for (active, (on, off)) in [(HOME, ("Home server", "Second server")), (OTHER, ("Second server", "Home server"))] {
            let mut state = two_servers(active);
            let (shapes, ppp) = draw_at(&mut state, percent);
            let home = rooms(&shapes, "Home server", "alpha", "beta");
            let second = rooms(&shapes, "Second server", "delta", "epsilon");
            let (a, o) = if active == HOME { (&home, &second) } else { (&second, &home) };
            let close = |x: f32, y: f32| same(x, y, ppp);
            let heights = |x: Option<f32>, y: Option<f32>| match (x, y) {
                (Some(x), Some(y)) => close(x, y),
                _ => false,
            };
            let alike = close(a.left_plain, o.left_plain)
                && close(a.left_voice, o.left_voice)
                && heights(a.height_plain, o.height_plain)
                && heights(a.height_voice, o.height_voice)
                && close(a.header_to_first, o.header_to_first)
                && close(a.first_to_second, o.first_to_second);
            assert!(alike, "the active server's rooms ({on}) and another server's ({off}) are drawn differently at {percent}%: {a:?} against {o:?}");
            assert!(close(a.left_plain, a.left_voice), "a voice mark moves a room's name at {percent}%: {a:?}");
            let h = a.height_plain.expect("a room row has its own ground");
            assert!(close(a.first_to_second, h), "rooms under {on} have a gap between them at {percent}%: {a:?}");
        }
    }
}
