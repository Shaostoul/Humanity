//! What each home machine drew over its last game day (2026-10-22, the
//! operator's question of 2026-10-21: "Do rooms not automatically calculate
//! their volumes on the fly?").
//!
//! The rooms do: the ship's flood fill measures every room as it stands, and
//! the air model runs each fan, humidifier, air handler, scrubber and heater
//! at the share of the time its controller needs in THAT room. The static
//! power meters did not follow: they charged a controller-driven machine the
//! `average_watts` a person had measured once in the shipped home
//! (`machines::MachinePower::Consumer`), so a room made bigger or smaller left
//! its fan's figure where it was, and a player's own rooms and machines would
//! never have one.
//!
//! So the machines' power tick (`ElectricalSystem`, `record_world`) writes
//! down, every tick and on the game clock, what each placed machine's
//! `PowerConsumer` drew: the draw the system that runs it set (the air step
//! for the air machines and heaters, `farming::humidity`, `life_support`,
//! `farming::heat`; the garden's timer for a grow light; the crafting system
//! for a work station; a steady load's own watts), while the island powered
//! it, and what it asked for while the island had shed it. The record is a
//! rolling day: the last 24 whole game hours, an hour at a time, so the
//! figure follows a changed room within a day. Every place that shows or uses
//! a machine's daily energy reads it (`machines::MachineHome::day_load`): the
//! Construction page's Usage meter and machine card, the Home page's loops,
//! and the time away (`crafting::away::day_power_balance`). Until a machine
//! has run a whole day the data's figure stands in for it, and the pages say
//! which figures are measured and which are estimated.
//!
//! WHAT IS CHARGED. A day's NEED: what the machine drew plus what it asked
//! for while it was shed. The meters answer what the home needs to run its
//! machines; an hour a fan sat shed for want of power is an hour its room
//! went without, not an hour the home needed less, and the time away must not
//! read a shed load's hours as spare power. In the default Station-supplied
//! mode nothing is shed (the ship's reactor feeds every island), so need and
//! draw are the same; the record keeps the shed part apart so a page can say
//! how much of it went unmet.
//!
//! The record is kept by machine instance id (`MachineInstanceId`), with the
//! machine's type: a different machine placed under the same id starts a new
//! record. It is saved with the home (`persistence::WorldSave::machine_day_energy`)
//! and put back with it (`save_load::resume_home`). The time away adds
//! nothing to it: only time the simulation ran is a measured day.
//!
//! WHY IT LIVES WITH THE MACHINES (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was `machines::day_energy`, and the machines' own meters
//! (`MachineHome::day_draw`, `MeterBasis::recorded`) read it, so the ship reached up into the
//! simulation. The record is the home's machines' data; the simulation writes it (the
//! ElectricalSystem calls `record_world`) and its tests stay there (systems/day_energy_tests.rs).

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::hot_reload::data_store::DataStore;

/// DataStore key of the record, `Mutex<DayEnergy>`.
pub const KEY: &str = "machine_day_energy";
/// Game seconds in one of the record's hours: an hour of the game clock is always 3,600 game
/// seconds (`systems::time::SECONDS_PER_HOUR`; a calendar changes how many make a day, never
/// the hour). Written out here because the simulation's clock is above the ship, and the test
/// `the_record_hour_is_the_clock_hour` (systems/day_energy_tests.rs) holds the two equal.
pub const HOUR_S: f64 = 3600.0;
/// The hours in a recorded day. 24 whatever the calendar's day length: every
/// daily figure the meters show is per 24 hours (kWh a day = W x 24).
pub const DAY_HOURS: usize = 24;
/// How close to a whole hour counts as whole, game seconds (`MachineDay::add`).
const HOUR_EPSILON_S: f64 = 1e-6;

/// Watt-hours in one of the record's hours.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct HourEnergy {
    /// What the machine drew from its island.
    #[serde(default)]
    pub drawn_wh: f64,
    /// What it asked for while its island had shed it, and so did not get.
    #[serde(default)]
    pub shed_wh: f64,
}

/// One machine's rolling day.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MachineDay {
    /// The machine type the record is of (its catalog id).
    #[serde(default)]
    pub machine: String,
    /// The whole hours recorded, oldest first, at most `DAY_HOURS`.
    #[serde(default)]
    pub hours: VecDeque<HourEnergy>,
    /// The hour in progress.
    #[serde(default)]
    pub current: HourEnergy,
    /// Game seconds of the hour in progress recorded so far.
    #[serde(default)]
    pub current_s: f64,
}

/// A machine's last day, watt-hours.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DayFigure {
    pub drawn_wh: f64,
    pub shed_wh: f64,
}

impl DayFigure {
    /// What the machine needed over the day: drawn, and asked for while shed.
    pub fn need_wh(&self) -> f64 {
        self.drawn_wh + self.shed_wh
    }

    /// The day's need as an average draw, W.
    pub fn average_w(&self) -> f64 {
        self.need_wh() / DAY_HOURS as f64
    }
}

impl MachineDay {
    /// Record `secs` game seconds of drawing `drawn_w` while asking for
    /// `asked_w` (more than drawn only while shed), across hour boundaries.
    /// Returns how many hours it made whole.
    pub fn add(&mut self, drawn_w: f64, asked_w: f64, secs: f64) -> usize {
        let mut whole = 0;
        if !(secs > 0.0) {
            return whole;
        }
        let drawn_w = drawn_w.max(0.0);
        let shed_w = (asked_w - drawn_w).max(0.0);
        // A stretch longer than the day only repeats it: the last day is all
        // at this draw however much longer it ran.
        let mut left = secs.min((DAY_HOURS as f64 + 1.0) * HOUR_S);
        while left > 0.0 {
            let room = (HOUR_S - self.current_s).max(0.0);
            let take = left.min(room);
            self.current.drawn_wh += drawn_w * take / 3600.0;
            self.current.shed_wh += shed_w * take / 3600.0;
            left -= take;
            // Within a microsecond of whole is whole: a frame's seconds (1/60, say) do not add
            // up to the hour exactly, and a sliver short would hold the hour a frame late.
            if take + HOUR_EPSILON_S >= room {
                // The hour is whole: it joins the day, the oldest leaves it.
                self.hours.push_back(self.current);
                while self.hours.len() > DAY_HOURS {
                    self.hours.pop_front();
                }
                self.current = HourEnergy::default();
                self.current_s = 0.0;
                whole += 1;
            } else {
                self.current_s += take;
            }
        }
        whole
    }

    /// The last day, once a whole one is recorded.
    pub fn day(&self) -> Option<DayFigure> {
        (self.hours.len() >= DAY_HOURS).then(|| {
            self.hours.iter().fold(DayFigure::default(), |a, h| DayFigure { drawn_wh: a.drawn_wh + h.drawn_wh, shed_wh: a.shed_wh + h.shed_wh })
        })
    }

    /// Hours recorded so far, up to a day.
    pub fn hours_recorded(&self) -> f64 {
        (self.hours.len() as f64 + self.current_s / HOUR_S).min(DAY_HOURS as f64)
    }
}

/// Every home machine's rolling day, by machine instance id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DayEnergy {
    #[serde(default)]
    pub machines: BTreeMap<String, MachineDay>,
    /// Counts each whole hour recorded and each restore, so a page can copy
    /// the record only when it changed. Not saved.
    #[serde(skip)]
    pub revision: u64,
}

impl DayEnergy {
    pub fn is_empty(&self) -> bool {
        self.machines.is_empty()
    }

    /// Record `secs` of machine `id` (of type `machine`) drawing `drawn_w`
    /// while asking for `asked_w`. A record of another type under the id
    /// starts afresh.
    pub fn record(&mut self, id: &str, machine: &str, drawn_w: f64, asked_w: f64, secs: f64) {
        let day = match self.machines.get_mut(id) {
            Some(d) => d,
            None => self.machines.entry(id.to_string()).or_default(),
        };
        if day.machine != machine {
            *day = MachineDay { machine: machine.to_string(), ..Default::default() };
        }
        if day.add(drawn_w, asked_w, secs) > 0 {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// The record of machine `id`, if it is of type `machine`.
    pub fn of(&self, id: &str, machine: &str) -> Option<&MachineDay> {
        self.machines.get(id).filter(|d| d.machine == machine)
    }

    /// Machine `id`'s last day, once it has run a whole one as a `machine`.
    pub fn day_of(&self, id: &str, machine: &str) -> Option<DayFigure> {
        self.of(id, machine).and_then(MachineDay::day)
    }

    /// Hours of machine `id` recorded so far, up to a day.
    pub fn hours_of(&self, id: &str, machine: &str) -> f64 {
        self.of(id, machine).map_or(0.0, MachineDay::hours_recorded)
    }
}

/// Register an empty record (lib.rs, at startup, beside the power readout).
pub fn register(data: &mut DataStore) {
    data.insert(KEY, std::sync::Mutex::new(DayEnergy::default()));
}

/// Record one tick of the machines' power: `secs` game seconds of every
/// placed machine's `PowerConsumer`, its draw while its island powered it and
/// its request while shed. Called by the ElectricalSystem after it has
/// decided who is shed, with the game seconds it ran (a no-op with no record
/// registered).
pub fn record_world(world: &hecs::World, data: &DataStore, secs: f64) {
    use crate::ecs::components::{MachineInstanceId, MachineType, PowerConsumer};
    if !(secs > 0.0) {
        return;
    }
    let Some(Ok(mut rec)) = data.get::<std::sync::Mutex<DayEnergy>>(KEY).map(|m| m.lock()) else { return };
    for (_, (id, kind, pc)) in world.query::<(&MachineInstanceId, Option<&MachineType>, &PowerConsumer)>().iter() {
        let asked = f64::from(pc.draw_watts.max(0.0));
        let drawn = if pc.enabled { asked } else { 0.0 };
        rec.record(&id.0, kind.map_or("", |k| k.0.as_str()), drawn, asked, secs);
    }
}

/// The record as it stands (empty with none registered).
pub fn snapshot(data: &DataStore) -> DayEnergy {
    data.get::<std::sync::Mutex<DayEnergy>>(KEY).and_then(|m| m.lock().ok().map(|r| r.clone())).unwrap_or_default()
}

/// The record's revision (0 with none registered): it moves on with each
/// whole hour recorded and each restore.
pub fn revision(data: &DataStore) -> u64 {
    data.get::<std::sync::Mutex<DayEnergy>>(KEY).and_then(|m| m.lock().ok().map(|r| r.revision)).unwrap_or(0)
}

/// The record for the save: the machines placed in `world`, or the whole
/// record when none is (a save written before the machines spawn keeps it
/// rather than dropping it).
pub fn for_save(world: &hecs::World, data: &DataStore) -> DayEnergy {
    let mut rec = snapshot(data);
    let placed: std::collections::HashSet<String> =
        world.query::<&crate::ecs::components::MachineInstanceId>().iter().map(|(_, id)| id.0.clone()).collect();
    if !placed.is_empty() {
        rec.machines.retain(|id, _| placed.contains(id));
    }
    rec
}

/// Put a saved record back (the save is authoritative; an empty one clears
/// it, so a home loaded over another does not inherit its machines' days).
pub fn restore(data: &DataStore, saved: &DayEnergy) {
    if let Some(Ok(mut r)) = data.get::<std::sync::Mutex<DayEnergy>>(KEY).map(|m| m.lock()) {
        let revision = r.revision.wrapping_add(1);
        *r = saved.clone();
        r.revision = revision;
    }
}

