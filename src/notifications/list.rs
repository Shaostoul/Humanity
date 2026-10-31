//! The in-app list (rung 2, docs/design/notifications.md section 3): the last [`KEPT`] notices,
//! behind the bell in the header, with the count of the ones not yet seen.
//!
//! - **What goes in:** whatever THE rule lets through ([`super::Outcome::list`]), also with the
//!   window in front and with desktop notices and sound both off. A line already on screen and a
//!   line inside the burst guard are not listed.
//! - **A burst is one entry:** a later notice for a conversation whose entry has not been seen yet
//!   folds into that entry (its sender and time are the latest, and it moves to the top), so an
//!   hour of chatting while the panel stays shut is one entry, not one every five seconds.
//! - **Seen means the panel was opened** ([`List::mark_seen`]).
//! - **Kept in memory for the run only, never written to disk** (rung 4 adds the encrypted local
//!   store beside the DMs). What was said is kept only when the person's "What a notice shows"
//!   setting shows the message at the time it arrives; otherwise only who, where and when.
//! - The unseen count is kept as it changes, so the header's bell reads it in constant time each
//!   frame.

use std::collections::VecDeque;

use super::decide::Candidate;
use super::settings::Show;

/// How many notices the list keeps; past this, the oldest go.
pub const KEPT: usize = 100;

/// One notice in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The kind's id in the data file.
    pub kind: String,
    /// The server it came through, normalised (`chat::norm_server_url`).
    pub server: String,
    /// A server's room it was said in; `None` for a conversation with a person.
    pub room: Option<String>,
    /// What the Chat page shows it under: "dm:<key>", or the room's id.
    pub conversation: String,
    /// Who wrote it, as the app showed them.
    pub sender: String,
    /// The room's name.
    pub room_name: String,
    /// What was said, only when the person's setting showed the message when it arrived.
    pub text: Option<String>,
    /// Unix seconds of its latest notice.
    pub at: i64,
    /// The panel has been opened since it arrived.
    pub seen: bool,
}

impl Entry {
    /// The candidate the notice's words are built from (`decide::words`); the text is empty when
    /// it was not kept.
    pub fn candidate(&self) -> Candidate {
        Candidate {
            kind: self.kind.clone(),
            server: self.server.clone(),
            room: self.room.clone(),
            conversation: self.conversation.clone(),
            sender: self.sender.clone(),
            room_name: self.room_name.clone(),
            text: self.text.clone().unwrap_or_default(),
            ..Default::default()
        }
    }
}

/// The list, newest first.
#[derive(Debug, Default)]
pub struct List {
    entries: VecDeque<Entry>,
    unseen: usize,
}

impl List {
    /// Add the notice `c` that arrived at `now`, as `show` allows its words to be kept.
    pub fn add(&mut self, c: &Candidate, show: Show, now: i64) {
        let text = (show == Show::NameAndText && !c.text.is_empty()).then(|| c.text.clone());
        let same = |e: &Entry| !e.seen && e.kind == c.kind && e.server == c.server && e.conversation == c.conversation;
        if let Some(i) = self.entries.iter().position(same) {
            if let Some(mut e) = self.entries.remove(i) {
                e.sender = c.sender.clone();
                e.room_name = c.room_name.clone();
                e.text = text;
                e.at = now;
                self.entries.push_front(e);
            }
            return;
        }
        self.entries.push_front(Entry {
            kind: c.kind.clone(),
            server: c.server.clone(),
            room: c.room.clone(),
            conversation: c.conversation.clone(),
            sender: c.sender.clone(),
            room_name: c.room_name.clone(),
            text,
            at: now,
            seen: false,
        });
        self.unseen += 1;
        while self.entries.len() > KEPT {
            if self.entries.pop_back().is_some_and(|old| !old.seen) {
                self.unseen -= 1;
            }
        }
    }

    /// How many notices the person has not seen yet (the number on the bell).
    pub fn unseen(&self) -> usize {
        self.unseen
    }

    /// The panel was opened: every notice in it is seen.
    pub fn mark_seen(&mut self) {
        if self.unseen == 0 {
            return;
        }
        for e in self.entries.iter_mut() {
            e.seen = true;
        }
        self.unseen = 0;
    }

    /// Every notice, newest first.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Empty the list (the panel's Clear).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.unseen = 0;
    }
}
