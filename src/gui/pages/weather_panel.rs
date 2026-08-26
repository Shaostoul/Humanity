//! Live weather panel (v0.1050, F11).
//!
//! Operator: "is there a way for me to cycle the weather live in game? Like
//! press F11 to bring up the weather menu to press various buttons, twist
//! knobs, adjust sliders, whatever? I'd like to see the weather effects you've
//! made... I'd also like to see how the waves change based on weather. We got a
//! great lightly disturbed water but, what about calm glassy water or extremely
//! stormy water?"
//!
//! This is the GUI-first rule applied to weather: everything the sim can do to
//! the sky and the sea is reachable from inside the app, with no console and no
//! waiting for a random roll. It is also the only practical way to REVIEW the
//! ocean, whose entire character comes from the JONSWAP wind (see
//! `docs/design/water-fft.md`): the spectrum is rebuilt from wind speed, so the
//! wind slider is the sea-state slider.
//!
//! Permanent dev/operator tooling per the forever-development norm - not a
//! debug hack to be trimmed.
//!
//! Writes into the DataStore's "weather_control" Mutex (the
//! `time_set_hour_request` pattern), because WeatherSystem lives inside the
//! SystemRunner and the GUI cannot touch it directly.

use egui::{Context, RichText};

use crate::gui::theme::Theme;
use crate::gui::GuiState;
use crate::systems::weather::WeatherCondition;

#[cfg(test)]
thread_local! {
    static TEST_STOP_RECTS: std::cell::RefCell<
        std::collections::HashMap<&'static str, egui::Rect>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Rect of a named time stop from the last layout, for the click test.
#[cfg(test)]
pub fn test_stop_rect(name: &str) -> Option<egui::Rect> {
    TEST_STOP_RECTS.with(|m| m.borrow().get(name).copied())
}

/// One row of the preset ladder: a name, the condition, intensity, and the
/// wind that defines its sea. Wind is the important column - it is what the
/// ocean spectrum is built from.
pub struct WeatherPreset {
    pub name: &'static str,
    pub condition: WeatherCondition,
    pub intensity: f32,
    pub wind_speed: f32,
    /// One line on what to look for, so the panel teaches as well as sets
    /// (the inline-explanation norm: the explanation lives ON the thing).
    pub note: &'static str,
}

/// The ladder, calm to violent. Wind values are the real JONSWAP drivers:
/// 0.5 m/s is glass, 8 m/s is the moderate breeze the ocean used to be
/// hardcoded to, 25 m/s is a genuine storm sea (8 m+ significant height).
pub const PRESETS: &[WeatherPreset] = &[
    WeatherPreset {
        name: "Glassy calm",
        condition: WeatherCondition::Clear,
        intensity: 0.0,
        wind_speed: 0.5,
        note: "Near-mirror sea. Watch the sun reflection go from a scattered glitter path to a single hard disc.",
    },
    WeatherPreset {
        name: "Light breeze",
        condition: WeatherCondition::Clear,
        intensity: 0.1,
        wind_speed: 3.0,
        note: "Short capillary chop only, no swell to speak of.",
    },
    WeatherPreset {
        name: "Moderate",
        condition: WeatherCondition::Cloudy,
        intensity: 0.3,
        wind_speed: 8.0,
        note: "The sea the FFT ocean was hardcoded to before v0.1050.",
    },
    WeatherPreset {
        name: "Light rain",
        condition: WeatherCondition::Rain,
        intensity: 0.4,
        wind_speed: 6.0,
        note: "Rain emitters plus a wind-built chop.",
    },
    WeatherPreset {
        name: "Heavy rain",
        condition: WeatherCondition::Rain,
        intensity: 0.9,
        wind_speed: 12.0,
        note: "Dense precipitation, visibility down, real swell building.",
    },
    WeatherPreset {
        name: "Storm",
        condition: WeatherCondition::Storm,
        intensity: 0.8,
        wind_speed: 18.0,
        note: "Steep seas and whitecaps - the Jacobian foam mask earns its keep here.",
    },
    WeatherPreset {
        name: "Hurricane",
        condition: WeatherCondition::Storm,
        intensity: 1.0,
        wind_speed: 25.0,
        note: "JONSWAP at 25 m/s is genuinely violent. Expect 8 m+ significant wave height.",
    },
    WeatherPreset {
        name: "Snow",
        condition: WeatherCondition::Snow,
        intensity: 0.6,
        wind_speed: 4.0,
        note: "Snow emitters; the sea stays a cold light chop.",
    },
    WeatherPreset {
        name: "Fog",
        condition: WeatherCondition::Fog,
        intensity: 0.7,
        wind_speed: 1.0,
        note: "Visibility collapse over an almost-still sea.",
    },
    WeatherPreset {
        name: "Sandstorm",
        condition: WeatherCondition::Sandstorm,
        intensity: 0.8,
        wind_speed: 15.0,
        note: "Desert event - drive somewhere dry to see it properly.",
    },
];

const CONDITIONS: &[(WeatherCondition, &str)] = &[
    (WeatherCondition::Clear, "Clear"),
    (WeatherCondition::Cloudy, "Cloudy"),
    (WeatherCondition::Rain, "Rain"),
    (WeatherCondition::Storm, "Storm"),
    (WeatherCondition::Snow, "Snow"),
    (WeatherCondition::Fog, "Fog"),
    (WeatherCondition::Sandstorm, "Sandstorm"),
];

/// Describe a wind speed the way a sailor would, so the slider means
/// something without needing the Beaufort table memorised.
pub fn sea_description(wind: f32) -> &'static str {
    if wind < 1.0 {
        "glass"
    } else if wind < 3.5 {
        "ripples"
    } else if wind < 6.0 {
        "small wavelets"
    } else if wind < 10.0 {
        "moderate waves"
    } else if wind < 14.0 {
        "fresh, whitecaps"
    } else if wind < 20.0 {
        "rough, spray"
    } else {
        "storm sea"
    }
}

/// Turn a clock multiplier into how long a full game day takes in real time,
/// so the speed slider says something concrete instead of a bare number.
pub fn fmt_day_length(scale: f32) -> String {
    let secs = crate::systems::time::SECONDS_PER_DAY / scale.max(0.001) as f64;
    if secs >= 90.0 {
        format!("{:.0} minutes", secs / 60.0)
    } else {
        format!("{:.0} seconds", secs)
    }
}

/// Draw the panel. Returns true when the operator changed something, so the
/// caller can publish the new control state.
pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) -> bool {
    if !state.show_weather_panel {
        return false;
    }
    let mut changed = false;
    let mut open = true;
    egui::Window::new("Weather and time (F11)")
        .open(&mut open)
        .default_pos([40.0, 80.0])
        .default_width(330.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "Everything that sets the mood of a scene: the clock, the sky \
                     and the sea. Wind is the sea - the ocean spectrum is rebuilt \
                     from wind speed, so these sliders change the waves too.",
                )
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);

            // TIME OF DAY (v0.1224).
            // Operator: "I do not see the control for adjusting time of day
            // while on the homestead. Where can I find that?" There was none.
            // The clock could only be moved over IPC by the screenshot/probe
            // hook (v0.871), whose own comment promised "future in-app dev
            // controls" that never arrived. Time belongs in THIS panel rather
            // than Settings, because it is the same job as the wind slider:
            // it is how the sky, the sun angle and the sea get reviewed.
            //
            // The slider is LOCAL mean solar time, matching the HUD clock and
            // the sun you can actually see. The engine clock underneath is
            // GLOBAL (lon-0), so we convert on the way in. Exposing the global
            // hour here would re-run the sun-clock incident of 2026-08-18,
            // where a 20:04 readout sat beside a noon sun because the two were
            // lon/15 = 8.2 h apart.
            ui.label(RichText::new("Time of day").strong().color(theme.accent()));
            ui.add_space(2.0);
            let g_hour = state.game_time.as_ref().map(|t| t.hour).unwrap_or(0.0);
            let has_local = state
                .game_time
                .as_ref()
                .and_then(|t| t.local_hour)
                .is_some();
            let l_hour = state
                .game_time
                .as_ref()
                .and_then(|t| t.local_hour)
                .unwrap_or(g_hour);
            // Local minus global: this site's longitude offset, in hours.
            let lon_off = l_hour - g_hour;
            let hh = state.time_pick_hour.floor().clamp(0.0, 23.0) as u32;
            let mm = ((state.time_pick_hour - state.time_pick_hour.floor()) * 60.0) as u32;
            ui.label(
                RichText::new(if has_local {
                    format!("{:02}:{:02} local solar time at this site", hh, mm)
                } else {
                    format!("{:02}:{:02}", hh, mm)
                })
                .size(theme.font_size_small)
                .color(theme.text_primary()),
            );
            let scrub = ui.add(
                egui::Slider::new(&mut state.time_pick_hour, 0.0..=24.0)
                    .text("Hour")
                    .fixed_decimals(2),
            );
            if scrub.changed() {
                state.time_hour_request =
                    Some((state.time_pick_hour - lon_off).rem_euclid(24.0));
            }
            // Follow the running clock whenever the operator is not holding the
            // handle, so the slider reads as a clock instead of drifting away
            // from the sky it is meant to describe.
            if !scrub.dragged() && !scrub.has_focus() {
                state.time_pick_hour = l_hour;
            }
            // Named stops: the lighting situations worth returning to. They are
            // only meaningful in LOCAL solar time, which is why the slider is.
            const STOPS: &[(&str, f32, &str)] = &[
                ("Dawn", 6.0, "Sun on the horizon, longest shadows, warmest light."),
                ("Morning", 9.0, "Clean side light. The everyday working sky."),
                ("Noon", 12.0, "Sun overhead. Hardest test for cloud and water shading."),
                ("Golden", 17.0, "Low warm sun, where the sunset tinting shows."),
                ("Sunset", 18.4, "Sun at the horizon: the terminator itself."),
                ("Night", 1.0, "Sun far below the horizon. Everything sunlit should be dark."),
            ];
            for row in STOPS.chunks(3) {
                ui.horizontal(|ui| {
                    for (name, hour, note) in row {
                        let b = ui.add_sized(
                            [98.0, 24.0],
                            egui::Button::new(RichText::new(*name).size(theme.font_size_small))
                                .fill(theme.bg_tertiary()),
                        );
                        #[cfg(test)]
                        TEST_STOP_RECTS.with(|m| {
                            m.borrow_mut().insert(*name, b.rect);
                        });
                        if b.clicked() {
                            state.time_pick_hour = *hour;
                            state.time_hour_request = Some((*hour - lon_off).rem_euclid(24.0));
                        }
                        b.on_hover_text(*note);
                    }
                });
            }
            ui.add_space(theme.spacing_sm);
            // Freeze is what earns this section its keep for review work: a game
            // day is 20 real minutes, so an unfrozen clock walks the sun about 3
            // degrees while you study one frame, and no two captures of the same
            // scene are comparable.
            let mut frozen = state.time_frozen;
            if ui.checkbox(&mut frozen, "Hold the clock still").changed() {
                state.time_frozen = frozen;
                state.time_scale_request =
                    Some(if frozen { 0.0 } else { state.time_speed });
            }
            if !state.time_frozen {
                if ui
                    .add(
                        egui::Slider::new(&mut state.time_speed, 1.0..=600.0)
                            .text("Clock speed")
                            .logarithmic(true)
                            .fixed_decimals(0),
                    )
                    .changed()
                {
                    state.time_scale_request = Some(state.time_speed);
                }
                ui.label(
                    RichText::new(format!(
                        "{:.0}x real time. A full day takes {}.",
                        state.time_speed,
                        fmt_day_length(state.time_speed)
                    ))
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
                );
            }
            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.add_space(theme.spacing_sm);

            // Manual takeover. Off = the sim rolls its own weather every
            // 5-15 minutes, which is why a chosen sky never used to stick.
            let mut manual = state.weather_manual;
            if ui
                .checkbox(&mut manual, "Take control (pause random weather)")
                .changed()
            {
                state.weather_manual = manual;
                changed = true;
            }
            if !state.weather_manual {
                ui.label(
                    RichText::new("Sim is driving. Pick a preset to take over.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.add_space(theme.spacing_sm);

            ui.label(RichText::new("Presets").strong().color(theme.accent()));
            ui.add_space(2.0);
            // Two per row keeps the labels readable at the default width.
            for pair in PRESETS.chunks(2) {
                ui.horizontal(|ui| {
                    for p in pair {
                        let active = state.weather_manual
                            && state.weather_pick_condition == p.condition
                            && (state.weather_pick_wind - p.wind_speed).abs() < 0.01;
                        let btn = ui.add_sized(
                            [148.0, 26.0],
                            egui::Button::new(
                                RichText::new(p.name).size(theme.font_size_small),
                            )
                            .fill(if active {
                                theme.accent()
                            } else {
                                theme.bg_tertiary()
                            }),
                        );
                        if btn.clicked() {
                            state.weather_manual = true;
                            state.weather_pick_condition = p.condition;
                            state.weather_pick_intensity = p.intensity;
                            state.weather_pick_wind = p.wind_speed;
                            state.weather_pick_note = p.note.to_string();
                            state.weather_retrigger = true;
                            changed = true;
                        }
                        btn.on_hover_text(p.note);
                    }
                });
            }
            if !state.weather_pick_note.is_empty() {
                ui.add_space(theme.spacing_sm);
                ui.label(
                    RichText::new(&state.weather_pick_note)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }

            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.add_space(theme.spacing_sm);

            ui.label(RichText::new("Condition").strong().color(theme.accent()));
            ui.add_space(2.0);
            for row in CONDITIONS.chunks(4) {
                ui.horizontal(|ui| {
                    for (cond, label) in row {
                        let active = state.weather_pick_condition == *cond;
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(*label).size(theme.font_size_small),
                                )
                                .fill(if active {
                                    theme.accent()
                                } else {
                                    theme.bg_tertiary()
                                }),
                            )
                            .clicked()
                        {
                            state.weather_manual = true;
                            state.weather_pick_condition = *cond;
                            state.weather_retrigger = true;
                            state.weather_pick_note.clear();
                            changed = true;
                        }
                    }
                });
            }

            ui.add_space(theme.spacing_sm);
            if ui
                .add(
                    egui::Slider::new(&mut state.weather_pick_wind, 0.0..=30.0)
                        .text("Wind m/s")
                        .fixed_decimals(1),
                )
                .changed()
            {
                state.weather_manual = true;
                changed = true;
            }
            ui.label(
                RichText::new(format!(
                    "Sea: {} ({:.0} m/s)",
                    sea_description(state.weather_pick_wind),
                    state.weather_pick_wind
                ))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
            if ui
                .add(
                    egui::Slider::new(&mut state.weather_pick_intensity, 0.0..=1.0)
                        .text("Intensity")
                        .fixed_decimals(2),
                )
                .changed()
            {
                state.weather_manual = true;
                changed = true;
            }

            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.add_space(theme.spacing_sm);

            // ── EFFECT KNOBS (v0.1060) ──
            // Operator: "We should add a bunch of sliders / customizable
            // variables for the various weather effects instead of using
            // specific hardcoded values." These are the real drivers, not
            // cosmetic multipliers: precipitation density scales the spawn rate
            // AND the emitter population cap, and the fog scale multiplies the
            // extinction the whole aerial term is built on.
            ui.label(RichText::new("Effect strength").strong().color(theme.accent()));
            ui.add_space(2.0);
            if ui
                .add(
                    egui::Slider::new(&mut state.settings.precip_density, 0.1..=100.0)
                        .text("Precipitation density")
                        .logarithmic(true)
                        .fixed_decimals(2),
                )
                .changed()
            {
                state.settings_dirty = true;
            }
            ui.label(
                RichText::new(
                    "Rain, snow and hail. Scales spawn rate AND the particle cap -                      the cap is what used to pin heavy rain to light rain, since past                      it a higher rate bought nothing. Goes to 100x on purpose: the                      measured cost is ~17-20 ns per particle per frame on ONE core,                      so 100x rain (160k drops) is about 3 ms of CPU. Past roughly                      50k total particles you are trading frames for density, which                      is a fair trade if you want to see the ceiling.",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            if ui
                .add(
                    egui::Slider::new(&mut state.settings.fog_density, 0.0..=4.0)
                        .text("Fog / dust density")
                        .fixed_decimals(2),
                )
                .changed()
            {
                state.settings_dirty = true;
            }
            ui.label(
                RichText::new(
                    "Multiplies the extinction a Fog, Sandstorm, Snow, Storm or Rain                      condition adds to the air. 0 disables weather fog entirely and                      leaves clear-air aerial perspective.",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );

            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.add_space(theme.spacing_sm);

            // Live readback: what the sim actually exports, which lags the
            // sliders through the 30 s condition transition. Seeing both is
            // the point - it makes the smoothing legible instead of feeling
            // like the panel is ignoring you.
            if let Some(w) = state.weather.as_ref() {
                ui.label(RichText::new("Live").strong().color(theme.accent()));
                ui.label(
                    RichText::new(format!(
                        "{}  {:.0}C  wind {:.1} m/s",
                        w.condition, w.temperature, w.wind_speed
                    ))
                    .size(theme.font_size_small)
                    .color(theme.text_primary()),
                );
                if !w.event.is_empty() {
                    ui.label(
                        RichText::new(format!("event: {}", w.event))
                            .size(theme.font_size_small)
                            .color(theme.warning()),
                    );
                }
            }
            ui.label(
                RichText::new(
                    "Ocean spectrum regenerates when the wind moves ~1 m/s; \
                     the same seed keeps the wave phases, so it reshapes \
                     instead of jumping.",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
        });
    if !open {
        state.show_weather_panel = false;
    }
    changed
}
