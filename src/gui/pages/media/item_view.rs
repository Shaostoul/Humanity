//! An item's own page (Media round 5, docs/design/media-round-5-plan.md 3.9). Its window (the
//! picture or the video, the words, the licence, the files with Download, Copy link, Edit) is lane
//! 1D's; Wave 0 holds what the page shows and puts every answer meant for it in its place
//! ([`on_answer`]), so the requests can be sent and read before the window exists.
//!
//! What lands here: one item read by its number ([`Job::Item`], a list read with `id`), the item as
//! an edit or a file added or taken off left it (each answers `{ok, item}`), and a view counted
//! (always `{ok: true}`). An item that changed is changed wherever the page shows it: on its own
//! page, in the grid and on the featured shelf.

use serde_json::Value;

use super::api::{self, Item, Refusal};
use super::Job;
use crate::gui::GuiState;

/// An item's page, while it is open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemPage {
    /// The item's number.
    pub id: i64,
    /// The item as the server last gave it; None until it has answered, or when it is not there
    /// for this person.
    pub item: Option<Item>,
    /// One line about the last thing done or refused on this page.
    pub status: String,
}

/// What the page says when the item is not on the Media page, or this person may not see it.
pub const NOT_THERE: &str = "That item is not on the Media page, or you may not see it.";

/// Put `item` wherever the page shows it: its own page (when it is that item's), the grid and the
/// featured shelf.
pub(crate) fn take_item(state: &mut GuiState, item: Item) {
    for shown in state.media.list.items.iter_mut().chain(state.media.featured.iter_mut()).filter(|i| i.id == item.id) {
        *shown = item.clone();
    }
    if let Some(page) = state.media.item.as_mut().filter(|p| p.id == item.id) {
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
            page.item = found;
        }
        (Job::Edit { id } | Job::FileAdd { id } | Job::FileRemove { id, .. }, Ok(v)) => match v.get("item").and_then(Item::from_json) {
            Some(item) => {
                take_item(state, item);
                say(state, id, String::new());
            }
            None => say(state, id, "The server's answer was not understood.".to_string()),
        },
        (Job::Viewed { .. }, Ok(_)) => {}
        (Job::Viewed { id }, Err(e)) => log::warn!("Media: a view of item {id} was not counted: {e}"),
        (Job::Item { id } | Job::Edit { id } | Job::FileAdd { id } | Job::FileRemove { id, .. }, Err(e)) => say(state, id, e.words),
        (other, _) => log::warn!("Media: {other:?} is not an item page's answer"),
    }
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
