//! An item shown on the Files page (Media round 5, lane 1D; the three panes since 2026-10-19,
//! docs/design/files-showcase.md "The three panes"). A press on a row in the list (or a tile in the
//! Gallery) makes it the item shown: the middle pane shows it to fit (viewer.rs), and the right
//! pane, drawn here ([`draw_details`]), holds everything about it:
//!
//! - the title; "by Ann in Fire flow arts"; its labels; "Kept by the server"; "Members only";
//! - "4 picks, 12 views, 3 downloads", and the day it was put on the page;
//! - its buttons: Copy link (`https://<host>/files/<id>`, the page the relay serves, never
//!   `/files?id=`), Edit (the uploader and admins), Pick, Keep (admins and moderators), Report and
//!   Remove item, and, for anyone signed in or not, Report a copyright problem (takedown.rs); the
//!   admins' three Feature choices under them;
//! - the whole description, as plain text with its line breaks;
//! - the licence by name, linked to its deed, with the server's own line for it, and a line when
//!   it is offered for the game;
//! - its files, the item's own first, each with its type, name, size and Download (counted by the
//!   relay: open.rs `download_origin`), and View in 3D on a `.glb` or `.stl`.
//!
//! EDIT IS IN PLACE (operator, 2026-10-19: "When clicking edit, instead of popping up another
//! window, just change the details panel on the right to accept edits. Once done clicking save
//! undoes the GUI changes for edit mode."): Edit turns this pane into the edit form (edit.rs
//! `draw_in_place`), in its three coloured parts with the pane's border in the nav bar's moving
//! colours, so edit mode stands out at a glance; Save sends it and, once answered, the pane is the
//! item's details again. Cancel with nothing changed goes straight back; with changes it asks
//! first ([`Discard`]), and so does pressing another item, closing the item's window, or leaving
//! the page ([`draw_leave_question`]).
//!
//! An item posted in chat opens in a window of its own over the chat ([`draw_window`]): the same
//! two parts, the item to fit and its details, side by side, with the same edit in place. The
//! Files page itself is never pushed in front of the conversation the person is reading.
//!
//! Opening an item counts a view (`POST /api/files/viewed`; the relay counts a person once a day).
//!
//! What lands here from the server ([`on_answer`]): one item read by its number ([`Job::Item`], a
//! list read with `id`), the item as an edit, a visibility change or a file taken off left it (each
//! answers `{ok, item}`), and a view counted (always `{ok: true}`). An item that changed is changed
//! wherever the page shows it: here, in the list and on the featured shelf ([`take_item`]); and
//! the pane follows the list's copy, where a Pick, Keep or Feature lands.

use egui::RichText;
use serde_json::Value;

use super::api::{self, Item, ItemFile, Refusal};
use super::api_items;
use super::open::{self, Opened};
use super::{signed_in_here, signer_or_say, spawn, spawn_with, tiles, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The item shown, while there is one.
#[derive(Debug, Clone, Default)]
pub struct ItemPage {
    /// The item's number.
    pub id: i64,
    /// The item as the server last gave it; None until it has answered, or when it is not there
    /// for this person.
    pub item: Option<Item>,
    /// One line about the last thing done or refused on it.
    pub status: String,
    /// The edit form, while the details pane is in edit mode (edit.rs).
    pub edit: Option<super::edit::EditForm>,
    /// A file's Remove, waiting for its confirmation.
    pub confirm_file: Option<ItemFile>,
    /// The item's own Remove, waiting for its confirmation.
    pub confirm_remove: bool,
    /// The address Copy link last copied, shown under it.
    pub copied: Option<String>,
    /// Shown in a window of its own (opened from a card in chat while the Files page is not on
    /// screen). On the Files page it is the item in the panes, whatever this says.
    pub window: bool,
    /// The edit form holds changes and something would leave it: asked first.
    pub confirm_discard: Option<Discard>,
}

/// What would leave an edit form that holds changes, waiting for the person's answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Discard {
    /// Cancel was pressed.
    Cancel,
    /// Another item was pressed in the list (or a tile in the Gallery): it is shown once the
    /// changes are let go.
    Open(Item),
    /// The item's window was closed.
    Close,
}

/// What the pane says when the item is not on the Files page, or this person may not see it.
pub const NOT_THERE: &str = "That item is not on the Files page, or you may not see it.";
/// What the pane says once a file was renamed, and once a picture on the item became its cover.
pub const FILE_RENAMED: &str = "Renamed.";
pub const COVER_CHOSEN: &str = "That picture is the cover now.";
/// What the pane says once a file was taken off the item.
pub const FILE_REMOVED: &str = "Removed from the item.";
/// What the pane says once an edit went.
pub const SAVED: &str = "Saved.";
/// The question when an edit form with changes would be left, and its two answers.
pub const DISCARD_TITLE: &str = "Unsaved changes";
pub const DISCARD_THEM: &str = "Discard the changes";
pub const KEEP_EDITING: &str = "Keep editing";
/// The question when the page was left with an edit unsaved, and its first answer.
pub const GO_BACK: &str = "Go back to them";
/// The middle and right panes while nothing is chosen.
pub const CHOOSE_LINE: &str = "Choose something in the list to see it here.";
pub const DETAILS_LINE: &str = "Its details, licence and files show here, with Edit for your own.";
/// The id of an item's window over the chat.
pub const WINDOW_ID: &str = "media_item_page";

/// The question's words for item `title`.
pub fn discard_words(title: &str) -> String {
    format!("Your changes to \"{title}\" are not saved.")
}

/// Put `item` wherever the page shows it: the pane (when it is that item's), the list and the
/// featured shelf.
pub(crate) fn take_item(state: &mut GuiState, item: Item) {
    for shown in state.files.list.items.iter_mut().chain(state.files.featured.iter_mut()).filter(|i| i.id == item.id) {
        *shown = item.clone();
    }
    if let Some(page) = state.files.item.as_mut().filter(|p| p.id == item.id) {
        if let Some(form) = page.edit.as_mut() {
            // The edit form's files follow the item (a file added or taken off while it is open);
            // the words being edited are the person's and stay as they are.
            form.was.files = item.files.clone();
            form.was.cover = item.cover.clone();
        }
        page.item = Some(item);
    }
}

/// Say `words` on the item when it is item `id`, else on the Files page.
fn say(state: &mut GuiState, id: i64, words: String) {
    match state.files.item.as_mut().filter(|p| p.id == id) {
        Some(page) => page.status = words,
        None => state.files.status = words,
    }
}

/// Whether an edit of item `id` (the edit itself, or its visibility) is still on its way.
pub(crate) fn saving(state: &GuiState, id: i64) -> bool {
    state.files.jobs.iter().any(|(j, _)| *j == Job::Edit { id })
}

/// An answer for the item shown (pages/files.rs `on_done` hands these over).
pub(crate) fn on_answer(state: &mut GuiState, job: Job, result: Result<Value, Refusal>) {
    match (job, result) {
        (Job::Item { id }, Ok(v)) => {
            let answer = api::parse_list(&v);
            // The labels' names, as the website's `openById` takes them: an item opened by its
            // number before the list ever answered still names a labelled item's labels.
            if state.files.list.labels.is_empty() && !answer.labels.is_empty() {
                state.files.list.labels = answer.labels;
            }
            let found = answer.items.into_iter().find(|i| i.id == id);
            let page = state.files.item.get_or_insert_with(|| ItemPage { id, ..Default::default() });
            if page.id != id {
                return;
            }
            // It had nothing yet: it was opened by its number (`open_by_number`), and this is the
            // opening, so it counts a view the way a row's press does.
            let first = page.item.is_none();
            page.status = if found.is_some() { String::new() } else { NOT_THERE.to_string() };
            match found {
                Some(item) => {
                    if first {
                        count_view(state, &item);
                    }
                    take_item(state, item);
                }
                None => page.item = None,
            }
        }
        (Job::Edit { id }, Ok(v)) => match v.get("item").and_then(Item::from_json) {
            Some(item) => {
                take_item(state, item);
                // The edit form closes, and the pane is the item's details again, once everything
                // it sent is answered (the edit, and the visibility when that changed too). A
                // refusal of the other half keeps the form with its words, and nothing is said to
                // be saved.
                let refused = state.files.item.as_ref().filter(|p| p.id == id).and_then(|p| p.edit.as_ref()).is_some_and(|f| !f.problem.is_empty());
                if !saving(state, id) && !refused {
                    if let Some(page) = state.files.item.as_mut().filter(|p| p.id == id) {
                        page.edit = None;
                        page.confirm_discard = None;
                    }
                    say(state, id, SAVED.to_string());
                }
            }
            None => say(state, id, "The server's answer was not understood.".to_string()),
        },
        (Job::FileRemove { id, .. }, Ok(v)) => match v.get("item").and_then(Item::from_json) {
            Some(item) => {
                take_item(state, item);
                say(state, id, FILE_REMOVED.to_string());
            }
            None => say(state, id, "The server's answer was not understood.".to_string()),
        },
        (ref job @ (Job::FileRename { id, .. } | Job::FileCover { id, .. }), Ok(v)) => match v.get("item").and_then(Item::from_json) {
            Some(item) => {
                take_item(state, item);
                let words = if matches!(job, Job::FileRename { .. }) { FILE_RENAMED } else { COVER_CHOSEN };
                say(state, id, words.to_string());
            }
            None => say(state, id, "The server's answer was not understood.".to_string()),
        },
        (Job::FileRename { id, .. } | Job::FileCover { id, .. }, Err(e)) => {
            // The relay's own words, in the edit form when it is open.
            match state.files.item.as_mut().filter(|p| p.id == id).and_then(|p| p.edit.as_mut()) {
                Some(form) => form.problem = e.words,
                None => say(state, id, e.words),
            }
        }
        (Job::Viewed { .. }, Ok(_)) => {}
        (Job::Viewed { id }, Err(e)) => log::warn!("Files: a view of item {id} was not counted: {e}"),
        (Job::Edit { id }, Err(e)) => {
            // The relay's own words, shown as they come, in the edit form when it is open.
            match state.files.item.as_mut().filter(|p| p.id == id).and_then(|p| p.edit.as_mut()) {
                Some(form) => form.problem = e.words,
                None => say(state, id, e.words),
            }
        }
        (Job::Item { id } | Job::FileRemove { id, .. }, Err(e)) => say(state, id, e.words),
        (other, _) => log::warn!("Files: {other:?} is not an item's answer"),
    }
}

/// Whether the item shown holds an edit with changes not saved.
pub(crate) fn unsaved(state: &GuiState) -> bool {
    state.files.item.as_ref().and_then(|p| p.edit.as_ref()).is_some_and(|f| f.changed())
}

/// Show `item`: in the panes, with its transfers, and a view counted. The item already shown is
/// left as it is (a playing video plays on). Another item, while the edit form holds changes,
/// waits for the person's answer first ([`Discard::Open`]).
pub(crate) fn open(state: &mut GuiState, item: &Item) {
    if let Some(page) = state.files.item.as_mut() {
        if page.id == item.id && page.item.is_some() {
            return;
        }
        if page.edit.as_ref().is_some_and(|f| f.changed()) {
            page.confirm_discard = Some(Discard::Open(item.clone()));
            return;
        }
    }
    state.files.item = Some(ItemPage { id: item.id, item: Some(item.clone()), ..Default::default() });
    state.files.open = Some(Opened::new(item.clone()));
    count_view(state, item);
}

/// Show `item` in a window of its own over the page that asked (a card in chat).
pub(crate) fn open_in_window(state: &mut GuiState, item: &Item) {
    open(state, item);
    if let Some(page) = state.files.item.as_mut().filter(|p| p.id == item.id) {
        page.window = true;
    }
}

/// Show item `id` by its number (the Reports page's Open, through media.rs `open_item_by_number`).
/// An item the list or the featured shelf holds opens as it is, as a row's press opens it. Any
/// other is asked for by its number, a list read with `id` ([`Job::Item`]), the way the website's
/// `route` asks (files-app.js `openById`): it is shown at once saying "Loading...", and
/// [`on_answer`] puts the item in (counting the view then), or says it is not there for this person.
pub(crate) fn open_by_number(state: &mut GuiState, id: i64) {
    // An edit with changes is never dropped for another item: it stays, and says why.
    if unsaved(state) && state.files.item.as_ref().is_some_and(|p| p.id != id) {
        if let Some(form) = state.files.item.as_mut().and_then(|p| p.edit.as_mut()) {
            form.problem = format!("Save or cancel the changes here first, then open item {id} again.");
        }
        return;
    }
    let known = state.files.list.items.iter().chain(state.files.featured.iter()).find(|i| i.id == id).cloned();
    if let Some(item) = known {
        open(state, &item);
        return;
    }
    state.files.item = Some(ItemPage { id, ..Default::default() });
    // Another item's transfers go with it (`Drop for Opened` removes a video's copy).
    state.files.open = None;
    if super::server_here(state).is_empty() {
        if let Some(page) = state.files.item.as_mut() {
            page.status = NOT_THERE.to_string();
        }
        return;
    }
    // Signed when this person is signed in here, so a member is answered members-only items too.
    let filters = api::Filters { id: Some(id), ..Default::default() };
    let req = api::list_request(&filters, None, 1, &state.profile_public_key, super::signer_here(state).as_ref());
    spawn(state, Job::Item { id }, req);
}

/// Count a view of `item` (`POST /api/files/viewed`, unsigned). The session's upload token goes in
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

/// Whether this person put `item` on the Files page.
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

/// "On the Files page since 12 October 2026" (UTC), from the server's time in milliseconds; empty when it
/// gave none.
pub(crate) fn since_line(item: &Item) -> String {
    let ms = if item.published_at > 0 { item.published_at } else { item.uploaded_at };
    if ms <= 0 {
        return String::new();
    }
    format!("On the Files page since {}", day_words(ms))
}

/// A day in words, "12 October 2026", from a time in milliseconds (UTC; the same civil-date
/// reckoning as game_admin.rs `format_ban_date`).
pub(crate) fn day_words(ms: i64) -> String {
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let days = (ms / 1000).div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{d} {} {year}", MONTHS[(m - 1).clamp(0, 11) as usize])
}

/// What the item's parts asked for this frame, done once they are drawn ([`act`]).
#[derive(Default)]
pub(crate) struct Pressed {
    pub play: bool,
    /// Play here on a sound (the middle pane's waveform).
    pub play_sound: bool,
    pub download: Option<ItemFile>,
    pub view_full: bool,
    /// The address Copy link put on the clipboard.
    pub copied: Option<String>,
    pub edit: bool,
    pub pick: bool,
    pub keep: bool,
    pub report: bool,
    pub remove: bool,
    /// "Report a copyright problem" (takedown.rs): for anyone, signed in or not.
    pub copyright: bool,
    /// View in 3D on a `.glb` or `.stl` file, and Close the 3D view.
    pub view_3d: Option<ItemFile>,
    pub close_3d: bool,
}

/// The item shown and its transfers, taken out of the state to be drawn (put back by [`finish`]),
/// the item made the list's copy first (where a Pick, Keep or Feature lands). None with nothing
/// shown.
pub(crate) fn take(state: &mut GuiState) -> Option<(ItemPage, Option<Opened>)> {
    let Some(mut page) = state.files.item.take() else {
        state.files.open = None;
        return None;
    };
    if let Some(fresh) = state.files.list.items.iter().chain(state.files.featured.iter()).find(|i| i.id == page.id) {
        if page.item.as_ref() != Some(fresh) {
            page.item = Some(fresh.clone());
        }
    }
    let mut opened = state.files.open.take().filter(|o| o.item.id == page.id);
    if let Some(item) = page.item.clone() {
        let o = opened.get_or_insert_with(|| Opened::new(item.clone()));
        o.item = item;
    }
    Some((page, opened))
}

/// Do what was pressed and put the item back.
pub(crate) fn finish(state: &mut GuiState, mut page: ItemPage, mut opened: Option<Opened>, pressed: Pressed) {
    if let (Some(item), Some(o)) = (page.item.clone(), opened.as_mut()) {
        act(state, &mut page, o, &item, pressed);
    }
    // Something done while it was out (a Remove that answered at once) may have chosen another.
    if state.files.item.is_none() {
        state.files.item = Some(page);
        state.files.open = opened;
    }
}

/// The right pane: the edit form while editing (edit.rs), else everything about the item.
pub(crate) fn draw_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, page: &mut ItemPage, opened: Option<&Opened>, pressed: &mut Pressed) {
    let Some(item) = page.item.clone() else {
        let words = if page.status.is_empty() { "Loading..." } else { page.status.as_str() };
        ui.label(RichText::new(words).size(theme.font_size_body).color(theme.text_muted()));
        return;
    };
    if page.edit.is_some() {
        super::edit::draw_in_place(ui, theme, state, page);
        return;
    }
    egui::ScrollArea::vertical().id_salt(("media_details", item.id)).auto_shrink([false, false]).show(ui, |ui| {
        ui.label(RichText::new(item.display_title()).size(theme.font_size_heading).color(theme.text_primary()).strong());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(by_line(&item)).size(theme.font_size_small).color(theme.text_secondary()));
            for name in tiles::label_names(state, &item) {
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
        ui.label(RichText::new(counts_line(&item)).size(theme.font_size_small).color(theme.text_muted()));
        let since = since_line(&item);
        if !since.is_empty() {
            ui.label(RichText::new(since).size(theme.font_size_small).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_sm);
        draw_actions(ui, theme, state, &item, pressed);
        if let Some(link) = &page.copied {
            ui.label(RichText::new(format!("Copied: {link}")).size(theme.font_size_small).color(theme.success()));
        }
        if !page.status.is_empty() {
            ui.label(RichText::new(&page.status).size(theme.font_size_small).color(theme.text_secondary()));
        }
        if !item.description.trim().is_empty() {
            ui.add_space(theme.spacing_sm);
            // The whole description, as plain text: its own line breaks are kept.
            ui.add(egui::Label::new(RichText::new(item.description.trim()).size(theme.font_size_body).color(theme.text_primary())).wrap());
        }
        ui.add_space(theme.spacing_sm);
        draw_licence(ui, theme, state, &item);
        ui.add_space(theme.spacing_sm);
        if let Some(o) = opened {
            draw_files(ui, theme, state, o, &item, pressed);
        }
    });
}

/// The licence by name (linked to its deed, with the server's own line for it), and a line when
/// the item is offered for the game. A licence this server no longer offers is named by its id.
fn draw_licence(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, item: &Item) {
    let licences = &state.files.list.licences;
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

/// The item's files, the item's own first, each with its name, kind, size, Download and, on a
/// `.glb` or `.stl`, View in 3D.
fn draw_files(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, opened: &Opened, item: &Item, pressed: &mut Pressed) {
    let files = files_of(item);
    // View in 3D: not where the protected setup hides this person's pictures (viewer.rs).
    let may_view_3d = !matches!(tiles::show_of(state, item), tiles::Show::Hidden(_));
    widgets::subsection_label(ui, theme, if files.len() == 1 { "File" } else { "Files" });
    let small = widgets::ButtonSize::Small;
    let rows = files.iter().map(|f| (f, false)).chain(item.cover.iter().map(|c| (c, true)));
    for (f, cover) in rows {
        // Its type (the badge) and word of caution (net/file_risk.rs, 2026-10-17): by its name, then
        // by what is inside once a Download saved it.
        let name = name_of(item, f);
        let saved = opened.saved_path(f.file).map(|p| (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), p));
        let (file_type, caution) = match &saved {
            Some((saved_name, path)) => (crate::net::file_risk::file_type_saved(saved_name, path), crate::net::file_risk::assess_saved(saved_name, path)),
            None => (crate::net::file_risk::file_type(&name, None), crate::net::file_risk::assess(&name, None)),
        };
        ui.horizontal_wrapped(|ui| {
            widgets::file_badge::show(ui, theme, &file_type);
            let line = if cover { format!("{name}, cover picture, {}", api::size_words(f.size_bytes)) } else { format!("{name}, {}", api::size_words(f.size_bytes)) };
            ui.label(RichText::new(line).size(theme.font_size_small).color(theme.text_primary()));
            if opened.downloading(f.file) {
                ui.label(RichText::new("Downloading...").size(theme.font_size_small).color(theme.text_muted()));
            } else if widgets::Button::secondary("Download").size(small).tooltip("Save it in your downloads folder.").show(ui, theme) {
                pressed.download = Some(f.clone());
            }
            if may_view_3d && !cover && crate::gui::model_view::load::viewable(&name) {
                let tip = crate::gui::model_view::view_tip(&api::size_words(f.size_bytes));
                if widgets::Button::secondary(crate::gui::model_view::VIEW_IN_3D).size(small).tooltip(&tip).show(ui, theme) {
                    pressed.view_3d = Some(f.clone());
                }
            }
        });
        if let Some(caution) = caution {
            widgets::file_caution::show(ui, theme, &caution);
        }
        if let Some(words) = opened.saved_words(f.file) {
            let color = if words.starts_with("Saved") { theme.success() } else { theme.danger() };
            ui.label(RichText::new(words).size(theme.font_size_small).color(color));
        }
    }
    if may_edit(state, item) && (files.len() > 1 || item.cover.is_some()) {
        widgets::body_hint(ui, theme, "Add, rename or remove files and the cover in Edit.");
    }
}

/// The name file `f` of `item` is shown by (Download saves it under `open::file_download_name`).
fn name_of(item: &Item, f: &ItemFile) -> String {
    open::shown_file_name(item, f)
}

/// The item's buttons: Copy link for anyone; Edit for the uploader and admins; then, signed in,
/// Pick (not on your own), Keep (admins and moderators), Report (not on your own) and Remove item
/// (your own, or any as an admin); Report a copyright problem for anyone; and under them the
/// admins' three Feature choices.
fn draw_actions(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: &Item, pressed: &mut Pressed) {
    let signed_in = signed_in_here(state);
    let mine = own(state, item);
    let admin = super::is_admin(state);
    let mut copy_link = false;
    ui.horizontal_wrapped(|ui| {
        if may_edit(state, item) {
            pressed.edit = widgets::Button::primary(EDIT).tooltip(EDIT_TIP).show(ui, theme);
        }
        copy_link = widgets::Button::secondary(COPY_LINK).tooltip("Copy this item's own address, to share it.").show(ui, theme);
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
        // "Report a copyright problem" (2026-10-13, takedown.rs): for anyone, signed in or not, a
        // member or not.
        pressed.copyright = super::takedown::item_button(ui, theme);
    });
    if copy_link {
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

/// Copy link's and Edit's labels.
pub const COPY_LINK: &str = "Copy link";
pub const EDIT: &str = "Edit";
pub const EDIT_TIP: &str = "Change its words, licence, files and cover, here in this pane.";

/// View in 3D on file `f`: the viewer opens and fetches the file from the address the page has for
/// it (`ItemFile::url`: public `/uploads/...`, or the members' route with this session's upload
/// token in a header, as pictures are fetched). Not the counting download route: a look is not a
/// download.
pub(crate) fn open_3d(state: &GuiState, opened: &mut Opened, item: &Item, f: &ItemFile) {
    let name = if f.file == 0 { open::download_name(item) } else { open::file_download_name(item, f) };
    let url = if f.url.starts_with("http://") || f.url.starts_with("https://") {
        f.url.clone()
    } else {
        format!("{}{}", super::base_of(state).trim_end_matches('/'), f.url)
    };
    let header = api::token_header(item, super::token_here(state).as_deref());
    opened.viewer = Some(crate::gui::model_view::Viewer::open(&name, f.size_bytes, move || {
        crate::gui::model_view::fetch(&url, header.as_ref().map(|(n, v)| (*n, v.as_str())))
    }));
}

/// Do what the item's buttons asked, once its parts are drawn.
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
    if pressed.play_sound {
        open::start_sound(state, opened);
    }
    if let Some(f) = pressed.download {
        open::start_download(state, opened, &f);
    }
    if pressed.view_full {
        let key = if item.kind == "picture" { tiles::cache_key(state, item) } else { tiles::cover_key(state, item) };
        state.image_viewer_url = Some(key);
    }
    if let Some(f) = pressed.view_3d {
        open_3d(state, opened, item, &f);
    }
    if pressed.close_3d {
        opened.viewer = None;
    }
    if pressed.edit {
        page.edit = Some(super::edit::EditForm::of(item));
        page.status.clear();
    }
    if pressed.report {
        crate::engine::report::open_for_media(state, item);
    }
    if pressed.copyright {
        super::takedown::open_form(state, Some(item));
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

/// An item posted in chat, in a window of its own over the chat (media.rs `draw_item_window`):
/// the item to fit and its details side by side (stacked on a tall screen), the same parts the
/// Files page's panes draw, with the same edit in place; and its confirmations. Closing it with an
/// edit unsaved asks first.
pub(crate) fn draw_window(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some((mut page, mut opened)) = take(state) else { return };
    let screen = ctx.screen_rect().size();
    let size = egui::vec2((screen.x - 80.0).clamp(320.0, 1200.0), (screen.y - 140.0).clamp(240.0, 780.0));
    let arrange = widgets::panes::arrange_for(size, 2, super::panes::MIN_PANE, theme.spacing_md);
    let shares_id = egui::Id::new("media_item_window_shares");
    let defaults = [0.55f32, 0.45];
    let mut shares: Vec<f32> = ctx.data(|d| d.get_temp::<Vec<f32>>(shares_id)).unwrap_or_else(|| defaults.to_vec());
    let min = if arrange == widgets::panes::Arrange::Across { super::panes::MIN_PANE } else { super::panes::MIN_PANE_DOWN };
    let mut pressed = Pressed::default();
    let mut open = true;
    let title = page.item.as_ref().map(|i| i.display_title()).unwrap_or_else(|| "Item on the Files page".to_string());
    widgets::dialog(ctx, theme, WINDOW_ID, &title, &mut open, |ui| {
        ui.allocate_ui(size, |ui| {
            ui.set_min_size(size);
            widgets::panes::show(ui, theme, egui::Id::new("media_item_window"), arrange, &mut shares, &defaults, min, |ui, i, card| {
                if i == 0 {
                    super::viewer::draw(ui, theme, state, &page, opened.as_mut(), &mut pressed);
                } else {
                    if page.edit.is_some() {
                        super::panes::edit_border(ui, theme, card);
                    }
                    draw_details(ui, theme, state, &mut page, opened.as_ref(), &mut pressed);
                }
            });
        });
    });
    ctx.data_mut(|d| d.insert_temp(shares_id, shares));
    let removed = draw_confirmations(ctx, theme, state, &mut page);
    let closing = !open;
    if closing && page.edit.as_ref().is_some_and(|f| f.changed()) {
        page.confirm_discard = Some(Discard::Close);
    }
    finish(state, page, opened, pressed);
    // Closed (its X, a click beside it, or the item removed) with nothing unsaved: the item and its
    // transfers go, and a video's temporary copy with them (`Drop for Opened`).
    let keep = state.files.item.as_ref().is_some_and(|p| p.confirm_discard.is_some() || p.edit.as_ref().is_some_and(|f| f.changed()));
    if removed || (closing && !keep) {
        state.files.item = None;
        state.files.open = None;
    }
}

/// The confirmations over the Files page (a file's Remove, the item's Remove, the question before
/// changes are let go), drawn after the page.
pub(crate) fn draw_page_dialogs(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut page) = state.files.item.take() else { return };
    let removed = draw_confirmations(ctx, theme, state, &mut page);
    if state.files.item.is_none() {
        state.files.item = Some(page);
    }
    if removed {
        state.files.item = None;
        state.files.open = None;
    }
}

/// The page was left (the nav bar, Esc, a link: any way) while the item's edit form holds changes:
/// asked over whatever page it is now, "Go back to them" or "Discard the changes". Nothing is lost
/// while it waits; with no page on screen (the world), it waits for the next one.
pub(crate) fn draw_leave_question(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if state.active_page == crate::gui::GuiPage::None || !unsaved(state) {
        return;
    }
    let title = state.files.item.as_ref().and_then(|p| p.item.as_ref()).map(|i| i.display_title()).unwrap_or_default();
    let mut open = true;
    let (mut back, mut discard) = (false, false);
    widgets::dialog(ctx, theme, "media_leave_unsaved", DISCARD_TITLE, &mut open, |ui| {
        ui.label(RichText::new(discard_words(&title)).size(theme.font_size_body).color(theme.text_primary()));
        ui.horizontal(|ui| {
            back = widgets::Button::primary(GO_BACK).tooltip("The Files page, with the changes as they were.").show(ui, theme);
            discard = widgets::Button::danger(DISCARD_THEM).show(ui, theme);
        });
    });
    if back || !open {
        state.files.place = super::Place::Shared;
        state.settings.files_layout.gallery = false;
        state.push_nav_to(crate::gui::GuiPage::Files);
    } else if discard {
        if let Some(page) = state.files.item.as_mut() {
            page.edit = None;
            page.confirm_discard = None;
        }
    }
}

/// The confirmations over the item: removing a file from it, removing it, and letting go of an
/// edit's changes. True when the item's removal was sent: it goes, and the Files page says how it
/// went.
fn draw_confirmations(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, page: &mut ItemPage) -> bool {
    let Some(item) = page.item.clone() else { return false };
    if let Some(why) = page.confirm_discard.clone() {
        let mut open = true;
        let (mut discard, mut keep) = (false, false);
        widgets::dialog(ctx, theme, "media_item_confirm_discard", DISCARD_TITLE, &mut open, |ui| {
            ui.label(RichText::new(discard_words(&item.display_title())).size(theme.font_size_body).color(theme.text_primary()));
            ui.horizontal(|ui| {
                discard = widgets::Button::danger(DISCARD_THEM).show(ui, theme);
                keep = widgets::Button::secondary(KEEP_EDITING).show(ui, theme);
            });
        });
        if discard {
            page.edit = None;
            page.confirm_discard = None;
            match why {
                Discard::Cancel => {}
                Discard::Open(next) => {
                    // The next item, shown once this one is put back (media.rs reads it from here).
                    state.files.open_next = Some(next);
                }
                Discard::Close => state.files.close_window = true,
            }
        } else if keep || !open {
            page.confirm_discard = None;
        }
    }
    if let Some(f) = page.confirm_file.clone() {
        let mut open = true;
        let (mut yes, mut no) = (false, false);
        let name = open::shown_file_name(&item, &f);
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
        widgets::dialog(ctx, theme, "media_item_confirm_remove", "Remove from Files", &mut open, |ui| {
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

/// Name file `f` of `item` `name` (`POST /api/files/item-files/rename`; the relay keeps its ending).
pub(crate) fn rename_file(state: &mut GuiState, item: &Item, f: &ItemFile, name: &str) -> Result<(), String> {
    let (Ok(r), Some(signer)) = (item.reference(), signer_or_say(state)) else {
        return Err(super::UNLOCK_LINE.to_string());
    };
    spawn(state, Job::FileRename { id: item.id, file: f.file }, api_items::file_rename_request(&signer, &r, f.file, &f.filename, name));
    Ok(())
}

/// Make picture file `f`, already on `item`, its cover (`POST /api/files/item-files/cover`).
pub(crate) fn cover_from_file(state: &mut GuiState, item: &Item, f: &ItemFile) -> Result<(), String> {
    let (Ok(r), Some(signer)) = (item.reference(), signer_or_say(state)) else {
        return Err(super::UNLOCK_LINE.to_string());
    };
    spawn(state, Job::FileCover { id: item.id, file: f.file }, api_items::file_cover_request(&signer, &r, f.file, &f.filename));
    Ok(())
}

/// Take file `f` off `item` (`POST /api/files/item-files/remove`, by its number and its own stored name).
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

    /// AN ANSWER FOR AN ITEM LANDS IN ITS PLACE: a read by number shows the item, or says it is not
    /// there; an edit's `{ok, item}` changes the item in the pane and in the list; a refusal is said
    /// on the item it was for.
    ///
    /// Seen red 2026-10-12 with `take_item` changing only the page and not the grid: "the grid shows
    /// the edited title / left: \"Old\" / right: \"New\"".
    #[test]
    fn an_answer_for_an_items_page_lands_in_its_place() {
        crate::config::keep_saves_off_disk();
        let mut state = GuiState::default();
        let old = Item { id: 4, title: "Old".into(), ..Default::default() };
        state.files.list.items.push(old);
        on_answer(&mut state, Job::Item { id: 4 }, Ok(serde_json::json!({ "items": [{ "id": 4, "title": "Old" }] })));
        assert_eq!(state.files.item.as_ref().and_then(|p| p.item.as_ref()).map(|i| i.title.as_str()), Some("Old"));
        on_answer(&mut state, Job::Edit { id: 4 }, Ok(serde_json::json!({ "ok": true, "item": { "id": 4, "title": "New", "featured": "main" } })));
        assert_eq!(state.files.item.as_ref().and_then(|p| p.item.as_ref()).map(|i| i.title.as_str()), Some("New"));
        assert_eq!(state.files.list.items[0].title, "New", "the grid shows the edited title");
        assert!(state.files.list.items[0].featured.is_on());
        on_answer(&mut state, Job::FileRemove { id: 4, file: 7 }, Err(Refusal::new(400, "The first file is the item itself: remove the item instead.")));
        assert_eq!(state.files.item.as_ref().map(|p| p.status.as_str()), Some("The first file is the item itself: remove the item instead."));
        state.files.item = None;
        on_answer(&mut state, Job::Item { id: 9 }, Ok(serde_json::json!({ "items": [] })));
        assert_eq!(state.files.item.as_ref().map(|p| (p.id, p.status.as_str(), p.item.is_none())), Some((9, NOT_THERE, true)));
    }

    /// The day an item was put on the page reads as a person writes it.
    ///
    /// Seen red 2026-10-19 with the month counted one on: (left: "9 November 2025", right: "9
    /// October 2025").
    #[test]
    fn the_day_reads_as_a_person_writes_it() {
        assert_eq!(day_words(1_760_000_000_000), "9 October 2025");
        assert_eq!(day_words(0), "1 January 1970");
        assert_eq!(since_line(&Item::default()), "");
    }
}

#[cfg(test)]
#[path = "item_tests.rs"]
pub(crate) mod item_tests;

#[cfg(test)]
#[path = "relay_round5_tests.rs"]
mod relay_round5_tests;
