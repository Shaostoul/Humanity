//! From machines.rs's tests: every shipped home against the starter quests, read through the
//! recipes, blueprints, trade goods, quests and food the simulation owns.


/// EVERY SHIPPED HOME HAS WHAT THE STARTER QUESTS NEED (2026-10-04, the first-hour
/// audit's B4). A hand craft needs its recipe's station placed in the home (the station
/// gate in CraftingSystem). Settings > Home Design = Solo loads home_solo.ron, which placed
/// no smelter and no workbench, and the only smelter a player can build, the furnace
/// (data/blueprints/basic.ron), takes 3 iron ingots: so First Steps' second step, an iron
/// ingot, could never be done in the one-person home, nor Toolsmith's hammer after it.
///
/// The starter chain is the quest accepted at spawn (`gs_first_steps`, lib.rs) and each
/// quest that follows it in the data (the one naming it as `prerequisite`), until the chain
/// branches. For each shipped home: a Craft step's recipe has its station placed, a Make
/// step's item comes from a recipe whose station is placed, and every material a Build step
/// uses is in the home's stores (data/places/seed.json) or sold at a trading post the home
/// places (data/trade_goods.ron). Ore is not checked here: the drone on the Inventory page
/// mines it in every home, and tests/recipe_sources_lint.rs checks every quest item has a
/// source.
///
/// Red, run on the shipped files before this: "home_solo.ron: First Steps, step 2 (Smelt an
/// iron ingot at a smelter (with coal or graphite)): no recipe that makes iron_ingot_0 has
/// its station placed (smelt_iron at smelter, smelt_iron_graphite at smelter)" and
/// "home_solo.ron: Toolsmith, step 1 (Forge a hammer): craft_hammer needs a workbench, which
/// this home does not place".
///
/// THE OPENING (2026-10-04): First Steps is now the first ten minutes, and the check covers
/// what each of its steps uses as well. An Eat step needs food in the starting kit
/// (the homestead's kit in data/homes/starts.ron since increment 6; only the backpack's food
/// has an Eat button). A Plant step needs the
/// tower design its line names placed in the home, growing something the kit has a seed for.
/// A Make or Craft step needs, besides its station, every input of that recipe to hand: in
/// the home's stores, in the kit, sold at a trading post the home places, or got by an earlier
/// step of the chain (its ore, its ingot, a quest's reward). The opening's front door is the
/// plot's, not the home layout's: engine::crossing_tests::quests_opening holds it for every plot.
///
/// Red, run on the shipped files with the kit's rations taken out and the fishing rod's
/// recipe asking for a fishing line nothing sells (the Barn's planks and wire are sold at the
/// trading post too, so taking them out of the Barn leaves the rod makeable): "home.ron:
/// First Steps, step 2 (Eat something: press I, click a Basic Ration, then Eat): nothing in
/// the starting kit is eaten food" and "home.ron: First Steps, step 4 (Craft a tool: Esc >
/// Crafting > Carve Fishing Rod > Craft): no recipe that makes fishing_rod_0 has its station
/// and every input to hand (craft_fishing_rod lacks fishing_line_0)", the same for
/// home_solo.ron.
#[test]
fn every_shipped_home_has_what_the_starter_quests_need() {
    use crate::quests::{QuestObjective, QuestRegistry};
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    let quests = QuestRegistry::from_ron_dir(&root.join("data").join("quests"));
    let recipes = crate::crafting::RecipeRegistry::from_csv(&std::fs::read(root.join("data").join("recipes.csv")).unwrap())
        .expect("recipes.csv parses");
    let blueprints = crate::construction::BlueprintRegistry::from_ron(&std::fs::read(root.join("data").join("blueprints").join("basic.ron")).unwrap())
        .expect("basic.ron parses");
    let goods = crate::economy::TradeGoodsRegistry::from_ron(&std::fs::read(root.join("data").join("trade_goods.ron")).unwrap())
        .expect("trade_goods.ron parses");

    // What the home's stores hold at the start: every item under the "home" entity.
    let seed: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root.join("data").join("places").join("seed.json")).unwrap())
        .expect("seed.json parses");
    fn stocked(v: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
        if let Some(item) = v.get("item").and_then(|i| i.as_str()) {
            out.insert(item.to_string());
        }
        for c in v.get("children").and_then(|c| c.as_array()).into_iter().flatten() {
            stocked(c, out);
        }
    }
    let mut stores = std::collections::BTreeSet::new();
    for e in seed["entities"].as_array().expect("seed.json lists entities") {
        if e["id"] == "home" {
            stocked(e, &mut stores);
        }
    }
    assert!(stores.contains("wood_plank_0"), "the home's stores parsed: {stores:?}");

    // The starter chain, in order.
    let mut chain = vec![quests.get("gs_first_steps").expect("the starter quest ships")];
    loop {
        let last = chain.last().unwrap().id.clone();
        let next: Vec<_> = quests.quests.values().filter(|q| q.prerequisite.as_deref() == Some(last.as_str())).collect();
        if next.len() != 1 || chain.iter().any(|q| q.id == next[0].id) {
            break;
        }
        chain.push(next[0]);
    }
    assert!(chain.len() >= 2, "First Steps leads on to Toolsmith");

    // What a new homestead player carries (the homestead's kit in data/homes/starts.ron, the
    // list save_load::starting_kit reads; the starter chain is the homestead's opening).
    let starts = humanity_ship::starts::Starts::parse(&std::fs::read(root.join("data").join(humanity_ship::starts::STARTS_FILE)).unwrap())
        .expect("starts.ron parses");
    let kit: std::collections::BTreeSet<String> =
        starts.get("homestead").expect("the homestead start").kit.iter().map(|(id, _)| id.clone()).collect();
    assert!(kit.len() > 5, "the starting kit parsed: {kit:?}");
    // The tower designs (data/towers/aeroponic_configs.ron): which plants each grows. A tower
    // of design X is placed as machine "aeroponic_tower_X".
    #[derive(serde::Deserialize)]
    struct Towers {
        towers: Vec<Tower>,
    }
    #[derive(serde::Deserialize)]
    struct Tower {
        id: String,
        name: String,
        plantings: Vec<Planting>,
    }
    #[derive(serde::Deserialize)]
    struct Planting {
        plant: String,
    }
    let towers = ron::from_str::<Towers>(&std::fs::read_to_string(root.join("data").join("towers").join("aeroponic_configs.ron")).unwrap())
        .expect("aeroponic_configs.ron parses")
        .towers;

    let station = |r: &crate::crafting::Recipe| r.required_station.as_deref().map(|s| s.strip_suffix("_0").unwrap_or(s).to_string());
    let mut problems = Vec::new();
    for file in ["home.ron", "home_solo.ron"] {
        let home = crate::grow_machines::load_home(&root.join("data").join("machines").join(file)).unwrap_or_else(|| panic!("{file} parses"));
        let placed: std::collections::BTreeSet<String> = home.all_instances().into_iter().map(|i| i.machine).collect();
        let has = |s: &Option<String>| s.as_ref().map_or(true, |s| placed.contains(s));
        // What the player has to hand: the stores, the kit, the trading post's goods, and what
        // the chain's earlier steps and rewards give (added as the walk goes on).
        let mut to_hand: std::collections::BTreeSet<String> = stores.union(&kit).cloned().collect();
        if placed.contains("trading_post") {
            to_hand.extend(goods.goods.keys().cloned());
        }
        for q in &chain {
            for (n, step) in q.steps.iter().enumerate() {
                let at = format!("{file}: {}, step {} ({})", q.name, n + 1, step.description);
                // A recipe whose station is placed and whose every input is to hand.
                let can_make = |r: &crate::crafting::Recipe, to_hand: &std::collections::BTreeSet<String>| {
                    has(&station(r)) && r.inputs.iter().all(|(id, _)| to_hand.contains(id))
                };
                let lacking = |r: &crate::crafting::Recipe, to_hand: &std::collections::BTreeSet<String>| {
                    let missing: Vec<&str> = r.inputs.iter().map(|(id, _)| id.as_str()).filter(|id| !to_hand.contains(*id)).collect();
                    match (has(&station(r)), missing.is_empty()) {
                        (false, _) => format!("{} needs a {}", r.id, station(r).unwrap()),
                        (true, false) => format!("{} lacks {}", r.id, missing.join(", ")),
                        (true, true) => format!("{} can be made", r.id),
                    }
                };
                match &step.objective {
                    QuestObjective::Craft { recipe_id, .. } => {
                        let r = recipes.recipes.get(recipe_id).unwrap_or_else(|| panic!("{at}: no recipe {recipe_id}"));
                        if !has(&station(r)) {
                            problems.push(format!("{at}: {recipe_id} needs a {}, which this home does not place", station(r).unwrap()));
                        } else if !can_make(r, &to_hand) {
                            problems.push(format!("{at}: {}", lacking(r, &to_hand)));
                        }
                        to_hand.extend(r.outputs.iter().map(|(id, _)| id.clone()));
                    }
                    QuestObjective::Make { item_id, .. } => {
                        let makers = recipes.recipes_producing(item_id);
                        if !makers.iter().any(|r| has(&station(r))) {
                            let list: Vec<String> = makers.iter().map(|r| format!("{} at {}", r.id, station(r).unwrap_or_else(|| "hand".into()))).collect();
                            problems.push(format!("{at}: no recipe that makes {item_id} has its station placed ({})", list.join(", ")));
                        } else if !makers.iter().any(|r| can_make(r, &to_hand)) {
                            let why: Vec<String> = makers.iter().map(|r| lacking(r, &to_hand)).collect();
                            problems.push(format!("{at}: no recipe that makes {item_id} has its station and every input to hand ({})", why.join("; ")));
                        }
                        to_hand.insert(item_id.clone());
                    }
                    QuestObjective::Build { blueprint_id } => {
                        let bp = blueprints.blueprints.get(blueprint_id).unwrap_or_else(|| panic!("{at}: no blueprint {blueprint_id}"));
                        for (m, _) in &bp.materials {
                            let sold = placed.contains("trading_post") && goods.get(m).is_some();
                            if !stores.contains(m) && !sold {
                                problems.push(format!("{at}: {blueprint_id} uses {m}, which is not in the home's stores and no trading post here sells"));
                            }
                        }
                    }
                    // Only the backpack's food has an Eat button, so the kit must hold some.
                    QuestObjective::Eat { item_id, .. } => {
                        let kinds = crate::food::consume_kinds();
                        let food = kit.iter().any(|id| kinds.get(id) == Some(&false) && item_id.as_ref().map_or(true, |want| want == id));
                        if !food {
                            problems.push(format!("{at}: nothing in the starting kit is eaten food{}", item_id.as_ref().map(|i| format!(" ({i})")).unwrap_or_default()));
                        }
                    }
                    // The tower design the line names, placed here, growing a plant the kit has a seed for.
                    QuestObjective::Plant { crop_id, .. } => {
                        match towers.iter().find(|t| step.description.contains(&t.name)) {
                            None => problems.push(format!("{at}: the line names no tower design")),
                            Some(t) => {
                                if !placed.contains(&format!("aeroponic_tower_{}", t.id)) {
                                    problems.push(format!("{at}: this home places no {} tower (aeroponic_tower_{})", t.name, t.id));
                                }
                                let sowable = t.plantings.iter().any(|p| {
                                    kit.contains(&format!("seed_{}_0", p.plant)) && crop_id.as_ref().map_or(true, |c| c == &p.plant)
                                });
                                if !sowable {
                                    problems.push(format!("{at}: the starting kit has a seed for nothing the {} tower grows", t.name));
                                }
                            }
                        }
                    }
                    // Gathered here, to hand from now on.
                    QuestObjective::Gather { item_id, .. } => {
                        to_hand.insert(item_id.clone());
                    }
                    _ => {}
                }
            }
            to_hand.extend(q.rewards.iter().map(|(id, _)| id.clone()));
        }
    }
    assert!(problems.is_empty(), "the starter quests cannot be done in every shipped home:\n  {}", problems.join("\n  "));
}
