//! Settings > Gameplay > Time: the one game clock's three settings
//! (2026-09-27, decision-briefs.md Brief 6, `systems::time`).
//!
//! The operator: "The default day length should be 24 hours. Though we want
//! it to be configurable. Like, maybe prefer a 20 hour day or 36 hour day.
//! However that shouldn't change how long an hour is unless they change the
//! setting that makes stuff happen faster/slower." So there are three
//! controls: the time speed (the only thing that speeds the world up), the
//! hours in a day, and the days in a year. The F11 panel's clock speed is the
//! same setting; its "Hold the clock still" is a dev hold over it.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState, HintDisplay};
use crate::systems::time;

/// How long a day of `hours` takes in real time at `speed`, in words.
pub fn day_in_real_time(hours: u32, speed: f32) -> String {
    let secs = f64::from(hours) * time::SECONDS_PER_HOUR / f64::from(speed.max(0.001));
    if secs >= 2.0 * 3600.0 {
        format!("{:.0} hours", secs / 3600.0)
    } else if secs >= 90.0 {
        format!("{:.0} minutes", secs / 60.0)
    } else {
        format!("{:.0} seconds", secs)
    }
}

/// Draw the Time section. Marks the settings dirty when anything changes.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay) {
    ui.label(RichText::new("Time").color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "How fast the world runs. At 1x (Realistic) an hour is a real hour: a day and a \
         night take a real day, and a lettuce takes its real 45 days. Everything follows \
         the one clock together: the sun, crops, water tanks, batteries, weather, and your \
         hunger and thirst. Simplified (72x) puts a day in 20 minutes, a lettuce in 15 \
         hours of play. Sleeping in a bed passes the night in a few seconds either way.",
    );
    ui.horizontal_wrapped(|ui| {
        for (speed, name) in time::TIME_SPEED_PRESETS {
            let selected = (state.settings.time_speed - speed).abs() < f32::EPSILON;
            let label = format!("{name} ({speed}x)");
            if ui.radio(selected, RichText::new(label).color(theme.text_primary())).clicked() && !selected {
                state.settings.time_speed = speed;
                state.settings_dirty = true;
            }
        }
    });
    let mut speed = state.settings.time_speed;
    if widgets::labeled_slider(ui, theme, "Custom speed", &mut speed, time::MIN_TIME_SPEED..=time::MAX_TIME_SPEED) {
        state.settings.time_speed = time::clamp_time_speed(speed);
        state.settings_dirty = true;
    }
    ui.label(
        RichText::new(format!(
            "A day takes {} of real time.",
            day_in_real_time(state.settings.hours_per_day, state.settings.time_speed)
        ))
        .size(theme.font_size_small)
        .color(theme.text_secondary()),
    );

    ui.add_space(theme.spacing_sm);
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "Hours in a day, 24 like Earth by default. An hour stays an hour: a 36-hour day \
         has a longer day and a longer night, noon at 18:00, and a crop still takes the \
         same number of hours to grow.",
    );
    let mut hours = state.settings.hours_per_day as f32;
    if widgets::labeled_slider(
        ui,
        theme,
        "Hours in a day",
        &mut hours,
        time::MIN_HOURS_PER_DAY as f32..=time::MAX_HOURS_PER_DAY as f32,
    ) {
        state.settings.hours_per_day = time::clamp_hours_per_day(hours.round() as u32);
        state.settings_dirty = true;
    }

    ui.add_space(theme.spacing_sm);
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "Days in a year, 365 like Earth by default. The four seasons each take a quarter \
         of it, so a shorter year brings the seasons round sooner.",
    );
    let mut days = state.settings.days_per_year as f32;
    if widgets::labeled_slider(
        ui,
        theme,
        "Days in a year",
        &mut days,
        time::MIN_DAYS_PER_YEAR as f32..=time::MAX_DAYS_PER_YEAR as f32,
    ) {
        state.settings.days_per_year = time::clamp_days_per_year(days.round() as u32);
        state.settings_dirty = true;
    }
    ui.add_space(theme.spacing_lg);
}

#[cfg(test)]
mod tests {
    /// The readout names a day in units a person can picture.
    #[test]
    fn a_day_reads_in_real_time() {
        assert_eq!(super::day_in_real_time(24, 1.0), "24 hours");
        assert_eq!(super::day_in_real_time(24, 72.0), "20 minutes");
        assert_eq!(super::day_in_real_time(36, 72.0), "30 minutes");
        assert_eq!(super::day_in_real_time(24, 1000.0), "86 seconds");
    }
}
