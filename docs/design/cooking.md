# Cooking: heat, time and doneness

**Status:** rung 1 built 2026-11-21 (this document's section 4). The whole plan is here so each
rung is a rung of one system, not a throwaway (CLAUDE.md, "Plan the whole feature, build it in
rungs"). Activity `cooking` in [activities.md](activities.md) section 4 (item 1, the first activity
after the operator confirmed the order principle on 2026-11-14: "Sounds fine. They all need to be
developed eventually."). The teaching chain is Cook It Through in
[quest-chains.md](quest-chains.md) section 2.2.

## 0. In plain words

Cooking used to be a menu click: press Cook, wait a few seconds, get a dish. Now a dish has a
**method** (boiling, frying, baking, roasting, grilling, steaming), you choose a **heat** and a
**time**, and what comes out follows from them: **undercooked**, **done**, **overcooked** or
**burnt**. Meat, eggs and dry beans carry a real risk until they reach a safe temperature (or, for
beans, a full boil for long enough); undercooking leaves that risk in the dish, burning loses the
food, and overcooking costs vitamins. A food thermometer shows what is happening inside the food,
which is the real lesson: CDC, "The only way to tell if food is safely cooked is to use a food
thermometer."

Two modes and a middle one (Settings > Gameplay > Cooking, CLAUDE.md's dual-modes rule):

| Level | What you do | What you see |
|---|---|---|
| **Simplified** (the default) | Nothing to choose: the station picks a sensible heat and time | It always comes out done and safe |
| **Guided** | Choose the heat and the time | The temperature inside the food, as a thermometer would show it, whether or not you carry one (the gear-gated rule's plain toggle) |
| **Realistic** | Choose the heat and the time | The temperature inside only with a food thermometer in your backpack; without one, only the heat and the time |

## 1. The model (data: `data/food_system.ron`, code: `crates/humanity-systems/src/cooking.rs`)

**The heat around the food.** Each cooking method in `cooking_methods` names the temperature of
what surrounds the food at each heat (`medium_c`: low, medium, high) and how fast that medium
gets heat into food (`heat_transfer`, boiling water = 1.0). Boiling water stays at 100 C at
sea-level pressure however high the burner is turned, so turning up the heat under a pot of
water does not make it hotter, while a pan or an oven does get hotter. The other medium
temperatures are typical kitchen values, a game choice labelled so in the data.

**The temperature inside.** Each food class in `food_classes` starts at `start_c` (4 C from the
fridge, 20 C from the pantry) and heats toward the medium on a first-order curve with time
constant `heat_up_min / heat_transfer`:

    core(t) = medium - (medium - start) * exp(-t / tau)

so a thick piece in hot air (baking) heats far slower than a thin one in boiling water, and a
medium cooler than the food's done temperature never gets it there, however long it cooks.

**Doneness.** The dish is ready `tender_min` minutes after its core reaches `done_c` (0 for meat
and eggs, longer for beans, rice and vegetables, which soften with time). Before that it is
undercooked. After it, it is overcooked once `overcook_after_min` more have passed, and burnt
once `burn_after_min` have passed at a medium of `burns_above_c` or more (boiling and steaming
never burn the food; a dry pan or an oven can).

**Safety** is separate from doneness, because a dish can be soft and still unsafe. A class with a
safety line is safe only once its core has reached `safe.core_c` (sourced: CDC's safe internal
temperatures), or, for dry beans, only after `safe.boil_min` minutes at a full boil (FDA: boil
for at least 30 minutes; a slow cooker's 75 C or less does not destroy the toxin). FDA gives no
safe line below the boil, so the game counts beans simmered below a full boil as unsafe, however
soft: a cautious game choice, not FDA's finding. Dough, batter and dishes with raw egg in them are
safe once done (CDC lists "Raw dough or raw batter made with uncooked flour" and undercooked eggs
among its riskier foods, with no temperature), and undercooked keep their raw flour's or egg's
risk. Vegetables and grains carry no cooking safety line.

**What the dish keeps.** A done dish is its own nutrition profile (USDA's cooked rows, which
already carry the losses of a proper cook). An undercooked unsafe dish keeps the risk of its raw
food (`raw_profile`'s `raw_consumption_risk`, the game's existing odds) and the player falls ill
on that chance when it is eaten. An overcooked dish loses its heat-sensitive vitamins a second
time: vitamin C and the B group are multiplied by the class's `keeps` again, USDA's retention
factor for that food and method (Release 6, 2007, cited per row), on top of the proper cook the
profile already carries. A game model built on a sourced figure, said so in the data; for the B
vitamins USDA's own table shows longer cooking keeping less (legumes cooked 15 to 20 minutes keep
60% of their folate, 45 to 75 minutes 45%), while legumes' vitamin C holds at 65% from 15 minutes
to 2.5 hours. Beans use that sourced ratio directly (2 to 2.5 hours against 45 to 75 minutes). The intake ledger (`intake.rs`) counts what was eaten after these changes.
A burnt dish is lost: the ingredients are spent and nothing edible comes out.

**Meal quality** (`meal_quality_levels`, loaded and unused until now): a done dish is graded by
the cook's level in the recipe's skill (the highest level whose `min_cook_skill` the cook has); an
overcooked dish is at most `poor`, an undercooked one `terrible`. Eating it rolls each of its
`status_effects` at `status_effect_chance` (well fed, inspired, a strength boost), which is what
the data says a good meal does for you. Its morale, health and reputation numbers wait for systems
that have those (no morale meter exists).

**Where the state lives.** A cooked dish's doneness, safety and meal grade ride on the stack's
grade byte (`ItemStack::quality`), which food never used (only durable goods are graded, and no
food is durable): `cooking::DishState::{to_byte, from_byte}`. So stacks of the same dish only
merge when they came out the same, and the state goes everywhere a stack goes (storage, saves,
the death pack) with nothing new to carry. 0 means "not cooked by hand" (bought or machine-made),
which eats as the profile says.

**Station heat.** A dish is cooked at its recipe's station (the stove, the oven) and the station
must be powered, as before. Rung 1 offers every station the three heat levels; rung 2 makes the
station's own watts or fuel set how fast the medium comes up.

## 2. Budget

Rung 1 is event-driven: the outcome is one closed-form evaluation per craft (an `exp` and a `ln`),
and the cooking panel evaluates it only on frames it is drawn. Per-frame cost with nothing being
cooked: zero. Network: none; a dish is made into the player's own backpack, and on a server world
crafting is the player's own as it always was. Rung 2's live pot is one curve per cooking pot,
stepped on the game clock with the crafts (a few hundred nanoseconds each), and still never
networked (cosmetic steam and sizzle are local, CLAUDE.md's compute budget).

## 3. The rungs

| Rung | What it adds | Waits for |
|---|---|---|
| **1 (built)** | Method, heat and time; doneness from the core-temperature model per food class; safety lines from CDC and FDA; undercooked keeps the risk, burnt loses the food, overcooked loses vitamins (USDA retention); meal quality acted on; the three levels; the food thermometer; the `Cook` quest objective; Cook It Through with what exists | |
| **2** | The pot on the burner on the game clock: a dish cooks while you walk away and burns if forgotten; the thermometer reads live; the station's watts or a campfire's fuel set the heat; resting meat (CDC's 3 minutes) as a step | the station heat model, the crafting clock per station |
| **3** | Choose the method per dish (steam the vegetables instead of boiling them) with USDA's retention per method; water and oil as measured ingredients; the boiling point falling with air pressure (the ship's air, the mountains); protein made available by cooking (egg protein digestibility) | sourced figures for each, a refutation pass |
| **4** | Leftovers: the fridge within 2 hours (CDC), reheating to 165 F; E4 (Use: washing hands at the sink) and E6 (Store) | quest-chains.md E4, E6 |
| **5** | Hands-on procedure (chop, stir, knead, prove), the mess hall's cooks cooking by this model (the Background mode), recipes of several stages | the activity minigame pattern |
| **6** | The real-life twin: a meal and food-safety log in the app (two-realities.md) | the Home trackers |

## 4. Rung 1 as built

- Data: `cooking_methods` typed (`medium_c`, `heat_transfer`; the unsourced
  `nutrition_modifier`, `taste_modifier`, `preservation_modifier`, `cooking_time_multiplier` and
  `food_safety_risk` removed, since nothing ever read them and the model replaces them, and
  `roasted` added), `meal_quality_levels` typed (the `terrible` level's food poisoning removed:
  an undercooked dish's risk is now the model's, from its raw food), and two new tables,
  `food_classes` and `dishes` (one row per hand-cooked recipe: method, class, the item it makes,
  and the heat the station picks in Simplified). Recipes that are not a heat procedure (pressing
  juice, churning butter, blending spice, drying and canning, which are preserving's) are listed
  in `not_dishes` with the reason, so a new stove recipe has to be decided.
- New items: `food_thermometer_0` (in the homestead kit and sold at the trading post),
  `egg_boiled_0` (profile `eggs_cooked`) and `beans_cooked_0` (profile `legumes_cooked`, USDA SR
  Legacy NDB 16028). New recipes: `cook_boiled_eggs`, `cook_beans`, `cook_fish` (raw fish from the
  trading post), `cook_roast_chicken`. The homestead kit carries six eggs.
- The Crafting page's cooking card (`src/gui/pages/cooking_panel.rs`); the web Crafting page shows
  each dish's method and its safety line from the same data.
- The `Cook` objective (`quests/objectives.rs`): a dish that came out done or overcooked AND safe
  reports `cooked_<item>`.

The sources and the refutation pass: `docs/reference/findings/2026-11-21-cooking-safety-facts.md`.
