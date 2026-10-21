//! The body list's Stations rows (Maps increment 1): every station in
//! `data/stations/*.ron`, by its own name, with its altitude computed from
//! its orbit the way the engine places it. Until 2026-10-21 this was one
//! hard-coded row, "Home Station · 400 km LEO", which had been wrong since
//! v0.1225 made the home a synchronous orbit.
//!
//! The altitude: `station::orbit::semi_major_axis_m` with the parent's
//! gravitational parameter and the rotation period the frame loop uses for
//! it (Earth: `MU_EARTH` and the 24-hour SOLAR day, `REAL_SECONDS_PER_DAY`),
//! less the parent's radius from `sol.json` (Earth: the game's 6,371 km
//! sphere). For the shipped home that is 42,241 km from Earth's centre,
//! 35,870 km up. Real geostationary satellites sit at 35,786 km above the
//! equator because they keep pace with the 23 h 56 min SIDEREAL day; this
//! ship keeps pace with the 24-hour solar day on purpose (lib.rs, the
//! station block: "Synchronous here therefore means synchronous with the
//! day/night cycle the player actually sees"), which puts it 77 km higher.
//! The row says what the game does, and its hover says why it differs.

use egui::RichText;

use crate::cosmos::find_body;
use crate::gui::theme::Theme;
use crate::gui::GuiState;
use crate::station::orbit::{self, PeriodSpec, StationDef};

use super::format_with_commas;

/// Newton's gravitational constant, m^3 kg^-1 s^-2 (CODATA 2018), for a
/// station around a body other than Earth (whose own mu the engine uses).
const G: f64 = 6.674_30e-11;

/// One station as the Stations rows show it.
#[derive(Debug, Clone, PartialEq)]
pub struct StationRow {
    pub id: String,
    pub name: String,
    /// The body it orbits, by its `sol.json` id.
    pub parent: String,
    /// Height of its orbit over the parent's surface (the semi-major axis
    /// less the parent's radius), km.
    pub altitude_km: f64,
    /// It keeps pace with its parent's turn, so it hangs over one place.
    pub synchronous: bool,
}

/// The parent's gravitational parameter (m^3/s^2) and the rotation period
/// (seconds) a synchronous orbit around it keeps pace with: the same two
/// numbers the frame loop gives `orbit::propagate` for Earth.
pub fn parent_constants(parent: &str) -> (f64, f64) {
    if parent == "earth" {
        return (orbit::MU_EARTH, orbit::REAL_SECONDS_PER_DAY);
    }
    let mu = find_body(parent).map(|b| G * b.mass_kg).filter(|m| *m > 0.0).unwrap_or(orbit::MU_EARTH);
    (mu, orbit::REAL_SECONDS_PER_DAY)
}

/// A station's row: its name and its computed altitude.
pub fn station_row(def: &StationDef) -> StationRow {
    let (mu, turn_s) = parent_constants(&def.parent);
    let a_m = orbit::semi_major_axis_m(&def.orbit, mu, turn_s);
    let radius_m = find_body(&def.parent).map(|b| b.radius_km * 1000.0).unwrap_or(0.0);
    StationRow {
        id: def.id.clone(),
        name: def.name.clone(),
        parent: def.parent.clone(),
        altitude_km: (a_m - radius_m) / 1000.0,
        synchronous: def.orbit.period == PeriodSpec::Synchronous,
    }
}

/// Every station in `data_dir/stations`, in id order.
pub fn station_rows(data_dir: &std::path::Path) -> Vec<StationRow> {
    crate::station::load_all(data_dir).iter().map(station_row).collect()
}

/// The row's second line: "35,870 km up · synchronous over one place".
pub fn row_detail(row: &StationRow) -> String {
    let alt = format_with_commas(row.altitude_km.round() as i64);
    if row.synchronous {
        format!("{alt} km up · synchronous, hangs over one place")
    } else {
        format!("{alt} km up")
    }
}

/// The Stations rows at the top of the body list.
pub(super) fn draw_station_rows(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Stations").size(theme.font_size_small).color(theme.accent()).strong());
    let rows = state.maps.stations.get_or_insert_with(|| station_rows(&crate::data_dir())).clone();
    if rows.is_empty() {
        ui.label(RichText::new("No stations in data/stations.").size(theme.font_size_small).color(theme.text_muted()));
    }
    for row in &rows {
        let yours = state.maps.me.as_ref().is_some_and(|m| m.ship_id == row.id);
        ui.horizontal(|ui| {
            let name = if yours { format!("{} · your home", row.name) } else { row.name.clone() };
            ui.label(RichText::new(name).size(theme.font_size_body).color(theme.text_primary()));
            if row.id == "home" && crate::gui::widgets::toggle(ui, theme, "Track", &mut state.settings.track_station) {
                state.settings_dirty = true;
            }
        });
        let detail = ui.label(RichText::new(row_detail(row)).size(theme.font_size_small).color(theme.text_secondary()));
        if row.synchronous && row.parent == "earth" {
            detail.on_hover_text(
                "One orbit per 24-hour game day, so it stays over the same place on Earth. Real \
                 geostationary satellites sit at 35,786 km because they keep pace with the 23 h 56 min \
                 turn of the Earth against the stars; this ship keeps pace with the day you see, \
                 which puts it a little higher.",
            );
        }
    }
    ui.label(
        RichText::new("Tracking shows a ring and label on your ship in the world, so you can always find home.")
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
}
