//! Removing someone from a P2P group I created (section 10j of docs/design/blocking-and-safe-mode.md,
//! "Remove them from the group", 2026-10-10), with no GUI so the order of what is posted is tested.
//!
//! WHY THE KEY GOES FIRST. A group's messages are encrypted under its current key, and everyone
//! the key was sealed to keeps it. The creator's existing rekey (`api_v2::rekey_if_creator_needs`)
//! only adds people who joined since; it never leaves anyone out. So removing someone from the
//! member list alone would leave them able to read everything said from then on. The removal
//! therefore first posts a NEW group key (the next epoch) sealed to every remaining member,
//! leaving the person out even while the server still lists them, and only then the signed
//! `group_member_v1` remove. A key that cannot be made (the group's current key or its member
//! list cannot be read, the key cannot be sealed, the server refuses it) removes no one: nobody
//! is ever "removed" yet still able to read. The web client does the same in the same order
//! (web/chat/chat-groups-p2p.js `rotateP2pGroupKey`, `removeP2pMember`). While the removal is on
//! its way, the creator's own rekey never seals a key to the person removed (`left_out`); once
//! the server's member list has dropped them, someone it lists again has rejoined with a new
//! ticket and is sealed to like any member.
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

// ── Who this device removed, for this run ───────────────────────────────────────────────────

/// The people this device is removing from each group in this run of the app. The creator's rekey
/// (`api_v2::rekey_if_creator_needs`) runs on a worker thread every couple of seconds while the
/// group is open, and seals a new key to every member the server LISTS who lacks one. Between our
/// new key (which leaves the person out) and our removal reaching the server, the person is still
/// listed and lacks the new key, so the rekey would seal them one at once and undo the removal. It
/// reads this list (`left_out`) and never seals to anyone on it while their removal is on its way.
/// Process-wide because the rekey runs where no app state is reachable; kept only for the run,
/// since after a restart the server's list no longer names them.
///
/// Once a member list the server gave no longer names them, the removal has landed, and from a
/// list asked for after that, someone named again has REJOINED (a new invite ticket) and is sealed
/// to like any member (`left_out`). Without that, a person removed and let back in was left out of
/// every new key until the app restarted. "Asked for after" matters: two rekeys can overlap, and a
/// list asked for before the removal landed still names them, so it must not count as a rejoin.
static REMOVED_HERE: std::sync::Mutex<Vec<Removed>> = std::sync::Mutex::new(Vec::new());

struct Removed {
    group_id: String,
    /// Lower-case hex.
    key: String,
    /// When a member list without them came back: the removal had landed by then.
    gone_seen: Option<std::time::Instant>,
}

/// Note that this device is removing `member` from `group_id`. True when this is a new removal
/// (so one that fails can be taken back with `forget_removed`): they were not on the list, or an
/// earlier removal of theirs had landed, which means they are back and being removed again.
pub fn note_removed(group_id: &str, member: &str) -> bool {
    let key = member.trim().to_ascii_lowercase();
    let mut list = REMOVED_HERE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = list.iter_mut().find(|r| r.group_id == group_id && r.key == key) {
        // Removed again after rejoining: wait for this removal to land too.
        return r.gone_seen.take().is_some();
    }
    list.push(Removed { group_id: group_id.to_string(), key, gone_seen: None });
    true
}

/// Take `member` off the list for `group_id` (a removal that changed nothing after all).
pub fn forget_removed(group_id: &str, member: &str) {
    let key = member.trim().to_ascii_lowercase();
    REMOVED_HERE.lock().unwrap_or_else(|e| e.into_inner()).retain(|r| !(r.group_id == group_id && r.key == key));
}

/// The people this device removed from `group_id` in this run and has not seen rejoin (lower-case
/// hex keys).
pub fn removed_here(group_id: &str) -> Vec<String> {
    REMOVED_HERE.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|r| r.group_id == group_id).map(|r| r.key.clone()).collect()
}

/// Whom a creator's rekey leaves out of `group_id`, given the member list (`roster`, each key hex
/// and its Kyber key) the server returned to a request made at `asked`. Someone the list no
/// longer names has had their removal land: noted, and left on the list for now (a slower list,
/// asked for earlier, may still name them). Someone it names again, asked for after a list
/// without them came back, has rejoined: they leave the list and are sealed to normally.
pub fn left_out(group_id: &str, roster: &[(String, Option<String>)], asked: std::time::Instant) -> Vec<String> {
    let mut list = REMOVED_HERE.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    list.retain_mut(|r| {
        if r.group_id != group_id {
            return true;
        }
        let named = roster.iter().any(|(k, _)| k.trim().eq_ignore_ascii_case(&r.key));
        match (named, r.gone_seen) {
            (false, None) => {
                r.gone_seen = Some(now);
                true
            }
            (true, Some(seen)) if asked > seen => false, // rejoined
            _ => true,
        }
    });
    list.iter().filter(|r| r.group_id == group_id).map(|r| r.key.clone()).collect()
}

/// Remove `member` (their key, hex) from `group_id`, signed with the identity of `seed` (the
/// group's creator: the server accepts a group key and another person's removal only from them).
/// A new group key sealed to everyone else goes first; then the removal. Returns the new epoch and
/// its key, so an open view of the group sends under it at once.
pub fn remove_member(server: &mut impl GroupServer, seed: &[u8], group_id: &str, member: &str) -> Result<(u64, Vec<u8>), RemoveError> {
    let gone = super::group_report::key_norm(member).ok_or_else(|| RemoveError::NoKey("not a key".into()))?;
    // The current epoch, so the new key is the next one. ONLY the server saying it has none means
    // this is the first (epoch 1). A failure to ask, or a key it holds that does not read, removes
    // no one: guessing "none" there would post epoch 1 over the group's real current key, a
    // smaller number than the key everyone is using (the 2026-10-10 review).
    let current = match server.epoch_payload(group_id).map_err(RemoveError::NoKey)? {
        None => 0,
        Some(payload) => parse_group_epoch_key_payload(&payload).map_err(RemoveError::NoKey)?.epoch,
    };
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
    // On the list BEFORE the new key goes out, so a rekey that runs between the key and the
    // removal cannot seal them a copy (`removed_here`).
    let newly = note_removed(group_id, &gone);
    let posted = build_group_epoch_key_v1(group_id, epoch, &key, &sealable)
        .and_then(|builder| api_v2::sign_submission(seed, builder))
        .and_then(|(_, epoch_json)| server.post(&epoch_json));
    if let Err(e) = posted {
        // No new key: no one was removed, so they stay a member whom later keys must reach.
        if newly {
            forget_removed(group_id, &gone);
        }
        return Err(RemoveError::NoKey(e));
    }
    // From here they stay on the list even if the server refuses the removal: the new key went
    // out without them, and a later rekey must not hand it to them.
    // Only now the removal: the same signed `group_member_v1` a leave posts, with them as subject.
    let subject = hex::decode(&gone).map_err(|e| RemoveError::Refused(e.to_string()))?;
    let removal = api_v2::member_remove_builder(group_id, &subject).map_err(RemoveError::Refused)?;
    let (_, removal_json) = api_v2::sign_submission(seed, removal).map_err(RemoveError::Refused)?;
    server.post(&removal_json).map_err(RemoveError::Refused)?;
    Ok((epoch, key))
}
