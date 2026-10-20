//! A Media item posted in chat, shown as a card (operator, 2026-10-17: "can we make it when someone
//! clicks on the file in chat it brings up the full popup window for that item instead of just
//! having truncated version. I uploaded a set of files but, they're all part of the same thing.
//! I'd rather people know exactly what they're downloading instead of guessing ... a card in the
//! chat for the media thing(s). Like a simplified version of the full popup card. Card could start
//! as a collapsed line ... switchable between expanded and collapsed").
//!
//! A message holding the address of an item on the Files page of the server it came through
//! (`https://host/files/<id>`, or `/files/<id>`; add.rs `post_text` posts exactly that) shows,
//! under its words, a card for the item instead of the bare address:
//! - collapsed (as it starts): its type's badge, its title, how many files and how much in all,
//!   a triangle to open it out, and Open;
//! - expanded: also its cover or picture (under the Files page's own rules for pictures: the
//!   protected setup, content labels, a non-friend's picture waiting for a click), who put it there,
//!   its description and every file it holds, each with its type's badge and size.
//! The title, the picture and Open bring up the item's full window (item_view.rs `draw_window`)
//! over the chat, with a Download for each file: since 2026-10-19 the Files page's own two right
//! panes, the item to fit and its details, side by side, with Edit in place for its uploader. A
//! window rather than a jump to the Files page, so the person stays in the conversation they were
//! reading.
//!
//! The item is asked for once (a `Job::Card` through the Files page's own requests), signed when
//! this person is signed in so a member sees members-only items. The chat draws its messages while
//! they are borrowed from the app's state, so the card draws from a [`View`] made beforehand and
//! says what was pressed as [`Act`]s, applied at the start of the chat's next frame ([`apply`]).
//!
//! The website's twin: web/chat/chat-files-card.js.

use super::api::{self, Item};
use super::{item_view, tiles};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use egui::{Color32, RichText, Sense, Stroke};
use serde_json::Value;

/// One item as the chat's cards know it.
#[derive(Debug, Clone)]
pub enum CardState {
    Loading,
    Ready(Item),
    /// Not on the Files page, or not for this person to see.
    Missing,
    /// The request did not go through, with why.
    Failed(String),
}

/// What a card's buttons asked for, applied at the start of the chat's next frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Ask the server for item `id`.
    Fetch(i64),
    /// Open or close the card `key` (one message's card for one item).
    Toggle(String),
    /// Bring up item `id`'s full window.
    Open(i64),
    /// Ask for item `id`'s cover or picture.
    Picture(i64),
    /// Show the picture `key` that waits for a click (a non-friend's, or a labelled item's).
    Allow(String),
}

/// The words of the card.
pub const OPEN: &str = "Open";
pub const OPEN_TIP: &str = "See everything about it, with a Download for each file.";
pub const MORE_TIP: &str = "Show more";
pub const LESS_TIP: &str = "Show less";
pub const LOADING: &str = "Loading the item from the Files page...";
pub const SHOW_PICTURE: &str = "Show the picture";

/// "3 files, 4.1 MB" (one file: "1 file, 2.0 MB").
pub fn count_line(item: &Item) -> String {
    let n = item.file_count.max(1);
    let total = if item.total_bytes > 0 { item.total_bytes } else { item.size_bytes };
    format!("{n} {}, {}", if n == 1 { "file" } else { "files" }, api::size_words(total))
}

/// The Media items a message links to on the server it came through (`server_url`): each
/// `http(s)://<that server>/files/<number>` or `/files/<number>`, in order, once each. Another
/// website's `/files/` pages are not items of this server and stay plain links.
pub fn item_links(text: &str, server_url: &str) -> Vec<i64> {
    let mut out = Vec::new();
    for token in text.split_whitespace() {
        if let Some((_, id)) = own_link(token, server_url) {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// `token` as a Media item's address on the server at `server_url`: the address (the token without
/// the punctuation after it) and the item's number. The WHOLE token must be the address: a token
/// holding one inside other letters is not one (a review, 2026-10-17: taking `/files/5` out of
/// `htt/files/5ps://evil.example` by plain replacement made a live link of what was left). The
/// scheme matches in any case (a phone writes "Https://"); another website's `/files/` page is
/// not this server's item.
fn own_link<'a>(token: &'a str, server_url: &str) -> Option<(&'a str, i64)> {
    let link = token.trim_end_matches(|c: char| ".,;:!?)]}'\"".contains(c));
    let scheme = ["https://", "http://"].into_iter().find(|s| link.get(..s.len()).is_some_and(|p| p.eq_ignore_ascii_case(s)));
    let path = match scheme {
        Some(s) => {
            let rest = &link[s.len()..];
            let route = crate::gui::widgets::image_cache::route_image(&format!("{s}{rest}"), server_url);
            if !matches!(route, crate::gui::widgets::image_cache::ImageRoute::Load(_)) {
                return None;
            }
            &rest[rest.find('/')?..]
        }
        None => link,
    };
    let id = path.strip_prefix("/files/")?.trim_end_matches('/');
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())).then(|| id.parse().ok()).flatten().map(|n| (link, n))
}

/// `text` without the addresses of items `ids` on the server at `server_url` (the card shows
/// them): only a token that IS one goes, the punctuation after it staying. A line with nothing
/// taken out is kept exactly as it was (a code block keeps its indentation); a line with something
/// taken out keeps its indentation and its other words, one space apart, and goes if nothing is
/// left.
pub fn without_links(text: &str, server_url: &str, ids: &[i64]) -> String {
    if ids.is_empty() {
        return text.to_string();
    }
    let mut lines = Vec::new();
    for line in text.split('\n') {
        let mut kept = Vec::new();
        let mut took = false;
        for token in line.split_whitespace() {
            match own_link(token, server_url) {
                Some((link, id)) if ids.contains(&id) => {
                    took = true;
                    let after = &token[link.len()..];
                    if !after.is_empty() {
                        kept.push(after);
                    }
                }
                _ => kept.push(token),
            }
        }
        if !took {
            lines.push(line.to_string());
        } else if !kept.is_empty() {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            lines.push(format!("{indent}{}", kept.join(" ")));
        }
    }
    lines.join("\n")
}

/// The picture part of an expanded card.
#[derive(Debug, Clone)]
pub enum Picture {
    /// No picture: a file of another kind with no cover, or one too large to show here.
    None,
    /// Left out, with the line to say so (the protected setup).
    Hidden(String),
    /// Waiting for a click (a non-friend's picture, or a labelled item's), with what to say.
    Ask { line: String, key: String },
    /// Being fetched.
    Loading,
    Ready(egui::TextureId, egui::Vec2),
}

/// One card as it is drawn: made from the app's state before the messages are drawn.
#[derive(Debug, Clone)]
pub struct View {
    /// The card's key (its message and its item), which remembers whether it is open.
    pub key: String,
    pub id: i64,
    pub state: CardState,
    pub open: bool,
    pub picture: Picture,
}

/// The card's key for item `id` in a message sent at `ts` by `sender`.
pub fn key_of(sender: &str, ts: u64, id: i64) -> String {
    format!("{sender}:{ts}:{id}")
}

/// The views of the cards for `ids` in a message (sent by `sender` at `ts`), and the Acts they need
/// before they can be drawn (an item not asked for yet, a picture not fetched yet).
pub fn views(state: &GuiState, sender: &str, ts: u64, ids: &[i64]) -> (Vec<View>, Vec<Act>) {
    let mut acts = Vec::new();
    let views = ids
        .iter()
        .map(|&id| {
            let key = key_of(sender, ts, id);
            let open = state.files.cards_open.contains(&key);
            let card = state.files.cards.get(&id).cloned();
            if card.is_none() {
                acts.push(Act::Fetch(id));
            }
            let card = card.unwrap_or(CardState::Loading);
            let picture = match (&card, open) {
                (CardState::Ready(item), true) => picture_of(state, sender, item, &mut acts),
                _ => Picture::None,
            };
            View { key, id, state: card, open, picture }
        })
        .collect();
    (views, acts)
}

/// What an open card shows for `item`'s picture, under the Files page's rules (tiles.rs `show_of`):
/// its cover, else its own picture when it is one. The protected setup also leaves it out when the
/// person who posted the link (`sender`) is not a friend, whoever put the item there: a stranger
/// cannot put a picture in front of a protected person by linking someone else's item (the
/// website's card holds the same rule, web/chat/chat-files-card.js).
fn picture_of(state: &GuiState, sender: &str, item: &Item, acts: &mut Vec<Act>) -> Picture {
    let has_picture = item.cover.is_some() || item.kind == "picture";
    if !has_picture {
        return Picture::None;
    }
    if crate::engine::protected::hides_pictures_from(state, sender) {
        return Picture::Hidden(state.protected.preset.as_ref().map(|p| p.picture_hidden_line.clone()).unwrap_or_default());
    }
    let key = if item.cover.is_some() { tiles::cover_key(state, item) } else { tiles::cache_key(state, item) };
    match tiles::show_of(state, item) {
        tiles::Show::Hidden(line) => Picture::Hidden(line),
        tiles::Show::Label(names) => Picture::Ask { line: format!("Labelled: {}.", names.join(", ")), key: tiles::cache_key(state, item) },
        tiles::Show::Ask => Picture::Ask { line: format!("Pictures from {} show only when you choose.", item.by_name()), key: tiles::cache_key(state, item) },
        tiles::Show::TooBig => Picture::None,
        tiles::Show::Load => match state.image_cache.get_texture(&key) {
            Some(tex) => {
                let [w, h] = tex.size();
                Picture::Ready(tex.id(), egui::vec2(w as f32, h as f32))
            }
            None => {
                acts.push(Act::Picture(item.id));
                Picture::Loading
            }
        },
    }
}

/// Draw the cards of one message under it, indented like its other cards; what was pressed goes
/// to `acts`.
pub fn draw_all(ui: &mut egui::Ui, theme: &Theme, views: &[View], row_bg: Color32, acts: &mut Vec<Act>) {
    for view in views {
        egui::Frame::NONE.fill(row_bg).inner_margin(egui::Margin { left: 40, right: 8, top: 2, bottom: 4 }).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            egui::Frame::NONE
                .fill(theme.bg_card())
                .stroke(Stroke::new(1.0, theme.border()))
                .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| draw_card(ui, theme, view, acts));
        });
    }
}

fn draw_card(ui: &mut egui::Ui, theme: &Theme, view: &View, acts: &mut Vec<Act>) {
    // The chat draws its rows tight; a card spaces its parts as the other file cards do.
    ui.spacing_mut().item_spacing = egui::vec2(theme.spacing_sm, theme.spacing_xs);
    let small = |t: &str| RichText::new(t).size(theme.font_size_small);
    let item = match &view.state {
        CardState::Ready(item) => item,
        CardState::Loading => {
            ui.label(small(LOADING).color(theme.text_muted()));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
            return;
        }
        CardState::Missing => {
            ui.label(small(item_view::NOT_THERE).color(theme.text_muted()));
            return;
        }
        CardState::Failed(words) => {
            ui.label(small(&format!("This item from the Files page could not be loaded: {words}")).color(theme.danger()));
            return;
        }
    };
    // The line it starts as: a triangle to open it out, its type, its title (a click opens the
    // item's window), how many files and how much, and Open.
    ui.horizontal_wrapped(|ui| {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::click());
        let tri = rect.shrink(3.0);
        if view.open {
            widgets::icons::paint_triangle_down(ui.painter(), tri, theme.text_secondary());
        } else {
            widgets::icons::paint_triangle_right(ui.painter(), tri, theme.text_secondary());
        }
        if resp.on_hover_text(if view.open { LESS_TIP } else { MORE_TIP }).clicked() {
            acts.push(Act::Toggle(view.key.clone()));
        }
        let own_type = crate::net::file_risk::file_type(&super::open::shown_file_name(item, &item_view::files_of(item)[0]), None);
        widgets::file_badge::show(ui, theme, &own_type);
        let title = ui.add(egui::Label::new(RichText::new(item.display_title()).size(theme.font_size_body).color(theme.accent()).strong()).sense(Sense::click()));
        if title.on_hover_text(OPEN_TIP).clicked() {
            acts.push(Act::Open(item.id));
        }
        ui.label(small(&count_line(item)).color(theme.text_muted()));
        if widgets::Button::secondary(OPEN).size(widgets::ButtonSize::Small).tooltip(OPEN_TIP).show(ui, theme) {
            acts.push(Act::Open(item.id));
        }
    });
    if !view.open {
        return;
    }
    ui.add_space(theme.spacing_xs);
    match &view.picture {
        Picture::None => {}
        Picture::Hidden(line) => {
            ui.label(small(line).color(theme.text_muted()));
        }
        Picture::Ask { line, key } => {
            ui.horizontal_wrapped(|ui| {
                ui.label(small(line).color(theme.text_muted()));
                if widgets::Button::secondary(SHOW_PICTURE).size(widgets::ButtonSize::Small).show(ui, theme) {
                    acts.push(Act::Allow(key.clone()));
                }
            });
        }
        Picture::Loading => {
            ui.label(small("Loading the picture...").color(theme.text_muted()));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        Picture::Ready(tex, size) => {
            let width = ui.available_width().min(360.0);
            let shown = egui::vec2(width, (width * size.y / size.x.max(1.0)).min(360.0));
            let resp = ui.add(egui::Image::new((*tex, shown)).sense(Sense::click()));
            if resp.on_hover_text(OPEN_TIP).clicked() {
                acts.push(Act::Open(item.id));
            }
        }
    }
    ui.label(small(&item_view::by_line(item)).color(theme.text_secondary()));
    if !item.description.trim().is_empty() {
        ui.add(egui::Label::new(small(item.description.trim()).color(theme.text_primary())).wrap());
    }
    // Every file it holds, each with its type: what someone downloads, before they do.
    for f in item_view::files_of(item).iter().chain(item.cover.iter()) {
        let name = super::open::shown_file_name(item, f);
        let cover = item.cover.as_ref() == Some(f);
        ui.horizontal_wrapped(|ui| {
            widgets::file_badge::show(ui, theme, &crate::net::file_risk::file_type(&name, None));
            let line = if cover { format!("{name}, cover picture, {}", api::size_words(f.size_bytes)) } else { format!("{name}, {}", api::size_words(f.size_bytes)) };
            ui.label(small(&line).color(theme.text_primary()));
        });
    }
}

/// Do what the cards asked (at the start of the chat's frame, when the state is free to change).
pub fn apply(state: &mut GuiState) {
    let acts = std::mem::take(&mut state.files.card_acts);
    for act in acts {
        match act {
            Act::Fetch(id) => fetch(state, id),
            Act::Toggle(key) => {
                if !state.files.cards_open.remove(&key) {
                    state.files.cards_open.insert(key);
                }
            }
            // In a window of its own over the chat (item_view.rs `draw_window`): the person stays in
            // the conversation they were reading, with the item to fit and its details beside it.
            Act::Open(id) => match state.files.cards.get(&id) {
                Some(CardState::Ready(item)) => {
                    let item = item.clone();
                    item_view::open_in_window(state, &item);
                }
                _ => {
                    item_view::open_by_number(state, id);
                    if let Some(page) = state.files.item.as_mut().filter(|p| p.id == id) {
                        page.window = true;
                    }
                }
            },
            Act::Picture(id) => {
                if let Some(CardState::Ready(item)) = state.files.cards.get(&id).cloned() {
                    if item.cover.is_some() {
                        tiles::request_cover(state, &item);
                    } else {
                        tiles::request_picture(state, &item);
                    }
                }
            }
            Act::Allow(key) => state.image_cache.allow(&key),
        }
    }
}

/// Ask the server for item `id`, once (signed when this person is signed in, so a member is
/// answered members-only items too).
fn fetch(state: &mut GuiState, id: i64) {
    if state.files.cards.contains_key(&id) || super::server_here(state).is_empty() {
        return;
    }
    state.files.cards.insert(id, CardState::Loading);
    let filters = api::Filters { id: Some(id), ..Default::default() };
    let req = api::list_request(&filters, None, 1, &state.profile_public_key, super::signer_here(state).as_ref());
    super::spawn(state, super::Job::Card { id }, req);
}

/// The server answered item `id` for a card.
pub fn on_answer(state: &mut GuiState, id: i64, result: Result<Value, api::Refusal>) {
    let card = match result {
        Ok(v) => {
            let answer = api::parse_list(&v);
            // The server's names for its content labels, so a labelled card says "Flashing
            // lights", not "flashing", before the Files page has been opened (a review,
            // 2026-10-17). The Files page's own answer replaces them when it comes.
            if state.files.list.labels.is_empty() {
                state.files.list.labels = answer.labels.clone();
            }
            match answer.items.into_iter().find(|i| i.id == id) {
                Some(item) => CardState::Ready(item),
                None => CardState::Missing,
            }
        }
        Err(e) => CardState::Failed(e.words),
    };
    state.files.cards.insert(id, card);
}

#[cfg(test)]
#[path = "chat_card_tests.rs"]
mod tests;
