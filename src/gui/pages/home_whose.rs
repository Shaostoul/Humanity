//! WHOSE MACHINES THE FIGURES COUNT (2026-11-21, the operator's "Yes" to "Should a household's
//! figures count only its own machines? I recommend yes, with the ship's machines shown as the
//! ship's."): the Home page's loops and the Construction page's meters count only the household's
//! own machines (`humanity_ship::machines::owner`), so each says so in words, and the ship's own
//! machines merged into the household's file (the Commons' gardens, fish tank and humidifiers,
//! General Services' respawner) are shown as the ship's, with what they grow and draw for everyone
//! aboard. Before this the family home's food figure counted the Commons garden as its own; now
//! it is lower, and these lines say why.

use egui::{RichText, Ui};
use humanity_core::i18n::{t, tf};
use humanity_ship::machines::{MachineHome, MeterBasis, Owner, ShipShare};

use crate::gui::theme::Theme;
use crate::gui::widgets::cards;

/// What the household's figures count and what the ship's machines are, as the page says it.
#[derive(Debug, Clone, PartialEq)]
pub struct Whose {
    /// "Every figure here counts only your household's own 236 machines."
    pub household: String,
    /// The ship's machines' line, None when the layout holds none of the ship's.
    pub ship: Option<String>,
}

/// The words for `home` under `basis` (each ship machine's day measured or estimated, as the
/// meters read the household's own).
pub fn whose(home: &MachineHome, basis: MeterBasis) -> Whose {
    let household = tf("home.whose_household", &[("n", &home.instances_of(Owner::Household).len())]);
    let share: ShipShare = home.ship_share(basis);
    let ship = (share.machines > 0).then(|| {
        tf(
            "home.whose_ship",
            &[
                ("n", &share.machines.to_string()),
                ("kcal", &format!("{:.0}", share.grown_kcal_per_day)),
                ("kwh", &format!("{:.1}", share.power_kwh_day)),
            ],
        )
    });
    Whose { household, ship }
}

/// The Home page's lines under the loops' heading: the household's own figures, then the ship's
/// machines apart, muted, with the hint on hover saying why the ship's are not counted.
pub fn draw(ui: &mut Ui, theme: &Theme, w: &Whose) {
    cards::group_heading(ui, theme, t("home.whose_title"), None);
    ui.label(RichText::new(&w.household).size(theme.font_size_small).strong().color(theme.text_primary()))
        .on_hover_text(t("home.whose_household_hint"));
    if let Some(ship) = &w.ship {
        ui.label(RichText::new(ship).size(theme.font_size_small).color(theme.text_muted())).on_hover_text(t("home.whose_ship_hint"));
    }
    ui.add_space(theme.spacing_sm);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(file: &str) -> MachineHome {
        let p = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data").join("machines").join(file);
        humanity_systems::grow_machines::load_home(&p).unwrap_or_else(|| panic!("{file} loads"))
    }

    /// THE SHIP'S MACHINES APPEAR AS THE SHIP'S (2026-11-21): every household that merges the
    /// ship's rows (the family home, the one-person home, a cabin) shows a line for the ship's own
    /// machines, its count the ship's rows and its food the Commons garden's, and the household's
    /// line counts only its own rows; the two counts add up to every placed machine, so nothing is
    /// counted twice or lost.
    ///
    /// Seen red 2026-11-21 with home_solo.ron's `ship_machines` line taken out (the one-person home
    /// as it was before it merged the ship's rows): "home_solo.ron: the ship's rows are merged in".
    #[test]
    fn the_ships_machines_appear_as_the_ships() {
        for file in ["home.ron", "home_solo.ron", "cabin.ron"] {
            let h = home(file);
            let w = whose(&h, MeterBasis::default());
            let (own, ship) = (h.instances_of(Owner::Household).len(), h.instances_of(Owner::Ship).len());
            assert_eq!(own + ship, h.all_instances().len(), "{file}: every machine is the household's or the ship's");
            assert!(ship > 0, "{file}: the ship's rows are merged in");
            assert!(w.household.contains(&own.to_string()), "{file}: the household's line counts its own {own}: {}", w.household);
            let line = w.ship.unwrap_or_else(|| panic!("{file}: the ship's machines are shown as the ship's"));
            assert!(line.contains(&ship.to_string()), "{file}: the ship's line counts its {ship}: {line}");
            let kcal = h.ship_share(MeterBasis::default()).grown_kcal_per_day;
            assert!(kcal > 0.0 && line.contains(&format!("{kcal:.0}")), "{file}: the Commons garden grows for the ship ({kcal:.0} kcal/day): {line}");
        }
    }
}
