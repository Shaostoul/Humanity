//! Skill tests: the shipped base set is whole, stages rise with practice, there is no cap, and
//! every verb in the practice table awards what it says (skills rung 1, 2026-11-14).

use super::*;

fn repo(rel: &str) -> Vec<u8> {
    std::fs::read(format!("{}/{rel}", env!("HUMANITY_REPO_ROOT"))).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn skills_csv() -> &'static [u8] {
    include_bytes!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/skills/skills.csv"))
}

/// The registry as the engine loads it: skills, ladders, families and the practice table.
fn shipped() -> SkillRegistry {
    super::shipped_for_tests()
}

/// A world with one player and the shipped registry, ready for XP.
fn world_with_player() -> (DataStore, hecs::World, hecs::Entity) {
    let mut data = DataStore::new();
    data.insert("skill_registry", shipped());
    data.insert("xp_grants", std::sync::Mutex::new(Vec::<SkillXPEvent>::new()));
    let mut world = hecs::World::new();
    let player = world.spawn((PlayerSkills::new(),));
    (data, world, player)
}

/// en.json flattened to its dotted keys.
fn en_keys() -> std::collections::BTreeSet<String> {
    fn walk(prefix: &str, v: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, c) in m {
                    let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                    walk(&key, c, out);
                }
            }
            _ => {
                out.insert(prefix.to_string());
            }
        }
    }
    let v: serde_json::Value = serde_json::from_slice(&repo("data/i18n/en.json")).expect("en.json parses");
    let mut out = std::collections::BTreeSet::new();
    walk("", &v, &mut out);
    out
}

#[test]
fn from_csv_parses_the_real_skill_registry() {
    let reg = SkillRegistry::from_csv(skills_csv()).expect("skills.csv parses");
    assert_eq!(reg.skills.len(), 90, "the approved base set is 90 skills (docs/design/skills.md section 2)");
    let mining = reg.get("mining").expect("mining skill present");
    assert_eq!(mining.family, "making");
    assert_eq!(mining.xp_per_level, 100);
    // The curve: level 1 needs xp_base * 1^1.5 = 100.
    assert_eq!(mining.xp_for_level(1), 100);
    let staff = reg.get("staff_spinning").expect("staff present");
    assert!(staff.transfers.iter().any(|(to, f)| to == "polearms" && (*f - 0.25).abs() < 1e-6), "staff declares its transfer to the fighting staff");
}

/// The shipped base set is whole: every skill names a family in families.json, every family has
/// skills, every skill has a ladder of the six stages in stages.json's order with levels that
/// rise, and every word a person reads (name, description, each "I can", the stage and family
/// names) is a key of data/i18n/en.json.
///
/// Seen red 2026-11-14 with baking's Beginner moved to level 1 and keyed to a missing statement:
/// "baking: beginner starts at level 1 after 1; stages must rise" and "baking: skills.baking.s2_z is
/// not in en.json".
#[test]
fn the_shipped_skill_ladders_are_whole() {
    let reg = shipped();
    let keys = en_keys();
    let mut bad = Vec::new();
    assert_eq!(reg.stage_ids.first().map(String::as_str), Some("untrained"), "Untrained comes first");
    assert_eq!(reg.stage_ids.len(), 7, "six named stages plus Untrained");
    for fam in &reg.families {
        if !reg.skills.values().any(|d| &d.family == fam) {
            bad.push(format!("family {fam} has no skill"));
        }
        for k in ["name", "desc"] {
            if !keys.contains(&format!("skill_families.{fam}.{k}")) {
                bad.push(format!("family {fam}: no skill_families.{fam}.{k} in en.json"));
            }
        }
    }
    for st in &reg.stage_ids {
        for k in ["name", "meaning"] {
            if !keys.contains(&format!("skill_stages.{st}.{k}")) {
                bad.push(format!("stage {st}: no skill_stages.{st}.{k} in en.json"));
            }
        }
    }
    let mut ids: Vec<&String> = reg.skills.keys().collect();
    ids.sort();
    for id in ids {
        let d = &reg.skills[id];
        if !reg.families.contains(&d.family) {
            bad.push(format!("{id}: family {} is not in families.json", d.family));
        }
        for k in [d.name_key(), d.desc_key()] {
            if !keys.contains(&k) {
                bad.push(format!("{id}: no {k} in en.json"));
            }
        }
        let named: Vec<&str> = d.ladder.iter().map(|s| s.stage.as_str()).collect();
        let want: Vec<&str> = reg.stage_ids.iter().skip(1).map(String::as_str).collect();
        if named != want {
            bad.push(format!("{id}: ladder stages {named:?}, want {want:?}"));
        }
        let mut last = 0;
        for s in &d.ladder {
            if s.from_level <= last {
                bad.push(format!("{id}: {} starts at level {} after {last}; stages must rise", s.stage, s.from_level));
            }
            last = s.from_level;
            if s.can.is_empty() {
                bad.push(format!("{id}: {} has no I can statement", s.stage));
            }
            for k in &s.can {
                if !keys.contains(k) {
                    bad.push(format!("{id}: {k} is not in en.json"));
                }
            }
        }
        for (to, f) in &d.transfers {
            if reg.get(to).is_none() || !(*f > 0.0 && *f < 1.0) {
                bad.push(format!("{id}: transfer to {to}:{f} names no skill or no fraction in 0..1"));
            }
        }
    }
    assert!(bad.is_empty(), "the skill data is not whole:\n  {}", bad.join("\n  "));
}

/// BUG-248: THE WEBSITE'S XP TABLE IS THE GAME'S RULE. data/skills/xp_curve.json, which the
/// website's Profile and Tasks pages level a person's skills from, is `xp_for_level` for a
/// skill of its `base`, step by step for its first `levels_listed` levels (its `xp_per_level[i]`
/// is the XP to go from level i to i + 1; past the table the pages use the same formula, since
/// skills have no top level). Run by `just validate-data` too; on a difference it prints the
/// right table. scripts/tests/xp-curve-web.test.js holds the pages to the file.
///
/// Seen red 2026-11-04 with the file as it was (each step 1.5 times the last): "the website's
/// table is the game's rule at base 100" (left [100, 150, 225, 338, ...], right [100, 282, 519,
/// 800, ...]).
#[test]
fn the_websites_xp_curve_is_the_games_rule() {
    let path = concat!(env!("HUMANITY_REPO_ROOT"), "/data/skills/xp_curve.json");
    let file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("the table")).expect("it is JSON");
    let base = file["base"].as_u64().expect("the table names its base") as u32;
    let listed = file["levels_listed"].as_u64().expect("and how many levels it lists") as u32;
    let steps: Vec<u32> = file["xp_per_level"].as_array().expect("and its steps").iter().map(|v| v.as_u64().unwrap_or(0) as u32).collect();
    let rule: Vec<u32> = (1..=listed).map(|level| xp_for_level_at(base, level)).collect();
    assert_eq!(steps, rule, "the website's table is the game's rule at base {base}");
}

#[test]
fn channel_xp_grant_levels_up_the_player() {
    let (data, mut world, player) = world_with_player();
    let mut sys = SkillSystem::new();

    // 50 XP — below the 100 needed for level 1 → no level-up yet.
    award_skill_xp(&data, "mining", 50);
    sys.tick(&mut world, 0.0, &data);
    {
        let sk = world.get::<&PlayerSkills>(player).unwrap();
        assert_eq!(sk.level("mining"), 0);
        assert_eq!(sk.xp("mining"), 50);
    }

    // Another 50 → 100 total → reaches level 1 (leftover XP resets to 0).
    award_skill_xp(&data, "mining", 50);
    sys.tick(&mut world, 0.0, &data);
    {
        let sk = world.get::<&PlayerSkills>(player).unwrap();
        assert_eq!(sk.level("mining"), 1, "100 XP reaches mining level 1");
        assert_eq!(sk.xp("mining"), 0, "leftover XP after the level-up is 0");
    }
}

/// A stage's numbers rise with practice and its NAME changes exactly at its threshold: baking's
/// Beginner starts at level 3 (data/skills/stages.json), so at level 2 the person is a Novice and
/// the attempt that reaches level 3 makes them a Beginner; the progress toward the next stage
/// rises with each attempt in between.
///
/// Seen red 2026-11-14 with stage_at off by one (the next stage instead of the one reached):
/// "level 0 is Untrained" (left Some("novice")).
#[test]
fn a_stage_rises_with_practice_and_changes_name_at_its_threshold() {
    let (data, mut world, player) = world_with_player();
    let mut sys = SkillSystem::new();
    let baking = shipped().get("baking").cloned().expect("baking is a skill");
    let beginner = baking.ladder.iter().find(|s| s.stage == "beginner").expect("a Beginner stage").from_level;
    assert_eq!(baking.stage_at(0).map(|s| s.stage.as_str()), None, "level 0 is Untrained");

    let mut last_progress = -1.0f32;
    let mut saw_novice = false;
    for _ in 0..400 {
        award_skill_xp(&data, "baking", 20);
        sys.tick(&mut world, 0.0, &data);
        let (level, xp) = {
            let sk = world.get::<&PlayerSkills>(player).unwrap();
            (sk.level("baking"), sk.xp("baking"))
        };
        let stage = baking.stage_at(level).map(|s| s.stage.clone()).unwrap_or_default();
        if level < beginner {
            if level >= 1 {
                assert_eq!(stage, "novice", "level {level} is still a Novice");
                saw_novice = true;
                let p = baking.stage_progress(level, xp);
                assert!(p > last_progress, "progress to Beginner rises with practice ({p} after {last_progress})");
                last_progress = p;
            }
        } else {
            assert!(saw_novice, "passed through Novice on the way");
            assert_eq!(stage, "beginner", "reaching level {beginner} names the stage Beginner");
            return;
        }
    }
    panic!("400 practices never reached level {beginner}");
}

/// No cap anywhere (the operator, 2026-11-14: "Let's not focus on a cap."): XP keeps raising the
/// level past the Master stage's start, and the bar keeps filling.
///
/// Seen red 2026-11-14 with a cap put back at the Master stage: the level stopped at 17.
#[test]
fn there_is_no_cap_past_master() {
    let (data, mut world, player) = world_with_player();
    let mut sys = SkillSystem::new();
    let master = shipped().get("mining").unwrap().mastery_level();
    for _ in 0..50 {
        award_skill_xp(&data, "mining", 100_000);
    }
    sys.tick(&mut world, 0.0, &data);
    let level = world.get::<&PlayerSkills>(player).unwrap().level("mining");
    assert!(level > master + 10, "five million XP goes far past Master (level {level}, Master from {master})");
}

/// skills.md section 3: work at your level teaches most, work far below almost nothing; a good
/// result teaches more than a careless one.
#[test]
fn practice_weight_rewards_the_right_work() {
    assert!(practice_weight(5, Some(5), None) > practice_weight(5, Some(4), None), "at your level beats one below");
    assert!(practice_weight(20, Some(1), None) <= 0.1 + 1e-6, "far below teaches almost nothing (the floor)");
    assert!(practice_weight(10, Some(1), None) < 0.11, "nine levels below teaches about a tenth");
    assert!(practice_weight(1, Some(9), None) < practice_weight(1, Some(1), None), "far above teaches a little");
    assert!(practice_weight(3, None, Some(1.0)) > practice_weight(3, None, Some(0.0)) * 2.5, "a good result teaches about three times a careless one");
    assert_eq!(practice_weight(3, None, None), 1.0, "nothing known, nothing weighed");
}

/// A declared transfer speeds learning: a staff spinner learning the fighting staff gets the
/// declared quarter extra, and nobody gets it the other way round.
#[test]
fn a_declared_transfer_speeds_learning() {
    let (data, mut world, player) = world_with_player();
    let mut sys = SkillSystem::new();
    world.get::<&mut PlayerSkills>(player).unwrap().skills.insert("staff_spinning".into(), SkillProgress { level: 5, xp: 0 });
    award_skill_xp(&data, "polearms", 40);
    award_skill_xp(&data, "fishing", 40);
    sys.tick(&mut world, 0.0, &data);
    let sk = world.get::<&PlayerSkills>(player).unwrap();
    assert_eq!(sk.xp("polearms"), 50, "staff to staff-and-spear: 40 XP plus the declared 25%");
    assert_eq!(sk.xp("fishing"), 40, "fishing declares no transfer from staff");
}

/// Every verb in data/skills/practice.json awards exactly the skills it names (the wiring is
/// data-driven, so this one test covers every family's re-pointed and newly wired verbs), and a
/// continuous verb gives one attempt per its distance.
///
/// Seen red 2026-11-14 with connect.air pointed at the retired engineering: "connect.air: skill
/// engineering is not a row of skills.csv" and "connect.air: engineering got no XP".
#[test]
fn the_shipped_practice_table_awards_what_it_says() {
    let reg = shipped();
    let mut bad = Vec::new();
    let mut ids: Vec<&String> = reg.practices.keys().collect();
    ids.sort();
    for id in ids {
        let p = &reg.practices[id];
        for s in &p.skills {
            if reg.get(s).is_none() {
                bad.push(format!("{id}: skill {s} is not a row of skills.csv"));
            }
        }
        if p.xp == 0 {
            bad.push(format!("{id}: teaches 0 XP"));
        }
        let (data, mut world, player) = world_with_player();
        let mut sys = SkillSystem::new();
        match p.per_metres {
            Some(m) => practice_distance(&data, id, m, None),
            None => practice(&data, id, None),
        }
        sys.tick(&mut world, 0.0, &data);
        let sk = world.get::<&PlayerSkills>(player).unwrap();
        for s in &p.skills {
            if sk.xp(s) == 0 && sk.level(s) == 0 {
                bad.push(format!("{id}: {s} got no XP"));
            }
        }
    }
    // A landed strike trains the weapon's own skill (data/equipment.csv), so its entry names none.
    let (data, mut world, player) = world_with_player();
    let mut sys = SkillSystem::new();
    practice_with(&data, "landed_strike", "blades", None);
    sys.tick(&mut world, 0.0, &data);
    if world.get::<&PlayerSkills>(player).unwrap().xp("blades") == 0 {
        bad.push("landed_strike with a knife: blades got no XP".into());
    }
    assert!(bad.is_empty(), "the practice table does not teach what it says:\n  {}", bad.join("\n  "));
}

/// Every family that has a live verb trains at least one of its skills from one: through a recipe
/// (data/recipes.csv skill_required), the practice table, or a weapon's skill (data/equipment.csv).
/// Science and flow arts wait for their activities (no telescope, weather reading or prop
/// procedure exists yet; skills.md section 10 names the rung). Seen red 2026-11-14 with the music
/// track and the painting recipe pointed elsewhere: "family arts has a live verb but none of its
/// skills is trained".
#[test]
fn every_family_with_a_live_verb_is_trained() {
    let reg = shipped();
    let recipes = crate::crafting::RecipeRegistry::from_csv(&repo("data/recipes.csv")).expect("recipes.csv parses");
    let mut trained: std::collections::BTreeSet<String> = recipes.recipes.values().filter_map(|r| r.skill_required.clone()).collect();
    trained.extend(reg.practices.values().flat_map(|p| p.skills.iter().cloned()));
    let equipment = String::from_utf8(repo("data/equipment.csv")).unwrap();
    let mut lines = equipment.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = header.iter().position(|h| *h == "skill").expect("equipment.csv has a skill column");
    for l in lines {
        if let Some(s) = l.split(',').nth(col).filter(|s| !s.trim().is_empty()) {
            trained.insert(s.trim().to_string());
        }
    }
    trained.insert("unarmed".into()); // bare hands, frame_settles' landed strike
    for fam in ["body", "food", "making", "building", "travel", "safety", "combat", "community", "arts"] {
        let got: Vec<&String> = reg.skills.values().filter(|d| d.family == fam && trained.contains(&d.id)).map(|d| &d.id).collect();
        assert!(!got.is_empty(), "family {fam} has a live verb but none of its skills is trained");
    }
}

/// Drift guard (the recipe-skill lint): every non-empty `skill_required` in
/// recipes.csv MUST resolve to a real skill id in skills.csv, or the XP award
/// silently no-ops on the registry lookup miss.
#[test]
fn every_recipe_skill_is_a_real_skill() {
    let skills = SkillRegistry::from_csv(skills_csv()).unwrap();
    let recipes = crate::crafting::RecipeRegistry::from_csv(include_bytes!(concat!(
        env!("HUMANITY_REPO_ROOT"),
        "/data/recipes.csv"
    )))
    .expect("recipes.csv parses");
    let mut orphans: Vec<String> = recipes
        .recipes
        .values()
        .filter_map(|r| {
            r.skill_required
                .as_ref()
                .filter(|s| skills.get(s).is_none())
                .map(|s| format!("{} -> {}", r.id, s))
        })
        .collect();
    orphans.sort();
    assert!(
        orphans.is_empty(),
        "recipes reference skills absent from data/skills/skills.csv (XP would \
         silently vanish). Fix recipes.csv skill_required or add the skill:\n  {}",
        orphans.join("\n  ")
    );
}

/// Every recipe teaches a skill (skills.md section 6: 22 recipes trained nothing).
///
/// Seen red 2026-11-14 on the old recipes.csv: 22 recipes with an empty skill_required.
#[test]
fn every_shipped_recipe_trains_a_skill() {
    let recipes = crate::crafting::RecipeRegistry::from_csv(&repo("data/recipes.csv")).expect("recipes.csv parses");
    let mut none: Vec<&str> = recipes.recipes.values().filter(|r| r.skill_required.is_none()).map(|r| r.id.as_str()).collect();
    none.sort();
    assert!(none.is_empty(), "{} recipes train no skill: {}", none.len(), none.join(", "));
}

/// The migration choice (skills.md section 6): a save's progress in a retired id is dropped, not
/// mapped, because nobody plays yet; progress in a skill of the base set is kept. Seen red 2026-11-14
/// with the drop taken out: two skills left (the retired id kept).
#[test]
fn a_retired_skill_id_in_a_save_is_dropped() {
    let (data, mut world, player) = world_with_player();
    {
        let mut sk = world.get::<&mut PlayerSkills>(player).unwrap();
        sk.skills.insert(["metal", "working"].concat(), SkillProgress { level: 7, xp: 3 });
        sk.skills.insert("farming".into(), SkillProgress { level: 4, xp: 9 });
    }
    SkillSystem::new().tick(&mut world, 0.0, &data);
    let sk = world.get::<&PlayerSkills>(player).unwrap();
    assert_eq!(sk.skills.len(), 1, "only the base set's farming survives");
    assert_eq!(sk.level("farming"), 4);
}

/// #8b dev affordance: the dev_max_skills command puts EVERY skill at Master (so skill-gated
/// recipes stay testable under the "100% unlocked" posture). There is no top level to max to.
#[test]
fn dev_max_skills_puts_every_skill_at_master() {
    let (mut data, mut world, player) = world_with_player();
    data.insert("dev_max_skills", std::sync::Mutex::new(true));
    let mut sys = SkillSystem::new();
    sys.tick(&mut world, 0.0, &data);
    let reg = shipped();
    let sk = world.get::<&PlayerSkills>(player).unwrap();
    assert_eq!(sk.level("mining"), reg.get("mining").unwrap().mastery_level(), "mining at Master");
    assert_eq!(reg.get("mining").unwrap().stage_at(sk.level("mining")).map(|s| s.stage.as_str()), Some("master"));
    assert_eq!(sk.skills.len(), reg.skills.len(), "every skill present after dev-max");
}
