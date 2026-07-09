//! Dev page (v0.777) - operator-facing developer tools, under Platform > Dev.
//! Permanent dev infrastructure (operator: "assume perpetual development; never
//! trim dev/debug tooling"), the home for the entity spawn/edit workflow and
//! future instrumentation (world inspector, teleport, time/weather control).
//!
//! First tool: spawn ANY creature/NPC species from data/creatures.csv in front
//! of the player, and despawn them for cleanup. Spawning only sets
//! GuiState.pending_dev_spawn; lib.rs does the actual ECS spawn at the player's
//! position next frame (this page has no world handle). Editing an existing
//! creature is the walk-up companion (a later increment).

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

            let all = species();

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
