//! What changed in the garden, for the readers that keep what they worked out from it (2026-10-31,
//! docs/design/sim-frame-cost.md, "The Garden views by area").
//!
//! The Garden page shows a row for every crop and a few lines for every grow area, and the
//! Inventory wall screen's turn rebuilt all of them (about 4.5 ms of views with the showcase's
//! 2,000 crops, on a busy machine). Since farm-clock rung 2 an area's data changes only when
//! something writes it, so the page keeps each area's rows and builds them again only when one of
//! the numbers below moved. CLAUDE.md's reuse rule: data kept across frames is tagged with a
//! version its source bumps on EVERY change, and a version that misses a write is a stale-cache
//! bug, the very thing the operator distrusts caches for. So every write is listed here, with
//! where it is bumped:
//!
//! - **An area's own version** (`area`): every run of the farm's body (`FarmingSystem::run`)
//!   whose scope names the area, before it acts. That is (a) the area's turn advancing it, (b) the
//!   run that follows a command with no time to settle what the command left, and (c) a reader
//!   bringing it up to now (`farming::settle`): all three are runs over named areas. An area's
//!   run writes only what that area carries (farm-clock.md, "What an area's advance carries": its
//!   crops and their soil, pollination and picking records, its pests, weeds, pH units, banked
//!   nitrogen and feeder bags), never another area's.
//! - **The epoch** (every area at once): a run of the WHOLE farm (every frame with the interval
//!   at every frame, the Dev page's "Turn off all reuse", a burn); the commands run (a harvest, a
//!   sowing, a pest control: a command may write any crop or area, so all of them); the clock's
//!   rebase moving every crop's `planted_at` (`FarmingSystem::tick`); a save loaded and handed
//!   over (`clock::hand_over_resume`, after `save_load::catch_up_world` aged its crops); a save
//!   applied to the world and the home's air replaced at world entry (`touch_all`, called by
//!   `save_load::apply_save_to_world` and `engine::world_load::replace_home_air`).
//! - **The air** (`air`): every step of the rooms' and the home's air (a run with `air_s`), which
//!   also ages the hives' colonies.
//!
//! What is NOT here, because a reader sees it for itself: a crop spawned or despawned by anyone
//! (a sowing, a harvest, the dev showcase, a load) changes the area's list of crops, which the
//! reader compares; the data files and the machine layout (a hot reload, a machine edit publishing
//! the grow plots) change the DataStore's stamp of their key (`DataStore::stamp`); the settings
//! and the live machine state (power, tanks, light) are read by value every time.
//!
//! The numbers live in a DataStore channel ([`KEY`]), one per store, so two farms (two tests on
//! one thread, or side by side) never see each other's: a reader with no channel reads every area
//! as changed every time.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::hot_reload::data_store::DataStore;

/// The DataStore channel (`Mutex<Versions>`), registered with the farm clock's (`clock::register`).
pub const KEY: &str = "garden_versions";

/// The garden's change numbers. Every bump takes the next number of one running count, so no two
/// bumps ever give the same number and a reader compares numbers for equality only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Versions {
    /// Moves when anything may have written every area (see the module notes).
    pub epoch: u64,
    /// Moves on every step of the air.
    pub air: u64,
    /// Each area's own number, by its tag (a crop's `tower_id`, "" for the hand-planted crops); 0
    /// for an area never written since the channel was registered.
    areas: HashMap<String, u64>,
    count: u64,
}

impl Versions {
    fn next(&mut self) -> u64 {
        self.count += 1;
        self.count
    }

    /// The area's own number.
    pub fn area(&self, name: &str) -> u64 {
        self.areas.get(name).copied().unwrap_or(0)
    }

    /// Has any area's own number moved (a run over named areas: the turns, a reader's settle, the
    /// run after a command)?
    pub fn any_area(&self) -> bool {
        !self.areas.is_empty()
    }

    /// Every area may have changed.
    pub fn touch_all(&mut self) {
        self.epoch = self.next();
    }

    /// This area changed.
    pub fn touch_area(&mut self, name: &str) {
        let v = self.next();
        match self.areas.get_mut(name) {
            Some(n) => *n = v,
            None => {
                self.areas.insert(name.to_string(), v);
            }
        }
    }

    /// The air stepped.
    pub fn touch_air(&mut self) {
        self.air = self.next();
    }
}

/// Register the channel (with the farm clock's, at boot).
pub fn register(data: &mut DataStore) {
    data.insert(KEY, Mutex::new(Versions::default()));
}

/// Change the numbers, if the channel is there. False without one.
pub fn with(data: &DataStore, f: impl FnOnce(&mut Versions)) -> bool {
    match data.get::<Mutex<Versions>>(KEY).map(|m| m.lock()) {
        Some(Ok(mut v)) => {
            f(&mut v);
            true
        }
        _ => false,
    }
}

/// Read the numbers: None without the channel, which a reader takes as "everything changed".
pub fn read<R>(data: &DataStore, f: impl FnOnce(&Versions) -> R) -> Option<R> {
    match data.get::<Mutex<Versions>>(KEY).map(|m| m.lock()) {
        Some(Ok(v)) => Some(f(&v)),
        _ => None,
    }
}

/// Something outside the farm wrote the garden (a save applied, the home's air replaced): every
/// area may have changed.
pub fn touch_all(data: &DataStore) {
    with(data, Versions::touch_all);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every bump gives a number never given before, an area's own moves only with its own bump or
    /// none, and without the channel nothing is read.
    #[test]
    fn every_bump_is_a_new_number() {
        let mut data = DataStore::new();
        assert!(read(&data, |v| v.epoch).is_none(), "no channel: nothing to read");
        register(&mut data);
        let before = read(&data, |v| (v.epoch, v.air, v.area("bed_a"), v.area("bed_b"))).unwrap();
        assert_eq!(before, (0, 0, 0, 0));
        with(&data, |v| v.touch_area("bed_a"));
        let a = read(&data, |v| (v.area("bed_a"), v.area("bed_b"))).unwrap();
        assert!(a.0 > 0 && a.1 == 0, "only bed_a moved: {a:?}");
        with(&data, |v| v.touch_air());
        touch_all(&data);
        let after = read(&data, |v| (v.epoch, v.air, v.area("bed_a"))).unwrap();
        assert!(after.0 != after.1 && after.1 != after.2 && after.0 != after.2, "all different: {after:?}");
        assert_eq!(after.2, a.0, "bed_a's own number stays until its own bump");
    }
}
