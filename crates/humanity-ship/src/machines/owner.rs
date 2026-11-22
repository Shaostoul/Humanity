//! WHOSE A MACHINE IS (2026-11-21, the operator's "Yes" to "Should a household's figures count
//! only its own machines? I recommend yes, with the ship's machines shown as the ship's.").
//!
//! Every household file merges the ship's own rows (data/machines/ship.ron: the Commons' gardens,
//! fish tank and humidifiers, General Services' pods), so a loaded layout holds two owners'
//! machines. A row is the SHIP'S when its layout links the ship's file (`ship_machines`) and it
//! stands in a ship zone (any zone but "home"); every other row is the HOUSEHOLD'S. That is the
//! rule `split_for_save` sends a row back to ship.ron by, so whose a machine is and which file it
//! is saved to can never disagree: the data says it, through the file a row lives in.
//!
//! A household's figures (the Home page's food and its loops, the power, water and data meters,
//! the grow-light meter, the buildability report and its power circuit) count only the
//! household's own machines (`MachineHome::utility_meters` and the rest call
//! `instances_of(Owner::Household)`). Before this they added up every row, so the family home's
//! food loop counted the Commons garden as its own, and the one-person homestead could not merge
//! the ship's rows at all without the Commons closing its food loop and needing its battery.
//! The ship's machines are counted as the ship's, for everyone aboard (`ship_share`), and the
//! Home page says which figures are whose.

use super::{MachineHome, MachineInstance, MeterBasis};

/// Whose a placed machine is: the household's (its own file's rows) or the ship's (the rows
/// merged in from the ship's machine file, in the ship's shared zones).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    Household,
    Ship,
}

/// What the ship's own machines in a loaded layout add up to (`MachineHome::ship_share`): the
/// figures a household's own are kept apart from. Each machine at the same figure the
/// household's meters read for their own (its computed food, its day's draw measured or
/// estimated, its water), whoever powers it: the ship's reactor runs the ship's machines.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ShipShare {
    /// How many of the ship's machines the layout holds (instances and array cells).
    pub machines: usize,
    /// The food they grow, kcal a day, for everyone aboard.
    pub grown_kcal_per_day: f32,
    /// What they draw averaged over a day, kWh.
    pub power_kwh_day: f32,
    /// The water they draw, litres a day.
    pub water_l_day: f32,
}

impl MachineHome {
    /// Whose the placed row `inst` is (see the module).
    pub fn owner_of(&self, inst: &MachineInstance) -> Owner {
        if self.is_ship_row_zone(&inst.zone) {
            Owner::Ship
        } else {
            Owner::Household
        }
    }

    /// Whose the placed machine `id` is, None when no row has that id.
    pub fn owner_of_id(&self, id: &str) -> Option<Owner> {
        self.all_instances().into_iter().find(|i| i.id == id).map(|i| self.owner_of(&i))
    }

    /// Every placed row (instances and array cells) that is `owner`'s, in `all_instances` order.
    pub fn instances_of(&self, owner: Owner) -> Vec<MachineInstance> {
        self.all_instances().into_iter().filter(|i| self.owner_of(i) == owner).collect()
    }

    /// The food `owner`'s placed machines grow, kcal a day: each at its type's computed figure.
    pub fn grown_kcal_per_day_of(&self, owner: Owner) -> f32 {
        self.instances_of(owner).iter().filter_map(|i| self.grown.get(&i.machine)).map(|g| g.kcal_per_day).sum()
    }

    /// The ship's own machines' figures (`ShipShare`), under `basis` for each one's day.
    pub fn ship_share(&self, basis: MeterBasis) -> ShipShare {
        let rows = self.instances_of(Owner::Ship);
        let mut share = ShipShare { machines: rows.len(), grown_kcal_per_day: self.grown_kcal_per_day_of(Owner::Ship), ..Default::default() };
        for inst in &rows {
            if let Some(def) = self.catalog.get(&inst.machine) {
                share.power_kwh_day += self.day_draw(inst, def, basis.recorded).watts * 24.0 / 1000.0;
                share.water_l_day += self.water_demand_lpm_of(inst, &rows) * 1440.0;
            }
        }
        share
    }
}
