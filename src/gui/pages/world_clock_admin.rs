//! Server Settings > ADMIN > Shared world clock (2026-10-04).
//!
//! The operator: "Sure, let's do 72x but, make sure there's admin tools for me
//! to adjust it from inside the app." That evening the default became real
//! time: "For normal mode, especially for my MMO server, let's have everything
//! be real time, not the 72x." The shared world's clock runs on the relay
//! (`relay::handlers::game_state::GameWorld::time_scale`, saved as the server
//! setting `world_time_scale`, 1 on a new server) and every game in the world
//! follows it (`game_time_sync`, `engine::net_route`). This is the
//! in-app control: pick a speed, see in plain words what it means, Apply. The
//! relay changes the running world at once and tells every connected game; no
//! restart. The web mirror is the Game Admin window (`web/chat/chat-game-admin.js`).
//!
//! What a speed means is worked out from the data, not written in: the length
//! of the shared world's day (`systems::time::HOST_HOURS_PER_DAY`) and the time
//! a lettuce takes to grow (`data/plants.csv`, read when the section is drawn).

use std::io::BufRead;

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::systems::time;

/// The crop the explanation names: a fast, familiar one.
const EXAMPLE_CROP: &str = "lettuce";

/// Draw the Shared world clock subsection (called from `game_admin::draw_section`,
/// inside the admin-gated ADMIN section of Server Settings).
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::section_header(ui, theme, "Shared world clock");
    widgets::body_hint(
        ui,
        theme,
        "How fast time passes in this server's shared world, for everyone in it: the sun, \
         crops, water tanks, batteries, the weather, and each player's hunger and thirst all \
         follow this one clock. A player's own Time setting applies only when they play alone. \
         A change reaches every connected game at once; the server does not restart, and the \
         world keeps its date.",
    );
    ui.add_space(theme.spacing_sm);

    let lettuce = example_crop_growth_days();
    let current = state.server_settings.as_ref().map(|s| s.world_time_scale as f32);
    // An applied choice is done once the server says it back.
    if let (Some(d), Some(c)) = (state.game_admin_clock_draft, current) {
        if (d - c).abs() < 1e-3 {
            state.game_admin_clock_draft = None;
        }
    }
    match current {
        Some(c) => ui.label(
            RichText::new(format!("Now: {}", explainer(c, lettuce)))
                .size(theme.font_size_small)
                .color(theme.text_primary()),
        ),
        None => ui.label(
            RichText::new("Connect to a server to see how fast its world runs.")
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        ),
    };
    ui.add_space(theme.spacing_xs);

    let mut chosen = shown_speed(state.game_admin_clock_draft, current);
    let mut picked = false;
    ui.horizontal_wrapped(|ui| {
        for (speed, name) in time::TIME_SPEED_PRESETS {
            let selected = (chosen - speed).abs() < 1e-3;
            let label = format!("{name} ({}x)", speed_text(speed));
            if ui.radio(selected, RichText::new(label).color(theme.text_primary())).clicked() && !selected {
                chosen = speed;
                picked = true;
            }
        }
    });
    let mut slider = chosen;
    if widgets::labeled_slider(ui, theme, "Custom speed", &mut slider, time::MIN_TIME_SPEED..=time::MAX_TIME_SPEED) {
        chosen = time::clamp_time_speed(slider.round());
        picked = true;
    }
    if picked {
        state.game_admin_clock_draft = Some(chosen);
    }
    let differs = current.map_or(true, |c| (c - chosen).abs() >= 1e-3);
    if differs {
        ui.label(
            RichText::new(format!("If applied: {}", explainer(chosen, lettuce)))
                .size(theme.font_size_small)
                .color(theme.accent()),
        );
    }
    ui.add_space(theme.spacing_sm);
    ui.add_enabled_ui(differs, |ui| {
        if widgets::Button::primary("Apply to the shared world")
            .tooltip("Run the shared world's clock at this speed, for every player, from now on.")
            .show(ui, theme)
        {
            // Only a message that went out is reported as asked (the plot form's rule).
            state.game_admin_status = if send_world_clock(state, chosen) {
                format!("Asked the server to run the shared world at {}x.", speed_text(chosen))
            } else {
                "Not connected to the server.".into()
            };
        }
    });
}

/// The speed the section shows as picked: the admin's unapplied pick, else the
/// server's speed, else, with no server to ask, the speed a new server runs at
/// (`relay::storage::default_world_time_scale`: real time, 1x).
fn shown_speed(draft: Option<f32>, current: Option<f32>) -> f32 {
    draft.or(current).unwrap_or(crate::relay::storage::default_world_time_scale() as f32)
}

/// What `speed` means, in words a player can picture: how long a day of the
/// shared world takes in real time, and how long `lettuce_days` (the example
/// crop's growing time, in days) take. 1x says so plainly rather than "a
/// lettuce in 45 days instead of 45 days".
pub fn explainer(speed: f32, lettuce_days: Option<f64>) -> String {
    let speed = time::clamp_time_speed(speed);
    let crop = |d: f64| format!("{} days", number_text(d));
    if (speed - time::REALISTIC_TIME_SPEED).abs() < 1e-3 {
        return match lettuce_days {
            Some(d) => format!("at 1x the world keeps real time: a day takes a real day, and a lettuce its real {}.", crop(d)),
            None => "at 1x the world keeps real time: a day takes a real day.".to_string(),
        };
    }
    let day = super::settings_time::day_in_real_time(time::HOST_HOURS_PER_DAY, speed);
    let mut s = format!("at {}x a day passes in {day}", speed_text(speed));
    if let Some(d) = lettuce_days {
        let real_s = d * time::EARTH_DAY_S / f64::from(speed);
        s.push_str(&format!(", and a lettuce grows in about {} instead of {}", real_duration(real_s), crop(d)));
    }
    s.push('.');
    s
}

/// A real stretch of time in the largest unit that keeps it a whole number
/// worth saying: days from two days up, hours from two hours, minutes from 90 s.
pub fn real_duration(secs: f64) -> String {
    let (n, unit) = if secs >= 2.0 * time::EARTH_DAY_S {
        (secs / time::EARTH_DAY_S, "days")
    } else if secs >= 2.0 * time::SECONDS_PER_HOUR {
        (secs / time::SECONDS_PER_HOUR, "hours")
    } else if secs >= 90.0 {
        (secs / 60.0, "minutes")
    } else {
        (secs, "seconds")
    };
    format!("{} {unit}", n.round())
}

/// A speed as people write it: 72, not 72.0; one decimal only when it has one.
fn speed_text(speed: f32) -> String {
    number_text(f64::from(speed))
}

fn number_text(v: f64) -> String {
    // Rounded first, a half up, as the web's `toFixed(1)` does; `{:.1}` alone
    // would take 2.25 to 2.2 where the web says 2.3.
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round())
    } else {
        format!("{:.1}", (v * 10.0).round() / 10.0)
    }
}

/// The example crop's growing time from `data/plants.csv` on disk, read each
/// time the section is drawn (no copy kept, so an edit to the file shows at
/// once). None when the file or the row is missing: the explanation then
/// names the day alone.
fn example_crop_growth_days() -> Option<f64> {
    let file = std::fs::File::open(crate::data_dir().join("plants.csv")).ok()?;
    crop_growth_days(std::io::BufReader::new(file), EXAMPLE_CROP)
}

/// `growth_days` of the crop `id` in a plants.csv: the header (its first line
/// that is not a `#` comment) says which column it is; reading stops at the
/// crop's row. Quoted fields may hold commas.
pub fn crop_growth_days(csv: impl BufRead, id: &str) -> Option<f64> {
    let mut column: Option<usize> = None;
    for line in csv.lines() {
        let line = line.ok()?;
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = csv_fields(line);
        match column {
            None => column = Some(fields.iter().position(|f| f.trim() == "growth_days")?),
            Some(c) if fields.first().map(|f| f.trim()) == Some(id) => {
                return fields.get(c)?.trim().parse::<f64>().ok().filter(|d| d.is_finite() && *d > 0.0);
            }
            Some(_) => {}
        }
    }
    None
}

/// One CSV line's fields, with double-quoted fields allowed to hold commas.
fn csv_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    out.push(cur);
    out
}

/// Send `server_settings_update` with only the clock's speed (the other
/// settings are left as they are; the relay checks the sender is an admin and
/// answers with the new `server_settings_state` and a `game_time_sync`). False
/// when there is no open connection to send it on.
fn send_world_clock(state: &GuiState, speed: f32) -> bool {
    match state.ws_client.as_ref() {
        Some(client) if client.is_connected() => {
            let msg = serde_json::json!({ "type": "server_settings_update", "world_time_scale": speed });
            client.send(&msg.to_string());
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::screen_surface::find_text_in_shapes;

    /// WHAT 72x MEANS, IN WORDS (operator, 2026-10-04: say plainly what it
    /// means, worked out from the data). A day of the shared world (24 hours)
    /// at 72x passes in 20 real minutes, and a 45-day lettuce grows in about
    /// 15 real hours; at 24x a day is an hour and the lettuce 45 hours; at 1x
    /// it says real time plainly.
    ///
    /// Seen red 2026-10-04 with the crop's real time computed as `d / speed`
    /// (days, not seconds): "at 72x: at 72x a day passes in 20 minutes, and a
    /// lettuce grows in about 1 seconds instead of 45 days."
    #[test]
    fn the_explanation_says_what_a_speed_means() {
        let s = explainer(72.0, Some(45.0));
        assert_eq!(s, "at 72x a day passes in 20 minutes, and a lettuce grows in about 15 hours instead of 45 days.", "at 72x: {s}");
        assert_eq!(explainer(24.0, Some(45.0)), "at 24x a day passes in 60 minutes, and a lettuce grows in about 45 hours instead of 45 days.");
        assert_eq!(explainer(1.0, Some(45.0)), "at 1x the world keeps real time: a day takes a real day, and a lettuce its real 45 days.");
        assert_eq!(explainer(720.0, None), "at 720x a day passes in 2 minutes.");
    }

    /// The lettuce in the explanation is the one in data/plants.csv, not a
    /// number written into the code: the real file's row is found, and the
    /// words at 72x follow from it.
    ///
    /// Seen red 2026-10-04 with the column one to the right of growth_days
    /// read (water_liters_per_day): "at 72x a day passes in 20 minutes, and a
    /// lettuce grows in about 0 seconds instead of 0.2 days."
    #[test]
    fn the_example_crop_comes_from_plants_csv() {
        let csv = std::fs::read_to_string("data/plants.csv").expect("data/plants.csv");
        let days = crop_growth_days(csv.as_bytes(), EXAMPLE_CROP).expect("lettuce has a growth_days in data/plants.csv");
        let hours = (days * 24.0 / 72.0).round();
        let s = explainer(72.0, Some(days));
        assert!(s.contains(&format!("about {hours} hours instead of {} days", number_text(days))), "{s}");
        // The header names the column; a quoted comma does not shift it.
        let quoted = "# comment\nid,name,description,growth_days\nbean,Bean,\"climbs, twines\",60\n";
        assert_eq!(crop_growth_days(quoted.as_bytes(), "bean"), Some(60.0));
        assert_eq!(crop_growth_days(quoted.as_bytes(), "pea"), None);
    }

    /// The web Game Admin window says the same thing, so its lettuce must be
    /// the data's lettuce (it cannot read the CSV: the website serves only the
    /// JSON under data/), and it offers the same speeds as this section.
    /// Seen red 2026-10-04 with the web constant set to 40: "web/chat/chat-game-admin.js
    /// says a lettuce takes 40 days, data/plants.csv says 45".
    #[test]
    fn the_web_window_names_the_same_lettuce_and_speeds() {
        let csv = std::fs::read_to_string("data/plants.csv").expect("data/plants.csv");
        let days = crop_growth_days(csv.as_bytes(), EXAMPLE_CROP).expect("lettuce row");
        let js = std::fs::read_to_string("web/chat/chat-game-admin.js").expect("web/chat/chat-game-admin.js");
        let line = js.lines().find(|l| l.contains("var LETTUCE_GROWTH_DAYS =")).expect("the web constant");
        let value = line.split('=').nth(1).and_then(|r| r.split(';').next()).expect("a value");
        let web: f64 = value.trim().parse().expect("a number");
        assert_eq!(web, days, "web/chat/chat-game-admin.js says a lettuce takes {web} days, data/plants.csv says {days}");
        for (speed, name) in time::TIME_SPEED_PRESETS {
            let entry = format!("[{}, '{name}']", speed_text(speed));
            assert!(js.contains(&entry), "the web window offers {name} ({}x) too: {entry} in CLOCK_PRESETS", speed_text(speed));
        }
    }

    /// WITH NO SERVER TO ASK, THE SECTION SHOWS A NEW SERVER'S SPEED: real
    /// time, 1x (the operator, 2026-10-04 evening: "For normal mode,
    /// especially for my MMO server, let's have everything be real time, not
    /// the 72x."), the relay's own default. The web window shows the same (its
    /// CLOCK_DEFAULT, which is also what it reads a speed that is not a number
    /// as, the way `time::clamp_time_speed` does here). An admin's unapplied
    /// pick, and else the server's speed, come first.
    #[test]
    fn with_no_server_the_section_shows_a_new_servers_speed() {
        assert_eq!(shown_speed(None, None), 1.0, "not connected, the section shows 1x");
        assert_eq!(f64::from(shown_speed(None, None)), crate::relay::storage::default_world_time_scale(), "the speed a new server runs at");
        assert_eq!(shown_speed(None, Some(24.0)), 24.0, "the server's speed");
        assert_eq!(shown_speed(Some(720.0), Some(24.0)), 720.0, "the admin's unapplied pick");
        let js = std::fs::read_to_string("web/chat/chat-game-admin.js").expect("web/chat/chat-game-admin.js");
        let line = js.lines().find(|l| l.contains("var CLOCK_DEFAULT =")).expect("the web constant");
        let value = line.split('=').nth(1).and_then(|r| r.split(';').next()).expect("a value");
        let web: f32 = value.trim().parse().expect("a number");
        assert_eq!(web, shown_speed(None, None), "web/chat/chat-game-admin.js shows {web}x with no server, this section {}x", shown_speed(None, None));
        assert_eq!(web, time::clamp_time_speed(f32::NAN), "web/chat/chat-game-admin.js reads a speed that is not a number as {web}x, this section as {}x", time::clamp_time_speed(f32::NAN));
    }

    /// NATIVE AND WEB SAY THE SAME NUMBERS (review of 2026-10-04, finding 3).
    /// The web window rounds with `Math.round` and `toFixed(1)`, a half up;
    /// Rust's `{:.0}` and `{:.1}` round a half to the even number, so at 576x
    /// (a day in 2.5 minutes) this section said 2 minutes and the web window
    /// 3. Both round a half up now. The ties: 576x is 2.5 minutes, 64x 22.5
    /// minutes, 320x 4.5 minutes, and a 30-hour day at 12x 2.5 hours.
    ///
    /// Seen red 2026-10-04 before the fix: "576x is 2.5 minutes, left:
    /// \"2 minutes\", right: \"3 minutes\"".
    #[test]
    fn a_half_rounds_up_as_the_web_window_rounds_it() {
        use super::super::settings_time::day_in_real_time;
        assert_eq!(day_in_real_time(24, 576.0), "3 minutes", "576x is 2.5 minutes");
        assert_eq!(day_in_real_time(24, 64.0), "23 minutes", "64x is 22.5 minutes");
        assert_eq!(day_in_real_time(24, 320.0), "5 minutes", "320x is 4.5 minutes");
        assert_eq!(day_in_real_time(30, 12.0), "3 hours", "a 30-hour day at 12x is 2.5 hours");
        assert_eq!(number_text(2.25), "2.3", "toFixed(1) rounds 2.25 up");
        assert_eq!(number_text(72.0), "72");
        assert_eq!(number_text(2.5), "2.5");
        assert!(explainer(576.0, Some(45.0)).starts_with("at 576x a day passes in 3 minutes,"));
    }

    /// One headless frame of the clock section in a plain panel, with `events`.
    fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 700.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw(ui, theme, state));
        })
    }

    fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
        let out = frame(ctx, theme, state, Vec::new());
        let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text} is drawn")).rect.center();
        let m = egui::Modifiers::default();
        frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    }

    /// Picking a speed and pressing Apply, the way an admin does: the server
    /// runs at a new server's 1x, the admin picks "A day an hour (24x)", the section says
    /// what that would mean, and Apply with the link down says the server was
    /// NOT asked and keeps the choice.
    ///
    /// Seen red 2026-10-04 with the status set whether or not anything was
    /// sent: "with no connection the section says: Asked the server to run the
    /// shared world at 24x.".
    #[test]
    fn picking_a_speed_and_applying_with_no_connection_says_so() {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut state = GuiState::default();
        state.server_settings = Some(crate::relay::storage::ServerSettings::default());
        assert!(state.ws_client.is_none());
        click(&ctx, &theme, &mut state, "A day an hour (24x)");
        assert_eq!(state.game_admin_clock_draft, Some(24.0), "the admin's pick");
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        assert!(find_text_in_shapes(&out.shapes, "If applied: at 24x a day passes in 60 minutes").is_some(), "the section says what 24x means");
        click(&ctx, &theme, &mut state, "Apply to the shared world");
        assert_eq!(state.game_admin_status, "Not connected to the server.", "with no connection the section says: {}", state.game_admin_status);
        assert_eq!(state.game_admin_clock_draft, Some(24.0), "and keeps the choice");
    }
}
