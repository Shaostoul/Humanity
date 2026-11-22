# Preserving: drying, canning and salting against spoilage

**Status:** rung 1 built 2026-11-21 (this document's section 4). The whole plan is here so each rung
is a rung of one system, not a throwaway (CLAUDE.md, "Plan the whole feature, build it in rungs").
Activity `preserving` in [activities.md](activities.md) section 4 (item 3, after cooking and the
skills wiring, under the order principle the operator confirmed on 2026-11-14: "Sounds fine. They
all need to be developed eventually."). The teaching chains are Keep It and its branches in
[quest-chains.md](quest-chains.md) section 2.2. Shaped like [cooking.md](cooking.md).

## 0. In plain words

Spoilage already runs: every food carries a clock that the temperature where it is kept speeds up
or slows down, and food past its time poisons whoever eats it. Until now the only answers were to
eat it or sell it. Preserving is the third answer: **dry** it, **can** it or (rung 2) **salt** it,
and it keeps for months instead of days.

Each is a procedure with a right way and wrong ways, as the sources teach it:

- **Drying** takes water out. Fruit is dry at about 20% water, vegetables at about 10% and brittle
  (NCHFP). An oven or dehydrator at 60 C (140 F) dries apple rings in 6 to 24 hours; the sun dries
  fruit only, only on days of 29 C (85 F) or more, and takes several days. Food taken off before it
  is dry goes mouldy as fast as fresh food.
- **Canning** heats food sealed in jars. Acid food (most fruit) is safe in a boiling-water canner for
  the tested time. Low-acid food (every fresh vegetable, meat, fish) is safe ONLY in a pressure
  canner, vented 10 minutes, at the tested pressure for the tested time: botulinum spores survive
  boiling water. A low-acid jar canned any other way carries a **botulism** risk, and the game says
  so on the jar, because in real life the toxin "cannot be seen, smelled or tasted" (CDC).
- **Salting** draws water out with salt by weight. The one complete, sourced home procedure found is
  a dry-cured country ham (8% salt by green weight, below 4 C for about 60 days, then months to
  equalise): it needs the long process clock, so it is rung 2 (section 3).

Three levels (Settings > Gameplay > Preserving, CLAUDE.md's dual-modes rule):

| Level | What you do | What you see |
|---|---|---|
| **Simplified** (the default) | Nothing to choose: the station picks the tested method and time | It always comes out safe |
| **Guided** | Choose the method, the time, and for a pressure canner the weight and the venting | Before you start: whether it is the tested process, and how it will come out |
| **Realistic** | The same choices | The tested table for the food as the guide prints it, every altitude and jar size; how your choice came out only when it comes out |

## 1. The model (data: `data/food_system.ron` `preserving`, code: `crates/humanity-systems/src/preserving.rs`)

**Drying.** Each drying procedure names its food's water at the start (`water_start`, USDA's raw row)
and its drying class, whose `water_done` is the source's dryness (fruit 0.20, vegetables 0.10,
NCHFP). Water leaves on the thin-layer drying curve (Lewis's first-order model, on the dry basis,
water per gram of solids):

    X(t) = X0 * exp(-k * t),   water share = X / (1 + X)

`k` per hour in an oven or dehydrator at 60 C is per procedure (slice thickness), a game choice
sized so the time to dry falls inside the source's range (apples 6 to 24 hours, carrots 6 to 10).
The sun dries at `rate_share` of that (a game choice, about three days for apples: "several days")
and only food its `classes` allow (fruit: "Vegetables ... and meats are not recommended for
out-of-doors drying"), and only in air of `min_air_c` (29.4 C, 85 F) or warmer, judged by the air
around the person when the drying starts. Dry when the water share is at or below `water_done`;
otherwise it **spoils** as its fresh food would. The weight that comes out follows the water:
`out = in * (1 - water_start) / (1 - water_done)`, and the recipe's item counts are sized to it (the
test holds them within one item).

**Canning.** Each canning procedure names whether its food is acid (pH 4.6 or lower: most fruit) and
its **tested processes**, copied from the USDA guide's tables as NCHFP prints them: per canner
(boiling water, weighted-gauge pressure, dial-gauge pressure), the minutes per jar size and the
altitude bands, and for pressure the pounds per altitude band. The game reads the band for the
home's elevation (0 ft aboard: the home's air is at sea-level pressure; rung 3 reads a planet's
ground), and the card shows the whole table, every band and jar size, because the right number
depends on both. A jar comes out:

- **safe** when its canner has a tested process for the food, the minutes reach it, and for a
  pressure canner the canner was vented 10 minutes and the weight reaches the band's pounds;
- **botulism risk** when the food is low-acid and it is not safe (no tested boiling-water process
  exists for a low-acid food: "Do not use a boiling water canner for low-acid foods because it will
  not protect against botulism", CDC);
- **spoils** when the food is acid and it is not safe (under-processed acid food may spoil; it is
  not a botulism risk at pH 4.6 or lower).

**Where the state lives.** A preserved food's state rides on its stack's grade byte
(`ItemStack::quality`), as a cooked dish's does: `preserving::PreserveState::{to_byte, from_byte}`,
0x41 safe, 0x42 spoils, 0x43 botulism (0 for a bought jar, which eats as its profile says). Stacks
merge only when they came out the same, and the state goes wherever the stack goes.

**What preserving changes.** A safe preserved food is its own item with its own USDA profile and its
own clock: dried apples keep 6 months at room temperature (NCHFP: "1 year at 60ºF, 6 months at
80ºF", so the cool zone's slower clock nearly matches the 60 F figure), dried carrots half that
("about half the shelf-life of fruits"), a home-canned jar a year (the guide's "within a year", a
quality limit: a sealed, properly processed jar does not spoil below 35 C, 95 F). A jar that spoils
keeps only as long as its fresh food. Eating a botulism jar gives Botulism on `botulism_risk`, a
game chance: no source gives a chance per jar, and "If in doubt, throw it out!" (CDC). Its course
and weakness are game choices in `data/status_effects.csv`, said so there. Eating a jar hands the
empty jar back.

**Botulism by mode, and the medbay (the operator's call, 2026-11-21).** "In Realistic, botulism is
fatal unless treated at a medbay; in Simplified it is serious but survivable" (his "Yes" to the
orchestrator's recommendation; Simplified is the Illness and injury setting's Forgiving). Its harm
is the illness's own row in `data/medical/illnesses.ron`, by mode: 1 health an hour in Realistic, so a
full body's 100 health is gone in about 4 of the 7 game days and healing stops while ill; in
Forgiving half that over half the course, never below a quarter of full health. Treated means
reached: the player stands in a medbay (a room of the `medbay` type: General Services' medbay today;
the Medical Bay district is a block on the map with no rooms yet, and becomes a medbay by the same
room type the day it gets an interior) and chooses Get treated (the Inventory page's Status section, or [E] there);
asked anywhere else, the words say where to go (`systems::medbay`, `engine/medbay.rs`,
`gui/pages/medbay.rs`). Its words are by key (`illness.botulism.*`, `medbay.*`): what it does in
this mode, and that a medbay treats it. The numbers are the game's, labelled so; the chance per jar
(0.5) and the 7-day course were kept as built (the orchestrator, 2026-11-21).

**The canners are kept at home (2026-11-21).** A 20 L boiling-water canner and a 22 L pressure
canner are 42 L of the 65 L backpack: in the homestead kit they filled it to the litre, and the soak's
person, who could then take no food from storage, starved (a player would have met the same full
pack). Rather than carry them, the canning station holds its canner: `data/crafting/tools.ron`
`at_home` names the tools a craft takes from the home's storage wherever that storage counts for a
craft (BUG-147's reach, as the craft's ingredients), and from the backpack anywhere. The homestead's
canners start in the Barn (`data/places/seed.json`). A canner used from storage is not worn yet
(rung 2). The Crafting page says where each tool is ("in home storage (it stays at home)"), and the
website says "(kept at home)".

## 2. Budget

Rung 1 is event-driven like cooking's: one closed-form evaluation per craft (an `exp` and a `ln` for
drying, a table lookup for canning), and the preserving card evaluates it only on frames it is drawn.
Per frame with nothing being preserved: zero. Network: none; a preserved food is made into the
player's own backpack. Rung 2's long processes (the dehydrator overnight, the ham's 60 days) are one
clock per batch stepped with the crafts, still never networked.

## 3. The rungs

| Rung | What it adds | Waits for |
|---|---|---|
| **1 (built)** | Drying (oven or sun) on the water curve; water-bath and pressure canning against the tested tables; botulism by mode (fatal in Realistic unless a medbay treats it, survivable in Forgiving) and the medbay's treatment; the canners kept at home; the three levels; new items and profiles; Dry It and Can It | |
| **2 (next)** | **Salting first** (the orchestrator, 2026-11-21): the country ham on the long process clock (quest-chains.md P7), with salt measured in grams (8 lb salt and 2 lb sugar per 100 lb, below 40 F, 2 days a pound or 60 days, at least 4% salt and 18% weight lost, University of Kentucky ASC-213) and Salt It. Then on the same clock: drying and canning as a batch you leave and come back to; Ferment It (F9's sauerkraut); conditioning dried fruit 7 to 10 days; botulism's 18 to 36 hour onset; a canner used from home storage worn as a carried one is | P7, a measured salt unit |
| **3** | Altitude from the ground (a planet's elevation changes the band); quarts and half-pints as jar items (half-pints take the pint time); new lids each time; seal testing (the how-to exists) and the unsealed jar (eat within several days, or reprocess within 24 hours only under the guide's conditions) | the jar and lid items, the world's elevation |
| **4** | Smoking, freezing as a procedure, pickling with vinegar, jams and jellies; each with its own sourced tables and a refutation pass | sources |
| **5** | Hands-on procedure (slice, blanch, fill to the headspace, watch the gauge) in the activity minigame pattern | the minigame pattern |
| **6** | The real-life twin: a pantry log of what you canned and dried, and when to use it (two-realities.md) | the Home trackers |

## 4. Rung 1 as built

- Data: food_system.ron's unsourced `preservation_methods` (shelf-life and nutrition multipliers no
  source supported and no code read) replaced by `preserving` (drying methods and classes, the
  canning procedures with their tested tables, the drying procedures, `botulism_risk`), every figure
  with its source and the refutation pass's verdict beside it.
- New items: `dried_apples_0`, `dried_carrots_0`, `applesauce_jar_0`, `canned_carrots_0`,
  `canned_potatoes_0` (each with a USDA profile and portion), `water_bath_canner_0` and
  `pressure_canner_0` (tools at the stove kept at home, data/crafting/tools.ron `at_home`; in the
  homestead's Barn and at the trading post). New recipes: Dry Apples, Dry Carrots (oven), Can Applesauce, Can Carrots, Can
  Potatoes (stove). `cook_dried_fruit` became Dry Apples (the no-legacy-names rule) and
  `cook_canned_food` (potatoes and carrots at a bare stove) became the two pressure-canned recipes.
- The Crafting page's preserving card (`src/gui/pages/preserving_panel.rs`); the web Crafting page
  shows each procedure's method and its tested process from the same data.
- The `Preserve` objective (`quests/objectives.rs`): a preserved food that came out safe reports
  `preserved_<item>`.
- Dry It and Can It after Keep It. Salt It and Ferment It wait for rung 2 (P7); Dry It's jerky step
  waits for raw mutton to be obtainable (no trading post sells it).

The sources and the refutation pass: `docs/reference/findings/2026-11-21-preserving-safety-facts.md`.
