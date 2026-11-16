//! Practice quality tests (skills rung 2, 2026-11-15): repeats lose value and recover, a guide's
//! bonus is once per stage, a teacher helps only within reach and two stages up, Off leaves it all
//! off, and the data files hold together.
//!
//! RED EVIDENCE (2026-11-15), all four behaviour tests run against a sabotaged quality.rs in one
//! build (variety forced to 1.0, the once-per-stage check removed, the reach filter removed, and
//! Off reading the Simplified rules):
//!   repeats_of_one_task_lose_value_and_recover_after_a_day: "the 4th attempt in a day is worth
//!     less than the 2nd (5 vs 5): [8, 5, 5, 5, 5, 5, 5, 5]"
//!   a_guide_bonus_applies_once_per_stage: "reading the same guide again at the same stage gives
//!     nothing more: [Applied { skill: "farming", stage: "novice", multiplier: 1.5, until: 28800.0 }]"
//!   a_teacher_helps_only_within_reach_and_two_stages_up: "a teacher 12 m away does not help (9 vs 6)"
//!   skill_learning_off_leaves_every_attempt_as_it_was: "with skill learning Off the 8th attempt is
//!     worth the 1st (8 vs 9): [9, 8, 8, 8, 8, 8, 8, 8]"
//! and each passed once the sabotage was reverted.

use super::quality::*;
use super::*;
use std::sync::Mutex;

fn repo(rel: &str) -> Vec<u8> {
    std::fs::read(format!("{}/{rel}", env!("HUMANITY_REPO_ROOT"))).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The shipped practice-quality table and guide map.
fn shipped_quality() -> PracticeQuality {
    PracticeQuality::from_json(&repo("data/skills/practice_quality.json"), &repo("data/skills/guides.json")).expect("the shipped practice quality loads")
}

/// A world with one player, the shipped registry and quality table, the clock at `t`, and skill
/// learning at `mode`.
fn world(mode: LearningMode) -> (DataStore, hecs::World, hecs::Entity) {
    let mut data = DataStore::new();
    data.insert("skill_registry", super::shipped_for_tests());
    data.insert("xp_grants", Mutex::new(Vec::<SkillXPEvent>::new()));
    data.insert(DATA_KEY, shipped_quality());
    data.insert("game_time", Mutex::new(crate::time::GameTime::default()));
    register(&mut data);
    publish_mode(&data, mode);
    let mut world = hecs::World::new();
    let player = world.spawn((PlayerSkills::new(),));
    (data, world, player)
}

fn set_clock(data: &DataStore, secs: f64) {
    data.get::<Mutex<crate::time::GameTime>>("game_time").unwrap().lock().unwrap().set_elapsed(secs);
}

/// One attempt at the verb `id` (a practice.json entry), ticked: what `skill` gained.
fn attempt(data: &DataStore, world: &mut hecs::World, player: hecs::Entity, sys: &mut SkillSystem, id: &str, skill: &str) -> u32 {
    let before = total_xp(world, player, skill);
    practice(data, id, None);
    sys.tick(world, 0.0, data);
    total_xp(world, player, skill) - before
}

/// All the XP ever earned in `skill` (levels converted back), so a level-up does not hide a gain.
fn total_xp(world: &hecs::World, player: hecs::Entity, skill: &str) -> u32 {
    static REG: std::sync::OnceLock<SkillRegistry> = std::sync::OnceLock::new();
    let def = REG.get_or_init(super::shipped_for_tests).get(skill).unwrap();
    let sk = world.get::<&PlayerSkills>(player).unwrap();
    (1..=sk.level(skill)).map(|l| def.xp_for_level(l)).sum::<u32>() + sk.xp(skill)
}

/// THE SAME TASK REPEATED LOSES VALUE OVER AN IN-GAME DAY AND RECOVERS AFTER (skills.md section 3,
/// point 3). Laying out (drafting, 4 XP) at Realistic: the first attempt ever is worth the most (x1.5),
/// the next counts in full, each after that is worth less down to the floor; another task in the
/// same skill is new again; a day later the first task is worth full value again (not the
/// first-time bonus: that is once ever). The Skills page's explanation counts the attempts.
#[test]
fn repeats_of_one_task_lose_value_and_recover_after_a_day() {
    let (data, mut world, player) = world(LearningMode::Realistic);
    let mut sys = SkillSystem::new();
    // Collecting clay trains mining at 5 XP: big enough that x0.75 steps show after rounding.
    let gains: Vec<u32> = (0..8).map(|_| attempt(&data, &mut world, player, &mut sys, "collect.clay_raw_0", "mining")).collect();
    assert!(gains[0] > gains[1], "the very first attempt ever is worth the most ({} vs {}): {gains:?}", gains[0], gains[1]);
    assert!(gains[3] < gains[1], "the 4th attempt in a day is worth less than the 2nd ({} vs {}): {gains:?}", gains[3], gains[1]);
    assert!(gains[7] <= gains[3], "and the 8th no more than the 4th: {gains:?}");
    {
        let sk = world.get::<&PlayerSkills>(player).unwrap();
        let why = sk.memory.last.get("mining").expect("the last attempt is explained");
        assert_eq!(why.attempt, 8, "the explanation counts the attempts today");
        assert!(why.variety < 1.0 && why.first_time == 1.0);
    }
    // Another material is another task: new, so full value again (and its own first time).
    let other = attempt(&data, &mut world, player, &mut sys, "collect.sand_0", "mining");
    assert!(other >= gains[1], "the same verb on another material is a new task ({other} vs {}): {gains:?}", gains[1]);
    // A day later the clay counts in full again: the repeats rested.
    let day = crate::time::GameTime::default().seconds_per_day();
    set_clock(&data, day + 1.0);
    let rested = attempt(&data, &mut world, player, &mut sys, "collect.clay_raw_0", "mining");
    assert_eq!(rested, gains[1], "after a day's rest the task is worth full value again: {gains:?} then {rested}");
}

/// The curve itself, as the shipped table gives it (the numbers the report quotes).
#[test]
fn the_shipped_curves_fall_to_their_floors() {
    let q = shipped_quality();
    let simple = q.rules(LearningMode::Simplified).variety.expect("simplified has variety");
    let real = q.rules(LearningMode::Realistic).variety.expect("realistic has variety");
    let s: Vec<f32> = (1..=10).map(|k| simple.factor(k, simple.full_attempts)).collect();
    let r: Vec<f32> = (1..=10).map(|k| real.factor(k, real.full_attempts)).collect();
    println!("simplified per attempt: {s:?}\nrealistic per attempt: {r:?}");
    assert_eq!(s[0], 1.0);
    assert!(s.iter().all(|v| *v >= simple.floor - 1e-6) && r.iter().all(|v| *v >= real.floor - 1e-6), "never below the floor");
    assert!(s.windows(2).all(|w| w[1] <= w[0]) && r.windows(2).all(|w| w[1] <= w[0]), "never rises with repeats");
    assert!(s.iter().zip(&r).all(|(a, b)| a >= b), "the simplified level is gentler at every attempt");
    assert!(simple.floor >= 0.5, "simplified never takes more than half");
}

/// READING THE GUIDE FOR YOUR STAGE MULTIPLIES PRACTICE, ONCE PER GUIDE PER STAGE (section 3,
/// point 4). A Novice farmer reads the Novice farming guide: the next harvest counts x1.5. Reading it
/// again gives nothing more; reading the Beginner guide while a Novice gives nothing; the bonus
/// ends with its hours.
#[test]
fn a_guide_bonus_applies_once_per_stage() {
    let (data, mut world, player) = world(LearningMode::Simplified);
    let mut sys = SkillSystem::new();
    let q = shipped_quality();
    let novice_guide = q.guide_for("farming", "novice").expect("farming has a Novice guide").to_string();
    let beginner_guide = q.guide_for("farming", "beginner").expect("farming has a Beginner guide").to_string();
    // Make the player a Novice farmer (level 1).
    world.get::<&mut PlayerSkills>(player).unwrap().skills.insert("farming".into(), SkillProgress { level: 1, xp: 0 });
    // Without the guide: harvesting one crop, two attempts (past the first-time bonus).
    let mut harvest = |data: &DataStore, world: &mut hecs::World, crop: &str| {
        let before = total_xp(world, player, "farming");
        practice_scaled(data, "harvest", crop, 20, None);
        sys.tick(world, 0.0, data);
        total_xp(world, player, "farming") - before
    };
    harvest(&data, &mut world, "tomato");
    let plain = harvest(&data, &mut world, "tomato");

    note_guide_read(&data, &beginner_guide);
    harvest(&data, &mut world, "kale");
    let notices = drain_notices(&data);
    assert!(notices.is_empty(), "the Beginner guide does nothing for a Novice: {notices:?}");

    note_guide_read(&data, &novice_guide);
    let _ = harvest(&data, &mut world, "kale");
    let notices = drain_notices(&data);
    assert!(matches!(notices.as_slice(), [GuideNotice::Applied { skill, stage, .. }] if skill == "farming" && stage == "novice"), "the Novice guide applies to farming: {notices:?}");
    let helped = harvest(&data, &mut world, "tomato");
    assert!(helped as f32 >= plain as f32 * 1.4, "the guide's bonus multiplies practice ({helped} vs {plain})");
    {
        let sk = world.get::<&PlayerSkills>(player).unwrap();
        assert!(sk.memory.last["farming"].guide.is_some(), "the explanation names the guide");
    }

    note_guide_read(&data, &novice_guide);
    harvest(&data, &mut world, "kale");
    let again = drain_notices(&data);
    assert!(matches!(again.as_slice(), [GuideNotice::AlreadyRead { .. }]), "reading the same guide again at the same stage gives nothing more: {again:?}");
    let until = world.get::<&PlayerSkills>(player).unwrap().memory.guides_read["farming|novice"].until;

    // Past its hours the bonus is gone.
    set_clock(&data, until + 1.0);
    harvest(&data, &mut world, "tomato");
    let after = harvest(&data, &mut world, "tomato");
    assert!((after as f32) < helped as f32 * 0.9, "the bonus ends with its hours ({after} vs {helped})");
}

/// PRACTISING BESIDE SOMEONE TWO STAGES ABOVE YOU MULTIPLIES IT, ONLY WITHIN REACH (section 3,
/// point 4). An untrained first-aider beside a Beginner (two stages up) within 5 m: x1.5, and the HUD
/// has them; 12 m away: nothing; a Novice (one stage up) beside them: nothing.
#[test]
fn a_teacher_helps_only_within_reach_and_two_stages_up() {
    let (data, mut world, player) = world(LearningMode::Simplified);
    let mut sys = SkillSystem::new();
    let reach = shipped_quality().rules(LearningMode::Simplified).mentor.expect("simplified has the mentor bonus").reach_m;
    let teacher = |distance_m: f32, stage: &str| Helper { name: "Dr. Kel".into(), distance_m, stages: [("mining".to_string(), stage.to_string())].into_iter().collect() };
    // Four first attempts at four tasks of 5 XP each (each its own first time, so each weighed alike).
    let alone = attempt(&data, &mut world, player, &mut sys, "collect.stone_raw_0", "mining");

    publish_helpers(&data, vec![teacher(reach + 7.0, "beginner")]);
    let far = attempt(&data, &mut world, player, &mut sys, "collect.clay_raw_0", "mining");
    assert_eq!(far, alone, "a teacher {} m away does not help ({far} vs {alone})", reach + 7.0);

    publish_helpers(&data, vec![teacher(reach - 2.0, "novice")]);
    let one_up = attempt(&data, &mut world, player, &mut sys, "collect.sand_0", "mining");
    assert_eq!(one_up, alone, "one stage above is not enough ({one_up} vs {alone})");

    publish_helpers(&data, vec![teacher(reach - 2.0, "beginner")]);
    let near = attempt(&data, &mut world, player, &mut sys, "collect.ore_sample_0", "mining");
    assert!(near as f32 >= alone as f32 * 1.4, "a teacher two stages up within reach multiplies the attempt ({near} vs {alone})");
    let sk = world.get::<&PlayerSkills>(player).unwrap();
    assert_eq!(sk.memory.last["mining"].mentor.as_ref().map(|m| m.0.as_str()), Some("Dr. Kel"), "the explanation names the teacher");
    let helpers = vec![teacher(reach - 2.0, "beginner")];
    assert_eq!(sk.memory.beside_now(crate::time::GameTime::default().elapsed_seconds, &helpers, reach).len(), 1, "the HUD says so while they are near");
    assert!(sk.memory.beside_now(0.0, &[teacher(reach + 7.0, "beginner")], reach).is_empty(), "and stops when they walk away");
}

/// SKILL LEARNING OFF LEAVES EVERY ATTEMPT AS IT WAS (CLAUDE.md dual modes): no repeats penalty, no
/// first-time bonus, no guide or teacher bonus.
#[test]
fn skill_learning_off_leaves_every_attempt_as_it_was() {
    let (data, mut world, player) = world(LearningMode::Off);
    let mut sys = SkillSystem::new();
    publish_helpers(&data, vec![Helper { name: "Dr. Kel".into(), distance_m: 1.0, stages: [("mining".to_string(), "master".to_string())].into_iter().collect() }]);
    let gains: Vec<u32> = (0..8).map(|_| attempt(&data, &mut world, player, &mut sys, "collect.clay_raw_0", "mining")).collect();
    assert_eq!(gains[7], gains[0], "with skill learning Off the 8th attempt is worth the 1st ({} vs {}): {gains:?}", gains[7], gains[0]);
    assert_eq!(gains[0], 5, "and the 1st is the verb's own XP, no first-time or teacher bonus: {gains:?}");
    note_guide_read(&data, "your-first-tomato");
    sys.tick(&mut world, 0.0, &data);
    assert!(drain_notices(&data).is_empty(), "a guide read with skill learning Off starts no bonus");
}

/// A grant that is not practice (a quest reward) is never weighed by variety or help.
#[test]
fn a_plain_grant_is_not_weighed() {
    let (data, mut world, player) = world(LearningMode::Realistic);
    let mut sys = SkillSystem::new();
    for _ in 0..6 {
        award_skill_xp(&data, "mining", 7);
        sys.tick(&mut world, 0.0, &data);
    }
    assert_eq!(total_xp(&world, player, "mining"), 42, "six quest rewards of 7 are 42");
}

/// The memory survives a save: the repeats today and the guides read come back.
#[test]
fn practice_memory_round_trips_through_json() {
    let mut m = PracticeMemory::default();
    m.recent.insert("mining|collect.clay_raw_0".into(), vec![10.0, 20.0]);
    m.done.insert("mining|collect.clay_raw_0".into());
    m.guides_read.insert("farming|novice".into(), GuideRead { slug: "your-first-tomato".into(), at: 5.0, until: 100.0 });
    let back: PracticeMemory = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
    assert_eq!(back, m);
}

/// THE DATA HOLDS TOGETHER: practice_quality.json has the levels levels.json offers for
/// `skill_learning` (and each is a level this module knows); every guide in guides.json is a real
/// skill, a real stage and a document in the Library; a verb's own `full_attempts` is positive.
#[test]
fn the_practice_quality_data_holds_together() {
    let q = shipped_quality();
    let reg = super::shipped_for_tests();
    let mut bad = Vec::new();
    let levels: serde_json::Value = serde_json::from_slice(&repo("data/gameplay/levels.json")).unwrap();
    let setting = levels["settings"].as_array().unwrap().iter().find(|s| s["id"] == "skill_learning").expect("levels.json has the skill_learning setting");
    for l in setting["levels"].as_array().unwrap() {
        let id = l["id"].as_str().unwrap();
        if LearningMode::from_level(id).is_none() {
            bad.push(format!("levels.json skill_learning/{id} is not a level this module knows"));
        }
        if !q.levels.contains_key(id) {
            bad.push(format!("practice_quality.json has no rules for skill_learning/{id}"));
        }
    }
    // Every Library document's slug, worked out from its file the way the Library does
    // (gui/loaders.rs `library_slug_of_file`: no .md, underscores to hyphens, lower case).
    fn files(v: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(f) = m.get("file").and_then(|f| f.as_str()) {
                    out.insert(f.trim_end_matches(".md").replace('_', "-").to_lowercase());
                }
                m.values().for_each(|c| files(c, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|c| files(c, out)),
            _ => {}
        }
    }
    let mut slugs = std::collections::BTreeSet::new();
    files(&serde_json::from_slice(&repo("data/library/index.json")).unwrap(), &mut slugs);
    assert!(slugs.len() > 100, "the Library index read ({} documents)", slugs.len());
    let mut skills: Vec<_> = q.guides.iter().collect();
    skills.sort_by(|a, b| a.0.cmp(b.0));
    for (skill, stages) in skills {
        if reg.get(skill).is_none() {
            bad.push(format!("guides.json: {skill} is not a skill"));
        }
        for (stage, slug) in stages {
            if !reg.stage_ids.iter().any(|s| s == stage) || stage == "untrained" {
                bad.push(format!("guides.json: {skill}/{stage} is not a stage"));
            }
            if !slugs.contains(slug) {
                bad.push(format!("guides.json: {skill}/{stage} names {slug}, which is not a Library document"));
            }
        }
    }
    for (id, p) in &reg.practices {
        if p.full_attempts == Some(0) {
            bad.push(format!("practice.json {id}: full_attempts 0"));
        }
    }
    // The crew who can teach (data/npc/crew.ron `skills`): each a real skill at a real stage.
    let crew = String::from_utf8(repo("data/npc/crew.ron")).unwrap();
    let mut taught = 0;
    for line in crew.lines().map(str::trim).filter(|l| l.starts_with("skills:")) {
        let pairs: Vec<&str> = line.split('"').collect();
        for kv in pairs.chunks(4).filter(|c| c.len() >= 4) {
            let (skill, stage) = (kv[1], kv[3]);
            taught += 1;
            if reg.get(skill).is_none() {
                bad.push(format!("crew.ron: {skill} is not a skill"));
            }
            if !reg.stage_ids.iter().any(|s| s == stage) {
                bad.push(format!("crew.ron: {skill} at {stage}, which is not a stage"));
            }
        }
    }
    if taught == 0 {
        bad.push("crew.ron: no crew member lists the skills they can teach".into());
    }
    assert!(bad.is_empty(), "the practice quality data does not hold together:\n  {}", bad.join("\n  "));
}
