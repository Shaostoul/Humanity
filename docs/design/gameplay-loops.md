# HumanityOS: Gameplay Systems & Loops (holistic map)

> **Status:** design anchor for the gameplay arc (opened 2026-05-30 from the operator's
> vision brain-dump). This is the map of *what loops exist and how they connect*; each
> loop ships as its own increment. Most of the underlying systems already exist in code
> (see `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`, ~40 systems implemented,
> mostly unregistered), the work is **wiring them into loops + spawning content + the
> connective glue**, not writing them from scratch.

## Development posture: fully unlocked

For development we play **as if the player has unlocked everything 100%**, all recipes
available, materials on hand, every system active, so every loop is testable *as we
build it*, and we can balance space requirements, timings, and interactions across the
whole system at once. Progression / skill-gating / tech-unlock is a **presentation layer
added last** (it just hides/locks parts of an already-working system). A dev/creative
provisioning (stock materials + unlock recipes) is the first thing we build.

## The big picture: three layers + the connective tissue

```
   NEEDS (why)            PRODUCTION (how)                WORLD (where/with-what)
   ──────────             ────────────────                ───────────────────────
   nutrition  ◄────────── cook ◄── manufacture ◄── refine ◄── acquire
   hydration              (food)   (components/      (raw→     (mine via drone,
   health                          items/tools)      material)  farm, forage, salvage)
   oxygen                    │            │              │            │
   temperature               └── construct (base/infrastructure) ◄────┘
   energy/rest                                                   power · water · logistics
   sanitation                                                   time · weather · vehicles
```

You produce **to meet needs**; meeting needs (esp. nutrition) **buffs you**, which makes
producing/acquiring easier, the core compounding loop. Scarcity (finite asteroids),
decay/maintenance, and disasters apply pressure.

## 1. Character survival needs: *why you produce*

| Need | Model (proposed) | System (exists) |
|---|---|---|
| **Nutrition** | Baseline **Satiation** (hunger) must stay > 0 or health drains. Above baseline, a few axes (calories/energy, protein, vitamins), **variety + surplus → buffs**, deficiency → debuffs. **Cooked** food = more satiation + better/longer buffs; **raw** = edible but less, some risky. | `food`, `farming` |
| **Hydration** | **Thirst** baseline; water from sources/processing. | `hydrology` (water bodies), `food` |
| **Health** | Injury/healing; death at 0. | `combat`, `medical` |
| **Oxygen** | Breathable air in enclosed/space; suffocation otherwise. | `atmosphere` |
| **Temperature** | Cold/heat stress; mitigated by shelter/clothing/HVAC. | `weather`, `hvac` |
| **Energy/rest** | Exertion drains; rest restores; affects work speed. | `psychology`, `aging` |
| **Sanitation** | Waste handling; neglect → disease pressure. | `waste`, `ecology` |

**Buffs from good nutrition (operator's idea):** move speed, reload/work speed, night
vision, stamina, carry weight, etc., concrete, visible rewards for eating well. Harm
from poor nutrition: the inverse debuffs + health drain.

## 2. Production chain: *how you sustain + grow*

### Acquire (raw inputs)
- **Mining via space drone (operator's core loop):** player commissions a **drone** →
  it travels to an **asteroid** → mines requested ores (iron, nickel, platinum…) over
  **time** → returns home → **drops off raw material**. Asteroids hold **finite**
  resources; in MMO many players swarm one for the valuable ores then the scraps; when
  **fully consumed or abandoned the asteroid entity is deleted**. (Asteroid voxel system
  + ore veins already exist in `terrain/asteroid.rs`; the drone is a timed autonomous
  task + cargo return.)
- **Farm / garden:** plant → grow (driven by `game_time` + water + season) → harvest. A
  **concise base plant set covers complete nutrition forever**; players expand. (See §4.)
- **Forage / gather** (surface pickups) and **salvage / recycle** (break items back to
  materials) round out acquisition.

### Refine → Manufacture → Construct
- **Refine:** raw ore/material → refined material (e.g. iron ore → iron ingot → steel).
  Recipe chains already exist (`crafting`, `smelting`/`refining` categories).
- **Manufacture:** material → **components** (motors, gears, screws, rails…) → items /
  tools / machines. (`crafting`, `manufacturing`; components data exists.)
- **Construct:** items → structures / base / infrastructure. (`construction`/`placement`.)

## 3. Connective world systems

Drones & vehicles + **autonomous tasks** (the mining drone is the first); **logistics /
storage** (typed material-aware **containers, already built**, + inventory);
**power/electrical** (machines need power); **water/hydrology**; **time/seasons** (wired);
**weather**; **economy/trading** (player↔player, relay-mediated); **governance**.

## 4. Concise complete plant set (proposed starting point)

Goal: the smallest set of plants whose harvests **together** cover complete human
nutrition (carbs, protein, fats, key vitamins/minerals), grounded in real survival
agronomy (fits the educational mission). Players expand from here.

| Plant | Covers | Notes |
|---|---|---|
| **Potato** | Carbohydrate/calorie staple, vitamin C, potassium | High calories/area, fast, stores well |
| **Soybean** (or beans/lentil) | Protein + fat | Complete-ish protein, nitrogen-fixing |
| **Kale/Spinach** (leafy green) | Vitamins A/K/folate, iron, calcium | Fast cycle, cut-and-come-again |
| **Tomato** | Vitamin C, antioxidants, flavor | Cookable; high yield |
| **Sunflower** (seed/oil) | Fats/oil, vitamin E | Oil pressing → cooking fat |
| **Carrot** | Vitamin A, fiber, root storage | Root crop, long storage |

(6 plants ≈ complete nutrition. Final list/balance is the operator's call, this is a
defensible default.)

## What exists vs what needs building

- **Exists (mostly unregistered, see the lint):** farming, asteroid+mining, crafting,
  food, medical, atmosphere, hydrology, weather, electrical, vehicles, construction,
  economy, logistics, AI/autonomy.
- **Needs building:** the **loops** that connect them, **content entities** (spawned
  crops/asteroids/drones), the **full-unlock dev provisioning**, the **GUI→ECS command
  bridge** (so UI buttons drive real ECS actions), and the new glue: the **drone task**,
  **nutrition effects/buffs**, and the **real crafting consume/produce**.

## Proposed build order

1. **✅ DONE (v0.329.0), Full-unlock dev provisioning**, "Dev: stock all materials" stocks one stack of every recipe input (raws + intermediates); the GUI→ECS command bridge it needs (GuiState flag → main-loop DataStore Mutex channel → owning System drains in its tick) is reused everywhere. *(testing enabler)*
2. **✅ DONE (v0.329.0), Real crafting loop**, Craft button → CraftingSystem consume inputs / produce outputs → inventory updates live. *(first end-to-end playable loop)*
3. **✅ DONE (v0.330.0), Cooking + nutrition**, `Vitals` (satiation/hydration) + `StatusEffects` components; `FoodSystem` registered & extended: eat (Eat button → consume bridge) restores satiation/hydration from `food_system.ron` nutrition profiles, raw food rolls `raw_consumption_risk` → `food_poisoning`, a full meal → `well_fed`; hunger/thirst decay → `hungry`/`thirsty` conditions → starvation/dehydration health drain; timed effects expire. `StatusEffectRegistry` (status_effects.csv) keeps durations/modifiers in data. Cooking = the existing crafting recipes (cook_soup etc.) producing safer, more-satiating food. Inventory page shows satiation/hydration bars + active-effect chips.
   - **#3b, partly DONE (v0.334.0), rest tracked:** (a) **status-effect *modifier consumption***, ✅ **SPEED done (v0.334.0):** the camera's `speed_multiplier` is set each frame from the player's active effects' `speed:X:multiply` mods (new `well_nourished` buff = +10% from a good meal; `thirsty`/`flu` = −20%), so movement buffs/debuffs are now tangible (`StatusEffectRegistry::net_stat_multiplier`). Still deferred: `stamina_regen` mods (need a stamina system) + `vision_range`/night_vision (need renderer wiring). (b) **night_vision from vitamins**, needs vitamin modeling in `NutritionProfile` + the vision wiring. (c) **✅ Drink action (v0.339.0):** drink items (subcategory drink/liquid + water_*) get a Drink button → FoodSystem restores hydration (+30), mirroring Eat. Hydration is now symmetric with satiation. (d) **spoilage → nutrition**, spoiled food should lose nutrition / risk illness (FoodSystem tracks spoilage but doesn't yet apply it on eat). (e) **data-driven item→profile link**, `FoodSystem::profile_id_for` classifies by prefix (mirrors the existing spoilage classifier); a column on items.csv would make it fully data-driven.
4. **✅ DONE (v0.331.0), Gardening loop**, PLANT (a seed item's "Plant" button → consume the seed → spawn a CropInstance), GROW (FarmingSystem already advances stage from game_time + water/health), WATER + HARVEST (mature crop → yield produce per plants.csv yield_min/max into the inventory → despawn), all via the GUI→ECS bridge. The inventory page gains a **Garden panel** listing crops (stage / growth / water / health) with Water + Harvest buttons, plus a **"Dev: grow all"** testing affordance (skip the game-day wait). Mapping is `seed_<id>_0 → <id> → vegetable_/fruit_/grain_<id>_0`, validated against item_registry. Closes the chain: garden → harvest → cook/eat (#3).
   - **#4b deferred (tracked):** a data-driven `harvest_item` column on plants.csv (124 of 129 plants have no produce item in items.csv yet, so only the ~5 with matching items currently yield on harvest); soil/water-source entities + tilling/irrigation; 3D crop placement + visuals (crops are logical entities now, not yet placed/rendered in the world).
5. **✅ DONE (v0.332.0), Drone↔asteroid mining**, new `AsteroidBody` (finite multi-ore entity) + `Drone` (Outbound→Mining→Returning→Done state machine) components + a registered `DroneSystem`. Commission a drone for an ore (Mining panel button → DroneSystem picks an asteroid that has it; home = the player) → it spends OUTBOUND+MINING+RETURNING seconds → delivers the mined ore into the player inventory; an asteroid mined to empty is **DELETED** (the operator's "consumed → deleted"). Mining mutations use intent-collection (no ECS borrow conflicts). Added `nickel_ore_0`/`platinum_ore_0` items + 2 test asteroids (iron/nickel/platinum/copper). The inventory page gains a **Mining panel**: asteroids + remaining ore, a commission button per available ore, and an active-drone list (ore/phase/cargo). Proven by `mining::drone_tests`.
   - **#5b deferred (tracked):** server-authoritative MMO asteroids (relay `GameWorld`, currently single-player native ECS) + the swarm + **abandoned**-deletion; 3D asteroid voxel visuals (`terrain::asteroid` octree exists, unused by the loop) + actual drone flight/position; refining nickel/platinum (smelt recipes) so the mined ore feeds the crafting tree.
6. **✅ DONE (v0.333.0), Refining chain depth**, the ore mined in #5 is now refineable. Added `nickel_ingot_0` / `platinum_ingot_0` / `stainless_steel_ingot_0` items + `smelt_nickel` / `smelt_platinum` / `smelt_stainless` recipes (smelter_0). **2-tier depth:** ore → ingot (smelt_nickel) → alloy (smelt_stainless consumes iron_ingot **+** nickel_ingot). Closes **mine → refine → craft**. Locked by `crafting::refining_chain_tests` (recipes exist + every input/output is a real item + the multi-tier dependency holds). Pure data + a test, the crafting mechanism (#2) already executes any recipe.
   - **#6b deferred (tracked):** deeper trees (platinum → catalysts/electronics; more alloys), recipe **byproducts** (the schema supports them, data missing), chemistry→crafting links, the broad DB expansion the 2026-05-29 audit flagged, now meaningfully consumable because crafting/refining actually run.
7. **IN PROGRESS, Survival systems online.** **✅ #7a energy/rest (v0.335.0):** `energy` on `Vitals` drains while awake → `fatigued` (−15% speed, #3b) below 25% → a **Rest** button refills it. **✅ #7b environment-coupled oxygen + temperature (v0.336.0):** an `EnvironmentContext {sealed, oxygenated, ambient_temp_c}` is computed each frame from the player's position vs the sealed homestead AABB (encompassing all rooms, stored in EngineState) → FoodSystem drains oxygen in vacuum (`hypoxia` → `suffocation`) + drifts body temp toward ambient when exposed (`hypothermia`/`heat_exhaustion`), with Health loss; re-entering the homestead recovers. New `oxygen` + `body_temp_c` on Vitals; new effects + reused `hypothermia`; inventory panel shows an oxygen bar + body-temp + a Sealed/EXPOSED indicator. Also made **hunger tangible** (was inert `stamina_regen` → `speed:0.9`, like thirst). Walking outside the homestead is now a real threat, the space/asteroid-survival groundwork. **✅ WeatherSystem registered (v0.337.0):** season-driven weather (temperature + conditions) is exported to the DataStore via a Mutex; the exposed-environment ambient temperature now USES it (winter / storms → colder outside → faster hypothermia), and the weather HUD bridge works. First of the deferred sim systems off the lint list with a real consumer. **✅ #7c sanitation/compost (v0.338.0):** organic `waste` (a Vitals field) accrues while living + when eating → an `unsanitary` debuff above 75% → a **Compost** button (vitals panel) turns waste into `fertilizer_0` and clears it → a **Fertilize** button on crops consumes fertilizer to boost crop health (and growth is health-weighted, so it grows faster). Closes the **food → waste → compost → soil → food** cycle (a real survival skill, fits the mission). **All five listed survival needs (nutrition / energy / oxygen / temperature / sanitation) are now live.** **Remaining #7:** register atmosphere/hydrology/ecology/disasters as their content + consumers appear (don't un-defer cosmetically). (**#7b-tail:** real per-zone enclosed-space/airlock volumes + suit/clothing insulation, instead of the single homestead AABB; tie body-temp into RoomEnvironment/HVAC; space-vs-planet exposure model.)
8. **✅ DONE (v0.340.0–v0.342.0), Progression layer.** skills/XP/quests/tech-unlock turn the working systems into a goal-driven game. **✅ #8a skills + XP foundation (v0.340.0):** the `skills/` scaffold (SkillDef/SkillProgress/PlayerSkills/SkillRegistry/SkillSystem, exponential `xp_base·level^1.5` curve) was 90% built but unwired, now wired end-to-end. `SkillRegistry::from_csv` loads the 22-skill `data/skills/skills.csv`; a `PlayerSkills` component spawns on the player; `SkillSystem` is registered LAST and drains a shared `"xp_grants"` DataStore channel that the action systems push to (the decoupled path, they hold `&DataStore`, not `&mut SkillSystem`). XP sources wired: **craft → the recipe's skill** (10 + skill_level·5), **harvest → farming**, **mine-deliver → mining**. Live levels + XP render in the existing profile **Skills** panel (was static defaults). **Data-integrity fix (no-bandaid):** recipes.csv `skill_required` used a non-canonical vocabulary (smithing/textile/medical/chemistry/crafting/construction) that matched NO skill in skills.csv → XP would have silently no-op'd on the registry miss. Reconciled all 235 rows to canonical skill ids via a category-aware remap (leather→tailoring, electronics→engineering, medical→medicine, …); the `Recipe` struct now parses `skill_required`+`skill_level` (was dropped). A new unit lint `skills::skill_tests::every_recipe_skill_is_a_real_skill` makes the two vocabularies impossible to drift apart again. SkillSystem off the engine-wiring deferred list; `skill_registry` is now required runtime data. Proven by 4 tests (registry parse, channel→level-up, the drift lint, end-to-end craft→XP).
   - **✅ #8b tech-unlock (v0.341.0):** skills now GATE crafting, the payoff that makes leveling matter. `CraftingSystem` rejects a craft when the crafter's `PlayerSkills.level(skill) < recipe.skill_level` (AUTHORITATIVE, enforced system-side via `meets_skill_requirement`, not just greyed in the GUI; an entity without PlayerSkills, e.g. an NPC/bot, isn't blocked). The crafting page shows "Requires {skill} Lv N (you: Lv M)" per recipe (danger colour when unmet) + locks the Craft button (distinct "Skill level too low" vs "Missing ingredients"); `GuiRecipe` + `load_crafting_recipes` now carry `skill_required`/`skill_level`. A **Dev: max skills** button (profile Skills panel → `dev_max_skills` channel → SkillSystem maxes every skill to its `max_level`) preserves the "develop as if 100% unlocked" testing posture under the gate. Proven by `crafting::skill_xp_tests::skill_gate_blocks_under_level_then_allows` + `skills::skill_tests::dev_max_skills_maxes_every_skill` (382 tests, relay clean).
   - **✅ #8c quests (v0.342.0):** the `quests/` scaffold (QuestDef/QuestRegistry/ActiveQuest/QuestTracker/QuestSystem + `data/quests/*.ron`) wired end-to-end. `QuestRegistry::from_ron_dir` loads every `data/quests/*.ron` (each a `Vec<QuestDef>`); `QuestSystem` registered LAST; a `QuestTracker` spawns on the player auto-accepting the **Getting Started** chain (`data/quests/getting_started.ron`, satisfiable Gather/Craft objectives; the existing tutorial/farming/construction chains use Build objectives that need the still-deferred ConstructionSystem, so they load but can't complete yet). Count-based objectives (Craft/Harvest) advance via a shared `quest_events` channel the action systems push to on completion (`craft_<recipe>` / `harvest_<crop>`, the same hook as the XP award); Gather objectives check live inventory. Completing a quest grants its item rewards + **auto-accepts dependents** (prerequisite chaining). A **Quests** section on the profile page shows active steps (with a progress bar) + completed quests. Proven by `quests::quest_tests` (RON dir load, Gather→complete+reward, craft-event→complete→chain). **Bonus fix:** a #8b fresh-player **deadlock**, a new player is level 0 in every skill and the only metalworking-XP source is crafting metalworking recipes, so gating level-1 recipes made each skill un-bootstrappable. Level-1 recipes are now the free **starter tier**; gating begins at level 2.
   - **#8 deferred (tracked):** **#8c-tail**, the relay runs a SEPARATE authoritative quest chain for MMO; the native QuestSystem drives single-player. Reconciling which is authoritative when connected to a server (+ a quest-accept UI for non-auto quests) is deferred. Also: per-recipe skill refinement is coarse for the old `crafting` grab-bag (defaulted by category); skill **passive_bonuses**/**unlocks**/**synergy**/**decay** (schema-documented, data-file TBD); a dedicated Progression page if the profile panel outgrows its home.

Each step: register/wire the relevant system(s), spawn the content, verify with an
automated test where possible + the operator playing. The engine-wiring lint tracks which
systems have come online.
