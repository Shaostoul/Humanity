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
use crate::gui::{GuiState, HomesteadDesign, DesignRoom};
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
            draw_design(ui, theme, &design);
        });
}

fn draw_design(ui: &mut egui::Ui, theme: &Theme, design: &HomesteadDesign) {
    ui.label(RichText::new("Your Home").size(theme.font_size_title).color(theme.text_primary()));
    ui.label(RichText::new(&design.name).size(theme.font_size_heading).color(theme.accent()));
    ui.label(RichText::new(&design.description).size(theme.font_size_small).color(theme.text_muted()));
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
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new("To be fully self-sustaining, this generation must meet the demand above. Exact closure scoring (output vs demand, per loop) is the next data layer.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        });
        ui.add_space(theme.spacing_sm);

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
    });
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
