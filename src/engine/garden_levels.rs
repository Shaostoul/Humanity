//! Settings > Gameplay's garden and ship levels into their systems (2026-11-12).
//!
//! The operator, 2026-11-11: "I feel like we need to provide more than two options for our gameplay
//! settings. Including just turning settings off completely." Garden pests, pollination, soil pH,
//! picking and ship life support were hand-drawn switches on the Gameplay page (pollination and
//! soil pH only Off / On, picking and life support two choices); since this day each is a row of
//! data/gameplay/levels.json's "garden" group, saved in `SettingsState::gameplay` like the
//! survival levels, and this file turns the chosen levels into what each system reads:
//!
//! - garden pests, pollination and soil pH: the level's `value`, the share of a loss a crop really
//!   takes (0 Off, 0.5 Gentle, 1 Realistic), into "garden_pest_severity",
//!   `pollination::MODE_KEY` and `soil_ph::MODE_KEY`;
//! - picking: `picking::Mode` (Off, Forgiving, Realistic);
//! - ship life support: `life_support::Mode` (Station-supplied, Realistic), whose channel and the
//!   Home and Construction pages' meters keep its true/false (`Mode::on_grid`).
//!
//! What each level does is its system's, held by a test there per level (farming's
//! `gentle_keeps_half_of_what_an_unpollinated_crop_loses`, `every_soil_ph_level_takes_what_its_words_say`,
//! `picking_off_harvests_every_crop_once`, the pests' severity tests); the test here holds that every
//! level the file offers reaches its system as that system's own value.

use std::collections::BTreeMap;
use std::sync::Mutex;

use humanity_core::hot_reload::data_store::DataStore;
use humanity_systems::farming::{picking, pollination, soil_ph};
use humanity_systems::life_support;

use crate::gui::gameplay_levels::{level_of, value_of};
use crate::gui::GardenPests;

/// The DataStore key the pests, weeds and diseases read their level from (`farming::pests`).
const PEST_SEVERITY_KEY: &str = "garden_pest_severity";

/// The garden's and the ship's levels as their systems take them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GardenLevels {
    /// Garden pests, diseases and weeds: the share of their damage taken.
    pub pests: f32,
    /// Pollination: the share of an unpollinated crop's loss taken.
    pub pollination: f32,
    /// Soil pH: the share of the pH cap's loss taken.
    pub soil_ph: f32,
    pub picking: picking::Mode,
    pub life_support: life_support::Mode,
}

impl GardenLevels {
    /// The levels `saved` (config.json's `gameplay`) holds, each setting's default where it holds
    /// none the file lists.
    pub fn of(saved: &BTreeMap<String, String>) -> Self {
        Self {
            pests: value_of(saved, "garden_pests").clamp(0.0, 1.0),
            pollination: value_of(saved, "pollination").clamp(0.0, 1.0),
            soil_ph: value_of(saved, "soil_ph").clamp(0.0, 1.0),
            picking: picking::Mode::from_level(level_of(saved, "picking")).unwrap_or_default(),
            life_support: life_support::Mode::from_level(level_of(saved, "life_support")).unwrap_or_default(),
        }
    }

    /// Into the systems' channels (the main loop, each frame, and before a save's time-away
    /// catch-up), with the Garden panel's requests this frame: an amendment, a hand pollination
    /// or a colony, a crop to clear.
    pub fn publish(&self, data: &DataStore, garden: &mut GardenPests) {
        if let Some(Ok(mut s)) = data.get::<Mutex<f32>>(PEST_SEVERITY_KEY).map(|m| m.lock()) {
            *s = self.pests;
        }
        soil_ph::bridge(data, self.soil_ph, garden.ph_pending.take());
        pollination::publish(data, self.pollination, garden.pollinate_pending.take());
        picking::publish(data, self.picking, garden.clear_pending.take());
        life_support::publish(data, self.life_support.on_grid());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::gameplay_levels;

    /// A store with every channel these levels go to, as the main loop registers them.
    fn store() -> DataStore {
        let mut data = DataStore::new();
        data.insert(PEST_SEVERITY_KEY, Mutex::new(humanity_systems::farming::pests::DEFAULT_PEST_SEVERITY));
        soil_ph::register(&mut data);
        pollination::register(&mut data);
        picking::register(&mut data);
        life_support::register(&mut data);
        data
    }

    fn pests(data: &DataStore) -> f32 {
        *data.get::<Mutex<f32>>(PEST_SEVERITY_KEY).unwrap().lock().unwrap()
    }

    /// EVERY GARDEN AND SHIP LEVEL REACHES ITS SYSTEM (2026-11-12): for each level of garden pests,
    /// pollination, soil pH, picking and ship life support in data/gameplay/levels.json, choosing
    /// it and publishing puts that level's own value in the channel its system reads (and Off is
    /// off: pollination and soil pH report themselves off, picking reads Off); a new player's
    /// levels are Gentle, Gentle, Gentle, Forgiving and Station-supplied.
    ///
    /// Seen red 2026-11-12 before the file listed them (a setting the file lacks has no value):
    /// "a new player's garden is Gentle".
    #[test]
    fn every_garden_level_reaches_its_system() {
        let fresh = crate::gui::GuiState::default();
        let data = store();
        let first = GardenLevels::of(&fresh.settings.gameplay);
        first.publish(&data, &mut GardenPests::default());
        assert_eq!((pests(&data), pollination::severity(&data), soil_ph::severity(&data)), (0.5, 0.5, 0.5), "a new player's garden is Gentle");
        assert_eq!((picking::mode(&data), life_support::is_realistic(&data)), (picking::Mode::Forgiving, false), "Forgiving picking, Station-supplied air");
        for id in ["garden_pests", "pollination", "soil_ph", "picking", "life_support"] {
            let s = gameplay_levels::setting(id).unwrap_or_else(|| panic!("{id} is a row of data/gameplay/levels.json"));
            for l in &s.levels {
                let mut state = crate::gui::GuiState::default();
                state.settings.set_level(id, &l.id);
                let data = store();
                GardenLevels::of(&state.settings.gameplay).publish(&data, &mut state.garden_pests);
                let want = l.value.unwrap_or(-1.0);
                match id {
                    "garden_pests" => assert_eq!(pests(&data), want, "garden pests {}", l.id),
                    "pollination" => {
                        assert_eq!(pollination::severity(&data), want, "pollination {}", l.id);
                        assert_eq!(pollination::mode_on(&data), l.id != "off", "pollination {} on or off", l.id);
                    }
                    "soil_ph" => {
                        assert_eq!(soil_ph::severity(&data), want, "soil pH {}", l.id);
                        assert_eq!(soil_ph::is_on(&data), l.id != "off", "soil pH {} on or off", l.id);
                    }
                    "picking" => assert_eq!(Some(picking::mode(&data)), picking::Mode::from_level(&l.id), "picking {}", l.id),
                    _ => assert_eq!(life_support::is_realistic(&data), l.id == "realistic", "life support {}", l.id),
                }
            }
        }
    }

    /// The panel's requests ride along: an amendment, a hand pollination and a crop to clear are
    /// handed to their systems and taken from the panel.
    ///
    /// Seen red 2026-11-12 with `publish` handing picking no Clear request: "taken from the panel".
    #[test]
    fn the_panels_requests_reach_their_systems() {
        let data = store();
        let mut garden = GardenPests {
            ph_pending: Some(("bed_1".to_string(), "lime".to_string())),
            pollinate_pending: Some(pollination::Request::Hand("ntower_3".to_string())),
            clear_pending: Some(42),
            ..Default::default()
        };
        GardenLevels::of(&BTreeMap::new()).publish(&data, &mut garden);
        assert!(garden.ph_pending.is_none() && garden.pollinate_pending.is_none() && garden.clear_pending.is_none(), "taken from the panel");
        assert_eq!(data.get::<Mutex<Option<(String, String)>>>("soil_ph_request").unwrap().lock().unwrap().clone(), Some(("bed_1".to_string(), "lime".to_string())));
        assert_eq!(data.get::<Mutex<Option<pollination::Request>>>(pollination::REQUEST_KEY).unwrap().lock().unwrap().clone(), Some(pollination::Request::Hand("ntower_3".to_string())));
        assert_eq!(*data.get::<Mutex<Option<u64>>>(picking::CLEAR_KEY).unwrap().lock().unwrap(), Some(42));
    }
}
