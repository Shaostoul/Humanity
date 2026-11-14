# Activities: every minigame that makes up the one game

**Status: catalog and recommendations, 2026-11-14.** Two things here are the operator's decisions,
quoted in section 6 (hunting and butchering are in; the order principle of section 4 is
confirmed). Everything else (the ten-item ordering within that principle, the skill changes, the
answers to the open questions) is this pass's recommendation (an agent's), marked as such. The data is
[`data/gameplay/activities.json`](../../data/gameplay/activities.json), one entry per activity;
[`tests/activities_catalog_lint.rs`](../../tests/activities_catalog_lint.rs) holds every claim in it
to something checkable (every evidence path exists, every skill is a row of
`data/skills/skills.csv`, every status is one of four, every live entry names a source file and a
FEATURES.md heading, every id is named in this document).

The operator, 2026-11-14: "I guess something we need to be thinking of, what are all of the
minigames that make up our one game? Like, mining is a mini game. Cooking is a mini game. Building
structures and vehicles are mini games. So forth."

Companions: [engagement-modes.md](engagement-modes.md) (one simulation, five ways to engage),
[progression-skills-gear.md](progression-skills-gear.md) (skills, and their 20-row budget),
[gameplay-loop-map.md](gameplay-loop-map.md) (the tiers and the closure ladder; its current-state
columns are a stale snapshot, so nothing below takes status from it),
[two-realities.md](two-realities.md) (every domain exists twice),
[educational-gameplay.md](educational-gameplay.md), [quest-chains.md](quest-chains.md) (the
survival chains that teach most of the body and food activities).

## 0. In plain words

- The game is one world, and inside it there are about ninety things a person can *do*: grow food,
  cook it, keep animals, mine, smelt, build, wire a home, drive, look after their body, trade,
  vote, make music. Each of those is an **activity**. The operator calls them minigames.
- **93 activities** are catalogued in nine families. **26 are live** (you can do them now and a
  simulation decides the result), **35 partial** (some of it runs, usually as a menu click instead
  of the real procedure), **15 designed** (a design document plans it) and **17 ideas**.
- No activity is compulsory. Each one is the hands-on way to meet a need; a person who would rather
  not play it can buy the result, automate it, share it with a group, or have it handled in the
  background.
- The best next activities are the ones that **use up something the game already makes**: cooking
  and preserving for the garden's harvest, slag uses for the smelter's waste, repair for the parts
  the workshop turns out. Section 4 lists ten, in order.
- Eight of the twenty skills can never rise today, because nothing trains them, and one skill
  (engineering) is asked to cover fourteen activities. Section 5.
- The operator decided on 2026-11-14 that hunting and butchering are in ("They're too valuable to
  leave out") and confirmed the order. Combat and point defense's targets stay marked **operator
  to decide**; the catalog does not decide them (section 6).

## 1. What an activity is

An activity is the **Direct mode** of a domain ([engagement-modes.md](engagement-modes.md)): the
hands-on way to meet a need that the simulation tracks whether or not you play. Cooking is the
Direct mode of food preparation; the mess hall's free crew meal is its Background mode, the trading
post its Trade mode. That is what lets the game hold ninety activities without forcing anyone
through ninety minigames: a person picks the few they enjoy and meets the rest of their needs
another way, and the simulation underneath is the same either way.

So an activity is not a separate game bolted on. Every one runs on the one simulation (the same
food system, the same power sim, the same inventory), consumes something another activity
produces, and produces something another consumes. Its job is to make one part of that simulation
something you do with your hands and learn from.

## 2. The anatomy every activity shares

Every activity, built or not, answers the same nine questions. The data file has a field for each,
so a page or a quest can read them.

| Part | What it means | Field in the data |
|---|---|---|
| **The real procedure is the interaction** | You learn the real skill by doing it the real way: knead, prove, bake; heat, strike, quench; read the soil test, choose the fix. A menu click is the placeholder, not the activity (CLAUDE.md, "Realistic-first"). | `status` is `partial` while the procedure is a click |
| **The simulation decides the result** | The outcome comes from the model (nutrients, heat, wear, chemistry), never from a timer or a dice roll dressed as one. The procedure feeds the model; it does not replace it. | `evidence` (the source files that simulate it) |
| **Two modes: simplified and full realism** | CLAUDE.md, "Every deep system ships two modes". Realistic severity exists; general play defaults to softened. `data/gameplay/levels.json` already holds 15 such settings (Off / Forgiving / Realistic and their kin); an activity names its setting. | `levels_setting` (a setting id in levels.json, or null) |
| **The skill it trains** | Learn by doing ([progression-skills-gear.md](progression-skills-gear.md) pillar 1). Registering a verb requires wiring its XP in the same increment. | `skill` (a row of skills.csv, or null) |
| **Its real-life twin in the app** | [two-realities.md](two-realities.md): the game is the safe practice space, the app is where it counts. Today the twin is mostly a Library guide (the Real Skills curriculum) or a How-to; the Home trackers (diet, fitness, possessions) are planned and not built. | `real_life_twin`, `real_life_twin_path` |
| **The modes that replace it** | Who meets this need without playing it: Trade, Automation, Cooperative, Background. Direct is always present, since an activity IS the Direct mode. | `engagement_modes` |
| **What it consumes and produces** | Which loop it closes. An activity whose output nothing consumes is a faucet into storage; one that consumes what is already piling up is a sink, and sinks come first (section 4). | `consumes`, `produces`, `tier` |
| **Its per-frame and network budget class** | CLAUDE.md, "Compute and bandwidth are a hard budget". `frame`: none (a page or a tick), light (a prompt or a gauge), heavy (per-frame physics or visuals, which must state its ms budget in its design before it is built). `network`: local (never networked, cosmetic simulation included), state (low-rate gameplay state), realtime (positions and inputs). | `budget.frame`, `budget.network` |
| **Who decides whether it exists** | Most activities are engineering. A few are values the operator alone sets. | `operator_to_decide` (the question, or null) |

### Status, and how it was read

- **live**: you can do it in the game today and a simulation decides the result. The entry names a
  source file and the FEATURES.md heading that documents it.
- **partial**: some of it runs (a recipe, a machine, a page, one half of the loop), but the activity
  as designed is incomplete, usually because the real procedure is a menu click.
- **designed**: a design document plans it; no code runs the activity (a scaffold or data may
  exist, and the entry names it).
- **idea**: named here or in passing; no design yet.

Statuses were read on 2026-11-14 from `docs/FEATURES.md` (by heading), `docs/ai/code-map.md`, the
files themselves, and `DEFERRED_SYSTEMS` in `tests/engine_wiring_lint.rs` (which the build
enforces, so it cannot go stale). A system listed there is a scaffold, not live, whatever an older
doc says. Two traps met while reading, worth knowing before trusting a status elsewhere:

- `gameplay-loop-map.md` says so itself: its current-state columns are a July snapshot.
- FEATURES.md still carries four stale entries under Game Systems that contradict the lint: "Combat",
  "AI Behaviors", "Vehicles/Mechs" and "Economy" say "NOT registered", while CombatSystem (v0.760),
  AISystem (v0.761), VehicleSystem and EconomySystem (v0.747) are registered. The accurate entries
  are "Combat (v0.760 - v0.765)" and the Economy Automation ones; this catalog cites those.
- Forage flora and resource nodes (v0.978, v0.982) shipped with no FEATURES.md entry; this pass
  added one so the catalog's evidence has a heading to point at.

## 3. The catalog by family

Counts by family and status:

| Family | Activities | Live | Partial | Designed | Idea |
|---|---|---|---|---|---|
| Body | 6 | 4 | 1 | 0 | 1 |
| Food and water | 15 | 4 | 8 | 1 | 2 |
| Materials and making | 15 | 2 | 8 | 2 | 3 |
| Building and engineering | 14 | 8 | 5 | 0 | 1 |
| Moving and travel | 10 | 1 | 5 | 3 | 1 |
| Science | 6 | 0 | 3 | 2 | 1 |
| Risk and safety | 10 | 2 | 2 | 6 | 0 |
| Community | 8 | 4 | 1 | 0 | 3 |
| Arts and play | 9 | 1 | 2 | 1 | 5 |
| **All** | **93** | **26** | **35** | **15** | **17** |

What changed from the starting list the orchestrator gave: drinking water became its own activity
(`water_safe`, the one the water_purification skill belongs to); fire making moved beside its skill
(`fire_making`, partial: the Campfire ability builds a real fire); mining split three ways (gathering
by hand, mining by drone, and **piloting** a drone, which is the minigame drone-mining.md designs);
composting, seed saving, storage, gathering by hand, swimming, the shuttle trip, exploring, rescue
and revival, contributing to the fleet, talking with people aboard, watching together and sports
were added; milling joined baking; "lab work" became testing soil, water and air (the investigation
pattern engagement-modes.md designs); "leadership and crew" split from guilds and cooperatives. Each
row's full evidence (every path, consumes and produces, twin, modes, budget) is in the data file;
the table carries the first two paths.

<!-- CATALOG TABLES: generated from data/gameplay/activities.json by scripts/activities-tables.js; regenerate rather than edit -->

### Body

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `eating_drinking` Eating and drinking | live | none | FEATURES: "Ship's food stores (ship homes increment 3, 2026-10-04)"; `systems/food.rs`; `src/engine/mess_hall.rs`; design: `quest-chains.md` | The mess hall's free crew meal is the Background mode for food made real; vitals_drain Off is the simplified mode. |
| `sleeping` Sleeping and resting | live | none | FEATURES: "Construction"; `systems/sleep.rs`; `data/blueprints/basic.ron`; design: `quest-chains.md` | A built bed provides rest: E sleeps 8 game hours (FEATURES.md, Construction, 'A built bed and chest do something'). |
| `hygiene_sanitation` Hygiene and sanitation | partial | none | `systems/food.rs`; `data/home_outline.json`; design: `quest-chains.md` | The waste meter and the Compost action that empties it into stored urine run in food.rs; no toilet is placed (home_outline.json says 'not in game yet') and washing is not modelled. Clean Hands is designed in quest-chains.md 2.5. |
| `fitness` Fitness and exercise | idea | none | `data/aging_fitness.ron`; `systems/aging.rs` | aging_fitness.ron authors six fitness levels and exercises that nothing reads (AgingSystem is deferred in tests/engine_wiring_lint.rs). respawn-and-revival.md names conditioning as later work. |
| `staying_warm` Staying warm (and cool) | live | none | FEATURES: "Body heat (2026-09-27)"; `systems/body_heat.rs`; design: `body-heat.md` | Heat made versus heat lost to weather, clothes and shelter; Off / Forgiving / Realistic levels. |
| `first_aid_medicine` First aid and medicine | live | medicine | FEATURES: "Wounds and healing (first-hour audit S8, 2026-10-12)"; `systems/wounds.rs`; `systems/treatment.rs`; design: `quest-chains.md` | Treating a wound gives no medicine XP today; only crafting medical items (12 recipes) does. |

### Food and water

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `water_safe` Finding, purifying and storing water | partial | water_purification | FEATURES: "Tap Water and Water Vessels (fluids as litres)"; `systems/fluids.rs`; `systems/plumbing.rs`; design: `quest-chains.md` | Tap water, vessels and the water purifier machine run; the hands-on chain (boil, disinfect, filter, distil, rain) is designed in quest-chains.md 2.1. The water_purification skill has no XP source. |
| `gardening_farming` Gardening and farming | live | farming | FEATURES: "Farming"; `systems/farming/mod.rs`; `systems/farming/soil_ph.rs`; design: `survival-garden.md` | The game's proudest loop. Four garden levels (pests, pollination, soil pH, picking) are its simplified and realistic modes. |
| `mushrooms` Mushrooms and fruiting tents | live | farming | FEATURES: "Farming"; `src/engine/fruiting_tent.rs`; `data/garden/grow_media.ron`; design: `survival-garden.md` | Racks of fruiting blocks under a fitted fruiting tent; part of the farming system. |
| `beekeeping` Beekeeping | partial | farming | FEATURES: "Farming"; `systems/farming/pollination.rs`; `data/garden/pollination.ron` | A placed bumblebee hive with a working colony pollinates indoor crops; honeybees, honey and wax do not exist. |
| `livestock` Keeping animals | live | farming | FEATURES: "Livestock (v0.751)"; `systems/livestock.rs`; `systems/animal_care.rs`; design: `gameplay-loop-map.md` | Realistic (animals sicken and die) and Forgiving (production only) levels. Products are renewable only (eggs, milk, wool); slaughter for meat is in scope since the operator's hunting and butchering decision (2026-11-14, activities.md section 6) and is not built. |
| `foraging` Foraging wild food | partial | foraging | FEATURES: "Forage flora and resource nodes (v0.978, v0.982)"; `data/entities/wild_spawns.ron`; `systems/livestock.rs`; design: `quest-chains.md` | Berry bushes and wild flax regrow and collect by walk-up E, but the collect trains farming, not foraging, and nothing teaches identification or lookalikes yet (food_forage in quest-chains.md 2.2). |
| `fishing` Fishing | idea | none | none | In scope under the operator's hunting and butchering decision (2026-11-14, activities.md section 6). Planet shores (ocean.md) are where it would happen. |
| `aquaponics` Aquaponics (fish and plants together) | partial | farming | FEATURES: "Farming"; `data/machines/home.ron`; `systems/grow_machines.rs`; design: `survival-garden.md` | The tank is placed and draws power; grow_machines.rs counts no fish, and survival-garden.md designs its B12, omega-3 and sludge. Harvesting the fish is in scope under the hunting and butchering decision (2026-11-14). |
| `cooking` Cooking | partial | cooking | FEATURES: "Quality Grades for Crafted Goods"; `systems/crafting/mod.rs`; `systems/crafting/quality.rs`; design: `quest-chains.md` | 31 cooking recipes, gated to the stove and oven (v0.749), with quality grades; the cooking methods and meal quality in food_system.ron are loaded and acted on by nothing, and the procedure is a menu click. |
| `baking_milling` Milling and baking | partial | cooking | FEATURES: "Crafting"; `data/recipes.csv`; `systems/crafting/mod.rs` | grain_mill_0 (5 recipes, no skill) and oven_0 (8) recipes such as grind_flour and cook_bread; recipe-menu only. |
| `preserving` Preserving: drying, salting, canning, cold storage | partial | cooking | `data/recipes.csv`; `systems/food.rs`; design: `quest-chains.md` | Spoilage and its temperature zones run (food.rs) and cook_dried_fruit / cook_dried_meat exist; food_system.ron's preservation_methods are loaded and unused, and salting and canning are designed in quest-chains.md 2.2. |
| `fermenting` Fermenting | designed | cooking | design: `quest-chains.md` | food_keep_ferment in quest-chains.md 2.2. |
| `brewing` Brewing (vinegar, kombucha, beer, wine) | idea | cooking | none | No design and no recipe. |
| `composting` Composting | live | farming | FEATURES: "Byproducts and Machine Output to Storage"; `systems/food.rs`; `systems/farming/soil.rs`; design: `survival-garden.md` | The composter machine (6 farming recipes) and the Compost action close waste back to the garden. |
| `seed_saving` Saving seeds and breeding plants | partial | farming | FEATURES: "Farming"; `systems/farming/mod.rs`; `systems/genetics.rs` | A harvest returns seed in proportion to how well it set (picking_tests.rs, seed_return_follows_the_harvest); choosing and breeding traits is a deferred scaffold (GeneticsSystem). |

### Materials and making

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `gathering_by_hand` Gathering wood, stone, clay and ore by hand | partial | mining | FEATURES: "Forage flora and resource nodes (v0.978, v0.982)"; `data/entities/wild_spawns.ron`; `systems/livestock.rs` | Resource nodes collect by walk-up E on a regrow timer. They train farming (the livestock collect bridge), not mining, and there is no hand-mining procedure (pick, chisel, swing). |
| `drone_mining` Mining by drone | live | mining | FEATURES: "Economy Automation Phase 1: the living production chain (v0.663)"; `systems/mining.rs`; `src/engine/drone.rs`; design: `drone-mining.md` | Commission a drone, give standing orders; it flies, mines and returns ore. This is mostly the Automation mode already; the hands-on game is drone_piloting. |
| `drone_piloting` Piloting a mining drone | designed | mining | design: `drone-mining.md` | drone-mining.md: 'Piloted mode is optional': the person takes the controls at the rock; realistic remote piloting depends on distance (light delay). |
| `prospecting` Prospecting and survey | designed | mining | `systems/geology.rs`; design: `drone-mining.md` | drone-mining.md: a rock must be surveyed first, by the fleet's sensors or a visiting craft. GeologySystem is a deferred scaffold. |
| `smelting_refining` Smelting and refining (with its slag) | live | metalworking | FEATURES: "Economy Automation Phase 1: the living production chain (v0.663)"; `data/recipes.csv`; `data/machines/home.ron`; design: `slag.md` | 18 smelting and 38 refining recipes; the smelter runs smelt_iron on its own as an AutoRefine machine. slag.md designs slag's every use (the endless sink); none is built. |
| `smithing` Smithing and metalworking | partial | metalworking | FEATURES: "Hand Tools and Tool Wear"; `data/recipes.csv`; `systems/crafting/tools.rs` | forge_0 (39) and anvil_station_0 (5) recipes with tool wear and quality grades; the procedure (heat, strike, quench) is a menu click. |
| `welding` Welding and joining metal | idea | metalworking | `data/items.csv` | A welding torch item exists (items.csv) and no recipe or tool rule uses it; engine-architecture.md's education table names welding as a taught skill. |
| `woodworking` Woodworking | partial | woodworking | FEATURES: "Hand Tools and Tool Wear"; `data/recipes.csv`; `systems/crafting/tools.rs` | 45 recipes trained by woodworking (workbench, sawmill, hand); menu crafting. |
| `tailoring_weaving` Spinning, weaving and tailoring | partial | tailoring | FEATURES: "Crafting"; `data/recipes.csv` | 39 textile recipes on the loom and the sewing machine (tan_leather among them); menu crafting. Clothes feed staying_warm. |
| `pottery` Pottery and ceramics | partial | none | FEATURES: "Crafting"; `data/recipes.csv`; design: `quest-chains.md` | kiln_0 has 9 recipes trained by three different skills (metalworking, woodworking, shelter_building) or none; water_filter_ceramic is designed in quest-chains.md 2.1. |
| `glassmaking` Glassmaking | partial | metalworking | FEATURES: "Crafting"; `data/recipes.csv`; design: `slag.md` | smelt_glass, draw_glass_fiber and craft_fiberglass exist; slag.md designs slag into glass and wool. |
| `chemistry` Chemistry: soap, medicines, fuels, compounds | partial | alchemy | FEATURES: "Chemistry Database"; `data/recipes.csv`; `data/chemistry`; design: `ingredients-and-labels.md` | 29 chemistry recipes, 14 on the chemistry set, trained by medicine, engineering, alchemy (3) or none. The skill is still called Alchemy. |
| `recycling_salvage` Recycling and salvage | idea | none | none | No recipe takes a finished item apart. closed-loop-test.md and homestead-solo-design.md recycle water and air, not goods. |
| `fabrication_printing` Fabrication and 3D printing | idea | engineering | `data/recipes.csv` | build_3d_printer makes a printer item; nothing prints with it. |
| `electronics` Electronics | partial | engineering | FEATURES: "Crafting"; `data/recipes.csv` | 35 electronics_bench_0 recipes; menu crafting, no circuit is built or tested by hand. |

### Building and engineering

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `construction` Construction | live | shelter_building | FEATURES: "Construction"; `systems/construction/mod.rs`; `src/engine/build_place.rs`; design: `construction-architecture.md` | Blueprint, ghost, timed build, a Structure with what it provides; construction gives shelter_building XP. |
| `home_layout` Home layout and interior design | live | shelter_building | FEATURES: "HomeStructure Model + the Ship Superstructure (v0.766 - v0.769)"; `ship/home_structure.rs`; `src/engine/editor.rs`; design: `home-design.md` | The build editor: interior walls, openings, door panels and locks, lights, storeys. |
| `electrical` Electrical power and wiring | live | engineering | FEATURES: "Live Electrical System"; `systems/electrical.rs`; `systems/solar.rs`; design: `utility-wiring.md` | Supply, demand and battery state of charge; wires route through conduits in the build editor. |
| `plumbing` Plumbing | live | engineering | FEATURES: "Live Water / Plumbing Sim + Power Coupling (v0.608)"; `systems/plumbing.rs`; design: `spline-routing.md` | Per plumbing island, coupled to power; pipes carry materials and marker bands. |
| `climate_control` Heating and cooling a home | live | engineering | FEATURES: "Farming"; `systems/farming/heat.rs`; design: `body-heat.md` | Electric heaters, a heat pump and a certified wood stove, built at a workbench (FEATURES.md, under Farming, 'Heaters'). |
| `life_support` Air and life support | live | engineering | FEATURES: "Live Air / Atmosphere Sim + power -> air -> Vitals (v0.617 - v0.618)"; `systems/atmosphere.rs`; `systems/life_support.rs`; design: `ship-life-support.md` | Station supplied (Background) or Realistic levels. |
| `data_networking` Data cabling and networks | partial | engineering | FEATURES: "Data / Telecom Media, Stage 1 (v0.619)"; `crates/humanity-core/src/utilities.rs`; design: `telecom.md` | Stages 1 to 3 shipped (media, link physics, routing); nothing in the game needs a working link yet, and stage 4 (detection) is designed. |
| `pulleys_rigging` Pulleys and rigging | partial | engineering | FEATURES: "Pulleys: the block-and-tackle designer (native, rung 1, 2026-11-13)"; `systems/pulley/mod.rs`; `data/pulleys/pulleys.ron`; design: `pulleys.md` | Rung 1 built the model, the data, the Calculator mode and the geometry; rigging a block in the world is a later rung. |
| `vehicle_building` Building vehicles | partial | engineering | FEATURES: "Economy Automation Phase 2, Stage 1: Vehicle Kits (v0.677)"; `systems/vehicles/mod.rs`; `data/vehicles/kits.ron` | A kit is crafted and deploys into a real vehicle; there is no part-by-part assembly. |
| `ship_building` Building ships | idea | engineering | none | mothership-master-plan.md plans the fleet's ships and slag.md's per-ship vote on armour; no player shipyard is designed. |
| `running_machines` Running machines | live | engineering | FEATURES: "Machine Walk-Up Cards (live)"; `src/gui/pages/hud.rs`; `data/machines/home.ron` | Walk-up cards with live values and an Auto-build recipe picker; the machines then run on their own. |
| `maintenance_repair` Maintenance and repair | partial | engineering | FEATURES: "Hand Tools and Tool Wear"; `systems/crafting/tools.rs`; `systems/abilities.rs`; design: `game/construction.md` | Hand tools wear and break; the Repair ability heals the player's target. Machines and structures do not wear, so nothing needs repairing. |
| `hauling_logistics` Carrying, hauling and logistics | partial | none | FEATURES: "Carrying weight (2026-10-04, BUG-136)"; `systems/encumbrance.rs`; `src/engine/carry_load.rs`; design: `ship-homes-and-logistics.md` | Carrying weight slows and stops an overloaded body; goods physically moved between homes and the fleet are designed in ship-homes-and-logistics.md. |
| `storage` Storage and containers | live | none | FEATURES: "Typed Containers (volume-capped vessels)"; `systems/inventory/mod.rs`; `data/containers`; design: `containers.md` | Volume-capped typed vessels, wrong-class damage, visible racks and tank gauges. |

### Moving and travel

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `walking_climbing` Walking, climbing and falling well | live | none | FEATURES: "Ladder Climb (v0.589)"; `systems/surface_move.rs`; `systems/falls.rs`; design: `gravity-and-movement.md` | Stairs, ramps, ladders, elevators, teleporters; falls hurt by how hard a body lands. |
| `swimming` Swimming | partial | none | FEATURES: "Carrying weight (2026-10-04, BUG-136)"; `systems/surface_move.rs`; design: `ocean.md` | A swimming movement mode exists on planet surfaces; breath, cold water and drowning are not modelled. |
| `driving` Driving | partial | none | FEATURES: "Economy Automation Phase 2, Stage 3 slice 1: Vehicles Move (v0.680-v0.682)"; `src/lib.rs`; `systems/vehicles/mod.rs` | Get into a cab and drive (src/lib.rs, driving_vehicle) or summon a vehicle; arcade movement, no fuel use. |
| `rail_transit` Rail and transit aboard | partial | none | FEATURES: "Train / Rail Line (v0.592)"; `ship/transit.rs`; `ship/structure.rs`; design: `game/ship_zoning_transit.md` | Train platforms pair into a rail line and transit links move a person aboard; the rail cars are render-only. |
| `shuttle_trip` The shuttle down to a planet and back | designed | none | design: `leaving-the-ship.md` | A proposal: booking, boarding and the trip phase by phase (section 4). |
| `piloting` Piloting spacecraft | idea | none | `systems/flight.rs` | flight.rs keeps the fleet's flight state and the felt gravity; nobody flies a craft. leaving-the-ship.md makes the player a shuttle passenger. |
| `docking` Docking and station-keeping | designed | none | `systems/docking.rs`; `data/docking.ron`; design: `leaving-the-ship.md` | DockingSystem (the airlock state machine) is a deferred scaffold. |
| `zero_g_eva` Zero-g movement and spacewalks | designed | none | `data/status_effects.csv`; design: `gravity-and-movement.md` | The operator's direction (2026-09-15): grappling hooks, jet packs, mag boots, ropes, ladders. The zero_gravity status effect row exists. |
| `navigation` Navigation by map and by stars | partial | navigation | FEATURES: "Maps Page"; `src/engine/map_view.rs`; `systems/quests/mod.rs`; design: `game/celestial_navigation.md` | The map and Travel quest steps exist; no compass, sextant or star fix, and the navigation skill has no XP source. |
| `exploring` Exploring and sightseeing | partial | navigation | FEATURES: "Quests"; `data/quests/exploration.ron`; `data/entities/destinations.ron` | Exploration quests walk to destinations (Travel objectives). |

### Science

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `geology` Geology | idea | mining | `systems/geology.rs`; `data/geology.ron` | GeologySystem is a deferred scaffold. |
| `botany_identification` Botany and identifying plants | designed | foraging | `data/plants.csv`; design: `quest-chains.md` | Wild Food, Carefully (food_forage, quest-chains.md 2.2) designs lookalikes; species data needs its refutation pass before it reaches a screen. |
| `astronomy` Astronomy and stargazing | partial | none | FEATURES: "Stars Renderer"; `src/renderer/stars.rs`; `systems/astronomy.rs` | The real star catalog is in the sky to look at; telescopes are a deferred scaffold (AstronomySystem). |
| `testing_lab` Testing soil, water and air | designed | none | `systems/farming/soil_ph.rs`; design: `engagement-modes.md` | engagement-modes.md's investigation pattern (soil sample, why it matters, cheapest fix) is the design; soil pH and nutrients are simulated underneath. |
| `weather_reading` Reading the weather | partial | none | FEATURES: "Weather System"; `systems/weather.rs`; `systems/precipitation.rs`; design: `environment-fields.md` | The weather is simulated and the HUD line reads the air you stand in; forecasting from clouds, wind and pressure is not an activity yet. |
| `ecology` Ecology and pest management | partial | farming | FEATURES: "The garden's and the ship's gameplay levels (2026-11-12)"; `systems/farming/pests.rs`; `systems/farming/weeds.rs` | Garden pests, diseases and weeds run; wider ecology (populations, contagion) is the deferred EcologySystem. |

### Risk and safety

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `firefighting` Fire safety and firefighting | designed | none | `systems/fire.rs`; `data/fire_system.ron`; design: `fire-props-and-combustion.md` | FireSystem (spread and suppression) is a deferred scaffold; smoke alarms are designed in quest-chains.md 2.3 (warm_alarm). Fire visuals run locally, never networked. |
| `fire_making` Making and tending a fire | partial | fire_making | FEATURES: "Abilities (v0.753)"; `systems/construction/fires.rs`; `systems/abilities.rs`; design: `quest-chains.md` | The Campfire ability builds a campfire that burns down, takes a log on E and warms the body; it needs no skill and gives no fire_making XP. Making a Fire is designed in quest-chains.md 2.3. |
| `damage_control` Damage control and hull breaches | designed | engineering | `src/gui/pages/dev_music.rs`; design: `engine-architecture.md` | engine-architecture.md section 6 (Hull Breach Simulation) is the design; only the music's breach alarm exists (the Dev page's knob). |
| `point_defense` Point defense against debris | designed, operator to decide | none | `data/machines/home.ron`; design: `game/turret-defense-minigame.md` | A marker defense_turret sits on the garage hull. Build, maintain, operate. |
| `radiation_shielding` Radiation and shielding | designed | engineering | design: `slag.md` | slag.md designs slag and rock as shielding mass, with a player vote per ship on armour. |
| `disaster_response` Disaster response | designed | none | `systems/disasters.rs`; `systems/weather_events.rs`; design: `weather-water-roadmap.md` | The extreme-weather event registry exists (weather-water-roadmap.md section 5) and DisasterSystem is a deferred scaffold; nothing asks the player to respond. |
| `wildlife_safety` Living with wildlife | live | none | FEATURES: "Combat (v0.760 - v0.765)"; `systems/ai/mod.rs`; `systems/livestock.rs`; design: `gameplay-loop-map.md` | Predators hunt prey and the player and livestock flee them; the home is a safe zone and wolves are out of its wild spawns. |
| `combat` Combat | live, operator to decide | melee | FEATURES: "Combat (v0.760 - v0.765)"; `systems/combat/mod.rs`; `data/equipment.csv`; design: `gameplay-loop-map.md` | Melee swing, offensive abilities, armour per damage type, loot; kills train melee or ranged. gameplay-loop-map.md calls combat a risk layer serving survival, not a genre. |
| `hunting_butchering` Hunting and butchering | partial | none | FEATURES: "Combat (v0.760 - v0.765)"; `systems/combat/mod.rs`; `data/creatures.csv` | IN (operator, 2026-11-14; his words in activities.md section 6). How graphic it looks follows CLAUDE.md's dual-modes rule. Today a player's killing blow rolls the creature's loot (a wolf: raw meat, fur, bone) into the pack; there is no butchering procedure. |
| `rescue_revival` Rescue and revival | designed | medicine | `systems/death_pack.rs`; design: `respawn-and-revival.md` | Death and the left pack are live (death_pack.rs); reviving someone else is the design in respawn-and-revival.md. |

### Community

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `trading` Trading | live | trading | FEATURES: "Trade Page"; `src/gui/pages/trade.rs`; `src/gui/pages/vendor.rs`; design: `economy_model.md` | The trading post vendor, escrow trades that move real items, and the market. The trading skill has no XP source. |
| `fleet_contribution` Contributing to the fleet | live | none | FEATURES: "Fleet ledger (2026-10-04)"; `src/gui/pages/fleet_ledger.rs`; `src/engine/fleet.rs`; design: `ship-homes-and-logistics.md` | Each player's used and contributed ledger on the relay; the fleet's supply is unlimited in early development. |
| `governance` Voting and governance | live | leadership | FEATURES: "Native Governance page: live feed + signed voting + proposal form (v0.660)"; `src/gui/pages/governance.rs`; design: `laws.md` | The in-game ship vote (slag armour per ship, slag.md) is designed on top of it. |
| `guilds_cooperatives` Guilds and cooperatives | partial | leadership | FEATURES: "Guilds Page"; `src/gui/pages/guilds.rs`; design: `gameplay-loop-map.md` | The Guilds page browses and joins real guilds; guild production pools (the Cooperative mode) are designed in gameplay-loop-map.md T4. |
| `conversation` Talking with people aboard | live | none | FEATURES: "Embeddable AI, rung 3: the talk card, written for everyone and AI for those who chose it (2026-11-12)"; `src/engine/talk.rs`; `data/dialogues.ron`; design: `embeddable-ai.md` | Written dialogue for everyone; an AI brain only for those who chose to download one. |
| `teaching` Teaching | idea | none | none | educational-gameplay.md names cooperative skill application; nothing lets one player teach another. |
| `leadership_crew` Leading a crew | idea | leadership | `data/npc` | Relay crew NPCs follow a data-driven chore rotation (FEATURES.md, Crew Chore AI); nobody directs them. |
| `events_gatherings` Events and gatherings | idea | none | none | The recreation room exists aboard (data/machines/ship.ron); nothing schedules a gathering in the game. |

### Arts and play

| Activity | Status | Skill | Evidence | Note |
|---|---|---|---|---|
| `music` Music | partial | none | FEATURES: "The Music page: explore, tune, keep, export and see the music (2026-11-13)"; `crates/humanity-music`; `src/gui/pages/music`; design: `music.md` | The score is composed live and the Music page lets a person explore, tune, keep and export pieces. Playing an instrument by hand in the world is not in music.md. |
| `fire_props` Fire props and flow arts | designed | none | design: `fire-props-and-combustion.md` | 1:1 fire staff, poi, fire axe and darts with real fire, smoke and gases; nothing is built. |
| `painting` Painting and drawing | idea | none | `systems/creative_arts.rs`; `data/creative_arts.ron` | CreativeArtsSystem (works progressing at a rate per day) is a deferred scaffold; painting by hand is not designed. |
| `photography_film` Photography and film | partial | none | FEATURES: "Live feeds and in-game cameras on screens (in-world screens rung 3, 2026-09-16)"; `src/engine/screens.rs`; `src/gui/pages/studio.rs`; design: `in-world-screens.md` | In-game cameras feed in-world screens; a player camera or photo mode is not built. |
| `writing` Writing | idea | none | none | Nothing writable exists in the world. |
| `dance` Dance | idea | none | none | Needs the character animation rungs (character-animation.md). |
| `watching_together` Watching together on in-world screens | live | none | FEATURES: "Video on in-world screens (native, in-world screens rung 5 integration, 2026-09-16)"; `src/engine/screens.rs`; design: `in-world-screens.md` | Video and the readable web play on walls; YouTube, Twitch and Rumble wait on the embedded browser (embedded-browser.md). |
| `screen_games` Games played on in-world screens | idea | none | `src/engine/screens.rs` | The screens exist to play on; no in-world game (board, card, arcade) does. |
| `sports` Sports and physical games | idea | none | none | Not designed; would ride the movement and physics already built. |

<!-- END CATALOG TABLES -->

## 4. The suggested order (the principle confirmed by the operator, 2026-11-14)

**Confirmed.** The orchestrator put the principle to the operator on 2026-11-14: activities that use
up something already produced come first (cooking uses the harvest, repair uses tool wear,
smelting uses drone-mined rock), before brand-new chains like glass or pottery. The operator's
answer, verbatim: "Sounds fine. They all need to be developed eventually." The ten items below are
this pass's ordering within that principle.

The closure ladder's rule ([gameplay-loop-map.md](gameplay-loop-map.md)): stakes before content,
**sinks before faucets**, one-player economy before community, reuse before new systems. Most of
its ten rungs have since shipped (death and recovery, construction, credits and the vendor, quest
repair, market and trade wiring, much of "environment matters", creatures and combat, abilities,
guilds). What is still open is mostly the same rule one level down: the game makes plenty and too
little of what it makes gets *used*. So the activities that come first are the ones that consume
something already produced, then the ones that make a dead skill trainable, then new chains.

1. **Cooking as a real activity** (`cooking`, partial). The sink for the whole T1 loop: 132 plant
   species, eggs, milk and mushrooms already arrive, and cooking is a menu click while
   `food_system.ron`'s cooking methods and meal quality are loaded and acted on by nothing. Station
   gating and quality grades exist; the procedure (heat, time, doneness) and the nutrition it
   changes are the increment. quest-chains.md's Cook It Through is the teaching chain. The cooking
   skill already trains, so this deepens a live loop rather than starting one.
2. **Every live verb trains its own skill** (a wiring sweep, not an activity). Eight skills have no
   XP source at all (section 5), while the verbs that should train them already run: the forage
   collect trains farming instead of foraging, the resource-node collect trains farming instead of
   mining, the campfire trains nothing, treating a wound trains nothing, the vendor and escrow
   trades train nothing, Travel steps train nothing, votes train nothing. progression-skills-gear.md
   already requires this ("registering a system that adds a verb REQUIRES wiring its award_skill_xp
   call"). Small, and it turns eight dead rows into live ones.
3. **Preserving** (`preserving`, partial). The second sink for the harvest, and the answer to
   spoilage, which already runs: surplus food rots today with nothing to do about it but eat or
   sell it. Drying exists as two recipes; salting, canning and the unused `preservation_methods`
   are designed in quest-chains.md (Keep It and its branches), with how-tos already in the app.
4. **Water you can trust** (`water_safe`, partial). quest-chains.md section 4.1 names the water chain
   (rungs 0 and 1) as the first to build; it gives the water_purification skill its only XP source
   and teaches the most life-critical procedure in the catalog (boil, disinfect, filter).
5. **Smelting as a real activity, and slag's first uses** (`smelting_refining`). Smelting uses the
   drone-mined rock, and every ore smelt leaves slag that nothing consumes; slag.md designs slag as
   the endless sink (blocks, pavers, soil, shielding). The first rung turns a growing pile into
   building material, which construction then consumes. Glass from slag waits with glassmaking,
   a new chain.
6. **Maintenance and repair** (`maintenance_repair`, partial). Machines and structures never wear,
   so the workshop's parts (smithing, electronics) have no recurring consumer. Wear plus repair is
   the classic recurring sink, and the Library already teaches keeping things working and finding
   why something broke.
7. **Hygiene and sanitation** (`hygiene_sanitation`, partial). Place the urine-diverting toilet
   that `food.rs` stands in for (`home_outline.json`: "not in game yet") and the Clean Hands chain:
   it closes body waste to the compost properly and teaches the habit that prevents most illness.
8. **Testing soil, water and air** (`testing_lab`, designed). engagement-modes.md calls the
   investigation panel (sample, why it matters, cheapest fix, idle resources) "the unit of UI design
   for Direct mode". Built once as a universal widget for the garden, where pH and nutrients are
   already simulated, it is reused by water, air and power.
9. **Piloting a mining drone** (`drone_piloting`, designed). The operator named mining as a
   minigame; drone-mining.md designs the optional piloted approach on top of the live drone loop
   (reuse before new systems), with realistic light delay as its two modes.
10. **Point defense against debris** (`point_defense`, designed). A recurring sink for power and
    production, and a cooperative hook (several people crew one ring). Build mode and walk-up
    machines, its stated prerequisites, now exist. Recommended first slice: the turret doc's own
    (one fixed turret, a scripted debris wave, power drawn while firing), debris only until the
    operator answers open question 2.

Deliberately later: anything that starts a new chain with nothing waiting on it (brewing, welding,
3D printing, ship building), and the arts, which need the character animation rungs (dance,
music by hand) or have no simulation to sit on yet (painting, writing).

## 5. Skill gaps

The skill table is [`data/skills/skills.csv`](../../data/skills/skills.csv) (20 rows), and
progression-skills-gear.md sets a budget: "Keep exactly 20 skills. Depth comes from perk unlocks,
not skill count. A new skill requires retiring or splitting an old one."

**Skills nothing trains (8 of 20).** XP comes today from crafting (each recipe's skill), construction
(shelter_building), harvests and the walk-up collect (farming), drones (mining), combat kills
(melee, ranged), abilities (their skill_required) and quest rewards (metalworking and woodworking
only). No code or data ever trains: **foraging, fire_making, water_purification, navigation,
defense, tactics, trading, leadership.** Five of them have a live verb waiting (order item 2).

**Skills with no activity but combat (3).** ranged, defense and tactics belong only to `combat`,
whose existence as an activity is the operator's question.

**One skill doing the work of many.** engineering covers fourteen activities (electrical, plumbing,
heating and cooling, life support, electronics, data networks, pulleys, vehicles, ships, running
machines, repair, damage control, radiation, fabrication), so its level says little about what a
person can do. Farming covers eight (fine: they are one trade), cooking five.

**Activities with no skill (39).** Most are right that way: living (eating, sleeping, walking),
travel, community and play are not trades. The ones that teach a real, assessable skill and have
no home: pottery (the kiln's nine recipes train three different skills, or none), the science
family (weather reading, astronomy, testing, geology rides mining), firefighting, staying warm,
piloting and driving.

**Two naming defects found on the way.** The alchemy skill trains real chemistry recipes (soap,
medicines, fuel) under a fantasy name. And `data/abilities.csv` gates 94 fantasy-flavoured rows on
eleven school ids (pyromancy, cryomancy, arcana and the rest) that are not rows of skills.csv, so
those gates name skills that cannot exist.

**Recommendation (not a decision).** Keep the 20-row budget, and in this order:

1. Wire the five dead skills that have a live verb waiting (order item 2) before touching the table.
2. Rename alchemy to chemistry, outright, id included (CLAUDE.md, the names rule); fantasy schools
   stay as ability flavour, and the eleven school ids either become perk lines of real skills or
   the fantasy abilities stop naming a skill.
3. Give the kiln one owner: shelter_building (its bricks, tiles and ceramic filters are building
   and water materials).
4. The operator's combat answer frees slots or does not. If combat is defence only, melee plus
   defense become one self-defence skill and ranged plus tactics one marksmanship skill (which
   point defense trains), freeing two rows.
5. Spend freed rows where the catalog is starved: split electrical work (power, wiring,
   electronics, networks) out of engineering, and add one field-science skill (survey, testing,
   weather, sky). Arts stay skill-free (open question 4).

## 6. The operator's decisions, and the questions still open

### Decided (2026-11-14)

1. **Hunting and butchering are IN.** The operator, verbatim: "I rather teach everything even if
   we don't use the skills in real life because, at least then we understand what our ancestors
   went through to get us to where we are today. They're too valuable to leave out." This covers
   everything the question reached: `hunting_butchering` itself (today a player's killing blow
   rolls a creature's loot into the pack, a wolf giving raw meat, fur and bone, with no butchering
   procedure), `fishing`, the aquaponic tank's fish (`aquaponics`) and slaughter of kept animals
   (`livestock`, which gives only eggs, milk and wool today). How graphic it looks is governed by
   the standing dual-modes rule in CLAUDE.md ("Every deep system ships two modes": "Realistic
   severity exists; general play defaults to softened."); no separate choice was made for this
   activity.
2. **The order principle in section 4 is confirmed** ("Sounds fine. They all need to be developed
   eventually."): what uses up something already produced comes before brand-new chains.

### Open, each with this pass's recommendation

1. **Combat as an activity.** The code is live (melee, offensive abilities, armour, loot) and four
   skills are reserved for it. *Recommendation:* the default game has defence against animals and
   hazards only, a risk layer serving survival as gameplay-loop-map.md already says; fighting
   people is never an activity; fantasy combat lives only in opt-in fantasy content. (The hunting
   decision above is about animals and food; it does not answer this.)
2. **Point defense: debris only, or raiders too?** The turret design names "raiders, hostile drones"
   beside debris. *Recommendation:* debris only; it keeps the turret an engineering and cooperation
   game and outside open question 1.
3. **The skill budget.** Keep exactly 20 (progression-skills-gear.md) or let the table grow with the
   catalog? *Recommendation:* keep 20 and rebalance as section 5 sets out. Hunting and fishing, now
   in, have no skill either; under the cap they fit foraging (wild food, by any means) or a combined
   self-reliance line once the combat answer frees a row.
4. **Do arts and play train skills?** *Recommendation:* no XP for music, painting, dance, writing,
   watching or games: the reward is the thing made or shared, and an XP bar on play turns it into
   work. Fire props are the exception worth discussing, since their safety knowledge is real.

## 7. Where the code lives

An activity is not a module of its own; it is three pieces in the places the code already keeps
them:

- **Its simulation** is a module or folder in [`crates/humanity-systems`](../crates/humanity-systems.md)
  (about 82,000 lines today, one file or folder per system: `farming/`, `food.rs`, `mining.rs`,
  `crafting/`, `pulley/`, `body_heat.rs` and the rest). A new activity's model goes there, beside the
  system it extends, and runs whether or not anyone plays the activity (engagement-modes.md: one
  simulation, the mode only changes the interface).
- **Its interaction** (the walk-up prompt, the procedure, the camera, the HUD card) lives in the
  engine (`src/engine/`, with pages and cards under `src/gui/`).
- **Its data** (recipes, species, rates, levels, the procedure's steps) lives under `data/`, so a new
  dish, ore or animal is a row, not code.

A family becomes its own crate only once it is large and has a clean interface; the orchestrator
is proposing that in [crate-split.md](crate-split.md). Until then the catalog's families are a way
to read the game, not a code boundary.

## 8. Adding or changing an activity

The catalog is data (CLAUDE.md, Infinite-of-X): a new activity is a new entry in
`data/gameplay/activities.json` plus its id, in backticks, in this document. The lint refuses an
entry whose evidence path, design doc or twin path does not exist; whose skill is not in skills.csv;
whose status is not one of the four; a live entry with no source file or no FEATURES.md heading; a
designed entry with no design doc; a levels_setting not in levels.json; an activity that does not
support the direct mode; and a sensitive activity (combat, point_defense) whose
operator_to_decide question has been removed (an activity leaves that list only with the
operator's answer quoted in section 6, as hunting_butchering did). When an activity's status changes (a recipe becomes a
procedure, a scaffold is registered), change its entry in the same commit and regenerate the
tables in section 3.

Nothing renders the catalog yet, so its names are plain English in the data. When the Play hub, the
skills page, quests or tutorials start showing them, add the words with `node scripts/i18n-add.js`
and point each entry at its key.
