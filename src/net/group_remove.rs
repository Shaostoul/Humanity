//! Removing someone from a P2P group I created (section 10j of docs/design/blocking-and-safe-mode.md,
//! "Remove them from the group", 2026-10-10), with no GUI so the order of what is posted is tested.
//!
//! WHY THE KEY GOES FIRST. A group's messages are encrypted under its current key, and everyone
//! the key was sealed to keeps it. The creator's existing rekey (`api_v2::rekey_if_creator_needs`)
//! only adds people who joined since; it never leaves anyone out. So removing someone from the
//! member list alone would leave them able to read everything said from then on. The removal
//! therefore first posts a NEW group key (the next epoch) sealed to every remaining member,
//! leaving the person out even while the server still lists them, and only then the signed
//! `group_member_v1` remove. A key that cannot be made (the member list cannot be read, the key
//! cannot be sealed, the server refuses it) removes no one: nobody is ever "removed" yet still
//! able to read. The web client does the same in the same order (web/chat/chat-groups-p2p.js
//! `rotateP2pGroupKey`, `removeP2pMember`).
//!
//! They keep the keys to what they already saw: a new key protects only what is said next.
//!
//! The server is reached through `GroupServer`, so a test can record every request in order; the
//! app's own is `Http`, over the same endpoints the rest of the group code uses (net/api_v2.rs).

use super::api_v2;
use super::group_e2ee::{build_group_epoch_key_v1, parse_group_epoch_key_payload, random_epoch_key, GroupMemberKey};

/// The three things a removal asks of the server.
pub trait GroupServer {
    /// The group's latest `group_epoch_key_v1` payload, or None when it has none yet.
    fn epoch_payload(&mut self, group_id: &str) -> Result<Option<Vec<u8>>, String>;
    /// The group's members as the server lists them: each key (hex) and its Kyber key, if known.
    fn members(&mut self, group_id: &str) -> Result<Vec<(String, Option<String>)>, String>;
    /// Post one signed object (its submission JSON).
    fn post(&mut self, submission_json: &str) -> Result<(), String>;
}

/// The app's server: the group endpoints of `server_url`.
pub struct Http<'a>(pub &'a str);

impl GroupServer for Http<'_> {
    fn epoch_payload(&mut self, group_id: &str) -> Result<Option<Vec<u8>>, String> {
        api_v2::fetch_group_epoch_payload(self.0, group_id)
    }
    fn members(&mut self, group_id: &str) -> Result<Vec<(String, Option<String>)>, String> {
        api_v2::fetch_group_members(self.0, group_id)
    }
    fn post(&mut self, submission_json: &str) -> Result<(), String> {
        api_v2::post_submission_json(self.0, submission_json)
    }
}

/// Why a removal did not go through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveError {
    /// The new group key could not be made or was not accepted, so no one was removed.
    NoKey(String),
    /// The new key went out, but the server refused the removal itself.
    Refused(String),
}

impl RemoveError {
    /// What the creator is told (the web's words).
    pub fn sentence(&self, name: &str, group: &str) -> String {
        match self {
            RemoveError::NoKey(_) => format!("Could not remove {name} from {group}: a new group key could not be made."),
            RemoveError::Refused(_) => format!("Could not remove {name} from {group}: the server refused it."),
        }
    }
}

/// Remove `member` (their key, hex) from `group_id`, signed with the identity of `seed` (the
/// group's creator: the server accepts a group key and another person's removal only from them).
/// A new group key sealed to everyone else goes first; then the removal. Returns the new epoch and
/// its key, so an open view of the group sends under it at once.
pub fn remove_member(server: &mut impl GroupServer, seed: &[u8], group_id: &str, member: &str) -> Result<(u64, Vec<u8>), RemoveError> {
    let gone = super::group_report::key_norm(member).ok_or_else(|| RemoveError::NoKey("not a key".into()))?;
    // The current epoch, so the new key is the next one. None yet (or unreadable): this is the first.
    let current = server
        .epoch_payload(group_id)
        .ok()
        .flatten()
        .and_then(|p| parse_group_epoch_key_payload(&p).ok())
        .map(|p| p.epoch)
        .unwrap_or(0);
    let roster = server.members(group_id).map_err(RemoveError::NoKey)?;
    let sealable: Vec<GroupMemberKey> = roster
        .into_iter()
        // Left out even while the server still lists them: the remove has not been posted yet.
        .filter(|(key, _)| !key.eq_ignore_ascii_case(&gone))
        .filter_map(|(key, kyber)| {
            let bytes = hex::decode(&key).ok()?;
            Some(GroupMemberKey { fp: api_v2::author_fingerprint_hex(&bytes), kyber_pub_b64: kyber? })
        })
        .collect();
    let epoch = current + 1;
    let key = random_epoch_key();
    let builder = build_group_epoch_key_v1(group_id, epoch, &key, &sealable).map_err(RemoveError::NoKey)?;
    let (_, epoch_json) = api_v2::sign_submission(seed, builder).map_err(RemoveError::NoKey)?;
    server.post(&epoch_json).map_err(RemoveError::NoKey)?;
    // Only now the removal: the same signed `group_member_v1` a leave posts, with them as subject.
    let subject = hex::decode(&gone).map_err(|e| RemoveError::Refused(e.to_string()))?;
    let removal = api_v2::member_remove_builder(group_id, &subject).map_err(RemoveError::Refused)?;
    let (_, removal_json) = api_v2::sign_submission(seed, removal).map_err(RemoveError::Refused)?;
    server.post(&removal_json).map_err(RemoveError::Refused)?;
    Ok((epoch, key))
}
