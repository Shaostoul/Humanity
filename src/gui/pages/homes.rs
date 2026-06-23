//! Home — your offline homestead (v0.379).
//!
//! Surfaces the existing Fibonacci homestead blueprint (data/blueprints/
//! fibonacci_homestead.ron) as a browsable Design. Pick a build SCALE (Solo /
//! Family / Community / Colony) and see, for that scale:
//!   - the total power + water DEMAND,
//!   - the aggregated BILL OF MATERIALS (the parts list — the real-world-bridge
//!     north star: what you would 3D-print / buy / trade to build it),
//!   - a SELF-SUFFICIENCY summary (the generation / collection / recycling systems
//!     that close the loops),
//!   - the rooms grouped by construction tier.
//!
//! Offline-first (operator 2026-06-07: "keep developing offline; multiplayer
//! after offline singleplayer works"). Read-only for now (no editing / building
//! yet). Server + Real homes, and exact closure SCORING (output vs demand per
//! loop), are the next data layers. See docs/design/homes-as-profiles.md.

use egui::{RichText, ScrollArea, Frame};
use crate::gui::{GuiState, HomesteadDesign, DesignRoom, TowerConfig, TowerCompat};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// Build scales, from the blueprint's scaling_notes. Each includes every room up
/// to a Fibonacci-index ceiling (Solo 1-8, Family 1-13, Community 1-55, Colony all).
#[derive(Clone, Copy, PartialEq)]
enum Scale {
    Solo,
    Family,
    Community,
    Colony,
}

impl Scale {
    fn ceiling(self) -> u32 {
        match self {
            Scale::Solo => 8,
            Scale::Family => 13,
            Scale::Community => 55,
            Scale::Colony => u32::MAX,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Scale::Solo => "Solo",
            Scale::Family => "Family",
            Scale::Community => "Community",
            Scale::Colony => "Colony",
        }
    }
    fn blurb(self) -> &'static str {
        match self {
            Scale::Solo => "Just you: rooms F1 to F8.",
            Scale::Family => "A household: rooms F1 to F13.",
            Scale::Community => "A village: rooms F1 to F55.",
            Scale::Colony => "Everything: all rooms to F233+.",
        }
    }
}

fn with_scale<R>(f: impl FnOnce(&mut Scale) -> R) -> R {
    thread_local! {
        static SCALE: RefCell<Scale> = RefCell::new(Scale::Solo);
    }
    SCALE.with(|s| f(&mut s.borrow_mut()))
}

/// "steel_ingot_0" -> "Steel Ingot" for display (the data ids are game item ids).
fn humanize(id: &str) -> String {
    let base = id.strip_suffix("_0").unwrap_or(id);
    base.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            // Clone so the immutable design can be browsed without holding a borrow
            // of state (cheap; the blueprint is small).
            let Some(design) = state.homestead_design.clone() else {
                ui.label(RichText::new("Home").size(theme.font_size_title).color(theme.text_primary()));
                ui.add_space(theme.spacing_sm);
                ui.label(
                    RichText::new("No homestead blueprint loaded (expected data/blueprints/fibonacci_homestead.ron).")
                        .color(theme.text_muted()),
                );
                return;
            };
            let power = LivePower {
                gen: state.power_generation,
                usage: state.power_consumption,
                balance: state.power_balance,
                battery_wh: state.power_battery_wh,
                capacity_wh: state.power_battery_capacity_wh,
                autonomy: state.power_autonomy_hours,
            };
            draw_design(ui, theme, &design, &state.tower_configs, &state.tower_compat, &state.homestead_loops, power);
        });
}

/// Live power readout from the running sim (SolarSystem + ElectricalSystem -> PowerStatus
/// -> GuiState), passed into `draw_design` (which has no `&GuiState`). Zero in a build
/// with no home.ron -> the Live power card hides itself.
#[derive(Clone, Copy, Default)]
struct LivePower {
    gen: f32,
    usage: f32,
    balance: f32,
    battery_wh: f32,
    capacity_wh: f32,
    autonomy: f32,
}

fn draw_design(
    ui: &mut egui::Ui,
    theme: &Theme,
    design: &HomesteadDesign,
    towers: &[TowerConfig],
    compat: &[TowerCompat],
    loops: &[crate::machines::HomeLoop],
    power: LivePower,
) {
    ui.label(RichText::new("Your Home").size(theme.font_size_title).color(theme.text_primary()));
    ui.label(RichText::new(&design.name).size(theme.font_size_heading).color(theme.accent()));
    ui.label(RichText::new(&design.description).size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_xs);

    // "Your Home" identity (the save-wrapper model, v0.380): this design IS your
    // offline home. Progressive disclosure -- one home, no manager, until there is
    // a reason for more. Play-loads-this-home + progress persistence is next.
    widgets::card(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Offline home").size(theme.font_size_small).strong().color(theme.accent()));
            ui.label(RichText::new("you own the save").size(theme.font_size_small).color(theme.text_muted()));
        });
        ui.label(
            RichText::new("This homestead is now a save profile (kind: offline, design: fibonacci). Saving your progress here and entering it from Play is the next step.")
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
    });
    ui.add_space(theme.spacing_sm);

    // Scale selector.
    let scale = with_scale(|s| {
        ui.horizontal(|ui| {
            for sc in [Scale::Solo, Scale::Family, Scale::Community, Scale::Colony] {
                if widgets::Button::secondary(sc.label()).active(*s == sc).show(ui, theme) {
                    *s = sc;
                }
            }
        });
        *s
    });
    ui.label(RichText::new(scale.blurb()).size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_sm);

    // Rooms included at this scale.
    let rooms: Vec<&DesignRoom> = design
        .rooms
        .iter()
        .filter(|r| r.fibonacci_index <= scale.ceiling())
        .collect();

    let total_power: u32 = rooms.iter().map(|r| r.requirements.power_watts).sum();
    let total_water: u32 = rooms.iter().map(|r| r.requirements.water_liters_per_day).sum();

    // Aggregate the bill of materials across the included rooms.
    let mut bom: Vec<(String, u32)> = Vec::new();
    for r in &rooms {
        for (id, qty) in &r.requirements.materials {
            if let Some(slot) = bom.iter_mut().find(|s| s.0 == *id) {
                slot.1 += *qty;
            } else {
                bom.push((id.clone(), *qty));
            }
        }
    }
    bom.sort_by(|a, b| b.1.cmp(&a.1));
    let total_parts: u32 = bom.iter().map(|(_, q)| *q).sum();

    // Live power from the running sim (passed in; draw_design has no &GuiState).
    let live_gen = power.gen;
    let live_use = power.usage;
    let live_balance = power.balance;
    let live_battery_wh = power.battery_wh;
    let live_capacity = power.capacity_wh;
    let live_autonomy = power.autonomy;

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        // ── At a glance ──
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new("At a glance").size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
            widgets::detail_row(ui, theme, "Rooms", &rooms.len().to_string());
            widgets::detail_row(ui, theme, "Power demand", &format!("{} W", total_power));
            widgets::detail_row(ui, theme, "Water demand", &format!("{} L/day", total_water));
            widgets::detail_row(ui, theme, "Distinct parts", &bom.len().to_string());
            widgets::detail_row(ui, theme, "Total parts", &total_parts.to_string());
        });
        ui.add_space(theme.spacing_sm);

        // ── Live power (the running sim, v0.518) ──
        // Generation swings with day/night; the battery charges on surplus + discharges
        // on deficit. This is the home as a LIVE sim, not the authored demand above.
        if live_gen > 0.0 || live_use > 0.0 || live_capacity > 0.0 {
            widgets::card(ui, theme, |ui| {
                ui.label(RichText::new("Live power").size(theme.font_size_body).strong().color(theme.text_primary()));
                ui.label(
                    RichText::new("The running sim: solar tracks the time of day, the battery buffers the swing.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
                ui.add_space(theme.spacing_xs);
                widgets::detail_row(ui, theme, "Generation", &format!("{:.0} W", live_gen));
                widgets::detail_row(ui, theme, "Consumption", &format!("{:.0} W", live_use));
                let (bal_text, bal_color) = if live_balance >= 0.0 {
                    (format!("+{:.0} W surplus", live_balance), theme.success())
                } else {
                    (format!("{:.0} W deficit", live_balance), theme.danger())
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Balance").size(theme.font_size_small).color(theme.text_secondary()));
                    ui.label(RichText::new(bal_text).size(theme.font_size_small).strong().color(bal_color));
                });
                if live_capacity > 0.0 {
                    let pct = (live_battery_wh / live_capacity * 100.0).clamp(0.0, 100.0);
                    widgets::detail_row(
                        ui,
                        theme,
                        "Battery",
                        &format!("{:.0}%  ({:.1} kWh)  ~{:.1} h autonomy", pct, live_battery_wh / 1000.0, live_autonomy),
                    );
                }
            });
            ui.add_space(theme.spacing_sm);
        }

        // ── Self-sufficiency systems ──
        let kit = self_sufficiency_kit(&bom);
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new("Self-sufficiency systems").size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.label(
                RichText::new("What closes your loops: generate power, collect and recycle water, grow food.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);
            if kit.is_empty() {
                ui.label(RichText::new("None at this scale.").size(theme.font_size_small).color(theme.text_muted()));
            } else {
                for (label, n) in &kit {
                    widgets::detail_row(ui, theme, label, &n.to_string());
                }
            }
        });
        ui.add_space(theme.spacing_sm);

        // ── Loop closure (the self-sufficiency demonstration, v0.432) ──
        // The four coupled loops with whether each closes, from data/machines/home.ron.
        if !loops.is_empty() {
            let closed = loops.iter().filter(|l| l.closes).count();
            widgets::card(ui, theme, |ui| {
                ui.label(RichText::new("Closed-loop self-sufficiency").size(theme.font_size_body).strong().color(theme.text_primary()));
                ui.label(
                    RichText::new(format!(
                        "{}/{} loops close. You are only as self-sufficient as your weakest loop.",
                        closed, loops.len()
                    ))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
                );
                ui.add_space(theme.spacing_xs);
                for l in loops {
                    let (mark, mark_color) = if l.closes {
                        ("closed", theme.success())
                    } else {
                        ("short", theme.danger())
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&l.name).size(theme.font_size_small).strong().color(theme.text_primary()));
                        ui.label(RichText::new(mark).size(theme.font_size_small).strong().color(mark_color));
                        if l.weakest {
                            ui.label(RichText::new("weakest loop").size(theme.font_size_small).color(theme.warning()));
                        }
                    });
                    widgets::detail_row(ui, theme, "  demand", &l.demand);
                    widgets::detail_row(ui, theme, "  supply", &l.supply);
                    ui.label(RichText::new(&l.note).size(theme.font_size_small).color(theme.text_secondary()));
                    ui.add_space(theme.spacing_xs);
                }
            });
            ui.add_space(theme.spacing_sm);
        }

        // ── Bill of materials ──
        egui::CollapsingHeader::new(
            RichText::new(format!("Bill of materials ({} kinds, {} total)", bom.len(), total_parts))
                .size(theme.font_size_body)
                .strong()
                .color(theme.text_primary()),
        )
        .id_salt("homes_bom")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                RichText::new("Everything to build it. Real-world part numbers and where to 3D-print / buy / trade each one are the planned next layer.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);
            for (id, qty) in &bom {
                widgets::detail_row(ui, theme, &humanize(id), &format!("x{}", qty));
            }
        });
        ui.add_space(theme.spacing_sm);

        // ── Rooms by tier ──
        ui.label(RichText::new("Rooms").size(theme.font_size_heading).color(theme.text_primary()));
        ui.add_space(theme.spacing_xs);
        for tier in &design.tiers {
            let tier_rooms: Vec<&DesignRoom> = rooms
                .iter()
                .filter(|r| r.tier.as_str() == tier.id.as_str())
                .copied()
                .collect();
            if tier_rooms.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(
                RichText::new(format!("{} ({})", tier.name, tier_rooms.len()))
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.accent()),
            )
            .id_salt(("homes_tier", tier.id.as_str()))
            .default_open(true)
            .show(ui, |ui| {
                if !tier.description.is_empty() {
                    ui.label(RichText::new(&tier.description).size(theme.font_size_small).color(theme.text_muted()));
                    ui.add_space(theme.spacing_xs);
                }
                for r in tier_rooms {
                    draw_room(ui, theme, r);
                }
            });
        }

        // ── Aeroponic towers (the food loop; v0.382) ──
        if !towers.is_empty() {
            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new("Aeroponic towers").size(theme.font_size_heading).color(theme.text_primary()));
            ui.label(
                RichText::new("Your homestead's food loop: two curated 50-slot vertical towers.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);
            for (i, tower) in towers.iter().enumerate() {
                draw_tower(ui, theme, tower, compat.get(i));
            }
        }
    });
}

/// One aeroponic tower, collapsible: purpose + what it covers / its gaps +
/// disclaimer + the 50-slot planting list (count, plant, role, note).
fn draw_tower(ui: &mut egui::Ui, theme: &Theme, tower: &TowerConfig, compat: Option<&TowerCompat>) {
    let planted: u32 = tower.plantings.iter().map(|p| p.slots).sum();
    egui::CollapsingHeader::new(
        RichText::new(format!("{}  ({}/{} slots)", tower.name, planted, tower.slots))
            .size(theme.font_size_body)
            .strong()
            .color(theme.accent()),
    )
    .id_salt(("tower", tower.id.as_str()))
    .show(ui, |ui| {
        if !tower.purpose.is_empty() {
            ui.label(RichText::new(&tower.purpose).size(theme.font_size_small).color(theme.text_secondary()));
        }
        if !tower.description.is_empty() {
            ui.label(RichText::new(&tower.description).size(theme.font_size_small).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_xs);
        if !tower.covers.is_empty() {
            ui.label(
                RichText::new(format!("Covers: {}", tower.covers.join(", ")))
                    .size(theme.font_size_small)
                    .color(theme.success()),
            );
        }
        if !tower.gaps.is_empty() {
            ui.label(
                RichText::new(format!("Gaps: {}", tower.gaps.join(", ")))
                    .size(theme.font_size_small)
                    .color(theme.warning()),
            );
            if !tower.gaps_note.is_empty() {
                ui.label(RichText::new(&tower.gaps_note).size(theme.font_size_small).color(theme.text_muted()));
            }
        }
        if !tower.disclaimer.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&tower.disclaimer).size(theme.font_size_small).color(theme.text_muted()).italics());
        }
        // ── Grow-together check: can these plants share one reservoir + air? ──
        // (operator: "make sure they'd all grow together too"). Aeroponics shares a
        // reservoir + air, not soil, so the constraint is a common pH/temp/humidity
        // window. Green = one shared window; warnings name the plants that conflict.
        if let Some(c) = compat {
            ui.add_space(theme.spacing_xs);
            let shared: Vec<String> = [
                c.ph.map(|(a, b)| format!("pH {:.1}-{:.1}", a, b)),
                c.temp.map(|(a, b)| format!("{:.0}-{:.0}°C", a, b)),
                c.humidity.map(|(a, b)| format!("humidity {:.0}-{:.0}%", a * 100.0, b * 100.0)),
            ]
            .into_iter()
            .flatten()
            .collect();
            if c.conflicts.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "✓ These {} plants share one reservoir: {}",
                        c.species,
                        shared.join(", ")
                    ))
                    .size(theme.font_size_small)
                    .color(theme.success()),
                );
            } else {
                if !shared.is_empty() {
                    ui.label(
                        RichText::new(format!("Shared where they can: {}", shared.join(", ")))
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                }
                for note in &c.conflicts {
                    ui.label(
                        RichText::new(format!("⚠ {}", note))
                            .size(theme.font_size_small)
                            .color(theme.warning()),
                    );
                }
                ui.label(
                    RichText::new(
                        "Split the flagged plants into a separate tower or climate zone.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
                );
            }
            // Self-sufficiency numbers: total daily water draw + the harvest window
            // (ties the tower into the homestead's water + food loops).
            let mut stats: Vec<String> = Vec::new();
            if c.water_per_day_total > 0.0 {
                stats.push(format!("Water draw ~{:.1} L/day", c.water_per_day_total));
            }
            if c.full_harvest_days > 0.0 {
                stats.push(format!(
                    "Harvest ~{:.0}-{:.0} days",
                    c.first_harvest_days, c.full_harvest_days
                ));
            }
            if !stats.is_empty() {
                ui.label(
                    RichText::new(stats.join("  ·  "))
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }
        }
        ui.add_space(theme.spacing_sm);
        for p in &tower.plantings {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("x{}", p.slots)).size(theme.font_size_small).strong().color(theme.accent()));
                ui.label(RichText::new(humanize(&p.plant)).size(theme.font_size_small).color(theme.text_primary()));
                if !p.role.is_empty() {
                    ui.label(RichText::new(format!("· {}", p.role)).size(theme.font_size_small).color(theme.text_muted()));
                }
            });
            if !p.note.is_empty() {
                ui.label(RichText::new(&p.note).size(theme.font_size_small).color(theme.text_secondary()));
            }
            ui.add_space(2.0);
        }
        // ── Parts list: what to actually BUILD the tower from (the game->real
        //    bridge / north star). Data-driven (the RON's `parts`); a starting
        //    bill of materials the operator + community refine. ──
        if !tower.parts.is_empty() {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Parts list (real-world build)")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new(
                    "What to 3D-print, buy, trade, or scavenge to build it. A starting list, refine for your setup.",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            ui.add_space(2.0);
            for part in &tower.parts {
                ui.horizontal(|ui| {
                    if !part.qty.is_empty() {
                        ui.label(RichText::new(&part.qty).size(theme.font_size_small).strong().color(theme.accent()));
                    }
                    ui.label(RichText::new(&part.name).size(theme.font_size_small).color(theme.text_primary()));
                    if !part.source.is_empty() {
                        ui.label(
                            RichText::new(format!("· {}", part_source_label(&part.source)))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    }
                });
                if !part.note.is_empty() {
                    ui.label(RichText::new(&part.note).size(theme.font_size_small).color(theme.text_secondary()));
                }
                ui.add_space(2.0);
            }
        }
    });
    ui.add_space(theme.spacing_xs);
}

/// Humanize a part `source` tag for display ("3d_print" -> "3D-print").
fn part_source_label(source: &str) -> &str {
    match source {
        "3d_print" => "3D-print",
        "diy" => "DIY",
        other => other,
    }
}

fn draw_room(ui: &mut egui::Ui, theme: &Theme, r: &DesignRoom) {
    egui::CollapsingHeader::new(
        RichText::new(format!("F{}  {}", r.fibonacci_index, r.name)).color(theme.text_primary()),
    )
    .id_salt(("homes_room", r.id.as_str()))
    .show(ui, |ui| {
        if !r.purpose.is_empty() {
            ui.label(RichText::new(&r.purpose).size(theme.font_size_small).color(theme.text_secondary()));
        }
        widgets::detail_row(ui, theme, "Footprint", &format!("{:.0} x {:.0} m", r.size.x, r.size.z));
        widgets::detail_row(ui, theme, "Power", &format!("{} W", r.requirements.power_watts));
        widgets::detail_row(ui, theme, "Water", &format!("{} L/day", r.requirements.water_liters_per_day));
        if !r.environment_notes.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&r.environment_notes).size(theme.font_size_small).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Parts").size(theme.font_size_small).color(theme.text_secondary()));
        for (id, qty) in &r.requirements.materials {
            widgets::detail_row(ui, theme, &humanize(id), &format!("x{}", qty));
        }
    });
}

/// Sum the generation / collection / recycling components in the BOM by group, so
/// the page can show, at a glance, the systems that close the homestead's loops.
fn self_sufficiency_kit(bom: &[(String, u32)]) -> Vec<(&'static str, u32)> {
    let groups: [(&str, &[&str]); 5] = [
        ("Solar panels", &["solar_panel"]),
        ("Wind turbines", &["wind_turbine"]),
        ("Generators + batteries", &["generator", "battery"]),
        ("Water (tank / pump / purifier / irrigation)", &["water_tank", "water_pump", "water_purifier", "irrigation"]),
        ("Food + recycling (greenhouse / composter)", &["greenhouse", "composter"]),
    ];
    let mut out = Vec::new();
    for (label, subs) in groups {
        let n: u32 = bom
            .iter()
            .filter(|(id, _)| subs.iter().any(|s| id.contains(*s)))
            .map(|(_, q)| *q)
            .sum();
        if n > 0 {
            out.push((label, n));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn tower_configs_parse_max_variety() {
        // Loads the real data/towers/aeroponic_configs.ron from the crate root.
        let towers = crate::gui::load_tower_configs(std::path::Path::new("data"));
        assert_eq!(towers.len(), 2, "expected 2 towers, got {}", towers.len());
        for t in &towers {
            let total: u32 = t.plantings.iter().map(|p| p.slots).sum();
            assert!(
                total <= t.slots,
                "tower '{}' plantings {} exceed capacity {}",
                t.id, total, t.slots
            );
            // Max variety: one of each type, no duplicate plants.
            let mut ids: Vec<&str> = t.plantings.iter().map(|p| p.plant.as_str()).collect();
            let n = ids.len();
            ids.sort();
            ids.dedup();
            assert_eq!(
                ids.len(), n,
                "tower '{}' has duplicate plants (max-variety expects distinct)",
                t.id
            );
            assert!(n >= 20, "tower '{}' should showcase variety (>=20 distinct), has {}", t.id, n);
            // Each tower carries a real-world parts list (the game->real bridge),
            // and every part names something with a source.
            assert!(
                t.parts.len() >= 5,
                "tower '{}' should have a starter parts list (>=5), has {}",
                t.id, t.parts.len()
            );
            for part in &t.parts {
                assert!(!part.name.is_empty(), "tower '{}' has a part with no name", t.id);
                assert!(!part.source.is_empty(), "part '{}' in '{}' has no source", part.name, t.id);
            }
        }
    }
}
