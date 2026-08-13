//! Planet Tuner: live per-planet physics editing (artificial-planet ladder
//! item 3, docs/design/artificial-planet.md).
//!
//! The whole loop this page closes: every planet is a data file
//! (data/planets/<id>.ron), the file watcher rebuilds the planet within one
//! frame of a save, and assets::ron_edit rewrites single fields WITHOUT
//! destroying the file's hand-written comments. So tuning a world is: move a
//! slider, press Save, look up. No restart, no text editor, no risk of a
//! corrupt file (ron_edit refuses to return unparseable output).
//!
//! Two halves:
//! - READOUT (top): what the simulation says about where you are standing
//!   right now: gravity at your altitude, temperature where you stand vs the
//!   body's global reference, air pressure and magnetic field from the
//!   cosmos catalog, breathability. Mirrored each frame by
//!   engine::frame_lock::publish_planet_tuner_readout.
//! - EDITORS (below): a working copy of the frame-locked body's PlanetDef.
//!   Edits accumulate locally; Save writes ONLY the changed fields into the
//!   RON. Hot reload then re-parses the file, so `current` catches up and
//!   the dirty markers clear themselves.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::terrain::planet::PlanetDef;

/// Live values mirrored from the engine each frame while the Platform page
/// is open. Written by frame_lock::publish_planet_tuner_readout; read here.
#[derive(Default, Clone)]
pub struct PlanetTunerReadout {
    /// Frame-locked to a body at all (false = home frame / deep space).
    pub locked: bool,
    /// Cosmos body id (e.g. "earth") and display name.
    pub body_id: String,
    pub body_name: String,
    /// Whether data/planets/<id>.ron exists (loaded into planet_defs).
    pub has_def: bool,
    /// Gravity sampled at the player's current altitude (gravity_curve
    /// aware), plus the inputs that produced it.
    pub g_at_player: f32,
    pub altitude_m: f32,
    pub latitude_deg: f32,
    /// The two temperatures of the v1 model: where you stand vs the body's
    /// global reference (see Weather field docs for the contract).
    pub temp_at_player_c: f32,
    pub temp_global_c: f32,
    pub breathable_outside: bool,
    pub day_length_hours: f32,
}

/// Page state: the readout mirror plus the editor's working copy.
/// One field on GuiState (`state.planet_tuner`) holds all of it.
#[derive(Default)]
pub struct PlanetTunerState {
    pub readout: PlanetTunerReadout,
    /// What the engine currently has loaded (refreshed every frame, so a
    /// hot reload after Save shows up here immediately).
    pub current: Option<PlanetDef>,
    /// The editable copy. Initialized from `current` when the locked body
    /// changes (or on Reload); otherwise never overwritten, so edits
    /// survive frames.
    pub working: Option<PlanetDef>,
    /// Which body `working` belongs to; a body switch re-seeds it.
    pub working_body: String,
    /// Raw text editor for gravity_curve, the one multi-line field. Applied
    /// through the same validated rewrite as everything else, so garbage
    /// cannot reach the file.
    pub curve_text: String,
    /// Last save/apply outcome, shown under the buttons.
    pub status: String,
    pub status_is_error: bool,
}

// ── RON value formatting ────────────────────────────────────────────────────
// set_top_level_field takes the replacement as RON source text. Rust's Debug
// float formatting always emits a decimal point (9.0, never 9), which keeps
// a whole-number slider value unambiguously a float when the file reloads.

fn fmt_f32(v: f32) -> String {
    format!("{v:?}")
}

fn fmt_opt_f32(v: Option<f32>) -> String {
    match v {
        Some(x) => format!("Some({x:?})"),
        None => "None".to_string(),
    }
}

fn fmt_color(c: [f32; 4]) -> String {
    format!("({:?}, {:?}, {:?}, {:?})", c[0], c[1], c[2], c[3])
}

fn fmt_opt_color(c: Option<[f32; 4]>) -> String {
    match c {
        Some(v) => format!("Some({})", fmt_color(v)),
        None => "None".to_string(),
    }
}

/// Public: the frame_lock mirror uses this to seed the curve text box when
/// the locked body changes.
pub fn fmt_curve(curve: &Option<Vec<(f64, f32)>>) -> String {
    match curve {
        None => "None".to_string(),
        Some(pts) => {
            let mut s = String::from("Some([\n");
            for (alt, g) in pts {
                s.push_str(&format!("    ({alt:?}, {g:?}),\n"));
            }
            s.push_str("])");
            s
        }
    }
}

/// The changed-field list: (field name, new value as RON text). Only these
/// get written, so untouched fields keep their exact bytes and comments.
fn dirty_fields(current: &PlanetDef, working: &PlanetDef) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let mut f32_field = |name: &'static str, c: f32, w: f32| {
        if c != w {
            out.push((name, fmt_f32(w)));
        }
    };
    f32_field("gravity", current.gravity, working.gravity);
    f32_field("atmosphere_scale", current.atmosphere_scale, working.atmosphere_scale);
    f32_field("sea_level", current.sea_level, working.sea_level);
    f32_field("surface_relief", current.surface_relief, working.surface_relief);
    f32_field("noise_frequency", current.noise_frequency, working.noise_frequency);
    f32_field(
        "polar_cap_latitude",
        current.polar_cap_latitude,
        working.polar_cap_latitude,
    );
    if current.noise_octaves != working.noise_octaves {
        out.push(("noise_octaves", working.noise_octaves.to_string()));
    }
    if current.has_water != working.has_water {
        out.push(("has_water", working.has_water.to_string()));
    }
    if current.breathable != working.breathable {
        out.push(("breathable", working.breathable.to_string()));
    }
    if current.atmosphere_color != working.atmosphere_color {
        out.push(("atmosphere_color", fmt_opt_color(working.atmosphere_color)));
    }
    if current.scale_height_m != working.scale_height_m {
        out.push(("scale_height_m", fmt_opt_f32(working.scale_height_m)));
    }
    if current.cloud_coverage != working.cloud_coverage {
        out.push(("cloud_coverage", fmt_opt_f32(working.cloud_coverage)));
    }
    let mut color_field = |name: &'static str, c: [f32; 4], w: [f32; 4]| {
        if c != w {
            out.push((name, fmt_color(w)));
        }
    };
    color_field("land_color", current.land_color, working.land_color);
    color_field("water_color", current.water_color, working.water_color);
    color_field("shore_color", current.shore_color, working.shore_color);
    color_field("highland_color", current.highland_color, working.highland_color);
    color_field("mountain_color", current.mountain_color, working.mountain_color);
    color_field("cap_color", current.cap_color, working.cap_color);
    if current.basin_color != working.basin_color {
        out.push(("basin_color", fmt_opt_color(working.basin_color)));
    }
    out
}

/// Fold a list of (field, value) rewrites into the RON text, appending
/// fields the file omits (most planet RONs leave serde-defaulted fields
/// out, and flipping those is half the tuner's job). The final text must
/// parse as a full PlanetDef before it is accepted: ron_edit's generic
/// validation admits anything that is syntactically RON (a bare identifier
/// is a legal ron::Value), but only a PlanetDef-shaped file will actually
/// LOAD, and a file the loader rejects is a bricked planet.
fn apply_fields(text: &str, fields: &[(&str, String)]) -> Result<String, String> {
    let mut out = text.to_string();
    for (field, value) in fields {
        out = crate::assets::ron_edit::set_or_append_top_level_field(&out, field, value)
            .map_err(|e| format!("{field}: {e}"))?;
    }
    ron::from_str::<PlanetDef>(&out)
        .map_err(|e| format!("result would not load as a planet: {e}"))?;
    Ok(out)
}

/// Read the body's RON file, apply the rewrites, write it back. One read,
/// one validated write; any failure leaves the file untouched.
fn write_fields(body_id: &str, fields: &[(&str, String)]) -> Result<usize, String> {
    let path = crate::data_dir().join("planets").join(format!("{body_id}.ron"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let out = apply_fields(&text, fields)?;
    std::fs::write(&path, out).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(fields.len())
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Planet Tuner")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new(
                    "Edit the current planet's data file and watch the world rebuild \
                     within a frame. Changed fields are written into \
                     data/planets/<body>.ron without touching its comments.",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            // Same two-part gate as the Dev section: Dev play mode first
            // (names the real blocker), then the cheats kill-switch.
            if !state.settings.play_mode.allows(crate::config::Capability::DevTools) {
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("The Planet Tuner is Dev-mode only.")
                            .size(theme.font_size_body)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new(
                            "Switch Play mode to Dev in Settings > Gameplay to edit \
                             planet physics live.",
                        )
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                });
                return;
            }
            if !theme.cheats_enabled {
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Cheats are switched off.")
                            .size(theme.font_size_body)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new(
                            "Enable cheats in Settings to use the tuner; it edits the \
                             same data files mods use.",
                        )
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                });
                return;
            }

            let locked = state.planet_tuner.readout.locked;
            let has_def = state.planet_tuner.readout.has_def;
            if !locked {
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Not standing on a planet.")
                            .size(theme.font_size_body)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new(
                            "The tuner edits the world you are frame-locked to. Use \
                             Dev > Travel to visit a body (Earth, the Moon, Mars, \
                             Pluto ship data files today), then come back here.",
                        )
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                });
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                draw_readout(ui, theme, state);
                ui.add_space(theme.spacing_md);
                if !has_def {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new("This body has no data file yet.")
                                .size(theme.font_size_body)
                                .strong()
                                .color(theme.text_primary()),
                        );
                        ui.label(
                            RichText::new(
                                "It is simulated as an airless rock. Give it a \
                                 data/planets/<id>.ron (copy moon.ron as a start) and \
                                 it becomes tunable here.",
                            )
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                        );
                    });
                    return;
                }
                draw_editors(ui, theme, state);
            });
        });
}

/// The live numbers card: what the simulation says about HERE, right now.
fn draw_readout(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    let r = &state.planet_tuner.readout;
    // Pressure + magnetic field live in the cosmos catalog, not PlanetDef;
    // the scout's pattern: read them straight from the page.
    let catalog = crate::cosmos::find_body(&r.body_id);
    widgets::card_with_header(ui, theme, &format!("Standing on {}", r.body_name), |ui| {
        widgets::detail_row(ui, theme, "Gravity here", &format!("{:.2} m/s\u{00b2} ({:.2} g)", r.g_at_player, r.g_at_player / 9.81));
        widgets::detail_row(ui, theme, "Altitude (above nominal surface)", &format!("{:.0} m", r.altitude_m));
        widgets::detail_row(ui, theme, "Latitude", &format!("{:.1}\u{00b0}", r.latitude_deg));
        widgets::detail_row(
            ui,
            theme,
            "Temperature here / body reference",
            &format!("{:.1} C / {:.1} C", r.temp_at_player_c, r.temp_global_c),
        );
        if let Some(b) = catalog {
            if b.surface_pressure_pa > 0.0 {
                widgets::detail_row(
                    ui,
                    theme,
                    "Surface pressure",
                    &format!(
                        "{:.0} Pa ({:.2} atm)",
                        b.surface_pressure_pa,
                        b.surface_pressure_pa / 101_325.0
                    ),
                );
            } else {
                widgets::detail_row(ui, theme, "Surface pressure", "none recorded (vacuum or unknown)");
            }
            if b.magnetic_field_t > 0.0 {
                widgets::detail_row(
                    ui,
                    theme,
                    "Magnetic field",
                    &format!("{:.1} microtesla", b.magnetic_field_t * 1.0e6),
                );
            } else {
                widgets::detail_row(ui, theme, "Magnetic field", "no global field");
            }
        }
        widgets::detail_row(
            ui,
            theme,
            "Air outside",
            if r.breathable_outside { "breathable" } else { "not breathable (suit rules)" },
        );
        widgets::detail_row(ui, theme, "Day length", &format!("{:.1} h", r.day_length_hours));
    });
}

fn draw_editors(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let accent = theme.accent();
    let body_id = state.planet_tuner.readout.body_id.clone();
    // Split-borrow dance: take working out, edit, put back.
    let Some(mut working) = state.planet_tuner.working.take() else {
        return;
    };
    let current = state.planet_tuner.current.clone();

    widgets::subsection_header(ui, theme, accent, "Physics", "");
    widgets::settings_row(ui, theme, "Surface gravity (m/s\u{00b2})", |ui| {
        widgets::labeled_slider_entry(ui, theme, "", &mut working.gravity, 0.0..=30.0, 50.0, 0.05);
    });
    widgets::setting_hint(
        ui,
        theme,
        state.settings.hint_display,
        "Walking, jumping, and falling on this body. Earth 9.81, Moon 1.62. \
         The gravity curve below can vary it with altitude and depth.",
    );
    widgets::toggle(ui, theme, "Breathable air", &mut working.breathable);
    widgets::setting_hint(
        ui,
        theme,
        state.settings.hint_display,
        "Whether an unsuited human can breathe the open surface air (below \
         the 8 km ceiling). Off means suit rules outside the hull.",
    );

    widgets::subsection_header(ui, theme, accent, "Atmosphere", "");
    let mut has_atmo = working.atmosphere_color.is_some();
    if widgets::toggle(ui, theme, "Has atmosphere", &mut has_atmo) {
        working.atmosphere_color = if has_atmo {
            Some([0.17, 0.41, 1.0, 0.5])
        } else {
            None
        };
    }
    if let Some(color) = working.atmosphere_color.as_mut() {
        widgets::settings_row(ui, theme, "Scattering color + density", |ui| {
            // Edit a COPY and only accept it on a real user change: egui's
            // color button converts through its internal color space every
            // frame, and that round-trip drifts the floats slightly even
            // with zero interaction, which used to mark every color field
            // dirty the moment the page opened.
            let mut edited = *color;
            if ui.color_edit_button_rgba_unmultiplied(&mut edited).changed() {
                *color = edited;
            }
        });
        widgets::setting_hint(
            ui,
            theme,
            state.settings.hint_display,
            "RGB = per-channel scattering strength (Earth scatters blue \
             hardest, so its sky is blue); alpha = overall density.",
        );
        let mut sh = working.scale_height_m.unwrap_or(8500.0);
        widgets::settings_row(ui, theme, "Scale height (m)", |ui| {
            if widgets::labeled_slider_entry(ui, theme, "", &mut sh, 1000.0..=30000.0, 100_000.0, 50.0)
            {
                working.scale_height_m = Some(sh);
            }
        });
        widgets::setting_hint(
            ui,
            theme,
            state.settings.hint_display,
            "How fast the air thins with altitude. Earth 8500, Mars 11100.",
        );
    }
    let mut has_clouds = working.cloud_coverage.is_some();
    if widgets::toggle(ui, theme, "Cloud deck", &mut has_clouds) {
        working.cloud_coverage = if has_clouds { Some(0.42) } else { None };
    }
    if let Some(cov) = working.cloud_coverage.as_mut() {
        widgets::settings_row(ui, theme, "Cloud coverage", |ui| {
            widgets::labeled_slider(ui, theme, "", cov, 0.0..=1.0);
        });
    }

    widgets::subsection_header(ui, theme, accent, "Water and terrain", "");
    widgets::toggle(ui, theme, "Liquid water", &mut working.has_water);
    let has_heightmap = working.heightmap.is_some();
    if has_heightmap {
        widgets::setting_hint(
            ui,
            theme,
            state.settings.hint_display,
            "This body uses a real elevation grid, so sea level is measured \
             from the grid at load time; the sea_level field is ignored.",
        );
    } else {
        widgets::settings_row(ui, theme, "Sea level", |ui| {
            widgets::labeled_slider(ui, theme, "", &mut working.sea_level, 0.0..=1.0);
        });
    }
    widgets::settings_row(ui, theme, "Surface relief", |ui| {
        widgets::labeled_slider_entry(ui, theme, "", &mut working.surface_relief, 0.0..=0.05, 0.2, 0.0005);
    });
    widgets::setting_hint(
        ui,
        theme,
        state.settings.hint_display,
        "Mountain height as a fraction of the planet's radius. Earth's real \
         relief is about 0.003.",
    );
    widgets::settings_row(ui, theme, "Terrain noise frequency", |ui| {
        widgets::labeled_slider(ui, theme, "", &mut working.noise_frequency, 0.1..=10.0);
    });
    let mut octaves = working.noise_octaves as f32;
    widgets::settings_row(ui, theme, "Terrain noise octaves", |ui| {
        if widgets::labeled_slider(ui, theme, "", &mut octaves, 1.0..=8.0) {
            working.noise_octaves = octaves.round().clamp(1.0, 8.0) as u32;
        }
    });
    widgets::settings_row(ui, theme, "Polar cap latitude", |ui| {
        widgets::labeled_slider(ui, theme, "", &mut working.polar_cap_latitude, 0.0..=1.2);
    });
    widgets::setting_hint(
        ui,
        theme,
        state.settings.hint_display,
        "Caps appear where |sin latitude| exceeds this. Above 1.0 disables \
         caps entirely (the Moon).",
    );

    widgets::subsection_header(ui, theme, accent, "Surface palette", "");
    let mut color_row = |ui: &mut egui::Ui, label: &str, c: &mut [f32; 4]| {
        widgets::settings_row(ui, theme, label, |ui| {
            // Copy-then-accept-on-change, same reason as the atmosphere
            // color: the widget's internal conversion drifts untouched
            // values and would false-dirty every palette row.
            let mut edited = *c;
            if ui.color_edit_button_rgba_unmultiplied(&mut edited).changed() {
                *c = edited;
            }
        });
    };
    color_row(ui, "Land", &mut working.land_color);
    color_row(ui, "Water", &mut working.water_color);
    color_row(ui, "Shore", &mut working.shore_color);
    color_row(ui, "Highland", &mut working.highland_color);
    color_row(ui, "Mountain", &mut working.mountain_color);
    color_row(ui, "Ice cap", &mut working.cap_color);

    widgets::subsection_header(
        ui,
        theme,
        accent,
        "Gravity curve",
        "The manufactured-world hook: (altitude_m, g) control points, \
         linear between, clamped past the ends. Negative altitude is below \
         the surface.",
    );
    ui.add(
        egui::TextEdit::multiline(&mut state.planet_tuner.curve_text)
            .desired_rows(4)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Monospace),
    );
    if ui
        .button(RichText::new("Apply gravity curve").size(theme.font_size_small))
        .clicked()
    {
        let value = state.planet_tuner.curve_text.trim().to_string();
        match write_fields(&body_id, &[("gravity_curve", value)]) {
            Ok(_) => {
                state.planet_tuner.status = "Gravity curve saved; planet reloading.".to_string();
                state.planet_tuner.status_is_error = false;
            }
            Err(e) => {
                state.planet_tuner.status = format!("Curve rejected: {e}");
                state.planet_tuner.status_is_error = true;
            }
        }
    }

    ui.add_space(theme.spacing_md);
    let dirty = current
        .as_ref()
        .map(|c| dirty_fields(c, &working))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let save = ui.add_enabled(
            !dirty.is_empty(),
            egui::Button::new(
                RichText::new(format!("Save {} change(s) to {body_id}.ron", dirty.len()))
                    .size(theme.font_size_body),
            ),
        );
        if save.clicked() {
            match write_fields(&body_id, &dirty) {
                Ok(n) => {
                    state.planet_tuner.status =
                        format!("{n} field(s) saved; the planet is rebuilding.");
                    state.planet_tuner.status_is_error = false;
                }
                Err(e) => {
                    state.planet_tuner.status = format!("Save failed: {e}");
                    state.planet_tuner.status_is_error = true;
                }
            }
        }
        if ui
            .button(RichText::new("Discard edits").size(theme.font_size_body))
            .clicked()
        {
            if let Some(c) = current.as_ref() {
                working = c.clone();
                state.planet_tuner.curve_text = fmt_curve(&c.gravity_curve);
            }
            state.planet_tuner.status = "Edits discarded; showing the file's values.".to_string();
            state.planet_tuner.status_is_error = false;
        }
    });
    if !state.planet_tuner.status.is_empty() {
        let color = if state.planet_tuner.status_is_error {
            theme.danger()
        } else {
            theme.success()
        };
        ui.label(
            RichText::new(&state.planet_tuner.status)
                .size(theme.font_size_small)
                .color(color),
        );
    }

    state.planet_tuner.working = Some(working);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mars_text() -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(root.join("data/planets/mars.ron")).expect("mars.ron readable")
    }

    fn mars_def() -> PlanetDef {
        ron::from_str(&mars_text()).expect("mars.ron parses")
    }

    /// The bug the snapshot caught in the flesh: an untouched working copy
    /// must report ZERO dirty fields, or the Save button lies the moment
    /// the page opens.
    #[test]
    fn identical_defs_have_no_dirty_fields() {
        let def = mars_def();
        assert!(dirty_fields(&def, &def.clone()).is_empty());
    }

    /// Edits flow end to end in memory against the REAL mars.ron text:
    /// dirty detection names exactly the touched fields; existing fields
    /// rewrite in place; fields the file OMITS (breathable, cloud_coverage
    /// on Mars, both serde-defaulted) get appended; the result parses with
    /// the new values and every untouched field intact; and the file's
    /// comments all survive.
    #[test]
    fn dirty_fields_apply_to_the_real_file_text() {
        let current = mars_def();
        let mut working = current.clone();
        working.gravity = 5.5;
        working.breathable = true;
        working.atmosphere_color = Some([0.2, 0.3, 0.9, 0.6]);
        working.cloud_coverage = Some(0.25);
        let dirty = dirty_fields(&current, &working);
        let names: Vec<&str> = dirty.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec!["gravity", "breathable", "atmosphere_color", "cloud_coverage"]
        );
        let text = mars_text();
        let rewritten = apply_fields(&text, &dirty).expect("all four apply");
        let reparsed: PlanetDef = ron::from_str(&rewritten).expect("rewritten mars.ron parses");
        assert_eq!(reparsed.gravity, 5.5);
        assert!(reparsed.breathable);
        assert_eq!(reparsed.atmosphere_color, Some([0.2, 0.3, 0.9, 0.6]));
        assert_eq!(reparsed.cloud_coverage, Some(0.25));
        // Untouched fields kept their exact values.
        assert_eq!(reparsed.radius, current.radius);
        assert_eq!(reparsed.terrain_seed, current.terrain_seed);
        // Every whole-line comment from the original survives verbatim.
        for line in text.lines().filter(|l| l.trim_start().starts_with("//")) {
            assert!(
                rewritten.contains(line),
                "comment line lost in rewrite: {line}"
            );
        }
    }

    /// The curve text box round-trips end to end: fmt_curve output is
    /// APPENDED to mars.ron (which has no gravity_curve field) and parses
    /// back to the same points. And the banana test: garbage that is
    /// syntactically valid RON but not a valid PlanetDef is refused by the
    /// final loads-as-a-planet check, never written.
    #[test]
    fn curve_text_round_trips_and_garbage_is_refused() {
        let curve = Some(vec![(-200_000.0_f64, 9.81_f32), (0.0, 9.81), (100_000.0, 4.9)]);
        let formatted = fmt_curve(&curve);
        let text = mars_text();
        let rewritten = apply_fields(&text, &[("gravity_curve", formatted)])
            .expect("formatted curve appends to a file without one");
        let def: PlanetDef = ron::from_str(&rewritten).expect("parses");
        assert_eq!(def.gravity_curve, curve);
        // `banana` is a legal bare RON identifier, so generic validation
        // admits it; only the PlanetDef parse knows it cannot load. This
        // exact input used to slip through and would have bricked the file.
        let err = apply_fields(&text, &[("gravity_curve", "Some([(banana)])".to_string())])
            .expect_err("garbage curve refused");
        assert!(err.contains("would not load as a planet"), "got: {err}");
    }
}
