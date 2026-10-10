//! One connection's own mailbox fetch (section 10o O1 of docs/design/blocking-and-safe-mode.md,
//! 2026-10-10), without a socket.
//!
//! WHY. Every device signed in as a person receives every `dm_batch` page, because they share one
//! mailbox and the relay sends a page to every socket of that key. Before 10o the app counted any
//! last page as "my mailbox was read" (10n N7) and moved its read position (the DM store's
//! high-water mark) on any page and on any live `dm_new`. So another device's fetch could start
//! this one's pass sweep before this device had read its own mail, and with a backlog over one
//! page, a live DM moved the position past rows never read, choice notes included: the next page
//! was asked for from there, and those rows were never fetched.
//!
//! The rule now: every `dm_fetch` carries a fresh random `ref`, which the relay echoes on that
//! fetch's page (`src/relay/handlers/msg_handlers.rs` `handle_dm_fetch`). A page carrying any other
//! ref, or none, is another device's (or a stale one) and is ignored entirely: its rows come in this
//! device's own fetch anyway. The mailbox counts as read only on the last page (`done`) carrying
//! this connection's own ref; the next page is asked for from the last id of this connection's own
//! previous page; and until its own fetch is done, a live `dm_new` is handled but does not move the
//! read position (engine/mailbox.rs).
//!
//! Held per connection: on GuiState for the server we are on (`dm_fetch`), on each parked
//! connection (`ServerConnection.mailbox`), carried through park and unpark.

use serde_json::{json, Value};

/// This connection's own mailbox fetch. A new socket starts from `Default` (nothing sent, nothing
/// read).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailboxFetch {
    /// The fetch went out on this connection (its first `dm_fetch`): once per connection.
    pub sent: bool,
    /// Its last page, carrying its own ref, has been read and applied (10n N7: only then does the
    /// pass sweep run, and only then does a live `dm_new` move the read position).
    pub done: bool,
    /// The ref of the `dm_fetch` this connection waits on. A page carrying any other is not ours.
    reference: Option<String>,
    /// Where the next page starts: the last id of this connection's own previous page (the read
    /// position the fetch began from, before its first page).
    after: i64,
}

/// What a `dm_batch` page is to this connection.
#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    /// Not ours: another device's page, a stale one, or one with no ref. Nothing is done with it,
    /// not its rows, not the read position, not "the mailbox was read".
    Foreign,
    /// One of this connection's own pages: apply its rows, then move the read position to
    /// `last_id`.
    Own {
        /// The largest row id on it (0 on an empty page).
        last_id: i64,
        /// The next `dm_fetch` to send (`done: false`): a fresh ref, paging from `last_id`.
        next: Option<Value>,
        /// It was the last page: this connection's mailbox is read.
        read: bool,
    },
}

impl MailboxFetch {
    /// A connection whose mailbox has been read, for tests that start after that.
    #[cfg(test)]
    pub fn already_read() -> Self {
        Self { sent: true, done: true, ..Default::default() }
    }

    /// Begin this connection's fetch from `after_id` (the store's read position): the `dm_fetch`
    /// frame to send, carrying a fresh ref. None, changing nothing, when no ref can be made (no
    /// randomness from the system); the caller tries again on the next channel list.
    pub fn begin(&mut self, after_id: i64) -> Option<Value> {
        let reference = super::put_answers::new_ref()?;
        *self = Self { sent: true, done: false, reference: Some(reference.clone()), after: after_id };
        Some(json!({ "type": "dm_fetch", "after_id": after_id, "ref": reference }))
    }

    /// Has this connection's mailbox been read to its own last page? Until then the pass sweep
    /// waits (10n N7), and a live `dm_new` does not move the read position (10o O1): the row is
    /// still ahead of where this connection's own next page starts, so moving past it would skip
    /// every row between.
    pub fn is_read(&self) -> bool {
        self.sent && self.done
    }

    /// The last page (`Page::Own { read: true }`) has been read and its rows applied: done, and no
    /// page is awaited any more.
    pub fn finish(&mut self) {
        self.done = true;
        self.reference = None;
    }

    /// Read a `dm_batch` frame: ours, or not. Ours only when it carries the ref of the fetch this
    /// connection waits on; each of our fetches gets one page back, so a ref is used once.
    pub fn page(&mut self, frame: &Value) -> Page {
        let ours = self.reference.is_some() && frame.get("ref").and_then(Value::as_str) == self.reference.as_deref();
        if !ours {
            return Page::Foreign;
        }
        let last_id = frame
            .get("messages")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|m| m.get("id").and_then(Value::as_i64)).max().unwrap_or(0))
            .unwrap_or(0);
        let done = frame.get("done").and_then(Value::as_bool).unwrap_or(true);
        let from = self.after;
        self.after = from.max(last_id);
        // Each of our fetches gets one page back, so its ref is used once.
        self.reference = None;
        if done {
            // Not `done` yet: that is set by `finish` once the page's rows have been applied, so
            // nothing that waits for the mailbox runs halfway through its last page.
            return Page::Own { last_id, next: None, read: true };
        }
        // More rows remain, so this page was full and its last id is past where it started. A
        // page that did not move on would be asked for again forever: stop instead, unread (the
        // sweep then waits for the next connection, as when a server never sends its last page).
        if last_id <= from {
            log::warn!("a mailbox page said more remain but moved no further; fetch stopped");
            return Page::Own { last_id, next: None, read: false };
        }
        let next = super::put_answers::new_ref().map(|reference| {
            self.reference = Some(reference.clone());
            json!({ "type": "dm_fetch", "after_id": self.after, "ref": reference })
        });
        Page::Own { last_id, next, read: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(reference: Option<&str>, ids: &[i64], done: bool) -> Value {
        let mut frame = json!({ "type": "dm_batch", "messages": ids.iter().map(|i| json!({ "id": i, "content": "x" })).collect::<Vec<_>>(), "done": done });
        if let Some(r) = reference {
            frame["ref"] = json!(r);
        }
        frame
    }

    /// 10o O1 WITHOUT A SOCKET: a fetch carries a fresh ref; a page carrying another ref, or none,
    /// is not ours and changes nothing; our page that is not the last asks for the next one from
    /// its own last id, with a fresh ref (the old one is then not ours any more); our last page
    /// counts the mailbox read once its rows were applied (`finish`). A page that says more remain
    /// but moved no further stops the fetch unread instead of asking for the same page forever.
    /// Seen red 2026-10-10 with `page` taking every page as ours (the ref check removed): "a page
    /// with another device's ref is not ours" failed (left `Own { .. }`).
    #[test]
    fn only_pages_carrying_this_connections_ref_are_its_own() {
        let mut f = MailboxFetch::default();
        assert!(!f.is_read());
        assert_eq!(f.page(&batch(Some("abc"), &[1], true)), Page::Foreign, "nothing awaited, nothing ours");
        let first = f.begin(40).expect("a fetch");
        let mine = first["ref"].as_str().unwrap().to_string();
        assert_eq!((first["type"].as_str(), first["after_id"].as_i64()), (Some("dm_fetch"), Some(40)));
        assert_eq!(mine.len(), 32, "a fresh random ref");

        assert_eq!(f.page(&batch(Some("another-device"), &[41, 90], true)), Page::Foreign, "a page with another device's ref is not ours");
        assert_eq!(f.page(&batch(None, &[41, 90], true)), Page::Foreign, "nor one with none");
        assert!(!f.is_read(), "and neither counts the mailbox read");

        let Page::Own { last_id, next: Some(next), read: false } = f.page(&batch(Some(&mine), &[41, 57], false)) else { panic!("our first page") };
        assert_eq!(last_id, 57);
        assert_eq!(next["after_id"].as_i64(), Some(57), "the next page from the last id of our own page");
        let again = next["ref"].as_str().unwrap().to_string();
        assert_ne!(again, mine, "with a fresh ref");
        assert_eq!(f.page(&batch(Some(&mine), &[58], true)), Page::Foreign, "the old ref is used once");

        assert_eq!(f.page(&batch(Some(&again), &[58, 70], true)), Page::Own { last_id: 70, next: None, read: true });
        assert!(!f.is_read(), "not before its rows were applied");
        f.finish();
        assert!(f.is_read(), "our last page counts the mailbox read");
        assert_eq!(f.page(&batch(Some(&again), &[71], true)), Page::Foreign, "and nothing is awaited after it");

        // A new socket starts from Default: a page of the old socket's fetch, read after it
        // opened, neither pages on nor counts as the last page.
        let mut old = MailboxFetch::default();
        let r = old.begin(0).unwrap()["ref"].as_str().unwrap().to_string();
        old = MailboxFetch::default();
        assert_eq!(old.page(&batch(Some(&r), &[1], true)), Page::Foreign, "the old socket's page is not the new one's");

        let mut stuck = MailboxFetch::default();
        let r = stuck.begin(10).unwrap()["ref"].as_str().unwrap().to_string();
        assert_eq!(stuck.page(&batch(Some(&r), &[], false)), Page::Own { last_id: 0, next: None, read: false }, "no progress: stopped, unread");
        assert!(!stuck.is_read());
    }
}
