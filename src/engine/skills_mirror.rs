//! The skill rows the pages read (`GuiState::skills`): built only on a frame that draws a page
//! reading them, and on such a frame only when what they are made from moved (2026-11-21).
//!
//! WHY. frame_mirrors.rs cleared `GuiState::skills` on EVERY world frame and built a `GuiSkill`
//! row for each of the 90 skills (its name looked up in the person's language, its stages, its
//! "I can" lists cloned), then sorted them, whether or not anything showed them. The work was real
//! every frame, and under load the v0.1568.0 startup check saw `cpu.span.mirror_skills` at 504 ms
//! in one page frame. The rows have two readers, found by reading every use of `GuiState::skills`
//! (2026-11-21):
//!
//! - the skill sheet: the Skills section of the Real page (gui/pages/profile.rs `draw_skills`,
//!   through pages/profile_skills.rs). On the main screen that is the Real page (or the Profile
//!   page, its alias, frame_ui_run.rs) on its "skills" section; on a wall screen, the Profile
//!   page on its Skills section (the bedroom's Profile screen beside the mirror, `profile::draw`;
//!   the standing mirror itself until it became a real mirror on 2026-11-21) or the Real page
//!   on the "skills" section;
//! - the Crafting page: a recipe's skill level, the skill's name and the expected grade
//!   (gui/pages/crafting.rs `player_skill_level`, `skill_display_name`).
//!
//! So the rows are built only on a frame that draws one of them (`read_this_frame`), the way the
//! Garden data is (engine/garden_mirror.rs, docs/design/sim-frame-cost.md S1). A wall screen is
//! known by the pages the screens framed last tick (`Screens::framed_pages`), as for the Quests
//! chain window and practice quality's words; a screen coming into view warms up before it paints
//! (`screen_surface::turn_plan`), by which time its page is among them.
//!
//! KEPT WHILE NOTHING MOVED. On a frame that reads them the rows are built again only when what
//! they are made from moved (`key`): the player's skills (every skill's level and XP), the skill
//! registry (its DataStore stamp: a hot reload), the language (a skill's name) and whether there
//! is a player at all. The player's skills are read as a fingerprint of their content, taken fresh
//! on every reading frame, rather than a version number the skill system bumps: `PlayerSkills` is
//! a public map written by the skills' tick, the load, the new-game reset, the dev cheats and the
//! tests, and a version is only as good as the writer that remembers to bump it, while the
//! fingerprint of the map itself cannot miss a write. Its cost is one pass over the trained skills
//! (a hash each), against building 90 rows.
//!
//! The Dev page's "Turn off all reuse" (humanity_core::reuse) builds the rows on every frame that
//! reads them, as frame_mirrors.rs did on every frame (`Kept`). Practice quality's words in each
//! row (why the last practice gave what it gave, the guide, a teacher near) are written by
//! engine/practice_view.rs on every frame the sheet is drawn, onto kept rows or new ones alike.

use std::cell::RefCell;
use std::hash::Hasher;

use humanity_core::ecs::components::Controllable;
use humanity_core::hot_reload::data_store::DataStore;
use humanity_systems::skills::{PlayerSkills, SkillRegistry};

use crate::engine::state::EngineState;
use crate::gui::pages::profile::ProfileSection;
use crate::gui::widgets::offscreen::KeyHasher;
use crate::gui::{GuiPage, GuiSkill};

/// What is kept between frames (the main thread's `KEPT`; a test keeps its own).
#[derive(Default)]
/// reuse: site: the skill rows the Skills section and the Crafting page read: with the switch off
/// they are built on every frame that draws one of those pages, instead of kept while the key of
/// what they are made from (the player's skills, the registry's stamp, the language) holds.
pub(crate) struct Kept {
    /// The key the rows in `GuiState::skills` were last built under; None: never built.
    key: Option<u64>,
    /// Builds so far (what the tests count).
    pub(crate) builds: u64,
}

thread_local! {
    static KEPT: RefCell<Kept> = RefCell::new(Kept::default());
}

/// The skill sheet (the Real page's Skills section) is drawn this frame: on the main screen
/// (`main`, the open page) or on a wall screen framed last tick (`framed`). `real_section` is the
/// Real page's open section, `profile_section` the one a wall screen's Profile page draws.
pub(crate) fn sheet_drawn(main: GuiPage, framed: &[GuiPage], real_section: &str, profile_section: ProfileSection) -> bool {
    let skills = real_section == "skills";
    // The main screen draws the Profile page as the Real page, on the Real page's section.
    let main_sheet = matches!(main, GuiPage::Real | GuiPage::Profile) && skills;
    // A wall screen's Profile page draws its own section (`profile::draw`), its Real page the
    // shared one.
    let wall_sheet =
        (framed.contains(&GuiPage::Profile) && profile_section == ProfileSection::Skills) || (framed.contains(&GuiPage::Real) && skills);
    main_sheet || wall_sheet
}

/// A page that reads the skill rows is drawn this frame: the skill sheet or the Crafting page.
pub(crate) fn read_this_frame(main: GuiPage, framed: &[GuiPage], real_section: &str, profile_section: ProfileSection) -> bool {
    sheet_drawn(main, framed, real_section, profile_section) || crate::engine::screens::page_drawn(main, framed, GuiPage::Crafting)
}

/// The key of everything the rows are made from (the module notes).
fn key(world: &hecs::World, data: &DataStore) -> u64 {
    let mut h = KeyHasher::default();
    h.write_u64(data.stamp("skill_registry"));
    h.write(humanity_core::i18n::language().as_bytes());
    let mut q = world.query::<(&PlayerSkills, &Controllable)>();
    match q.iter().next() {
        Some((_, (skills, _))) => {
            // The map's own order is not its content, so each entry is hashed on its own and the
            // hashes summed.
            let mut sum = 0u64;
            for (id, p) in &skills.skills {
                let mut e = KeyHasher::default();
                e.write_usize(id.len());
                e.write(id.as_bytes());
                e.write_u32(p.level);
                e.write_u32(p.xp);
                sum = sum.wrapping_add(e.finish());
            }
            h.write_u8(1);
            h.write_usize(skills.skills.len());
            h.write_u64(sum);
        }
        None => h.write_u8(0),
    }
    h.finish()
}

/// The rows: every defined skill with the player's progress (0 when untrained), a complete skill
/// sheet, each with its named stage, its "I can" list and the progress to the next stage (skills
/// rung 1), in family order, then by name. None without a player or a registry.
fn build(world: &hecs::World, data: &DataStore, out: &mut Vec<GuiSkill>) {
    out.clear();
    let Some(reg) = data.get::<SkillRegistry>("skill_registry") else { return };
    if let Some((_, (skills, _))) = world.query::<(&PlayerSkills, &Controllable)>().iter().next() {
        for (id, def) in reg.skills.iter() {
            let prog = skills.skills.get(id);
            let level = prog.map_or(0, |p| p.level);
            let xp = prog.map_or(0, |p| p.xp);
            out.push(GuiSkill::from_def(def, reg, level, xp));
        }
        out.sort_by(|a, b| a.family_order.cmp(&b.family_order).then(a.name.cmp(&b.name)));
    }
}

/// One frame of the rows: built when a page reading them is drawn (`read`) and what they are made
/// from moved since they were built, or on every such frame with `reuse` off. True when it built.
pub(crate) fn frame(world: &hecs::World, data: &DataStore, out: &mut Vec<GuiSkill>, kept: &mut Kept, reuse: bool, read: bool) -> bool {
    if !read {
        return false;
    }
    let key = key(world, data);
    if reuse && kept.key == Some(key) {
        return false;
    }
    // Its cost, on the Performance page's "Data for the open pages" row (budget_systems.ron).
    let _cost = humanity_core::frame_costs::stage("cpu.page_data.skills");
    build(world, data, out);
    kept.key = Some(key);
    kept.builds += 1;
    true
}

/// This frame's rows, for the main screen's page and the wall screens framed last tick.
pub(crate) fn run(state: &mut EngineState) {
    let gs = &state.gui_state;
    let read = read_this_frame(gs.active_page, &state.screens.framed_pages, &gs.active_real_section, gs.profile_section);
    let reuse = humanity_core::reuse::enabled();
    KEPT.with(|k| frame(&state.game_world.world, &state.data_store, &mut state.gui_state.skills, &mut k.borrow_mut(), reuse, read));
}

#[cfg(test)]
#[path = "skills_mirror_tests.rs"]
mod tests;
