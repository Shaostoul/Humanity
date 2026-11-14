//! The Calculator page's Pulleys mode: the block-and-tackle designer (docs/design/pulleys.md,
//! rung 1, 2026-11-13). Say what you are lifting and who pulls; the configurator in
//! `humanity_systems::pulley` picks the system, sizes every part from the load, and this panel
//! says what it costs in words and draws it as a schematic from the same numbers.
//!
//! Why the Calculator page: the designer is a calculator for a real job (your real life, the two
//! realities rule: the app's pages are real life), it sits beside the arithmetic in the Tools
//! menu, and the website's Calculator already has modes for the web mirror to join. The game's
//! block-and-tackle item will open this same panel (the design doc, rung 3).

use egui::{Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use humanity_core::i18n::{t, t_dyn, t_marked, tf};
use humanity_systems::pulley::{self, Advice, Inputs, Material, Mode, Outcome, PulleyData, Refusal, StageDesign, Verdict, World};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::Hint;

/// The gravity used before the worlds are read, and when the chosen one is not among them: Earth's.
const EARTH_G: f64 = 9.81;

/// What the person chose, and the last answer (kept only while the reuse switch allows).
pub struct PulleyPanel {
    pub load_kg: f64,
    pub lift_m: f64,
    pub puller: String,
    pub rope: String,
    pub bearing: String,
    pub realistic: bool,
    /// A rope size the person fixed, mm; None sizes it for the load.
    pub rope_d_mm: Option<f64>,
    /// Where the lift happens (a `World`'s id: "earth", "moon", "ship_rim"): its gravity makes the
    /// load's weight (2026-11-13, the operator: "Sure.").
    pub world: String,
    /// The rows and the block material, read when the panel first opens.
    data: Option<(PulleyData, Option<Material>)>,
    /// The worlds to choose from (data/planets/ and the ship's spin), read with the rows.
    worlds: Vec<World>,
    /// The last answer and the key of the inputs it was worked out from.
    /// reuse: site: the Calculator's Pulleys mode keeps its last answer under a key of its inputs;
    /// with the switch off it is worked out again every time.
    kept: Option<(u64, Outcome)>,
}

impl Default for PulleyPanel {
    fn default() -> Self {
        PulleyPanel {
            load_kg: 1000.0,
            lift_m: 3.0,
            puller: "person".into(),
            rope: "manila".into(),
            bearing: "bronze_bushed".into(),
            realistic: true,
            rope_d_mm: None,
            world: "earth".into(),
            data: None,
            worlds: Vec::new(),
            kept: None,
        }
    }
}

impl PulleyPanel {
    fn mode(&self) -> Mode {
        if self.realistic {
            Mode::Realistic
        } else {
            Mode::Simplified
        }
    }

    fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.load_kg.to_bits(), self.lift_m.to_bits(), &self.puller, &self.rope, &self.bearing, self.realistic, &self.world).hash(&mut h);
        self.rope_d_mm.map(f64::to_bits).hash(&mut h);
        h.finish()
    }

    /// The chosen world's gravity, m/s^2 (Earth's until the worlds are read or when the choice is
    /// not among them).
    pub fn gravity(&self) -> f64 {
        self.worlds.iter().find(|w| w.id == self.world).map_or(EARTH_G, |w| w.gravity)
    }

    /// The answer for the current choices: worked out again whenever a choice changes, and every
    /// frame while the reuse switch is off (Dev > Performance).
    fn outcome(&mut self) -> Option<&Outcome> {
        if self.data.is_none() {
            let d = PulleyData::load();
            let m = pulley::data::block_material(&d.block);
            self.data = Some((d, m));
            self.worlds = pulley::data::worlds();
        }
        let key = self.key();
        let fresh = humanity_core::reuse::enabled() && self.kept.as_ref().is_some_and(|(k, _)| *k == key);
        if !fresh {
            let (d, m) = self.data.as_ref()?;
            let mat = (*m)?;
            let inp = Inputs {
                load_kg: self.load_kg,
                lift_m: self.lift_m,
                gravity: self.gravity(),
                puller: d.puller(&self.puller).or(d.pullers.first())?,
                rope: d.rope(&self.rope).or(d.ropes.first())?,
                bearing: d.bearing(&self.bearing).or(d.bearings.first())?,
                mode: self.mode(),
                rope_d_mm: self.rope_d_mm,
            };
            self.kept = Some((key, pulley::recommend(&inp, d, mat)));
        }
        self.kept.as_ref().map(|(_, o)| o)
    }
}

/// A force in words: newtons, kilonewtons or meganewtons by size.
fn force(n: f64) -> String {
    if n >= 1.0e6 {
        tf("pulley.force_mn", &[("v", &format!("{:.2}", n / 1.0e6))])
    } else if n >= 1.0e4 {
        tf("pulley.force_kn", &[("v", &format!("{:.1}", n / 1.0e3))])
    } else {
        tf("pulley.force_n", &[("v", &format!("{n:.0}"))])
    }
}

fn mass(kg: f64) -> String {
    if kg >= 10_000.0 {
        tf("pulley.mass_t", &[("v", &format!("{:.1}", kg / 1000.0))])
    } else {
        tf("pulley.mass_kg", &[("v", &format!("{kg:.0}"))])
    }
}

fn length(m: f64) -> String {
    if m >= 100.0 {
        tf("pulley.length_m", &[("v", &format!("{m:.0}"))])
    } else {
        tf("pulley.length_m", &[("v", &format!("{m:.1}"))])
    }
}

fn mm(m: f64) -> String {
    tf("pulley.length_mm", &[("v", &format!("{:.0}", m * 1000.0))])
}

fn duration(s: f64) -> String {
    if s >= 7200.0 {
        tf("pulley.time_h", &[("v", &format!("{:.1}", s / 3600.0))])
    } else if s >= 120.0 {
        tf("pulley.time_min", &[("v", &format!("{:.0}", s / 60.0))])
    } else {
        tf("pulley.time_s", &[("v", &format!("{s:.0}"))])
    }
}

/// The Pulleys mode, drawn into the Calculator page's panel.
pub fn draw(ui: &mut Ui, theme: &Theme, p: &mut PulleyPanel) {
    ui.label(RichText::new(t("pulley.intro")).size(theme.font_size_body).color(theme.text_secondary()));
    ui.add_space(theme.spacing_sm);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(340.0);
            inputs(ui, theme, p);
        });
        ui.add_space(theme.spacing_md);
        ui.vertical(|ui| {
            let Some(out) = p.outcome().cloned() else {
                ui.label(RichText::new(t("pulley.no_data")).color(theme.warning()));
                return;
            };
            let puller_key = p.data.as_ref().and_then(|(d, _)| d.puller(&p.puller)).map(|r| r.name_key.clone()).unwrap_or_default();
            results(ui, theme, &out, &puller_key, p.load_kg);
            ui.add_space(theme.spacing_sm);
            widgets::card_with_header(ui, theme, t("pulley.schematic"), |ui| {
                schematic(ui, theme, &out, p.load_kg, &puller_key);
            });
        });
    });
}

fn inputs(ui: &mut Ui, theme: &Theme, p: &mut PulleyPanel) {
    let Some((d, _)) = p.data.as_ref() else {
        // The rows load with the first answer; ask for it so the lists below have rows.
        let _ = p.outcome();
        return;
    };
    let pullers: Vec<(String, String)> = d.pullers.iter().map(|r| (r.id.clone(), r.name_key.clone())).collect();
    let ropes: Vec<(String, String, Vec<f64>)> = d.ropes.iter().map(|r| (r.id.clone(), r.name_key.clone(), r.diameters_mm.clone())).collect();
    let bearings: Vec<(String, String)> = d.bearings.iter().map(|r| (r.id.clone(), r.name_key.clone())).collect();
    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, t("pulley.load"), |ui| {
            let speed = (p.load_kg * 0.01).max(0.1);
            ui.add(egui::DragValue::new(&mut p.load_kg).range(0.1..=1.0e9).speed(speed).max_decimals(1)).hint("pulley.load_hint");
            ui.label(RichText::new(t("pulley.kg")).color(theme.text_muted()));
        });
        // Where the lift happens, beside the load: its gravity makes the load's weight.
        widgets::form_row(ui, theme, t("pulley.world"), |ui| {
            let words = |w: &World| {
                let name = w.name_key.as_deref().map_or_else(|| w.name.clone(), |k| t_dyn(k).to_string());
                tf("pulley.world_choice", &[("world", &name), ("pct", &format!("{:.0}", w.gravity / EARTH_G * 100.0))])
            };
            let shown = p.worlds.iter().find(|w| w.id == p.world).map(words).unwrap_or_default();
            egui::ComboBox::from_id_salt("pulley_world")
                .selected_text(shown)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for w in &p.worlds {
                        ui.selectable_value(&mut p.world, w.id.clone(), words(w)).hint("pulley.world_hint");
                    }
                })
                .response
                .hint("pulley.world_hint");
        });
        widgets::form_row(ui, theme, t("pulley.lift"), |ui| {
            ui.add(egui::DragValue::new(&mut p.lift_m).range(0.1..=10_000.0).speed(0.05).max_decimals(2)).hint("pulley.lift_hint");
            ui.label(RichText::new(t("pulley.m")).color(theme.text_muted()));
        });
        widgets::form_row(ui, theme, t("pulley.puller"), |ui| {
            let shown = pullers.iter().find(|(id, _)| *id == p.puller).map(|(_, k)| t_dyn(k)).unwrap_or("");
            egui::ComboBox::from_id_salt("pulley_puller")
                .selected_text(shown)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (id, key) in &pullers {
                        ui.selectable_value(&mut p.puller, id.clone(), t_dyn(key)).hint("pulley.puller_hint");
                    }
                })
                .response
                .hint("pulley.puller_hint");
        });
        widgets::form_row(ui, theme, t("pulley.rope"), |ui| {
            let shown = ropes.iter().find(|(id, _, _)| *id == p.rope).map(|(_, k, _)| t_dyn(k)).unwrap_or("");
            let before = p.rope.clone();
            egui::ComboBox::from_id_salt("pulley_rope")
                .selected_text(shown)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (id, key, _) in &ropes {
                        ui.selectable_value(&mut p.rope, id.clone(), t_dyn(key)).hint("pulley.rope_hint");
                    }
                })
                .response
                .hint("pulley.rope_hint");
            if p.rope != before {
                p.rope_d_mm = None;
            }
        });
        widgets::form_row(ui, theme, t("pulley.rope_size"), |ui| {
            let sizes = ropes.iter().find(|(id, _, _)| *id == p.rope).map(|(_, _, s)| s.clone()).unwrap_or_default();
            let shown = match p.rope_d_mm {
                Some(v) => tf("pulley.length_mm", &[("v", &format!("{v}"))]),
                None => t("pulley.rope_size_auto").to_string(),
            };
            egui::ComboBox::from_id_salt("pulley_rope_size")
                .selected_text(shown)
                .width(220.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut p.rope_d_mm, None, t("pulley.rope_size_auto")).hint("pulley.rope_size_hint");
                    for v in sizes {
                        ui.selectable_value(&mut p.rope_d_mm, Some(v), tf("pulley.length_mm", &[("v", &format!("{v}"))]))
                            .hint("pulley.rope_size_hint");
                    }
                })
                .response
                .hint("pulley.rope_size_hint");
        });
        widgets::form_row(ui, theme, t("pulley.bearing"), |ui| {
            let shown = bearings.iter().find(|(id, _)| *id == p.bearing).map(|(_, k)| t_dyn(k)).unwrap_or("");
            egui::ComboBox::from_id_salt("pulley_bearing")
                .selected_text(shown)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (id, key) in &bearings {
                        ui.selectable_value(&mut p.bearing, id.clone(), t_dyn(key)).hint("pulley.bearing_hint");
                    }
                })
                .response
                .hint("pulley.bearing_hint");
        });
        widgets::form_row(ui, theme, t("pulley.mode"), |ui| {
            ui.selectable_value(&mut p.realistic, true, t("pulley.mode_realistic")).hint("pulley.mode_realistic_hint");
            ui.selectable_value(&mut p.realistic, false, t("pulley.mode_simplified")).hint("pulley.mode_simplified_hint");
        });
    });
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new(t("pulley.sources")).size(theme.font_size_small).color(theme.text_muted()));
}

/// One labelled figure in the results card.
fn figure(ui: &mut Ui, theme: &Theme, label: &str, value: &str, hint: &'static str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(theme.text_muted())).hint(hint);
        ui.label(RichText::new(value).color(theme.text_primary()).strong()).hint(hint);
    });
}

/// A figure whose label carries a glossary mark (its term opens its card).
fn figure_marked(ui: &mut Ui, theme: &Theme, marked: &str, value: &str, hint: &'static str) {
    ui.horizontal(|ui| {
        widgets::glossary_words(ui, theme, marked, theme.font_size_body, theme.text_muted()).hint(hint);
        ui.label(RichText::new(value).color(theme.text_primary()).strong()).hint(hint);
    });
}

fn plan_words(out: &Outcome) -> String {
    out.stages.iter().map(|s| s.parts.to_string()).collect::<Vec<_>>().join(" x ")
}

fn results(ui: &mut Ui, theme: &Theme, out: &Outcome, puller_key: &str, load_kg: f64) {
    widgets::card_with_header(ui, theme, t("pulley.result"), |ui| {
        let headline = match (out.verdict, out.stages.len()) {
            (Verdict::Direct, _) => tf("pulley.system_direct", &[("puller", &t_dyn(puller_key))]),
            (_, 1) => {
                let s = &out.stages[0];
                tf("pulley.system_simple", &[("parts", &s.parts), ("fixed", &s.fixed_sheaves), ("moving", &s.moving_sheaves)])
            }
            (_, n) => tf("pulley.system_compound", &[("stages", &n), ("plan", &plan_words(out))]),
        };
        ui.label(RichText::new(headline).size(theme.font_size_heading).color(theme.text_primary()));
        if out.verdict == Verdict::BeyondTackle && out.refusal.is_none() {
            ui.label(RichText::new(t("pulley.beyond_simplified")).color(theme.warning()));
        }
        match &out.refusal {
            Some(Refusal::RopeOverLimit { stage, rope_d_mm, wll_n, tension_n }) => {
                let words = tf(
                    "pulley.refused_rope",
                    &[("stage", &(stage + 1)), ("d", &format!("{rope_d_mm}")), ("wll", &force(*wll_n)), ("tension", &force(*tension_n))],
                );
                ui.label(RichText::new(words).color(theme.danger()));
            }
            Some(Refusal::NotEnoughAdvantage { best_real_ma, needed_ma }) => {
                let words = tf("pulley.refused_advantage", &[("best", &format!("{best_real_ma:.1}")), ("need", &format!("{needed_ma:.1}"))]);
                ui.label(RichText::new(words).color(theme.danger()));
            }
            None => {}
        }
        ui.add_space(theme.spacing_xs);
        for (i, s) in out.stages.iter().enumerate() {
            ui.label(RichText::new(stage_words(i, s)).size(theme.font_size_small).color(theme.text_secondary()));
        }
        ui.add_space(theme.spacing_xs);
        let adv = tf(
            "pulley.advantage_value",
            &[("real", &format!("{:.1}", out.real_ma)), ("ideal", &format!("{:.0}", out.ideal_ma)), ("eff", &format!("{:.0}", out.efficiency * 100.0))],
        );
        figure_marked(ui, theme, t_marked("pulley.advantage"), &adv, "pulley.advantage_hint");
        let pull = tf("pulley.pull_value", &[("force", &force(out.pull_n)), ("kg", &format!("{:.0}", out.pull_n / EARTH_G))]);
        figure(ui, theme, t("pulley.pull"), &pull, "pulley.pull_hint");
        figure(ui, theme, t("pulley.rope_pulled"), &length(out.rope_pulled_m), "pulley.rope_pulled_hint");
        figure(ui, theme, t("pulley.time"), &duration(out.time_s), "pulley.time_hint");
        let rope = tf("pulley.rope_total_value", &[("len", &length(out.total_rope_m)), ("mass", &mass(out.rope_mass_kg))]);
        figure(ui, theme, t("pulley.rope_total"), &rope, "pulley.rope_total_hint");
        figure(ui, theme, t("pulley.blocks_mass"), &mass(out.blocks_mass_kg), "pulley.blocks_mass_hint");
        ui.add_space(theme.spacing_xs);
        let loss = ((1.0 - out.efficiency) * 100.0).max(0.0);
        let trade = tf(
            "pulley.trade",
            &[("ideal", &format!("{:.0}", out.ideal_ma)), ("loss", &format!("{loss:.0}")), ("load", &mass(load_kg))],
        );
        ui.label(RichText::new(trade).color(theme.text_primary()));
        for a in &out.advice {
            let words = match a {
                Advice::GearedHoist { hoist_key, hoist_efficiency, tackle_efficiency } => tf(
                    "pulley.advice_hoist",
                    &[
                        ("hoist", &t_dyn(hoist_key)),
                        ("h", &format!("{:.0}", hoist_efficiency * 100.0)),
                        ("t", &format!("{:.0}", tackle_efficiency * 100.0)),
                    ],
                ),
                Advice::MoreHands { minutes } => tf("pulley.advice_more_hands", &[("min", &format!("{minutes:.0}"))]),
                Advice::FirstLayer => t("pulley.advice_first_layer").to_string(),
                Advice::Strokes { strokes } => tf("pulley.advice_strokes", &[("n", strokes)]),
            };
            ui.label(RichText::new(words).size(theme.font_size_small).color(theme.info()));
        }
    });
}

fn stage_words(i: usize, s: &StageDesign) -> String {
    tf(
        "pulley.stage_line",
        &[
            ("n", &(i + 1)),
            ("parts", &s.parts),
            ("fixed", &s.fixed_sheaves),
            ("moving", &s.moving_sheaves),
            ("rope", &mm(s.rope_d_mm / 1000.0)),
            ("sheave", &mm(s.upper.tread_d)),
            ("len", &mm(s.upper.cheek_h)),
            ("travel", &length(s.travel_m)),
        ],
    )
}

/// The system drawn from the same numbers: the beam and the ground, every block with its
/// sheaves, every part of rope, the load and the pull. Even-numbered stages hang from the beam;
/// odd ones are anchored to the ground, because each stage pulls the one before's hauling part
/// away from its block (the physics, not a drawing choice).
fn schematic(ui: &mut Ui, theme: &Theme, out: &Outcome, load_kg: f64, puller_key: &str) {
    let w = ui.available_width().clamp(320.0, 720.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 360.0), Sense::hover());
    resp.hint("pulley.schematic_hint");
    let painter = ui.painter_at(rect);
    let line = Stroke::new(1.5_f32, theme.text_secondary());
    let rope = Stroke::new(1.5_f32, theme.accent());
    let edge = Stroke::new(1.0_f32, theme.border());
    let top = rect.top() + 14.0;
    let ground = rect.bottom() - 14.0;
    // The beam and the ground.
    painter.line_segment([Pos2::new(rect.left() + 8.0, top), Pos2::new(rect.right() - 8.0, top)], Stroke::new(3.0_f32, theme.text_muted()));
    painter.line_segment([Pos2::new(rect.left() + 8.0, ground), Pos2::new(rect.right() - 8.0, ground)], Stroke::new(3.0_f32, theme.text_muted()));
    let n_st = out.stages.len().max(1);
    let col_w = (rect.width() - 16.0) / n_st as f32;
    let small = egui::FontId::proportional(theme.font_size_small);
    // Where the previous stage's hauling part ends, to hook the next stage's moving block to.
    let mut prev_haul: Option<Pos2> = None;
    // A stage's sheave spacing and where its moving block's hook is: even stages hang from the beam
    // (moving block low, hook below it), odd ones stand on the ground (moving block high, hook above).
    let gap_of = |n: usize| (col_w / (n as f32 + 3.0)).clamp(6.0, 22.0);
    let hook_y_of = |j: usize| {
        let r = 0.5 * gap_of(out.stages[j].parts.max(1) as usize);
        if j % 2 == 0 { ground - 120.0 + r + 10.0 } else { top + 120.0 - r - 10.0 }
    };
    for (i, s) in out.stages.iter().enumerate() {
        let hanging = i % 2 == 0;
        let x0 = rect.left() + 8.0 + col_w * i as f32;
        let n = s.parts.max(1) as usize;
        let gap = gap_of(n);
        let r = gap * 0.5;
        let x_part = |k: usize| x0 + col_w * 0.5 + (k as f32 - n as f32 * 0.5) * gap;
        let (fixed_y, moving_y) = if hanging { (top + 34.0, ground - 120.0) } else { (ground - 34.0, top + 120.0) };
        let dir: f32 = if hanging { 1.0 } else { -1.0 };
        // Parts of rope: part k runs between the blocks at x_part(k).
        for k in 1..=n {
            painter.line_segment([Pos2::new(x_part(k), fixed_y), Pos2::new(x_part(k), moving_y)], rope);
        }
        // The hauling part: from the fixed block to the next stage's hook, or to the puller.
        let haul_y = if i + 1 < out.stages.len() { hook_y_of(i + 1) } else if hanging { ground - 40.0 } else { top + 40.0 };
        let haul_end = Pos2::new(x_part(0), haul_y);
        painter.line_segment([Pos2::new(x_part(0), fixed_y), haul_end], rope);
        // Sheaves: fixed sheave j between parts 2j and 2j+1 at the fixed block; moving sheave j
        // between parts 2j+1 and 2j+2 at the moving block.
        let mut fixed_box = Rect::NOTHING;
        let mut moving_box = Rect::NOTHING;
        for k in 0..n {
            let cx = 0.5 * (x_part(k) + x_part(k + 1));
            let at_fixed = k % 2 == 0;
            let cy = if at_fixed { fixed_y } else { moving_y };
            let c = Pos2::new(cx, cy);
            painter.circle_stroke(c, r, Stroke::new(1.5_f32, theme.text_primary()));
            let b = Rect::from_center_size(c, Vec2::splat(2.0 * r + 6.0));
            if at_fixed {
                fixed_box = fixed_box.union(b);
            } else {
                moving_box = moving_box.union(b);
            }
        }
        // The dead end is made fast to a block: the moving one after an odd number of parts, the
        // fixed one after an even number, so that block reaches over the last part.
        let dead = Rect::from_center_size(Pos2::new(x_part(n), if n % 2 == 1 { moving_y } else { fixed_y }), Vec2::splat(6.0));
        if n % 2 == 1 && moving_box.is_positive() {
            moving_box = moving_box.union(dead);
        } else if n % 2 == 0 {
            fixed_box = fixed_box.union(dead);
        }
        if fixed_box.is_positive() {
            painter.rect_stroke(fixed_box, 3.0, edge, egui::StrokeKind::Outside);
            // The fixed block hangs from (or stands on) its anchor.
            let anchor_y = if hanging { top } else { ground };
            let edge_y = if hanging { fixed_box.top() } else { fixed_box.bottom() };
            painter.line_segment([Pos2::new(fixed_box.center().x, anchor_y), Pos2::new(fixed_box.center().x, edge_y)], line);
        }
        // The moving block (a plain hook when the stage has one part).
        if moving_box.is_positive() {
            painter.rect_stroke(moving_box, 3.0, edge, egui::StrokeKind::Outside);
        } else {
            painter.circle_filled(Pos2::new(x_part(n), moving_y), 3.0, theme.text_primary());
        }
        let hook_x = if moving_box.is_positive() { moving_box.center().x } else { x_part(n) };
        let hook_y = moving_y + dir * (r + 10.0);
        painter.line_segment([Pos2::new(hook_x, moving_y + dir * r), Pos2::new(hook_x, hook_y)], line);
        if i == 0 {
            // The load hangs from the first stage's moving block.
            let load = Rect::from_center_size(Pos2::new(hook_x, hook_y + 18.0), Vec2::new(70.0, 30.0));
            painter.rect_filled(load, 3.0, theme.bg_card());
            painter.rect_stroke(load, 3.0, Stroke::new(1.5_f32, theme.warning()), egui::StrokeKind::Outside);
            painter.text(load.center(), egui::Align2::CENTER_CENTER, mass(load_kg), small.clone(), theme.text_primary());
        } else if let Some(prev) = prev_haul {
            // This stage's moving block takes the previous stage's hauling part.
            painter.line_segment([prev, Pos2::new(hook_x, hook_y)], rope);
        }
        painter.text(
            Pos2::new(x_part(n) + r + 6.0, 0.5 * (fixed_y + moving_y)),
            egui::Align2::LEFT_CENTER,
            tf("pulley.parts_label", &[("n", &s.parts)]),
            small.clone(),
            theme.text_muted(),
        );
        prev_haul = Some(haul_end);
    }
    // The pull, at the last hauling part's end.
    if let Some(end) = prev_haul {
        let up = out.stages.len() % 2 == 0;
        let tip = Pos2::new(end.x, end.y + if up { -18.0 } else { 18.0 });
        painter.arrow(end, tip - end, Stroke::new(2.0_f32, theme.success()));
        let label = tf("pulley.pull_label", &[("who", &t_dyn(puller_key)), ("force", &force(out.pull_n))]);
        let align = if end.x > rect.center().x { egui::Align2::RIGHT_CENTER } else { egui::Align2::LEFT_CENTER };
        let at = Pos2::new(end.x + if end.x > rect.center().x { -8.0 } else { 8.0 }, tip.y);
        painter.text(at, align, label, small, theme.text_primary());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE SAME LOAD ON THE MOON NEEDS ABOUT A SIXTH OF THE PULL (2026-11-13; docs/design/pulleys.md,
    /// "His answers": "Sure."): the world chosen beside the load sets the gravity it weighs by, read
    /// from data/planets/ (the Moon's 1.62 m/s^2 against Earth's 9.81) and the ship's spin at its rim.
    /// Seen red 2026-11-13 with the panel's gravity fixed at Earth's (`EARTH_G` in `outcome`, as
    /// before the choice): "on the Moon the pull is 1.000 of Earth's (204.8 N against 204.8 N), not
    /// about a sixth".
    #[test]
    fn the_same_load_on_the_moon_needs_about_a_sixth_of_the_pull() {
        // 20 kg: a person lifts it straight up on either world, so the pull is the load's weight.
        let mut p = PulleyPanel { load_kg: 20.0, ..PulleyPanel::default() };
        let earth = p.outcome().expect("an answer on Earth").pull_n;
        p.world = "moon".into();
        let moon = p.outcome().expect("an answer on the Moon").pull_n;
        let ratio = moon / earth;
        assert!((0.155..=0.18).contains(&ratio), "on the Moon the pull is {ratio:.3} of Earth's ({moon:.1} N against {earth:.1} N), not about a sixth");
        let ids: Vec<&str> = p.worlds.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids.first(), Some(&"earth"), "Earth comes first: {ids:?}");
        assert!(ids.contains(&"mars") && ids.contains(&"ship_rim"), "the planets and the ship's rim are offered: {ids:?}");
        p.world = "ship_rim".into();
        assert!((9.6..=9.9).contains(&p.gravity()), "the ship's rim spins for about 1 g: {}", p.gravity());
    }
}
