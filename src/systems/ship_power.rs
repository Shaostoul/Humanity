//! The ship's reactor as the homes' baseline supply, and the ledger of what
//! each home draws from the ship (2026-09-27, the operator's decision in
//! docs/PRIORITIES.md Blocked 3b: "base power budget on nuclear reactors and
//! then players can build solar and other means of producing electricity ...
//! essentially provide unlimited (at least to start)").
//!
//! THE FEED. Each power island of the player's home carries a feed TAP, a
//! `ShipFeed` entity on that island (`spawn_feed_taps`, from
//! `engine::home_spawn`). In the default Ship life support mode
//! (Station-supplied) a tapped island is tied to the ship's bus: the
//! electrical sim uses the island's own generation first, then its
//! batteries, and the reactor supplies whatever is left, up to the reactor's
//! whole output (data/ship_power.ron), which no home comes near, so a tapped
//! island never sheds a load. What the island makes past its loads and past
//! what its batteries can take goes back to the ship. In the Realistic mode
//! the tie is off (`feed_watts` is None) and the island runs on what it makes
//! and stores, exactly as before. A planet build site never has a tap: the
//! ship is in orbit.
//!
//! THE LEDGER. Every watt-hour a home draws from the ship, and every one it
//! returns, is added to `ShipSupplyLedger` in the DataStore (f64 watt-hours:
//! an f32 total loses the per-frame increment once it passes about 100 kWh,
//! the tank-level bug of `docs/design/ship-life-support.md` section 1). It is
//! keyed by home, then by utility (`Utility::id`: "power" today; water and
//! air when the ship supplies them), so a fleet ledger is the same map summed
//! over homes (`fleet_total`). It is saved with the home
//! (`WorldSave::ship_supply`).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::hot_reload::data_store::DataStore;

/// The shipped data file, so a fresh install has the reactor.
pub const SHIP_POWER_RON: &str = include_str!("../../data/ship_power.ron");
/// DataStore key of the reactor data (`ShipPowerData`).
pub const DATA_KEY: &str = "ship_power";
/// DataStore key of the ledger, `Mutex<ShipSupplyLedger>`.
pub const LEDGER_KEY: &str = "ship_supply";
/// The player's home in the ledger. One home per player today; in a fleet
/// each home's taps carry their own id.
pub const PLAYER_HOME: &str = "home";
/// The ledger's line for electricity (`Utility::Electricity.id()`), in Wh.
pub const POWER: &str = "power";

/// The ship's reactor (data/ship_power.ron).
#[derive(Debug, Clone, Deserialize)]
pub struct ShipPowerData {
    /// Which plant it is, for the cards.
    pub reactor: String,
    /// Its electrical output, watts: the most the ship's bus can give.
    pub electric_watts: f64,
}

impl ShipPowerData {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The data directory's copy, or the shipped one.
    pub fn load() -> Self {
        let path = crate::data_dir().join("ship_power.ron");
        // Disk first; the shipped copy only when the file is missing or does not
        // parse, and then the log says so (embedded_data::note_builtin_copy: a rig
        // refuses a run that served a built-in copy, BUG-133).
        let why = match std::fs::read_to_string(&path) {
            Ok(text) => match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => format!("{} does not parse ({e})", path.display()),
            },
            Err(e) => format!("{} could not be read ({e})", path.display()),
        };
        crate::embedded_data::note_builtin_copy("ship_power.ron", why);
        Self::parse(SHIP_POWER_RON).expect("the shipped data/ship_power.ron parses")
    }
}

/// A home power island's tie to the ship's bus: one per island, carrying the
/// home it belongs to (the ledger's key).
#[derive(Debug, Clone, PartialEq)]
pub struct ShipFeed {
    pub home: String,
}

/// What a home drew from one ship supply and returned to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct SupplyTally {
    #[serde(default)]
    pub drawn: f64,
    #[serde(default)]
    pub returned: f64,
}

/// Every home's supply from the ship: home id, then utility id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShipSupplyLedger {
    #[serde(default)]
    pub homes: BTreeMap<String, BTreeMap<String, SupplyTally>>,
}

impl ShipSupplyLedger {
    /// Add what `home` drew from and returned to the ship's `utility`.
    pub fn record(&mut self, home: &str, utility: &str, drawn: f64, returned: f64) {
        let t = self.homes.entry(home.to_string()).or_default().entry(utility.to_string()).or_default();
        t.drawn += drawn.max(0.0);
        t.returned += returned.max(0.0);
    }

    /// One home's tally of one utility.
    pub fn tally(&self, home: &str, utility: &str) -> SupplyTally {
        self.homes.get(home).and_then(|h| h.get(utility)).copied().unwrap_or_default()
    }

    /// The whole ship's (or fleet's) tally per utility: every home summed.
    pub fn fleet_total(&self) -> BTreeMap<String, SupplyTally> {
        let mut out: BTreeMap<String, SupplyTally> = BTreeMap::new();
        for lines in self.homes.values() {
            for (u, t) in lines {
                let s = out.entry(u.clone()).or_default();
                s.drawn += t.drawn;
                s.returned += t.returned;
            }
        }
        out
    }
}

/// Register the reactor and an empty ledger (from `life_support::register`,
/// at startup).
pub fn register(data_store: &mut DataStore) {
    data_store.insert(DATA_KEY, ShipPowerData::load());
    data_store.insert(LEDGER_KEY, std::sync::Mutex::new(ShipSupplyLedger::default()));
}

/// The watts a tapped island may take from the ship's bus now: the reactor's
/// output in the Station-supplied mode, None in the Realistic mode (the home
/// runs on its own) or with no reactor registered (headless tests).
pub fn feed_watts(data: &DataStore) -> Option<f64> {
    let d = data.get::<ShipPowerData>(DATA_KEY)?;
    (!crate::systems::life_support::is_realistic(data)).then_some(d.electric_watts.max(0.0))
}

/// Add to the ledger (a no-op with none registered).
pub fn record(data: &DataStore, home: &str, utility: &str, drawn: f64, returned: f64) {
    if let Some(Ok(mut l)) = data.get::<std::sync::Mutex<ShipSupplyLedger>>(LEDGER_KEY).map(|m| m.lock()) {
        l.record(home, utility, drawn, returned);
    }
}

/// One home's tally of one utility as it stands (zero with no ledger).
pub fn tally(data: &DataStore, home: &str, utility: &str) -> SupplyTally {
    data.get::<std::sync::Mutex<ShipSupplyLedger>>(LEDGER_KEY)
        .and_then(|m| m.lock().ok().map(|l| l.tally(home, utility)))
        .unwrap_or_default()
}

/// The ledger as it stands, for the save.
pub fn ledger(data: &DataStore) -> ShipSupplyLedger {
    data.get::<std::sync::Mutex<ShipSupplyLedger>>(LEDGER_KEY)
        .and_then(|m| m.lock().ok().map(|l| l.clone()))
        .unwrap_or_default()
}

/// Put a saved ledger back (the save is authoritative).
pub fn restore(data: &DataStore, saved: &ShipSupplyLedger) {
    if let Some(Ok(mut l)) = data.get::<std::sync::Mutex<ShipSupplyLedger>>(LEDGER_KEY).map(|m| m.lock()) {
        *l = saved.clone();
    }
}

/// Spawn one feed tap per island in `islands` for `home`, tagged as a home
/// machine so a world entry's despawn clears them with the rest.
pub fn spawn_feed_taps(world: &mut hecs::World, home: &str, islands: impl IntoIterator<Item = u32>) {
    let mut seen: Vec<u32> = islands.into_iter().collect();
    seen.sort_unstable();
    seen.dedup();
    for island in seen {
        world.spawn((
            crate::ecs::components::HomeMachine,
            ShipFeed { home: home.to_string() },
            crate::ecs::components::PowerCircuit { island },
        ));
    }
}

/// The feed taps in the world, by island (None = entities with no circuit).
pub fn taps(world: &hecs::World) -> HashMap<Option<u32>, String> {
    world
        .query::<(&ShipFeed, Option<&crate::ecs::components::PowerCircuit>)>()
        .iter()
        .map(|(_, (f, pc))| (pc.map(|p| p.island), f.home.clone()))
        .collect()
}

/// A planet build site's power island: a number no home island uses (home
/// islands count up from 0), the same for every piece of the site and across
/// a restart, so the site's generators and its stations meet on it and never
/// on the home's (which the ship's reactor feeds).
pub fn site_island(site: &crate::systems::construction::PlanetSite) -> u32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(site.body.as_bytes());
    for v in [site.origin.x, site.origin.y, site.origin.z] {
        eat(&v.to_bits().to_le_bytes());
    }
    0x8000_0000 | (h as u32 & 0x7fff_ffff)
}

#[cfg(test)]
#[path = "ship_power_tests.rs"]
mod tests;
