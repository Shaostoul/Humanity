//! Practice quality between the skills and the screen (skills rung 2, 2026-11-15,
//! docs/design/skills.md section 3, points 3 and 4).
//!
//! Each frame, after the skill sheet is mirrored (frame_mirrors.rs):
//! - Settings > Gameplay > Skill learning's level into the skills (`quality::publish_mode`);
//! - the people near the player who could teach: each crew member in the world
//!   (`net::sync::RemoteNpc`) whose row of data/npc/crew.ron lists `skills`, with their distance.
//!   Other players join when the presence sync carries their stages (a later step: what a player
//!   shares of their skills is their choice, and the sync does not carry it yet);
//! - a Library guide read to its end (the Library page sets `PracticeGui::guide_read`) into the
//!   skills, and what it did back out as a toast ("Read the Novice guide for Farming: ...");
//! - the words: each skill's "why the last practice gave what it gave", the guide for its stage and
//!   a teacher near (`GuiSkill::fill_practice`), the "How practice counts" rules, and the HUD's line
//!   while a teacher's help applies.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use humanity_core::i18n::{t_dyn, tf};
use humanity_systems::skills::quality::{self, GuideNotice, Helper, LearningMode, PracticeQuality};
use humanity_systems::skills::{PlayerSkills, SkillRegistry};

use crate::engine::state::EngineState;

/// Each crew member's skills by name, from data/npc/crew.ron (`skills`: skill id to stage id).
/// Read once for the run, like the rest of the game's data at startup: a roster, not something
/// worked out and kept (the no-reuse switch has nothing to turn off here).
fn crew_skills() -> &'static HashMap<String, BTreeMap<String, String>> {
    static CREW: OnceLock<HashMap<String, BTreeMap<String, String>>> = OnceLock::new();
    CREW.get_or_init(|| crate::gui::talk::book::crew().into_iter().filter(|c| !c.skills.is_empty()).map(|c| (c.name, c.skills)).collect())
}

/// The crew near the player who have skills, with how far away they are (metres).
fn helpers_near(state: &EngineState) -> Vec<Helper> {
    let crew = crew_skills();
    if crew.is_empty() {
        return Vec::new();
    }
    let me = state.camera.position;
    let mut out = Vec::new();
    for (_e, (t, npc)) in state.game_world.world.query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemoteNpc)>().iter() {
        let Some(skills) = crew.get(&npc.name) else { continue };
        // Across the floor: the crew member's mark stands above their feet and the camera is the
        // eye, so the height between them is not distance.
        let d = t.position - me;
        let distance_m = (d.x * d.x + d.z * d.z).sqrt();
        if distance_m > 50.0 {
            continue;
        }
        out.push(Helper { name: npc.name.clone(), distance_m, stages: skills.iter().map(|(k, v)| (k.clone(), v.clone())).collect() });
    }
    out
}

/// A stage id in words ("Novice").
fn stage_name(stage: &str) -> &'static str {
    t_dyn(&format!("skill_stages.{stage}.name"))
}

/// A multiplier in words: 1.5 is "1.5", 2.0 is "2".
fn times(x: f32) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// One frame of practice quality (see the module notes).
pub(crate) fn run(state: &mut EngineState) {
    let mode = LearningMode::from_level(state.gui_state.settings.level("skill_learning")).unwrap_or_default();
    quality::publish_mode(&state.data_store, mode);
    let helpers = helpers_near(state);
    quality::publish_helpers(&state.data_store, helpers.clone());
    if let Some(slug) = state.gui_state.practice.guide_read.take() {
        quality::note_guide_read(&state.data_store, &slug);
    }
    let Some(q) = state.data_store.get::<PracticeQuality>(quality::DATA_KEY) else { return };
    let rules = q.rules(mode);
    let ui_now = state.egui_ctx.input(|i| i.time);
    let game_now = humanity_systems::time::elapsed_now(&state.data_store);

    // What reading a guide did, said once (the skills' tick handled it).
    for n in quality::drain_notices(&state.data_store) {
        let text = match n {
            GuideNotice::Applied { skill, stage, multiplier, until } => tf(
                "skills_page.guide_applied",
                &[
                    ("stage", &stage_name(&stage)),
                    ("skill", &t_dyn(&format!("skills.{skill}.name"))),
                    ("times", &times(multiplier)),
                    ("hours", &times(((until - game_now) / 3600.0) as f32)),
                ],
            ),
            GuideNotice::AlreadyRead { skill, stage } => {
                tf("skills_page.guide_already", &[("stage", &stage_name(&stage)), ("skill", &t_dyn(&format!("skills.{skill}.name")))])
            }
        };
        state.gui_state.toast(text, crate::gui::ToastKind::Info, ui_now);
    }

    state.gui_state.practice.reading = Some(q.reading.clone());
    // The Skills page's words only while the Profile page is drawn (here or on a wall screen):
    // the HUD's teacher line below is the only part every frame needs.
    let page_open = crate::engine::screens::page_drawn(state.gui_state.active_page, &state.screens.framed_pages, crate::gui::GuiPage::Profile);
    if page_open {
        state.gui_state.practice.rules = crate::gui::pages::profile_skills::rule_lines(&rules);
    }
    let stage_ids = if page_open { state.data_store.get::<SkillRegistry>("skill_registry").map(|r| r.stage_ids.clone()).unwrap_or_default() } else { Vec::new() };
    let titles: HashMap<&str, &str> = if page_open {
        state.gui_state.library.iter().flat_map(|s| s.categories.iter()).flat_map(|c| c.entries.iter()).map(|e| (e.slug.as_str(), e.title.as_str())).collect()
    } else {
        HashMap::new()
    };
    let title_of = |slug: &str| titles.get(slug).map(|t| t.to_string());
    let mut beside_lines = Vec::new();
    for (_e, (skills, _c)) in state.game_world.world.query::<(&PlayerSkills, &humanity_core::ecs::components::Controllable)>().iter() {
        if page_open {
            let facts = crate::gui::pages::profile_skills::PracticeFacts {
                quality: q,
                mode,
                memory: &skills.memory,
                helpers: &helpers,
                stage_ids: &stage_ids,
                now: game_now,
                title_of: &title_of,
            };
            for sk in state.gui_state.skills.iter_mut() {
                sk.fill_practice(&facts);
            }
        }
        if let Some(m) = &rules.mentor {
            for b in skills.memory.beside_now(game_now, &helpers, m.reach_m) {
                beside_lines.push(tf(
                    "skills_page.hud_beside",
                    &[("name", &b.name), ("skill", &t_dyn(&format!("skills.{}.name", b.skill))), ("times", &times(b.multiplier))],
                ));
            }
        }
        break;
    }
    state.gui_state.practice.beside = beside_lines;
}
