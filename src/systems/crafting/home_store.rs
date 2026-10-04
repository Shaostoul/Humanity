//! A hand craft draws on the home's storage too (BUG-147, 2026-10-04).
//!
//! Since the vehicle recipes carry real bills of materials (BUG-145), a
//! spacecraft pod takes about 2,200 L of parts and a freighter about 29,000 L,
//! while the backpack holds 65 L. Nobody builds a boat out of their pockets:
//! they build it in the workshop from what is stacked in the shed. So a manual
//! craft takes its inputs from the backpack first, then from the home's storage
//! (the Barn's crates, the garage bags, the chests built in the home), then,
//! for a measure of tap water, from the home's tanks: the same order the
//! automated machines and the build menu already used. What the backpack
//! cannot take of the result goes to home storage too, keeping its grade.
//!
//! WHERE IT COUNTS. Only the player's OWN home, and only while they are where
//! it is: aboard, with their home on this ship. On a planet's ground the home
//! is in orbit (the build menu's rule since 2026-09-27: "on a planet you build
//! from what you carry"), in open space nothing is near, and a guest on a
//! shared ship has no home there (it is put away, `ShipStructure::put_home_away`).
//! A neighbour's home and the ship's shared stores never reach this game at
//! all: a neighbour's home is drawn from the shipped design with nothing in it
//! (`ship::neighbours`), and the relay holds no items of anyone's (its player
//! inventory is always empty; trades flip a status and each game moves its own
//! backpack; the mess hall's meals live on the relay, `ship_stores.rs`). What
//! this game can reach is its own backpack and the "home_stock" mirror of its
//! own organize-layer pool, which `engine::stock_piles::publish_home_stock`
//! builds without the chests built on a planet.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::hot_reload::data_store::DataStore;
use crate::systems::construction::StationsWhere;

/// DataStore key (bool): the player is where their own home's storage is,
/// published each frame by `engine::built_uses::publish_stations`. Absent
/// (tests, headless) counts as yes; the "home_stock" mirror being there at
/// all is the other half of the answer.
pub const HOME_STORAGE_HERE: &str = "home_storage_here";

/// DataStore key: hand-made goods for home storage, as (item, quantity,
/// grade), filed by the main loop (`engine::stock_piles::receive_machine_outputs`)
/// beside what the machines make. The machines' own channel carries no grade
/// because a machine turns out standard goods; a hand craft's grade is the
/// crafter's, and a good put away keeps it.
pub const HAND_MADE_TO_STORAGE: &str = "home_stock_hand_made";

/// Put the hand-made channel in the DataStore (crafting::register, at boot).
pub fn register(store: &mut DataStore) {
    store.insert(HAND_MADE_TO_STORAGE, Mutex::new(Vec::<(String, u32, u8)>::new()));
}

/// Where one input of a craft comes from: the backpack first, then home
/// storage, then (a measure of tap water only) the home's tanks. `short` is
/// what all of them together lack. The Crafting page shows the same split,
/// so it and the craft never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Draw {
    pub pack: u32,
    pub home: u32,
    pub tap: u32,
    pub short: u32,
}

impl Draw {
    /// The split of `need` units given what the backpack holds, what home
    /// storage holds and what the tanks can supply (each 0 where it does not
    /// count).
    pub fn of(need: u32, in_pack: u32, in_home: u32, from_tap: u32) -> Draw {
        let pack = in_pack.min(need);
        let home = in_home.min(need - pack);
        let tap = from_tap.min(need - pack - home);
        Draw { pack, home, tap, short: need - pack - home - tap }
    }
}

/// The home's storage as a hand craft here can reach it: the "home_stock"
/// mirror while the player is where it is, else nothing.
pub struct HomeStore<'a> {
    stock: Option<&'a Mutex<HashMap<String, u32>>>,
    /// Why it does not count here, when it does not (for the notices).
    pub not_here: Option<&'static str>,
}

impl<'a> HomeStore<'a> {
    /// What a hand craft can reach right now (see the module comment).
    pub fn here(data: &'a DataStore) -> Self {
        let place = data
            .get::<Mutex<StationsWhere>>("stations_where")
            .and_then(|m| m.lock().ok().map(|w| w.clone()))
            .unwrap_or_default();
        let at_home = data
            .get::<Mutex<bool>>(HOME_STORAGE_HERE)
            .and_then(|m| m.lock().ok().map(|g| *g))
            .unwrap_or(true);
        let not_here = not_here_reason(&place, at_home);
        let stock = if not_here.is_none() { data.get::<Mutex<HashMap<String, u32>>>("home_stock") } else { None };
        HomeStore { stock, not_here }
    }

    /// The home's storage counts for this craft.
    pub fn reachable(&self) -> bool {
        self.stock.is_some()
    }

    /// How many of `id` home storage holds (0 when it does not count here).
    pub fn count(&self, id: &str) -> u32 {
        self.stock
            .and_then(|m| m.lock().ok().map(|s| s.get(id).copied().unwrap_or(0)))
            .unwrap_or(0)
    }

    /// Take `qty` of `id` out of home storage. The main loop takes the same
    /// out of the placed containers right after the tick
    /// (`engine::stock_piles::take_consumed_home_stock`).
    pub fn take(&self, id: &str, qty: u32) {
        if qty == 0 {
            return;
        }
        if let Some(Ok(mut s)) = self.stock.map(|m| m.lock()) {
            if let Some(c) = s.get_mut(id) {
                *c = c.saturating_sub(qty);
            }
        }
    }

    /// Home storage can take what the backpack cannot of a craft's result:
    /// it counts here and the main loop is there to file it.
    pub fn takes_goods(&self, data: &DataStore) -> bool {
        self.reachable() && data.contains(HAND_MADE_TO_STORAGE)
    }
}

/// Why the home's storage (and its tanks) do not count for a craft made
/// where the player is, in words the player reads; None when they do.
/// `at_home` is false for a guest, whose home is not on this ship.
pub fn not_here_reason(place: &StationsWhere, at_home: bool) -> Option<&'static str> {
    match place {
        StationsWhere::Site(_) => Some("on a planet you craft from what you carry: your home's storage is in orbit"),
        StationsWhere::Nowhere => Some("away from your home you craft from what you carry"),
        StationsWhere::Home if !at_home => Some("your home is not on this ship, so its storage does not count"),
        StationsWhere::Home => None,
    }
}

/// File hand-made goods for home storage (`HAND_MADE_TO_STORAGE`).
pub fn send_to_storage(data: &DataStore, goods: &[(String, u32, u8)]) {
    if goods.is_empty() {
        return;
    }
    if let Some(Ok(mut out)) = data.get::<Mutex<Vec<(String, u32, u8)>>>(HAND_MADE_TO_STORAGE).map(|m| m.lock()) {
        out.extend(goods.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The split takes the backpack first, then storage, then the tanks, and
    /// never more than the need. Seen red against a split that took storage
    /// first: "left: Draw { pack: 0, home: 7, tap: 0, short: 0 }".
    #[test]
    fn the_backpack_is_used_first_then_storage_then_the_tanks() {
        assert_eq!(Draw::of(7, 3, 10, 0), Draw { pack: 3, home: 4, tap: 0, short: 0 });
        assert_eq!(Draw::of(7, 9, 10, 0), Draw { pack: 7, home: 0, tap: 0, short: 0 });
        assert_eq!(Draw::of(7, 1, 2, 3), Draw { pack: 1, home: 2, tap: 3, short: 1 });
        assert_eq!(Draw::of(0, 5, 5, 5), Draw::default());
    }
}
