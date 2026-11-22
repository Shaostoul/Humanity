//! The skill rows are built only on a frame that draws a page reading them, and on such a frame
//! only when what they are made from moved (skills_mirror.rs, 2026-11-21).
//!
//! Against the shipped skill data (data/skills/), a player entity with `PlayerSkills`, and the
//! frame function the engine runs (`frame`), counting the builds.

use super::{frame, read_this_frame, sheet_drawn, Kept};
use crate::gui::pages::profile::ProfileSection;
use crate::gui::{GuiPage, GuiSkill};
use humanity_core::ecs::components::Controllable;
use humanity_core::hot_reload::data_store::DataStore;
use humanity_systems::skills::{PlayerSkills, SkillProgress, SkillRegistry};

/// The skill registry as the engine loads it (skills, ladders, families).
fn registry() -> SkillRegistry {
    let read = |rel: &str| std::fs::read(format!("{}/data/skills/{rel}", env!("HUMANITY_REPO_ROOT"))).unwrap_or_default();
    SkillRegistry::from_csv(&read("skills.csv"))
        .and_then(|r| r.with_stages(&read("stages.json")))
        .and_then(|r| r.with_families(&read("families.json")))
        .expect("the shipped skill data loads")
}

/// A world with a player who has practised nothing, and the registry in the store.
fn world_and_data() -> (hecs::World, DataStore, hecs::Entity) {
    let mut data = DataStore::new();
    data.insert("skill_registry", registry());
    let mut world = hecs::World::new();
    let player = world.spawn((PlayerSkills::new(), Controllable));
    (world, data, player)
}

/// NO PAGE READING THEM, NO ROWS BUILT. Ten frames in the world with no page that reads the skill
/// rows build none (frame_mirrors.rs built all 90 on every one).
///
/// Seen red 2026-11-21 against the old rule (`frame` building on every frame): "ten frames that
/// draw no page reading the rows built 10 time(s)".
#[test]
fn no_rows_are_built_on_a_frame_no_reader_draws() {
    let (world, data, _) = world_and_data();
    let (mut rows, mut kept) = (Vec::<GuiSkill>::new(), Kept::default());
    for _ in 0..10 {
        frame(&world, &data, &mut rows, &mut kept, true, false);
    }
    assert_eq!(kept.builds, 0, "ten frames that draw no page reading the rows built {} time(s)", kept.builds);
    assert!(rows.is_empty(), "and left no rows");
}

/// A READER BUILDS THE ROWS, AND THEY FOLLOW EVERY CHANGE. The first frame a reader draws builds a
/// row per skill; more frames with nothing changed keep them; a level gained, XP alone, a hot reload
/// of the skill data and the player leaving each build them again the next reading frame, and the
/// rows show the change.
///
/// Seen red 2026-11-21 against the old rule (`frame` building on every frame): "five more frames
/// with nothing moved: the rows are kept, but 6 builds".
#[test]
fn a_reader_builds_the_rows_and_they_follow_every_change() {
    let (mut world, mut data, player) = world_and_data();
    let (mut rows, mut kept) = (Vec::<GuiSkill>::new(), Kept::default());
    let all = registry().skills.len();
    assert!(all >= 50, "the shipped set has its skills ({all})");
    assert!(frame(&world, &data, &mut rows, &mut kept, true, true), "the first frame a reader draws builds the rows");
    assert_eq!(rows.len(), all, "a row per skill");
    assert!(rows.iter().all(|r| r.level == 0 && r.xp == 0), "nothing practised yet");
    for _ in 0..5 {
        frame(&world, &data, &mut rows, &mut kept, true, true);
    }
    assert_eq!(kept.builds, 1, "five more frames with nothing moved: the rows are kept, but {} builds", kept.builds);

    // A level gained (farming practised to level 4).
    world.get::<&mut PlayerSkills>(player).unwrap().skills.insert("farming".into(), SkillProgress { level: 4, xp: 120 });
    assert!(frame(&world, &data, &mut rows, &mut kept, true, true), "a level gained builds the rows again");
    let farming = |rows: &[GuiSkill]| rows.iter().find(|r| r.id == "farming").map(|r| (r.level, r.xp)).expect("a farming row");
    assert_eq!(farming(&rows), (4, 120), "and they show it");
    assert_eq!(rows.len(), all, "still a row per skill");

    // XP alone, toward the next level.
    world.get::<&mut PlayerSkills>(player).unwrap().skills.get_mut("farming").unwrap().xp = 121;
    assert!(frame(&world, &data, &mut rows, &mut kept, true, true), "XP alone builds the rows again");
    assert_eq!(farming(&rows), (4, 121), "and they show it");
    assert!(!frame(&world, &data, &mut rows, &mut kept, true, true), "then nothing moved again");

    // A hot reload of the skill data (the registry put in the store again: its stamp moves).
    data.insert("skill_registry", registry());
    assert!(frame(&world, &data, &mut rows, &mut kept, true, true), "a hot reload of the skill data builds them again");

    // The player gone (back to the menu): no rows.
    world.despawn(player).unwrap();
    assert!(frame(&world, &data, &mut rows, &mut kept, true, true), "the player leaving builds them again");
    assert!(rows.is_empty(), "no player, no rows");
    assert_eq!(kept.builds, 5, "one build for each change, and none between");
}

/// THE REUSE SWITCH TURNS THE KEEPING OFF. With the Dev page's "Turn off all reuse" (`reuse`
/// false) every frame that draws a reader builds the rows, nothing moved or not; a frame that
/// draws none still builds nothing.
///
/// Seen red 2026-11-21 with `frame` ignoring `reuse` (the key alone deciding): "four reading frames
/// with the reuse switch off built 1 time(s)".
#[test]
fn the_reuse_switch_builds_the_rows_on_every_reading_frame() {
    let (world, data, _) = world_and_data();
    let (mut rows, mut kept) = (Vec::<GuiSkill>::new(), Kept::default());
    for _ in 0..4 {
        frame(&world, &data, &mut rows, &mut kept, false, true);
    }
    assert_eq!(kept.builds, 4, "four reading frames with the reuse switch off built {} time(s)", kept.builds);
    frame(&world, &data, &mut rows, &mut kept, false, false);
    assert_eq!(kept.builds, 4, "and a frame that draws no reader still builds none");
}

/// THE READERS. The skill sheet is the Real page's Skills section: on the main screen the Real page
/// (or the Profile page, its alias) on the "skills" section; on a wall screen the Profile page on
/// its own Skills section (the bedroom's Profile screen) or the Real page on the "skills" section. The rows'
/// other reader is the Crafting page, on either. Nothing else reads them.
///
/// Seen red 2026-11-21 against practice_view.rs's old rule for the sheet (`page_drawn(main, framed,
/// Profile)`, which the main screen never meets past its first frame: frame_ui_run.rs turns the
/// Profile page into the Real page): "the main screen's Real page on its Skills section".
#[test]
fn the_readers_are_the_skill_sheet_and_the_crafting_page() {
    use GuiPage::{Crafting, Inventory, Profile, Quests, Real};
    let none = GuiPage::None;
    let (body, skills) = (ProfileSection::BodyMeasurements, ProfileSection::Skills);
    // The main screen.
    assert!(sheet_drawn(Real, &[], "skills", body), "the main screen's Real page on its Skills section");
    assert!(sheet_drawn(Profile, &[], "skills", body), "the main screen's Profile page, drawn as the Real page on its section");
    assert!(!sheet_drawn(Real, &[], "body", skills), "the main screen's Real page on another section");
    // A wall screen.
    assert!(sheet_drawn(none, &[Profile], "body", skills), "a wall screen's Profile page on its Skills section");
    assert!(!sheet_drawn(none, &[Profile], "skills", body), "a wall screen's Profile page draws its own section, here another");
    assert!(sheet_drawn(none, &[Quests, Real], "skills", body), "a wall screen's Real page on the Skills section");
    assert!(!sheet_drawn(Inventory, &[Quests], "skills", skills), "neither page drawn");
    // The Crafting page, and nothing else.
    assert!(read_this_frame(Crafting, &[], "body", body), "the main screen's Crafting page");
    assert!(read_this_frame(none, &[Crafting], "body", body), "a wall screen's Crafting page");
    assert!(read_this_frame(Real, &[], "skills", body), "the sheet reads them too");
    assert!(!read_this_frame(none, &[], "skills", skills), "in the world with no page open, nothing reads them");
    assert!(!read_this_frame(Inventory, &[Quests], "body", skills), "nor on pages that do not show them");
}
