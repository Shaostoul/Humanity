//! The owner's changes that re-wrap every held copy, the storage half (2026-11-20, docs/design/
//! hide-and-keep.md, "How the owner role is set"): a HANDOVER of the role while the owner is alive,
//! and NAMING, CHANGING or REMOVING the successor (storage/owner.rs). A child of storage/owner.rs
//! (#[path]); the handler is handlers/owner.rs.
//!
//! WHY EACH IS MORE THAN A ROW. Every held copy is sealed to the current owner's key card (and the
//! successor's, when one is named) and opens for nobody else. Each part's body is sealed under its
//! own part key, and its header wraps that key once per recipient (core `held_seal`, format v4). So
//! a change RE-WRAPS, it never re-encrypts (the owner branch's second review, 2026-11-20, which
//! replaced the re-seal flow): the OWNER'S DEVICE fetches each part's header alone, opens its own
//! slot, wraps the same key for the new set of recipients ([`OwnerChange::recipients`]: the new
//! owner alone for a handover, the owner and the successor for a naming, the owner alone for a
//! removal) and sends the new header ([`take_new_header`]). The relay never takes a body byte during
//! a change and keeps every body exactly as it is: a new header is taken only when it is a header
//! alone, names the right number of recipients, and carries the very KEY COMMITMENT of the header
//! it replaces (the body's pieces are sealed under the key that commitment names), so no device can
//! swap what a copy holds. No restore key travels: part 0's body never changes.
//!
//! TWO PHASES. Once every new header is in, the relay builds each new part beside the original
//! (the new header, then the original's body copied byte for byte, [`build_new_parts`]) and changes
//! nothing yet. Each recipient's own device then opens ONLY its own slot of every new header, checks
//! the commitment, and signs an acceptance of [`expected_acceptance`]: the digest of every new
//! header's hash and the change's ROUND (a fresh id per begin), which the relay works out from the
//! headers it staged ([`Storage::check_owner_accept`]; handlers/owner_accept.rs).
//!
//! ONE STEP OR NONE ([`Storage::owner_change_in`]). The new parts wait in `.handover-<package>`
//! beside the packages (never served to anyone; a recipient is served the new headers alone).
//! Carrying the change out, once accepted, writes a marker naming it, swaps each package's folder
//! for its new one (the old one kept as `.pre-handover-<package>`), and then, in ONE transaction,
//! moves the role or the successor, records every held item as sealed to the new recipients, and
//! writes the change into the owner log. Any failure puts every folder back; a stop part-way is
//! finished or undone when the relay next starts ([`Storage::owner_handover_recover_in`]): the
//! marker's change in the log means it happened. Every step that WRITES, renames or removes in these
//! folders holds the storage's one held-folder lock (`Storage::held_swap`), so none of them runs
//! inside another (the second review's R4; the third review's S1 found the new headers and the new
//! parts taken without it, now [`Storage::owner_take_new_header`] and
//! [`Storage::owner_build_new_parts`]): taking a new header, building the new parts, the swap,
//! clearing a change's new parts, the held folder's sweep, destroying a copy, and sealing a hidden
//! post's copy. Reads a recipient's check makes (a new header served) take no lock: the carry-out
//! re-reads every new header under the lock and requires EVERY recorded acceptance to be the digest
//! of exactly what it carries out ([`Storage::owner_change_in`]), so nothing a reader saw can
//! differ from what is carried out unnoticed.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use humanity_protocol::held_seal;
use humanity_protocol::key_card;
use humanity_protocol::moderation::{OWNER_ACCEPT, OWNER_ACTIONS};
use humanity_protocol::object::Object;
use rusqlite::params;

use super::super::files::Folders;
use super::super::held::{held_dir_beside, part_file, HeldRow};
use super::super::moderation::payload_text;
use super::OWNER_ROLE;
use crate::relay::storage::Storage;

#[cfg(test)]
thread_local! {
    /// A test's hook, called on the thread carrying a change out once every folder is swapped and
    /// before the transaction (the second review's R4 race test: the sweep must wait for it).
    pub(crate) static SWAP_HOOK: std::cell::RefCell<Option<Box<dyn FnMut()>>> = const { std::cell::RefCell::new(None) };
}

/// How far an owner change's own time may be from the relay's clock when it starts.
pub const HANDOVER_FRESH_MS: i64 = 5 * 60 * 1000;

/// The start of a package's new folder's name while its new headers arrive and its new parts wait.
pub const STAGING: &str = ".handover-";
/// The start of a package's own folder's name while a change swaps it out.
pub const BEFORE: &str = ".pre-handover-";
/// The file naming a change while its folders are swapped.
pub const MARKER: &str = ".handover.json";
/// How long a new folder may sit unchanged before the held folder's sweep removes it: longer than a
/// change's re-wrap and acceptance windows together (handlers/owner.rs), so no change that can still
/// be carried out loses its new parts to the sweep.
pub const STAGING_LIFETIME_MS: i64 = 2 * 60 * 60 * 1000;

/// Which change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// `owner_handover`: the role to `target`.
    Handover,
    /// `owner_successor`: `target` named the successor.
    Successor,
    /// `owner_successor_remove`: the successor `target` removed.
    SuccessorRemove,
}

impl ChangeKind {
    /// The signed action's name, which is also its owner log `kind`.
    pub fn action(self) -> &'static str {
        match self {
            ChangeKind::Handover => "owner_handover",
            ChangeKind::Successor => "owner_successor",
            ChangeKind::SuccessorRemove => "owner_successor_remove",
        }
    }
}

/// An owner change that checked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerChange {
    pub kind: ChangeKind,
    pub object_id: String,
    pub owner: String,
    /// The member the change names: the new owner, the new successor, or the successor removed.
    pub target: String,
    /// Whom every part is sealed to afterwards, in slot order (the owner first), with the time of
    /// the card each is sealed to.
    pub recipients: Vec<(String, u64)>,
    pub reason: String,
    pub signed_at: i64,
}

impl OwnerChange {
    /// Whether the owner is the only person who checks this change (a successor removal): the
    /// owner log says so ("checked by the owner alone").
    pub fn owner_alone(&self) -> bool {
        self.recipients.len() == 1 && self.recipients[0].0 == self.owner
    }
}

/// The new folder of `package`.
pub fn staging_dir(folders: Folders, package: &str) -> PathBuf {
    held_dir_beside(folders).join(format!("{STAGING}{package}"))
}

/// The file a new header of part `part` waits in until the new part is built.
pub fn header_file(part: usize) -> String {
    format!("{part}.header")
}

/// The header at the start of the file `path`: exactly its bytes (as many as its count byte says).
pub fn read_header(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut f = std::fs::File::open(path)?;
    let mut head = vec![0u8; held_seal::MAGIC.len() + 1];
    f.read_exact(&mut head)?;
    let n = head[held_seal::MAGIC.len()] as usize;
    if &head[..held_seal::MAGIC.len()] != held_seal::MAGIC || n == 0 || n > held_seal::MAX_RECIPIENTS {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a sealed held part"));
    }
    head.resize(held_seal::header_len(n), 0);
    f.read_exact(&mut head[held_seal::MAGIC.len() + 1..])?;
    Ok(head)
}

impl Storage {
    /// Check the signed owner action in `object` sent by the signed-in `socket_key` at `now_ms`: the
    /// signature, the signer (the one owner, signed in), this server, the time, that it was never
    /// used, and the member it names (for a handover or a naming: a member here other than the
    /// owner, not banned, with a key card that checks out, since every held copy will be sealed to
    /// it; for a removal: the successor named now). Why not, in words.
    pub fn check_owner_change(&self, object: &Object, socket_key: &str, now_ms: i64) -> Result<OwnerChange, String> {
        let action = payload_text(object, "action").unwrap_or_default();
        if object.object_type != "mod_action_v1" || !OWNER_ACTIONS.contains(&action.as_str()) {
            return Err("this is not one of the owner's signed actions".into());
        }
        object.verify_signature().map_err(|_| "the action's signature does not check out".to_string())?;
        let owner = hex::encode(&object.author_public_key);
        if owner != socket_key {
            return Err("the action was signed by another key than the one signed in".into());
        }
        if self.owner_keys().unwrap_or_default() != [owner.clone()] {
            return Err("only this server's owner can do that".into());
        }
        if object.space_id.as_deref() != self.server_did().ok().as_deref() {
            return Err("the action names another server".into());
        }
        let signed_at = object.created_at.unwrap_or(0) as i64;
        if (signed_at - now_ms).abs() > HANDOVER_FRESH_MS {
            return Err("the action is too old or dated ahead; check this device's clock and try again".into());
        }
        if payload_text(object, "target_kind").as_deref() != Some("identity") {
            return Err("the action names a person".into());
        }
        let reason = payload_text(object, "reason").unwrap_or_default();
        if reason.trim().is_empty() {
            return Err("the action carries its reason".into());
        }
        let object_id = object.object_id().map_err(|_| "the action cannot be named".to_string())?.to_hex();
        if self.owner_handover_used(&object_id).map_err(|e| format!("the server could not read its log ({e})"))? {
            return Err("this action was already carried out once".into());
        }
        let target = payload_text(object, "target").unwrap_or_default();
        let kind = match action.as_str() {
            "owner_handover" => ChangeKind::Handover,
            "owner_successor" => ChangeKind::Successor,
            _ => ChangeKind::SuccessorRemove,
        };
        let no_card = || "has no key card here that checks out, so held copies could not be sealed to them. Their key card arrives the next time they sign in with the app or the website".to_string();
        let owner_card = self.checked_card_at(&owner).ok_or_else(|| {
            "your own key card has not reached this server, so held copies could not be sealed to you. It arrives when you sign in with the app or the website".to_string()
        })?;
        let recipients = match kind {
            ChangeKind::SuccessorRemove => {
                if self.successor().as_deref() != Some(target.as_str()) {
                    return Err("the person named is not the successor".into());
                }
                vec![(owner.clone(), owner_card)]
            }
            ChangeKind::Handover | ChangeKind::Successor => {
                if target == owner {
                    return Err(if kind == ChangeKind::Handover { "you already hold the owner role" } else { "the owner cannot be their own successor" }.into());
                }
                if self.name_for_key(&target).ok().flatten().is_none_or(|n| n.is_empty()) {
                    return Err("the person named is not a member of this server".into());
                }
                if self.is_banned(&target).unwrap_or(true) {
                    return Err("the person named is banned here".into());
                }
                let card_at = self.checked_card_at(&target).ok_or_else(|| format!("the person named {}", no_card()))?;
                if kind == ChangeKind::Handover {
                    vec![(target.clone(), card_at)]
                } else {
                    vec![(owner.clone(), owner_card), (target.clone(), card_at)]
                }
            }
        };
        Ok(OwnerChange { kind, object_id, owner, target, recipients, reason, signed_at })
    }

    /// The time of `key`'s key card here when it checks out (signed by that very key and naming
    /// it), else None.
    pub fn checked_card_at(&self, key: &str) -> Option<u64> {
        let card = self.get_key_card(key).ok().flatten()?;
        (key_card::card_for(&card, key) && key_card::kyber_bytes(&card).is_some()).then_some(card.at)
    }

    /// Check a recipient's signed acceptance `object` of the pending change `c`, sent by the
    /// signed-in `socket_key` at `now_ms` (`json` as it arrived): the signature, the signer (a
    /// recipient of `c`, signed in), the action, this server, the time, that it names `c` and was
    /// never used, and its reason `checked <digest>`. The acceptance, or why not in words.
    pub fn check_owner_accept(&self, object: &Object, socket_key: &str, now_ms: i64, c: &OwnerChange, json: &str) -> Result<Acceptance, String> {
        if object.object_type != "mod_action_v1" || payload_text(object, "action").as_deref() != Some(OWNER_ACCEPT) {
            return Err("this is not an acceptance of an owner change".into());
        }
        object.verify_signature().map_err(|_| "the acceptance's signature does not check out".to_string())?;
        let key = hex::encode(&object.author_public_key);
        if key != socket_key {
            return Err("the acceptance was signed by another key than the one signed in".into());
        }
        if !c.recipients.iter().any(|(k, _)| *k == key) {
            return Err("the change does not seal the copies to you".into());
        }
        if object.space_id.as_deref() != self.server_did().ok().as_deref() {
            return Err("the acceptance names another server".into());
        }
        if (object.created_at.unwrap_or(0) as i64 - now_ms).abs() > HANDOVER_FRESH_MS {
            return Err("the acceptance is too old or dated ahead; check this device's clock".into());
        }
        if payload_text(object, "target_kind").as_deref() != Some("object") || payload_text(object, "target").as_deref() != Some(c.object_id.as_str()) {
            return Err("the acceptance names another change".into());
        }
        let digest = payload_text(object, "reason").unwrap_or_default().strip_prefix("checked ").map(str::to_string).filter(|d| d.len() == 64).ok_or_else(|| "the acceptance does not say what was checked".to_string())?;
        let object_id = object.object_id().map_err(|_| "the acceptance cannot be named".to_string())?.to_hex();
        if self.owner_handover_used(&object_id).map_err(|e| format!("the server could not read its log ({e})"))? {
            return Err("this acceptance was already used once".into());
        }
        Ok(Acceptance { key, object_id, digest, json: json.to_string() })
    }

    /// Carry out the change `c` of round `round` (the module's header) with the held items `rows`
    /// (those it still covers: `ids` were covered when it began), whose new parts
    /// [`Storage::owner_build_new_parts`] built and every recipient accepted (`accepted`), at
    /// `now_ms`; `object_json` is the signed action as it arrived. Nothing changes unless all of it
    /// does. Holds the held-folder lock throughout, and under it works out
    /// [`expected_acceptance`] from the new parts as they are NOW and requires every recorded
    /// acceptance to carry exactly that digest (the third review's S1: an acceptance recorded earlier
    /// was never compared again, so headers changed after a recipient checked them could have been
    /// carried out under their name).
    #[allow(clippy::too_many_arguments)]
    pub fn owner_change_in(&self, c: &OwnerChange, round: &str, rows: &[HeldRow], ids: &[i64], object_json: &str, accepted: &[Acceptance], now_ms: i64, folders: Folders) -> Result<(), String> {
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        let held = held_dir_beside(folders);
        if rows.iter().any(|r| !ids.contains(&r.id)) {
            return Err("a post was held while it ran, so its copy was not re-wrapped; start again. Nothing was changed".into());
        }
        let want = expected_acceptance(round, rows, folders).map_err(|why| format!("{why}; start again. Nothing was changed"))?;
        if accepted.len() != c.recipients.len() || c.recipients.iter().any(|(k, _)| !accepted.iter().any(|a| &a.key == k && a.digest == want)) {
            return Err("the re-wrapped copies are not exactly what everyone who accepted checked, so the change was not carried out; start again. Nothing was changed".into());
        }
        // Every new part must be in place before anything moves.
        for row in rows {
            for part in 0..row.parts.max(0) as usize {
                if !staging_dir(folders, &row.package).join(part_file(part)).is_file() {
                    return Err(format!("held item #{}'s new part {part} is missing; start again. Nothing was changed", row.id));
                }
            }
        }
        if !rows.is_empty() {
            std::fs::create_dir_all(&held).map_err(|e| format!("the held folder could not be reached ({e})"))?;
            let marker = serde_json::json!({ "object_id": c.object_id, "packages": rows.iter().map(|r| r.package.clone()).collect::<Vec<_>>() });
            write_synced(&held.join(MARKER), marker.to_string().as_bytes())?;
            sync_dir(&held);
        }
        let mut swapped: Vec<&str> = Vec::new();
        // Put every swapped package back. The marker goes only when every rename back worked: if one
        // failed, the next start's recovery finds the marker and finishes the undo (the owner
        // review's finding 9).
        let undo = |swapped: &[&str]| {
            let mut all_back = true;
            for package in swapped.iter().rev() {
                let (home, before) = (held.join(package), held.join(format!("{BEFORE}{package}")));
                if before.is_dir() {
                    let _ = std::fs::rename(&home, held.join(format!("{STAGING}{package}")));
                    all_back &= std::fs::rename(&before, &home).is_ok();
                }
            }
            sync_dir(&held);
            if all_back {
                let _ = std::fs::remove_file(held.join(MARKER));
            } else {
                tracing::error!("Owner change: a held copy could not be put back; the next start finishes it (the marker is kept)");
            }
        };
        for row in rows {
            let package = row.package.as_str();
            let (home, staging, before) = (held.join(package), held.join(format!("{STAGING}{package}")), held.join(format!("{BEFORE}{package}")));
            let step = copy_synced(&home.join(held_seal::RESTORE_PLAN_FILE), &staging.join(held_seal::RESTORE_PLAN_FILE))
                .and_then(|_| std::fs::rename(&home, &before))
                .and_then(|_| {
                    std::fs::rename(&staging, &home).inspect_err(|_| {
                        let _ = std::fs::rename(&before, &home);
                    })
                });
            if let Err(e) = step {
                undo(&swapped);
                return Err(format!("a held copy's folder could not be swapped ({e}). Nothing was changed"));
            }
            swapped.push(package);
        }
        sync_dir(&held);
        #[cfg(test)]
        SWAP_HOOK.with(|h| {
            if let Some(f) = h.borrow_mut().as_mut() {
                f();
            }
        });
        let swapped_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        let (first, also) = (c.recipients[0].clone(), c.recipients.get(1).map(|r| r.0.clone()).unwrap_or_default());
        // A change only the owner checked says so in the log (the second review: the owner is then
        // the one person whose device vouched for the new headers).
        let reason = if c.owner_alone() { format!("{} (checked by the owner alone)", c.reason) } else { c.reason.clone() };
        let done = self.with_conn_mut(|conn| -> Result<Result<(), String>, rusqlite::Error> {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let owners: Vec<String> = {
                let mut st = tx.prepare("SELECT public_key FROM user_roles WHERE role = 'owner'")?;
                let keys = st.query_map([], |r| r.get::<_, String>(0))?;
                keys.collect::<Result<_, _>>()?
            };
            if owners != [c.owner.clone()] {
                return Ok(Err("the owner role changed while it ran".into()));
            }
            // The very set of held items swapped, not merely as many (the owner review's finding 9: a
            // count matched when one item was destroyed and another held while it ran).
            let mut held_now: Vec<i64> = {
                let mut st = tx.prepare("SELECT id FROM held_items")?;
                let ids = st.query_map([], |r| r.get::<_, i64>(0))?;
                ids.collect::<Result<_, _>>()?
            };
            held_now.sort_unstable();
            let mut want = swapped_ids.clone();
            want.sort_unstable();
            if held_now != want {
                return Ok(Err("the held items changed while it ran (a post held, or a copy destroyed or restored); start again".into()));
            }
            match c.kind {
                ChangeKind::Handover => {
                    tx.execute("UPDATE user_roles SET role = 'admin' WHERE public_key = ?1", params![c.owner])?;
                    tx.execute(
                        "INSERT INTO user_roles (public_key, role) VALUES (?1, ?2) ON CONFLICT(public_key) DO UPDATE SET role = ?2",
                        params![c.target, OWNER_ROLE],
                    )?;
                    tx.execute("DELETE FROM owner_successor", [])?;
                }
                ChangeKind::Successor => {
                    let card_at = c.recipients[1].1 as i64;
                    tx.execute(
                        "INSERT INTO owner_successor (id, public_key, card_at, named_at) VALUES (1, ?1, ?2, ?3)
                         ON CONFLICT(id) DO UPDATE SET public_key = ?1, card_at = ?2, named_at = ?3",
                        params![c.target, card_at, now_ms],
                    )?;
                }
                ChangeKind::SuccessorRemove => {
                    tx.execute("DELETE FROM owner_successor", [])?;
                }
            }
            for id in &swapped_ids {
                tx.execute("UPDATE held_items SET sealed_to = ?2, card_at = ?3, also_sealed_to = ?4 WHERE id = ?1", params![id, first.0, first.1 as i64, also])?;
            }
            tx.execute(
                "INSERT INTO owner_log (kind, old_key, new_key, object_id, reason, at, object) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![c.kind.action(), c.owner, c.target, c.object_id, reason, now_ms, object_json],
            )?;
            // Each recipient's signed acceptance, kept beside the change (and never usable again).
            for a in accepted {
                tx.execute(
                    "INSERT INTO owner_log (kind, old_key, new_key, object_id, reason, at, object) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![OWNER_ACCEPT, c.owner, a.key, a.object_id, format!("checked {}", a.digest), now_ms, a.json],
                )?;
            }
            tx.commit()?;
            Ok(Ok(()))
        });
        match done {
            Ok(Ok(())) => {
                for package in &swapped {
                    let _ = std::fs::remove_dir_all(held.join(format!("{BEFORE}{package}")));
                }
                // Every new folder left, those of items that went while it ran (destroyed, expired,
                // restored) included: never left behind (the owner review's finding 7).
                clear_staging_unlocked(folders);
                let _ = std::fs::remove_file(held.join(MARKER));
                sync_dir(&held);
                Ok(())
            }
            Ok(Err(why)) => {
                undo(&swapped);
                Err(format!("{why}. Nothing was changed"))
            }
            Err(e) => {
                undo(&swapped);
                Err(format!("the server could not save it ({e}). Nothing was changed"))
            }
        }
    }

    /// When the relay starts: finish or undo a change a stop interrupted (the module's header), and
    /// remove new folders no change is waiting for. Whether a change was found.
    pub fn owner_handover_recover_in(&self, folders: Folders) -> bool {
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        let held = held_dir_beside(folders);
        let found = match std::fs::read(held.join(MARKER)).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
            Some(marker) => {
                let object_id = marker["object_id"].as_str().unwrap_or("");
                let happened = !object_id.is_empty() && self.owner_handover_used(object_id).unwrap_or(false);
                let mut all_done = true;
                for package in marker["packages"].as_array().into_iter().flatten().filter_map(|p| p.as_str()) {
                    let before = held.join(format!("{BEFORE}{package}"));
                    if !before.is_dir() {
                        continue;
                    }
                    if happened {
                        let _ = std::fs::remove_dir_all(&before);
                    } else {
                        let _ = std::fs::remove_dir_all(held.join(package));
                        if let Err(e) = std::fs::rename(&before, held.join(package)) {
                            all_done = false;
                            tracing::error!("Owner change: could not put held copy {package} back ({e}); it is in {}", before.display());
                        }
                    }
                }
                sync_dir(&held);
                // Kept while a copy could not be put back, so the next start tries again.
                if all_done {
                    let _ = std::fs::remove_file(held.join(MARKER));
                }
                tracing::warn!("Owner change: one was interrupted and has been {}", if happened { "finished" } else { "undone" });
                true
            }
            None => false,
        };
        clear_staging_unlocked(folders);
        found
    }

    /// Remove every new folder (a change that was not finished; started again from the top), under
    /// the held-folder lock.
    pub fn owner_handover_clear_staging_in(&self, folders: Folders) {
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        clear_staging_unlocked(folders);
    }
}

/// Remove every new folder; the caller holds the held-folder lock.
fn clear_staging_unlocked(folders: Folders) {
    let Ok(entries) = std::fs::read_dir(held_dir_beside(folders)) else { return };
    for e in entries.filter_map(|e| e.ok()) {
        if e.file_name().to_string_lossy().starts_with(STAGING) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// A recipient's acceptance of a change, checked ([`Storage::check_owner_accept`]): who, the signed
/// object's id and JSON, and the digest of the headers their device checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acceptance {
    pub key: String,
    pub object_id: String,
    pub digest: String,
    pub json: String,
}

/// Why a new header was not taken ([`take_new_header`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderRefused {
    /// More (or fewer) bytes than a header for that many people: a change never takes body bytes.
    NotAHeader,
    /// It does not start as a sealed part does, or names another number of recipients.
    Mismatch,
    /// Its key commitment is not the one of the header it replaces: it would not head the body.
    Commitment,
    /// The disk refused.
    Disk(String),
}

impl Storage {
    /// [`take_new_header`] under the held-folder lock (the module's header).
    pub fn owner_take_new_header(&self, row: &HeldRow, folders: Folders, part: usize, recipients: usize, bytes: &[u8]) -> Result<(), HeaderRefused> {
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        take_new_header(row, folders, part, recipients, bytes)
    }

    /// [`build_new_parts`] under the held-folder lock (the module's header).
    pub fn owner_build_new_parts(&self, rows: &[HeldRow], recipients: usize, folders: Folders) -> Result<(), String> {
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        build_new_parts(rows, recipients, folders)
    }
}

/// Take the new header `bytes` of part `part` of held item `row`, sealed to `recipients` people
/// (the module's header): exactly a header for that many people (a change never takes body bytes),
/// carrying the commitment of the part's header now. It waits as `<part>.header` in the item's new
/// folder; sent again, it replaces the one before. The caller holds the held-folder lock
/// ([`Storage::owner_take_new_header`]); tests call it bare to stand in for a write by any path.
pub(crate) fn take_new_header(row: &HeldRow, folders: Folders, part: usize, recipients: usize, bytes: &[u8]) -> Result<(), HeaderRefused> {
    let disk = |e: std::io::Error| HeaderRefused::Disk(e.to_string());
    if bytes.len() != held_seal::header_len(recipients) {
        return Err(HeaderRefused::NotAHeader);
    }
    let new = held_seal::parse_header(bytes).map_err(|_| HeaderRefused::Mismatch)?;
    if new.recipients != recipients {
        return Err(HeaderRefused::Mismatch);
    }
    let now = read_header(&held_dir_beside(folders).join(&row.package).join(part_file(part))).map_err(disk)?;
    if held_seal::header_commitment(&now) != Some(new.commitment) {
        return Err(HeaderRefused::Commitment);
    }
    let dir = staging_dir(folders, &row.package);
    std::fs::create_dir_all(&dir).map_err(disk)?;
    let incoming = dir.join(format!("{part}.header.incoming"));
    write_synced(&incoming, bytes).map_err(HeaderRefused::Disk)?;
    std::fs::rename(&incoming, dir.join(header_file(part))).map_err(disk)
}

/// The finish of the owner's re-wrap: for every part of every held item in `rows`, sealed to
/// `recipients` people, check its new header again (there, a header for that many, the commitment of
/// the part's header now) and build the new part beside the original: the new header, then the
/// original's body copied byte for byte from the relay's own copy. Nothing is changed. Why not, in
/// words naming the item. The caller holds the held-folder lock ([`Storage::owner_build_new_parts`]).
pub(crate) fn build_new_parts(rows: &[HeldRow], recipients: usize, folders: Folders) -> Result<(), String> {
    for row in rows {
        let id = row.id;
        let (home, staging) = (held_dir_beside(folders).join(&row.package), staging_dir(folders, &row.package));
        for part in 0..row.parts.max(0) as usize {
            let header = std::fs::read(staging.join(header_file(part))).map_err(|_| format!("held item #{id}'s part {part} has no new header"))?;
            let refused = || format!("held item #{id}'s new header for part {part} does not fit the part it heads");
            take_check(&header, recipients).ok_or_else(refused)?;
            let original = home.join(part_file(part));
            let old = read_header(&original).map_err(|e| format!("held item #{id}'s part {part} could not be read ({e})"))?;
            if held_seal::header_commitment(&old) != held_seal::header_commitment(&header) {
                return Err(refused());
            }
            let incoming = staging.join(format!("{part}.incoming"));
            let built = (|| -> std::io::Result<u64> {
                let mut src = std::fs::File::open(&original)?;
                std::io::Seek::seek(&mut src, std::io::SeekFrom::Start(old.len() as u64))?;
                let mut dst = std::fs::File::create(&incoming)?;
                dst.write_all(&header)?;
                let body = std::io::copy(&mut src, &mut dst)?;
                dst.sync_all()?;
                Ok(body)
            })();
            let body = built.map_err(|e| format!("held item #{id}'s new part {part} could not be written ({e})"))?;
            let held_len = std::fs::metadata(&original).map(|m| m.len()).unwrap_or(0);
            if body + old.len() as u64 != held_len {
                let _ = std::fs::remove_file(&incoming);
                return Err(format!("held item #{id}'s part {part} changed while it was copied"));
            }
            std::fs::rename(&incoming, staging.join(part_file(part))).map_err(|e| format!("held item #{id}'s new part {part} could not be put in place ({e})"))?;
            let _ = std::fs::remove_file(staging.join(header_file(part)));
        }
    }
    Ok(())
}

/// A header alone, for `recipients` people: Some(()) when it is.
fn take_check(header: &[u8], recipients: usize) -> Option<()> {
    let h = held_seal::parse_header(header).ok()?;
    (header.len() == held_seal::header_len(recipients) && h.recipients == recipients).then_some(())
}

/// The new header of part `part` of `row`, as a recipient checks it: the start of the new part the
/// relay built ([`build_new_parts`]).
pub fn new_header(row: &HeldRow, folders: Folders, part: usize) -> std::io::Result<Vec<u8>> {
    read_header(&staging_dir(folders, &row.package).join(part_file(part)))
}

/// The digest a recipient's acceptance of the change in round `round` must carry for `rows`
/// (handlers/owner_accept.rs): core `held_seal::acceptance_digest` over the hash of every new
/// header the relay staged. Why not, in words naming the item.
pub fn expected_acceptance(round: &str, rows: &[HeldRow], folders: Folders) -> Result<String, String> {
    let mut items = Vec::new();
    for row in rows {
        let hashes = (0..row.parts.max(0) as usize)
            .map(|part| new_header(row, folders, part).map(|h| held_seal::header_hash(&h)).map_err(|_| format!("held item #{}'s new part {part} is missing", row.id)))
            .collect::<Result<Vec<_>, _>>()?;
        items.push((row.package.clone(), hashes));
    }
    Ok(held_seal::acceptance_digest(round, &items))
}

/// Sync the folder `dir` itself, so renames inside it reach the disk before what follows (the
/// owner review's finding 9). Unix only: Windows has no directory sync through the standard
/// library, and the relay runs on Linux.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Err(e) = std::fs::File::open(dir).and_then(|d| d.sync_all()) {
        tracing::warn!("Owner change: could not sync {} ({e})", dir.display());
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Copy `from` to `to` and sync the copy before anything relies on it (the third review's S4: the
/// restore plan was copied unsynced and the synced original then deleted, so a power cut could leave
/// a package whose plan had never reached the disk).
pub(crate) fn copy_synced(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::copy(from, to)?;
    std::fs::OpenOptions::new().write(true).open(to)?.sync_all()
}

/// Write `bytes` to a new file at `path` and sync it.
fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::File::create(path)
        .and_then(|mut f| {
            f.write_all(bytes)?;
            f.sync_all()
        })
        .map_err(|e| format!("the change could not be written down ({e})"))
}
