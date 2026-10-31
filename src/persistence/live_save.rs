//! WHICH SAVE IS LIVE, AND WHAT WAITS FOR WHICH SAVE (ship homes increment 6,
//! docs/design/ship-homes-increment-6-plan.md 3.6): the session's state of the save files, on the
//! one main thread.
//!
//! - `ActiveHome`: the live save (your own home, or one server's), its file, start, household,
//!   server and place aboard; every save writes to its file and every world load builds from it.
//! - The RESTORE HOLD: a snapshot put back over the live save holds its saves off until it is loaded
//!   into the running game (`hold_saves_for_restore`, `reload_after_restore`).
//! - "Snapshot now" (`snapshot_active_home_now`): a copy of the live save as it was last written.
//! - The CHOICES waiting for a slot's fresh save (a home chosen on a server's card, the new-home
//!   row's), `slot_for` (the slot a pairing asks for, pure), and an erased account's save set aside.
//!
//! Plain state about the saves, with no engine or GUI in it, so it sits with the save files under
//! the crates that read and write it: the engine's saving and swapping (save_load.rs,
//! engine/save_slot.rs), and the GUI's pages (where you live, Settings' snapshots and household, the
//! connection list's erase). Here since the crate split's rung 0, part B (2026-10-31); it was split
//! between save_load.rs and engine/save_slot.rs, and the GUI reached up into both for it.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::persistence::{self, SaveSlot, ShipPlace, WorldSave};

/// Which save is live and what it is (ship homes increment 6, docs/design/ship-homes-increment-6-plan.md
/// 3.6): the slot (your own home, or one server's), its file, the start and household it is, the
/// server it belongs to, and where its home stands aboard. Every save writes to `path`, and the
/// household file comes from `start` and `household` (`machines::home_ron_path`). Until anything
/// sets it, it is your own home, a homestead for a family, where every save before the starts was.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveHome {
    pub slot: persistence::SaveSlot,
    pub path: PathBuf,
    pub start: String,
    pub household: String,
    pub server: String,
    pub ship_place: Option<persistence::ShipPlace>,
}

impl ActiveHome {
    /// Your own home: saves/offline_home.json, a homestead for a family.
    pub fn offline() -> ActiveHome {
        let slot = persistence::SaveSlot::Offline;
        ActiveHome {
            path: persistence::slot_path(&persistence::saves_dir(), &slot),
            slot,
            start: persistence::default_start(),
            household: persistence::default_household(),
            server: String::new(),
            ship_place: None,
        }
    }

    /// The live save `save`, kept at `path` for `slot` (engine/save_slot.rs `take_up`, when a
    /// change of pairing makes another save live): its start, household, server and place.
    pub fn of_save(slot: persistence::SaveSlot, path: PathBuf, save: &WorldSave) -> ActiveHome {
        ActiveHome {
            slot,
            path,
            start: save.start.clone(),
            household: save.household.clone(),
            server: save.server.clone(),
            ship_place: save.ship_place.clone(),
        }
    }

    /// A server's save (`SaveSlot::Server`): it always keeps its progress, whatever "Start every
    /// session from the default home" says, because that setting is about your own home.
    pub fn is_server(&self) -> bool {
        matches!(self.slot, persistence::SaveSlot::Server { .. })
    }
}

// Thread-local for the reason RESTORED_SAVE_WAITING below is: every save, every load and every
// world build runs on the one main (event loop) thread, and a thread-local keeps each test's
// active home its own.
thread_local! {
    static ACTIVE_HOME: std::cell::RefCell<Option<ActiveHome>> = const { std::cell::RefCell::new(None) };
}

/// The live save (`ActiveHome::offline` until something sets another).
pub fn active_home() -> ActiveHome {
    ACTIVE_HOME.with(|a| a.borrow().clone()).unwrap_or_else(ActiveHome::offline)
}

/// The live save's household machine file: `machines::home_ron_path` for the active home's
/// start and household (ship homes increment 6). Beside the live save it reads since the crate
/// split's rung 0 (`home_ron_path` read the active home itself before, and the ship may not reach
/// up into the engine).
pub fn active_home_machines_path(data_dir: &std::path::Path) -> PathBuf {
    let active = active_home();
    crate::machines::home_ron_path(data_dir, &active.start, &active.household)
}

/// Make `home` the live save: what every later save writes and every world load builds from.
pub fn set_active_home(home: ActiveHome) {
    ACTIVE_HOME.with(|a| *a.borrow_mut() = Some(home));
}

/// The live save's file (`ActiveHome::path`): saves/offline_home.json for your own home.
pub fn active_home_path() -> PathBuf {
    active_home().path
}

// ── The restore hold (2026-10-03) ─────────────────────────────────────────────────────────────
//
// Settings > Data > "Save snapshots" can put an earlier copy of the active home
// back (persistence::restore_snapshot). The file on disk is then the restored
// home, but the live world still holds what it replaced, and the next periodic
// save (or the save on quit) would write that straight back over the restore.
// So a restore of the active home (1) asks for the restored file to be loaded
// into the world, through the same `launcher_pending_load` path the character
// picker uses (lib.rs applies it once the world is up, rewinding crafts and
// catching up the time since the snapshot like any time away), and (2) holds
// `save_active_home` off until that load has happened. The load ends in
// save_load.rs `apply_save_to_world` or `apply_identity`, which release the hold. Restored
// from the main menu before the world was ever entered and then quit: the hold
// keeps the quit-save off the restored file, and the next launch loads it.
//
// Thread-local on purpose: every save, every restore click and every apply
// runs on the one main (event loop) thread, and a thread-local keeps each
// test's hold its own, where a process-wide flag would be released at random
// by the many tests that call apply_save_to_world in parallel. If saving ever
// moves to another thread, this has to move with it.
thread_local! {
    static RESTORED_SAVE_WAITING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The file a restore of the live save asked to be loaded into the running game (taken once,
    /// by `take_pending_load`).
    static RELOAD_WAITING: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// True while a restored active home is waiting to be loaded into the world
/// (saves of the active home are held off until then). Settings shows it.
pub fn restored_save_waiting() -> bool {
    RESTORED_SAVE_WAITING.with(|w| w.get())
}

/// A restore of the live save put its file back: hold its saves off until it is loaded.
pub fn hold_saves_for_restore() {
    RESTORED_SAVE_WAITING.with(|w| w.set(true));
}

/// The live world comes from a save on disk again (a load, or a load that can never happen): the
/// saves may go on.
pub fn release_restore_hold() {
    RESTORED_SAVE_WAITING.with(|w| w.set(false));
}

/// Ask for `path`, the live save a restore just put back, to be loaded into the running game
/// (Settings' Restore; taken once by save_load.rs `take_pending_load`).
pub fn reload_after_restore(path: PathBuf) {
    RELOAD_WAITING.with(|r| *r.borrow_mut() = Some(path));
}

/// The restored live save waiting to be loaded into the running game, taken once.
pub fn take_reload_waiting() -> Option<PathBuf> {
    RELOAD_WAITING.with(|r| r.borrow_mut().take())
}

/// What a "Snapshot now" click did, for the line Settings shows.
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotNow {
    /// A new copy was kept, at this path.
    Kept(PathBuf),
    /// The save has not changed since the last copy kept, so that copy
    /// already holds it and nothing new was taken (a new one would only push
    /// the oldest copy out).
    AlreadyKept,
    /// There is no save of the home yet, so there is nothing to keep.
    NotSavedYet,
}

/// Settings > Data "Snapshot now": keep a copy of the active home as it was
/// last saved (the game saves every two minutes and on quit), whatever the
/// spacing, unless the save is unchanged since the last copy.
pub fn snapshot_active_home_now() -> Result<SnapshotNow, String> {
    snapshot_home_now_at(&active_home_path(), persistence::now_ms())
}

/// `snapshot_active_home_now` for a save at any path and moment (tests).
fn snapshot_home_now_at(path: &std::path::Path, now_ms: u64) -> Result<SnapshotNow, String> {
    if !path.is_file() {
        return Ok(SnapshotNow::NotSavedYet);
    }
    // Forced, so the only way it keeps nothing for a save that exists is the
    // save being unchanged since the last copy (persistence::snapshot_save).
    Ok(match persistence::snapshot_save(path, &persistence::snapshots_root_for(path), now_ms, true)? {
        Some(kept) => SnapshotNow::Kept(kept),
        None => SnapshotNow::AlreadyKept,
    })
}

// ── The choices waiting for a slot's fresh save ─────────────────────────────────────────────────

/// A start chosen for a save that does not exist yet: a home chosen on a server's card (`chose`
/// with its plot, `on_game_message`) or the new-home row's choice (`choose_new_home`). The save
/// `follow` then makes for that slot is of this start and household, standing on `place`.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub start: String,
    pub household: String,
    pub place: Option<ShipPlace>,
}

// Thread-local for the reason ACTIVE_HOME is: every swap runs on the one main thread, and a
// thread-local keeps each test's its own.
thread_local! {
    /// Choices waiting for their slot's fresh save, at most one per slot.
    static CHOICES: RefCell<Vec<(SaveSlot, Choice)>> = const { RefCell::new(Vec::new()) };
}

/// Remember `choice` for `slot`'s fresh save (replacing an earlier one for that slot).
pub fn remember_choice(slot: &SaveSlot, choice: Choice) {
    CHOICES.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|(s, _)| s != slot);
        c.push((slot.clone(), choice));
    });
}

/// The choice waiting for `slot`, if any.
pub fn choice_for(slot: &SaveSlot) -> Option<Choice> {
    CHOICES.with(|c| c.borrow().iter().find(|(s, _)| s == slot).map(|(_, ch)| ch.clone()))
}

/// The choice for `slot` was made into its save (engine/save_slot.rs `follow`).
pub fn forget_choice(slot: &SaveSlot) {
    CHOICES.with(|c| c.borrow_mut().retain(|(s, _)| s != slot));
}

/// The new-home row's choice (lane 2B: Characters (Play), "A new home"): your own home, when it
/// has no save yet, is made of this start and household.
pub fn choose_new_home(start: &str, household: &str) {
    remember_choice(&SaveSlot::Offline, Choice { start: start.to_string(), household: household.to_string(), place: None });
}

// ── The slot a pairing asks for ──────────────────────────────────────────────────────────────────

/// The save a pairing asks for (engine/save_slot.rs `wanted`) from its four facts (pure): the
/// pairing, the identity's public key, the server's normalized address and whether this
/// identity's account there was erased.
pub fn slot_for(last_world: &str, identity: &str, server: &str, erased_here: bool) -> SaveSlot {
    let identity = identity.trim().to_ascii_lowercase();
    if last_world.starts_with("home:") || identity.is_empty() || server.is_empty() || erased_here {
        return SaveSlot::Offline;
    }
    SaveSlot::Server { server: server.to_string(), identity }
}

// ── An erased account ────────────────────────────────────────────────────────────────────────────

/// Set the save of `slot` aside (3.6, an erased account): renamed beside itself to
/// `<name>.erased-<day>.json` (a second one that day `<name>.erased-<day>-2.json`), never
/// deleted, so a sign-up there again starts fresh while the old one can still be looked at. The
/// new path, None when there was no save.
pub fn set_aside(saves_dir: &Path, slot: &SaveSlot, day: &str) -> Option<PathBuf> {
    let path = crate::persistence::slot_path(saves_dir, slot);
    if !path.exists() {
        return None;
    }
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let aside = (1..1000)
        .map(|n| if n == 1 { format!("{stem}.erased-{day}.json") } else { format!("{stem}.erased-{day}-{n}.json") })
        .map(|name| path.with_file_name(name))
        .find(|p| !p.exists())?;
    match std::fs::rename(&path, &aside) {
        Ok(()) => {
            log::info!("An erased account's save on that server is set aside as {}", aside.display());
            Some(aside)
        }
        Err(e) => {
            log::warn!("An erased account's save could not be set aside ({e}); it stays where it was");
            None
        }
    }
}

/// `set_aside` for this identity's save on `server` (gui/connections.rs, when the server confirms
/// the account erased; `server` normalized, chat.rs `norm_server_url`, as the slots key it), today's
/// date in the name. When that save is the live one, the live world goes on writing into the file
/// set aside until engine/save_slot.rs `follow` swaps to your own home (`wanted` says so for an
/// erased account), so nothing is written back where a sign-up again would find it.
pub fn set_aside_erased(saves_dir: &Path, server: &str, identity: &str) -> Option<PathBuf> {
    let slot = slot_for("", identity, server, false);
    if slot == SaveSlot::Offline {
        return None;
    }
    let aside = set_aside(saves_dir, &slot, &crate::persistence::utc_day(crate::persistence::now_ms()))?;
    let mut active = active_home();
    if active.slot == slot {
        active.path = aside.clone();
        set_active_home(active);
    }
    Some(aside)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Snapshot now" says what it did: nothing to keep before the first
    /// save, a new copy for a changed save, and "already kept" for a save
    /// unchanged since the last copy (where it used to keep a duplicate and
    /// push the oldest copy out).
    ///
    /// Red check (2026-10-03): with the "Unchanged since the last copy" loop
    /// removed from `persistence::snapshot_save_protecting`, this failed with
    /// `a second click on an unchanged home says it is already kept`, `left:
    /// Kept(...), right: AlreadyKept`. Restored byte for byte, it passes.
    #[test]
    fn snapshot_now_says_when_the_home_is_already_kept() {
        let root = std::env::temp_dir().join(format!("hos_snapshot_now_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("saves").join("offline_home.json");
        let t = 1_759_400_000_000u64;
        assert_eq!(snapshot_home_now_at(&path, t).unwrap(), SnapshotNow::NotSavedYet);

        persistence::save_world(&path, &WorldSave::new_offline("Home", "homestead")).unwrap();
        assert!(matches!(snapshot_home_now_at(&path, t + 1_000).unwrap(), SnapshotNow::Kept(_)));
        assert_eq!(
            snapshot_home_now_at(&path, t + 2_000).unwrap(),
            SnapshotNow::AlreadyKept,
            "a second click on an unchanged home says it is already kept"
        );
        // The home changes (written without the snapshot a save would keep):
        // the next click keeps it.
        let mut changed = WorldSave::new_offline("Home", "homestead");
        changed.character_name = "Changed".to_string();
        crate::atomic_write::write_atomic(&path, serde_json::to_string_pretty(&changed).unwrap().as_bytes()).unwrap();
        assert!(matches!(snapshot_home_now_at(&path, t + 3_000).unwrap(), SnapshotNow::Kept(_)));
        let slot = persistence::snapshots_root_for(&path).join("offline_home");
        assert_eq!(persistence::list_snapshots(&slot).len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
