//! World persistence — save/load game state to JSON files.
//!
//! Save directory: platform data dir + "/saves/"
//! - Windows: `%APPDATA%/HumanityOS/saves/`
//! - Linux:   `$XDG_DATA_HOME/HumanityOS/saves/` or `~/.local/share/HumanityOS/saves/`
//! - macOS:   `~/Library/Application Support/HumanityOS/saves/`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Complete world save state. A "home" (the homes-as-save-profiles model, v0.380)
/// IS a WorldSave that knows its `kind` + `design`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSave {
    pub name: String,
    pub timestamp: u64,
    pub game_time: f64,
    pub player_position: [f32; 3],
    pub player_rotation: [f32; 4],
    pub player_health: f32,
    pub inventory: Vec<(String, u32)>,
    /// Wear and grade of each saved backpack stack, in the same order as
    /// `inventory` (2026-09-26: a restart used to renew every carried tool
    /// and erase its grade). An older save without it restores unworn.
    #[serde(default)]
    pub inventory_state: Vec<(u32, u8)>,
    pub skills: HashMap<String, (u32, u32)>,
    pub constructions: Vec<ConstructionSave>,
    /// Craft batches in flight (2026-09-25); see CraftSave.
    #[serde(default)]
    pub crafts: Vec<crate::systems::crafting::CraftSave>,
    /// False for a save written while "Start every session from the default
    /// home" was on and no progress save existed yet: it carries the
    /// character only, so applying it must never replace the default home
    /// (an empty inventory in it means "not recorded", not "empty").
    #[serde(default = "default_true_save")]
    pub progress_saved: bool,
    pub weather_state: String,
    /// What this home IS (the "who owns the truth" axis): "offline" (you own the
    /// local save), "server" (a relay owns it), or "real" (physical sensors own
    /// it). Defaults to "offline" so pre-v0.380 saves load unchanged. Server + Real
    /// are deferred; offline is the only live kind today.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// The Design (blueprint) this home is built from, e.g. "fibonacci". Defaults
    /// to "fibonacci" (the first + only design today).
    #[serde(default = "default_design")]
    pub design: String,
    /// The GAME character's name (v0.448), DECOUPLED from the chat/network profile name
    /// (which is the Dilithium identity). A character lives in the save (self-custodial,
    /// local), separate from who you are in chat. serde-default for old saves.
    #[serde(default = "default_character_name")]
    pub character_name: String,
    /// The player's avatar appearance (v0.440). serde-default so pre-v0.440 saves load
    /// unchanged (they get the default blockman).
    #[serde(default)]
    pub appearance: crate::ecs::components::Appearance,
    /// The player's equipped cosmetic outfit (v0.440). serde-default for old saves.
    #[serde(default)]
    pub outfit: crate::ecs::components::Outfit,
    /// The organize-layer container contents (v0.516): every item that lives in a
    /// container other than the live backpack, tagged with its container path. The
    /// backpack itself is `inventory` above. None means this save never wrote a
    /// pool (a fresh character), and the seeded default stays; Some(empty) is a
    /// home whose storage really is empty and comes back empty (2026-09-27: an
    /// empty Vec used to mean both, so emptying every container, saving, then
    /// stashing and pressing ESC > Play kept the live pool beside the rewound
    /// backpack and doubled the stashed goods).
    #[serde(default)]
    pub placed_items: Option<Vec<crate::systems::inventory::placed::PlacedItem>>,
    /// Vehicles standing in the world (economy Phase 2 Stage 1, v0.677), deployed
    /// from kit items. serde-default so older saves load with none. NOTE: the
    /// separate `constructions` field above is dormant schema (never written or
    /// read by the live save path) — do not conflate the two.
    #[serde(default)]
    pub deployed_vehicles: Vec<VehicleSave>,
    /// Growing crops (v0.863): every CropInstance round-trips so the garden
    /// survives restarts (operator: "We should just be able to load up and have
    /// our changes persist"). serde-default so older saves load with none, and
    /// the showcase auto-seed then fills the grow surfaces instead.
    #[serde(default)]
    pub crops: Vec<crate::ecs::components::CropInstance>,
    /// Each saved crop's soil (2026-09-26, N-P-K), parallel to `crops`;
    /// None for a crop that had no soil record yet.
    #[serde(default)]
    pub crop_soil: Vec<Option<crate::ecs::components::CropSoil>>,
    /// Each saved crop's pollination record (2026-09-26,
    /// farming::pollination), parallel to `crops`; None for a crop without
    /// one. serde-default so a save from before it loads with none.
    #[serde(default)]
    pub crop_pollination: Vec<Option<crate::ecs::components::CropPollination>>,
    /// Each saved crop's picking state (2026-09-26, farming::picking),
    /// parallel to `crops`; None for a crop without one (not yet ripe, or
    /// harvested once). serde-default so a save from before it loads with
    /// none, and a ripe picked crop then starts its window afresh.
    #[serde(default)]
    pub crop_picking: Vec<Option<crate::ecs::components::CropPicking>>,
    /// What each emptied unit's soil still held, for the next crop sown there.
    #[serde(default)]
    pub soil_memory: crate::ecs::components::SoilMemory,
    /// The player's credit balance (v0.747, ladder rung 3). serde-default -1 =
    /// "no wallet saved yet" so pre-v0.747 saves keep the fresh-start default
    /// (10,000 CR) instead of loading as broke.
    #[serde(default = "default_credits")]
    pub credits: i64,
    /// Quest progress (v0.748, ladder rung 4). None = a pre-quest-save file;
    /// the player then starts fresh with the auto-accepted First Steps chain,
    /// exactly as before. Some = the full tracker round-trips, so completing
    /// gs_first_steps twice is no longer the player experience.
    #[serde(default)]
    pub quests: Option<crate::systems::quests::QuestTracker>,
    /// The homestead animals' yield timers (2026-09-27, offline progression):
    /// (herd slot, seconds since last collected), one per living animal. The
    /// herd respawns from data/entities/livestock.ron on world entry, so
    /// without this every restart made every animal ready to collect again.
    #[serde(default)]
    pub herd: Vec<(String, f32)>,
    /// The asteroids as they stood (2026-09-27), mined-down ore and all.
    /// None = not recorded, keep the fresh set; Some = authoritative, and an
    /// asteroid missing from it was mined out.
    #[serde(default)]
    pub asteroids: Option<Vec<crate::ecs::components::AsteroidBody>>,
    /// The mining drone in flight, with its cargo (2026-09-27): the ore in its
    /// hold has already left the asteroid, so dropping it lost the ore.
    #[serde(default)]
    pub drone: Option<crate::ecs::components::Drone>,
    /// The drone's standing order ("Keep mining"), (asteroid id, manifest).
    #[serde(default)]
    pub mining_order: Option<(String, Vec<(String, u32)>)>,
    /// What each home machine holds, by instance id (2026-09-27): a battery
    /// bank's charge, a water tank's litres, a vessel's contents. Before it
    /// every launch reset each bank and tank to half and emptied every
    /// vessel. A machine missing from it (a save from before, or one placed
    /// since) keeps its spawn level. See systems::machine_levels.
    #[serde(default)]
    pub machine_levels: Vec<crate::systems::machine_levels::MachineLevels>,
    /// What the home has drawn from and returned to the ship's supply
    /// (2026-09-27, systems::ship_power): the reactor's kWh, metered. Empty in
    /// a save from before it; the meter then starts at zero.
    #[serde(default)]
    pub ship_supply: crate::systems::ship_power::ShipSupplyLedger,
    /// The relay trades whose items this backpack has already moved
    /// (2026-10-02, systems::inventory::TradeSettlements), by trade id. Saved
    /// with the inventory it changed, so a restart can neither settle a trade
    /// twice nor lose one. Empty in a save from before it: a completed trade
    /// then settles once more at the next connect, which is right, because
    /// until this the desktop app never moved a traded item at all.
    #[serde(default)]
    pub settled_trades: Vec<String>,
}

fn default_credits() -> i64 {
    -1
}

/// One deployed vehicle in a save: what it is + where it stands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleSave {
    /// The ASSEMBLED vehicle's items.csv id (e.g. "truck_pickup_0").
    pub item_id: String,
    pub position: [f32; 3],
    /// Yaw around +Y in radians (deployed vehicles only ever yaw).
    pub yaw: f32,
}

fn default_kind() -> String {
    "offline".to_string()
}
fn default_design() -> String {
    "fibonacci".to_string()
}
fn default_character_name() -> String {
    "Wanderer".to_string()
}

impl WorldSave {
    /// A fresh OFFLINE home built from the given design, with empty progress. The
    /// "save wrapper" entry point: a home is a WorldSave that knows its kind +
    /// design. `timestamp` is left 0; the save flow stamps it when the file is
    /// written.
    pub fn new_offline(name: impl Into<String>, design: impl Into<String>) -> Self {
        WorldSave {
            name: name.into(),
            timestamp: 0,
            game_time: 0.0,
            player_position: [0.0, 0.0, 0.0],
            player_rotation: [0.0, 0.0, 0.0, 1.0],
            player_health: 100.0,
            inventory: Vec::new(),
            skills: HashMap::new(),
            constructions: Vec::new(),
            crafts: Vec::new(),
            progress_saved: true,
            weather_state: "clear".to_string(),
            kind: "offline".to_string(),
            design: design.into(),
            character_name: default_character_name(),
            appearance: Default::default(),
            outfit: Default::default(),
            placed_items: None,
            deployed_vehicles: Vec::new(),
            crops: Vec::new(),
            crop_soil: Vec::new(),
            crop_pollination: Vec::new(),
            crop_picking: Vec::new(),
            inventory_state: Vec::new(),
            soil_memory: Default::default(),
            credits: -1,
            quests: None,
            herd: Vec::new(),
            asteroids: None,
            drone: None,
            mining_order: None,
            machine_levels: Vec::new(),
            ship_supply: Default::default(),
            settled_trades: Vec::new(),
        }
    }
}

/// A saved construction/building in the world: one ECS entity carrying either
/// a finished `Structure` or an in-progress `Construction` (2026-09-25; the
/// field existed from the start but nothing wrote it, so every build was
/// discarded at exit). Carries what the blueprint registry would otherwise
/// supply, because the save is applied where no registry is in reach.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstructionSave {
    pub blueprint_id: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    /// The box the renderer draws is the Transform scale (blueprint size).
    #[serde(default = "unit_scale")]
    pub scale: [f32; 3],
    pub health: f32,
    #[serde(default)]
    pub max_health: f32,
    /// What the finished structure provides (Structure.provides).
    #[serde(default)]
    pub provides: Option<String>,
    /// Some((progress, build_time)) in seconds while still under
    /// construction; None once it is a finished Structure.
    #[serde(default)]
    pub building: Option<(f32, f32)>,
    /// The finished structure's stable uid (Structure.uid, 2026-09-27): a
    /// built chest's contents are filed under it, so it must come back as
    /// the same number. 0 for a scaffold, which has none yet.
    #[serde(default)]
    pub uid: u32,
    /// A door set into the piece stands open (2026-09-28, the `DoorOpen`
    /// marker). Absent from older saves: the door is shut.
    #[serde(default)]
    pub open: bool,
    /// The planet build site the piece stands in (2026-09-27, BUG-102): its
    /// body and the site origin in the body's frame, in f64, and then
    /// `position`/`rotation` are site-local. None (and absent from saves
    /// written before sites existed) = the home frame, aboard.
    #[serde(default)]
    pub site: Option<crate::systems::construction::PlanetSite>,
}

fn default_true_save() -> bool {
    true
}

fn unit_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

/// Get the platform-appropriate saves directory.
///
/// Falls back to `./saves/` if the platform data directory cannot be determined.
pub fn saves_dir() -> PathBuf {
    // Portable mode (v0.707): saves travel beside the exe.
    if let Some(p) = crate::storage::portable_saves_dir() {
        return p;
    }
    // Try standard platform data dirs without adding a dependency.
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata)
                .join("HumanityOS")
                .join("saves");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("HumanityOS")
                .join("saves");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg)
                .join("HumanityOS")
                .join("saves");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("HumanityOS")
                .join("saves");
        }
    }

    PathBuf::from("saves")
}

/// Save the world state to a JSON file at the given path.
///
/// Two protections since 2026-10-03, both in `write_save_bytes`: the save
/// that is about to be overwritten is first kept as a snapshot under
/// `backups/saves/` (at most one every `SNAPSHOT_SPACING_SECS`, the newest
/// `SNAPSHOTS_KEPT` kept), and the new bytes reach the file through a temp
/// file and a rename, so a crash mid-write can never leave a torn save.
/// Before this the save was rewritten in place with `std::fs::write`, which
/// truncates the file first: a crash at the wrong moment lost the home.
pub fn save_world(path: &Path, save: &WorldSave) -> Result<(), String> {
    let json = serde_json::to_string_pretty(save)
        .map_err(|e| format!("Failed to serialize world save: {e}"))?;

    write_save_bytes(path, json.as_bytes(), &snapshots_root_for(path), now_ms())?;

    log::info!("World saved to {}", path.display());
    Ok(())
}

// ── Save snapshots and crash-safe writes (2026-10-03) ──────────────────────
//
// Layout, beside the saves folder in the app data dir (so portable mode keeps
// them beside the exe, and the Settings > Data storage migration, whose
// MIGRATE_DIRS already lists "backups", carries them along):
//
//   HumanityOS/
//     saves/offline_home.json                  the live save
//     backups/saves/offline_home/              one folder per save slot
//       offline_home_2026-10-03_03-41-12.345Z.json
//       offline_home_2026-10-03_03-58-40.002Z.json
//
// The names are the UTC moment the snapshot was taken, readable by a person
// who opens the folder and sortable by name. The GUI for all of this is
// Settings > Data > "Save snapshots" (src/gui/pages/settings.rs).

/// How many snapshots of each save slot are kept.
///
/// Ten automatic snapshots, spaced at least `SNAPSHOT_SPACING_SECS` apart,
/// reach back at least two and a half hours of play, and usually across
/// several sessions, because a snapshot is only taken when a save is about to
/// be overwritten (time with the game closed costs none). A forced snapshot
/// (a "Snapshot now" click, or the copy a restore keeps) can come sooner, but
/// only when the save has changed since the last copy, so clicking the button
/// again and again cannot roll the old copies out. A played home is about
/// 0.6 MB (558 KB on the operator's machine, 2026-10-03), so this is a few
/// megabytes per slot. Fewer would let one long evening roll every copy past
/// the moment something went wrong; many more only costs disk for copies
/// nobody scrolls back to.
pub const SNAPSHOTS_KEPT: usize = 10;

/// The least time between two AUTOMATIC snapshots of one slot.
///
/// The home is saved every two minutes while you play
/// (`save_load::maybe_periodic_save`), so a snapshot at every write would roll
/// all ten copies in twenty minutes: a save spoiled by a bug or a regretted
/// trade would push every good copy out before anyone noticed. A forced
/// snapshot ("Snapshot now" in Settings, and the copy a restore keeps of what
/// it replaces) ignores this spacing, though not the "unchanged" check.
///
/// The spacing is measured from the newest snapshot taken at or before the
/// current time, so a clock that was set back (a newest snapshot stamped "in
/// the future") cannot switch it off. See `snapshot_save_protecting`.
pub const SNAPSHOT_SPACING_SECS: u64 = 15 * 60;

/// Wall-clock milliseconds since the Unix epoch (0 if the clock is broken).
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The app data dir's `backups/` folder: beside `saves/`, which puts it at
/// `%APPDATA%\HumanityOS\backups` on Windows, or beside the exe in portable
/// mode. Shown in Settings > Data with an Open button.
pub fn backups_dir() -> PathBuf {
    match saves_dir().parent() {
        Some(root) if !root.as_os_str().is_empty() => root.join("backups"),
        _ => PathBuf::from("backups"),
    }
}

/// Where the snapshots of the save at `save_path` are kept: the `backups/saves/`
/// folder beside the folder the save sits in. For a real save that is
/// `backups_dir()/saves`. Deriving it from the save's own path (rather than
/// always from `saves_dir()`) keeps a test's throwaway save beside its own
/// throwaway backups, never in the player's real backups folder.
pub fn snapshots_root_for(save_path: &Path) -> PathBuf {
    match save_path.parent().and_then(|saves| saves.parent()) {
        Some(root) if !root.as_os_str().is_empty() => root.join("backups").join("saves"),
        _ => PathBuf::from("backups").join("saves"),
    }
}

/// The snapshots root for the real saves folder (`backups/saves/`).
pub fn save_snapshots_root() -> PathBuf {
    snapshots_root_for(&saves_dir().join("any.json"))
}

/// One kept copy of a save.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveSnapshot {
    pub path: PathBuf,
    /// When it was taken, Unix milliseconds (read back from its file name).
    pub taken_ms: u64,
    /// Tie-breaker for two snapshots taken in the same millisecond.
    pub seq: u32,
    /// Size on disk, for the Settings list.
    pub bytes: u64,
}

/// Days since 1970-01-01 to a (year, month, day) civil date (proleptic
/// Gregorian, UTC). Howard Hinnant's public-domain `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The inverse of `civil_from_days` (Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `offline_home_2026-10-03_03-41-12.345Z.json` (and `..Z_2.json` for a second
/// snapshot in the same millisecond). No colons: Windows forbids them.
fn snapshot_file_name(stem: &str, taken_ms: u64, seq: u32) -> String {
    let secs = (taken_ms / 1000) as i64;
    let milli = taken_ms % 1000;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let sod = secs.rem_euclid(86_400);
    let stamp = format!(
        "{y:04}-{mo:02}-{d:02}_{:02}-{:02}-{:02}.{milli:03}Z",
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    );
    if seq == 0 {
        format!("{stem}_{stamp}.json")
    } else {
        format!("{stem}_{stamp}_{seq}.json")
    }
}

/// Read a snapshot's moment back from its file name; None for any file that
/// is not one of ours (a stray file a person dropped in the folder, or a
/// half-written `.tmp`), which listing and pruning then leave alone.
fn parse_snapshot_name(stem: &str, file_name: &str) -> Option<(u64, u32)> {
    let rest = file_name.strip_prefix(stem)?.strip_prefix('_')?.strip_suffix(".json")?;
    let (stamp, seq) = match rest.split_once("Z_") {
        Some((stamp, seq)) => (stamp, seq.parse::<u32>().ok()?),
        None => (rest.strip_suffix('Z')?, 0),
    };
    // YYYY-MM-DD_HH-MM-SS.mmm
    let b = stamp.as_bytes();
    if b.len() != 23 || b[4] != b'-' || b[7] != b'-' || b[10] != b'_' || b[13] != b'-' || b[16] != b'-' || b[19] != b'.' {
        return None;
    }
    let num = |from: usize, to: usize| stamp.get(from..to)?.parse::<u64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, s, ms) = (num(11, 13)?, num(14, 16)?, num(17, 19)?, num(20, 23)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let days = days_from_civil(y as i64, mo as u32, d as u32);
    let secs = days * 86_400 + (h * 3600 + mi * 60 + s) as i64;
    if secs < 0 {
        return None;
    }
    Some((secs as u64 * 1000 + ms, seq))
}

/// One slot's snapshots, newest first. The slot is the folder's own name (the
/// save file's stem). A missing folder is simply no snapshots.
pub fn list_snapshots(slot_dir: &Path) -> Vec<SaveSnapshot> {
    let Some(stem) = slot_dir.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(slot_dir) else {
        return Vec::new();
    };
    let mut out: Vec<SaveSnapshot> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name();
            let (taken_ms, seq) = parse_snapshot_name(stem, name.to_str()?)?;
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| SaveSnapshot { path: e.path(), taken_ms, seq, bytes: meta.len() })
        })
        .collect();
    out.sort_by(|a, b| (b.taken_ms, b.seq).cmp(&(a.taken_ms, a.seq)));
    out
}

/// Every slot that has snapshots, as (slot name, its snapshots newest first),
/// slots in name order. For the Settings list.
pub fn list_snapshot_slots(root: &Path) -> Vec<(String, Vec<SaveSnapshot>)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut slots: Vec<(String, Vec<SaveSnapshot>)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let slot = e.file_name().to_str()?.to_string();
            let snaps = list_snapshots(&e.path());
            (!snaps.is_empty()).then_some((slot, snaps))
        })
        .collect();
    slots.sort_by(|a, b| a.0.cmp(&b.0));
    slots
}

/// Delete all but the newest `keep` snapshots in a slot folder. A snapshot in
/// `protect` is never deleted, even when it sorts oldest: it then stays as one
/// over the count until a later prune. Two callers protect one: a restore
/// protects the snapshot it is restoring from, and every snapshot protects
/// itself in the prune that follows it being taken (see
/// `snapshot_save_protecting` for why that matters when the clock is behind).
/// Returns how many were removed.
pub fn prune_snapshots(slot_dir: &Path, keep: usize, protect: &[&Path]) -> usize {
    // Compared by file name: every path here is inside the one slot folder.
    let protect_names: Vec<std::ffi::OsString> =
        protect.iter().filter_map(|p| p.file_name()).map(|n| n.to_os_string()).collect();
    let mut removed = 0;
    for old in list_snapshots(slot_dir).into_iter().skip(keep) {
        let name = old.path.file_name().map(|n| n.to_os_string());
        if name.is_some_and(|n| protect_names.contains(&n)) {
            continue;
        }
        match std::fs::remove_file(&old.path) {
            Ok(()) => removed += 1,
            Err(e) => log::warn!("Could not remove old save snapshot {}: {e}", old.path.display()),
        }
    }
    removed
}

/// Keep a copy of the save at `save_path` exactly as it is on disk now, in
/// `root/<slot>/`, then prune the slot to `SNAPSHOTS_KEPT`.
///
/// Nothing is taken, and Ok(None) comes back, when:
/// - there is no save there yet;
/// - `force` is false (the automatic path) and the previous snapshot (the
///   newest one taken at or before `now_ms`) is younger than
///   `SNAPSHOT_SPACING_SECS`;
/// - the save is byte for byte the same as the last snapshot kept, forced or
///   not. An identical copy is already there, and a second one would only
///   push the oldest copy out: ten "Snapshot now" clicks in a row used to
///   replace the whole history with ten copies of one save (review finding,
///   2026-10-03).
///
/// Ok(Some(path)) = the new copy.
pub fn snapshot_save(save_path: &Path, root: &Path, now_ms: u64, force: bool) -> Result<Option<PathBuf>, String> {
    snapshot_save_protecting(save_path, root, now_ms, force, None)
}

fn snapshot_save_protecting(
    save_path: &Path,
    root: &Path,
    now_ms: u64,
    force: bool,
    protect: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    if !save_path.is_file() {
        return Ok(None);
    }
    let stem = save_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("save file {} has no usable name", save_path.display()))?;
    let slot_dir = root.join(stem);
    let kept = list_snapshots(&slot_dir);
    // The snapshot a new one follows: the newest one taken at or before now.
    // Normally that is simply the newest. When the clock has been set back
    // (a dual boot with a clock out by hours, or a person changing it), the
    // newest by name is stamped in the future. The spacing used to skip its
    // check altogether in that case, so every 2-minute save kept a snapshot
    // and 16 minutes of play rolled six old copies out (review finding,
    // 2026-10-03). Measured from the newest one at or before now, the spacing
    // keeps working on whatever the clock says: two copies taken after the
    // clock went back are always a spacing apart. The price is a gap: as the
    // set-back clock walks into the stretch it had already stamped, each of
    // those copies becomes the previous one in turn, so no new copy is due
    // until the clock is past them. The old copies are all kept meanwhile,
    // which is the safe side. Only when EVERY copy is in the future is there
    // nothing to measure from (see the prune below).
    let previous = kept.iter().find(|s| s.taken_ms <= now_ms);
    if !force {
        if let Some(previous) = previous {
            let spacing_ms = SNAPSHOT_SPACING_SECS * 1000;
            if now_ms - previous.taken_ms < spacing_ms {
                return Ok(None);
            }
        }
    }
    let bytes = std::fs::read(save_path).map_err(|e| format!("could not read {}: {e}", save_path.display()))?;
    // Already kept: compare with EVERY copy in the slot, not just the last
    // one (review, 2026-10-03). After a Restore the live save is a byte copy
    // of the snapshot just restored, which is often not the newest, so
    // checking only the newest kept a duplicate on the next Restore and
    // pushed the oldest copy out: stepping back one copy at a time to find a
    // save from before a bug destroyed the copies it was heading toward. The
    // listing already holds each size, so a file is only read when its size
    // matches. A copy that cannot be read simply does not count as a match.
    let _ = previous;
    for copy in kept.iter() {
        if copy.bytes == bytes.len() as u64 && std::fs::read(&copy.path).is_ok_and(|b| b == bytes) {
            log::info!(
                "Not keeping another snapshot of {}: it has not changed since {}",
                save_path.display(),
                copy.path.display()
            );
            return Ok(None);
        }
    }
    std::fs::create_dir_all(&slot_dir)
        .map_err(|e| format!("could not create {}: {e}", slot_dir.display()))?;
    // Never overwrite an existing snapshot: a second one in the same
    // millisecond gets a sequence number.
    let mut seq = 0u32;
    let mut target = slot_dir.join(snapshot_file_name(stem, now_ms, seq));
    while target.exists() {
        seq += 1;
        target = slot_dir.join(snapshot_file_name(stem, now_ms, seq));
    }
    // The copy goes through the same temp-then-rename as a save, so a crash
    // mid-copy leaves a stray .tmp (ignored by listing) rather than a torn
    // snapshot that pruning would keep in place of a good one.
    write_atomic(&target, &bytes)?;
    // The new copy is protected in its own prune. Normally it sorts newest
    // and pruning never reaches it. But when the clock is behind EVERY kept
    // snapshot its name sorts oldest, and unprotected it was deleted the
    // moment it was written, so no new copy was kept until the clock caught
    // up. Protected, it stays as one over the count, the older copies are
    // untouched, and the next snapshot (spaced from this one) replaces it.
    let mut protected: Vec<&Path> = vec![target.as_path()];
    protected.extend(protect);
    prune_snapshots(&slot_dir, SNAPSHOTS_KEPT, &protected);
    log::info!("Kept a snapshot of {} as {}", save_path.display(), target.display());
    Ok(Some(target))
}

/// The temp file a save is written to before it is renamed over the real one:
/// `offline_home.json.tmp`, in the same folder (a rename only replaces
/// atomically within one volume). Its extension is not `.json`, so nothing
/// that scans the saves folder for saves ever mistakes it for one.
fn staging_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Step one of a crash-safe write: put all the bytes in the temp file and
/// flush them to the disk. The real file is not touched. A crash after this
/// and before `commit_staged` leaves the real file exactly as it was.
fn stage_write(path: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;
    let tmp = staging_path(path);
    let mut f = std::fs::File::create(&tmp).map_err(|e| format!("could not create {}: {e}", tmp.display()))?;
    f.write_all(bytes).map_err(|e| format!("could not write {}: {e}", tmp.display()))?;
    // On the disk, not just in the OS cache, before the rename makes it the
    // save: otherwise a power cut could leave a renamed but empty file.
    f.sync_all().map_err(|e| format!("could not flush {}: {e}", tmp.display()))?;
    Ok(tmp)
}

/// Step two: rename the temp file over the real one. The rename replaces the
/// file in one step, so a reader (or a crash) sees the old save or the new
/// one, never half of either. On Windows a rename can fail for a moment while
/// another program (an antivirus scan, a backup tool) holds the target open,
/// so it is retried briefly before giving up; giving up leaves the old save
/// intact.
fn commit_staged(tmp: &Path, path: &Path) -> Result<(), String> {
    let mut last_err = None;
    for attempt in 0..5u32 {
        match std::fs::rename(tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(10 << attempt));
            }
        }
    }
    Err(format!(
        "could not replace {} (the previous version is still there): {}",
        path.display(),
        last_err.map(|e| e.to_string()).unwrap_or_default()
    ))
}

/// Write `bytes` to `path` so that a crash at any moment leaves either the
/// whole old file or the whole new one.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = stage_write(path, bytes)?;
    commit_staged(&tmp, path)
}

/// Write a save's bytes: keep a snapshot of what is about to be replaced
/// (subject to the spacing), then write crash-safely. A snapshot that fails
/// (disk full, a permissions problem in backups/) is logged and the save goes
/// ahead anyway: the live save matters more than its history.
pub fn write_save_bytes(path: &Path, bytes: &[u8], snapshots_root: &Path, now_ms: u64) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create save directory: {e}"))?;
        }
    }
    if let Err(e) = snapshot_save(path, snapshots_root, now_ms, false) {
        log::warn!("Could not keep a snapshot before saving {} (saving anyway): {e}", path.display());
    }
    write_atomic(path, bytes).map_err(|e| format!("Failed to write save file: {e}"))
}

/// Put a snapshot back as the save at `save_path`. The snapshot must parse as
/// a save first (a damaged copy is refused and nothing changes). Then the
/// save it replaces is itself kept as a snapshot (forced, so a restore can
/// always be undone; when the last copy kept already holds exactly those
/// bytes, that copy serves and no second one is taken), and only then are
/// the snapshot's bytes written over the save, crash-safely. The snapshot restored from stays in its folder.
/// Returns the restored save, so the caller can load it into the game.
pub fn restore_snapshot(snapshot: &Path, save_path: &Path, now_ms: u64) -> Result<WorldSave, String> {
    let bytes = std::fs::read(snapshot).map_err(|e| format!("could not read the snapshot: {e}"))?;
    let restored: WorldSave = serde_json::from_slice(&bytes)
        .map_err(|e| format!("the snapshot is not a readable save: {e}"))?;
    if let Some(parent) = save_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("could not create the saves folder: {e}"))?;
        }
    }
    snapshot_save_protecting(save_path, &snapshots_root_for(save_path), now_ms, true, Some(snapshot))
        .map_err(|e| format!("could not keep a copy of the current save first: {e}"))?;
    write_atomic(save_path, &bytes)?;
    log::info!("Restored {} from snapshot {}", save_path.display(), snapshot.display());
    Ok(restored)
}

/// Load a world save from a JSON file at the given path.
pub fn load_world(path: &Path) -> Result<WorldSave, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read save file: {e}"))?;

    let save: WorldSave = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialize world save: {e}"))?;

    log::info!("World loaded from {}", path.display());
    Ok(save)
}

/// List all save files in the saves directory with their names and timestamps.
///
/// Returns a list of `(name, timestamp)` pairs sorted by timestamp descending
/// (most recent first).
pub fn list_saves(saves_dir: &Path) -> Vec<(String, u64)> {
    let entries = match std::fs::read_dir(saves_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut saves: Vec<(String, u64)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let path = e.path();
            let json = std::fs::read_to_string(&path).ok()?;
            let save: WorldSave = serde_json::from_str(&json).ok()?;
            Some((save.name, save.timestamp))
        })
        .collect();

    // Sort by timestamp, most recent first.
    saves.sort_by(|a, b| b.1.cmp(&a.1));
    saves
}

/// Auto-save the world to `auto_save.json` in the given directory.
pub fn auto_save(saves_dir: &Path, save: &WorldSave) {
    let path = saves_dir.join("auto_save.json");
    match save_world(&path, save) {
        Ok(_) => log::info!("Auto-save complete"),
        Err(e) => log::error!("Auto-save failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_save() -> WorldSave {
        WorldSave {
            name: "Test Save".to_string(),
            timestamp: 1700000000,
            game_time: 3600.0,
            player_position: [10.0, 5.0, -3.0],
            player_rotation: [0.0, 0.0, 0.0, 1.0],
            player_health: 100.0,
            inventory: vec![
                ("wood".to_string(), 50),
                ("stone".to_string(), 25),
            ],
            skills: {
                let mut m = HashMap::new();
                m.insert("mining".to_string(), (5, 1200));
                m.insert("farming".to_string(), (3, 450));
                m
            },
            constructions: vec![
                ConstructionSave {
                    blueprint_id: "wooden_wall".to_string(),
                    position: [20.0, 0.0, 15.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [2.0, 3.0, 0.2],
                    health: 100.0,
                    max_health: 100.0,
                    provides: None,
                    building: None,
                    uid: 1,
                    open: false,
                    site: None,
                },
            ],
            crafts: Vec::new(),
            progress_saved: true,
            weather_state: "clear".to_string(),
            kind: "offline".to_string(),
            design: "fibonacci".to_string(),
            character_name: "Test Character".to_string(),
            appearance: Default::default(),
            outfit: Default::default(),
            placed_items: None,
            deployed_vehicles: Vec::new(),
            crops: Vec::new(),
            crop_soil: Vec::new(),
            crop_pollination: Vec::new(),
            crop_picking: Vec::new(),
            inventory_state: Vec::new(),
            soil_memory: Default::default(),
            credits: -1,
            quests: None,
            herd: Vec::new(),
            asteroids: None,
            drone: None,
            mining_order: None,
            machine_levels: Vec::new(),
            ship_supply: Default::default(),
            settled_trades: Vec::new(),
        }
    }

    #[test]
    fn new_offline_has_defaults() {
        let h = WorldSave::new_offline("My Homestead", "fibonacci");
        assert_eq!(h.kind, "offline");
        assert_eq!(h.design, "fibonacci");
        assert_eq!(h.name, "My Homestead");
        assert!(h.inventory.is_empty());
    }

    #[test]
    fn round_trip_save_load() {
        let dir = std::env::temp_dir().join(format!("humanity_test_saves_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_roundtrip.json");

        let save = test_save();
        save_world(&path, &save).expect("save should succeed");
        let loaded = load_world(&path).expect("load should succeed");

        assert_eq!(loaded.name, save.name);
        assert_eq!(loaded.timestamp, save.timestamp);
        assert!((loaded.game_time - save.game_time).abs() < f64::EPSILON);
        assert_eq!(loaded.inventory.len(), save.inventory.len());
        assert_eq!(loaded.constructions.len(), save.constructions.len());
        assert_eq!(loaded.kind, save.kind);
        assert_eq!(loaded.design, save.design);

        // Clean up
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn legacy_save_without_kind_design_defaults() {
        // A pre-v0.380 save has no kind/design fields; serde defaults must fill them
        // (kind=offline, design=fibonacci) so old saves load unchanged.
        let dir = std::env::temp_dir().join(format!("humanity_test_legacy_save_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("legacy.json");
        let legacy = r#"{"name":"Old","timestamp":1,"game_time":0.0,"player_position":[0.0,0.0,0.0],"player_rotation":[0.0,0.0,0.0,1.0],"player_health":100.0,"inventory":[],"skills":{},"constructions":[],"weather_state":"clear"}"#;
        std::fs::write(&path, legacy).unwrap();
        let loaded = load_world(&path).expect("legacy load should succeed");
        assert_eq!(loaded.kind, "offline");
        assert_eq!(loaded.design, "fibonacci");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    // ── Save snapshots and crash-safe writes (2026-10-03) ──

    /// A throwaway tree shaped like the app data dir: `<tmp>/saves/home.json`,
    /// whose snapshots then land in `<tmp>/backups/saves/home/`, never in the
    /// player's real backups folder.
    fn snap_tree(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("hos_snapshot_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("saves")).unwrap();
        (root.clone(), root.join("saves").join("home.json"))
    }

    /// A moment well into 2025, so the file names carry a real date.
    const T0: u64 = 1_759_400_000_123;

    /// Rotation keeps exactly SNAPSHOTS_KEPT, and they are the newest ones, in
    /// order, each holding the version that was about to be overwritten.
    ///
    /// Red check (2026-10-03): with the `prune_snapshots(...)` call removed
    /// from `snapshot_save_protecting`, this failed with
    /// `assertion left == right failed: rotation keeps exactly the newest 10
    /// (left: 13, right: 10)`. Restored byte for byte, it passes.
    #[test]
    fn rotation_keeps_exactly_n_and_the_newest() {
        let (root, save) = snap_tree("rotation");
        let snaps_root = snapshots_root_for(&save);
        assert_eq!(snaps_root, root.join("backups").join("saves"));
        let spacing = SNAPSHOT_SPACING_SECS * 1000;
        let total = SNAPSHOTS_KEPT + 3;
        // Write version 0 (nothing to overwrite yet), then versions 1..=total,
        // each one spacing later, so every overwrite keeps a snapshot.
        for i in 0..=total {
            let at = T0 + i as u64 * spacing;
            write_save_bytes(&save, format!("version {i}").as_bytes(), &snaps_root, at).unwrap();
        }
        let kept = list_snapshots(&snaps_root.join("home"));
        assert_eq!(kept.len(), SNAPSHOTS_KEPT, "rotation keeps exactly the newest {SNAPSHOTS_KEPT}");
        for (k, snap) in kept.iter().enumerate() {
            // Newest first: the snapshot taken by write `total - k` holds the
            // version that write replaced, and its name carries that moment.
            let replaced = total - 1 - k;
            assert_eq!(std::fs::read_to_string(&snap.path).unwrap(), format!("version {replaced}"));
            assert_eq!(snap.taken_ms, T0 + (replaced as u64 + 1) * spacing, "the time read back from the name");
        }
        assert_eq!(std::fs::read_to_string(&save).unwrap(), format!("version {total}"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Automatic snapshots wait out SNAPSHOT_SPACING_SECS, and a forced one
    /// does not. (What a clock set back does to the spacing has its own two
    /// tests below; this test used to end with a "future-stamped newest does
    /// not block" step, which only checked the half that could not lose
    /// history.)
    ///
    /// Red checks:
    /// - 2026-10-03 (build): with the `if !force { ... }` spacing block
    ///   removed from `snapshot_save_protecting`, this failed with
    ///   `assertion left == right failed: a save 1 s after the last snapshot
    ///   keeps no new one (left: 2, right: 1)`.
    /// - 2026-10-03 (after the review fixes, re-run because the test changed):
    ///   the same break failed with the same message, `left: 2, right: 1`.
    /// Restored byte for byte, it passes.
    #[test]
    fn automatic_snapshots_wait_out_the_spacing() {
        let (root, save) = snap_tree("spacing");
        let snaps_root = snapshots_root_for(&save);
        let slot = snaps_root.join("home");
        let spacing = SNAPSHOT_SPACING_SECS * 1000;
        write_save_bytes(&save, b"a", &snaps_root, T0).unwrap();
        assert_eq!(list_snapshots(&slot).len(), 0, "a first save has nothing to keep");
        write_save_bytes(&save, b"b", &snaps_root, T0 + 1_000).unwrap();
        assert_eq!(list_snapshots(&slot).len(), 1, "the first overwrite keeps one");
        write_save_bytes(&save, b"c", &snaps_root, T0 + 2_000).unwrap();
        assert_eq!(list_snapshots(&slot).len(), 1, "a save 1 s after the last snapshot keeps no new one");
        write_save_bytes(&save, b"d", &snaps_root, T0 + 1_000 + spacing).unwrap();
        assert_eq!(list_snapshots(&slot).len(), 2, "a save after the spacing keeps another");
        assert!(snapshot_save(&save, &snaps_root, T0 + 1_001 + spacing, true).unwrap().is_some());
        assert_eq!(list_snapshots(&slot).len(), 3, "a forced snapshot ignores the spacing");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Fill a slot the way play does: versions 0..=SNAPSHOTS_KEPT written one
    /// spacing apart from T0, so the slot holds "version 0" .. "version 9",
    /// "version k" stamped T0 + (k + 1) spacings, and the save holds
    /// "version 10".
    fn fill_slot(save: &Path, snaps_root: &Path) {
        let spacing = SNAPSHOT_SPACING_SECS * 1000;
        for i in 0..=SNAPSHOTS_KEPT {
            write_save_bytes(save, format!("version {i}").as_bytes(), snaps_root, T0 + i as u64 * spacing).unwrap();
        }
    }

    /// What each kept snapshot of a slot holds, newest first.
    fn slot_contents(slot: &Path) -> Vec<String> {
        list_snapshots(slot).iter().map(|s| std::fs::read_to_string(&s.path).unwrap()).collect()
    }

    /// Clicking "Snapshot now" again and again cannot push the history out.
    /// The first click keeps the save (it changed since the last copy); every
    /// click after it finds the save unchanged and keeps nothing.
    ///
    /// Red check (2026-10-03): with the "Unchanged since the last copy" loop
    /// removed from `snapshot_save_protecting`, this failed with
    /// `ten clicks keep one new copy, and only version 0 rotated out`,
    /// `left: ["version 10" ten times], right: ["version 10", "version 9",
    /// ..., "version 1"]`: ten clicks had replaced the whole history, the
    /// reviewer's finding exactly. Restored byte for byte, it passes.
    #[test]
    fn repeated_snapshot_now_clicks_keep_the_history() {
        let (root, save) = snap_tree("clicks");
        let snaps_root = snapshots_root_for(&save);
        let slot = snaps_root.join("home");
        fill_slot(&save, &snaps_root);
        let click_at = T0 + (SNAPSHOTS_KEPT as u64 + 1) * SNAPSHOT_SPACING_SECS * 1000;
        // Ten forced snapshots a second apart, the save unchanged throughout.
        let kept_a_copy: Vec<bool> = (0..10u64)
            .map(|click| snapshot_save(&save, &snaps_root, click_at + click * 1_000, true).unwrap().is_some())
            .collect();
        // The history first, so a regression shows what it destroyed.
        let mut expected = vec!["version 10".to_string()];
        expected.extend((1..=9).rev().map(|v| format!("version {v}")));
        assert_eq!(
            slot_contents(&slot),
            expected,
            "ten clicks keep one new copy, and only version 0 rotated out"
        );
        assert!(kept_a_copy[0], "the first click keeps the save, which changed since the last copy");
        assert!(kept_a_copy[1..].iter().all(|k| !k), "a click on an unchanged save keeps nothing");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A clock set back an hour does not switch the spacing off. The
    /// reviewer's case: 16 minutes of 2-minute saves used to keep a copy at
    /// every save and roll six old copies out. Now they keep none, and the
    /// history is whole. Saving on, the first new copy comes 15 minutes after
    /// the clock has passed the newest copy's stamp: 38 saves (76 minutes)
    /// keep exactly one.
    ///
    /// Why none in the first 16 minutes, when a right clock would have kept
    /// one: the spacing is measured from the newest copy stamped at or before
    /// now, and as the set-back clock advances it walks into the copies it
    /// had already stamped ("version 6" at 15 minutes, "version 7" at 30, and
    /// so on). Each counts as the previous copy in turn, so the clock has to
    /// get past them before a new one is due. A gap in new copies is the safe
    /// side of that trade: the old copies are all still there.
    ///
    /// Red check (2026-10-03): with the spacing measured from the newest by
    /// name again (`kept.first()` with the old `taken_ms <= now_ms &&` guard,
    /// so a future-stamped newest skipped the check), this failed with
    /// `16 minutes of saves after the clock went back keep the history whole`,
    /// `left: ["version 9", "version 8", "version 7", "late 7", "version 6",
    /// "late 6", "late 5", "late 4", "late 3", "late 2"], right: ["version 9",
    /// ..., "version 0"]`: every save kept a copy and versions 0 to 5 were
    /// gone, the reviewer's finding exactly. Restored byte for byte, it passes.
    #[test]
    fn a_clock_set_back_does_not_switch_the_spacing_off() {
        let (root, save) = snap_tree("clock_back");
        let snaps_root = snapshots_root_for(&save);
        let slot = snaps_root.join("home");
        fill_slot(&save, &snaps_root);
        let history: Vec<String> = (0..=9).rev().map(|v| format!("version {v}")).collect();
        // The newest copy ("version 9") is stamped T0 + 10 spacings; the
        // clock now reads an hour earlier than that, so the newest four
        // copies are "in the future" and "version 5" is stamped exactly now.
        let newest_stamp = T0 + (SNAPSHOTS_KEPT as u64) * SNAPSHOT_SPACING_SECS * 1000;
        let back = newest_stamp - 3_600_000;
        for k in 1..=8u64 {
            write_save_bytes(&save, format!("late {k}").as_bytes(), &snaps_root, back + k * 120_000).unwrap();
        }
        assert_eq!(
            slot_contents(&slot),
            history,
            "16 minutes of saves after the clock went back keep the history whole"
        );
        // Save 37 is 14 minutes past the newest stamp, save 38 is 16 minutes
        // past it: only save 38 keeps a copy (of "late 37", the save it
        // overwrote), and only "version 0" rotates out.
        for k in 9..=38u64 {
            write_save_bytes(&save, format!("late {k}").as_bytes(), &snaps_root, back + k * 120_000).unwrap();
        }
        let mut expected = vec!["late 37".to_string()];
        expected.extend(history[..SNAPSHOTS_KEPT - 1].iter().cloned());
        assert_eq!(slot_contents(&slot), expected, "76 minutes of saves keep exactly one copy");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// With the clock behind EVERY kept snapshot, a new one is still kept
    /// (its name sorts oldest, so its own prune used to delete it at once),
    /// none of the older copies is lost, and the spacing still holds, measured
    /// from that new one.
    ///
    /// Red checks (2026-10-03):
    /// - with the new copy left out of its own prune's protect list (`vec![]`
    ///   instead of `vec![target.as_path()]`), this failed with `assertion
    ///   left == right failed: a copy is kept while the clock is behind every
    ///   snapshot (left: 10, right: 11)`;
    /// - with the spacing measured from the newest by name again (the same
    ///   break as the test above), it failed with `the spacing holds while
    ///   the clock is behind`, the behind-copy holding `"behind 7"` where it
    ///   should still hold `"version 10"`.
    /// Each restored byte for byte, it passes.
    #[test]
    fn with_every_snapshot_in_the_future_a_new_one_is_still_kept() {
        let (root, save) = snap_tree("all_future");
        let snaps_root = snapshots_root_for(&save);
        let slot = snaps_root.join("home");
        fill_slot(&save, &snaps_root);
        let history: Vec<String> = (0..=9).rev().map(|v| format!("version {v}")).collect();
        // A day before the oldest copy.
        let behind = T0 - 86_400_000;
        write_save_bytes(&save, b"behind 1", &snaps_root, behind).unwrap();
        let after_one = slot_contents(&slot);
        assert_eq!(after_one.len(), SNAPSHOTS_KEPT + 1, "a copy is kept while the clock is behind every snapshot");
        assert_eq!(after_one[..SNAPSHOTS_KEPT], history[..], "none of the older copies is lost");
        assert_eq!(after_one[SNAPSHOTS_KEPT], "version 10", "the new copy holds the save it replaced");
        // Saves for the next 14 minutes keep nothing: spaced from that copy.
        // Compared by contents, not by count: a copy at every save would also
        // leave eleven files (each new one replacing the last), only holding
        // a later save.
        for k in 1..=7u64 {
            write_save_bytes(&save, format!("behind {}", k + 1).as_bytes(), &snaps_root, behind + k * 120_000).unwrap();
        }
        assert_eq!(slot_contents(&slot), after_one, "the spacing holds while the clock is behind");
        // 16 minutes on, the next copy replaces the previous one, not an old one.
        write_save_bytes(&save, b"behind 9", &snaps_root, behind + 8 * 120_000).unwrap();
        let after_two = slot_contents(&slot);
        assert_eq!(after_two[..SNAPSHOTS_KEPT], history[..], "still none of the older copies is lost");
        assert_eq!(after_two[SNAPSHOTS_KEPT..], ["behind 8".to_string()], "the newer behind-copy replaced the older one");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A crash after the new save's bytes went to the temp file and before
    /// the rename leaves the previous save byte for byte, still loadable, and
    /// the next save goes through over the leftover temp file.
    ///
    /// Red check (2026-10-03): with `stage_write` writing straight to `path`
    /// (the old in-place `std::fs::write` behaviour) instead of the staging
    /// path, this failed at the first assertion with `the previous save is
    /// byte for byte intact` (the save held half of the unfinished one).
    /// Restored byte for byte, it passes.
    #[test]
    fn a_crash_between_write_and_rename_leaves_the_previous_save() {
        let (root, save) = snap_tree("crash");
        let mut before_save = test_save();
        before_save.name = "Before the crash".to_string();
        save_world(&save, &before_save).unwrap();
        let before = std::fs::read(&save).unwrap();

        // The next save dies part-way: only half its bytes were written, and
        // the process ended before the rename.
        let mut unfinished = test_save();
        unfinished.name = "Never finished".to_string();
        let full = serde_json::to_string_pretty(&unfinished).unwrap();
        stage_write(&save, &full.as_bytes()[..full.len() / 2]).unwrap();

        assert!(std::fs::read(&save).unwrap() == before, "the previous save is byte for byte intact");
        assert_eq!(load_world(&save).unwrap().name, "Before the crash");
        // The half-written temp file is not mistaken for a save.
        assert_eq!(list_saves(&root.join("saves")).len(), 1);

        // The next save replaces the leftover temp file and lands whole.
        save_world(&save, &unfinished).unwrap();
        assert_eq!(load_world(&save).unwrap().name, "Never finished");
        assert!(!staging_path(&save).exists(), "no temp file is left behind");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Restoring a snapshot puts its exact bytes back as the save, and the
    /// save it replaced is kept as the newest snapshot (even inside the
    /// spacing), so a restore can be undone. A damaged snapshot is refused
    /// and changes nothing.
    ///
    /// Red checks (2026-10-03):
    /// - with the `snapshot_save_protecting(...)` call removed from
    ///   `restore_snapshot`, this failed with `assertion left == right
    ///   failed: the restore kept a copy of what it replaced (left: 1,
    ///   right: 2)`;
    /// - with the `serde_json::from_slice::<WorldSave>` check removed (the
    ///   bytes restored without parsing), it failed with `a damaged
    ///   snapshot is refused`.
    /// Each restored byte for byte, it passes.
    #[test]
    fn restoring_a_snapshot_puts_its_bytes_back_and_keeps_what_it_replaced() {
        let (root, save) = snap_tree("restore");
        let snaps_root = snapshots_root_for(&save);
        let slot = snaps_root.join("home");
        let mut good = test_save();
        good.name = "Good home".to_string();
        let good_bytes = serde_json::to_string_pretty(&good).unwrap().into_bytes();
        let mut bad = test_save();
        bad.name = "Spoiled home".to_string();
        let bad_bytes = serde_json::to_string_pretty(&bad).unwrap().into_bytes();

        write_save_bytes(&save, &good_bytes, &snaps_root, T0).unwrap();
        write_save_bytes(&save, &bad_bytes, &snaps_root, T0 + 1_000).unwrap();
        let snaps = list_snapshots(&slot);
        assert_eq!(snaps.len(), 1);
        let good_snapshot = snaps[0].path.clone();
        assert!(std::fs::read(&good_snapshot).unwrap() == good_bytes);

        // Restore, well inside the spacing of that snapshot.
        let restored = restore_snapshot(&good_snapshot, &save, T0 + 2_000).unwrap();
        assert_eq!(restored.name, "Good home");
        assert!(std::fs::read(&save).unwrap() == good_bytes, "the snapshot's exact bytes are the save again");
        let after = list_snapshots(&slot);
        assert_eq!(after.len(), 2, "the restore kept a copy of what it replaced");
        assert!(std::fs::read(&after[0].path).unwrap() == bad_bytes, "the newest snapshot is the replaced save");
        assert!(good_snapshot.exists(), "the snapshot restored from stays");

        // A damaged snapshot is refused and nothing changes.
        let broken = slot.join(snapshot_file_name("home", T0 + 3_000, 0));
        std::fs::write(&broken, b"{ not a save").unwrap();
        assert!(restore_snapshot(&broken, &save, T0 + 4_000).is_err(), "a damaged snapshot is refused");
        assert!(std::fs::read(&save).unwrap() == good_bytes);
        assert_eq!(list_snapshots(&slot).len(), 3, "no copy kept for a refused restore");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Stepping back through the copies one Restore at a time keeps the
    /// whole history (review, 2026-10-03). After a Restore the live save is a
    /// byte copy of the snapshot just restored, and that snapshot is usually
    /// not the newest; the unchanged check used to compare only with the
    /// newest, so the NEXT Restore kept a duplicate and pushed the oldest copy
    /// out. A full slot of ten real saves, restored to version 6 and then to
    /// version 4, must still hold all ten versions plus the save the first
    /// Restore replaced, minus only the one rotation that copy needed.
    ///
    /// Red check (2026-10-03): with the unchanged check comparing only the
    /// previous and the newest copy again (the round-two code), this failed
    /// with `two restores keep every version but the one rotation`, left
    /// ["v10", "v2", ..., "v6", "v6", ..., "v9"]: v1 was traded for a second v6.
    /// Restored byte for byte, it passes.
    #[test]
    fn stepping_back_through_restores_keeps_the_history() {
        let (root, save) = snap_tree("restore_steps");
        let snaps_root = snapshots_root_for(&save);
        let slot = snaps_root.join("home");
        let spacing = SNAPSHOT_SPACING_SECS * 1000;
        let save_named = |n: &str| {
            let mut s = test_save();
            s.name = n.to_string();
            serde_json::to_string_pretty(&s).unwrap().into_bytes()
        };
        // Versions 0 to 10 written 15 minutes apart: the slot keeps 0 to 9,
        // and version 10 is the live save.
        for i in 0..=SNAPSHOTS_KEPT {
            write_save_bytes(&save, &save_named(&format!("v{i}")), &snaps_root, T0 + i as u64 * spacing).unwrap();
        }
        let names = |slot: &Path| -> Vec<String> {
            list_snapshots(slot)
                .iter()
                .map(|s| serde_json::from_slice::<WorldSave>(&std::fs::read(&s.path).unwrap()).unwrap().name)
                .collect()
        };
        let path_of = |v: &str| {
            list_snapshots(&slot)
                .into_iter()
                .find(|s| serde_json::from_slice::<WorldSave>(&std::fs::read(&s.path).unwrap()).unwrap().name == v)
                .unwrap()
                .path
        };
        let now = T0 + (SNAPSHOTS_KEPT as u64 + 2) * spacing;
        restore_snapshot(&path_of("v6"), &save, now).unwrap();
        restore_snapshot(&path_of("v4"), &save, now + 1_000).unwrap();
        let mut got = names(&slot);
        got.sort();
        // The first Restore kept the live save v10 (it was new) and rotated v0
        // out; the second found v6 already kept and kept nothing.
        let mut want: Vec<String> = (1..=10).map(|v| format!("v{v}")).collect();
        want.sort();
        assert_eq!(got, want, "two restores keep every version but the one rotation");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Restoring the OLDEST snapshot of a full slot must not prune it away
    /// in the same step (the restore's own copy makes one too many).
    ///
    /// Red check (2026-10-03): with `restore_snapshot` passing `None` instead
    /// of `Some(snapshot)` as the protected path, this failed with `the
    /// snapshot restored from survives the restore's own pruning`.
    /// Restored byte for byte, it passes.
    #[test]
    fn restoring_the_oldest_of_a_full_slot_keeps_it() {
        let (root, save) = snap_tree("restore_oldest");
        let snaps_root = snapshots_root_for(&save);
        let slot = snaps_root.join("home");
        let spacing = SNAPSHOT_SPACING_SECS * 1000;
        for i in 0..=SNAPSHOTS_KEPT {
            let mut s = test_save();
            s.name = format!("home {i}");
            let bytes = serde_json::to_string_pretty(&s).unwrap();
            write_save_bytes(&save, bytes.as_bytes(), &snaps_root, T0 + i as u64 * spacing).unwrap();
        }
        let full = list_snapshots(&slot);
        assert_eq!(full.len(), SNAPSHOTS_KEPT);
        let oldest = full.last().unwrap().path.clone();
        let restored = restore_snapshot(&oldest, &save, T0 + (SNAPSHOTS_KEPT as u64 + 1) * spacing).unwrap();
        assert_eq!(restored.name, "home 0");
        assert!(oldest.exists(), "the snapshot restored from survives the restore's own pruning");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn list_saves_works() {
        let dir = std::env::temp_dir().join(format!("humanity_test_list_saves_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let mut s1 = test_save();
        s1.name = "Save One".to_string();
        s1.timestamp = 1000;
        save_world(&dir.join("save1.json"), &s1).unwrap();

        let mut s2 = test_save();
        s2.name = "Save Two".to_string();
        s2.timestamp = 2000;
        save_world(&dir.join("save2.json"), &s2).unwrap();

        let saves = list_saves(&dir);
        assert_eq!(saves.len(), 2);
        // Most recent first
        assert_eq!(saves[0].0, "Save Two");
        assert_eq!(saves[1].0, "Save One");

        // Clean up
        let _ = std::fs::remove_file(dir.join("save1.json"));
        let _ = std::fs::remove_file(dir.join("save2.json"));
        let _ = std::fs::remove_dir(&dir);
    }
}
