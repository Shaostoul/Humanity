# Survival quest chains: water, food, warmth, air, cleanliness, first aid and rest

Status: **design, 2026-10-30.** Nothing here is built, and no quest data is written yet: the
operator reviews this first, then `data/quests/*.ron` is written from it rung by rung (section 4).
Every fact in section 3 was read from its primary source on 2026-10-30 (the few that could not be
are marked so, and listed in 3.6), and **every one needs a refutation pass before it reaches a
screen** (the house rule for teaching data: a previous round of researched species records lost 48
of 60 on adversarial review, almost all on the fields that can hurt someone). Companions: `docs/design/how-to.md` (the real procedures a quest step links),
`docs/design/gameplay-loop-map.md` and `docs/design/progression-skills-gear.md` (the design bible),
`src/systems/quests/` (the quest system as built).

## Why

The operator, 2026-10-30:

> "Can you think up a bunch of useful quest chains? (I forget how we were differentiating
> quests/tasks.) They could be things like 'hydration' and a simple meter to watch for. Then
> branches could be like purify water with evaporation and another for reverse osmosis and so
> forth. Another one for food with its own branches."

The game teaches real survival skills through simulation (the Real/Sim toggle). The body's needs
are already simulated at real scale: Hydration empties in about two days and then kills in about
one more, Satiation empties in about a week (`src/systems/food.rs`, v0.1005 "REAL SCALE"). What the
game does not yet do is lead a player through *how* to meet those needs the way a person would have
to in life. These chains do that, one need at a time, with a branch for each real method.

## Quests, tasks and how-tos: which is which

The operator asked, so here it is in one place:

| Word | What it is | Which reality | Where it lives |
|---|---|---|---|
| **Quest** | The game's path: a story goal with steps the game checks, rewards (items, skill XP), and the HUD's quest line. | Sim only | Quests page; `data/quests/*.ron`; `src/systems/quests/` |
| **Task** | A person's own real to-do. The Tasks page also holds the learn-by-doing guide panel (`data/onboarding/quests.json`), which is platform onboarding, not gameplay. | Real only | Tasks page |
| **How-to** | A real procedure as data: steps, checks, safety notes, sources. The game can render one too. | Real, and Sim where the game models it | How-to page; `data/how_to/` |

Operator, 2026-08-16: "Quests is for gameplay stuff and Tasks is for not-game stuff". The join
between them is the how-to: a quest step can say **Learn: Boil water to make it safe to drink**,
which opens the real how-to (`docs/design/how-to.md`, "How it connects to the other pages", the
Quests row). Proposed here: finishing a quest can offer **Do it for real**, which adds the real
how-to to the person's Tasks when they press it (that page's Tasks row), and a quest never writes
to Tasks on its own.

## 1. How chains and branches work in the data

### 1.1 What the code does today

A quest is a `QuestDef` (`src/systems/quests/mod.rs`): `id`, `name`, `description`, ordered
`steps`, item `rewards`, `xp_rewards`, and one `prerequisite: Option<String>`. Each step has a
`description` (the HUD line) and one `QuestObjective` (`objectives.rs`): `Gather`, `Craft`, `Make`,
`Harvest`, `Build`, `Travel`, `Talk`, `Eat`, `Plant`, `View`.

When a quest completes, `QuestSystem::tick` accepts **every** quest whose `prerequisite` names it
and that is not already active or done, and posts one notice: "New quests: A, B and C." A quest
with `prerequisite: None` that is not an opening shows on the Quests page under Available, where
the player accepts it (v0.748, `lib.rs`'s quest bridge).

Counting rules that matter for chains:

- Count steps (`Craft`, `Make`, `Eat`, `Plant`, `Harvest`, `Build`, `Talk`, `View`) count events
  from the moment the quest was accepted, so a thing done early is not wasted.
- `Gather` counts what is carried plus what the home holds, live.
- `Travel` counts only on its own step.

### 1.2 A branch is several quests naming the same prerequisite

So a chain like Water is a tree in the data with no new machinery:

```
water_first                     (offered: prerequisite None)
  └── water_unsafe              (prerequisite water_first)
        ├── water_boil          (prerequisite water_unsafe)   these all start together
        ├── water_disinfect     (prerequisite water_unsafe)   when water_unsafe finishes;
        ├── water_filter        (prerequisite water_unsafe)   finishing one never cancels,
        ├── ...                                                hides or locks another
```

**A branch never locks the others** (the operator's default for now). There is no `excludes`
field and none is proposed: doing the boiling branch does not close the filter branch, and the
player may do every branch, in any order, or none.

### 1.3 Chain roots are offered, not forced

Each chain's first quest has no prerequisite, so it sits under Available on the Quests page. The
call made here: the openings (First Steps, Settling In) do not auto-accept seven survival chains
at once, which would flood the HUD in a new player's first minutes. A quest can only have one
prerequisite, so a root could not follow both openings anyway. The Quests page's Available list is
where a player meets them; the opening's last notice can point there.

### 1.4 Joins: "after any of these" (new engine piece P1)

Some quests make sense after *any* of several branches: "Water for Bad Days" (storing safe water)
belongs after the player has made water safe by whichever method they chose. One `prerequisite`
cannot say that. The fix, changed outright (no compatibility shims before v1.0, CLAUDE.md):

```ron
// replaces `prerequisite: Option<String>`
after: AnyOf(["water_boil", "water_disinfect", "water_filter", "water_sun", "water_distil", "water_ro"]),
// the other forms:  after: None   after: Quest("water_first")   after: AllOf([...])
```

When any quest completes, every quest whose `after` is now satisfied, and that is not active or
done, is accepted (the same loop as today, with `AnyOf`/`AllOf` checked against
`completed_quests`). `AllOf` is for capstones only (section 2.9). The shipped-quest test that
already checks every objective's ids resolve (`shipped_quest_objective_ids_all_resolve`) also
checks every id in `after` names a quest.

### 1.5 The HUD line with several quests running (new engine piece P2)

The HUD shows the first active quest (`hud.rs`: `state.quests.iter().find(|q| !q.completed)`),
which is whichever was accepted first. Once branches run side by side, the player needs to choose:
`QuestTracker` gains `tracked: Option<String>`, set by a **Track** button on each active quest on
the Quests page; the HUD shows the tracked one, else the most recently accepted. Saved with the
tracker (it is already saved).

### 1.6 What the player sees of a chain

Another agent is building the view where developer mode shows every step of every quest and
normal play shows only the steps reached. So each quest's `description` must state the whole goal
on its own, without leaning on later steps, and no step's line should only make sense after
reading a later one.

### 1.7 Rules for writing a step

- **The HUD line is at most 64 characters with its counter** ("text (3/6)"): about 55 characters of
  text. `every_step_of_every_shipped_quest_fits_the_hud_line` already holds every shipped quest
  to it. The lines in section 2 were counted to fit.
- **A line names what the game shows** (the key, page, button), as the opening does. A rename must
  not leave a line pointing at nothing: extend `every_step_of_the_opening_names_what_the_game_shows`
  to these chains when their data is written.
- **A method's own branch uses `Craft` with its recipe; a goal that any method meets uses `Make`
  or a reading.** "Boil it" must be boiled; "store 20 L of safe water" can be water made safe any
  way.
- **The game never teaches a result safer than life.** A treatment joins the game only when what it
  does in the game is no safer than what it does in life. That is why chemical disinfection and
  filters wait for the water contents model (P5) instead of shipping as "makes Purified Water".
- **Every chain works in both modes.** Quests do not change with Settings > Gameplay; the modes
  they run under do: the Illness mode (Forgiving or Realistic, `systems::illness`), the Vitals
  drain slider, and Making things (Simplified or Realistic, `docs/design/how-to.md`). A step that
  counts a made thing counts it either way.
- **Each start can finish what it is offered.** A cabin has no stove and no garden. A survival
  quest is offered only to starts whose home has what its steps use (new field
  `starts: Vec<String>`, empty for all), and a test reads each offered quest against each start's
  kit and machines, the way `every_shipped_home_has_what_the_starter_quests_need` does for the
  openings.

### 1.8 Cost

Quests are checked on the current step of each active quest, once a tick. The new kinds read a
few numbers (a vital, a home reading, a ledger total) and compare; the ledger updates on each
eat or drink event, never by scanning. Budget: under 0.01 ms a frame for the whole quest system
with every chain active, measured by the frame-cost spans when built. Network footprint: none in
the home world; on a server world quest progress stays the player's own (BUG-185: other players
are never sent anyone's quests).

## 2. The chains

How to read the tables:

- **Objective**: a kind that exists in `objectives.rs`, or **new** (defined in 2.10).
- **Needs**: items, recipes, blueprints, machines. *exists* means it is in the game data today;
  *new* means it must be written; *fix* means it exists but section 3.5 found it teaches something
  the sources do not support.
- **How-to**: *exists* (in `data/how_to/`), *planned* (named in `data/how_to/planned.json`), or
  *new* (to add to `planned.json`, then write). A Library guide is named as `library:<file>`.
- **Facts**: ids in section 3. A step marked **⚠ lethal** or **⚠ serious** shows its safety line
  on screen, worded as the How-to page words hazards ("Can kill you if done wrong", "Can seriously
  hurt you"), with the emergency number for the person's place (`docs/design/how-to.md`, "Safety,
  hazard, and the two reading views").

### 2.1 Water (the hydration chain)

```
water_first  Water First
  └── water_unsafe  Water You Cannot Trust
        ├── water_boil        Boil It
        ├── water_disinfect   Disinfect It
        ├── water_filter      Filter It
        │     ├── water_filter_ceramic   Fire a Clay Filter
        │     └── water_filter_sand      Build a Sand Filter
        ├── water_sun         Let the Sun Do It            (sunlight and UV)
        ├── water_distil      Catch the Steam              (evaporation: pot still, solar still)
        │     └── water_reclaim          Every Drop Back   (the ship's reclaimer)
        ├── water_ro          Push It Through a Membrane   (reverse osmosis)
        └── water_rain        Water from the Sky           (collecting rain)
water_store  Water for Bad Days   after AnyOf(boil, disinfect, filter, sun, distil, ro)
```

**Where untreated water comes from** (new, part of P5): the homestead's cistern has a raw tap
before the purifier (rain catchment fills it; `data/machines/home_solo.ron` already routes rain
into `water_tank` and through `water_purifier`); a rain barrel the player builds (`water_rain`); a
stream or pond node in the world beside the existing forage nodes (`data/entities/wild_spawns.ron`);
and aboard the ship, the air handlers' condensate drain (the pipe media already name `condensate`,
`src/machines.rs`). And the emergency the chain prepares for: when the purifier has no power, the
home's taps deliver the cistern's water as it is, and the game says so the way a boil-water notice
would.

#### `water_first` Water First

*Offered. The meter.* "Your body loses water all day, about two and a half litres in the game's
reckoning, and more in the heat. Learn the signs of drying out, take in a day's water, and keep
your Hydration up for a whole day."

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "See your Hydration: press I, Inventory > Status" | `View("vitals")` | exists | | W1 |
| 2 | "Learn: Drink enough water (How-to)" | `Learn("how_to:drink_enough_water")` new | | new | W1, W2, W3 |
| 3 | "Fill a bottle at a water tank: E, then Fill" | `Gather(water_bottle_0, 1)` | water tank walk-up fill, exists | | |
| 4 | "Drink 2 bottles: press I, click one, then Drink" | `Drink(water_bottle_0, 2)` new | exists | | |
| 5 | "Take in 2.5 L of water in one day" | `Keep(intake.water_l_24h >= 2.5)` new | intake ledger (P3) | | W1, W4 |
| 6 | "Keep your Hydration above 60 for a whole day" | `Keep(vital.hydration >= 60, 24 h)` new | | | W3 |

Rewards: `water_jerrycan_empty_0` 1 (used by `water_store`); XP water_purification 20.

Step 5 counts food's water as well as drinks (the food system already credits food's water,
`FoodSystem::water_points`), which is itself the lesson: about a fifth of the water people take in
comes from food (W1). Step 6 is the operator's "simple meter to watch for": from full, a day of the
game's drain leaves 50, so holding 60 means drinking through the day and before sleep. At time
speed 1 it is a real day of play; the night asleep passes faster (the sleep clock), and it counts.

#### `water_unsafe` Water You Cannot Trust

*After `water_first`. The hub.* "Water that has not been treated can carry germs, dirt or
chemicals, and no single treatment fixes all three. Find untreated water and learn what is in it
before you choose how to make it safe."

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: Making Water Safe to Drink (Library)" | `Learn("library:making_water_safe_to_drink")` new | exists | | W5, W6 |
| 2 | "Fill a bottle with untreated water" | `Keep(stock.untreated_water_l >= 0.5)` new | untreated sources (P5) | | W17 |
| 3 | "Test it: E at the water with a Water Tester" | `Use(water_tester_0)` new | `water_tester_0` exists, its test is new | new: `test_water_you_found` | W18 |
| 4 | "Clear cloudy water: settle it, pour off the top" | `Craft(settle_water, 1)` | recipe new | exists (step of `boil_water_safe`) | W5 |

**⚠ lethal** on step 1's notice: "You cannot make water with fuel, chemicals or radioactive material
in it safe by boiling or disinfecting it" (W6). Rewards: XP water_purification 15.

#### `water_boil` Boil It

*After `water_unsafe`. The first branch built, because boiling is the one method whose game result
can be exactly as safe as life's: it kills every class of germ (W7).*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Boil water to make it safe (How-to)" | `Learn("how_to:boil_water_safe")` new | | exists | W7, W8 |
| 2 | "Boil a bottle: Crafting > Boil Water" | `Craft(boil_water, 1)` | recipe new: untreated water, `cooking_pot_0` (tool), `stove_0`; clears every germ class | exists | W7, W8 |
| 3 | "Drink water you boiled yourself" | `Drink(water_bottle_0, 1)` with contents "boiled" | P5 | | |

**⚠ serious** on step 2: scalds (the how-to's own safety note). Rewards: XP water_purification 25,
cooking 10.

#### `water_disinfect` Disinfect It

*After `water_unsafe`. Built with the contents model (P5), because the lesson is what bleach does
NOT do.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Disinfect water with bleach (How-to)" | `Learn("how_to:disinfect_water_with_bleach")` | | planned | W9, W10 |
| 2 | "Get plain unscented bleach" | `Gather(bleach_0, 1)` | item new | | W9 |
| 3 | "Disinfect a bottle: 2 drops, wait 30 minutes" | `Craft(disinfect_water_bleach, 1)` | recipe new; clears bacteria and viruses, not Cryptosporidium | planned | W9, W10 |
| 4 | "Treat water with chlorine dioxide tablets" | `Craft(disinfect_water_clo2, 1)` | item `chlorine_dioxide_tablet_0` new, recipe new | new: `disinfect_water_with_tablets` | W10, W11 |

**⚠ lethal** on steps 2 and 3: plain unscented bleach only, the dose for its strength, never mixed
with other cleaners (planned how-to `never_mix_cleaners`). Rewards: XP water_purification 25.

#### `water_filter` Filter It

*After `water_unsafe`. Built with P5. Teaches the pore sizes and "filter, then disinfect".*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Filter water you are not sure of" | `Learn("how_to:filter_water_portable")` | | new | W12, W13 |
| 2 | "Get a portable filter (0.3 micron or finer)" | `Gather(water_filter_portable_0, 1)` | item new | | W12 |
| 3 | "Filter a bottle: Crafting > Filter Water" | `Craft(filter_water_portable, 1)` | recipe new; clears parasites and bacteria, not viruses | new | W12 |
| 4 | "Filter, then disinfect the same water" | `Keep(stock.safe_water_l >= 0.5)` | P5 | new | W13 |

Rewards: `chlorine_dioxide_tablet_0` 10; XP water_purification 25.

#### `water_filter_ceramic` Fire a Clay Filter

*After `water_filter`.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Dig clay at a clay pit: E" | `Gather(clay_raw_0, 4)` | `clay_raw_0` and the clay pit node exist | | |
| 2 | "Fire a pot filter: Crafting > Fire Clay Filter" | `Make(ceramic_water_filter_0, 1)` | item and recipe new, at `kiln_0` (exists); its clay body is still to find out (3.6) | new: `make_a_clay_pot_filter` | W14 |
| 3 | "Filter a bottle through your clay filter" | `Craft(filter_water_ceramic, 1)` | recipe new | new | W14 |

Rewards: XP water_purification 20 (the skills list has no pottery skill yet).

#### `water_filter_sand` Build a Sand Filter

*After `water_filter`. The operator's "sand-and-charcoal".*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Gather sand, gravel and charcoal" | `Gather(sand_0, 4)` | `sand_0`, `gravel_0`, `coal_0` (lump charcoal) exist; sand pit node exists | | |
| 2 | "Build a sand filter: Crafting > Structures" | `Build(slow_sand_filter)` | blueprint new (`barrel_0`, sand, gravel, charcoal) | new: `build_a_sand_filter` | W15 |
| 3 | "Pour untreated water through it" | `Craft(filter_water_sand, 1)` | recipe new; clears cloudiness, partly germs | new | W15, W16 |

The charcoal layer is the lesson inside the lesson: it improves taste and traps some chemicals, but
"trap, but do not kill" microorganisms (W16). Rewards: XP water_purification 20.

#### `water_sun` Let the Sun Do It

*After `water_unsafe`. Needs the sun-timed process (P6).*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Fill clear plastic bottles with cleared water" | `Gather(pet_bottle_clear_0, 2)` | item new (transparent PET; the game's water bottle is HDPE) | new: `disinfect_water_with_sunlight` | W19 |
| 2 | "Lay them in full sun for 6 hours" | `Craft(sodis_water, 1)` | recipe new, timed by sun on the spot (P6); 2 days when cloudy | new | W19 |
| 3 | "Treat a bottle with a UV pen" | `Craft(uv_treat_water, 1)` | item `uv_purifier_portable_0` new; clear water only | new | W20 |

Rewards: XP water_purification 20.

#### `water_distil` Catch the Steam

*After `water_unsafe`. The operator's "evaporation". The pot still is rung 1 with boiling; the
solar still waits for P6 and for its yield to be sourced.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Distil water with a pot and lid" | `Learn("how_to:distil_water_pot")` | | new | W21 |
| 2 | "Distil a bottle: Crafting > Distil Water" | `Craft(distil_water_pot, 1)` | recipe new: untreated or salty water, `cooking_pot_0`, `cup_0` (exists), `stove_0`; clears germs, salt, metals | new | W21 |
| 3 | "Build a solar still outdoors" | `Build(solar_still)` | blueprint new (`plastic_sheet_0`, a container, a stone), outdoors only | new: `build_a_solar_still` | W22 |
| 4 | "Collect water from your solar still" | `Make(water_bottle_0, 1)` from the still | P6; yield still to find out | new | W22 |

**⚠ serious** on step 2: scalds and steam. Rewards: XP water_purification 30.

#### `water_reclaim` Every Drop Back

*After `water_distil`. The ship's version of the same physics: the space station recovers water
from urine by distilling it under vacuum (W23).*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: how a ship gets its water back" | `Learn("library:where_water_goes")` | exists | | W23 |
| 2 | "Build a water reclaimer" | `Build(water_reclaimer)` | machine new (vacuum distiller, power) | | W23 |
| 3 | "Recover 20 L of water from the reclaimer" | `Keep(home.reclaimed_water_l >= 20)` | reading new | | W23 |

The trade-off this teaches, and the game makes visible: the home's urine today goes to the garden
as nitrogen (`URINE_ITEM` in `food.rs`, `data/garden/nutrients.ron`); a reclaimer turns it into
water instead. The player chooses which, per day, on the reclaimer's card. Rewards: XP
engineering 30.

#### `water_ro` Push It Through a Membrane

*After `water_unsafe`. Needs P5 (and power).*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Build a reverse osmosis unit" | `Build(ro_unit)` | machine new: membrane, pump pressure, power, a reject-water outlet | | W24 |
| 2 | "Make 10 L of drinking water with it" | `Keep(home.ro_permeate_l >= 10)` | reading new | | W24, W25 |
| 3 | "Send its reject water to the garden" | `Keep(home.ro_reject_to_garden_l >= 10)` | the reject outlet piped to irrigation | | W25 |

Step 3 is the closed-loop lesson: a typical unit sends five litres or more down the drain for each
litre it makes (W25), and in a home that collects every drop, that water is the garden's. Rewards:
XP engineering 25, water_purification 15.

#### `water_rain` Water from the Sky

*After `water_unsafe`.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Set up a rain barrel with a first flush" | `Learn("how_to:collect_rainwater_first_flush")` | | planned | W17 |
| 2 | "Build a rain barrel with a first flush diverter" | `Build(rain_barrel)` | blueprint new (`barrel_0`, `pipe_pvc_0`, diverter) | planned | W17 |
| 3 | "Collect 20 L of rain" | `Keep(home.rain_barrel_l >= 20)` | reading new; weather rain exists | | |
| 4 | "Make a bottle of rain safe to drink" | `Keep(stock.safe_water_l >= 0.5)` | P5 | | W17 |

Rewards: XP water_purification 20.

#### `water_store` Water for Bad Days

*After `AnyOf` the method branches. Teaches the emergency store and its upkeep.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Store water for an emergency (How-to)" | `Learn("how_to:store_water_for_an_emergency")` | | planned | W26, W27 |
| 2 | "Sanitise a jerrycan: Crafting > Sanitise" | `Craft(sanitise_container, 1)` | recipe new: `water_jerrycan_empty_0`, `bleach_0`, water (rung 1 ships without this step: no bleach yet) | planned `clean_a_water_container` | W28 |
| 3 | "Fill a 20 L jerrycan with safe water" | `Keep(stock.safe_water_l >= 20)` | exists (fill), P5 | | W26 |
| 4 | "Keep it somewhere cool and dark" | `Keep(home.stored_water_cool_l >= 20)` | storage zone reading new | | W27 |

One 20 L jerrycan is more than CDC's three-day store for one person (3 US gallons, about 11.4 L,
for drinking and sanitation, W26); the two-week store CDC asks for if possible is about 53 L. The
completion notice offers **Do it for real**: the real how-to with its six-monthly Repeat on the
person's Tasks and Calendar. Rewards: XP water_purification 30.

### 2.2 Food

```
food_first  Food First
  ├── food_grow      Grow Your Own        (existing Start Farming joins after it: AnyOf)
  ├── food_cook      Cook It Through
  ├── food_keep      Keep It
  │     ├── food_keep_dry      Dry It
  │     ├── food_keep_salt     Salt It
  │     ├── food_keep_ferment  Ferment It
  │     └── food_keep_can      Can It
  ├── food_balance   Eat Across the Plate
  └── food_forage    Wild Food, Carefully   (built last, after its own refutation pass)
```

#### `food_first` Food First

*Offered. The meter.* "Your body runs on food energy. Food labels use 2,000 kcal a day as a
general guide, and your own need may be higher or lower. Watch your Satiation and eat a day's
worth."

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "See your Satiation: press I, Inventory > Status" | `View("vitals")` | exists | | F1 |
| 2 | "Read: Nutrition That Keeps You Working (Library)" | `Learn("library:nutrition_that_keeps_you_working")` | exists | | F1 |
| 3 | "Eat 2,000 kcal of food in one day" | `Keep(intake.kcal_24h >= 2000)` | P3 | | F1 |

The game's Satiation already runs on this number: a day's food at 2,000 kcal puts back a day's
hunger (`DAILY_KCAL`, `SATIATION_PER_KCAL` in `food.rs`). Rewards: `seed_lettuce_0` 5,
`seed_radish_0` 5; XP cooking 15.

#### `food_grow` Grow Your Own

*After `food_first`. Overlaps the existing Start Farming (`tutorial_farming_basics`, after Build
First Habitat): rather than rebuild it, Start Farming's `after` becomes
`AnyOf(["tutorial_first_habitat", "food_grow"])`, so it follows whichever the player did.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Plant 3 crops: press I, Garden, then Plant" | `Plant(any, 3)` | exists | planned `start_seeds_indoors` | |
| 2 | "Harvest a crop you grew" | `Harvest(lettuce, 1)` | exists | | |
| 3 | "Rinse it under running water, then eat it" | `Eat(vegetable_lettuce_0, 1)` | exists | | F2 |

Facts about each crop (days to harvest, light, water) are its row in `data/plants.csv`, sourced
row by row by the botanist; this chain cites none of its own. Rewards: XP farming 20.

#### `food_cook` Cook It Through

*After `food_first`. Needs a food thermometer.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Cook meat to a safe temperature" | `Learn("how_to:cook_meat_to_a_safe_temperature")` | | new | F2, F3 |
| 2 | "Wash your hands at a sink for 20 seconds" | `Use(provides "wash")` new | sink walk-up action new | new: `wash_your_hands` | C1 |
| 3 | "Get a food thermometer" | `Gather(food_thermometer_0, 1)` | item new | | F3 |
| 4 | "Cook fish until it reads 145 F (63 C)" | `Make(fish_grilled_0, 1)` | exists; Realistic making checks the reading as a how-to step | new | F3 |
| 5 | "Put leftovers in the fridge within 2 hours" | `Store(cold, any cooked, 1)` new | the storage-zone event (E6) | | F4, F5 |

**⚠ serious** on step 4: undercooked meat, poultry and fish. Rewards: XP cooking 25.

#### `food_keep` Keep It

*After `food_first`. The hub for keeping food.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: The Chemistry of Keeping Food (Library)" | `Learn("library:the_chemistry_of_keeping_food")` | exists | | F5, F6 |
| 2 | "Put fresh food in the fridge" | `Store(cold, any produce, 1)` new | the `cold` zone exists (`food_system.ron` `temperature_zones`), its band needs the fix in 3.5 | | F5, F6 |

Rewards: XP cooking 10.

#### `food_keep_dry` Dry It

*After `food_keep`. Rung 1: its recipes exist.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: Drying Food (Library)" | `Learn("library:drying_food")` | exists | | F7 |
| 2 | "Dry fruit: Crafting > Dry Fruit" | `Craft(cook_dried_fruit, 1)` | exists (`oven_0`) | | F7 |
| 3 | "Make jerky: heat it to 160 F first, then dry it" | `Craft(cook_jerky, 1)` | exists | new: `dry_jerky_safely` | F8 |

**⚠ serious** on step 3. Rewards: XP cooking 20.

#### `food_keep_salt` Salt It

*After `food_keep`. Its facts were not found in this pass (USDA FSIS refused automated reading,
section 3.6): built after a dated finding on salt curing, not before.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: Salting, Curing and Smoking (Library)" | `Learn("library:salting_curing_and_smoking")` | exists | | still to find out |
| 2 | "Collect salt at a salt flat: E" | `Gather(salt_food_0, 1)` | node exists | | |
| 3 | "Salt and dry meat: Crafting > Dry Meat" | `Craft(cook_dried_meat, 1)` | exists, **fix** its "165 F for wild game" (3.5) | | F8 |

#### `food_keep_ferment` Ferment It

*After `food_keep`. Needs the long room-temperature process (P7).*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: Fermenting Vegetables (Library)" | `Learn("library:fermenting_vegetables")` | exists | new: `make_sauerkraut` | F9 |
| 2 | "Harvest or buy 2 cabbages" | `Gather(vegetable_cabbage_0, 2)` | exists | | |
| 3 | "Pack cabbage and salt in a crock under brine" | `Craft(ferment_sauerkraut, 1)` | `fermenting_crock_0` new; recipe new, salt by the source's ratio | new | F9 |
| 4 | "Ferment it 3 to 4 weeks at 21 to 24 C" | `Make(sauerkraut_0, 1)` | P7: the crock's clock runs on game time, at the room's temperature | new | F9 |

The salt item is a 500 g container (`salt_food_0`), and the source's ratio is 3 tablespoons to 5 lb
of cabbage, so the recipe either works in large batches or salt gets a smaller unit; the recipe's
quantities are derived from the ratio and say so. Rewards: XP cooking 25.

#### `food_keep_can` Can It

*After `food_keep`. The most dangerous keeping method: built after its recipes are fixed.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: Why Canning Has Rules (Library)" | `Learn("library:why_canning_has_rules")` | exists | | F10, F11 |
| 2 | "Can fruit in a boiling-water canner" | `Craft(can_fruit_water_bath, 1)` | `water_bath_canner_0` new, recipe new; `canning_jar_0` exists | planned `can_fruit_water_bath` | F11 |
| 3 | "Can vegetables in a pressure canner" | `Craft(can_vegetables_pressure, 1)` | `pressure_canner_0` new; **fix** `cook_canned_food` (3.5) | new: `pressure_can_vegetables` | F10, F11 |
| 4 | "Learn: Test home-canned jar seals" | `Learn("how_to:test_jar_seals")` | | exists | F12 |

**⚠ lethal** on steps 1 to 4: botulism; low-acid food only in a pressure canner, tested recipes
only, and "if in doubt, throw it out" (F10, F12; planned how-to `dispose_of_a_spoiled_jar`).
Rewards: XP cooking 30.

#### `food_balance` Eat Across the Plate

*After `food_first`. Needs the intake ledger's vitamins (P3). The game's food data already carries
vitamins as shares of the FDA Daily Values (`data/food_system.ron`), and nothing reads them over
days yet.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: How Your Body Works (Library)" | `Learn("library:how_your_body_works")` | exists | | F13 |
| 2 | "Get 90 mg of vitamin C in one day" | `Keep(intake.vitamin_c_mg_24h >= 90)` | P3 | | F13 |
| 3 | "Keep that up for 3 days running" | `Keep(intake.vitamin_c_mg_24h >= 90, 72 h)` | P3 | | F13 |

90 mg is the higher of the two adult allowances (men 90, women 75, F13), so it is enough for any
adult. A deficiency illness (scurvy) is not modelled and this chain does not need it; if one is
added later it follows F13's "below 10 mg a day for many weeks", not a game-paced shortcut.
Rewards: XP cooking 20, medicine 10.

#### `food_forage` Wild Food, Carefully

*After `food_first`. Built LAST, after its own refutation pass and after the identification model
(P9). This is where teaching data has hurt before: the 2026 species review refuted most records on
toxicity and lookalikes.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: Identifying Wild Plants (Library)" | `Learn("library:identifying_wild_plants")` | exists | | F14, F15 |
| 2 | "Pick berries at a berry bush: E" | `Gather(<berry item>, 5)` | berry bush node exists; its species must be named | | |
| 3 | "Tell poison hemlock from wild carrot" | `Identify(conium_maculatum)` new | P9; both plants in the world | new: `tell_hemlock_from_wild_carrot` | F15 |

The chain never rewards eating a wild plant or mushroom the player has not identified, and in
Realistic mode a wrong identification poisons. Wild mushrooms get the quest's completion notice
(F14: deadly and edible ones can look alike, and cooking does not make the deadly ones safe), not a
step, because "do not eat it" is not a thing the game can count. **⚠ lethal** throughout.

### 2.3 Warmth and fire

```
warm_first  Keep Warm
  ├── warm_fire          Make a Fire
  ├── warm_alarm         Smoke Alarms
  ├── warm_heat_safely   Heat Without Poison     (carbon monoxide)
  └── warm_cool          Too Hot
```

#### `warm_first` Keep Warm

*Offered (homestead starts: the cold is outside the hull or outdoors).* "Your core is about 37 C.
Below 35 C is hypothermia, and it is an emergency. Learn its signs and keep your core warm on a
cold walk."

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "See your body temperature: Inventory > Status" | `View("vitals")` | exists | | H1 |
| 2 | "Read: Cold and Hypothermia (Library)" | `Learn("library:cold_and_hypothermia")` | exists | new: `warm_someone_with_hypothermia` | H1, H2 |
| 3 | "Walk out to the fields" | `Travel(outdoor_fields)` | exists | | |
| 4 | "Keep your core above 36 C outdoors for 1 hour" | `Keep(vital.body_temp_c >= 36, 1 h, at outdoor_fields)` new | `systems::body_heat` exists; clothing exists | | H1 |

**⚠ lethal** on step 2's notice (hypothermia signs and what to do). Rewards: `sleeping_bag_0` 1;
XP shelter_building 15.

#### `warm_fire` Make a Fire

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Gather 3 logs at a fallen log: E" | `Gather(wood_log_0, 3)` | exists | | |
| 2 | "Build a campfire outdoors: Crafting > Structures" | `Build(campfire)` | exists (BUG-153 campfire, outdoors only) | | |
| 3 | "Learn: Use a fire extinguisher (How-to)" | `Learn("how_to:use_a_fire_extinguisher")` | | planned | H3 |
| 4 | "Make a fire extinguisher: Crafting" | `Craft(craft_fire_extinguisher, 1)` | exists | planned | H3 |

**⚠ serious** on step 3. Rewards: XP fire_making 25.

#### `warm_alarm` Smoke Alarms

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Test your smoke alarms (How-to)" | `Learn("how_to:test_smoke_alarms")` | | planned | H4 |
| 2 | "Build a smoke detector: Crafting" | `Craft(assemble_smoke_detector, 1)` | exists | | H4 |
| 3 | "Fit one on every floor of your home" | `Keep(home.smoke_alarm_floors_missing <= 0)` | reading new; detector placement new | | H4 |

Rewards: XP engineering 15.

#### `warm_heat_safely` Heat Without Poison

*Carbon monoxide comes from heating and from burning fuel, so it lives in this chain.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Test a carbon monoxide alarm (How-to)" | `Learn("how_to:test_co_alarm")` | | planned | H5, H6 |
| 2 | "Build a CO alarm: Crafting" | `Craft(assemble_co_alarm, 1)` | item `co_alarm_0` and recipe new | planned | H6 |
| 3 | "Fit it near where you sleep" | `Keep(home.co_alarm_by_beds >= 1)` | reading new; placement new | | H6 |

**⚠ lethal** on every step: carbon monoxide has no smell or colour, and a sleeping person can die
of it before any symptom (H5). Rewards: XP engineering 20.

#### `warm_cool` Too Hot

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: When Heat Becomes Dangerous (Library)" | `Learn("library:when_heat_becomes_dangerous")` | exists | new: `cool_someone_with_heat_stroke` | H7, H8 |
| 2 | "Work outdoors on a hot day and stay below 38 C" | `Keep(vital.body_temp_c <= 38, 2 h, at outdoor_fields)` | `body_heat` and weather exist | | H7 |

**⚠ lethal** on step 1: heat stroke is an emergency; call for help, cool the person, and give them
nothing to drink (H7). Rewards: XP shelter_building 15.

### 2.4 Air in a sealed home

*Ship homes and the sealed homestead. The home already simulates its air (the model in
`src/systems/atmosphere.rs`, its state `HomeAirState` in `src/ecs/components.rs`; the CO2
percentage reaches the Home page as `air_co2_pct`), and a planted homestead's crops already hold it
near 465 ppm with the scrubber idle (the air loop's note in `data/machines/home_solo.ron`). So these
steps are easy in a home whose garden grows, and the lesson is what holds the air there: they are
hard in a home whose garden is bare or whose power is out.*

```
air_first  Every Breath
  ├── air_scrub    Scrub the Air
  └── air_plants   Let the Garden Breathe
```

#### `air_first` Every Breath

"Each person breathes in close to a kilogram of oxygen a day and breathes out about the same of
carbon dioxide. In a sealed home, something has to take the carbon dioxide out."

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "See your Oxygen: press I, Inventory > Status" | `View("vitals")` | exists | | A1 |
| 2 | "See your home's air: Home > Air" | `View("home_air")` | view key new on the Home page's air section | | A1, A2 |
| 3 | "Keep CO2 under 0.5% for a whole day" | `Keep(home.co2_pct <= 0.5, 24 h)` | reading new from `HomeAirState` | | A3 |

0.5% is NIOSH's limit for a worker's average over a working day (A3): a ceiling, not a comfort
target. A home target is still to find out (3.6) and replaces it when sourced. Rewards: XP
engineering 15.

#### `air_scrub` Scrub the Air

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Check the CO2 scrubber is running: E at it" | `Use(provides "co2_scrub")` new | home scrubber exists (`data/machines/`) | | A1 |
| 2 | "Keep CO2 under 0.5% for 3 days" | `Keep(home.co2_pct <= 0.5, 72 h)` | | | A3 |

#### `air_plants` Let the Garden Breathe

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Plant 4 crops under the grow lights" | `Plant(any, 4)` | exists | | A4 |
| 2 | "See how much CO2 the garden takes: Home > Air" | `View("home_air")` | | | A4 |

Crops take carbon dioxide in while lit (`data/life_support.ron`, from NASA crop measurements; A4 is
that file's citation and is re-read before this ships). Rewards: XP farming 15.

### 2.5 Clean hands and waste

```
clean_first  Clean Hands
  └── clean_compost  Waste Becomes Soil
```

#### `clean_first` Clean Hands

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: Handwashing and Hygiene (Library)" | `Learn("library:handwashing_and_hygiene")` | exists | new: `wash_your_hands` | C1, C2 |
| 2 | "Wash your hands at a sink for 20 seconds" | `Use(provides "wash")` new | sink walk-up new | new | C1 |
| 3 | "Keep your Waste meter under 75 for 3 days" | `Keep(vital.waste <= 75, 72 h)` | exists (Compost clears it) | | |

75 is the game's `UNSANITARY_THRESHOLD`, a game choice, said so on the step. Rewards: `soap_bar_0`
2; XP medicine 10.

#### `clean_compost` Waste Becomes Soil

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: Your First Compost (Library)" | `Learn("library:your_first_compost")` | exists | planned `start_a_compost_pile` | |
| 2 | "Compost your waste: press I, then Compost" | `Make(fertilizer_0, 1)` | exists; the Compost action must report `make_` events (P8) | | |
| 3 | "Draw off a day of stored urine for the garden" | `Make(urine_stored_0, 1)` | exists, P8 | | C3 |

Rewards: XP farming 15.

### 2.6 First aid

```
aid_first  Ready for Accidents
  ├── aid_wound      Dress a Wound
  ├── aid_burn       Cool a Burn
  └── aid_rehydrate  When Water Leaves You
```

#### `aid_first` Ready for Accidents

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: First Aid Until Help Arrives (Library)" | `Learn("library:first_aid_until_help_arrives")` | exists | | M1 |
| 2 | "Learn: Stop heavy bleeding (How-to)" | `Learn("how_to:stop_heavy_bleeding")` | | planned | M1 |
| 3 | "Keep 2 bandages in your pack" | `Gather(bandage_0, 2)` | exists | | |

**⚠ lethal** on step 2. Rewards: `bandage_0` 2; XP medicine 15.

#### `aid_wound` Dress a Wound

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Next time you are hurt: press I, Bandage, Use" | `Use(bandage_0)` new | exists: a bandage is used only when it helps (`systems::treatment`) | | M1 |

A step that waits for the next real wound (a bite, a burn from the campfire, a fall). It blocks
nothing, because branches never lock each other. Practising on a hurt crewmate is a later rung
(treating others is new). Rewards: XP medicine 20.

#### `aid_burn` Cool a Burn

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Cool a burn (How-to)" | `Learn("how_to:cool_a_burn")` | | planned | M2 |
| 2 | "Next burn: cool it under a running tap" | `Use(provides "wash")` while burned | treatment row new (cool water on a burn wound) | planned | M2 |

**⚠ serious** on both. Rewards: XP medicine 20.

#### `aid_rehydrate` When Water Leaves You

*Rung 1: everything exists (BUG-162's oral rehydration).*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Read: When Food or Water Makes You Sick" | `Learn("library:when_food_or_water_makes_you_sick")` | exists | | W3 |
| 2 | "Learn: Mix an oral rehydration packet (How-to)" | `Learn("how_to:mix_ors_packet")` | | exists | (its own sources) |
| 3 | "Mix oral rehydration salts: Crafting" | `Craft(mix_ors, 1)` | exists | exists | |
| 4 | "Drink it: press I, click it, then Drink" | `Drink(ors_solution_0, 1)` new | exists | | |

Rewards: `ors_sachet_0` 2; XP medicine 15.

### 2.7 Rest

#### `rest_first` A Night's Sleep

*Offered. The Energy meter.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "See your Energy: press I, Inventory > Status" | `View("vitals")` | exists | | R1 |
| 2 | "Sleep a full night in a bed: E at the bed" | `Sleep(7 h)` new | exists (`systems::sleep`) | | R1 |
| 3 | "Go 2 days without feeling fatigued" | `Keep(vital.energy >= 25, 48 h)` | exists | | |

25 is the game's `FATIGUED_THRESHOLD`, a game choice. Rewards: XP shelter_building 10.

### 2.8 Power: one branch onto the existing chain

Power Systems (`tutorial_power_grid`, `data/quests/tutorial.ron`) exists and is not rebuilt. One
branch joins it:

#### `power_backup` Backup Power, Safely

*After `tutorial_power_grid`.*

| # | HUD line | Objective | Needs | How-to | Facts |
|---|---|---|---|---|---|
| 1 | "Learn: Run a portable generator safely" | `Learn("how_to:run_a_generator_safely")` | | planned | H6 |
| 2 | "Get a portable generator" | `Gather(generator_portable_0, 1)` | exists | | |
| 3 | "Set it outside, 6 m from doors and windows" | `Build(generator_pad)` | placement rule new: outdoors, at least 6.1 m (20 ft) from any opening | | H6 |

**⚠ lethal** on every step. Rewards: XP engineering 20.

### 2.9 The capstone

#### `self_reliant` Self-Reliant

*`after: AllOf(["water_store", "food_keep", "warm_first", "air_first", "clean_first", "aid_first",
"rest_first"])`.* One step: `Keep(every vital in its healthy band, 72 h)`, a composite reading
(`vital.all_ok`). Rewards: a title for the profile and XP leadership 50. Built last.

### 2.10 The objective kinds these chains use that do not exist yet

| Kind | Shape | Counted from | Notes |
|---|---|---|---|
| **Drink** | `Drink { item_id: Option<String>, quantity: u32 }` | a `drink_<item>` event, pushed in `food.rs` where a drink is actually consumed (beside the `eat_` push) | `objectives.rs` already says "Drinking is not eating". The smallest piece here. |
| **Keep** | `Keep { reading: String, min: Option<f32>, max: Option<f32>, hours: f32, at: Option<String> }` | the reading, checked on the current step only, on the game clock | `hours: 0` means reach once. `hours > 0` means hold without a break; leaving the band starts the count again. `at` names a destination (`destinations.ron`) the player must be in. Time away from the game neither counts nor breaks it. Progress is whole game minutes under `keep_<step>` in the existing progress map, so nothing new is saved. |
| **Learn** | `Learn { page: String }` | a `learn_<page>` event when the How-to page opens a how-to (`how_to:<id>`) or the Library reader opens a guide (`library:<file>`) during play | Opening counts. Finishing a game run of a how-to is a later kind, **Practise**, when the how-to increments give runs in the game. |
| **Use** | `Use { item_id: Option<String>, provides: Option<String>, quantity: u32 }` | `use_<item>` when an item is actually applied (`systems::treatment` keeps an item that would do nothing, so it counts only real use), or a tool is used at a thing (a water tester at water); `use_<provides>` when a walk-up use runs (`built_uses`: a sink's "wash", a scrubber) | |
| **Sleep** | `Sleep { hours: u32 }` | a `sleep_<whole hours>` event on waking (`systems::sleep`) | Counts a single sleep of at least that long. |
| **Store** | `Store { zone: String, item: Option<String>, quantity: u32 }` | a `store_<zone>_<item>` event when food goes into a container of that storage zone | Rung 3. |
| **Identify** | `Identify { species: String }` | the identification model (P9) | Rung 6. |

**The readings `Keep` names** are a list in data, `data/quests/readings.ron` (label, unit, where
it comes from), so the Quests page can show "Hydration 62, keep it at 60 or more". The sources are
a fixed set in code, one match arm each, and the shipped-quest test refuses a reading not in the
list:

- `vital.*`: `hydration`, `satiation`, `energy`, `oxygen`, `body_temp_c`, `waste` (the `Vitals`
  component); `vital.all_ok` (every vital outside its warning threshold).
- `intake.*`: the rolling last 24 game hours of what the player took in: `water_l_24h`,
  `kcal_24h`, `vitamin_c_mg_24h`, later `protein_g_24h` (P3).
- `stock.*`: litres carried plus kept at home, by what is in them: `safe_water_l`,
  `untreated_water_l` (P5).
- `home.*`: `co2_pct` (`HomeAirState`), `rain_barrel_l`, `reclaimed_water_l`, `ro_permeate_l`,
  `ro_reject_to_garden_l`, `stored_water_cool_l`, `smoke_alarm_floors_missing`, `co_alarm_by_beds`
  (each added with the quest that needs it).

## 3. The facts each chain teaches, with sources

Every fact below: our own words, then the source (publisher, title, link), the source's own date,
the date we read it (all 2026-10-30), and the words that carry it. **Refutation pass: done
2026-10-30**, recorded in [the dated finding](../reference/findings/2026-10-30-quest-chain-facts-review.md): 38 confirmed, 19 corrected, 1 refuted in part
(W21), 3 unconfirmed (W22, A4, C3). Each fact ends with its verdict and a link to its entry there;
the text of a corrected or refuted fact is already the corrected wording, and the finding keeps
what changed and why. None of them goes into a quest, a notice or a how-to until that pass is
merged, and an UNCONFIRMED fact not until it is sourced and checked, exactly as a Curriculum topic
or a how-to moves from `sourced` to `verified`. The government sources (CDC, NIH, EPA, NASA,
OSHA, FEMA, NOAA, USDA, DHS) are public-domain works and are quoted as written; the others (the
National Academies, MedlinePlus's licensed encyclopedia, the University of Georgia's NCHFP,
university extension services, the Missouri Department of Conservation, the Red Cross, the NHS,
the journal article) are quoted only in short phrases.

The finding is the dated findings file the house rule asks for; the how-tos cite it through
`data/sources/registry.json`.

### 3.1 Water

**W1. How much water a person takes in a day.** The US National Academies found that adequately
hydrated women take in about 2.7 L of total water a day and men about 3.7 L, about four fifths from
drinks and one fifth from food, and that most healthy people meet their needs by letting thirst
guide them; very active people and people in hot climates may need more. The European Food Safety
Authority's figures are lower (2.0 L women, 2.5 L men, all sources; already cited in `food.rs` at
`HYDRATION_PER_LITRE`). The game drains 2.5 L a day at rest (Jequier and Constant 2010, also cited
there). The step text says "authorities give 2 to 3.7 litres a day from all sources, food
included" and does not pick one.
- National Academies, "Report Sets Dietary Intake Levels for Water, Salt, and Potassium...",
  https://www.nationalacademies.org/news/report-sets-dietary-intake-levels-for-water-salt-and-potassium-to-maintain-health-and-reduce-chronic-disease-risk
  (last update 2004-02-11). Quotes: "approximately 2.7 liters (91 ounces) of total water"; "men
  average approximately 3.7 liters"; "About 80 percent"; "letting thirst be their guide".
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W1](../reference/findings/2026-10-30-quest-chain-facts-review.md#w1)).

**W2. Water counts from food.** Same source as W1: "the other 20 percent is derived from food."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W2](../reference/findings/2026-10-30-quest-chain-facts-review.md#w2)).

**W3. The signs of drying out.** Thirst, a dry or sticky mouth, not urinating much, darker yellow
urine, dry cool skin, headache, muscle cramps; severe: very dark or no urine, confusion, dizziness,
a fast heartbeat, fast breathing, sunken eyes, shock. Get emergency help if the person loses
consciousness.
- MedlinePlus (US National Library of Medicine), "Dehydration",
  https://medlineplus.gov/ency/article/000982.htm (review date 2025-07-01). Quotes: "Darker yellow
  urine"; "Irritability or confusion"; "The person loses consciousness at any time."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W3](../reference/findings/2026-10-30-quest-chain-facts-review.md#w3)).

**W4. Drink more when you sweat.** W1's source: "those who are very physically active or who live
in hot climates may need to consume more water". The game already takes sweat out of Hydration
(`body_heat::HeatOutcome::sweat_l`).
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W4](../reference/findings/2026-10-30-quest-chain-facts-review.md#w4)).

**W5. Clear cloudy water before treating it.** CDC: bring *clear* water to the boil; filter cloudy
water through a clean cloth, paper towel or coffee filter, or let it settle and draw off the clear
water (the existing how-to `boil_water_safe` carries the second half with its own citation).
- CDC, "How to Make Water Safe in an Emergency",
  https://www.cdc.gov/water-emergency/about/index.html (dated 2024-09-19). Quote: "Bring clear
  water to a rolling boil for 1 minute (at elevations above 6,500 feet, boil for 3 minutes)."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W5](../reference/findings/2026-10-30-quest-chain-facts-review.md#w5)).

**W6. Chemicals are a different problem.** Neither boiling nor disinfecting makes water with fuel,
toxic chemicals or radioactive material safe. **⚠ lethal, on screen.**
- W5's source. Quote: "You cannot make water containing fuel, toxic chemicals, or radioactive
  materials safe by boiling or disinfecting it."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W6](../reference/findings/2026-10-30-quest-chain-facts-review.md#w6)): EPA (2026) agrees.

**W7. Boiling kills every kind of germ.** Viruses, bacteria and parasites, Cryptosporidium and
Giardia included.
- CDC, "A Guide to Drinking Water Treatment and Sanitation for Backcountry and Travel Use",
  https://www.cdc.gov/healthywater/drinking/travel/backcountry_water_treatment.html (dated
  2025-01-30). Quote: "Boiling kills all types of germs, including viruses, bacteria, and parasites
  (like Cryptosporidium and Giardia)."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W7](../reference/findings/2026-10-30-quest-chain-facts-review.md#w7)).

**W8. How long to boil.** A rolling boil for 1 minute; 3 minutes above 6,500 feet (CDC). The EPA's
line is 5,000 feet, and the existing how-to resolves the disagreement by boiling longer from 1,000
m (its step `height`). Aboard, the home's air is held at 101.3 kPa (`target_pressure_kpa` in
`data/hvac.ron`), sea-level pressure, so 1 minute; on a planet surface, the player's height. W5's quote; W7's source: "At elevations above 6,500 feet, boil for 3
minutes."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W8](../reference/findings/2026-10-30-quest-chain-facts-review.md#w8)): EPA writes "5,000 feet (1,000 meters)", its own figures not matching; the how-to's 1,000 m is the cautious reading.

**W9. Household bleach: what kind and how much.** Plain unscented household bleach of 5% to 9%
sodium hypochlorite: 2 drops per litre or quart of clear water, 8 drops (about ½ mL) per gallon;
twice that if the water is cloudy, murky, coloured or very cold; stir, then wait at least 30 minutes
before drinking. For 1% bleach, 10 drops per litre. The water should smell slightly of chlorine; if
it does not, dose again and wait 15 minutes more, and if it still does not, do not drink it.
**⚠ lethal, on screen** (wrong product or dose).
- W5's source. Quotes: "it will typically be between 5% and 9% sodium hypochlorite"; "Add 2 drops
  of bleach"; "Add ½ mL of bleach" (per gallon); "If the water is cloudy, murky, colored, or very
  cold, add double the amount of bleach listed below."; "Let the water stand for at least 30 minutes
  before you drink it."
- EPA, "Emergency Disinfection of Drinking Water",
  https://www.epa.gov/ground-water-and-drinking-water/emergency-disinfection-drinking-water (last
  updated 2026-02-24): 6% bleach 8 drops a gallon, 8.25% bleach 6 drops; "Double the amount of
  bleach if the water is cloudy, colored, or very cold."; "The water should have a slight chlorine
  odor."
- Ready.gov (FEMA), "Water", https://www.ready.gov/water (last updated 2021-02-22) gives a
  different strength and dose: "regular household liquid bleach that contains 5.25 to 6.0 percent
  sodium hypochlorite", "Add 1/8 teaspoon of bleach per gallon of water"; and "If it still does not
  smell of chlorine, discard it and find another source of water." See D2.
- **Refutation pass 2026-10-30: CORRECTED** ([finding W9](../reference/findings/2026-10-30-quest-chain-facts-review.md#w9)): added the double dose for cloudy, murky, coloured or very cold water (CDC, EPA), the ½ mL measure, and Ready.gov's smell check.

**W10. Bleach and iodine do not kill Cryptosporidium.** Chlorine and iodine kill most germs but
not Cryptosporidium, and do not work well against parasites in general.
- W5's source: "Iodine, tablets with iodine (tetraglycine hydroperiodide), or chlorine tablets kill
  most germs, but will not kill Cryptosporidium germs."
- W7's source: "Chlorine bleach and iodine kill bacteria and viruses but do not work well to kill
  parasites."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W10](../reference/findings/2026-10-30-quest-chain-facts-review.md#w10)): the Yellow Book agrees; iodine's medical limits found (3.6).

**W11. Chlorine dioxide does better against parasites.** Chlorine dioxide tablets kill Giardia,
and kill Cryptosporidium only when the product's instructions are followed, which means waiting
several hours. CDC's pages differ on how reliably (D1), and there is little data on cloudy or cold
water, so the game takes the cautious reading: partly, and only after the full wait.
- W5's source: "Chlorine dioxide tablets will kill Cryptosporidium as long as you follow the
  instructions correctly."
- W7's source: "Chlorine dioxide tablets can kill the parasite Giardia and are somewhat effective
  at killing Cryptosporidium."
- W12's Yellow Book chapter: ClO2 kills waterborne pathogens "including Cryptosporidium oocysts, at
  practical doses and contact times"; tablets and drops "require prolonged contact time of several
  hours"; "relatively few data are available on the use of these products for different water
  conditions".
- **Refutation pass 2026-10-30: CORRECTED** ([finding W11](../reference/findings/2026-10-30-quest-chain-facts-review.md#w11)): the Yellow Book: kills Cryptosporidium at practical doses and contact times of several hours, with few data across water conditions; the game stays at partly, after the full wait.

**W12. What a filter's pore size catches.** An absolute pore size of 1 micron or smaller removes
parasites; 0.3 micron or smaller removes bacteria and parasites; neither removes viruses.
Ultrafilters of about 0.01 micron remove viruses too.
- W5's source: "Must have an absolute pore size of 1 micron or smaller to remove parasites (such as
  Giardia or Cryptosporidium)."; "Must have an absolute pore (hole) size of 0.3 micron or smaller
  to remove bacteria"; "Will not remove viruses".
- CDC Yellow Book 2026, Backer and Hill, "Water Disinfection for Travelers",
  https://www.cdc.gov/yellow-book/hcp/preparing-international-travelers/water-disinfection-for-travelers.html
  (dated 2025-04-23): "Ultrafilters with pore size of 0.01 µm should be effective for removing
  viruses, bacteria, and parasites."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W12](../reference/findings/2026-10-30-quest-chain-facts-review.md#w12)).

**W13. Filter, then disinfect.** When boiling is not possible, filter the water and then disinfect
the filtered water.
- W7's source: "The next best option is to filter water and then disinfect the filtered water."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W13](../reference/findings/2026-10-30-quest-chain-facts-review.md#w13)).

**W14. Clay pot filters.** CDC lists ceramic pot filters among household treatments shown to cut
diarrhoea, with drawbacks: unknown effectiveness against viruses, no lasting protection so the water
can be contaminated again, the need to keep the filter and its container clean, and slow flow. A
2025 systematic review found they take out viruses poorly: a mean of 1.3 log (about 95%) for plain
ceramic and 1.9 log with silver or metal-oxide additives, almost all from laboratory tests. A clay
filter does not make water safe from viruses.
- CDC, "Preventing Diarrheal Disease in Developing Countries: Proven Household Water Treatment
  Options", https://stacks.cdc.gov/view/cdc/51530 (January 2010). Quote: "Drawbacks include unknown
  effectiveness against viruses, lack of residual protection that can lead to recontamination".
- Rao, Wells, Reynolds, Yoo, Kowalsky, DeFrance, Linden and Brown, "Systematic Review of the
  Microbiological Performance of Household Water Treatment Technologies", Environmental Science &
  Technology 59(41):21776-21789 (2025), doi:10.1021/acs.est.4c03494,
  https://pmc.ncbi.nlm.nih.gov/articles/PMC12550818/. Quote: "viral reductions at 1.9 (1.4–2.5)"
  (with additives). Its Table 1: plain ceramic, viruses, 1.3 (1.0–1.6) from 56 point estimates, 54
  of them in the laboratory.
- **Refutation pass 2026-10-30: CORRECTED** ([finding W14](../reference/findings/2026-10-30-quest-chain-facts-review.md#w14)): was "reduced viruses less than bacteria (1.9)"; now also plain ceramic 1.3 log, almost all from laboratory tests.

**W15. Sand filters.** CDC lists the household slow sand filter: it clears cloudiness, flows well
and lasts, with unknown effectiveness against viruses and the same recontamination risk. The 2025
review found high virus removal in the laboratory (4.7 log) but only 1.7 log in the few
measurements under typical household use, and the lowest bacteria removal of any method it compared
(1.9 log). A sand filter clears cloudiness and some germs; the water still needs disinfecting (D5).
- W14's CDC source: "Benefits include turbidity removal, high flow rate, ease of use, long-life".
- W14's review: "GMF technology overall performed better for viral microbes (LRV = 4.5 [4.0–5.0])",
  a result the authors call unexpected. Its Table 1 splits the 4.5: 4.7 (4.2–5.2) from 72 laboratory
  point estimates, 1.7 (1.1–2.3) from 5 under typical operating conditions.
- **Refutation pass 2026-10-30: CORRECTED** ([finding W15](../reference/findings/2026-10-30-quest-chain-facts-review.md#w15)): was "did better against viruses than against bacteria (4.5)"; that figure is laboratory data, and typical household use gave 1.7 log.

**W16. Charcoal traps but does not kill.** Activated charcoal filters trap microorganisms without
killing them and are generally not rated to remove microbes.
- W12's Yellow Book chapter: "GAC filters trap, but do not kill, microorganisms and they are
  generally not rated for microbe removal."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W16](../reference/findings/2026-10-30-quest-chain-facts-review.md#w16)).

**W17. Rainwater is not safe to drink as it falls off a roof.** Germs (bird droppings, for one)
and chemicals that get into it once it falls (asbestos, lead or copper) wash in; a first flush diverter
sends the first rain of a storm away from the store; if you drink, cook or bathe with it, test it
regularly for germs and chemicals and treat it for what is found. Killing germs does not take out
lead.
- CDC, "Collecting Rainwater and Your Health: An Overview",
  https://www.cdc.gov/drinking-water/about/collecting-rainwater-and-your-health-an-overview.html
  (dated 2024-07-23). Quotes: "Rainwater is not necessarily safe to drink without first removing
  germs and chemicals from it."; "bird poop on your roof can end up in your rainwater barrel or
  tank."; "Once rainwater falls, chemicals like asbestos, lead, or copper can get into the water.";
  "The device directs the first rainwater that falls during a storm away from your rainwater
  collection system."
- **Refutation pass 2026-10-30: CORRECTED** ([finding W17](../reference/findings/2026-10-30-quest-chain-facts-review.md#w17)): added the roof's chemicals (asbestos, lead, copper) and treating for what a test finds.

**W18. Test it rather than guess.** W17's source: "If you drink, cook, or bathe with rainwater, test
it regularly for germs and chemicals." What a home kit can and cannot tell you is still to find
out (3.6).
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W18](../reference/findings/2026-10-30-quest-chain-facts-review.md#w18)).

**W19. Sunlight (SODIS).** Clear plastic bottles laid on their side in sunlight for at least 6
hours, shaken now and then; 2 days running under cloud; clear very cloudy water first.
- W12's Yellow Book chapter: "exposed to sunlight for a minimum of 6 hours with intermittent
  agitation"; "Under cloudy weather conditions, water must be placed in the sunlight for 2
  consecutive days."; "travelers should clarify highly turbid water first."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W19](../reference/findings/2026-10-30-quest-chain-facts-review.md#w19)).

**W20. UV light needs clear water.** Portable UV kills parasites, bacteria and viruses in small
amounts of clear water; cloudy water shields germs from it.
- W7's source: "Portable units that deliver ultraviolet (UV) light can kill parasites, bacteria, and
  viruses in small amounts of clear water."; "UV light does not work well in cloudy water because
  small particles can block germs from the light."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W20](../reference/findings/2026-10-30-quest-chain-facts-review.md#w20)).

**W21. Distillation.** Boil the water and collect only the vapour that condenses: with a cup hung
under an upturned pot lid, boil for 20 minutes; the water that drips into the cup is distilled.
Distillation removes germs, salts and heavy metals. It does not remove chemicals that boil off with
the water, such as solvents and some pesticides: they condense back into the cup. Never count on a
pot still for water with fuel or solvents in it (CDC already says boiling cannot make
fuel-contaminated water safe, W6).
- W9's Ready.gov page: "Distillation involves boiling water and then collection of only the vapor
  that condenses."; "boil the water for 20 minutes."; "The water that drips from the lid into the
  cup is distilled." Its "as well as heavy metals, salts and most other chemicals." is refuted for
  volatile chemicals by the next two sources.
- University of Nebraska-Lincoln Extension, Dvorak and Skipton, "Drinking Water Treatment:
  Distillation", G1493, https://extensionpubs.unl.edu/publication/g1493/html/view (revised December
  2013): volatile solvents and VOCs such as benzene and toluene with boiling points near or below
  water's "will vaporize along with the water as it is boiled in the distiller".
- Connecticut Department of Public Health, "Distillation treatment systems for private drinking
  water systems",
  https://portal.ct.gov/dph/knowledge-base/articles/environmental-health/drinking-water/distillation-treatment
  (dated 2026-06-17): distillation is "not effective at removing most volatile organic compounds".
- **Refutation pass 2026-10-30: REFUTED in part** ([finding W21](../reference/findings/2026-10-30-quest-chain-facts-review.md#w21)): Ready.gov's "most other chemicals" is contradicted by Nebraska Extension and Connecticut DPH for chemicals that boil with the water; the procedure is confirmed.

**W22. A solar still.** Same physics as W21 with the sun as the heat. How much water one makes in
a day is **still to find out** (3.6): no yield number goes into the game until it is sourced.
- **Refutation pass 2026-10-30: UNCONFIRMED** ([finding W22](../reference/findings/2026-10-30-quest-chain-facts-review.md#w22)): still no yield from a reviewed source.

**W23. The space station recovers 98% of its water.** Its Urine Processor recovers water from
urine by vacuum distillation; a brine processor then evaporates the water left in the brine with
warm dry air; total recovery rose from 93 to 94% to 98%. Each crew member needs about a gallon a
day for drinking, food and hygiene. NASA's life support baseline (2022 edition) gives each crew
member 2.00 kg of drinking water and 0.5 kg for preparing food a day, plus 0.717 kg more for the
sweat of a daily exercise regime (3.217 kg in all; the 2018 edition gave 2.5 kg).
- NASA, "NASA Achieves Water Recovery Milestone on International Space Station",
  https://www.nasa.gov/missions/station/iss-research/nasa-achieves-water-recovery-milestone-on-international-space-station
  (published 2023-06-20, last updated 2026-03-02). Quotes: "recovers water from urine using vacuum
  distillation"; "Before the BPA, our total water recovery was between 93 and 94% overall"; "Each
  crew member needs about a gallon of water per day for consumption, food preparation, and
  hygiene".
- A1's 2022 source, Table 3-31: "Potable Water Content kg/CM-d 3.217", "0.5 kg used for food
  preparation, 2.00 kg for drinking water, and an additional 0.717 kg added due to increased
  perspiration as a result of the increased exercise profile".
- **Refutation pass 2026-10-30: CORRECTED** ([finding W23](../reference/findings/2026-10-30-quest-chain-facts-review.md#w23)): the baseline figure now from NASA's 2022 edition.

**W24. Reverse osmosis.** Pressure forces water through a semi-permeable membrane; RO filters
remove parasites, bacteria and viruses, and salt; the EPA lists lead, VOCs, PFAS and arsenic among
what they can potentially remove. Of portable methods, only RO desalinates.
- EPA WaterSense, "Point-of-Use Reverse Osmosis Systems",
  https://www.epa.gov/watersense/point-use-reverse-osmosis-systems (last updated 2026-08-21).
  Quote: "RO is the process by which pressure forces water through a semi-permeable membrane,"
- W7's source: "Reverse osmosis filters remove parasites, bacteria, and viruses. They also remove
  salt from water."
- W12's Yellow Book chapter: "Only reverse osmosis can desalinate water for ocean travel."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W24](../reference/findings/2026-10-30-quest-chain-facts-review.md#w24)): quotation punctuation corrected.

**W25. RO wastes water.** A typical point-of-use unit sends five gallons or more of reject water
down the drain for each gallon it makes; a WaterSense-labelled one, 2.3 or less.
- W24's EPA page: "five gallons or more of reject water for every gallon of treated water
  produced"; "just 2.3 gallons of water or less down the drain for every gallon of treated water
  they produce."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W25](../reference/findings/2026-10-30-quest-chain-facts-review.md#w25)).

**W26. How much to store.** At least 1 gallon (3.8 L) per person per day for 3 days, for drinking,
cooking and brushing teeth (Ready.gov: "for several days", for drinking and sanitation); a 2-week
supply if possible; more for pregnant women, people who are sick, pets, or a hot climate.
- CDC, "How to Create an Emergency Water Supply",
  https://www.cdc.gov/water-emergency/about/how-to-create-and-store-an-emergency-water-supply.html
  (dated 2025-06-27). Quotes: "Store at least 1 gallon of water per person, per day for 3 days.";
  "Try to store a 2-week supply if possible."; "You can use this water during an emergency for
  drinking, cooking, brushing teeth, and other uses."; "Consider storing more water than this for
  pregnant women, people who are sick, pets, or if you live in a hot climate."
- **Refutation pass 2026-10-30: CORRECTED** ([finding W26](../reference/findings/2026-10-30-quest-chain-facts-review.md#w26)): CDC says drinking, cooking and brushing teeth ("drinking and sanitation" is Ready.gov's); more for some people.

**W27. How to keep it.** Food-grade containers; never a container that held toxic chemicals;
somewhere cool (50 to 70 °F, 10 to 21 °C) and out of direct sunlight; replace it every six months.
- W26's source: "Do not use containers that were previously used to hold liquid or solid toxic
  chemicals, such as bleach or pesticides."; "Keep containers in a place with a cool temperature
  (50°F –70°F)."; "Replace water every six months."
- W9's Ready.gov page agrees on six months for water that is not commercially bottled.
- **Refutation pass 2026-10-30: CONFIRMED** ([finding W27](../reference/findings/2026-10-30-quest-chain-facts-review.md#w27)).

**W28. Sanitising a container.** Wash with soap and rinse; mix 1 teaspoon of unscented household
bleach (5 to 9%) in 1 quart of water in the container; cover it tightly and shake it well; wait at
least 30 seconds; pour it out; let it air-dry. **⚠ serious** (bleach).
- W26's source: "mixing 1 teaspoon of unscented liquid household chlorine bleach in 1 quart (4
  cups) of water"; "Cover the container tightly and shake it well."; "Wait at least 30 seconds and
  then pour the sanitizing solution out of the container."
- **Refutation pass 2026-10-30: CORRECTED** ([finding W28](../reference/findings/2026-10-30-quest-chain-facts-review.md#w28)): CDC says cover and shake, not swish.

### 3.2 Food

**F1. 2,000 kcal is a guide, not a requirement.** Food labels use 2,000 kcal a day as a general
guide; a person's need may be higher or lower with age, sex, height, weight and activity. (The
game's `DAILY_KCAL` cites the same figure from 21 CFR 101.9(d)(9).)
- FDA, "How to Understand and Use the Nutrition Facts Label",
  https://www.fda.gov/food/nutrition-facts-label/how-understand-and-use-nutrition-facts-label
  (content current as of 2024-03-05). Quotes: "2,000 calories a day is used as a general guide for
  nutrition advice."; "Your calorie needs may be higher or lower and vary depending on your age,
  sex, height, weight, and physical activity level."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F1](../reference/findings/2026-10-30-quest-chain-facts-review.md#f1)).

**F2. The four steps, and rinsing produce.** Clean, separate, cook, chill; wash hands at least 20
seconds; rinse fresh fruit and vegetables under running water.
- CDC, "Preventing Food Poisoning", https://www.cdc.gov/food-safety/prevention/index.html (dated
  2025-11-24). Quotes: "clean, separate, cook, and chill."; "Rinse fresh fruits and vegetables
  under running water."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F2](../reference/findings/2026-10-30-quest-chain-facts-review.md#f2)).

**F3. Safe internal temperatures.** Poultry 165 °F (74 °C); ground meat 160 °F (71 °C); whole
cuts of beef, veal, lamb and pork 145 °F (63 °C), then 3 minutes' rest; fish with fins 145 °F, or
until the flesh is opaque and flakes with a fork; leftovers and casseroles 165 °F. (Celsius figures
are our conversion; CDC gives Fahrenheit only. FoodSafety.gov's chart and FSIS refuse automated
reading; CDC's page carries the same list.) **⚠ serious, on screen.**
- F2's source. Quotes: "All poultry, including ground chicken and turkey": "165°F"; "Ground meats,
  such as beef and pork": "160°F"; "145°F (then allow the meat to rest for 3 minutes before carving
  or eating)"; "Fish with fins": "145°F or cook until the flesh is opaque and separates easily with
  a fork"; "Leftovers and casseroles": "165°F".
- **Refutation pass 2026-10-30: CORRECTED** ([finding F3](../reference/findings/2026-10-30-quest-chain-facts-review.md#f3)): "fish with fins"; the Celsius figures are our conversion.

**F4. Two hours out, one hour in the heat.** Refrigerate perishable food within 2 hours, within 1
hour if it was above 90 °F (32 °C); bacteria multiply fast between 40 and 140 °F (4 and 60 °C).
- F2's source (as read: refrigerate within 2 hours; within 1 hour above 90°F; the "Danger Zone"
  between 40°F and 140°F).
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F4](../reference/findings/2026-10-30-quest-chain-facts-review.md#f4)).

**F5. Fridge and freezer.** The fridge at or below 40 °F (4 °C), the freezer at 0 °F (−18 °C), checked
with an appliance thermometer; freezing stops bacteria growing but does not kill most of them.
- FDA, "Are You Storing Food Safely?",
  https://www.fda.gov/consumers/consumer-updates/are-you-storing-food-safely (content current as
  of 2023-01-18). Quotes: "Keep the refrigerator temperature at or below 40° F (4° C)."; "The
  freezer temperature should be 0° F (-18° C)."; "While freezing does not kill most bacteria, it
  does stop bacteria from growing."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F5](../reference/findings/2026-10-30-quest-chain-facts-review.md#f5)).

**F6.** The game's storage zones already slow spoilage by temperature
(`FoodData::spoilage_multiplier_at`); F5 is what the fridge's temperature must be, and the `cold`
zone's band does not yet match it (3.5).
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F6](../reference/findings/2026-10-30-quest-chain-facts-review.md#f6)): the description fix in 3.5 landed in BUG-216.

**F7. Drying.** Dehydrators and ovens dry at about 140 °F (60 °C). (The general drying facts beyond
the jerky source were not read from a primary source this pass: 3.6.)
- NCHFP (University of Georgia), "Jerky", https://nchfp.uga.edu/how/dry/jerky (no page date; from
  So Easy to Preserve, 6th ed., 2014). Quote: "preheated to 140ºF".
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F7](../reference/findings/2026-10-30-quest-chain-facts-review.md#f7)).

**F8. Jerky: heat it before or after drying.** Heat the meat to an internal 160 °F (71 °C),
poultry to 165 °F (74 °C), because drying alone may not kill E. coli O157:H7 or Salmonella. USDA
heats it before drying; NCHFP also allows heating the dried strips in a 275 °F oven afterwards. Then
dry at 130 to 140 °F; never sun-dry meat. **⚠ serious, on screen.**
- F7's source. Quotes: "allowing the internal temperature of the meat to reach 160ºF"; "necessary
  to eliminate disease-causing bacteria such as E. coli O157:H7, if present."
- University of Kentucky Cooperative Extension, Bastin and Norris, "Jerky Safety", FCS3-594,
  https://publications.ca.uky.edu/sites/publications.ca.uky.edu/files/FCS3594.pdf (issued March
  2017, citing USDA FSIS, "Jerky and Food Safety", 2016): USDA recommends "heating meat to 160°F
  and poultry to 165°F before the dehydrating process".
- **Refutation pass 2026-10-30: CORRECTED** ([finding F8](../reference/findings/2026-10-30-quest-chain-facts-review.md#f8)): the 165 °F poultry figure is now sourced (University of Kentucky Extension citing USDA FSIS); USDA heats before drying.

**F9. Sauerkraut.** About 3 tablespoons of canning salt to 5 pounds of cabbage; ferment at 70 to
75 °F (21 to 24 °C) for about 3 to 4 weeks, or 5 to 6 weeks at 60 to 65 °F; below 60 °F it may not
ferment, above 75 °F it may go soft; keep the cabbage under brine with a plate and weights.
- NCHFP, "Sauerkraut", https://nchfp.uga.edu/how/ferment/sauerkraut (no page date; adapted from the
  USDA Complete Guide to Home Canning, revised 2015). Quotes: "add 3 tablespoons of salt"; "kraut
  will be fully fermented in about 3 to 4 weeks"; "At temperatures lower than 60ºF, kraut may not
  ferment."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F9](../reference/findings/2026-10-30-quest-chain-facts-review.md#f9)).

**F10. Low-acid food only in a pressure canner.** Low-acid foods (pH above 4.6: all fresh
vegetables, figs, meats, poultry, fish, seafood and some tomatoes) must be canned in a pressure
canner, the only recommended method; a boiling-water canner does not protect against botulism.
Home-canned vegetables are the most common cause of botulism outbreaks in the US. You cannot see,
smell or taste the toxin. Use only recipes that follow the USDA guide. **⚠ lethal, on screen.**
- CDC, "Home-Canned Foods" (botulism prevention),
  https://www.cdc.gov/botulism/prevention/home-canned-foods.html (dated 2024-04-25). Quotes: "the
  only recommended method for canning low-acid foods"; "Home-canned vegetables, which are low-acid
  foods, are the most common cause of botulism outbreaks in the United States."; "You cannot see,
  smell, or taste the toxin that causes botulism."; "Use only recipes and cookbooks that follow the
  steps in the USDA guide."
- **Refutation pass 2026-10-30: CORRECTED** ([finding F10](../reference/findings/2026-10-30-quest-chain-facts-review.md#f10)): CDC's own words on the most common cause of outbreaks; figs and seafood added.

**F11. Why: the pH line and the canner's temperature.** Acid foods are pH 4.6 or lower; low-acid
foods are sterilised at 240 to 250 °F (116 to 121 °C), which pressure canners reach at 10 to 15 PSIG;
botulinum spores are very hard to destroy at boiling-water temperatures.
- NCHFP, "Ensuring Safe Canned Foods",
  https://nchfp.uga.edu/how/can/general-information/ensuring-safe-canned-foods (no page date;
  adapted from the USDA Complete Guide, revised 2015). Quotes: "Acid foods have a pH of 4.6 or
  lower."; "all low-acid foods should be sterilized at temperatures of 240° to 250°F"; "Botulinum
  spores are very hard to destroy at boiling-water temperatures".
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F11](../reference/findings/2026-10-30-quest-chain-facts-review.md#f11)).

**F12. Throw out a bad jar.** A container that leaks, bulges or is swollen, looks damaged, cracked
or abnormal, or spurts liquid or foam when opened, and food that is discoloured, mouldy or smells
bad. Never taste it to find out. "If in doubt, throw it out!" **⚠ lethal.**
- F10's source. Quotes: "The container is leaking, bulging, or swollen"; "The container looks
  damaged, cracked, or abnormal"; "If in doubt, throw it out! Never taste food to see if it's
  safe."
- **Refutation pass 2026-10-30: CORRECTED** ([finding F12](../reference/findings/2026-10-30-quest-chain-facts-review.md#f12)): added damaged or cracked containers and "Never taste food to see if it's safe."

**F13. Vitamin C.** Adults need 90 mg a day (men) or 75 mg (women), 35 mg more for smokers; scurvy
can occur when intake stays below 10 mg a day for many weeks; good sources include orange juice,
grapefruit juice, peaches, sweet red peppers and papayas.
- CDC, National Report on Biochemical Indicators of Diet and Nutrition in the U.S. Population,
  "Vitamin C" (2026), https://www.cdc.gov/nutrition-report/media/pdfs/2026/06/Water-Soluble-Vitamin-C-508.pdf.
  Quotes: "scurvy, which can occur if intake is below 10 mg/d for many weeks"; "For men, this
  equates to 90 mg/d, with 75 mg being the appropriate daily amount for women".
- MedlinePlus, "Scurvy", https://medlineplus.gov/ency/article/000355.htm (review date 2025-07-03):
  "general weakness, anemia, gum disease" (its signs). (NIH ODS refused automated reading.)
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F13](../reference/findings/2026-10-30-quest-chain-facts-review.md#f13)).

**F14. Wild mushrooms.** Edible and deadly Amanita mushrooms can be hard to tell apart by looks
alone; cooking does not destroy amatoxins; symptoms are delayed, often more than 6 hours. **⚠
lethal, on screen.**
- CDC MMWR 75(20):258-263, Brandecker et al., "Amanita Species Mushroom Poisonings, Northern
  California, November 2025-March 2026", https://www.cdc.gov/mmwr/volumes/75/wr/mm7520a2.htm
  (2026-05-28). Quotes (both sentences continue past these words): "Edible and toxic Amanita species can be
  difficult to distinguish based on appearance alone"; "Amatoxins are not denatured by cooking or
  other methods of food preparation"
- **Refutation pass 2026-10-30: CONFIRMED** ([finding F14](../reference/findings/2026-10-30-quest-chain-facts-review.md#f14)): quotation punctuation corrected (both sentences continue).

**F15. Poison hemlock and wild carrot.** Eating any part of poison hemlock can kill, and its sap
can blister skin, so handle it only in gloves. Its stems are smooth, often waxy, and spotted or
mottled with purple; wild carrot's are hairy without purple marks. Wild carrot's flower cluster
usually has one purple floret in the centre and poison hemlock's florets are all white, but a missing
purple floret does not mean hemlock. Poison hemlock's leaves smell musty. Common water hemlock,
probably the most toxic plant in North America, is another lookalike. The state's advice: if you are
not experienced at identifying plants, treat every wild member of the carrot family as possibly
deadly. **⚠ lethal, on screen.**
- Missouri Department of Conservation, "Poison Hemlock" (field guide),
  https://mdc.mo.gov/discover-nature/field-guide/poison-hemlock (no page date). Quotes: "consume any
  part of this plant can be fatally poisoned"; "The sap can cause blistering and welts in contact
  with skin."; "consider all wild members of the carrot family as potentially fatally toxic."
- Missouri Department of Conservation, "Queen Anne's Lace (Wild Carrot)",
  https://mdc.mo.gov/discover-nature/field-guide/queen-annes-lace-wild-carrot (no page date): "The
  central floret of the entire cluster is usually purple." (D12).
- **Refutation pass 2026-10-30: CORRECTED** ([finding F15](../reference/findings/2026-10-30-quest-chain-facts-review.md#f15)): the purple floret is "usually" there, so its absence proves nothing; water hemlock added; gloves.

### 3.3 Warmth, air, cleanliness, first aid and rest

**H1. Hypothermia.** Signs in adults: shivering, exhaustion, confusion, fumbling hands, memory loss,
slurred speech, drowsiness; below 95 °F (35 °C), get medical help at once. (The game's
`hypothermia` status starts at a 35 °C core: they agree.) **⚠ lethal, on screen.**
- CDC, "Preventing Hypothermia", https://www.cdc.gov/winter-weather/prevention/index.html (dated
  2026-08-25). Quotes: "shivering, exhaustion, confusion, fumbling hands, memory loss, slurred
  speech, drowsiness."; "If it is below 95° F, get medical attention immediately!"
- **Refutation pass 2026-10-30: CONFIRMED** ([finding H1](../reference/findings/2026-10-30-quest-chain-facts-review.md#h1)).

**H2. What to do when help is not near.** A warm room; wet clothing off; warm the centre of the
body (chest, neck, head, groin) first, with an electric blanket or skin to skin under loose dry
layers; warm drinks for someone awake but no alcohol, and never a drink for someone unconscious; CPR
even if the person seems dead; then medical help as soon as possible. H1's source: "Warm the center
of the person's body"; "do not give alcoholic drinks"; "Do not try to give beverages to an
unconscious person."; "Perform CPR, even if the person appears dead."
- **Refutation pass 2026-10-30: CORRECTED** ([finding H2](../reference/findings/2026-10-30-quest-chain-facts-review.md#h2)): added "Do not try to give beverages to an unconscious person." and skin-to-skin warming.

**H3. A fire extinguisher.** Pull the pin, aim low at the base of the fire, squeeze, sweep side to
side; back away in case it flares again; get out if the extinguisher empties or the fire grows
past its first stage, and never let the fire come between you and the way out. **⚠ serious.**
- OSHA eTool, "Portable Fire Extinguishers: Fire Extinguisher Use",
  https://www.osha.gov/etools/evacuation-plans-procedures/emergency-standards/portable-extinguishers/use
  (no date shown). Quotes: "Aim low, pointing the extinguisher nozzle (or its horn or hose) at the
  base of the fire."; "Do not allow the fire, heat, or smoke to come between you and your
  evacuation path."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding H3](../reference/findings/2026-10-30-quest-chain-facts-review.md#h3)).

**H4. Smoke alarms.** Test every month; one in every bedroom, outside each sleeping area, and on
every level, the basement included; interconnected if possible; replace 9-volt batteries at least
yearly; replace the alarm every 10 years.
- US Fire Administration (FEMA), "Smoke alarms",
  https://www.usfa.fema.gov/prevention/home-fires/prepare-for-fire/smoke-alarms/ (last reviewed
  2026-10-13). Quotes: "Test smoke alarms every month"; "Put alarms on every level of the home.";
  "Choose interconnected smoke alarms so that when one sounds, they all sound."; "Get new smoke
  alarms every 10 years." The page's placement list: "in every bedroom, outside each separate
  sleeping area, and on every level of a home".
- **Refutation pass 2026-10-30: CORRECTED** ([finding H4](../reference/findings/2026-10-30-quest-chain-facts-review.md#h4)): USFA: in every bedroom, outside each sleeping area, and on every level.

**H5. Carbon monoxide.** It has no smell or colour; common symptoms are headache, dizziness,
weakness, upset stomach, vomiting, chest pain and confusion; sleeping people can die before they
have symptoms; furnaces, generators and charcoal grills make it. **⚠ lethal, on screen.**
- CDC, "Carbon Monoxide (CO) Poisoning Basics", https://www.cdc.gov/carbon-monoxide/about/index.html
  (dated 2026-10-26). Quotes: "Carbon monoxide (CO) is an odorless, colorless gas that kills without
  warning."; "die from CO poisoning before they have symptoms."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding H5](../reference/findings/2026-10-30-quest-chain-facts-review.md#h5)).

**H6. Preventing it.** Battery or battery-backup CO alarms near every sleeping area, batteries
checked each spring and autumn; generators only outdoors, more than 20 feet (6.1 m) from windows,
doors and vents; never heat the home with a gas oven. **⚠ lethal.**
- H5's source. Quotes: "Install battery-operated or battery back-up CO detectors near every
  sleeping area in your home."; "Only use generators outside, more than 20 feet away from any
  windows, doors, and vents."; "Never heat your house with a gas oven."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding H6](../reference/findings/2026-10-30-quest-chain-facts-review.md#h6)).

**H7. Heat stroke and heat exhaustion.** Heat stroke: a body temperature of 103 °F (39.4 °C) or
higher, hot red dry or damp skin, a fast strong pulse, confusion, passing out: call the emergency
number, move the person somewhere cooler, cool them with cool cloths or a bath, and give them
nothing to drink. Heat exhaustion: heavy sweating, cold pale clammy skin, a fast weak pulse,
nausea, cramps: move somewhere cool, loosen clothes, cool wet cloths, sip water; get help if
vomiting, worse, or lasting more than an hour. **⚠ lethal, on screen.**
- CDC, "Heat-Related Illnesses" (sheet CS280226),
  https://www.cdc.gov/natural-disasters/media/pdfs/2026/09/Heat_Related_Illness-NEWLOGO.pdf (served
  2026; the file's own metadata says 2017). Quotes: "High body temperature (103°F or higher)"; "Call
  911 right away-heat stroke is a medical emergency"; "Do not give the person anything to drink";
  "Sip water".
- **Refutation pass 2026-10-30: CONFIRMED** ([finding H7](../reference/findings/2026-10-30-quest-chain-facts-review.md#h7)): MedlinePlus differs on drinks (D10) and gives 104 °F (D6).

**H8.** H7 meets the hydration chain: in heat stroke you give the person nothing to drink (CDC),
and nobody who is vomiting or unconscious gets anything by mouth (MedlinePlus, "Heat emergencies",
https://medlineplus.gov/ency/article/000056.htm, review date 2025-10-14, which otherwise lets an alert
person sip a drink: D10). So the Water chain's "drink more in the heat" (W4) never appears without
H7 beside it in the warmth chain's text.
- **Refutation pass 2026-10-30: CORRECTED** ([finding H8](../reference/findings/2026-10-30-quest-chain-facts-review.md#h8)): added MedlinePlus: nothing by mouth for anyone vomiting or unconscious.

**A1. What a person breathes.** NASA's life support baseline (2022 edition) for an 82 kg crew
member who exercises daily: 0.895 kg of oxygen in and 1.085 kg of carbon dioxide out a day, on
12.78 MJ of food. (The 2018 edition gave 0.816 and 1.04; the game's 2004 figures, 0.835 and 0.998
on 11.82 MJ, scale to within 1% of the 2022 edition per unit of food energy, our arithmetic: D9.)
- Ewert, Chen and Powell, "Life Support Baseline Values and Assumptions Document",
  NASA/TP-2015-218570/REV2 (February 2022), https://ntrs.nasa.gov/citations/20210024855, Table 3-31
  "Summary of Nominal Human Metabolic Interface Values". Quotes: "Carbon Dioxide Load kg/CM-d
  1.085"; "Oxygen Consumed kg/CM-d 0.895"; "Food Consumed; Energy Content MJ/CM-d 12.778"; "for a
  standard 82 kg crewmember with a respiratory quotient of 0.86".
- The 2018 edition, Anderson, Ewert and Keener, NASA/TP-2015-218570/REV1 (January 2018),
  https://ntrs.nasa.gov/citations/20180001338, Table 3-33, read again with each row kept whole:
  "- m Carbon Dioxide Load kg/CM-d 1.04 (1)"; "+ m Oxygen Consumed kg/CM-d 0.816 (1)";
  "+ m Potable Water Consumed 78 kg/CM-d 2.5 (1)". The earlier reading's rows were right; its
  respiratory quotient of 0.92 confirms them.
- **Refutation pass 2026-10-30: CORRECTED** ([finding A1](../reference/findings/2026-10-30-quest-chain-facts-review.md#a1)): the 2018 table was read correctly, but NASA's 2022 edition supersedes it.

**A2. Outside air.** The monthly mean at Mauna Loa for September 2026 was 426.07 ppm of CO2
(0.043%).
- NOAA Global Monitoring Laboratory, "Trends in Atmospheric Carbon Dioxide",
  https://gml.noaa.gov/ccgg/trends/ (updated 2026-10-09). Quote: "426.07 ppm".
- **Refutation pass 2026-10-30: CONFIRMED** ([finding A2](../reference/findings/2026-10-30-quest-chain-facts-review.md#a2)).

**A3. How much CO2 is too much.** NIOSH's recommended limit is 5,000 ppm (0.5%) averaged over a
work shift and 30,000 ppm for 15 minutes; 40,000 ppm (4%) is immediately dangerous to life or
health; symptoms include headache, dizziness, restlessness and breathing difficulty.
- NIOSH Pocket Guide, "Carbon dioxide", https://www.cdc.gov/niosh/npg/npgd0103.html (last
  reviewed 2019-10-30). Quotes: "TWA 5000 ppm (9000 mg/m3) ST 30,000 ppm (54,000 mg/m3)"; "40,000
  ppm".
- **Refutation pass 2026-10-30: CONFIRMED** ([finding A3](../reference/findings/2026-10-30-quest-chain-facts-review.md#a3)).

**A4. Plants take up CO2 while lit.** The crop gas-exchange figures are the NASA measurements
already cited row by row in `data/life_support.ron`; not re-read in this pass.
- **Refutation pass 2026-10-30: UNCONFIRMED** ([finding A4](../reference/findings/2026-10-30-quest-chain-facts-review.md#a4)): not re-read in this pass.

**C1. Washing hands.** Wet, lather, scrub at least 20 seconds, rinse, dry; before, during and after
preparing food, before eating, after the toilet; soap and water is best, and a sanitiser should be
at least 60% alcohol.
- CDC, "About Handwashing", https://www.cdc.gov/clean-hands/about/index.html (dated 2024-02-16).
  Quotes: "for at least 20 seconds"; "contains at least 60% alcohol."; "Washing hands with soap and
  water is the best way to get rid of germs in most situations."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding C1](../reference/findings/2026-10-30-quest-chain-facts-review.md#c1)).

**C2. Sanitiser and dirty hands.** Hand sanitiser does not work as well on visibly dirty or greasy
hands, does not remove chemicals such as pesticides or lead, and does not kill every germ: soap and
water first.
- CDC, "About Hand Sanitizer", https://www.cdc.gov/clean-hands/about/hand-sanitizer.html (dated
  2024-03-12). Quotes: "Hand sanitizers may not be as effective when hands are visibly dirty or
  greasy."; "Hand sanitizers might not remove harmful chemicals, such as pesticides, and heavy
  metals like lead."; "Sanitizers do not get rid of all types of germs."
- **Refutation pass 2026-10-30: CONFIRMED** ([finding C2](../reference/findings/2026-10-30-quest-chain-facts-review.md#c2)): now from CDC's own page.

**C3. Urine on crops.** WHO's 2006 guidelines on excreta use set storage times and a gap between
fertilising with urine and harvest; the game's `data/garden/nutrients.ron` cites an earlier
reading. The WHO PDF refused automated reading today: re-read before this step ships.
- **Refutation pass 2026-10-30: UNCONFIRMED** ([finding C3](../reference/findings/2026-10-30-quest-chain-facts-review.md#c3)): WHO's PDF still refused.

**M1. Heavy bleeding.** Press directly on the wound with a sterile bandage, clean cloth or even
clothing until it stops; raise the part when you can; if a dressing soaks through, add another on
top and keep pressing. If pressure does not stop life-threatening bleeding from an arm or leg, a
tourniquet goes 2 to 3 inches above the wound, between the wound and the heart, never over a joint,
over clothing or padding, with the time noted for the responders. Get emergency help for severe
bleeding, and always after a tourniquet. **⚠ lethal, on screen.**
- MedlinePlus, "Bleeding", https://medlineplus.gov/ency/article/000045.htm (review date
  2025-10-14). Quotes: "Maintain pressure until the bleeding stops."; "Do not remove a dressing if
  it becomes soaked with blood."; "a tourniquet can be used until medical help arrives."
- DHS, "Stop the Bleed: Applying a Tourniquet",
  https://www.dhs.gov/sites/default/files/publications/STB_Applying_Tourniquet_08-06-2018_0.pdf
  (2018-08-06). Quotes: "place a tourniquet at least 2-3 inches from the wound"; "The tourniquet
  should be placed between the wound and the heart."; "Do not apply a tourniquet over a joint, such
  as an elbow, knee, wrist, or ankle."
- **Refutation pass 2026-10-30: CORRECTED** ([finding M1](../reference/findings/2026-10-30-quest-chain-facts-review.md#m1)): arm or leg only, padding, between the wound and the heart; the Stop the Bleed poster agrees.

**M2. Burns.** Cool a burn under cool running water for 20 minutes, never ice or ice water, and not
past 40 minutes; watch a large burn or a small child for getting too cold, and do not soak a severe
burn in cold water. Take off jewellery and clothing near the burn unless it is stuck. Never butter,
oil, ice or household remedies; do not break blisters; call the emergency number for a burn the size
of a palm or larger, a full-thickness burn, a chemical or electrical burn, signs of shock, smoke
breathed in, or when unsure. **⚠ serious.**
- MedlinePlus, "Burns", https://medlineplus.gov/ency/article/000030.htm (review date 2026-04-09).
  Quotes: "not ice water"; "at least 5 to 30 minutes"; "The burn is caused by chemicals or
  electricity."
- American Red Cross Scientific Advisory Council, "Burn Cooling" advisory (approved June 2019),
  https://www.redcross.org/content/dam/redcross/docs/Burn%20Cooling%E2%80%94ADVISORY.pdf: "for a
  minimum of 10 minutes, ideally 20 minutes"; it advises against cooling beyond 40 minutes.
- NHS, "Burns and scalds", https://www.nhs.uk/conditions/burns-and-scalds/ (last reviewed
  2026-03-31): "Hold the burn or scald under cool running water for 20 minutes."
- **Refutation pass 2026-10-30: CORRECTED** ([finding M2](../reference/findings/2026-10-30-quest-chain-facts-review.md#m2)): was "at least 5 to 30 minutes"; now 20 minutes, which fits MedlinePlus, the Red Cross and the NHS, and never past 40.

**R1. Sleep.** Adults 18 to 60 need 7 or more hours a night; 61 to 64, 7 to 9; 65 and older, 7 to 8.
(`systems::sleep` cites the AASM consensus, Watson et al. 2015, to the same 7 hours.)
- CDC, "About Sleep", https://www.cdc.gov/sleep/about/index.html (dated 2024-05-15). Quote: "7 or
  more hours".
- **Refutation pass 2026-10-30: CONFIRMED** ([finding R1](../reference/findings/2026-10-30-quest-chain-facts-review.md#r1)).

### 3.4 Where the sources disagree

Updated by the refutation pass ([finding](../reference/findings/2026-10-30-quest-chain-facts-review.md#where-the-sources-disagree-the-designs-34-updated)).

- **D1. Chlorine dioxide and Cryptosporidium.** CDC's emergency page: kills it if the instructions
  are followed. CDC's backcountry page: "somewhat effective". CDC's Yellow Book: kills it at
  practical doses and contact times of several hours, with few data across water conditions (W11).
  The game: partly, and only after the full wait.
- **D2. Bleach strength and dose. Reconciled.** CDC's own table puts its 8 drops a gallon at "½ mL"
  and "a little less than ⅛ teaspoon"; EPA puts 16 drops at 1/4 teaspoon. Ready.gov's 1/8 teaspoon
  (about 0.62 mL, our arithmetic) is about a quarter more than CDC's ½ mL: the same dose band. CDC and
  EPA double it for cloudy or very cold water; Ready.gov and EPA check it by smell (W9). The game and
  the how-to use CDC's.
- **D3. Altitude for the longer boil.** CDC 6,500 ft; EPA "5,000 feet (1,000 meters)", which do not
  match each other; resolved in `boil_water_safe` by boiling longer from 1,000 m, the most cautious.
- **D4. Daily water.** National Academies 2.7 and 3.7 L, adequate intakes with no upper limit set;
  EFSA 2.0 and 2.5 L; both all sources. The step text gives the range.
- **D5. Sand filters and viruses. Resolved.** CDC 2010: unknown effectiveness. The 2025 review's 4.5
  log is laboratory data (4.7); under typical household use, 1.7 log from 5 estimates (W15). The
  game: partly.
- **D6. Heat stroke's temperature.** CDC's sheet lists 103 °F (39.4 °C) or higher as a sign;
  MedlinePlus gives above 104 °F (40 °C); NIOSH says it can rise to 106 °F or higher within 10 to 15
  minutes. The game's `heat_stroke` status at a 40 °C core (`data/status_effects.csv`) matches
  MedlinePlus; whether to warn between 39.4 and 40 °C is the body-heat owner's call.
- **D7. Cooling a burn. Resolved** (M2). MedlinePlus at least 5 to 30 minutes, the Red Cross at least
  10 and ideally 20 and never past 40, the NHS 20: the game teaches 20. They differ on a severe burn:
  MedlinePlus says not to put it in cold water; the Red Cross cools every thermal burn and watches
  for hypothermia.
- **D8. Jerky temperatures. Resolved** (F8). 160 °F for meat (NCHFP, USDA) and 165 °F for poultry
  (USDA, through the University of Kentucky). `cook_dried_meat` was corrected in BUG-216.
- **D9. What a person breathes. Resolved** (A1). NASA's editions: 2004 (0.835 kg oxygen, 0.998 kg CO2
  on 11.82 MJ, 70 kg), 2018 (0.816, 1.04), 2022 (0.895, 1.085 on 12.78 MJ, 82 kg, daily exercise).
  The game's air loop scales the 2004 figures by food energy, and per MJ they are within 1% of the
  2022 edition (our arithmetic), so its balance holds; text a player reads quotes the 2022 edition.
  The quest's "close to a kilogram of each" is true of all three.
- **D10. A drink in a heat emergency (new).** CDC's sheet: nothing to drink in heat stroke, sip water
  in heat exhaustion. MedlinePlus: an alert person may sip, and nobody vomiting or unconscious gets
  anything by mouth. The game follows CDC for heat stroke and adds MedlinePlus's rule (H8).
- **D11. When a soaked dressing means a tourniquet (new).** MedlinePlus: add another dressing and keep
  pressing; a tourniquet only if continuous pressure fails on life-threatening bleeding. DHS's Stop
  the Bleed poster: if pressure does not stop it and the dressing soaks through, apply a tourniquet.
  Same order, a different trigger; the full 2024 AHA and Red Cross first aid guideline would settle
  it (3.6).
- **D12. Wild carrot's purple floret (new).** Missouri's poison hemlock page: wild carrot "has" one;
  its wild carrot page: "usually". The game teaches "usually", and never the floret on its own (F15).
- **D13. Distillation and chemicals (new).** Ready.gov: removes "most other chemicals". Nebraska
  Extension and Connecticut's health department: not the volatile ones, which boil over with the
  water (W21). The game: distillation never clears fuel or solvents.

### 3.5 Existing game data the sources do not support (fix with the rung that touches it)

- **`purify_water`** (`data/recipes.csv`) turns `water_bottle_0`, which the item list calls
  "Purified drinking water", into `water_purified_0`: purified water purified again. Its input
  should be untreated water (P5). *(Still open on 2026-10-30: BUG-216 left it for P5.)*
- **`cook_canned_food`** cans potatoes and carrots, both low-acid, "at" `stove_0`. F10: low-acid
  food only in a pressure canner. It needs `pressure_canner_0` and a time from a tested recipe.
  `cook_soup` and `cook_stew` produce `canned_food_0`, which a pot of soup is not. *(BUG-216,
  2026-10-30: `cook_canned_food`'s description now says a pressure canner is needed, the recipe
  still sits at `stove_0` until `pressure_canner_0` exists; `cook_soup` and `cook_stew` now make soup
  and stew.)*
- **`cook_dried_meat`**: the "165 F for wild game" in its text (D8). *(Fixed in BUG-216: it now says
  160 F, poultry 165 F, which F8 supports.)*
- **`water_tester_0`** "Water quality analysis probe": what it detects is not said; real home tests
  differ by what they test for (W18, 3.6).
- **The `cold` storage zone** (`data/food_system.ron` `temperature_zones`) is described as
  "Refrigerator range" and runs from -1 to 8 °C. F5: a refrigerator at or below 4 °C (40 °F). The
  zone is a spoilage-rate band, so its upper edge may be a modelling choice, but the words teach 8 °C
  as a fridge; split the band at 4 °C or change the description. *(BUG-216 changed the description
  to say a real refrigerator holds 4 °C or below; the band is unchanged.)*

### 3.6 Still to find out (no number goes in until it is sourced and dated)

Updated by the refutation pass ([finding](../reference/findings/2026-10-30-quest-chain-facts-review.md#what-is-still-unknown-and-what-would-settle-it)).
Settled and removed from this list: iodine's medical limits (W10), distillation and chemicals (W21,
refuted), hand sanitiser on dirty or greasy hands (C2), the other burn-cooling times (M2), and the
poultry jerky temperature (F8). FSIS's and FoodSafety.gov's own charts were not needed: CDC's page
carries the list (F3).

- A solar still's yield per day (W22). Searched again: only a July 2025 medRxiv preprint, not peer
  reviewed (about 0.3 to 0.6 L a day), which is a lead to its cited sources. Jackson and van Bavel
  (Science, 1965, science.org refuses automated reading) and the US Army survival manual ATP 3-50.21
  would settle it, read by hand.
- WHO's 2006 excreta guidelines (C3): the PDF still refuses automated reading; read by hand before
  `clean_compost` step 3 ships.
- The crop gas-exchange citations in `data/life_support.ron` (A4): re-read before `air_plants`.
- The full 2024 AHA and American Red Cross Guidelines for First Aid (Circulation, doi
  10.1161/CIR.0000000000001281; refused): settles D11 and confirms the burn-cooling minimum.
- A number for chlorine dioxide's wait (W11): the Yellow Book says "several hours"; a product label
  or its EPA registration gives the recipe's figure.
- What clay body and firing a clay pot filter needs, and how long a sand filter's biological layer
  takes to mature.
- What home water test kits detect (coliform bacteria kits, test strips) and how long they take.
  CDC's rainwater page says to ask the local health department what to test for.
- Salt curing: USDA FSIS's pages refused automated reading; read them by hand for `food_keep_salt`.
- General drying times and temperatures for fruit and vegetables from NCHFP or USDA (F7).
- A home CO2 comfort target (A3 is a workplace limit); ASHRAE or a health agency.
- How much water is too much (water intoxication), for the hydration chain's notice. The National
  Academies set no upper limit for water; a clinical source on hyponatremia gives the warning signs.
- The illness and its odds from drinking untreated water of each kind: these are **game choices**
  where no source gives a per-drink chance, labelled so in the data, as the food poisoning odds
  are today.
- Where the ship's condensate stands as drinking water (the condensate source in 2.1): what is in
  air-handler condensate, from a primary source, before it is set as a source's contents.

### 3.7 What the refutation pass changes outside this section

Section 2 and the top of this document were written before the pass; these lines follow from the
corrected facts and are the designer's to make (the pass edited section 3 only):

1. The status line at the top of this document still says every fact needs a refutation pass.
2. `water_store`'s note calls CDC's three-day store "for drinking and sanitation": CDC says drinking,
   cooking and brushing teeth (W26).
3. `water_disinfect` step 3 ("2 drops, wait 30 minutes") holds for clear water; the recipe and the
   how-to need the double dose for cloudy or very cold water (W9).
4. `water_distil` and P5: distillation clears germs, salt and metals, never fuel or solvents; the
   treatment table's cell for distillation against fuel and solvents is "no", not "partly" (W21).
5. `water_filter_ceramic`: its recipe must not clear viruses (1.3 to 1.9 log); how much of the
   bacteria and parasites it clears is a game choice from the review's Table 1, labelled so (W14).
6. `aid_burn` step 2 should name the 20 minutes (M2).
7. `warm_alarm` step 3 counts floors; USFA also asks for one in every bedroom and outside each
   sleeping area (H4).
8. `food_forage` step 3 should teach a player to leave both plants alone rather than to pick wild
   carrot: Missouri's advice for the inexperienced is to treat the whole family as deadly, and a smell
   test means crushing a plant whose sap blisters (F15). A judgement, recorded as one in the finding.
9. Text a player reads that quotes NASA's figures (`water_reclaim`, `air_first`) uses the 2022
   edition (A1, W23).

## 4. The engine and content work list, in build order

### 4.1 The first chain to build: Water, rungs 0 and 1

Water first, for three reasons. Dehydration is the deadliest clock in the game and in life (the
game empties Hydration in two days and kills in about one more, `food.rs`). The meter, the drinking,
the vessels and a sourced how-to (`boil_water_safe`) already exist. And boiling is the one method
whose game result can be made exactly as safe as life's, so the first slice teaches nothing false.

### 4.2 The engine pieces

| Id | Piece | Where | Rung |
|---|---|---|---|
| **P1** | `after: None / Quest / AnyOf / AllOf` replaces `prerequisite` | `quests/mod.rs`, every `data/quests/*.ron` rewritten in the same commit (no shim) | 0 |
| **P2** | The tracked quest: `QuestTracker.tracked`, Track button, HUD reads it | `quests/mod.rs`, `gui/pages/quests.rs`, `hud.rs` | 0 |
| **E1** | `Drink` objective and the `drink_` event | `objectives.rs`, `food.rs` | 0 |
| **E2** | `Keep` objective, `data/quests/readings.ron`, the `vital.*` readings, the game-clock streak | `objectives.rs`, `quests/mod.rs` (query `Vitals` with the tracker) | 0 |
| **E3** | `Learn` objective, `learn_` events from the How-to page and the Library reader | `objectives.rs`, `gui/pages/how_to/`, the Library reader | 0 |
| **P3** | The intake ledger: 24 hourly buckets of water, kcal and vitamin C taken in, saved with the player; the `intake.*` readings | `food.rs` (updated where food and drink are consumed), save | 1 (water), 2 (kcal), 4 (vitamin C) |
| **P5** | **Water contents**: what is in a stack of water (cloudiness, bacteria, viruses, Giardia, Cryptosporidium, salt, metals, fuel and solvents), carried with the stack the way food's age clock is, blended worst-first when stacks join; tanks hold contents too; sources set them; a treatment recipe names a method in `data/water/treatment_methods.ron` (what it removes, per class, each cell cited to section 3); drinking rolls the illness for what is left (game odds, labelled); the card shows "Untreated", "Boiled", "Filtered", and what a tester has found; a plain toggle shows contents without a tester (the gear-gated rule, CLAUDE.md). The `stock.*` readings. | `systems/fluids.rs`, `inventory`, `crafting`, `illness`, `data/water/`, `data/medical/illnesses.ron` (a `waterborne_illness` row) | 1 (with only boil and distil, which clear every germ class), 3 (every other method) |
| **E4** | `Use` objective: `use_<item>` from `treatment` and tool uses, `use_<provides>` from `built_uses` | `objectives.rs`, `treatment.rs`, `engine/built_uses.rs` | 2 |
| **E5** | `Sleep` objective, `sleep_<h>` on waking | `objectives.rs`, `sleep.rs` | 2 |
| **P8** | The Compost action reports `make_` events for what it makes | `lib.rs` compost bridge (or wherever it moves) | 2 |
| **P10** | `starts` on `QuestDef` and the per-start test | `quests/mod.rs`, `opening_tests.rs` | 2 |
| **E6** | `Store` objective, the storage-zone event | `objectives.rs`, inventory moves | 3 |
| **P6** | The sun-timed process: a recipe whose timer advances only while its spot gets direct sun (the weather's cloud and the sun model the solar panels already read) | `crafting`, weather | 4 |
| **P7** | Long room-temperature processes: a crock's clock on game time, its room's temperature gating it | `crafting`, home air temperature | 4 |
| **P9** | Identification: plant cards show their features; picking which plant it is; a wrong pick in Realistic mode poisons | flora, items, illness | 6 |

### 4.3 The rungs

Each rung is shippable on its own, and **each rung's facts get their refutation pass before its
data is written** (a critic agent against the section 3 sources; merged results move the fact to
verified). Each new objective kind ships with a test made to fail first (a `Keep` streak that does
not reset when the reading leaves its band; a `Drink` that counts eating), per
`feedback_checks_that_cannot_fail`.

- **Rung 0, the plumbing.** P1, P2, E1, E2 (vital readings), E3. Rewrite the existing quest files
  to `after`. Tests: the AnyOf join, the tracked HUD line, each kind red then green.
- **Rung 1, Water.** P3 (water), P5 (contents, two methods), the raw sources (cistern raw tap,
  ship condensate drain), the `waterborne_illness` row, recipes `settle_water`, `boil_water`,
  `distil_water_pot`, fix `purify_water`. Quests `water_first`, `water_unsafe` (without the tester
  step), `water_boil`, `water_distil` (pot still only), `water_store` (without the sanitise step).
  How-tos: new `drink_enough_water`, new `distil_water_pot`; give `boil_water_safe` the `sim`
  reality.
- **Rung 2, the body's other meters.** E4, E5, P8, P10, P3 (kcal). Quests `food_first`,
  `food_grow` (and Start Farming's AnyOf), `food_keep` (Learn only, without the Store step),
  `food_keep_dry`, `rest_first`, `clean_first`, `clean_compost`, `aid_first`, `aid_wound`,
  `aid_rehydrate`. How-tos: new `wash_your_hands`, `dry_jerky_safely`; planned
  `stop_heavy_bleeding`, `start_a_compost_pile`. Fix `cook_dried_meat`'s text.
- **Rung 3, more ways to make water safe.** P5's other methods; items `bleach_0`,
  `chlorine_dioxide_tablet_0`, `water_filter_portable_0`; the water tester's test (and its
  finding, 3.6); recipes `disinfect_water_bleach`, `disinfect_water_clo2`, `filter_water_portable`,
  `sanitise_container`; blueprint `rain_barrel`. Quests `water_disinfect`, `water_filter`,
  `water_rain`, the tester step of `water_unsafe`, the sanitise step of `water_store`. E6 and the
  Store steps. The warmth chain: `warm_first`, `warm_fire`, `warm_alarm`, `warm_heat_safely`
  (items `co_alarm_0`, recipe `assemble_co_alarm`), `warm_cool`. How-tos: planned
  `disinfect_water_with_bleach`, `clean_a_water_container`, `collect_rainwater_first_flush`,
  `use_a_fire_extinguisher`, `test_smoke_alarms`, `test_co_alarm`, `never_mix_cleaners`; new
  `disinfect_water_with_tablets`, `filter_water_portable`, `warm_someone_with_hypothermia`,
  `cool_someone_with_heat_stroke`.
- **Rung 4, the sun, the clay and the crock.** P6, P7, P3 (vitamin C). Items `pet_bottle_clear_0`,
  `uv_purifier_portable_0`, `ceramic_water_filter_0`, `fermenting_crock_0`, `sauerkraut_0` (with
  its food profile), `food_thermometer_0`; blueprints `slow_sand_filter`, `solar_still` (when its
  yield is sourced). Quests `water_sun`, `water_filter_ceramic`, `water_filter_sand`, the solar
  still steps of `water_distil`, `food_keep_ferment`, `food_cook`, `food_balance`. How-tos: new
  `disinfect_water_with_sunlight`, `make_a_clay_pot_filter`, `build_a_sand_filter`,
  `build_a_solar_still`, `make_sauerkraut`, `cook_meat_to_a_safe_temperature`.
- **Rung 5, machines and the sealed home.** Machines `ro_unit`, `water_reclaimer`; the `home.*`
  readings; the CO2 view key. Quests `water_ro`, `water_reclaim`, `air_first`, `air_scrub`,
  `air_plants`, `power_backup` (its placement rule), `aid_burn` (the cooling treatment row),
  `food_keep_can` (items `pressure_canner_0`, `water_bath_canner_0`; fix `cook_canned_food`,
  `cook_soup`, `cook_stew`; recipes `can_fruit_water_bath`, `can_vegetables_pressure`), and the
  capstone `self_reliant`. How-tos: planned `run_a_generator_safely`, `cool_a_burn`,
  `can_fruit_water_bath`, `dispose_of_a_spoiled_jar`; new `pressure_can_vegetables`.
- **Rung 6, wild food.** P9, its own species refutation pass (toxicity and lookalikes first),
  then `food_forage` and the how-to `tell_hemlock_from_wild_carrot`.

### 4.4 Calls made here (each can be overruled in one line)

1. Chain roots are offered on the Quests page, not chained after the openings (1.3).
2. `after` replaces `prerequisite` outright, with `AnyOf` and `AllOf` (1.4).
3. Keep streaks run on the game clock, reset when the reading leaves its band, and pause while the
   game is not running (2.10).
4. Water quality is carried by the water itself (P5), not by separate "untreated" items: one fluid
   per vessel stays true, and what is in it is the stack's state, the way food's age is. Rung 1
   builds this real model with two methods rather than a placeholder item to be thrown away.
5. A treatment joins the game only when its game result is no safer than life's (1.7).
6. The reclaimer makes the urine trade-off (water or nitrogen) the player's choice on its card
   (2.1, `water_reclaim`).
7. Wild food is built last, after its own refutation pass (2.2).
