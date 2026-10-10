//! The block list (step C of docs/design/blocking-and-safe-mode.md, section 10d, 2026-10-09):
//! the identity keys this person has blocked, each with the date, and the notes to oneself that
//! carry a block or an unblock to the person's other devices. No socket and no GUI, so every rule
//! here has a unit test; the half that acts on the app is src/engine/block.rs.
//!
//! WHERE IT LIVES. One encrypted file per identity on this device, beside the DM stores
//! (`%APPDATA%/HumanityOS/dms/`), and NOT one per server like the DM store: a key is the same
//! person on every server, and the chat page merges posts from several servers into one Commons
//! view, so a list kept per server would let a blocked person's posts back in through another
//! server. The file is AES-256-GCM under a key derived from the seed (the DM store's scheme, its
//! own domain), so a copied file names nobody without the seed, and its name is a hash, so the
//! directory listing does not name the identity either. The server never holds any of it: a
//! block is a "negative edge", at least as sensitive as a friendship (section 4.2).
//!
//! THE NOTES TO ONESELF. Other devices of the same identity learn a change from a sealed note
//! the person sends only to themselves: `[[hum:block:v1]]<key>` or `[[hum:unblock:v1]]<key>`
//! (`CTL_BLOCK` / `CTL_UNBLOCK` in net/dm_pq.rs, which must match the web client). The rules,
//! agreed with the web client: notes are applied in mailbox order (the order they arrive in);
//! the date of a block is the note's signed time; blocking someone already blocked keeps the
//! first date; a note naming ourselves counts for nothing. One addition here that changes nothing
//! in ordinary use: each note is applied once (`first_sight`, by its signature), so a relay that
//! delivers an old note again cannot undo a later change, and the echo of a note this device sent
//! does not apply it a second time. A note made while no server is connected waits in this file
//! (`queue`, a newer one for the same person replacing an older one) and goes out on the next
//! connection.

use std::collections::HashMap;
use std::path::PathBuf;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng as AesOsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};

use super::dm_pq::{CTL_BLOCK, CTL_UNBLOCK};

/// BLAKE3 domain for the file's encryption key. Distinct from every other seed-derived key.
const LIST_KEY_DOMAIN: &str = "hum/block-list/v1";

/// The longest key a note may carry. A Dilithium3 key in hex is 3,904 characters; anything far
/// longer is not a key.
const MAX_KEY_CHARS: usize = 8_192;

/// How many applied notes are remembered (by signature hash) so a repeat is applied once.
/// Notes come from the person's own clicks, so this is years of them.
const NOTES_REMEMBERED: usize = 4_096;

/// A change made while no server was connected, waiting to be sent as a note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingNote {
    pub key: String,
    /// true = a block, false = an unblock.
    pub block: bool,
    /// When it was made (ms), which the note is signed with.
    pub at: u64,
}

impl PendingNote {
    pub fn note(&self) -> BlockNote {
        if self.block { BlockNote::Block(self.key.clone()) } else { BlockNote::Unblock(self.key.clone()) }
    }
}

/// What is encrypted into the file.
#[derive(Debug, Default, Serialize, Deserialize)]
struct ListBody {
    /// Blocked key -> when it was blocked (ms since the epoch: the signed time of the note, or
    /// the moment of the click on this device).
    #[serde(default)]
    blocked: HashMap<String, u64>,
    /// Notes not sent yet (made offline), oldest first, one per person.
    #[serde(default)]
    pending: Vec<PendingNote>,
    /// Signature hashes of the notes applied or sent here, newest last.
    #[serde(default)]
    notes_seen: Vec<String>,
}

/// A note to oneself about the block list, read from a DM's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockNote {
    Block(String),
    Unblock(String),
}

impl BlockNote {
    /// The note's text: the marker, then the key, nothing between.
    pub fn text(&self) -> String {
        match self {
            BlockNote::Block(key) => format!("{CTL_BLOCK}{key}"),
            BlockNote::Unblock(key) => format!("{CTL_UNBLOCK}{key}"),
        }
    }

    /// Read a note, or None when the text is not one (or carries no usable key). A text that
    /// starts with either marker is still a note for `is_note`, so a malformed one is dropped
    /// rather than shown as a message.
    pub fn parse(text: &str) -> Option<Self> {
        let (key, block) = match (text.strip_prefix(CTL_BLOCK), text.strip_prefix(CTL_UNBLOCK)) {
            (Some(k), _) => (k, true),
            (_, Some(k)) => (k, false),
            _ => return None,
        };
        let usable = !key.is_empty() && key.chars().count() <= MAX_KEY_CHARS && !key.chars().any(|c| c.is_whitespace() || c.is_control());
        if !usable {
            return None;
        }
        Some(if block { BlockNote::Block(key.to_string()) } else { BlockNote::Unblock(key.to_string()) })
    }

    /// Does this DM text start with either marker? (Such a text is never shown as a message.)
    pub fn is_note(text: &str) -> bool {
        text.starts_with(CTL_BLOCK) || text.starts_with(CTL_UNBLOCK)
    }

    pub fn key(&self) -> &str {
        match self {
            BlockNote::Block(key) | BlockNote::Unblock(key) => key,
        }
    }
}

/// The block list of one identity on this device.
pub struct BlockList {
    path: PathBuf,
    key: [u8; 32],
    /// Our own identity key: never blockable, and how a loaded list is matched to the identity
    /// that is unlocked now.
    me: String,
    body: ListBody,
}

impl BlockList {
    /// Load (or start empty) the list of `identity_hex`, keyed by its seed.
    pub fn load(seed: &[u8], identity_hex: &str) -> Self {
        let tag = blake3::hash(format!("block-list\n{identity_hex}").as_bytes());
        let path = super::dm_store::DmStore::store_dir().join(format!("{}.blocks", &tag.to_hex()[..24]));
        Self::load_at(seed, identity_hex, path)
    }

    fn load_at(seed: &[u8], identity_hex: &str, path: PathBuf) -> Self {
        let mut list = Self { path, key: blake3::derive_key(LIST_KEY_DOMAIN, seed), me: identity_hex.to_string(), body: ListBody::default() };
        if let Ok(raw) = std::fs::read(&list.path) {
            if raw.len() > 12 {
                let (nonce, ct) = raw.split_at(12);
                let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&list.key));
                // A file that does not open (damaged, or another seed's) starts an empty list
                // rather than stopping the chat page; it is replaced on the next save.
                if let Ok(plain) = cipher.decrypt(Nonce::from_slice(nonce), ct) {
                    if let Ok(body) = serde_json::from_slice::<ListBody>(&plain) {
                        list.body = body;
                    }
                }
            }
        }
        list
    }

    /// A list in its own file under the temp directory, for tests (never the real dms folder).
    #[cfg(test)]
    pub(crate) fn in_temp(seed: &[u8], identity_hex: &str, tag: &str) -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        Self::load_at(seed, identity_hex, std::env::temp_dir().join(format!("hum-block-test-{tag}-{nanos}.blocks")))
    }

    #[cfg(test)]
    pub(crate) fn remove_file_for_test(&self) {
        let _ = std::fs::remove_file(&self.path);
    }

    /// Persist (encrypt, write to a temp file, rename over the old one).
    pub fn save(&self) {
        let Ok(json) = serde_json::to_vec(&self.body) else { return };
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Aes256Gcm::generate_nonce(&mut AesOsRng);
        let Ok(ct) = cipher.encrypt(&nonce, json.as_slice()) else { return };
        let mut out = Vec::with_capacity(12 + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        if std::fs::write(&tmp, &out).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }

    /// Whose list this is.
    pub fn identity(&self) -> &str {
        &self.me
    }

    pub fn is_empty(&self) -> bool {
        self.body.blocked.is_empty()
    }

    /// Is `key` blocked? Called for every message drawn, so a short list is scanned (keys differ
    /// within their first bytes, so each comparison stops almost at once) rather than hashing a
    /// 3,904-character key per message.
    pub fn is_blocked(&self, key: &str) -> bool {
        match self.body.blocked.len() {
            0 => false,
            n if n <= 64 => self.body.blocked.keys().any(|k| k == key),
            _ => self.body.blocked.contains_key(key),
        }
    }

    /// When `key` was blocked, if it is.
    pub fn blocked_at(&self, key: &str) -> Option<u64> {
        self.body.blocked.get(key).copied()
    }

    /// Block `key` as of `at`. False when nothing changed: already blocked (the first date
    /// stands), our own key, or an empty one.
    pub fn block(&mut self, key: &str, at: u64) -> bool {
        if key.is_empty() || key == self.me || self.body.blocked.contains_key(key) {
            return false;
        }
        self.body.blocked.insert(key.to_string(), at);
        true
    }

    /// Unblock `key`. False when they were not blocked.
    pub fn unblock(&mut self, key: &str) -> bool {
        self.body.blocked.remove(key).is_some()
    }

    /// Apply a note (signed at `at`). True when the list changed.
    pub fn apply(&mut self, note: &BlockNote, at: u64) -> bool {
        match note {
            BlockNote::Block(key) => self.block(key, at),
            BlockNote::Unblock(key) => self.unblock(key),
        }
    }

    /// The first time a note (by its signature hash, `DmInner::dedupe_key`) is seen here: true,
    /// and it is remembered; every later time: false. A note this device sends is remembered as
    /// it goes out, so its echo is not applied again.
    pub fn first_sight(&mut self, note_id: &str) -> bool {
        if self.body.notes_seen.iter().any(|n| n == note_id) {
            return false;
        }
        self.body.notes_seen.push(note_id.to_string());
        if self.body.notes_seen.len() > NOTES_REMEMBERED {
            let extra = self.body.notes_seen.len() - NOTES_REMEMBERED;
            self.body.notes_seen.drain(..extra);
        }
        true
    }

    /// Keep a change made while no server is connected, to send as a note on the next
    /// connection. A newer change for the same person replaces the older one.
    pub fn queue(&mut self, note: &BlockNote, at: u64) {
        let block = matches!(note, BlockNote::Block(_));
        self.body.pending.retain(|p| p.key != note.key());
        self.body.pending.push(PendingNote { key: note.key().to_string(), block, at });
    }

    /// The changes waiting to be sent, oldest first.
    pub fn pending(&self) -> &[PendingNote] {
        &self.body.pending
    }

    /// Take the waiting changes to send them (the caller queues again any that could not go).
    pub fn take_pending(&mut self) -> Vec<PendingNote> {
        std::mem::take(&mut self.body.pending)
    }

    /// Everyone blocked, newest first (then by key, so the order is stable).
    pub fn entries(&self) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self.body.blocked.iter().map(|(k, at)| (k.clone(), *at)).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE NOTES (10d): exactly `[[hum:block:v1]]<key>` and `[[hum:unblock:v1]]<key>`, the words
    /// the web client builds too, read back the same; a marker with no key, a key with spaces,
    /// or a marker not at the start is not a usable note, but a text starting with a marker is
    /// still never a message.
    /// Seen red 2026-10-09 with `CTL_UNBLOCK` spelt "[[hum:unblock:v2]]" in net/dm_pq.rs: "the
    /// unblock marker is fixed", left "[[hum:unblock:v2]]abc".
    #[test]
    fn notes_have_the_agreed_words_and_read_back() {
        assert_eq!(CTL_BLOCK, "[[hum:block:v1]]", "the block marker is fixed (the web client matches it)");
        assert_eq!(BlockNote::Block("abc".into()).text(), "[[hum:block:v1]]abc");
        assert_eq!(BlockNote::Unblock("abc".into()).text(), "[[hum:unblock:v1]]abc", "the unblock marker is fixed");
        for note in [BlockNote::Block("0f1e".repeat(976)), BlockNote::Unblock("bot_helper".into())] {
            assert_eq!(BlockNote::parse(&note.text()), Some(note.clone()), "{note:?} reads back");
            assert!(BlockNote::is_note(&note.text()));
        }
        assert_eq!(BlockNote::parse("[[hum:block:v1]]"), None, "no key");
        assert_eq!(BlockNote::parse("[[hum:block:v1]]ab cd"), None, "a key has no spaces");
        assert_eq!(BlockNote::parse("[[hum:block:v1]]ab\ncd"), None, "or line breaks");
        assert_eq!(BlockNote::parse(" [[hum:block:v1]]abc"), None, "the marker must lead");
        assert_eq!(BlockNote::parse("hello"), None);
        assert!(BlockNote::is_note("[[hum:block:v1]]"), "a malformed note is still kept out of the messages");
        assert!(!BlockNote::is_note("I said [[hum:block:v1]]abc"));
    }

    /// The list: block and unblock by key with the date; blocking again keeps the first date;
    /// never our own key; each note applies once however often it is delivered; changes made
    /// offline wait, one per person, the newer replacing the older; and the whole list survives a
    /// restart encrypted, which another seed cannot read.
    /// Seen red 2026-10-09 with `block` overwriting the date of someone already blocked:
    /// "blocking again keeps the first date" failed (left Some(200)).
    #[test]
    fn the_list_keeps_the_first_date_applies_each_note_once_and_survives_a_restart() {
        let seed = [61u8; 32];
        let mut list = BlockList::in_temp(&seed, "me", "list");
        assert!(list.is_empty() && !list.is_blocked("ann"));
        assert!(!list.block("me", 5), "never our own key");
        assert!(!list.apply(&BlockNote::Block("me".into()), 5), "a note naming ourselves counts for nothing");
        assert!(list.block("ann", 100));
        assert!(!list.apply(&BlockNote::Block("ann".into()), 200), "already blocked");
        assert_eq!(list.blocked_at("ann"), Some(100), "blocking again keeps the first date");
        assert!(list.apply(&BlockNote::Unblock("ann".into()), 300));
        assert!(!list.is_blocked("ann") && !list.unblock("ann"));
        assert!(list.apply(&BlockNote::Block("ann".into()), 301), "notes apply in the order they come");

        assert!(list.first_sight("note-1"), "a note is applied the first time");
        assert!(!list.first_sight("note-1"), "and never again, however often it is delivered");

        list.queue(&BlockNote::Block("ben".into()), 400);
        list.queue(&BlockNote::Block("cy".into()), 410);
        list.queue(&BlockNote::Unblock("ben".into()), 420);
        assert_eq!(
            list.pending(),
            [PendingNote { key: "cy".into(), block: true, at: 410 }, PendingNote { key: "ben".into(), block: false, at: 420 }],
            "one per person, the newer replacing the older"
        );

        assert!(list.block("dee", 500));
        assert_eq!(list.entries(), vec![("dee".to_string(), 500), ("ann".to_string(), 301)], "newest first");
        list.save();
        let mut again = BlockList::load_at(&seed, "me", list.path.clone());
        assert_eq!(again.entries(), list.entries(), "kept across a restart");
        assert_eq!(again.pending().len(), 2, "with the notes still to send");
        assert!(!again.first_sight("note-1"), "and the notes already applied");
        assert_eq!(again.take_pending().len(), 2);
        assert!(again.pending().is_empty());
        let raw = std::fs::read(&list.path).unwrap();
        assert!(!raw.windows(3).any(|w| w == b"dee"), "the file names nobody in the clear");
        let other = BlockList::load_at(&[62u8; 32], "me", list.path.clone());
        assert!(other.is_empty(), "another seed reads nothing");
        list.remove_file_for_test();
    }
}
