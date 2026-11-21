//! The skill sheet on the Profile page's Skills section (skills rung 1, 2026-11-14,
//! docs/design/skills.md sections 3 and 4).
//!
//! Every skill of the base set, grouped by family (data/skills/families.json's order), each with
//! its named stage in words ("Baking: Novice, level 2"), what the person can do at the stages they
//! have reached ("I can ..."), what the next stage asks, and a bar from this stage to the next. The
//! numbers stay underneath; the words lead (the ELI5 mandate). Every word is a key of
//! data/i18n/en.json; the rows come from `GuiState::skills`, which src/engine/skills_mirror.rs fills
//! from the SkillRegistry on the frames that draw them. The web mirror is the Skills tab of
//! web/pages/profile.html.

use egui::RichText;
use humanity_core::i18n::{t, t_dyn, tf};

use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{GuiSkill, GuiState};

impl GuiSkill {
    /// The row for skill `def` at `level` with `xp` toward the next level: the one way the
    /// frame mirror and the page snapshot build it, so the snapshot shows what the game shows.
    pub fn from_def(def: &humanity_systems::skills::SkillDef, reg: &humanity_systems::skills::SkillRegistry, level: u32, xp: u32) -> Self {
        let reached = def.stage_index(level);
        let next = def.next_stage(level);
        Self {
            id: def.id.clone(),
            name: def.name(),
            family: def.family.clone(),
            family_order: reg.families.iter().position(|f| *f == def.family).unwrap_or(usize::MAX),
            level,
            xp,
            xp_needed: def.xp_for_level(level + 1),
            mastery_level: def.mastery_level(),
            stage: def.stage_at(level).map_or_else(|| "untrained".to_string(), |s| s.stage.clone()),
            next_stage: next.map(|s| s.stage.clone()),
            next_from_level: next.map_or(0, |s| s.from_level),
            stage_progress: def.stage_progress(level, xp),
            can_done: def.ladder.iter().take(reached).flat_map(|s| s.can.iter().cloned()).collect(),
            can_next: next.map(|s| s.can.clone()).unwrap_or_default(),
            ..Default::default()
        }
    }

    /// Practice quality in words (skills rung 2): why the last practice gave what it gave, the
    /// guide for the stage reached and what reading it does now, and a teacher near now. The one
    /// way the engine (src/engine/practice_view.rs) and the page snapshot fill it.
    pub fn fill_practice(&mut self, p: &PracticeFacts) {
        let rules = p.quality.rules(p.mode);
        self.why = p.memory.last.get(&self.id).map(why_lines).unwrap_or_default();
        self.guide = p.quality.guide_for(&self.id, &self.stage).map(|slug| {
            let title = (p.title_of)(slug).unwrap_or_else(|| slug.to_string());
            let status = match (&rules.guide, p.memory.guide_bonus(&self.id, &self.stage, p.now)) {
                (Some(g), Some(read)) => tf(
                    "skills_page.guide_running",
                    &[("times", &times(g.multiplier)), ("hours", &(((read.until - p.now) / 3600.0).ceil().max(1.0) as u32))],
                ),
                _ if p.memory.guide_read(&self.id, &self.stage) => t("skills_page.guide_used").to_string(),
                _ => String::new(),
            };
            (slug.to_string(), title, status)
        });
        let my_stage = p.stage_ids.iter().position(|s| *s == self.stage).unwrap_or(0);
        self.beside = rules.mentor.as_ref().and_then(|m| {
            p.helpers
                .iter()
                .filter(|h| h.distance_m <= m.reach_m)
                .filter_map(|h| {
                    let stage = h.stages.get(&self.id)?;
                    let idx = p.stage_ids.iter().position(|s| s == stage)?;
                    (idx >= my_stage + m.stages_above).then_some((h, stage))
                })
                .next()
                .map(|(h, stage)| tf("skills_page.beside_now", &[("name", &h.name), ("stage", &stage_name(stage)), ("times", &times(m.multiplier))]))
        });
    }
}

/// What practice quality knows this frame, for the words (skills rung 2).
pub struct PracticeFacts<'a> {
    pub quality: &'a humanity_systems::skills::quality::PracticeQuality,
    pub mode: humanity_systems::skills::quality::LearningMode,
    pub memory: &'a humanity_systems::skills::quality::PracticeMemory,
    pub helpers: &'a [humanity_systems::skills::quality::Helper],
    pub stage_ids: &'a [String],
    /// Game seconds now.
    pub now: f64,
    /// A Library slug's title (the Library's index).
    pub title_of: &'a dyn Fn(&str) -> Option<String>,
}

/// A multiplier in words: 1.5 is "1.5", 2.0 is "2".
fn times(x: f32) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A share in words as a whole percent: 0.85 is "85".
fn percent(x: f32) -> String {
    format!("{}", (x * 100.0).round() as i32)
}

/// Why the last practice gave what it gave, one line per thing that changed it, the total first
/// ("Last practice: 3 practice points (5 for the task itself)", "You did this same task 6 times
/// today: it is worth 32% now ...").
pub fn why_lines(why: &humanity_systems::skills::quality::PracticeWhy) -> Vec<String> {
    use humanity_systems::skills::quality::LearningMode;
    let mut v = vec![tf("skills_page.last_practice", &[("gained", &why.gained), ("base", &why.base)])];
    if why.mode == LearningMode::Off {
        v.push(t("skills_page.why_off").to_string());
    }
    if why.first_time > 1.0 {
        v.push(tf("skills_page.why_first_time", &[("times", &times(why.first_time))]));
    }
    if why.attempt > 0 {
        if why.variety < 0.999 {
            v.push(tf("skills_page.why_repeat", &[("attempt", &why.attempt), ("percent", &percent(why.variety))]));
        } else {
            v.push(tf("skills_page.why_full", &[("attempt", &why.attempt), ("full", &why.full_attempts)]));
        }
    }
    if let Some((stage, _, m)) = &why.guide {
        v.push(tf("skills_page.why_guide", &[("stage", &stage_name(stage)), ("times", &times(*m))]));
    }
    if let Some((name, stage, m)) = &why.mentor {
        v.push(tf("skills_page.why_mentor", &[("name", name), ("stage", &stage_name(stage)), ("times", &times(*m))]));
    }
    let key = match why.difficulty_gap {
        Some(g) if g > 2 => Some("skills_page.why_far"),
        Some(g) if g >= 0 => Some("skills_page.why_right"),
        Some(g) if g < -1 => Some("skills_page.why_easy"),
        _ => None,
    };
    if let Some(key) = key {
        v.push(tf(key, &[("percent", &percent(why.difficulty))]));
    }
    if (why.quality - 1.0).abs() > 0.005 {
        v.push(tf("skills_page.why_quality", &[("percent", &percent(why.quality))]));
    }
    if why.transfer > 1.005 {
        v.push(tf("skills_page.why_transfer", &[("percent", &percent(why.transfer))]));
    }
    v
}

/// "How practice counts" at the chosen level of skill learning, with the numbers from
/// data/skills/practice_quality.json.
pub fn rule_lines(rules: &humanity_systems::skills::quality::LevelRules) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(r) = &rules.variety {
        v.push(tf(
            "skills_page.how_variety",
            &[("full", &r.full_attempts), ("percent", &percent(r.per_repeat)), ("floor", &percent(r.floor)), ("first", &times(r.first_time))],
        ));
    }
    if let Some(g) = &rules.guide {
        v.push(tf("skills_page.how_guide", &[("times", &times(g.multiplier)), ("hours", &times(g.hours as f32))]));
    }
    if let Some(m) = &rules.mentor {
        v.push(tf("skills_page.how_mentor", &[("reach", &times(m.reach_m)), ("above", &m.stages_above), ("times", &times(m.multiplier))]));
    }
    if v.is_empty() {
        v.push(t("skills_page.how_off").to_string());
    }
    v
}

/// The stage's name in the person's language ("Novice").
fn stage_name(stage: &str) -> &'static str {
    t_dyn(&format!("skill_stages.{stage}.name"))
}

/// Draw the whole sheet. Empty (not in the world yet): one line saying how skills are earned.
/// Returns the Library guide to open when its button was pressed.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) -> Option<String> {
    ui.label(RichText::new(t("skills_page.intro")).size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_sm);
    if !state.practice.rules.is_empty() {
        let header = egui::CollapsingHeader::new(RichText::new(t("skills_page.how_title")).strong().color(theme.text_primary()))
            .id_salt("skills_how_practice_counts")
            .default_open(false)
            .show(ui, |ui| {
                for line in &state.practice.rules {
                    ui.label(RichText::new(line).size(theme.font_size_small).color(theme.text_secondary()));
                }
            });
        header.header_response.hint("skills_page.how_hint");
        ui.add_space(theme.spacing_sm);
    }
    if state.skills.is_empty() {
        ui.label(RichText::new(t("skills_page.enter_world")).size(theme.font_size_body).color(theme.text_muted()));
        return None;
    }
    // GuiState::skills is sorted by family order, then name: each run of one family is a group.
    let mut open = None;
    let mut start = 0;
    while start < state.skills.len() {
        let family = state.skills[start].family.clone();
        let end = state.skills[start..].iter().position(|s| s.family != family).map_or(state.skills.len(), |n| start + n);
        if let Some(slug) = draw_family(ui, theme, &family, &state.skills[start..end]) {
            open = Some(slug);
        }
        start = end;
    }
    open
}

/// One family: a header naming it and how many of its skills the person has practised, open when
/// any has been.
fn draw_family(ui: &mut egui::Ui, theme: &Theme, family: &str, skills: &[GuiSkill]) -> Option<String> {
    let mut open = None;
    let practised = skills.iter().filter(|s| s.level > 0 || s.xp > 0).count();
    let title = tf(
        "skills_page.family_row",
        &[("family", &t_dyn(&format!("skill_families.{family}.name"))), ("practised", &practised), ("total", &skills.len())],
    );
    let header = egui::CollapsingHeader::new(RichText::new(title).strong().color(theme.text_primary()))
        .id_salt(("skill_family", family))
        .default_open(practised > 0)
        .show(ui, |ui| {
            ui.label(
                RichText::new(t_dyn(&format!("skill_families.{family}.desc")))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            for sk in skills {
                if let Some(slug) = draw_skill(ui, theme, sk) {
                    open = Some(slug);
                }
            }
        });
    header.header_response.hint("skills_page.family_hint");
    open
}

/// One skill: its stage in words, then (opened) its bar, why its last practice gave what it gave,
/// the guide for its stage, its "I can" list and the next stage's.
fn draw_skill(ui: &mut egui::Ui, theme: &Theme, sk: &GuiSkill) -> Option<String> {
    let mut open = None;
    let row = tf("skills_page.skill_row", &[("skill", &sk.name), ("stage", &stage_name(&sk.stage)), ("level", &sk.level)]);
    let color = if sk.level > 0 { theme.text_primary() } else { theme.text_secondary() };
    let header = egui::CollapsingHeader::new(RichText::new(row).size(theme.font_size_body).color(color))
        .id_salt(("skill", &sk.id))
        .default_open(sk.level > 0)
        .show(ui, |ui| {
            ui.label(RichText::new(t_dyn(&format!("skills.{}.desc", sk.id))).size(theme.font_size_small).color(theme.text_muted()));
            ui.add_space(theme.spacing_xs);
            let bar_text = match &sk.next_stage {
                Some(next) => tf("skills_page.next_stage", &[("stage", &stage_name(next)), ("level", &sk.next_from_level)]),
                None => t("skills_page.at_master").to_string(),
            };
            ui.add(
                egui::ProgressBar::new(sk.stage_progress.clamp(0.0, 1.0))
                    .desired_width(320.0)
                    .text(RichText::new(bar_text).size(theme.font_size_small)),
            )
            .hint("skills_page.progress_hint");
            ui.label(
                RichText::new(tf("skills_page.xp_row", &[("xp", &sk.xp), ("needed", &sk.xp_needed), ("level", &(sk.level + 1))]))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            if let Some(beside) = &sk.beside {
                ui.label(RichText::new(beside).size(theme.font_size_small).color(theme.accent()));
            }
            if let Some((first, rest)) = sk.why.split_first() {
                ui.add_space(theme.spacing_xs);
                ui.label(RichText::new(first).size(theme.font_size_small).color(theme.text_secondary())).hint("skills_page.last_practice_hint");
                for line in rest {
                    statement_text(ui, theme, line, theme.text_muted());
                }
            }
            if let Some((slug, title, status)) = &sk.guide {
                ui.add_space(theme.spacing_xs);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(tf("skills_page.guide_for_stage", &[("title", title)])).size(theme.font_size_small).color(theme.text_secondary()));
                    if ui.small_button(t("skills_page.guide_open")).hint("skills_page.guide_open_hint").clicked() {
                        open = Some(slug.clone());
                    }
                });
                if !status.is_empty() {
                    statement_text(ui, theme, status, theme.text_muted());
                }
            }
            ui.add_space(theme.spacing_xs);
            if sk.can_done.is_empty() {
                ui.label(RichText::new(t("skills_page.untrained")).size(theme.font_size_small).color(theme.text_muted()));
            } else {
                ui.label(RichText::new(t("skills_page.can_now")).strong().size(theme.font_size_small).color(theme.text_secondary()));
                for key in &sk.can_done {
                    statement(ui, theme, key, theme.text_primary());
                }
            }
            if let Some(next) = &sk.next_stage {
                ui.add_space(theme.spacing_xs);
                ui.label(
                    RichText::new(tf("skills_page.can_next", &[("stage", &stage_name(next))]))
                        .strong()
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
                for key in &sk.can_next {
                    statement(ui, theme, key, theme.text_muted());
                }
            }
        });
    header.header_response.hint("skills_page.skill_hint");
    open
}

/// One "I can" statement, indented under its heading.
fn statement(ui: &mut egui::Ui, theme: &Theme, key: &str, color: egui::Color32) {
    statement_text(ui, theme, t_dyn(key), color);
}

/// A line already in words, indented under its heading.
fn statement_text(ui: &mut egui::Ui, theme: &Theme, text: &str, color: egui::Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new(text).size(theme.font_size_small).color(color));
    });
}
