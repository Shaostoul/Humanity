//! An item's own page (Media round 5, lane 1D; docs/design/media-round-5-plan.md 3.9). A click on
//! any tile opens it, as a window over the Media page:
//!
//! - the picture (View full size opens the app's image viewer), the video with Play here, or the
//!   cover; a labelled item or a stranger's picture keeps the tiles' rules (tiles.rs `show_of`):
//!   its label, or a press, first;
//! - the title; "by Ann in Fire flow arts"; its labels; "Kept by the server"; "Members only";
//! - "4 picks, 12 views, 3 downloads";
//! - the whole description, as plain text with its line breaks;
//! - the licence by name, linked to its deed, with the server's own line for it, and a line when
//!   it is offered for the game;
//! - its files, the item's own first, each with its name, kind, size and Download (counted by the
//!   relay: open.rs `download_origin`), and Remove on an added one or the cover for the uploader
//!   and admins (never on the first: that is the item itself);
//! - Copy link (`https://<host>/media/<id>`, the page the relay serves, never `/media?id=`), Edit
//!   (the uploader and admins: edit.rs), Pick, the three Feature choices (admins), Keep (admins
//!   and moderators), Report and Remove item.
//!
//! Opening it counts a view (`POST /api/media/viewed`; the relay counts a person once a day).
//!
//! What lands here from the server ([`on_answer`]): one item read by its number ([`Job::Item`], a
//! list read with `id`), the item as an edit, a visibility change or a file taken off left it (each
//! answers `{ok, item}`), and a view counted (always `{ok: true}`). An item that changed is changed
//! wherever the page shows it: on its own page, in the grid and on the featured shelf
//! ([`take_item`]); and the page follows the grid's copy, where a Pick, Keep or Feature lands.

use egui::RichText;
use serde_json::Value;

use super::api::{self, Item, ItemFile, Refusal};
use super::api_items;
use super::open::{self, Opened};
use super::{kinds, signed_in_here, signer_or_say, spawn, spawn_with, tiles, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// An item's page, while it is open.
#[derive(Debug, Clone, Default)]
pub struct ItemPage {
    /// The item's number.
    pub id: i64,
    /// The item as the server last gave it; None until it has answered, or when it is not there
    /// for this person.
    pub item: Option<Item>,
    /// One line about the last thing done or refused on this page.
    pub status: String,
    /// The Edit window, while open (edit.rs).
    pub edit: Option<super::edit::EditForm>,
    /// A file's Remove, waiting for its confirmation.
    pub confirm_file: Option<ItemFile>,
    /// The item's own Remove, waiting for its confirmation.
    pub confirm_remove: bool,
    /// The address Copy link last copied, shown under it.
    pub copied: Option<String>,
}

/// What the page says when the item is not on the Media page, or this person may not see it.
pub const NOT_THERE: &str = "That item is not on the Media page, or you may not see it.";
/// What the page says once a file was taken off the item.
pub const FILE_REMOVED: &str = "Removed from the item.";
/// What the page says once an edit went.
pub const SAVED: &str = "Saved.";

/// Put `item` wherever the page shows it: its own page (when it is that item's), the grid and the
/// featured shelf.
pub(crate) fn take_item(state: &mut GuiState, item: Item) {
    for shown in state.media.list.items.iter_mut().chain(state.media.featured.iter_mut()).filter(|i| i.id == item.id) {
        *shown = item.clone();
    }
    if let Some(page) = state.media.item.as_mut().filter(|p| p.id == item.id) {
        if let Some(form) = page.edit.as_mut() {
            // The Edit window's files follow the item (a file added or taken off while it is open);
            // the words being edited are the person's and stay as they are.
            form.was.files = item.files.clone();
            form.was.cover = item.cover.clone();
        }
        page.item = Some(item);
    }
}

/// Say `words` on the item's page when it is item `id`'s, else on the Media page.
fn say(state: &mut GuiState, id: i64, words: String) {
    match state.media.item.as_mut().filter(|p| p.id == id) {
        Some(page) => page.status = words,
        None => state.media.status = words,
    }
}

/// Whether an edit of item `id` (the edit itself, or its visibility) is still on its way.
pub(crate) fn saving(state: &GuiState, id: i64) -> bool {
    state.media.jobs.iter().any(|(j, _)| *j == Job::Edit { id })
}

/// An answer for the item's page (pages/media.rs `on_done` hands these over).
pub(crate) fn on_answer(state: &mut GuiState, job: Job, result: Result<Value, Refusal>) {
    match (job, result) {
        (Job::Item { id }, Ok(v)) => {
            let found = api::parse_list(&v).items.into_iter().find(|i| i.id == id);
            let page = state.media.item.get_or_insert_with(|| ItemPage { id, ..Default::default() });
            if page.id != id {
                return;
            }
            page.status = if found.is_some() { String::new() } else { NOT_THERE.to_string() };
            match found {
                Some(item) => take_item(state, item),
                None => page.item = None,
            }
        }
        (Job::Edit { id }, Ok(v)) => match v.get("item").and_then(Item::from_json) {
            Some(item) => {
                take_item(state, item);
                // The Edit window closes once everything it sent is answered (the edit, and the
                // visibility when that changed too).
                // A refusal of the other half keeps the window open with its words, and nothing
                // is said to be saved.
                let refused = state.media.item.as_ref().filter(|p| p.id == id).and_then(|p| p.edit.as_ref()).is_some_and(|f| !f.problem.is_empty());
                if !saving(state, id) && !refused {
                    if let Some(page) = state.media.item.as_mut().filter(|p| p.id == id) {
                        page.edit = None;
                    }
                    say(state, id, SAVED.to_string());
                }
            }
            None => say(state, id, "The server's answer was not understood.".to_string()),
        },
        (Job::FileAdd { id } | Job::FileRemove { id, .. }, Ok(v)) => match v.get("item").and_then(Item::from_json) {
            Some(item) => {
                take_item(state, item);
                say(state, id, FILE_REMOVED.to_string());
            }
            None => say(state, id, "The server's answer was not understood.".to_string()),
        },
        (Job::Viewed { .. }, Ok(_)) => {}
        (Job::Viewed { id }, Err(e)) => log::warn!("Media: a view of item {id} was not counted: {e}"),
        (Job::Edit { id }, Err(e)) => {
            // The relay's own words, shown as they come, in the Edit window when it is open.
            match state.media.item.as_mut().filter(|p| p.id == id).and_then(|p| p.edit.as_mut()) {
                Some(form) => form.problem = e.words,
                None => say(state, id, e.words),
            }
        }
        (Job::Item { id } | Job::FileAdd { id } | Job::FileRemove { id, .. }, Err(e)) => say(state, id, e.words),
        (other, _) => log::warn!("Media: {other:?} is not an item page's answer"),
    }
}

/// Open `item`'s page: the page and its transfers, and a view counted.
pub(crate) fn open(state: &mut GuiState, item: &Item) {
    state.media.item = Some(ItemPage { id: item.id, item: Some(item.clone()), ..Default::default() });
    state.media.open = Some(Opened::new(item.clone()));
    count_view(state, item);
}

/// Count a view of `item` (`POST /api/media/viewed`, unsigned). The session's upload token goes in
/// its header only for a members-only item, which only a member's view counts; a public one is
/// counted by the network address, so nothing more about the person is sent.
fn count_view(state: &mut GuiState, item: &Item) {
    if super::server_here(state).is_empty() {
        return;
    }
    let base = super::base_of(state);
    let token = if item.members_only() { super::token_here(state) } else { None };
    let req = api_items::viewed_request(item.id);
    spawn_with(state, Job::Viewed { id: item.id }, move || api_items::send_with_header(&base, &req, token.as_deref().map(|t| (api::TOKEN_HEADER, t))));
}

/// The item's files, the item's own first: as the server lists them, or, from a server that lists
/// none, the item's own file alone.
pub(crate) fn files_of(item: &Item) -> Vec<ItemFile> {
    if !item.files.is_empty() {
        return item.files.clone();
    }
    vec![ItemFile { file: 0, filename: item.stored.clone(), original_name: item.original_name.clone(), kind: item.kind.clone(), size_bytes: item.size_bytes, url: item.url.clone() }]
}

/// Whether this person put `item` on the Media page.
pub(crate) fn own(state: &GuiState, item: &Item) -> bool {
    !item.uploader_key.is_empty() && item.uploader_key == state.profile_public_key
}

/// Whether this person may edit `item`: its uploader or an admin, signed in to this server
/// (decision 1: moderators edit nothing; they still Keep).
pub(crate) fn may_edit(state: &GuiState, item: &Item) -> bool {
    signed_in_here(state) && (own(state, item) || super::is_admin(state))
}

/// "4 picks, 12 views, 3 downloads", each in the singular when it is one.
pub(crate) fn counts_line(item: &Item) -> String {
    let n = |count: i64, one: &str| if count == 1 { format!("1 {one}") } else { format!("{count} {one}s") };
    format!("{}, {}, {}", n(item.picks, "pick"), n(item.views, "view"), n(item.downloads, "download"))
}

/// "by Ann in Fire flow arts" (the category left out when the server named none).
pub(crate) fn by_line(item: &Item) -> String {
    if item.category_name.trim().is_empty() {
        format!("by {}", item.by_name())
    } else {
        format!("by {} in {}", item.by_name(), item.category_name.trim())
    }
}

/// What the page's buttons asked for, done once the window is drawn.
#[derive(Default)]
struct Pressed {
    play: bool,
    download: Option<ItemFile>,
    view_full: bool,
    copy_link: bool,
    /// The address Copy link put on the clipboard.
    copied: Option<String>,
    edit: bool,
    pick: bool,
    keep: bool,
    report: bool,
    remove: bool,
}

/// The item's page and its windows (the Edit window, a file picker, the confirmations), while one
/// is open. Drawn after the Media page (open.rs `draw`).
pub(crate) fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut page) = state.media.item.take() else {
        state.media.open = None;
        return;
    };
    // The page follows the grid's copy of the item, where a Pick, Keep or Feature lands.
    if let Some(fresh) = state.media.list.items.iter().chain(state.media.featured.iter()).find(|i| i.id == page.id) {
        if page.item.as_ref() != Some(fresh) {
            page.item = Some(fresh.clone());
        }
    }
    let mut opened = state.media.open.take().filter(|o| o.item.id == page.id);
    if let Some(item) = page.item.clone() {
        let o = opened.get_or_insert_with(|| Opened::new(item.clone()));
        o.item = item;
        open::poll(ctx, o);
    }

    let mut open = true;
    let mut pressed = Pressed::default();
    let title = page.item.as_ref().map(|i| i.display_title()).unwrap_or_else(|| "Media item".to_string());
    widgets::dialog_scrolling(ctx, theme, "media_item_page", &title, &mut open, |ui| {
        ui.set_max_width(720.0);
        match (page.item.clone(), opened.as_mut()) {
            (Some(item), Some(o)) => draw_page(ui, theme, state, &page, o, &item, &mut pressed),
            _ => {
                let words = if page.status.is_empty() { "Loading..." } else { page.status.as_str() };
                ui.label(RichText::new(words).size(theme.font_size_body).color(theme.text_muted()));
            }
        }
    });

    if let (Some(item), Some(o)) = (page.item.clone(), opened.as_mut()) {
        act(state, &mut page, o, &item, pressed);
    }
    super::edit::draw(ctx, theme, state, &mut page);
    let removed = draw_confirmations(ctx, theme, state, &mut page);

    // Closed (its X, a click beside it, or the item removed): the page and its transfers go, and
    // a video's temporary copy with them (`Drop for Opened`).
    if open && !removed {
        state.media.item = Some(page);
        state.media.open = opened;
    }
}

/// Everything inside the page's window.
fn draw_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, page: &ItemPage, opened: &mut Opened, item: &Item, pressed: &mut Pressed) {
    draw_face(ui, theme, state, opened, item, pressed);
    ui.add_space(theme.spacing_sm);

    ui.label(RichText::new(item.display_title()).size(theme.font_size_heading).color(theme.text_primary()).strong());
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(by_line(item)).size(theme.font_size_small).color(theme.text_secondary()));
        for name in tiles::label_names(state, item) {
            ui.label(RichText::new(name).size(theme.font_size_small).color(theme.warning()));
        }
        if item.kept {
            ui.label(RichText::new(tiles::KEPT_BADGE).size(theme.font_size_small).color(theme.success()));
        }
        if item.members_only() {
            ui.label(RichText::new("Members only").size(theme.font_size_small).color(theme.info()));
        }
        if item.featured.is_on() {
            ui.label(RichText::new(super::filters::feature_words(item.featured)).size(theme.font_size_small).color(theme.accent()));
        }
    });
    ui.label(RichText::new(counts_line(item)).size(theme.font_size_small).color(theme.text_muted()));
    if !item.description.trim().is_empty() {
        ui.add_space(theme.spacing_xs);
        // The whole description, as plain text: its own line breaks are kept.
        ui.label(RichText::new(item.description.trim()).size(theme.font_size_body).color(theme.text_primary()));
    }

    ui.add_space(theme.spacing_sm);
    draw_licence(ui, theme, state, item);

    ui.add_space(theme.spacing_sm);
    draw_files(ui, theme, state, opened, item, pressed);

    ui.add_space(theme.spacing_sm);
    draw_actions(ui, theme, state, item, pressed);
    if let Some(link) = &page.copied {
        ui.label(RichText::new(format!("Copied: {link}")).size(theme.font_size_small).color(theme.success()));
    }
    if !page.status.is_empty() {
        ui.label(RichText::new(&page.status).size(theme.font_size_small).color(theme.text_secondary()));
    }
}

/// The picture, the video, or the cover, under the tiles' rules (a label first, then a press for a
/// stranger's picture).
fn draw_face(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, opened: &mut Opened, item: &Item, pressed: &mut Pressed) {
    let show = tiles::show_of(state, item);
    let key = tiles::cache_key(state, item);
    match &show {
        tiles::Show::Hidden(line) => {
            widgets::body_hint(ui, theme, line);
            return;
        }
        tiles::Show::Label(names) => {
            ui.label(RichText::new(format!("Labelled: {}.", names.join(", "))).size(theme.font_size_body).color(theme.warning()));
            if widgets::Button::secondary("Show it").tooltip("Show what the label covers, on this item only.").show(ui, theme) {
                state.image_cache.allow(&key);
            }
            return;
        }
        tiles::Show::Ask => {
            ui.label(RichText::new(format!("{} is not your friend. Pictures show only when you choose.", item.by_name())).size(theme.font_size_body).color(theme.text_primary()));
            if widgets::Button::secondary("Show it").show(ui, theme) {
                state.image_cache.allow(&key);
            }
            return;
        }
        tiles::Show::TooBig | tiles::Show::Load => {}
    }
    if item.kind == "picture" {
        if tiles::too_big(item) {
            widgets::body_hint(ui, theme, &format!("This picture is {}, larger than this app shows in place. Download it to see it.", api::size_words(item.size_bytes)));
        } else {
            tiles::request_picture(state, item);
            if picture(ui, theme, state, &key) {
                pressed.view_full = widgets::Button::secondary("View full size").show(ui, theme);
            }
        }
        return;
    }
    // A video, a model or a file: its cover when it has one, else its kind's name.
    match item.cover.as_ref() {
        Some(_) if !tiles::cover_too_big(item) => {
            tiles::request_cover(state, item);
            picture(ui, theme, state, &tiles::cover_key(state, item));
        }
        _ => {
            ui.label(RichText::new(kinds::one_name(&item.kind)).size(theme.font_size_small).color(theme.text_muted()));
        }
    }
    if item.kind == "video" {
        pressed.play = open::draw_player(ui, theme, opened);
    }
}

/// Draw the picture the image cache holds under `key`, fitted to the window (at most 640 wide); a
/// line while it loads or when it could not. True when it was drawn.
fn picture(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, key: &str) -> bool {
    use crate::gui::widgets::image_cache::ImageStatus;
    if let Some(tex) = state.image_cache.get_texture(key) {
        let [w, h] = tex.size();
        let aspect = w.max(1) as f32 / h.max(1) as f32;
        let width = ui.available_width().min(640.0);
        ui.add(egui::Image::new((tex.id(), egui::vec2(width, width / aspect))));
        return true;
    }
    match state.image_cache.status(key) {
        ImageStatus::Failed(_) => {
            ui.label(RichText::new(tiles::COULD_NOT_LOAD).size(theme.font_size_small).color(theme.danger()));
            if widgets::Button::ghost("Try again").show(ui, theme) {
                state.image_cache.retry(key);
            }
        }
        _ => {
            ui.label(RichText::new("Loading...").size(theme.font_size_small).color(theme.text_muted()));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
    false
}

/// The licence by name (linked to its deed, with the server's own line for it), and a line when
/// the item is offered for the game. A licence this server no longer offers is named by its id.
fn draw_licence(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, item: &Item) {
    let licences = &state.media.list.licences;
    let id = if item.licence.trim().is_empty() { licences.default.clone() } else { item.licence.clone() };
    let known = licences.get(&id);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Licence:").size(theme.font_size_small).color(theme.text_secondary()));
        let name = known.map(|l| l.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
        match known.map(|l| l.url.as_str()).filter(|u| u.starts_with("https://")) {
            Some(url) => {
                ui.hyperlink_to(RichText::new(&name).size(theme.font_size_small), url).on_hover_text(url);
            }
            None => {
                ui.label(RichText::new(&name).size(theme.font_size_small).color(theme.text_primary()));
            }
        }
    });
    if let Some(line) = known.map(|l| l.line.as_str()).filter(|l| !l.trim().is_empty()) {
        widgets::body_hint(ui, theme, line);
    }
    if item.project_use {
        let line = if licences.game_line.trim().is_empty() { OFFERED_FOR_GAME.to_string() } else { licences.game_line.clone() };
        ui.label(RichText::new(line).size(theme.font_size_small).color(theme.success()));
    }
}

/// The line on an item offered for the game, when the server words it no other way.
/// Plain status words only: what the offer allows is the licence's, in the server's own words
/// (the line above it), never a sentence written here.
pub const OFFERED_FOR_GAME: &str = "Offered for the game.";

/// The item's files, the item's own first, each with its name, kind, size, Download and, for an
/// added file or the cover, Remove for the uploader and admins.
fn draw_files(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, opened: &Opened, item: &Item, pressed: &mut Pressed) {
    let files = files_of(item);
    widgets::subsection_label(ui, theme, if files.len() == 1 { "File" } else { "Files" });
    let small = widgets::ButtonSize::Small;
    let rows = files.iter().map(|f| (f, false)).chain(item.cover.iter().map(|c| (c, true)));
    for (f, cover) in rows {
        ui.horizontal_wrapped(|ui| {
            let name = if f.file == 0 { open::download_name(item) } else { open::file_download_name(item, f) };
            let what = if cover { "cover picture".to_string() } else { kinds::one_name(&f.kind).to_lowercase() };
            ui.label(RichText::new(format!("{name}, {what}, {}", api::size_words(f.size_bytes))).size(theme.font_size_small).color(theme.text_primary()));
            if opened.downloading(f.file) {
                ui.label(RichText::new("Downloading...").size(theme.font_size_small).color(theme.text_muted()));
            } else if widgets::Button::secondary("Download").size(small).tooltip("Save it in your downloads folder.").show(ui, theme) {
                pressed.download = Some(f.clone());
            }
        });
        if let Some(words) = opened.saved_words(f.file) {
            let color = if words.starts_with("Saved") { theme.success() } else { theme.danger() };
            ui.label(RichText::new(words).size(theme.font_size_small).color(color));
        }
    }
    if may_edit(state, item) && (files.len() > 1 || item.cover.is_some()) {
        widgets::body_hint(ui, theme, "Add or remove files and the cover in Edit.");
    }
}

/// The page's buttons: Copy link for anyone; Edit for the uploader and admins; then, signed in,
/// Pick (not on your own), Feature's three choices (admins), Keep (admins and moderators), Report
/// (not on your own) and Remove item (your own, or any as an admin).
fn draw_actions(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: &Item, pressed: &mut Pressed) {
    let signed_in = signed_in_here(state);
    let mine = own(state, item);
    let admin = super::is_admin(state);
    ui.horizontal_wrapped(|ui| {
        pressed.copy_link = widgets::Button::secondary(COPY_LINK).tooltip("Copy this item's own address, to share it.").show(ui, theme);
        if may_edit(state, item) {
            pressed.edit = widgets::Button::primary("Edit").tooltip("Change its words, licence, files and cover.").show(ui, theme);
        }
        if signed_in && !mine {
            let label = if item.picked_by_me { "Unpick" } else { "Pick" };
            pressed.pick = widgets::Button::secondary(label).show(ui, theme);
        }
        if signed_in && super::may_keep(state) {
            let label = if item.kept { tiles::STOP_KEEPING } else { tiles::KEEP };
            let tip = if item.kept { tiles::STOP_KEEPING_TIP } else { tiles::KEEP_TIP };
            pressed.keep = widgets::Button::secondary(label).tooltip(tip).show(ui, theme);
        }
        if signed_in && !mine {
            pressed.report = widgets::Button::ghost("Report").tooltip(tiles::REPORT_TIP).show(ui, theme);
        }
        // The confirmation is drawn over the page (draw_confirmations).
        pressed.remove = signed_in && (mine || admin) && widgets::Button::danger("Remove item").show(ui, theme);
    });
    if pressed.copy_link {
        // Copied here, where the frame's context is: the item's own page on this server.
        let link = api_items::item_link(&super::base_of(state), item.id);
        ui.ctx().copy_text(link.clone());
        pressed.copied = Some(link);
    }
    if signed_in && admin {
        // The admins' three choices, the tile's own control (filters.rs, lane 1C), in the words
        // both apps use; it sends the change itself.
        widgets::form_row(ui, theme, "Feature", |ui| {
            super::filters::draw_feature_choice(ui, theme, state, item);
        });
    }
}

/// Copy link's label.
pub const COPY_LINK: &str = "Copy link";

/// Do what the page's buttons asked, once the window is drawn.
fn act(state: &mut GuiState, page: &mut ItemPage, opened: &mut Opened, item: &Item, pressed: Pressed) {
    if pressed.remove {
        page.confirm_remove = true;
    }
    if pressed.copied.is_some() {
        page.copied = pressed.copied;
    }
    if pressed.play {
        open::start_play(state, opened);
    }
    if let Some(f) = pressed.download {
        open::start_download(state, opened, &f);
    }
    if pressed.view_full {
        state.image_viewer_url = Some(tiles::cache_key(state, item));
    }
    if pressed.edit {
        page.edit = Some(super::edit::EditForm::of(item));
    }
    if pressed.report {
        crate::engine::report::open_for_media(state, item);
    }
    if !(pressed.pick || pressed.keep) {
        return;
    }
    let Some(signer) = signer_or_say(state) else { return };
    let r = match item.reference() {
        Ok(r) => r,
        Err(words) => {
            page.status = words;
            return;
        }
    };
    if pressed.pick {
        spawn(state, Job::Pick { id: item.id, on: !item.picked_by_me }, api::pick_request(&signer, &r, !item.picked_by_me));
    }
    if pressed.keep {
        spawn(state, Job::Keep { id: item.id, on: !item.kept, from_admin: false }, api::keep_request(&signer, &r, !item.kept));
    }
}

/// The confirmations over the page: removing a file from the item, and removing the item. True
/// when the item's removal was sent: its page closes, and the Media page says how it went.
fn draw_confirmations(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, page: &mut ItemPage) -> bool {
    let Some(item) = page.item.clone() else { return false };
    if let Some(f) = page.confirm_file.clone() {
        let mut open = true;
        let (mut yes, mut no) = (false, false);
        let name = open::file_download_name(&item, &f);
        widgets::dialog(ctx, theme, "media_item_confirm_file", "Remove a file", &mut open, |ui| {
            ui.label(RichText::new(format!("Remove {name} from this item? It is deleted from the server.")).size(theme.font_size_body).color(theme.text_primary()));
            ui.horizontal(|ui| {
                yes = widgets::Button::danger("Remove the file").show(ui, theme);
                no = widgets::Button::secondary("Keep it").show(ui, theme);
            });
        });
        if yes {
            remove_file(state, page, &item, &f);
        }
        if yes || no || !open {
            page.confirm_file = None;
        }
    }
    if page.confirm_remove {
        let mut open = true;
        let (mut yes, mut no) = (false, false);
        widgets::dialog(ctx, theme, "media_item_confirm_remove", "Remove from Media", &mut open, |ui| {
            ui.label(RichText::new(item.display_title()).size(theme.font_size_body).color(theme.text_primary()).strong());
            ui.label(RichText::new(api::remove_words(&item)).size(theme.font_size_body).color(theme.text_secondary()));
            ui.horizontal(|ui| {
                yes = widgets::Button::danger("Remove").show(ui, theme);
                no = widgets::Button::secondary("Cancel").show(ui, theme);
            });
        });
        if yes || no || !open {
            page.confirm_remove = false;
        }
        if yes {
            match (item.reference(), signer_or_say(state)) {
                (Ok(r), Some(signer)) => {
                    let req = api::remove_request(&signer, &api::RemoveTarget::Item(r));
                    spawn(state, Job::Remove { id: Some(item.id), from_admin: false }, req);
                    return true;
                }
                (Err(words), _) => page.status = words,
                (_, None) => page.status = super::UNLOCK_LINE.to_string(),
            }
        }
    }
    false
}

/// Take file `f` off `item` (`POST /api/media/files/remove`, by its number and its own stored name).
pub(crate) fn remove_file(state: &mut GuiState, page: &mut ItemPage, item: &Item, f: &ItemFile) {
    if f.file == 0 {
        page.status = "The first file is the item itself: remove the item instead.".to_string();
        return;
    }
    let (Ok(r), Some(signer)) = (item.reference(), signer_or_say(state)) else {
        page.status = super::UNLOCK_LINE.to_string();
        return;
    };
    spawn(state, Job::FileRemove { id: item.id, file: f.file }, api_items::file_remove_request(&signer, &r, f.file, &f.filename));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AN ANSWER FOR AN ITEM'S PAGE LANDS IN ITS PLACE: a read by number opens the page with the item,
    /// or says it is not there; an edit's `{ok, item}` changes the item on its page and in the grid;
    /// a refusal is said on the page it was for.
    ///
    /// Seen red 2026-10-12 with `take_item` changing only the page and not the grid: "the grid shows
    /// the edited title / left: \"Old\" / right: \"New\"".
    #[test]
    fn an_answer_for_an_items_page_lands_in_its_place() {
        crate::config::keep_saves_off_disk();
        let mut state = GuiState::default();
        let old = Item { id: 4, title: "Old".into(), ..Default::default() };
        state.media.list.items.push(old);
        on_answer(&mut state, Job::Item { id: 4 }, Ok(serde_json::json!({ "items": [{ "id": 4, "title": "Old" }] })));
        assert_eq!(state.media.item.as_ref().and_then(|p| p.item.as_ref()).map(|i| i.title.as_str()), Some("Old"));
        on_answer(&mut state, Job::Edit { id: 4 }, Ok(serde_json::json!({ "ok": true, "item": { "id": 4, "title": "New", "featured": "main" } })));
        assert_eq!(state.media.item.as_ref().and_then(|p| p.item.as_ref()).map(|i| i.title.as_str()), Some("New"));
        assert_eq!(state.media.list.items[0].title, "New", "the grid shows the edited title");
        assert!(state.media.list.items[0].featured.is_on());
        on_answer(&mut state, Job::FileRemove { id: 4, file: 7 }, Err(Refusal::new(400, "The first file is the item itself: remove the item instead.")));
        assert_eq!(state.media.item.as_ref().map(|p| p.status.as_str()), Some("The first file is the item itself: remove the item instead."));
        state.media.item = None;
        on_answer(&mut state, Job::Item { id: 9 }, Ok(serde_json::json!({ "items": [] })));
        assert_eq!(state.media.item.as_ref().map(|p| (p.id, p.status.as_str(), p.item.is_none())), Some((9, NOT_THERE, true)));
    }
}

#[cfg(test)]
#[path = "item_tests.rs"]
mod item_tests;

#[cfg(test)]
#[path = "relay_round5_tests.rs"]
mod relay_round5_tests;
