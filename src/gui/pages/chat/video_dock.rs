//! The chat videos' dock (rung 3 of docs/design/media-player.md's "Videos inside chat messages",
//! 2026-11-06). The operator: "When we scroll them off page can we add like an anchored
//! picture-in-picture mode that's stuck to the side of the chat it's at? It could be like a
//! collapsible tap that has minimize (keeps playing unless paused) and close (closes anchored
//! section.)" And, on the person leaving the chat: "Let's have it follow around app. Like how we
//! have the chat footer on nonchat pages."
//!
//! chat/inline_video.rs decides WHEN a video goes into the dock (`ChatVideos::end_frame`: the video
//! playing whose row left the view, the same player) and when it comes back (its row drawn on
//! screen again); this file draws the dock and answers its controls. The engine calls
//! [`draw_video_dock`] every frame, after the page and before the Full window and the end of the
//! chat videos' frame (lib.rs), for the app screen's set only: a wall screen's set never docks.
//!
//! WHERE IT STANDS:
//! - On the Chat page, inside the message list at its top right corner, under the channel header,
//!   a small margin in (the list's rectangle as the page noted it, `ChatVideos::note_chat_view`),
//!   so it never covers the header or the message box. Right: a row's name and words start at the
//!   left. Top: the message box and the newest lines are at the bottom, where the person reads and
//!   types, and a row left as the person reads on leaves at the top.
//! - On every other app page, at the right edge standing on the bottom strip (layer 30, the chat
//!   footer's, gui/layers.rs; docs/design/app-bottom-strip.md), placed when it arrives on a page or
//!   the window changes size: the lowest spot up that edge where it covers none of the page's
//!   controls as egui recorded them the frame before ([`page_controls`]). No such spot: the tab.
//!   Opened from the tab anyway, it stands at the bottom right, the person's choice, until they
//!   leave the page.
//! - Never in the first-person world (the video pauses and the dock hides, coming back with the
//!   next page: `ChatVideos::hide_dock_in_world`), never on a wall screen.
//!
//! HOW BIG: [`SHARE`] of the room's width, between [`MIN_WIDTH`] and [`MAX_WIDTH`] points and never
//! wider than the room; the picture at the video's shape; the parts round it measured (kept for the
//! next frame in egui's memory). A picture under [`MIN_PICTURE`] points tall in the room (a small
//! window, Font Size 200% on 1280x720) starts it as the tab. Points throughout, so the Font Size
//! scales it like everything else. Theme tokens only, words and hints by key (`chat_video_dock.*`).
//!
//! COST: nothing while no video is docked (a comparison and egui's memory note of the page). Docked,
//! what a row's player costs (inline_video.rs's measurement); minimized, the picture is not made an
//! image (`VideoView::poll_with`). Nothing kept across runs.

use super::*;
use humanity_core::i18n::{t, tf};

use crate::gui::layers::{self, Layer};
use crate::gui::widgets::hint::Hint;
use crate::gui::GuiPage;

use super::inline_video::{ChatVideos, Origin};

/// The dock's width as a share of its room (the message list, or the page).
pub(crate) const SHARE: f32 = 0.4;
/// The narrowest and widest the dock is drawn, in points.
pub(crate) const MIN_WIDTH: f32 = 240.0;
pub(crate) const MAX_WIDTH: f32 = 420.0;
/// The shortest picture worth a dock, in points: below it the dock starts as the tab.
pub(crate) const MIN_PICTURE: f32 = 72.0;
/// A widget bigger than this share of the page is a surface (a map, a list), not a control: the
/// dock may stand over it, as a picture-in-picture stands over a page.
const SURFACE_SHARE: f32 = 0.25;
/// The dock's Area and the tab's, by id.
pub(crate) const AREA_ID: &str = "chat_video_dock";
pub(crate) const TAB_ID: &str = "chat_video_dock_tab";
/// Where egui keeps the height of the dock's parts round its picture, measured each frame.
const PARTS_KEY: &str = "chat_video_dock_parts";
/// Where egui keeps the page the last pass drew (the pass number and the page).
const PAGE_KEY: &str = "chat_video_dock_page";

/// Where the dock stands on a page that is not the Chat page, decided as it arrived there (or the
/// window changed size), and kept while it stays: it is not moved as the page scrolls under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spot {
    page: GuiPage,
    room: egui::Rect,
    size: egui::Vec2,
    /// Its top left corner, or None: no free spot, so the tab.
    at: Option<egui::Pos2>,
}

/// What the dock's controls asked for this frame.
#[derive(Default)]
struct Asked {
    play: bool,
    mute: bool,
    minimize: bool,
    close: bool,
    go_to: bool,
    open: bool,
}

/// The place the Chat page shows now, as a docked video names where it came from: the server (the
/// one the active connection signs in to, as the history keeps a chat step) and the channel, and
/// the words "From #global on united-humanity.us".
pub(crate) fn origin_here(state: &GuiState) -> Origin {
    let server = norm_server_url(&state.connected_server_url);
    let channel = state.chat_active_channel.clone();
    let shown = if let Some(room) = commons_room_of(&channel) {
        format!("#{room}")
    } else if let Some(key) = channel.strip_prefix("dm:") {
        crate::engine::dm::dm_display_name(state, key)
    } else if let Some(gid) = channel.strip_prefix("p2pgroup:") {
        state.p2p_groups.iter().find(|g| g.group_id == gid).map_or_else(|| gid.to_string(), |g| g.name.clone())
    } else {
        format!("#{}", state.chat_channels.iter().find(|c| c.id == channel).map_or(channel.as_str(), |c| c.name.as_str()))
    };
    let label = if server.is_empty() {
        tf("chat_video_dock.from_here", &[("channel", &shown)])
    } else {
        tf("chat_video_dock.from", &[("channel", &shown), ("server", &server_display_name(&server))])
    };
    Origin { server, channel, label }
}

/// The dock's width in a room `room_width` points wide.
pub(crate) fn dock_width(room_width: f32) -> f32 {
    (room_width * SHARE).clamp(MIN_WIDTH, MAX_WIDTH).min(room_width)
}

/// The page between the top menu and the bottom strip: where the dock stands on a page that is not
/// the Chat page (the title screen has no menu).
fn page_room(ctx: &egui::Context, theme: &Theme, page: GuiPage) -> egui::Rect {
    let mut room = crate::gui::bottom_strip::room_above(ctx);
    if page != GuiPage::MainMenu {
        room.min.y += crate::gui::pages::escape_menu::rows_height(theme);
    }
    room
}

/// Note the page this pass draws, and say whether the pass before drew the same one (only then is
/// egui's record of the controls drawn the frame before this page's).
fn note_page(ctx: &egui::Context, page: GuiPage) -> bool {
    let now = ctx.cumulative_pass_nr();
    let id = egui::Id::new(PAGE_KEY);
    let before = ctx.data(|d| d.get_temp::<(u64, GuiPage)>(id));
    ctx.data_mut(|d| d.insert_temp(id, (now, page)));
    before.is_some_and(|(pass, p)| p == page && pass + 1 == now)
}

/// The page's own controls inside `room`, as egui recorded them the frame before: every widget on
/// the page's layers (the page's egui order, `Layer::Page`, the dock's own left out) that takes a press and the
/// keyboard (a button, a box, a slider, a link; a label that only selects is not one), smaller than
/// [`SURFACE_SHARE`] of the room (bigger ones are surfaces: a map, a list).
fn page_controls(ctx: &egui::Context, room: egui::Rect) -> Vec<egui::Rect> {
    let own = [egui::Id::new(AREA_ID), egui::Id::new(TAB_ID)];
    let surface = room.area() * SURFACE_SHARE;
    ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .layers()
            .filter(|(layer, _)| layer.order == Layer::Page.order() && !own.contains(&layer.id))
            .flat_map(|(_, widgets)| widgets.iter())
            .filter(|w| w.sense.senses_click() && w.sense.is_focusable())
            .map(|w| w.interact_rect)
            .filter(|r| r.is_positive() && r.area() <= surface && r.intersects(room))
            .collect()
    })
}

/// The lowest spot at the right edge of `room` where a dock of `size` covers none of `controls`:
/// from the bottom up, each time standing just above the highest control it would cover. None when
/// it does not fit the room or no spot is free.
pub(crate) fn free_spot(room: egui::Rect, size: egui::Vec2, controls: &[egui::Rect], gap: f32) -> Option<egui::Pos2> {
    if size.x > room.width() + 0.5 || size.y > room.height() + 0.5 {
        return None;
    }
    let x = room.right() - size.x;
    let mut y = room.bottom() - size.y;
    while y >= room.top() - 0.5 {
        let at = egui::Rect::from_min_size(egui::pos2(x, y), size);
        let highest = controls.iter().filter(|c| c.intersects(at)).map(|c| c.top()).fold(f32::INFINITY, f32::min);
        if highest == f32::INFINITY {
            return Some(egui::pos2(x, y));
        }
        y = highest - gap - size.y;
    }
    None
}

/// `words` no wider than `width` points in `font`: whole when they fit, else cut between characters
/// with an ellipsis after them.
fn fit_words(ctx: &egui::Context, words: &str, font: &egui::FontId, width: f32) -> String {
    let wide = |s: &str| ctx.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), egui::Color32::PLACEHOLDER).size().x);
    if wide(words) <= width {
        return words.to_string();
    }
    let chars: Vec<char> = words.chars().collect();
    let cut = |n: usize| format!("{}\u{2026}", chars[..n].iter().collect::<String>().trim_end());
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if wide(&cut(mid)) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

/// The dock, every frame (lib.rs, after the page): nothing while no video is docked; in the world
/// it hides and its video pauses; on the frame its row comes back on screen it is not drawn (the
/// video is in its message again); otherwise it stands where the module's notes say, open or as
/// the tab, and its controls are answered.
pub(crate) fn draw_video_dock(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let page = state.active_page;
    let same_page_before = note_page(ctx, page);
    let Some((url, origin)) = state.chat_video.dock.as_ref().map(|d| (d.url.clone(), d.origin.clone())) else { return };
    if page == GuiPage::None {
        state.chat_video.hide_dock_in_world();
        return;
    }
    if state.chat_video.dock_row_back() {
        return;
    }
    let gap = theme.spacing_sm;
    let videos = &mut state.chat_video;

    // The room: the message list on the Chat page (as it noted it this frame), else the page.
    let list = if page == GuiPage::Chat { videos.list_rect() } else { None };
    let (room, avoid) = match list {
        Some(r) => (r.shrink(gap), false),
        None => (page_room(ctx, theme, page).shrink(gap), true),
    };
    let width = dock_width(room.width());
    let parts = ctx.data(|d| d.get_temp::<f32>(egui::Id::new(PARTS_KEY))).unwrap_or(3.0 * (theme.button_height + theme.spacing_sm) + 2.0 * gap);
    let picture_h = (width / videos.aspect_of(&url)).min(room.height() - parts).max(0.0);
    let too_small = room.width() < MIN_WIDTH || picture_h < MIN_PICTURE;
    let size = egui::vec2(width, parts + picture_h);

    // Where it stands: on the Chat page the list's top right; elsewhere a free spot, decided as it
    // arrives on the page (the frame after, when egui's record of the controls is this page's).
    let free = if !avoid {
        (!too_small).then(|| egui::pos2(room.right() - width, room.top()))
    } else {
        let Some(dock) = videos.dock.as_mut() else { return };
        let fresh = dock.spot.is_some_and(|s| s.page == page && (s.room.min - room.min).length() < 0.5 && (s.room.max - room.max).length() < 0.5 && (s.size - size).length() < 0.5);
        if !fresh {
            if !same_page_before {
                // The frame before drew another page: kept playing, drawn from the next frame.
                videos.on_dock(&url, ctx, false);
                ctx.request_repaint();
                return;
            }
            let at = if too_small { None } else { free_spot(room, size, &page_controls(ctx, room), gap) };
            dock.spot = Some(Spot { page, room, size, at });
        }
        dock.spot.and_then(|s| s.at)
    };
    let Some(dock) = videos.dock.as_mut() else { return };
    if dock.opened_on.is_some_and(|p| p != page) {
        dock.opened_on = None; // opened over a page's controls: only on that page
    }
    let anchor = if avoid { egui::pos2(room.right() - width, (room.bottom() - size.y).max(room.top())) } else { egui::pos2(room.right() - width, room.top()) };
    let at = free.or_else(|| (dock.opened_on == Some(page)).then_some(anchor));
    let minimized = dock.minimized;

    let (playing, place) = videos.state_of(&url);
    let mut asked = Asked::default();
    match at.filter(|_| !minimized) {
        Some(at) => {
            videos.on_dock(&url, ctx, true);
            asked = draw_open(ctx, theme, videos, &url, &origin, at, width, picture_h, playing, place);
        }
        None => {
            videos.on_dock(&url, ctx, false);
            let corner = if avoid { (egui::Align2::RIGHT_BOTTOM, room.right_bottom()) } else { (egui::Align2::RIGHT_TOP, room.right_top()) };
            asked.open = draw_tab(ctx, theme, corner, playing);
        }
    }

    // What was asked, in order.
    if asked.play {
        if let Some(d) = videos.dock.as_ref() {
            let (name, ffmpeg) = (d.name.clone(), d.ffmpeg.clone());
            videos.press_play(&url, &name, &ffmpeg);
        }
    }
    if asked.mute {
        videos.muted = !videos.muted;
    }
    if let Some(dock) = videos.dock.as_mut() {
        if asked.minimize {
            dock.minimized = true;
        }
        if asked.open {
            dock.minimized = false;
            if free.is_none() {
                dock.opened_on = Some(page);
            }
        }
    }
    if asked.close {
        videos.close_dock(ctx);
    }
    if asked.go_to {
        go_to_message(state, url, origin);
    }
}

/// The dock open at `at`, `width` points wide with a picture `picture_h` tall: where the video came
/// from with Minimize and Close, the picture, Play or Pause with the time and Mute, and Go to
/// message. Its parts' height is measured for the next frame.
#[allow(clippy::too_many_arguments)]
fn draw_open(ctx: &egui::Context, theme: &Theme, videos: &ChatVideos, url: &str, origin: &Origin, at: egui::Pos2, width: f32, picture_h: f32, playing: bool, place: Option<(f64, f64)>) -> Asked {
    let mut asked = Asked::default();
    let gap = theme.spacing_sm;
    let small = widgets::ButtonSize::Small;
    let inner = (width - 2.0 * gap - 2.0).max(0.0);
    let shown = layers::area(ctx, egui::Id::new(AREA_ID), Layer::Footer).fixed_pos(at).interactable(true).show(ctx, |ui| {
        Frame::NONE
            .fill(theme.bg_card())
            .stroke(Stroke::new(1.0_f32, theme.border()))
            .corner_radius(CornerRadius::same(theme.border_radius.round().clamp(0.0, 255.0) as u8))
            .inner_margin(egui::Margin::same(gap.round().clamp(0.0, 127.0) as i8))
            .show(ui, |ui| {
                ui.set_width(inner);
                ui.spacing_mut().item_spacing = egui::vec2(theme.spacing_xs, theme.spacing_xs);
                // Where it came from, then Minimize and Close.
                ui.allocate_ui_with_layout(egui::vec2(inner, theme.button_height), Layout::right_to_left(Align::Center), |ui| {
                    if widgets::Button::ghost(t("common.close")).size(small).hint("chat_video_dock.close_hint").show(ui, theme) {
                        asked.close = true;
                    }
                    if widgets::Button::ghost(t("chat_video_dock.minimize")).size(small).hint("chat_video_dock.minimize_hint").show(ui, theme) {
                        asked.minimize = true;
                    }
                    let font = egui::FontId::proportional(theme.font_size_small);
                    let words = fit_words(ui.ctx(), &origin.label, &font, ui.available_width());
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        // The whole of it where the pointer rests, when it was cut to fit.
                        let _ = ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted())).on_hover_text(&origin.label);
                    });
                });
                // The picture: pressed, it plays or pauses, as in its row.
                if picture_h >= 1.0 {
                    let (rect, press) = ui.allocate_exact_size(egui::vec2(inner, picture_h), egui::Sense::click());
                    let press = press.hint("chat_video.picture_hint");
                    if ui.is_rect_visible(rect) {
                        super::inline_video::paint_picture(ui, theme, videos, url, rect, false);
                    }
                    asked.play |= press.clicked();
                }
                // Play or Pause, the time, Mute.
                ui.allocate_ui_with_layout(egui::vec2(inner, theme.button_height), Layout::left_to_right(Align::Center), |ui| {
                    let (label, hint) = if playing { (t("chat_video.pause"), "chat_video.pause_hint") } else { (t("chat_video.play"), "chat_video_dock.play_hint") };
                    if widgets::Button::secondary(label).size(small).hint(hint).show(ui, theme) {
                        asked.play = true;
                    }
                    let (at_s, length) = place.unwrap_or((0.0, 0.0));
                    let at_words = crate::gui::widgets::video_view::clock(at_s);
                    let time = if length > 0.0 { tf("chat_video.time", &[("at", &at_words), ("length", &crate::gui::widgets::video_view::clock(length))]) } else { at_words };
                    ui.label(RichText::new(time).size(theme.font_size_small).color(theme.text_muted()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::Button::ghost(if videos.muted { t("chat_video.unmute") } else { t("chat_video.mute") }).size(small).hint("chat_video.mute_hint").show(ui, theme) {
                            asked.mute = true;
                        }
                    });
                });
                // Go to message: the Chat page at its row, where the video comes back.
                if widgets::Button::secondary(t("chat_video_dock.go_to")).size(small).hint("chat_video_dock.go_to_hint").show(ui, theme) {
                    asked.go_to = true;
                }
            });
    });
    // The parts round the picture, for the next frame's size.
    let parts = (shown.response.rect.height() - picture_h.max(0.0)).max(0.0);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(PARTS_KEY), parts));
    asked
}

/// The slim tab on the dock's edge, its corner at `corner` (pivot and point): "Video playing" or
/// "Video paused". True when it was pressed (the dock opens).
fn draw_tab(ctx: &egui::Context, theme: &Theme, corner: (egui::Align2, egui::Pos2), playing: bool) -> bool {
    let mut pressed = false;
    layers::area(ctx, egui::Id::new(TAB_ID), Layer::Footer).pivot(corner.0).fixed_pos(corner.1).interactable(true).show(ctx, |ui| {
        Frame::NONE
            .fill(theme.bg_card())
            .stroke(Stroke::new(1.0_f32, theme.border()))
            .corner_radius(CornerRadius::same(theme.border_radius.round().clamp(0.0, 255.0) as u8))
            .inner_margin(egui::Margin::same(theme.spacing_xs.round().clamp(0.0, 127.0) as i8))
            .show(ui, |ui| {
                let words = if playing { t("chat_video_dock.tab_playing") } else { t("chat_video_dock.tab_paused") };
                if widgets::Button::secondary(words).size(widgets::ButtonSize::Small).hint("chat_video_dock.tab_hint").show(ui, theme) {
                    pressed = true;
                }
            });
    });
    pressed
}

/// Go to message: the Chat page at the docked video's channel (its server switched to first when it
/// is a saved one and not the one open, the history's own way back to a chat step), and its row
/// scrolled into view when it is drawn (`ChatVideos::go_to`, waited for up to `GO_TO_WAIT`), where
/// the video comes back from the dock.
fn go_to_message(state: &mut GuiState, url: String, origin: Origin) {
    state.chat_video.go_to = Some((url, std::time::Instant::now()));
    let here = state.active_page == GuiPage::Chat && state.chat_active_channel == origin.channel && (origin.server.is_empty() || norm_server_url(&state.connected_server_url) == origin.server);
    if !here {
        let at = crate::gui::history::At::Chat { server: origin.server, channel: origin.channel };
        state.go_to(crate::gui::history::Place { page: GuiPage::Chat, at }, crate::gui::How::From);
    }
}

#[cfg(test)]
#[path = "video_dock_tests.rs"]
pub(crate) mod tests;
