//! Dev page (v0.777) - operator-facing developer tools, under Platform > Dev.
//! Permanent dev infrastructure (operator: "assume perpetual development; never
//! trim dev/debug tooling"), the home for the entity spawn/edit workflow and
//! future instrumentation (world inspector, time/weather control).
//!
//! First tool: spawn ANY creature/NPC species from data/creatures.csv in front
//! of the player, and despawn them for cleanup. Spawning only sets
//! GuiState.pending_dev_spawn; lib.rs does the actual ECS spawn at the player's
//! position next frame (this page has no world handle). Editing an existing
//! creature is the walk-up companion (a later increment).
//!
//! Second tool (v0.791.x): Travel - teleport to any rendered solar body (parked
//! ~4 radii out on the sunlit side) plus an FTL fly-speed multiplier, so the
//! operator can inspect planet surfaces instead of walking at 5 m/s toward
//! something 1.5e11 m away. Same pattern: the page only sets
//! GuiState.pending_dev_teleport / dev_fly_* and lib.rs applies them.

use egui::{RichText, ScrollArea};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// One spawnable species row, loaded once from creatures.csv for the list.
struct SpeciesRow {
    id: String,
    name: String,
    kind: String,
    hostility: String,
}

/// The 92 species, sorted by name, loaded once. Reads the same creatures.csv
/// the runtime CreatureRegistry does (embedded fallback for distributed builds),
/// so the list and the actual spawn never drift.
fn species() -> &'static Vec<SpeciesRow> {
    static CACHE: std::sync::OnceLock<Vec<SpeciesRow>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        let text = crate::embedded_data::read_data_or_embedded(&crate::data_dir(), "creatures.csv")
            .unwrap_or_default();
        let mut rows: Vec<SpeciesRow> =
            match crate::systems::livestock::CreatureRegistry::from_csv(text.as_bytes()) {
                Ok(reg) => reg
                    .defs
                    .into_values()
                    .map(|d| SpeciesRow {
                        id: d.id,
                        name: d.name,
                        kind: d.kind,
                        hostility: d.hostility,
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        rows
    })
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Dev").size(theme.font_size_title).color(theme.text_primary()));
            ui.label(
                RichText::new("Developer tools. Spawn any creature or NPC in front of you, or clear them out.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            // Play-mode gate first (task #50): the whole page is Dev-mode
            // tooling. Normal/Creative players get an honest pointer, not a
            // half-working page. Checked BEFORE the cheats switch so the
            // message names the real blocker.
            if !state.settings.play_mode.allows(crate::config::Capability::DevTools) {
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Dev tools are Dev-mode only.")
                            .size(theme.font_size_body)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new(
                            "Switch Play mode to Dev in Settings > Gameplay to use the \
                             spawn tools, travel/FTL, and the walk-up creature editor (G).",
                        )
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                });
                return;
            }
            // Same kill-switch as every other dev affordance (v0.779): the G
            // editor, "stock all materials", and the inventory dev buttons all
            // honor the Settings cheats toggle -- spawning hostile creatures
            // (or despawning the herd) shouldn't bypass it. Kept ON TOP of the
            // play-mode gate above (both must pass, see dev_cheats_active).
            if !theme.cheats_enabled {
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Dev cheats are turned off.")
                            .size(theme.font_size_body)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new("Enable them in Settings to use the spawn tools and the walk-up creature editor (G).")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                });
                return;
            }

            let all = species();

            // ── Travel (v0.791.x): teleport + FTL fly speed ──
            draw_travel_card(ui, theme, state);
            ui.add_space(theme.spacing_sm);

            // Status + cleanup.
            widgets::card(ui, theme, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} creatures in the world", state.dev_creature_count))
                            .size(theme.font_size_body)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Despawn all")
                            .on_hover_text("Remove every spawned creature from the world.")
                            .clicked()
                        {
                            state.pending_dev_despawn_creatures = true;
                        }
                    });
                });
                ui.label(
                    RichText::new(
                        "Spawn drops the species about 2 m in front of where you're standing. \
                         Enter the world first so it has somewhere to appear.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
                );
                ui.label(
                    RichText::new(
                        "Edit an existing one: in the world, look at any creature and press G \
                         to open its editor (rename, health, hostility, size, tint, despawn).",
                    )
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
                );
            });
            ui.add_space(theme.spacing_sm);

            // Search.
            ui.horizontal(|ui| {
                ui.label(RichText::new("Search").color(theme.text_secondary()));
                ui.text_edit_singleline(&mut state.dev_spawn_filter);
                if !state.dev_spawn_filter.is_empty() && ui.small_button("clear").clicked() {
                    state.dev_spawn_filter.clear();
                }
            });
            ui.add_space(theme.spacing_xs);

            let filter = state.dev_spawn_filter.to_lowercase();
            let mut spawn: Option<String> = None;
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if all.is_empty() {
                    ui.label(
                        RichText::new("No creatures.csv loaded (expected data/creatures.csv).")
                            .size(theme.font_size_small)
                            .color(theme.warning()),
                    );
                    return;
                }
                let mut shown = 0usize;
                for s in all.iter() {
                    if !filter.is_empty()
                        && !s.name.to_lowercase().contains(&filter)
                        && !s.id.to_lowercase().contains(&filter)
                        && !s.kind.to_lowercase().contains(&filter)
                    {
                        continue;
                    }
                    shown += 1;
                    ui.horizontal(|ui| {
                        if ui.button("Spawn").clicked() {
                            spawn = Some(s.id.clone());
                        }
                        ui.label(RichText::new(&s.name).strong().color(theme.text_primary()));
                        let meta = if s.hostility.is_empty() {
                            s.kind.clone()
                        } else {
                            format!("{} · {}", s.kind, s.hostility)
                        };
                        if !meta.is_empty() {
                            ui.label(
                                RichText::new(meta)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                        }
                    });
                }
                if shown == 0 {
                    ui.label(
                        RichText::new("No species match your search.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                }
            });

            if let Some(id) = spawn {
                state.pending_dev_spawn = Some(id);
            }
        });
}

/// Travel card (v0.791.x): teleport to any rendered solar body + the FTL fly
/// speed. The body list mirrors the celestial pass filter in lib.rs (the Sun,
/// every direct sun-orbiter, and our Moon - the bodies that actually have a
/// rendered sphere in the sky), so the list and the sky never drift. Clicking
/// a body sets pending_dev_teleport; lib.rs moves ship_world_pos to a sunlit
/// vantage ~4 radii out next frame and aims the camera. Offline/local world
/// only: teleporting while the relay's shared world is joined would fight the
/// co-presence position sync (and its anti-teleport validation), so the whole
/// card gates on copresence_active.
fn draw_travel_card(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Travel")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new("teleport + FTL flight for looking at the solar system")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });
        // Shared-world handling (v0.801): Dev mode = the ADMIN path - travel
        // works, but engaging it steps you OUT of the shared world (the relay
        // broadcasts your departure; others never see a frozen ghost) and
        // Return home steps you back in. Non-Dev modes keep the hard gate.
        if state.copresence_active {
            if state.settings.play_mode == crate::config::PlayMode::Dev {
                ui.label(
                    RichText::new(
                        "You are in the shared world. Traveling steps you OUT of it \
                         (others see you leave; your avatar does not linger) - Return \
                         home steps you back in.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.warning()),
                );
            } else {
                ui.label(
                    RichText::new(
                        "Travel tools require the Dev play mode while you are in the \
                         shared world (Settings > Gameplay > Play mode), so a normal \
                         session can never silently teleport.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.warning()),
                );
                return;
            }
        }

        // Fly mode + speed. The multiplier slider is logarithmic (1x..1e9x);
        // above x1k the SHIP flies (ship_world_pos, f64) so crossing an AU
        // takes seconds without wrecking f32 camera precision.
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.dev_fly_mode, "Fly mode");
            ui.label(
                RichText::new("no gravity, no walls; W flies where you look")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Speed").color(theme.text_secondary()));
            ui.add(
                egui::Slider::new(&mut state.dev_fly_speed_mult, 1.0..=1.0e9)
                    .logarithmic(true)
                    .custom_formatter(|v, _| {
                        crate::dev_travel::format_multiplier(v as f32)
                    }),
            );
            if ui.small_button("1x").on_hover_text("Reset to walking speed.").clicked() {
                state.dev_fly_speed_mult = 1.0;
            }
        });
        ui.label(
            RichText::new(
                "Mouse wheel while flying steps the speed x10 per notch. Above x1k \
                 the homeship itself flies (FTL) - it travels with you.",
            )
            .size(theme.font_size_small)
            .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_xs);

        // Teleport targets: home first, then the same bodies the sky renders.
        ui.horizontal_wrapped(|ui| {
            let home = ui.add_enabled(
                state.dev_travel_away,
                egui::Button::new("Return home"),
            );
            if home
                .on_hover_text("Restore the position you were at before the first teleport.")
                .clicked()
            {
                state.pending_dev_teleport = Some("home".to_string());
            }
            for b in crate::cosmos::sol_bodies() {
                // Mirror the celestial-pass visibility filter (lib.rs): the
                // Sun, direct sun-orbiters, and our Moon.
                let is_sun = b.body_type == "star";
                let direct_solar = b.parent.as_deref() == Some("sun");
                if !is_sun && !direct_solar && b.id != "moon" {
                    continue;
                }
                if ui
                    .button(&b.name)
                    .on_hover_text(format!(
                        "Teleport to {} ({}, radius {:.0} km) - arrives ~4 radii out \
                         on the sunlit side, with fly mode on.",
                        b.name, b.body_type, b.radius_km
                    ))
                    .clicked()
                {
                    state.pending_dev_teleport = Some(b.id.clone());
                }
            }
        });

        // Land on surface (2026-07-12): the buttons above arrive in ORBIT (~4
        // radii out), where the planet rotates beneath you (the ISS view) and
        // flying down to the ground is fiddly. THIS row drops you straight to
        // low altitude over the surface and engages surface mode - "down"
        // points at the planet centre, the horizon is level, gravity settles
        // you to standing height, and the ground is held STILL (the same place
        // the AI camera tool lands). Offered for the non-star bodies.
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new("Land on surface (stand on the ground, planet held still):")
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        ui.horizontal_wrapped(|ui| {
            for b in crate::cosmos::sol_bodies() {
                let is_sun = b.body_type == "star";
                let direct_solar = b.parent.as_deref() == Some("sun");
                if is_sun || (!direct_solar && b.id != "moon") {
                    continue;
                }
                if ui
                    .button(&b.name)
                    .on_hover_text(format!(
                        "Drop to the surface of {} and stand on the ground (surface \
                         mode: local gravity, level horizon, the planet held still).",
                        b.name
                    ))
                    .clicked()
                {
                    state.pending_dev_surface = Some(b.id.clone());
                }
            }
        });
        if state.dev_travel_away {
            ui.label(
                RichText::new("You are away from home. Vitals are safe while fly mode is on.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        }

        // ── Location bookmarks (v0.913, operator: "can we add a teleport
        // location selector for the spots I've added with the F6 key?") ──
        // F6 saves the exact pose; this list restores it. Grouped by
        // category; the category box tags FUTURE F6 saves, so building a
        // curated tour (Mt Fuji / Home 2 / beautiful test spots) is: type
        // the category, fly there, press F6.
        ui.add_space(theme.spacing_sm);
        ui.separator();
        ui.label(
            RichText::new("Location bookmarks (F6 saves your exact spot):")
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Category for new F6 saves:")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.bookmark_new_category)
                    .hint_text("e.g. Scenic, Home, Testing")
                    .desired_width(160.0),
            );
            if ui.button("Reload list").clicked() {
                state.location_bookmarks_dirty = true;
            }
        });
        if state.location_bookmarks.is_empty() {
            ui.label(
                RichText::new("No bookmarks yet. Press F6 anywhere in the 3D world to save one.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        } else {
            // Stable category order: first-seen wins.
            let mut cats: Vec<String> = Vec::new();
            for (_, c, _) in &state.location_bookmarks {
                if !cats.contains(c) {
                    cats.push(c.clone());
                }
            }
            let mut go: Option<String> = None;
            let mut del: Option<String> = None;
            let mut recat: Option<(String, String)> = None;
            for cat in cats {
                ui.label(
                    RichText::new(&cat)
                        .size(theme.font_size_small)
                        .color(theme.accent()),
                );
                ui.horizontal_wrapped(|ui| {
                    for (id, c, body) in &state.location_bookmarks {
                        if *c != cat {
                            continue;
                        }
                        if ui
                            .button(format!("{id} ({body})"))
                            .on_hover_text("Teleport to this saved spot (exact position and view direction).")
                            .clicked()
                        {
                            go = Some(id.clone());
                        }
                        if ui
                            .small_button("x")
                            .on_hover_text(format!("Delete bookmark {id} (cannot be undone)."))
                            .clicked()
                        {
                            del = Some(id.clone());
                        }
                        if ui
                            .small_button(">")
                            .on_hover_text(
                                "Move this bookmark into the category typed in the box above.",
                            )
                            .clicked()
                        {
                            let target = if state.bookmark_new_category.trim().is_empty() {
                                "Uncategorized".to_string()
                            } else {
                                state.bookmark_new_category.trim().to_string()
                            };
                            recat = Some((id.clone(), target));
                        }
                    }
                });
            }
            if let Some(id) = go {
                state.pending_bookmark_teleport = Some(id);
            }
            if let Some(id) = del {
                state.pending_bookmark_delete = Some(id);
            }
            if let Some(rc) = recat {
                state.pending_bookmark_recat = Some(rc);
            }
        }
    });
}

/// Walk-up creature editor (v0.778): a cursor-free panel (bottom-right, so it
/// clears the bottom-left chat feed) for the creature you're facing. Opened by
/// pressing G while looking at a creature with dev/cheats on. It edits GuiState
/// buffers that lib.rs snapshots on open and writes back to the entity live each
/// frame; Despawn removes it; Close or Esc returns to gameplay. Answers the
/// operator's "I can't walk up to them in FPS mode ... to edit them".
pub fn draw_creature_editor(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if state.dev_edit_target.is_none() {
        return;
    }
    let mut close = false;
    let mut despawn = false;
    let hmax = state.dev_edit_health_max.max(1.0);

    egui::Area::new(egui::Id::new("dev_creature_editor"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(300.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Edit creature").strong().color(theme.accent()));
                        if !state.dev_edit_species.is_empty() {
                            ui.label(
                                RichText::new(&state.dev_edit_species)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Name").color(theme.text_secondary()));
                        ui.text_edit_singleline(&mut state.dev_edit_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Health").color(theme.text_secondary()));
                        ui.add(egui::Slider::new(&mut state.dev_edit_health, 0.0..=hmax));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Max HP").color(theme.text_secondary()));
                        ui.add(egui::DragValue::new(&mut state.dev_edit_health_max).speed(1.0));
                    });
                    ui.checkbox(&mut state.dev_edit_hostile, "Hostile (attacks the player)");
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Size").color(theme.text_secondary()));
                        ui.add(egui::Slider::new(&mut state.dev_edit_scale, 0.1..=3.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Tint").color(theme.text_secondary()));
                        ui.color_edit_button_rgb(&mut state.dev_edit_tint);
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Despawn").clicked() {
                            despawn = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close (Esc)").clicked() {
                                close = true;
                            }
                        });
                    });
                });
        });

    // Despawn KEEPS dev_edit_target so lib.rs can resolve + remove the entity
    // next frame; the consumer clears it. Close just returns to gameplay.
    if despawn {
        state.pending_dev_edit_despawn = true;
    }
    if close {
        state.dev_edit_target = None;
    }
}
