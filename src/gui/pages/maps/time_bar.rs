//! The Maps page's clock: the date shown, Now, the day and month skips,
//! play and pause, the speed presets, and the days-from-today scrubber. The
//! System view advances it each frame; the sky events read it.
//! Moved out of `gui/pages/cosmos.rs` in Maps increment 0 (2026-10-20).

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

// ─────────────────────── Time controls (v0.208.0) ───────────────────────────

/// J2000.0 epoch in seconds since the UNIX epoch (1970-01-01 00:00 UTC).
/// J2000 = 2000-01-01 12:00:00 UTC = UNIX time 946,728,000.
const J2000_UNIX_SECONDS: f64 = 946_728_000.0;

#[cfg(test)]
thread_local! {
    /// Test-only stand-in for the wall clock. The page opens on TODAY's sky,
    /// so a snapshot of it changed every time it ran and could never be a
    /// baseline. `set_clock_for_snapshot` pins it for the calling thread.
    static TEST_NOW_SINCE_J2000: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// Test-only: pin (or with `None`, release) the clock this page reads, in
/// seconds since J2000.0, for the calling thread.
#[cfg(test)]
pub(crate) fn set_clock_for_snapshot(seconds_since_j2000: Option<f64>) {
    TEST_NOW_SINCE_J2000.with(|c| c.set(seconds_since_j2000));
}

/// Seconds since J2000.0 corresponding to the current real-world clock.
/// Used to initialize sim_time so the user sees today's planetary
/// configuration without scrubbing.
pub(crate) fn current_real_time_seconds_since_j2000() -> f64 {
    #[cfg(test)]
    if let Some(t) = TEST_NOW_SINCE_J2000.with(|c| c.get()) {
        return t;
    }
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    unix - J2000_UNIX_SECONDS
}

/// Format a sim_time (seconds since J2000) as "YYYY-MM-DD HH:MM UTC".
/// Uses Howard Hinnant's days-from-civil algorithm — works for any
/// proleptic-Gregorian year, no external date library required.
pub(super) fn format_sim_time(sim_time_seconds: f64) -> String {
    let unix = sim_time_seconds + J2000_UNIX_SECONDS;
    let unix_i = unix.floor() as i64;
    let days = unix_i.div_euclid(86_400);
    let secs_in_day = unix_i.rem_euclid(86_400);
    let hour = (secs_in_day / 3600) as u32;
    let minute = ((secs_in_day % 3600) / 60) as u32;
    let (year, month, day) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}  {:02}:{:02} UTC", year, month, day, hour, minute)
}

/// Convert "days since UNIX epoch" to (year, month, day) using
/// Howard Hinnant's algorithm — accurate for all proleptic-Gregorian
/// dates. https://howardhinnant.github.io/date_algorithms.html
fn days_to_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468; // shift so era 0 starts at year 0
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}

/// Format a sim_speed value (multiplier) into a human-readable label
/// like "1 day/sec", "1 year/sec", "paused", "REWIND 1 month/sec".
fn format_sim_speed(speed: f64) -> String {
    if speed == 0.0 { return "paused".to_string(); }
    let abs = speed.abs();
    let prefix = if speed < 0.0 { "REW " } else { "" };
    let unit = if abs >= 365.25 * 86_400.0 {
        let years = abs / (365.25 * 86_400.0);
        format!("{:.1} years/s", years)
    } else if abs >= 30.0 * 86_400.0 {
        let mo = abs / (30.0 * 86_400.0);
        format!("{:.1} mo/s", mo)
    } else if abs >= 86_400.0 {
        let d = abs / 86_400.0;
        format!("{:.1} d/s", d)
    } else if abs >= 3_600.0 {
        let h = abs / 3_600.0;
        format!("{:.1} h/s", h)
    } else if abs >= 60.0 {
        format!("{:.1} min/s", abs / 60.0)
    } else if abs >= 1.0 {
        format!("{:.0}× real-time", abs)
    } else {
        format!("{:.2}× real-time", abs)
    };
    format!("{}{}", prefix, unit)
}

/// Render the time controls bar at the top of the Solar System view.
/// Operator can pause / play / fast-forward / rewind / jump to "now".
pub(super) fn draw_time_controls(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgb(15, 15, 22))  // theme-exempt: Maps time bar bg
        .inner_margin(egui::Margin::symmetric(8, 4));
    frame.show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            // ── Date display ──
            ui.label(
                RichText::new(format_sim_time(state.maps.sim_time_seconds))
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .monospace()
                    .strong(),
            );
            ui.separator();

            // ── Live (Maps increment 1; was "Now"): back on the game's own
            //    clock, the ship and the planets where the game has them. Any
            //    scrub, skip or speed change below takes the clock off it. ──
            if widgets::Button::secondary("Live")
                .active(state.maps.live)
                .tooltip("Show the ship and the planets exactly where the game has them, right now. \
                          Scrubbing or changing the speed leaves Live; press this to come back.")
                .show(ui, theme)
            {
                state.maps.live = true;
                state.maps.sim_speed = 0.0;
                state.maps.sim_time_seconds = state.maps.me.as_ref().map_or_else(current_real_time_seconds_since_j2000, |m| m.sky_seconds);
            }
            ui.separator();

            // ── Transport controls ──
            // « » jump by 1 month at a time; ⏪ ⏩ jump by 1 day. Operator
            // can combine with play/pause to scrub the orbits visually.
            if widgets::Button::secondary("«")
                .tooltip("Skip back 1 month")
                .show(ui, theme)
            {
                state.maps.sim_time_seconds -= 30.0 * 86_400.0;
            }
            if widgets::Button::secondary("⏪")
                .tooltip("Skip back 1 day")
                .show(ui, theme)
            {
                state.maps.sim_time_seconds -= 86_400.0;
            }
            // Play / Pause: toggle between sim_speed = 0 and the last used speed.
            let playing = state.maps.sim_speed != 0.0;
            let label = if playing { "Pause" } else { "Play" };
            if widgets::Button::primary(label)
                .tooltip(if playing { "Pause the simulation." }
                         else { "Resume / start the simulation at the current speed (default 1 day/sec)." })
                .show(ui, theme)
            {
                if playing {
                    state.maps.sim_speed = 0.0;
                } else {
                    // Default to 1 day per real second when starting from paused.
                    state.maps.sim_speed = 86_400.0;
                }
            }
            if widgets::Button::secondary("⏩")
                .tooltip("Skip forward 1 day")
                .show(ui, theme)
            {
                state.maps.sim_time_seconds += 86_400.0;
            }
            if widgets::Button::secondary("»")
                .tooltip("Skip forward 1 month")
                .show(ui, theme)
            {
                state.maps.sim_time_seconds += 30.0 * 86_400.0;
            }
            ui.separator();

            // ── Speed preset buttons ──
            ui.label(RichText::new("Speed:").color(theme.text_secondary()).size(theme.font_size_small));
            for &(label, speed) in &[
                ("1 h/s",   3_600.0_f64),
                ("1 d/s",   86_400.0),
                ("1 mo/s",  30.0 * 86_400.0),
                ("1 y/s",   365.25 * 86_400.0),
                ("10 y/s",  10.0 * 365.25 * 86_400.0),
            ] {
                let is_active = (state.maps.sim_speed - speed).abs() < 1e-3;
                let is_active_neg = (state.maps.sim_speed + speed).abs() < 1e-3;
                if widgets::Button::secondary(label)
                    .active(is_active)
                    .tooltip("Forward at this speed")
                    .show(ui, theme)
                {
                    state.maps.sim_speed = speed;
                }
                // Rewind variant — right-click NOT available so use a small "-" prefix button.
                let _ = is_active_neg;
            }
            // Rewind toggle: flip the sign of sim_speed.
            if widgets::Button::secondary("Reverse")
                .tooltip("Flip the simulation direction (forward/backward).")
                .show(ui, theme)
            {
                state.maps.sim_speed = -state.maps.sim_speed;
            }

            // ── Current speed label ──
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(if state.maps.live { "· live, the game's own clock".to_string() } else { format!("· {}", format_sim_speed(state.maps.sim_speed)) })
                        .size(theme.font_size_small)
                        .color(theme.text_muted())
                        .italics(),
                );
            });
        });

        // ── Scrubber — slide to jump days/months/years quickly ──
        // Range is ±10 years from today's date, scaled in days.
        let now = current_real_time_seconds_since_j2000();
        let mut offset_days = (state.maps.sim_time_seconds - now) / 86_400.0;
        // The wall clock runs on between the frame that set sim time to "now"
        // and this one, so a paused view at today read "-0.00". Anything
        // inside the box's display precision is today.
        if offset_days.abs() < 0.005 {
            offset_days = 0.0;
        }
        let scrubbed = widgets::slider(
            ui,
            theme,
            egui::Slider::new(&mut offset_days, -(365.25 * 10.0)..=(365.25 * 10.0))
                .text("Days from today")
                .show_value(true),
        );
        if scrubbed.changed() {
            state.maps.sim_time_seconds = now + offset_days * 86_400.0;
        }
    });
}
